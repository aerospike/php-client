// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Per-call policy overrides.
//!
//! Every field is optional and every `None` means "leave the daemon's
//! configured default alone", so a caller sets only what it wants to change and
//! adding a field later is not a breaking change.
//!
//! One flat type covers reads and writes rather than mirroring the client's
//! `ReadPolicy`/`WritePolicy`/`BatchPolicy` split. Two reasons: PHP passes an
//! associative array, which has no natural place for a type distinction; and
//! the daemon knows which operation it is serving, so it can apply the relevant
//! subset and reject the rest. [`WirePolicy::write_only_fields_set`] exists so
//! that rejection can name what was wrong instead of ignoring it.
//!
//! The enums here deliberately mirror `aerospike-core`'s, but they are *copies*
//! rather than re-exports: this crate does not depend on the database client,
//! and an upstream variant appearing should be a deliberate protocol change
//! rather than something that silently alters the wire format.

use serde::{Deserialize, Serialize};

use crate::op::WireExpression;

/// Which node to prefer for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireReplica {
    /// The partition master.
    Master,
    /// Master and proles, chosen at random.
    MasterProles,
    /// Any node at random.
    Random,
    /// Master first, then proles in sequence on failure.
    Sequence,
    /// A node on the client's own rack first, if rack awareness is configured.
    PreferRack,
}

/// Read consistency in an availability-mode (AP) namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireReadModeAp {
    /// One replica.
    One,
    /// All replicas, so a conflict is detected.
    All,
}

/// Read consistency in a strong-consistency (SC) namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireReadModeSc {
    /// Session consistency: never read older than this client has written.
    Session,
    /// Linearizable across all clients.
    Linearize,
    /// Allow a possibly stale read from a replica.
    AllowReplica,
    /// Allow a read from an unavailable partition.
    AllowUnavailable,
}

/// What to do when the record already exists (or does not).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireRecordExistsAction {
    /// Create or update; merge bins.
    Update,
    /// Update only; fail if absent.
    UpdateOnly,
    /// Create or replace; drop absent bins.
    Replace,
    /// Replace only; fail if absent.
    ReplaceOnly,
    /// Create only; fail if present.
    CreateOnly,
}

/// Whether a write is guarded by the record's generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireGenerationPolicy {
    /// No guard.
    None,
    /// Write only if the generation matches exactly.
    ExpectGenEqual,
    /// Write only if the record's generation is greater.
    ExpectGenGreater,
}

/// How many replicas must commit before the server answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireCommitLevel {
    /// Master and all replicas.
    CommitAll,
    /// Master only.
    CommitMaster,
}

/// Record time-to-live.
///
/// The sentinels are named rather than encoded as magic negative integers,
/// because `-1` and `-2` meaning "never" and "don't touch" is exactly the kind
/// of detail that gets mixed up crossing a language boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireExpiration {
    /// Use the namespace's `default-ttl`.
    NamespaceDefault,
    /// Never expire.
    Never,
    /// Leave the current TTL untouched.
    DontUpdate,
    /// Expire this many seconds from now.
    Seconds(u32),
}

/// Optional per-call overrides. `None` everywhere means "use the daemon's
/// defaults", which is what [`Default`] produces.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WirePolicy {
    // ----- applies to any command -----
    /// Deadline for the whole command, including retries.
    pub total_timeout_ms: Option<u32>,
    /// Deadline for one socket operation.
    pub socket_timeout_ms: Option<u32>,
    /// Retries after the first attempt.
    pub max_retries: Option<u32>,
    /// Pause between retries.
    pub sleep_between_retries_ms: Option<u32>,
    /// Node preference.
    pub replica: Option<WireReplica>,
    /// AP-namespace read consistency.
    pub read_mode_ap: Option<WireReadModeAp>,
    /// SC-namespace read consistency.
    pub read_mode_sc: Option<WireReadModeSc>,
    /// Compress the request when it is large enough to be worth it.
    pub use_compression: Option<bool>,
    /// Store the user key alongside the digest.
    pub send_key: Option<bool>,
    /// The multi-record transaction this command runs inside, by id.
    ///
    /// Rides on the policy because that is where `aerospike-core` puts it —
    /// `base_policy.txn` — so every single-record verb and `batch` can join a
    /// transaction with no extra shape. A **query cannot**: an MRT covers records
    /// named by key, and a scan names none, so the daemon refuses it there rather
    /// than ignoring it. See [`crate::txn`].
    ///
    /// Not a write-only field: a transaction reads as well as writes, and its
    /// reads are exactly what the commit verifies.
    pub txn: Option<i64>,

    /// A filter the server applies before the command touches the record.
    ///
    /// Any of the three forms — see [`WireExpression`]. Built trees work against
    /// every server; the Aerospike Expression Language text form needs server
    /// 8.1.3+, and the daemon checks node versions for that one rather than
    /// letting an older server reject the text obscurely.
    ///
    /// A record the filter rejects comes back as [`crate::StatusCode::SERVER`]
    /// with result code 27 (`FILTERED_OUT`), for reads and writes alike: that
    /// is what `aerospike-core` surfaces, and reporting `OK` for a write that
    /// did not land would be worse.
    pub filter: Option<WireExpression>,

    // ----- writes only -----
    /// Create/update/replace semantics. Prefer the dedicated verbs where they
    /// exist; this is for callers that need the raw setting.
    pub record_exists_action: Option<WireRecordExistsAction>,
    /// Generation guard.
    pub generation_policy: Option<WireGenerationPolicy>,
    /// The generation to expect, with a guard set.
    pub generation: Option<u32>,
    /// Time-to-live for the record this write touches.
    pub expiration: Option<WireExpiration>,
    /// Replica commit requirement.
    pub commit_level: Option<WireCommitLevel>,
    /// Leave a tombstone on delete (Enterprise only).
    pub durable_delete: Option<bool>,
    /// Return a result slot for every operation, including ones that normally
    /// report nothing.
    pub respond_per_each_op: Option<bool>,
}

impl WirePolicy {
    /// Names of the write-only fields this policy sets.
    ///
    /// Lets the daemon reject `record_exists_action` on a read with a message
    /// naming the field, instead of silently dropping it — a silently ignored
    /// policy is a bug that surfaces much later, as data that is subtly wrong.
    #[must_use]
    pub fn write_only_fields_set(&self) -> Vec<&'static str> {
        let mut set = Vec::new();
        if self.record_exists_action.is_some() {
            set.push("record_exists_action");
        }
        if self.generation_policy.is_some() {
            set.push("generation_policy");
        }
        if self.generation.is_some() {
            set.push("generation");
        }
        if self.expiration.is_some() {
            set.push("expiration");
        }
        if self.commit_level.is_some() {
            set.push("commit_level");
        }
        if self.durable_delete.is_some() {
            set.push("durable_delete");
        }
        if self.respond_per_each_op.is_some() {
            set.push("respond_per_each_op");
        }
        set
    }

    /// Whether nothing at all is overridden.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == WirePolicy::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_policy_overrides_nothing() {
        let policy = WirePolicy::default();
        assert!(policy.is_empty());
        assert!(policy.write_only_fields_set().is_empty());
        assert_eq!(policy.filter, None);
    }

    #[test]
    fn write_only_fields_are_named_for_rejection() {
        let policy = WirePolicy {
            expiration: Some(WireExpiration::Never),
            durable_delete: Some(true),
            total_timeout_ms: Some(500),
            ..WirePolicy::default()
        };
        // The shared field must not be reported as write-only.
        assert_eq!(
            policy.write_only_fields_set(),
            vec!["expiration", "durable_delete"]
        );
        assert!(!policy.is_empty());
    }

    #[test]
    fn policies_round_trip_through_the_body_codec() {
        let policy = WirePolicy {
            total_timeout_ms: Some(1_000),
            socket_timeout_ms: Some(250),
            max_retries: Some(2),
            sleep_between_retries_ms: Some(10),
            replica: Some(WireReplica::PreferRack),
            read_mode_ap: Some(WireReadModeAp::All),
            read_mode_sc: Some(WireReadModeSc::Linearize),
            use_compression: Some(true),
            send_key: Some(false),
            filter: Some(WireExpression::Ael("$.age > 21".into())),
            txn: Some(-9_000_000_000_000_000_000),
            record_exists_action: Some(WireRecordExistsAction::CreateOnly),
            generation_policy: Some(WireGenerationPolicy::ExpectGenEqual),
            generation: Some(7),
            expiration: Some(WireExpiration::Seconds(600)),
            commit_level: Some(WireCommitLevel::CommitMaster),
            durable_delete: Some(true),
            respond_per_each_op: Some(true),
        };
        let bytes = crate::encode_body(&policy).unwrap();
        assert_eq!(crate::decode_body::<WirePolicy>(&bytes).unwrap(), policy);
    }

    #[test]
    fn ttl_sentinels_are_named_not_magic_numbers() {
        // The point of the enum: a caller cannot accidentally write -1 meaning
        // "never" when it meant "namespace default".
        for expiration in [
            WireExpiration::NamespaceDefault,
            WireExpiration::Never,
            WireExpiration::DontUpdate,
            WireExpiration::Seconds(0),
            WireExpiration::Seconds(u32::MAX),
        ] {
            let bytes = crate::encode_body(&expiration).unwrap();
            assert_eq!(
                crate::decode_body::<WireExpiration>(&bytes).unwrap(),
                expiration
            );
        }
    }
}
