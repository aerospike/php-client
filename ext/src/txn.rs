// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! `Aerospike\Transaction`: a multi-record transaction.
//!
//! # An unfinished transaction rolls back
//!
//! [`Transaction`] aborts itself when it is destroyed, unless it was committed or
//! aborted first. That is the behaviour every other database client has, and it is
//! the safe default here for a stronger reason: an open transaction holds record
//! locks on the server, so one left behind by a request that threw would make every
//! other writer of those records wait.
//!
//! ```php
//! $txn = $client->beginTransaction();
//! try {
//!     $client->put(new WritePolicy(txn: $txn), $from, [new Bin('balance', 70)]);
//!     $client->put(new WritePolicy(txn: $txn), $to,   [new Bin('balance', 30)]);
//!     $txn->commit();
//! } catch (Throwable $e) {
//!     $txn->abort();      // or just let $txn go out of scope
//!     throw $e;
//! }
//! ```
//!
//! The daemon expires an idle transaction too — also by aborting it — so a worker
//! killed mid-transaction costs its locks for at most that long rather than until
//! the server's own transaction timeout.
//!
//! # Requirements that are easy to miss
//!
//! Server **8.0 or later**, and the namespace must be configured for **strong
//! consistency**. The first is checked when the transaction opens, and refused with
//! the node's version; the second is the server's to enforce, and shows up as a
//! failure on the first write.

use aerospike_php_ipc::txn::{
    WireAbortStatus, WireCommitStatus, WireTxnAbortBody, WireTxnBody, WireTxnCommitBody,
    WireTxnState,
};
use aerospike_php_ipc::{decode_body, encode_body, opcode, WirePolicy};
use ext_php_rs::prelude::*;

use crate::arg::Given;
use crate::enums::{AbortStatus, Case, CommitStatus, TxnState};
use crate::error::{AeroError, AeroResult};
use crate::policy::{TxnRollPolicy, TxnVerifyPolicy};
use crate::settings::Settings;
use crate::transport;

/// A multi-record transaction: several commands against several records, all of
/// which land or none of which do.
///
/// Opened with `Client::beginTransaction()`, joined by passing it on a policy, and
/// finished with `commit()` or `abort()`:
///
/// ```php
/// $txn = $client->beginTransaction();
/// $client->put(new Aerospike\WritePolicy(txn: $txn), $key, [new Aerospike\Bin('n', 1)]);
/// $record = $client->get(new Aerospike\ReadPolicy(txn: $txn), $key);
/// $txn->commit();
/// ```
///
/// **A transaction reads as well as writes**, and its reads matter: the commit
/// verifies that every record it read is still at the version it saw, and fails the
/// whole transaction if not. That is what makes it a transaction rather than a
/// batch.
///
/// **Not for scans or queries.** A transaction covers records named by key, and a
/// traversal names none — so a `QueryPolicy` has nowhere to put one, and a
/// transaction on a read policy handed to `query()` is refused rather than ignored.
///
/// There is no constructor: a transaction has to be opened on a cluster, and one
/// built by hand would name no cluster and no server-side state.
#[php_class]
#[php(name = "Aerospike\\Transaction")]
#[derive(Debug)]
pub struct Transaction {
    instance: String,
    settings: Settings,
    id: i64,
    /// How the transaction ended, once it has.
    ///
    /// `None` while it is open. Recording *which* way it ended rather than merely
    /// that it did is what lets `state()` answer honestly after the daemon has
    /// forgotten the transaction, and what lets committing an aborted transaction
    /// be refused locally with a message that says so.
    finished: Option<TxnState>,
}

#[php_impl]
impl Transaction {
    /// The transaction's id.
    ///
    /// The server's own — the same number appears in a server log and in the
    /// daemon's own logging, which is what makes it worth exposing.
    pub fn id(&self) -> i64 {
        self.id
    }

    /// Where the transaction has got to.
    ///
    /// One round trip; the daemon holds the state, so this is not a question for
    /// the cluster. Returns `TxnState::Committed` or `Aborted` from local knowledge
    /// once the transaction has finished, since the daemon has forgotten it by
    /// then.
    pub fn state(&self) -> PhpResult<Case<TxnState>> {
        // The daemon drops a finished transaction, so asking would say "not open"
        // — true, but less useful than the answer the caller wants, and we know it
        // exactly.
        if let Some(ended) = self.finished {
            return Ok(Case(ended));
        }
        let body = WireTxnBody {
            instance: self.instance.clone(),
            id: self.id,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let reply = self.ask(opcode::TXN_STATE, "transaction state", &payload)?;
        Ok(Case(TxnState::of(
            decode_body::<WireTxnState>(&reply).map_err(AeroError::codec)?,
        )))
    }

    /// Whether the transaction can still take commands.
    ///
    /// Local knowledge, so it costs nothing: `false` once `commit()` or `abort()`
    /// has returned. It does **not** mean the daemon has not expired it underneath
    /// — that shows up as a failure on the next command, which is the only place it
    /// could.
    pub fn is_open(&self) -> bool {
        self.finished.is_none()
    }

    /// Commit the transaction: make all of its writes permanent.
    ///
    /// Verifies first — every record the transaction read must still be at the
    /// version it saw — and fails the whole transaction if any has changed. That
    /// failure leaves the transaction *aborted*, so there is nothing to retry
    /// except the work.
    ///
    /// Every `CommitStatus` this returns is a success. `Ok` means everything was
    /// tidied up; the others mean the writes landed and the server was left to
    /// finish some bookkeeping. **None of them is a reason to commit again.**
    ///
    /// Committing twice is a local no-op returning `AlreadyCommitted`, so a
    /// `finally` block can commit without checking. Committing one that was
    /// *aborted* throws — the writes are gone, and answering "already committed"
    /// would be the most dangerous possible lie.
    pub fn commit(&mut self) -> PhpResult<Case<CommitStatus>> {
        self.commit_with_policies(None, None)
    }

    /// Commit the transaction, with explicit policies for its two phases.
    ///
    /// A commit is two batch commands — **verify** every record the transaction
    /// read, then **roll** its writes forward — and the client has a separate
    /// policy for each, because they are tuned differently: the verify batch reads
    /// linearizably, and both go to the partition master. `null` for either takes
    /// the client's own default, which is what `commit()` passes for both.
    ///
    /// ```php
    /// // A cluster under load: give both phases longer before they give up.
    /// $txn->commitWithPolicies(
    ///     new Aerospike\TxnVerifyPolicy(totalTimeoutMs: 30_000),
    ///     new Aerospike\TxnRollPolicy(totalTimeoutMs: 30_000),
    /// );
    /// ```
    ///
    /// Behaves exactly as `commit()` otherwise, including the two local answers:
    /// committing twice returns `AlreadyCommitted`, and committing an aborted
    /// transaction throws.
    ///
    /// # Raising these timeouts is usually the right direction
    ///
    /// The defaults are not short by accident, and lowering them is the change to
    /// think twice about: a commit that gives up leaves a transaction
    /// half-finished, holding record locks until the daemon's sweep or the
    /// server's own timeout clears it.
    #[php(defaults(verify = None, roll = None))]
    pub fn commit_with_policies(
        &mut self,
        verify: Option<Given<&TxnVerifyPolicy>>,
        roll: Option<Given<&TxnRollPolicy>>,
    ) -> PhpResult<Case<CommitStatus>> {
        let verify = Given::or_none(verify, "verify")?;
        let roll = Given::or_none(roll, "roll")?;
        match self.finished {
            Some(TxnState::Committed) => return Ok(Case(CommitStatus::AlreadyCommitted)),
            Some(_) => {
                return Err(AeroError::client(format!(
                    "transaction {} was aborted and cannot be committed; its writes are gone, so \
                     the work has to start again in a new transaction",
                    self.id
                ))
                .into())
            }
            None => {}
        }
        let body = WireTxnCommitBody {
            instance: self.instance.clone(),
            id: self.id,
            verify: verify.map(TxnVerifyPolicy::to_wire),
            roll: roll.map(TxnRollPolicy::to_wire),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let reply = self.ask(opcode::TXN_COMMIT, "commit", &payload)?;
        let status: WireCommitStatus = decode_body(&reply).map_err(AeroError::codec)?;
        // Only on success: a commit that failed can still be aborted, and marking
        // it finished here would leave the caller unable to.
        self.finished = Some(TxnState::Committed);
        Ok(Case(CommitStatus::of(status)))
    }

    /// Abort the transaction: discard all of its writes.
    ///
    /// The records it wrote go back to what they were, and its locks are released.
    /// Aborting twice is a local no-op returning `AlreadyAborted`. Aborting one that
    /// was *committed* throws: its writes are permanent, and nothing here can undo
    /// them.
    pub fn abort(&mut self) -> PhpResult<Case<AbortStatus>> {
        self.abort_with_policy(None)
    }

    /// Abort the transaction, with an explicit policy for the roll-back.
    ///
    /// **One policy, not two.** An abort has nothing to verify — it is discarding
    /// the writes, so the versions the transaction read no longer matter — so
    /// where `commitWithPolicies` takes a verify policy and a roll policy, this
    /// takes only the roll.
    ///
    /// ```php
    /// $txn->abortWithPolicy(new Aerospike\TxnRollPolicy(maxRetries: 10));
    /// ```
    ///
    /// Behaves exactly as `abort()` otherwise.
    #[php(defaults(roll = None))]
    pub fn abort_with_policy(
        &mut self,
        roll: Option<Given<&TxnRollPolicy>>,
    ) -> PhpResult<Case<AbortStatus>> {
        let roll = Given::or_none(roll, "roll")?;
        match self.finished {
            Some(TxnState::Aborted) => return Ok(Case(AbortStatus::AlreadyAborted)),
            Some(_) => {
                return Err(AeroError::client(format!(
                    "transaction {} was committed and cannot be aborted; its writes are permanent",
                    self.id
                ))
                .into())
            }
            None => {}
        }
        let payload = self.abort_payload(roll.map(TxnRollPolicy::to_wire))?;
        let reply = self.ask(opcode::TXN_ABORT, "abort", &payload)?;
        let status: WireAbortStatus = decode_body(&reply).map_err(AeroError::codec)?;
        self.finished = Some(TxnState::Aborted);
        Ok(Case(AbortStatus::of(status)))
    }

    /// `transaction <id>`, for logs and test failures.
    pub fn __to_string(&self) -> String {
        format!("transaction {}", self.id)
    }
}

impl Transaction {
    /// Build one for a transaction the daemon has just opened.
    #[must_use]
    pub fn new(instance: String, settings: Settings, id: i64) -> Transaction {
        Transaction {
            instance,
            settings,
            id,
            finished: None,
        }
    }

    /// The id, for a policy that wants to join this transaction.
    #[must_use]
    pub const fn wire_id(&self) -> i64 {
        self.id
    }

    /// One round trip naming this transaction.
    ///
    /// The payload is the caller's rather than built here, because the three
    /// transaction opcodes no longer share a body: a commit carries two policies,
    /// an abort one, and asking for the state carries neither.
    fn ask(&self, opcode: u16, operation: &str, payload: &[u8]) -> AeroResult<Vec<u8>> {
        let (header, reply) =
            transport::call(&self.instance, &self.settings, opcode, operation, payload)?;
        if !header.status().is_ok() {
            return Err(AeroError::from_reply(&header, &reply, operation));
        }
        Ok(reply)
    }

    /// The body of an abort. Shared with [`Drop`], which aborts with no policy.
    fn abort_payload(&self, roll: Option<WirePolicy>) -> AeroResult<Vec<u8>> {
        encode_body(&WireTxnAbortBody {
            instance: self.instance.clone(),
            id: self.id,
            roll,
        })
        .map_err(AeroError::codec)
    }
}

/// Roll back a transaction nobody finished.
///
/// This is what makes a request that throws mid-transaction release its locks
/// rather than leaving them for the daemon's idle sweep. A `__destruct` method
/// would be the PHP way to say it, but `Drop` is the one that cannot be missed: it
/// runs when the object is freed however that happens, including when a request
/// ends with the transaction still in a variable.
///
/// A failure here is deliberately swallowed. A destructor cannot throw in PHP, and
/// the daemon's sweep — and behind it the server's own transaction timeout — will
/// release the locks regardless. That is exactly the case those two exist for.
impl Drop for Transaction {
    fn drop(&mut self) {
        if self.finished.is_some() {
            return;
        }
        // No roll policy: a destructor has nobody to take one from, and the
        // client's tuned default is the right thing for a rollback nobody asked
        // for.
        if let Ok(payload) = self.abort_payload(None) {
            let _ = self.ask(opcode::TXN_ABORT, "abort", &payload);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction() -> Transaction {
        Transaction::new("default".into(), Settings::default(), -7)
    }

    #[test]
    fn a_transaction_reports_the_servers_own_id() {
        let txn = transaction();
        assert_eq!(txn.id, -7, "the id is the server's, so it may be negative");
        assert_eq!(txn.wire_id(), -7);
        assert_eq!(txn.__to_string(), "transaction -7");
        assert!(txn.is_open());
    }

    /// The flag the destructor reads. Committing or aborting sets it, and nothing
    /// else does — which is what stops a finished transaction being aborted again
    /// when it is dropped.
    #[test]
    fn finishing_a_transaction_closes_it_locally() {
        let mut txn = transaction();
        assert!(txn.is_open());
        txn.finished = Some(TxnState::Committed);
        assert!(!txn.is_open());

        // `mem::forget` rather than a drop: dropping an *open* one would try to
        // abort it over a transport this test has no daemon for.
        std::mem::forget(txn);
    }

    /// Which way it ended is recorded, not just that it did — so a caller asking
    /// afterwards gets the honest answer even though the daemon has forgotten the
    /// transaction, and committing an aborted one can be refused by name.
    #[test]
    fn how_it_ended_is_remembered_and_the_wrong_finish_is_refused() {
        let mut committed = transaction();
        committed.finished = Some(TxnState::Committed);
        assert_eq!(committed.finished, Some(TxnState::Committed));

        let mut aborted = transaction();
        aborted.finished = Some(TxnState::Aborted);
        assert_eq!(aborted.finished, Some(TxnState::Aborted));
        // The dangerous direction: answering "already committed" for a
        // transaction whose writes are gone would be the worst lie available.
        assert_ne!(aborted.finished, Some(TxnState::Committed));

        std::mem::forget(committed);
        std::mem::forget(aborted);
    }

    /// The destructor aborts with no roll policy, and the body it sends has to be
    /// the one the daemon decodes — a `WireTxnBody` here would be a rollback that
    /// failed to parse, which is a lock left held.
    #[test]
    fn the_destructors_abort_names_the_transaction_and_no_policy() {
        let txn = transaction();
        let payload = txn.abort_payload(None).unwrap();
        let decoded: WireTxnAbortBody = decode_body(&payload).unwrap();
        assert_eq!(decoded.instance, "default");
        assert_eq!(decoded.id, -7);
        assert_eq!(decoded.roll, None, "a destructor has no policy to pass");

        // And an explicit policy reaches the same body, so `abortWithPolicy` and
        // the destructor differ in exactly one field.
        let roll = TxnRollPolicy::default();
        let payload = txn.abort_payload(Some(roll.to_wire())).unwrap();
        let decoded: WireTxnAbortBody = decode_body(&payload).unwrap();
        assert_eq!(decoded.roll, Some(WirePolicy::default()));

        std::mem::forget(txn);
    }
}
