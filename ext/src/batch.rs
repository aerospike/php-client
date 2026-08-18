// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

// The PHP-facing parameters are camelCase because PHP named arguments use the
// parameter name exactly as written; see `crate::policy` for why the lint cannot
// be silenced any closer than the module.
#![allow(non_snake_case)]

//! Batch: many records, several kinds of work, one round trip.
//!
//! A batch is **not** many gets. Its rows may each be a read, a write, a delete
//! or a UDF call, against any key in any namespace, and the client routes them
//! to the nodes that own them — one round trip per node instead of one per
//! record.
//!
//! ```php
//! use Aerospike\{BatchRead, BatchWrite, BatchDelete, Bin, Key, Op};
//!
//! $results = $client->batch(null, [
//!     BatchRead::all(new Key('test', 'users', 'alice')),
//!     BatchRead::some(new Key('test', 'users', 'bob'), ['name']),
//!     BatchWrite::ops(new Key('test', 'users', 'carol'), [Op::add(new Bin('hits', 1))]),
//!     BatchDelete::key(new Key('test', 'users', 'dave')),
//! ]);
//!
//! foreach ($results as $row) {
//!     if (!$row->isOk()) { continue; }         // that row failed, not the batch
//!     $row->record()?->bin('name');
//! }
//! ```
//!
//! # A failed row is not a failed batch
//!
//! `batch()` returns one [`BatchResult`] per row, **in the order the rows were
//! given**, and a row's failure belongs to that row: nine successes and one
//! filtered-out row is a successful batch with ten answers. Only a failure that
//! stops the batch being sent at all — an unreachable cluster, an unbuildable
//! row — throws.
//!
//! That is why `isOk()` exists and why there is no exception per row. A caller
//! that had to catch one to read the other nine would be worse off.

use aerospike_php_ipc::op::WireExpression;
use aerospike_php_ipc::batch::{
    WireBatchDeletePolicy, WireBatchKey, WireBatchReadPolicy, WireBatchResult, WireBatchRow,
    WireBatchUdfPolicy, WireBatchWritePolicy,
};
use aerospike_php_ipc::{BinSelector, WireValue};
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

use crate::arg::Given;
use crate::enums::{CommitLevel, GenerationPolicy, RecordExistsAction};
use crate::error::{AeroError, AeroResult};
use crate::ops::{self, Expression};
use crate::policy::Expiration;
use crate::record::{Bins, Key, Record};
use crate::value::{self, Path};

/// One row of a batch.
///
/// Built by the static methods of [`BatchRead`], [`BatchWrite`],
/// [`BatchDelete`] and [`BatchUdf`] — one class per kind, so the arguments a
/// kind needs are the arguments its methods take.
#[php_class]
#[php(name = "Aerospike\\BatchRow")]
#[derive(Debug, Clone)]
pub struct BatchRow {
    row: WireBatchRow,
}

#[php_impl]
impl BatchRow {
    /// Whether this row writes.
    ///
    /// Only a write row can ever be reported in doubt, so this is worth being
    /// able to check before sending.
    pub fn is_write(&self) -> bool {
        self.row.is_write()
    }

    /// The record this row names.
    pub fn key(&self) -> PhpResult<Key> {
        let key = self.row.key();
        Ok(Key::from_parts(
            key.namespace.clone(),
            key.set.clone(),
            key.key.clone(),
        ))
    }
}

impl BatchRow {
    /// The contract's row.
    #[must_use]
    pub fn to_wire(&self) -> WireBatchRow {
        self.row.clone()
    }
}

/// A batch read row.
#[php_class]
#[php(name = "Aerospike\\BatchRead")]
#[derive(Debug, Clone, Copy)]
pub struct BatchRead;

#[php_impl]
impl BatchRead {
    /// Read every bin of a record.
    #[php(defaults(filter = None))]
    pub fn all(
        key: Given<&Key>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
    ) -> PhpResult<BatchRow> {
        Ok(BatchRow {
            row: WireBatchRow::Read {
                key: wire_key(key.required("key")?),
                bins: BinSelector::All,
                policy: read_policy(filter, filterExp)?,
            },
        })
    }

    /// Read the named bins.
    #[php(defaults(filter = None))]
    pub fn some(
        key: Given<&Key>,
        bins: Vec<String>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
    ) -> PhpResult<BatchRow> {
        Ok(BatchRow {
            row: WireBatchRow::Read {
                key: wire_key(key.required("key")?),
                bins: Bins::only(bins)?.to_wire(),
                policy: read_policy(filter, filterExp)?,
            },
        })
    }

    /// Read no bins — the record's metadata only.
    ///
    /// Useful for asking "which of these keys exist" in one round trip.
    #[php(defaults(filter = None))]
    pub fn header(
        key: Given<&Key>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
    ) -> PhpResult<BatchRow> {
        Ok(BatchRow {
            row: WireBatchRow::Read {
                key: wire_key(key.required("key")?),
                bins: BinSelector::None,
                policy: read_policy(filter, filterExp)?,
            },
        })
    }

    /// Read by running operations, which lets a batch row use the collection
    /// reads — `ListOp::getByIndex`, `MapOp::getByKey` and the rest.
    ///
    /// Every operation must be a read. A write here is refused rather than sent
    /// as a read that quietly does nothing.
    #[php(defaults(filter = None))]
    pub fn ops(
        key: Given<&Key>,
        ops: Vec<&Zval>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
    ) -> PhpResult<BatchRow> {
        let ops = ops::operation_list(&ops)?;
        if let Some(write) = ops.iter().find(|op| op.is_write()) {
            return Err(AeroError::client(format!(
                "a batch read row can only run read operations, and this one includes a write to \
                 bin \"{}\". Use BatchWrite::ops() for that",
                write.bin().unwrap_or("<the record>")
            ))
            .into());
        }
        Ok(BatchRow {
            row: WireBatchRow::ReadOps {
                key: wire_key(key.required("key")?),
                ops,
                policy: read_policy(filter, filterExp)?,
            },
        })
    }
}

/// A batch write row.
#[php_class]
#[php(name = "Aerospike\\BatchWrite")]
#[derive(Debug, Clone, Copy)]
pub struct BatchWrite;

#[php_impl]
impl BatchWrite {
    /// Write by running operations against one record.
    ///
    /// The operations run in the order given, atomically, exactly as in
    /// `operate()` — a batch write row *is* an `operate` the batch carries.
    #[php(defaults(
        recordExistsAction = None,
        generationPolicy = None,
        generation = None,
        expiration = None,
        commitLevel = None,
        sendKey = None,
        durableDelete = None,
        filter = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn ops(
        key: Given<&Key>,
        ops: Vec<&Zval>,
        recordExistsAction: Option<Given<RecordExistsAction>>,
        generationPolicy: Option<Given<GenerationPolicy>>,
        generation: Option<Given<i64>>,
        expiration: Option<Given<&Expiration>>,
        commitLevel: Option<Given<CommitLevel>>,
        sendKey: Option<Given<bool>>,
        durableDelete: Option<Given<bool>>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
    ) -> PhpResult<BatchRow> {
        let ops = ops::operation_list(&ops)?;
        if ops.is_empty() {
            return Err(AeroError::client(
                "a batch write row needs at least one operation",
            )
            .into());
        }

        Ok(BatchRow {
            row: WireBatchRow::Write {
                key: wire_key(key.required("key")?),
                ops,
                policy: WireBatchWritePolicy {
                    record_exists_action: Given::or_none(recordExistsAction, "recordExistsAction")?
                        .map(RecordExistsAction::to_wire),
                    generation_policy: Given::or_none(generationPolicy, "generationPolicy")?
                        .map(GenerationPolicy::to_wire),
                    generation: generation_of(generation)?,
                    commit_level: Given::or_none(commitLevel, "commitLevel")?
                        .map(CommitLevel::to_wire),
                    expiration: Given::or_none(expiration, "expiration")?
                        .map(Expiration::to_wire),
                    send_key: Given::or_none(sendKey, "sendKey")?,
                    durable_delete: Given::or_none(durableDelete, "durableDelete")?,
                    filter: filter_of(filter, filterExp)?,
                },
            },
        })
    }
}

/// A batch delete row.
#[php_class]
#[php(name = "Aerospike\\BatchDelete")]
#[derive(Debug, Clone, Copy)]
pub struct BatchDelete;

#[php_impl]
impl BatchDelete {
    /// Delete one record.
    #[php(defaults(
        generationPolicy = None,
        generation = None,
        commitLevel = None,
        sendKey = None,
        durableDelete = None,
        filter = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn key(
        key: Given<&Key>,
        generationPolicy: Option<Given<GenerationPolicy>>,
        generation: Option<Given<i64>>,
        commitLevel: Option<Given<CommitLevel>>,
        sendKey: Option<Given<bool>>,
        durableDelete: Option<Given<bool>>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
    ) -> PhpResult<BatchRow> {
        Ok(BatchRow {
            row: WireBatchRow::Delete {
                key: wire_key(key.required("key")?),
                policy: WireBatchDeletePolicy {
                    generation_policy: Given::or_none(generationPolicy, "generationPolicy")?
                        .map(GenerationPolicy::to_wire),
                    generation: generation_of(generation)?,
                    commit_level: Given::or_none(commitLevel, "commitLevel")?
                        .map(CommitLevel::to_wire),
                    send_key: Given::or_none(sendKey, "sendKey")?,
                    durable_delete: Given::or_none(durableDelete, "durableDelete")?,
                    filter: filter_of(filter, filterExp)?,
                },
            },
        })
    }
}

/// A batch UDF row.
#[php_class]
#[php(name = "Aerospike\\BatchUdf")]
#[derive(Debug, Clone, Copy)]
pub struct BatchUdf;

#[php_impl]
impl BatchUdf {
    /// Call a registered UDF on one record.
    ///
    /// `$package` is the module's registered name without the `.lua`. The
    /// function must already be registered on the cluster; registering one is
    /// not part of this API yet.
    #[php(defaults(
        args = None,
        commitLevel = None,
        expiration = None,
        sendKey = None,
        durableDelete = None,
        filter = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn call(
        key: Given<&Key>,
        package: String,
        function: String,
        args: Option<Vec<&Zval>>,
        commitLevel: Option<Given<CommitLevel>>,
        expiration: Option<Given<&Expiration>>,
        sendKey: Option<Given<bool>>,
        durableDelete: Option<Given<bool>>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
    ) -> PhpResult<BatchRow> {
        let args = match args {
            None => Vec::new(),
            Some(args) => args
                .iter()
                .enumerate()
                .map(|(index, zval)| {
                    value::zval_to_wire(zval, &Path::bin(&format!("args[{index}]")))
                })
                .collect::<AeroResult<Vec<WireValue>>>()?,
        };

        Ok(BatchRow {
            row: WireBatchRow::Udf {
                key: wire_key(key.required("key")?),
                package,
                function,
                args,
                policy: WireBatchUdfPolicy {
                    commit_level: Given::or_none(commitLevel, "commitLevel")?
                        .map(CommitLevel::to_wire),
                    expiration: Given::or_none(expiration, "expiration")?
                        .map(Expiration::to_wire),
                    send_key: Given::or_none(sendKey, "sendKey")?,
                    durable_delete: Given::or_none(durableDelete, "durableDelete")?,
                    filter: filter_of(filter, filterExp)?,
                },
            },
        })
    }
}

/// One row's answer.
///
/// There is no constructor: a result describes what a server did, and one
/// nobody asked for would be a fiction.
#[php_class]
#[php(name = "Aerospike\\BatchResult")]
#[derive(Debug, Clone)]
pub struct BatchResult {
    result: WireBatchResult,
}

#[php_impl]
impl BatchResult {
    /// Whether this row succeeded.
    ///
    /// **Check this before reading the record.** A batch does not throw for a
    /// row that failed, because a failure belongs to the row and not to the
    /// batch.
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// The server's result code for this row, or `null` when it succeeded.
    ///
    /// The numbers Aerospike's own documentation lists: 2 for a missing record,
    /// 27 for one a filter rejected, 3 for a generation mismatch.
    pub fn result_code(&self) -> Option<i64> {
        self.result.result_code.map(i64::from)
    }

    /// Whether a failed **write** row may nevertheless have been applied.
    ///
    /// Never true for a read row. Do not blindly retry a non-idempotent row
    /// when this is true.
    pub fn is_in_doubt(&self) -> bool {
        self.result.in_doubt
    }

    /// What this row read, or `null`.
    ///
    /// `null` means one of two things, told apart by [`BatchResult::is_ok`]: a
    /// row that failed, or a read row that found no record.
    pub fn record(&self) -> PhpResult<Option<Record>> {
        Ok(self.result.record.clone().map(|body| {
            Record::from_parts(body, self.result.generation, self.result.ttl)
        }))
    }

    /// The server's explanation, when it sent one.
    pub fn message(&self) -> Option<String> {
        self.result.message.clone()
    }

    /// The 1.x spelling of [`BatchResult::record`].
    ///
    /// The 1.x client returned a `BatchRecord` per row, whose `getRecord()` is this.
    /// Its `getKey()` has no counterpart here, because a reply row does not carry the
    /// key — the rows come back in the order they were sent, so the key is the one on
    /// the command at the same index. `Aerospike\Compat\Client::batch()` pairs them
    /// up for you.
    #[php(name = "getRecord")]
    pub fn legacy_get_record(&self) -> PhpResult<Option<Record>> {
        self.record()
    }

    /// The 1.x spelling of [`BatchResult::result_code`].
    ///
    /// Note that 1.x reported success as `0` rather than as `null`, so this returns
    /// `0` where [`BatchResult::result_code`] returns `null` — old code compared it
    /// against `ResultCode::OK`.
    #[php(name = "getResultCode")]
    pub fn legacy_get_result_code(&self) -> i64 {
        self.result_code().unwrap_or(0)
    }
}

impl BatchResult {
    /// Build one from a reply row.
    #[must_use]
    pub fn from_wire(result: WireBatchResult) -> BatchResult {
        BatchResult { result }
    }
}

/// The contract's key for a row.
fn wire_key(key: &Key) -> WireBatchKey {
    let (namespace, set, user_key) = key.parts();
    WireBatchKey {
        namespace,
        set,
        key: user_key,
    }
}

/// A per-row read policy, which is only ever a filter.
fn read_policy(
    filter: Option<Given<String>>,
    filterExp: Option<Given<&Expression>>,
) -> PhpResult<WireBatchReadPolicy> {
    Ok(WireBatchReadPolicy {
        filter: filter_of(filter, filterExp)?,
    })
}

/// A row filter, from either spelling, refusing one that would filter nothing.
///
/// `filter` is Aerospike Expression Language text and `filterExp` is a built or
/// packed expression; they set the same field, so naming both is refused rather
/// than one silently winning — exactly as on a policy.
///
/// Returns an [`AeroError`] rather than a PHP exception so the rule is testable
/// without a running PHP: building the exception needs its class entry, which
/// only exists inside a PHP process. The `?` at each call site does the
/// conversion, where there is one.
fn filter_of(
    filter: Option<Given<String>>,
    filterExp: Option<Given<&Expression>>,
) -> AeroResult<Option<WireExpression>> {
    let text = checked(filter, "filter")?;
    let built = checked(filterExp, "filterExp")?;
    match (text, built) {
        (Some(_), Some(_)) => Err(AeroError::client(
            "filter: and filterExp: both name this row's filter, so only one of them can be \
             given. filter: takes Aerospike Expression Language text; filterExp: takes an \
             expression from Aerospike\\Exp",
        )),
        (Some(text), None) => {
            if text.trim().is_empty() {
                return Err(AeroError::client(
                    "filter is an empty expression; omit it rather than passing a filter that \
                     filters nothing",
                ));
            }
            Ok(Some(WireExpression::Ael(text)))
        }
        (None, Some(built)) => Ok(Some(built.to_wire())),
        (None, None) => Ok(None),
    }
}

/// A generation, narrowed to what the wire carries.
fn generation_of(generation: Option<Given<i64>>) -> AeroResult<Option<u32>> {
    match checked(generation, "generation")? {
        None => Ok(None),
        Some(value) => u32::try_from(value).map(Some).map_err(|_| {
            AeroError::client(format!(
                "generation must be between 0 and {}, but is {value}",
                u32::MAX
            ))
        }),
    }
}

/// [`Given::or_none`] with the type complaint as an [`AeroError`], for the same
/// reason [`filter_of`] returns one.
fn checked<'a, T: ext_php_rs::convert::FromZval<'a>>(
    given: Option<Given<T>>,
    argument: &str,
) -> AeroResult<Option<T>> {
    match given {
        None | Some(Given::Empty) => Ok(None),
        Some(Given::Value(value)) => Ok(Some(value)),
        Some(Given::Wrong(actual)) => Err(AeroError::client(format!(
            "${argument} was given {actual}, which is not what it takes"
        ))),
    }
}

/// Read a `BatchRow[]` argument, checking every element.
///
/// # Errors
/// A PHP `TypeError` for an element that is not a `BatchRow`, and a client
/// failure when the list is empty.
pub fn row_list(rows: &[&Zval]) -> PhpResult<Vec<WireBatchRow>> {
    if rows.is_empty() {
        return Err(AeroError::client(
            "a batch needs at least one row; the server has no answer for a request that asks \
             nothing",
        )
        .into());
    }

    let mut wire = Vec::with_capacity(rows.len());
    for (index, zval) in rows.iter().enumerate() {
        let row = zval.extract::<&BatchRow>().ok_or_else(|| {
            crate::arg::type_error(format!(
                "$rows must be a list of Aerospike\\BatchRow; item {index} is {}",
                crate::arg::type_of(zval)
            ))
        })?;
        wire.push(row.to_wire());
    }
    Ok(wire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::batch::WireBatchResult;

    #[test]
    fn a_generation_is_narrowed_and_a_negative_one_refused() {
        assert_eq!(generation_of(None).unwrap(), None);
        assert!(generation_of(Some(Given::Value(-1))).is_err());
    }

    /// The two meanings of a null record have to stay tellable apart, because a
    /// caller reads them with different code.
    #[test]
    fn a_failed_row_and_an_empty_read_both_have_no_record() {
        let failed = BatchResult::from_wire(WireBatchResult::failed(2, false, None));
        assert!(!failed.result.is_ok());
        assert!(failed.result.record.is_none());
        assert_eq!(failed.result.result_code, Some(2));

        let empty = BatchResult::from_wire(WireBatchResult::ok(None, 0, None));
        assert!(empty.result.is_ok());
        assert!(empty.result.record.is_none());
    }

    /// Only a write row can be in doubt; a read that timed out simply did not
    /// answer.
    #[test]
    fn in_doubt_is_carried_per_row() {
        let doubtful = BatchResult::from_wire(WireBatchResult::failed(9, true, None));
        assert!(doubtful.result.in_doubt);

        let certain = BatchResult::from_wire(WireBatchResult::failed(27, false, None));
        assert!(!certain.result.in_doubt);
    }

    #[test]
    fn an_empty_filter_is_refused() {
        assert!(filter_of(Some(Given::Value("   ".to_owned())), None).is_err());
        assert_eq!(
            filter_of(Some(Given::Value("$.n:INT > 1".to_owned())), None).unwrap(),
            Some(WireExpression::Ael("$.n:INT > 1".to_owned()))
        );
    }

    /// A row filter can be written either way, and naming both is refused: they
    /// set one field, so one of them would silently win.
    #[test]
    fn a_row_filter_takes_either_spelling_but_not_both() {
        let built = Expression::from_tree(aerospike_php_ipc::exp::WireExp::Value(
            aerospike_php_ipc::WireValue::Bool(true),
        ));
        assert_eq!(
            filter_of(None, Some(Given::Value(&built))).unwrap(),
            Some(built.to_wire())
        );

        let error = filter_of(
            Some(Given::Value("$.n:INT > 1".to_owned())),
            Some(Given::Value(&built)),
        )
        .expect_err("both spellings name one filter");
        assert!(error.to_string().contains("only one of them"), "{error}");

        assert_eq!(filter_of(None, None).unwrap(), None);
    }
}
