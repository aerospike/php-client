// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Multi-record transactions.
//!
//! A transaction spans several commands against several records, and either all
//! of its writes land or none do. Aerospike calls it an MRT; it needs server 8.0
//! or later **and a strong-consistency namespace**.
//!
//! # The transaction lives in the daemon, and that is unavoidable
//!
//! A `Txn` accumulates the keys it has read and written as commands run, so that
//! the commit can verify every read version and roll every write forward. That
//! set is real state with no server-side home before the commit, so the daemon
//! holds it and PHP holds a number.
//!
//! Compare the two other handles this contract has, because the difference is the
//! whole design:
//!
//! | handle | where the state is | what an abandoned one costs |
//! | --- | --- | --- |
//! | [`crate::admin::WireTaskHandle`] | on the server | nothing; it is only a description |
//! | [`crate::query`] cursor | in the daemon | memory, until it expires |
//! | this | in the daemon **and** on the server | **record locks**, until it is aborted |
//!
//! That last row is why an abandoned transaction is not merely dropped when it
//! expires: the server is holding a monitor record and locks on everything the
//! transaction wrote, so the daemon **aborts** it. The server's own MRT timeout
//! would eventually do the same, but "eventually" is the wrong answer for a lock.
//!
//! # The id is the server's, not the daemon's
//!
//! [`WireTxnHandle::id`] is `aerospike-core`'s own transaction id — the number the
//! server knows the transaction by — so it is the same number in the daemon's
//! registry, in a server log, and in PHP. A registry key invented here would have
//! been one more identifier to correlate.
//!
//! # A transaction travels on the policy
//!
//! There is no "run this command in this transaction" opcode: the id goes in
//! [`WirePolicy::txn`](crate::WirePolicy::txn), exactly as `aerospike-core` puts
//! the `Txn` on `base_policy.txn`. So every single-record verb and `batch` can
//! join a transaction without a second shape for each of them.
//!
//! Queries cannot. An MRT covers records named by key, and a scan or query names
//! none — so a transaction on a query policy is refused rather than ignored.

use serde::{Deserialize, Serialize};

use crate::policy::WirePolicy;

/// Payload of a [`TXN_BEGIN`](crate::opcode::TXN_BEGIN) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTxnBeginBody {
    /// Cluster instance the transaction runs against.
    ///
    /// A transaction belongs to one cluster: its reads and writes are verified
    /// and rolled forward there, so there is nothing coherent a second cluster
    /// could contribute.
    pub instance: String,
    /// How long the server should hold the transaction open before expiring it
    /// itself, in milliseconds. `None` uses the server's default.
    ///
    /// This is the *server's* timeout, and it is the backstop that makes an
    /// abandoned transaction eventually harmless even if the daemon dies too.
    pub timeout_ms: Option<u32>,
}

/// Payload of a [`TXN_BEGIN`](crate::opcode::TXN_BEGIN) reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTxnHandle {
    /// The transaction's id, as `aerospike-core` and the server know it.
    pub id: i64,
}

/// Payload of a [`TXN_STATE`](crate::opcode::TXN_STATE) request: which
/// transaction, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTxnBody {
    /// Cluster instance.
    pub instance: String,
    /// The transaction's id.
    pub id: i64,
}

/// Payload of a [`TXN_COMMIT`](crate::opcode::TXN_COMMIT) request.
///
/// # Two policies, because a commit is two steps
///
/// A commit **verifies** every record the transaction read — a batch of
/// version checks — and then **rolls** its writes forward, another batch. The
/// client has a separate policy for each, with different defaults: the verify
/// batch reads linearizably, and the roll batch goes to the master. So both are
/// carried, and either may be left to the client's own default.
///
/// `None` is not the same as an empty [`WirePolicy`]: an empty one overrides
/// nothing but still means "resolve a policy", while `None` says the caller had
/// no opinion at all. They behave identically today, and keeping them distinct
/// costs nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireTxnCommitBody {
    /// Cluster instance.
    pub instance: String,
    /// The transaction's id.
    pub id: i64,
    /// Overrides for the read-verification batch.
    pub verify: Option<WirePolicy>,
    /// Overrides for the roll-forward batch.
    pub roll: Option<WirePolicy>,
}

/// Payload of a [`TXN_ABORT`](crate::opcode::TXN_ABORT) request.
///
/// One policy, not two: an abort has nothing to verify — it is discarding the
/// writes, so their read versions no longer matter — and only rolls back. That
/// asymmetry is the client's, and it is why this is its own body rather than a
/// commit body with an ignored field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireTxnAbortBody {
    /// Cluster instance.
    pub instance: String,
    /// The transaction's id.
    pub id: i64,
    /// Overrides for the roll-back batch.
    pub roll: Option<WirePolicy>,
}

/// Where a transaction has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireTxnState {
    /// Accepting commands.
    Open,
    /// Every read has been verified; the commit is part-done.
    Verified,
    /// Committed. Its writes are permanent.
    Committed,
    /// Aborted. Its writes are gone.
    Aborted,
}

/// How a commit ended.
///
/// Every variant is a *success*: the transaction committed. The three besides
/// [`Ok`](Self::Ok) say that some tidying up was left to the server, which is
/// worth reporting but is not a reason to retry — retrying a committed
/// transaction is the one thing a caller must not do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireCommitStatus {
    /// Committed, and everything was tidied up.
    Ok,
    /// It had already been committed. Not an error: the same answer as
    /// committing it now.
    AlreadyCommitted,
    /// Committed, but the client gave up rolling the writes forward. The server
    /// will finish.
    RollForwardAbandoned,
    /// Committed and rolled forward, but the client gave up closing the
    /// transaction's monitor record. The server will.
    CloseAbandoned,
}

/// How an abort ended.
///
/// As with a commit, every variant means the transaction is *not* going to land:
/// the differences are only in how much tidying the server was left to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireAbortStatus {
    /// Aborted, and everything was tidied up.
    Ok,
    /// It had already been aborted.
    AlreadyAborted,
    /// Aborted, but the client gave up rolling the writes back. The server will.
    RollBackAbandoned,
    /// Rolled back, but the client gave up closing the monitor record.
    CloseAbandoned,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_body, encode_body, WirePolicy};

    #[test]
    fn a_begin_and_its_handle_round_trip() {
        let body = WireTxnBeginBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            timeout_ms: Some(10_000),
        };
        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<WireTxnBeginBody>(&bytes).unwrap(), body);

        // The id is the server's own, so it spans the whole i64 range including
        // negatives — a registry key invented here would not have.
        for id in [i64::MIN, -1, 0, 1, i64::MAX] {
            let handle = WireTxnHandle { id };
            let bytes = encode_body(&handle).unwrap();
            assert_eq!(decode_body::<WireTxnHandle>(&bytes).unwrap(), handle);
        }
    }

    #[test]
    fn a_transaction_travels_on_the_policy() {
        // No per-command "join this transaction" shape: the id rides on the
        // policy, exactly as the client puts the `Txn` on `base_policy.txn`.
        let policy = WirePolicy {
            txn: Some(-42),
            ..WirePolicy::default()
        };
        let bytes = encode_body(&policy).unwrap();
        assert_eq!(decode_body::<WirePolicy>(&bytes).unwrap().txn, Some(-42));

        // And it is not a write-only field: a read joins a transaction too, so
        // refusing it on a read would break the half of a transaction that reads.
        assert!(
            !policy.write_only_fields_set().contains(&"txn"),
            "a read must be able to join a transaction"
        );
        assert!(WirePolicy::default().txn.is_none());
        assert!(!policy.is_empty());
    }

    #[test]
    fn every_state_and_status_round_trips() {
        for state in [
            WireTxnState::Open,
            WireTxnState::Verified,
            WireTxnState::Committed,
            WireTxnState::Aborted,
        ] {
            let bytes = encode_body(&state).unwrap();
            assert_eq!(decode_body::<WireTxnState>(&bytes).unwrap(), state);
        }
        for status in [
            WireCommitStatus::Ok,
            WireCommitStatus::AlreadyCommitted,
            WireCommitStatus::RollForwardAbandoned,
            WireCommitStatus::CloseAbandoned,
        ] {
            let bytes = encode_body(&status).unwrap();
            assert_eq!(decode_body::<WireCommitStatus>(&bytes).unwrap(), status);
        }
        for status in [
            WireAbortStatus::Ok,
            WireAbortStatus::AlreadyAborted,
            WireAbortStatus::RollBackAbandoned,
            WireAbortStatus::CloseAbandoned,
        ] {
            let bytes = encode_body(&status).unwrap();
            assert_eq!(decode_body::<WireAbortStatus>(&bytes).unwrap(), status);
        }
    }

    #[test]
    fn a_txn_request_is_small() {
        // Every command in a transaction carries the id, so it has to be cheap.
        let body = WireTxnBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            id: i64::MAX,
        };
        let bytes = encode_body(&body).unwrap();
        assert!(bytes.len() < 32, "{} bytes", bytes.len());
    }

    /// A commit carries two policies because it is two batches — verify, then
    /// roll forward — and an abort carries one because it has nothing to verify.
    #[test]
    fn finishing_a_transaction_carries_the_policies_each_step_needs() {
        let overrides = WirePolicy {
            total_timeout_ms: Some(30_000),
            max_retries: Some(2),
            ..WirePolicy::default()
        };

        let commit = WireTxnCommitBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            id: -1,
            verify: Some(overrides.clone()),
            roll: Some(overrides.clone()),
        };
        let bytes = encode_body(&commit).unwrap();
        assert_eq!(decode_body::<WireTxnCommitBody>(&bytes).unwrap(), commit);

        // An abort has no `verify` field at all: it is discarding the writes, so
        // the versions it read no longer matter.
        let abort = WireTxnAbortBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            id: -1,
            roll: Some(overrides),
        };
        let bytes = encode_body(&abort).unwrap();
        assert_eq!(decode_body::<WireTxnAbortBody>(&bytes).unwrap(), abort);

        // Both default to "no opinion", which is the ordinary case and must stay
        // distinguishable from an empty policy that overrides nothing.
        for (verify, roll) in [(None, None), (None, Some(WirePolicy::default()))] {
            let body = WireTxnCommitBody {
                instance: crate::DEFAULT_INSTANCE.into(),
                id: 1,
                verify,
                roll,
            };
            let bytes = encode_body(&body).unwrap();
            assert_eq!(decode_body::<WireTxnCommitBody>(&bytes).unwrap(), body);
        }
    }
}
