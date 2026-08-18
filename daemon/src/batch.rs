// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The contract's batch rows → the client's, and its answers back.
//!
//! # Per-row policies inherit, they do not replace
//!
//! The client's per-record batch policies are concrete structs, not the
//! all-optional shape the contract uses: `BatchWritePolicy` has a
//! `RecordExistsAction`, not an `Option<RecordExistsAction>`. So each row starts
//! from the client's `Default` and overrides only the fields the row actually
//! set — which is what "unset means leave it alone" has to mean here, since
//! there is nothing else for it to inherit from.
//!
//! # A failed row is not a failed batch
//!
//! Translating answers back is where that shows: a row with a result code
//! becomes a [`WireBatchResult`] carrying it, and the batch as a whole still
//! succeeds. The only failures that belong to the *batch* are the ones that stop
//! it being sent at all.

use aerospike_core::{
    BatchDeletePolicy, BatchOperation, BatchReadPolicy, BatchRecord, BatchUDFPolicy,
    BatchWritePolicy, Bins, Value,
};
use aerospike_php_ipc::batch::{
    WireBatchDeletePolicy, WireBatchKey, WireBatchReadPolicy, WireBatchResult, WireBatchRow,
    WireBatchUdfPolicy, WireBatchWritePolicy,
};
use aerospike_php_ipc::op::WireExpression;
use aerospike_php_ipc::{BinSelector, RecordBody, TTL_NEVER_EXPIRES};

use crate::convert;
use crate::ops::{self, OpError};
use crate::policy::Capabilities;

/// Build the client's batch operations from the contract's rows, in order.
///
/// # Errors
/// [`OpError`] for a row whose key, values or operations cannot be built.
pub fn to_operations(
    rows: &[WireBatchRow],
    caps: &Capabilities,
) -> Result<Vec<BatchOperation>, OpError> {
    rows.iter().map(|row| to_operation(row, caps)).collect()
}

/// Build one row.
///
/// # Errors
/// [`OpError`], as [`to_operations`].
pub fn to_operation(row: &WireBatchRow, caps: &Capabilities) -> Result<BatchOperation, OpError> {
    let key = to_key(row.key())?;

    Ok(match row {
        WireBatchRow::Read { bins, policy, .. } => {
            let bins = match bins {
                BinSelector::All => Bins::All,
                BinSelector::None => Bins::None,
                BinSelector::Only(names) => Bins::Some(names.clone()),
            };
            BatchOperation::read(&read_policy(policy, caps)?, key, bins)
        }
        WireBatchRow::ReadOps { ops, policy, .. } => {
            // Every operation here must be a read: a write in a read row would
            // be sent as a read and quietly do nothing. Refused rather than
            // reordered into a write row, because a caller who wrote one meant
            // something the batch cannot do.
            if let Some(write) = ops.iter().find(|op| op.is_write()) {
                return Err(OpError::Unsupported(format!(
                    "a batch read row can only run read operations, and this one includes a write \
                     to bin {:?}. Use a write row for that",
                    write.bin().unwrap_or("<the record>")
                )));
            }
            BatchOperation::read_ops(
                &read_policy(policy, caps)?,
                key,
                ops::to_operations(ops, caps)?,
            )
        }
        WireBatchRow::Write { ops, policy, .. } => BatchOperation::write(
            &write_policy(policy, caps)?,
            key,
            ops::to_operations(ops, caps)?,
        ),
        WireBatchRow::Delete { policy, .. } => {
            BatchOperation::delete(&delete_policy(policy, caps)?, key)
        }
        WireBatchRow::Udf {
            package,
            function,
            args,
            policy,
            ..
        } => {
            let args = args
                .iter()
                .map(convert::to_value)
                .collect::<Result<Vec<Value>, _>>()?;
            BatchOperation::udf(
                &udf_policy(policy, caps)?,
                key,
                package,
                function,
                // `None` and an empty list mean the same thing to the server;
                // sending `None` keeps the wire shorter for the common case.
                if args.is_empty() { None } else { Some(args) },
            )
        }
    })
}

/// One row's answer, as the contract carries it.
///
/// A result code on the record means *that row* failed; the batch did not.
/// Three answers, and they must stay distinguishable: a **failure** (a result
/// code, possibly in doubt), a read that **found nothing** (success, no record),
/// and a read that **found something** (success with bins, generation and TTL). A
/// missing record and a failed row look alike to a caller who only checks for
/// bins, which is why the contract carries the result code separately.
#[must_use]
pub fn to_result(record: &BatchRecord) -> WireBatchResult {
    let (in_doubt, server_message, found) =
        (record.in_doubt, record.server_message(), record.record.as_ref());
    match record.result_code {
        Some(code) if code != aerospike_core::ResultCode::Ok => WireBatchResult::failed(
            // The client's result code is an enum over the server's byte, and
            // `u8` is the conversion it offers; the contract carries an i32
            // because a *client-side* code is negative.
            i32::from(u8::from(code)),
            in_doubt,
            server_message.map(ToOwned::to_owned),
        ),
        _ => {
            let Some(found) = found else {
                // A read row that found nothing: a success with no record,
                // which is a different answer from a write row's empty one.
                return WireBatchResult::ok(None, 0, None);
            };
            let bins = found
                .bins
                .iter()
                .map(|(name, value)| (name.clone(), convert::from_value(value)))
                .collect();
            // As on a single-key read: a finite TTL must never reach the
            // sentinel, however large the server's answer was.
            let ttl = found.time_to_live().map(|d| {
                u32::try_from(d.as_secs())
                    .unwrap_or(u32::MAX)
                    .min(TTL_NEVER_EXPIRES - 1)
            });
            WireBatchResult::ok(Some(RecordBody { bins }), found.generation, ttl)
        }
    }
}

fn to_key(key: &WireBatchKey) -> Result<aerospike_core::Key, OpError> {
    convert::to_key(&key.namespace, &key.set, &key.key).map_err(|e| {
        OpError::Expression(format!(
            "the key for {}:{} could not be built: {e}",
            key.namespace, key.set
        ))
    })
}

/// A row filter, which is AEL text and therefore needs the same version gate the
/// policy filter does.
fn row_filter(
    filter: Option<&WireExpression>,
    caps: &Capabilities,
) -> Result<Option<aerospike_core::expressions::Expression>, OpError> {
    match filter {
        None => Ok(None),
        // The same door as a parent policy's filter, so a per-row filter accepts
        // a built tree as readily as text.
        Some(filter) => Ok(Some(ops::to_expression(filter, caps)?)),
    }
}

fn read_policy(
    policy: &WireBatchReadPolicy,
    caps: &Capabilities,
) -> Result<BatchReadPolicy, OpError> {
    let mut built = BatchReadPolicy::default();
    built.filter_expression = row_filter(policy.filter.as_ref(), caps)?;
    Ok(built)
}

fn write_policy(
    policy: &WireBatchWritePolicy,
    caps: &Capabilities,
) -> Result<BatchWritePolicy, OpError> {
    let mut built = BatchWritePolicy::default();
    if let Some(action) = policy.record_exists_action {
        built.record_exists_action = crate::policy::record_exists_action(action);
    }
    if let Some(guard) = policy.generation_policy {
        built.generation_policy = crate::policy::generation_policy(guard);
    }
    if let Some(generation) = policy.generation {
        built.generation = generation;
    }
    if let Some(level) = policy.commit_level {
        built.commit_level = crate::policy::commit_level(level);
    }
    if let Some(expiration) = policy.expiration {
        built.expiration = crate::policy::expiration(expiration);
    }
    if let Some(send_key) = policy.send_key {
        built.send_key = send_key;
    }
    if let Some(durable) = policy.durable_delete {
        built.durable_delete = durable;
    }
    built.filter_expression = row_filter(policy.filter.as_ref(), caps)?;
    Ok(built)
}

fn delete_policy(
    policy: &WireBatchDeletePolicy,
    caps: &Capabilities,
) -> Result<BatchDeletePolicy, OpError> {
    let mut built = BatchDeletePolicy::default();
    if let Some(guard) = policy.generation_policy {
        built.generation_policy = crate::policy::generation_policy(guard);
    }
    if let Some(generation) = policy.generation {
        built.generation = generation;
    }
    if let Some(level) = policy.commit_level {
        built.commit_level = crate::policy::commit_level(level);
    }
    if let Some(send_key) = policy.send_key {
        built.send_key = send_key;
    }
    if let Some(durable) = policy.durable_delete {
        built.durable_delete = durable;
    }
    built.filter_expression = row_filter(policy.filter.as_ref(), caps)?;
    Ok(built)
}

fn udf_policy(policy: &WireBatchUdfPolicy, caps: &Capabilities) -> Result<BatchUDFPolicy, OpError> {
    let mut built = BatchUDFPolicy::default();
    if let Some(level) = policy.commit_level {
        built.commit_level = crate::policy::commit_level(level);
    }
    if let Some(expiration) = policy.expiration {
        built.expiration = crate::policy::expiration(expiration);
    }
    if let Some(send_key) = policy.send_key {
        built.send_key = send_key;
    }
    if let Some(durable) = policy.durable_delete {
        built.durable_delete = durable;
    }
    built.filter_expression = row_filter(policy.filter.as_ref(), caps)?;
    Ok(built)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::op::WireOp;
    use aerospike_php_ipc::policy::{WireExpiration, WireRecordExistsAction};
    use aerospike_php_ipc::WireKey;

    fn key() -> WireBatchKey {
        WireBatchKey {
            namespace: "test".into(),
            set: "batch".into(),
            key: WireKey::Str("a".into()),
        }
    }

    /// Every row kind must translate, since a missing arm is a panic in the
    /// request path.
    #[test]
    fn every_row_kind_translates() {
        let rows = vec![
            WireBatchRow::Read {
                key: key(),
                bins: BinSelector::All,
                policy: WireBatchReadPolicy::default(),
            },
            WireBatchRow::Read {
                key: key(),
                bins: BinSelector::Only(vec!["n".into()]),
                policy: WireBatchReadPolicy::default(),
            },
            WireBatchRow::ReadOps {
                key: key(),
                ops: vec![WireOp::GetBin { bin: "n".into() }],
                policy: WireBatchReadPolicy::default(),
            },
            WireBatchRow::Write {
                key: key(),
                ops: vec![WireOp::Put {
                    bin: "n".into(),
                    value: aerospike_php_ipc::WireValue::Int(1),
                }],
                policy: WireBatchWritePolicy::default(),
            },
            WireBatchRow::Delete {
                key: key(),
                policy: WireBatchDeletePolicy::default(),
            },
            WireBatchRow::Udf {
                key: key(),
                package: "p".into(),
                function: "f".into(),
                args: vec![],
                policy: WireBatchUdfPolicy::default(),
            },
        ];

        let built = to_operations(&rows, &Capabilities::all()).expect("every row must translate");
        assert_eq!(built.len(), rows.len());
    }

    /// A write in a read row would be sent as a read and silently do nothing, so
    /// it is refused with a message that names the bin and the alternative.
    #[test]
    fn a_write_in_a_read_row_is_refused() {
        let row = WireBatchRow::ReadOps {
            key: key(),
            ops: vec![
                WireOp::GetBin { bin: "n".into() },
                WireOp::Put {
                    bin: "n".into(),
                    value: aerospike_php_ipc::WireValue::Int(1),
                },
            ],
            policy: WireBatchReadPolicy::default(),
        };

        let error = to_operation(&row, &Capabilities::all())
            .expect_err("a write in a read row must be refused");
        let message = error.to_string();
        assert!(message.contains("\"n\""), "{message}");
        assert!(message.contains("write row"), "{message}");
    }

    /// A row's settings override the client's defaults and leave the rest alone
    /// — the per-row equivalent of "unset means do not override".
    #[test]
    fn a_row_policy_overrides_only_what_it_sets() {
        let bare = write_policy(&WireBatchWritePolicy::default(), &Capabilities::all()).unwrap();
        assert_eq!(bare, BatchWritePolicy::default());

        let set = write_policy(
            &WireBatchWritePolicy {
                record_exists_action: Some(WireRecordExistsAction::CreateOnly),
                expiration: Some(WireExpiration::Never),
                send_key: Some(true),
                ..WireBatchWritePolicy::default()
            },
            &Capabilities::all(),
        )
        .unwrap();

        assert!(matches!(
            set.record_exists_action,
            aerospike_core::RecordExistsAction::CreateOnly
        ));
        assert!(matches!(set.expiration, aerospike_core::Expiration::Never));
        assert!(set.send_key);
        // Untouched, so still the client's default.
        assert_eq!(set.generation, BatchWritePolicy::default().generation);
        assert_eq!(set.commit_level, BatchWritePolicy::default().commit_level);
    }

    /// A row filter is AEL text, so it needs the same version gate the policy
    /// filter does — on every row kind that accepts one.
    #[test]
    fn a_row_filter_needs_a_new_enough_cluster() {
        // Only the AEL gate is what a text filter needs; the string gate is
        // irrelevant here, so it stays supported.
        let old = Capabilities {
            ael: crate::policy::AelSupport::Unsupported {
                node: "BB9A".into(),
                version: "7.2.0".into(),
            },
            ..Capabilities::all()
        };
        let filter = Some(WireExpression::Ael("$.n:INT > 1".to_owned()));

        assert!(read_policy(
            &WireBatchReadPolicy {
                filter: filter.clone()
            },
            &old
        )
        .is_err());
        assert!(write_policy(
            &WireBatchWritePolicy {
                filter: filter.clone(),
                ..WireBatchWritePolicy::default()
            },
            &old
        )
        .is_err());
        assert!(delete_policy(
            &WireBatchDeletePolicy {
                filter: filter.clone(),
                ..WireBatchDeletePolicy::default()
            },
            &old
        )
        .is_err());
        assert!(udf_policy(
            &WireBatchUdfPolicy {
                filter,
                ..WireBatchUdfPolicy::default()
            },
            &old
        )
        .is_err());
    }

    /// A row with nothing filled in, as the client hands one out before the
    /// server has answered for it.
    fn row(has_write: bool) -> BatchRecord {
        BatchRecord::new(
            aerospike_core::Key::new("test", "demo", aerospike_core::Value::Int(1))
                .expect("an integer key is always valid"),
            has_write,
        )
    }

    /// The three answers a row can give have to stay distinguishable: a failure,
    /// a read that found nothing, and a read that found something.
    #[test]
    fn the_three_row_answers_are_distinct() {
        // A failure: the result code travels, and no record comes with it.
        let mut failed = row(false);
        failed.result_code = Some(aerospike_core::ResultCode::KeyNotFoundError);
        let failed = to_result(&failed);
        assert!(!failed.is_ok());
        assert_eq!(
            failed.result_code,
            Some(i32::from(u8::from(
                aerospike_core::ResultCode::KeyNotFoundError
            )))
        );
        assert!(failed.record.is_none());

        // In doubt travels separately from the code: a write that may have landed
        // is not the same answer as one that certainly did not. Note the
        // `has_write` — a read row can never be in doubt, which is the client's
        // rule and not this crate's.
        let mut doubtful = row(true);
        doubtful.result_code = Some(aerospike_core::ResultCode::Timeout);
        doubtful.in_doubt = true;
        doubtful.set_error_detail(Some(Box::new(aerospike_core::ServerErrorDetail {
            sub_code: aerospike_core::server_error::sub_code::NONE,
            message: "timed out".to_owned(),
            ..Default::default()
        })));
        let doubtful = to_result(&doubtful);
        assert!(doubtful.in_doubt);
        assert_eq!(doubtful.message.as_deref(), Some("timed out"));

        // A read that found nothing: a success, with no record. This is the answer
        // most easily mistaken for a failure by a caller that only checks bins.
        let missing = to_result(&row(false));
        assert!(missing.is_ok(), "no result code means the row succeeded");
        assert!(
            missing.record.is_none(),
            "a read that found nothing carries no record"
        );
        assert_eq!(missing.generation, 0);
        assert_eq!(missing.ttl, None);

        // A read that found something: bins, generation and a TTL come through.
        // `0` for the expiration is the server's "never expires".
        let mut found = row(false);
        let mut bins = aerospike_core::IndexMap::new();
        bins.insert("n".to_owned(), aerospike_core::Value::Int(7));
        found.record = Some(aerospike_core::Record::new(None, bins, None, 3, 0));
        let found = to_result(&found);
        assert!(found.is_ok());
        assert_eq!(found.generation, 3);
        assert_eq!(found.ttl, None, "expiration 0 is never-expires, not a TTL");
        let body = found.record.expect("a found row carries its bins");
        assert_eq!(body.bins.len(), 1);

        // `Ok` stated explicitly is a success, not a failure whose code is zero.
        let mut explicit_ok = row(false);
        explicit_ok.result_code = Some(aerospike_core::ResultCode::Ok);
        assert!(to_result(&explicit_ok).is_ok());
    }
}
