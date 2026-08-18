// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Batch: many records, many kinds of work, one round trip.
//!
//! # Why the rows are heterogeneous
//!
//! A batch is not "many gets". `aerospike-core`'s batch takes a list of
//! operations that may each be a read, a write, a delete or a UDF call, against
//! any key in any namespace, and the client routes them to the nodes that own
//! them. This mirrors that exactly, because splitting it into one call per kind
//! would give up the only thing a batch is for: one round trip per *node*
//! instead of one per record.
//!
//! # Rows answer in order, and failures are per row
//!
//! The reply carries one [`WireBatchResult`] per request row, in the order the
//! rows were sent, and a row's failure is **its own**: a batch where nine rows
//! succeed and one is filtered out is a successful batch with ten answers, not
//! a failure. That is why every row carries its own result code rather than the
//! reply carrying one status for the lot.
//!
//! Record metadata is per row too, so unlike a single-key read it travels in the
//! payload rather than in the reply header — there is no one generation for a
//! batch.

use serde::{Deserialize, Serialize};

use crate::op::{WireExpression, WireOp};
use crate::policy::{
    WireCommitLevel, WireExpiration, WireGenerationPolicy, WireRecordExistsAction,
};
use crate::{BinSelector, RecordBody, WireKey, WirePolicy, WireValue};

/// Which record a batch row names.
///
/// Its own namespace and set, because a batch may span both — that is one of
/// the things it is for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireBatchKey {
    /// Namespace.
    pub namespace: String,
    /// Set name; empty means the null set.
    pub set: String,
    /// Record key.
    pub key: WireKey,
}

/// Per-row settings for a batch read.
///
/// The parent [`WireBatchBody::policy`] carries everything shared — timeouts,
/// retries, replica choice. Only what can differ per row is here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WireBatchReadPolicy {
    /// A filter for this row alone, in any of the three forms. A row it rejects
    /// comes back with result code 27 rather than failing the batch.
    pub filter: Option<WireExpression>,
}

/// Per-row settings for a batch write.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WireBatchWritePolicy {
    /// Create/update/replace semantics.
    pub record_exists_action: Option<WireRecordExistsAction>,
    /// Generation guard.
    pub generation_policy: Option<WireGenerationPolicy>,
    /// The generation the guard compares against.
    pub generation: Option<u32>,
    /// Replica commit requirement.
    pub commit_level: Option<WireCommitLevel>,
    /// Time-to-live for the record this row writes.
    pub expiration: Option<WireExpiration>,
    /// Store the user key alongside the digest.
    pub send_key: Option<bool>,
    /// Leave a tombstone on delete (Enterprise only).
    pub durable_delete: Option<bool>,
    /// A filter for this row alone, in any of the three forms.
    pub filter: Option<WireExpression>,
}

/// Per-row settings for a batch delete.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WireBatchDeletePolicy {
    /// Generation guard.
    pub generation_policy: Option<WireGenerationPolicy>,
    /// The generation the guard compares against.
    pub generation: Option<u32>,
    /// Replica commit requirement.
    pub commit_level: Option<WireCommitLevel>,
    /// Store the user key alongside the digest.
    pub send_key: Option<bool>,
    /// Leave a tombstone (Enterprise only).
    pub durable_delete: Option<bool>,
    /// A filter for this row alone, in any of the three forms.
    pub filter: Option<WireExpression>,
}

/// Per-row settings for a batch UDF call.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WireBatchUdfPolicy {
    /// Replica commit requirement.
    pub commit_level: Option<WireCommitLevel>,
    /// Time-to-live for whatever the function writes.
    pub expiration: Option<WireExpiration>,
    /// Store the user key alongside the digest.
    pub send_key: Option<bool>,
    /// Leave a tombstone (Enterprise only).
    pub durable_delete: Option<bool>,
    /// A filter for this row alone, in any of the three forms.
    pub filter: Option<WireExpression>,
}

/// One row of a batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireBatchRow {
    /// Read some or all of a record's bins.
    Read {
        /// Which record.
        key: WireBatchKey,
        /// Which bins.
        bins: BinSelector,
        /// Per-row settings.
        policy: WireBatchReadPolicy,
    },
    /// Read by running operations that only read.
    ReadOps {
        /// Which record.
        key: WireBatchKey,
        /// What to run; every operation must be a read.
        ops: Vec<WireOp>,
        /// Per-row settings.
        policy: WireBatchReadPolicy,
    },
    /// Write by running operations.
    Write {
        /// Which record.
        key: WireBatchKey,
        /// What to run.
        ops: Vec<WireOp>,
        /// Per-row settings.
        policy: WireBatchWritePolicy,
    },
    /// Delete a record.
    Delete {
        /// Which record.
        key: WireBatchKey,
        /// Per-row settings.
        policy: WireBatchDeletePolicy,
    },
    /// Call a registered UDF on a record.
    Udf {
        /// Which record.
        key: WireBatchKey,
        /// Registered package name, without the `.lua`.
        package: String,
        /// Function inside the package.
        function: String,
        /// Arguments, in order.
        args: Vec<WireValue>,
        /// Per-row settings.
        policy: WireBatchUdfPolicy,
    },
}

impl WireBatchRow {
    /// Whether this row writes.
    ///
    /// Only a write row can ever be in doubt, which is why the reply carries the
    /// flag per row and why this has to be knowable without asking the server.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        matches!(
            self,
            WireBatchRow::Write { .. } | WireBatchRow::Delete { .. } | WireBatchRow::Udf { .. }
        )
    }

    /// The record this row names.
    #[must_use]
    pub const fn key(&self) -> &WireBatchKey {
        match self {
            WireBatchRow::Read { key, .. }
            | WireBatchRow::ReadOps { key, .. }
            | WireBatchRow::Write { key, .. }
            | WireBatchRow::Delete { key, .. }
            | WireBatchRow::Udf { key, .. } => key,
        }
    }
}

/// Payload of a [`crate::opcode::BATCH`] request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireBatchBody {
    /// Cluster instance from the daemon's configuration.
    pub instance: String,
    /// Settings shared by every row: timeouts, retries, replica choice.
    ///
    /// One [`WirePolicy`] rather than a batch-specific type, so the daemon
    /// resolves it through the same path as every other command. Fields that
    /// only make sense per row are on the rows.
    pub policy: WirePolicy,
    /// The rows, in the order their answers will come back.
    pub rows: Vec<WireBatchRow>,
}

impl WireBatchBody {
    /// Whether any row writes.
    #[must_use]
    pub fn has_write(&self) -> bool {
        self.rows.iter().any(WireBatchRow::is_write)
    }
}

/// One row's answer.
///
/// A row that failed carries its own [`result_code`](Self::result_code) and no
/// record. A row that succeeded carries a record — possibly with no bins, for a
/// write — plus its own generation and TTL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireBatchResult {
    /// The server's result code for this row, or `None` when it succeeded.
    pub result_code: Option<i32>,
    /// Whether a failed **write** row may nevertheless have been applied.
    pub in_doubt: bool,
    /// What came back, if anything.
    pub record: Option<RecordBody>,
    /// The record's generation, when there is a record.
    pub generation: u32,
    /// Seconds to live, or `None` when the record never expires.
    pub ttl: Option<u32>,
    /// The server's explanation, when it sent one.
    pub message: Option<String>,
}

impl WireBatchResult {
    /// A row that succeeded, with whatever it read.
    #[must_use]
    pub fn ok(record: Option<RecordBody>, generation: u32, ttl: Option<u32>) -> WireBatchResult {
        WireBatchResult {
            result_code: None,
            in_doubt: false,
            record,
            generation,
            ttl,
            message: None,
        }
    }

    /// A row that failed.
    #[must_use]
    pub fn failed(result_code: i32, in_doubt: bool, message: Option<String>) -> WireBatchResult {
        WireBatchResult {
            result_code: Some(result_code),
            in_doubt,
            record: None,
            generation: 0,
            ttl: None,
            message,
        }
    }

    /// Whether this row succeeded.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        self.result_code.is_none()
    }
}

/// Payload of a [`crate::opcode::BATCH`] reply: one answer per row, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireBatchReply {
    /// One per request row, in the same order.
    pub rows: Vec<WireBatchResult>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_body, encode_body};

    fn key(name: &str) -> WireBatchKey {
        WireBatchKey {
            namespace: "test".into(),
            set: "batch".into(),
            key: WireKey::Str(name.into()),
        }
    }

    /// Every row kind must survive the wire, and a batch that mixes them is the
    /// whole point of the shape.
    #[test]
    fn a_heterogeneous_batch_round_trips() {
        let body = WireBatchBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            policy: WirePolicy {
                total_timeout_ms: Some(500),
                ..WirePolicy::default()
            },
            rows: vec![
                WireBatchRow::Read {
                    key: key("a"),
                    bins: BinSelector::All,
                    policy: WireBatchReadPolicy::default(),
                },
                WireBatchRow::ReadOps {
                    key: key("b"),
                    ops: vec![WireOp::GetBin { bin: "n".into() }],
                    policy: WireBatchReadPolicy {
                        filter: Some(WireExpression::Ael("$.n:INT > 1".into())),
                    },
                },
                WireBatchRow::Write {
                    key: key("c"),
                    ops: vec![WireOp::Put {
                        bin: "n".into(),
                        value: WireValue::Int(1),
                    }],
                    policy: WireBatchWritePolicy {
                        expiration: Some(WireExpiration::Never),
                        ..WireBatchWritePolicy::default()
                    },
                },
                WireBatchRow::Delete {
                    key: key("d"),
                    policy: WireBatchDeletePolicy {
                        durable_delete: Some(true),
                        ..WireBatchDeletePolicy::default()
                    },
                },
                WireBatchRow::Udf {
                    key: key("e"),
                    package: "example".into(),
                    function: "touch".into(),
                    args: vec![WireValue::Int(1)],
                    policy: WireBatchUdfPolicy::default(),
                },
            ],
        };

        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<WireBatchBody>(&bytes).unwrap(), body);
    }

    /// Only a write row can be in doubt, so which rows write has to be knowable
    /// from the request alone.
    #[test]
    fn write_rows_are_told_from_read_rows() {
        let reads = [
            WireBatchRow::Read {
                key: key("a"),
                bins: BinSelector::All,
                policy: WireBatchReadPolicy::default(),
            },
            WireBatchRow::ReadOps {
                key: key("b"),
                ops: vec![],
                policy: WireBatchReadPolicy::default(),
            },
        ];
        for row in &reads {
            assert!(!row.is_write(), "{row:?}");
        }

        let writes = [
            WireBatchRow::Write {
                key: key("c"),
                ops: vec![],
                policy: WireBatchWritePolicy::default(),
            },
            WireBatchRow::Delete {
                key: key("d"),
                policy: WireBatchDeletePolicy::default(),
            },
            WireBatchRow::Udf {
                key: key("e"),
                package: "p".into(),
                function: "f".into(),
                args: vec![],
                policy: WireBatchUdfPolicy::default(),
            },
        ];
        for row in &writes {
            assert!(row.is_write(), "{row:?}");
        }

        let body = WireBatchBody {
            instance: "x".into(),
            policy: WirePolicy::default(),
            rows: reads.to_vec(),
        };
        assert!(!body.has_write());

        let mut mixed = body.clone();
        mixed.rows.push(writes[0].clone());
        assert!(mixed.has_write());
    }

    /// A row's failure is its own: the reply has to be able to carry a success
    /// and a failure side by side, which is what makes a batch a batch.
    #[test]
    fn a_reply_carries_successes_and_failures_together() {
        let reply = WireBatchReply {
            rows: vec![
                WireBatchResult::ok(
                    Some(RecordBody {
                        bins: vec![("n".into(), WireValue::Int(1))],
                    }),
                    3,
                    Some(600),
                ),
                WireBatchResult::failed(27, false, Some("filtered out".into())),
                WireBatchResult::failed(9, true, None),
            ],
        };

        assert!(reply.rows[0].is_ok());
        assert!(!reply.rows[1].is_ok());
        assert!(!reply.rows[1].in_doubt);
        assert!(reply.rows[2].in_doubt, "a timed-out write row can be in doubt");

        let bytes = encode_body(&reply).unwrap();
        assert_eq!(decode_body::<WireBatchReply>(&bytes).unwrap(), reply);
    }

    /// A read row that found nothing is a success with no record — not a
    /// failure, and not a record with no bins, which is what a *write* row
    /// answers with.
    #[test]
    fn a_missing_record_and_a_write_answer_differently() {
        let missing = WireBatchResult::ok(None, 0, None);
        let wrote = WireBatchResult::ok(Some(RecordBody { bins: vec![] }), 1, Some(100));

        assert!(missing.is_ok() && wrote.is_ok());
        assert_ne!(missing.record, wrote.record);
    }
}
