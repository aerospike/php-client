// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Multi-record transactions: the open ones, and what happens to the ones nobody
//! finishes.
//!
//! The contract's side is [`aerospike_php_ipc::txn`]. What lives here is the
//! registry, and the one decision that makes it different from every other handle
//! this daemon hands out.
//!
//! # An expired transaction is aborted, not dropped
//!
//! A scan cursor that expires costs the memory it held. A transaction that expires
//! is holding **record locks and a monitor record on the server**, and every other
//! writer of those records waits behind it. So [`Transactions::expire`] does not
//! simply forget an idle transaction — it aborts it, which releases the locks now
//! rather than when the server's own MRT timeout gets round to it.
//!
//! That is why expiry here is `async` and takes a client, where the cursor sweep is
//! a synchronous `retain`. The asymmetry is the point.
//!
//! # A transaction is shared, not checked out
//!
//! Unlike a cursor, a transaction is *borrowed* by every command that runs inside
//! it and stays in the registry throughout: `Txn`'s own fields are behind
//! `RwLock`s, so concurrent commands in one transaction are the client's business
//! and not something to serialise here. [`Transactions::get`] therefore clones the
//! `Arc` rather than removing the entry.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use aerospike_core::{AbortStatus, Client, CommitStatus, Txn, TxnState};
use aerospike_php_ipc::txn::{WireAbortStatus, WireCommitStatus, WireTxnState};

/// How long a transaction may sit unused before the daemon aborts it.
///
/// Shorter than the cursor default, and deliberately: an idle cursor costs memory,
/// an idle transaction costs other writers their locks.
pub const DEFAULT_TXN_IDLE: Duration = Duration::from_secs(30);

/// How many transactions may be open at once, across every worker.
pub const DEFAULT_MAX_TXNS: usize = 1_024;

/// Whether a cluster can run multi-record transactions.
///
/// Its own type rather than a bare `bool` for the same reason
/// [`crate::policy::AelSupport`] is: the interesting case is *why* not, and a
/// message that names the node and its version is the difference between a
/// fixable complaint and a puzzle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MrtSupport {
    /// Every node is new enough (8.0+).
    Supported,
    /// At least one node is not.
    Unsupported {
        /// The node that is too old.
        node: String,
        /// The version it reported.
        version: String,
    },
    /// There is no cluster view to ask — not connected, or still tending.
    Unknown,
}

impl MrtSupport {
    /// Ask `client`'s current cluster view.
    ///
    /// All-nodes: a transaction's records can live on any of them, so one old node
    /// makes the answer no.
    #[must_use]
    pub fn of(client: &Client) -> MrtSupport {
        let nodes = client.nodes();
        if nodes.is_empty() {
            return MrtSupport::Unknown;
        }
        for node in nodes {
            // The client's own gate rather than a version comparison
            // re-implemented here, so the two cannot drift.
            if !node.version().supports_mrt() {
                let v = node.version();
                return MrtSupport::Unsupported {
                    node: node.name().to_string(),
                    version: format!("{}.{}.{}.{}", v.major, v.minor, v.patch, v.build),
                };
            }
        }
        MrtSupport::Supported
    }

    /// The complaint for a cluster that cannot, or `None` when it can.
    ///
    /// A message rather than an error type: the caller turns it into a reply, and
    /// there is exactly one thing to say.
    #[must_use]
    pub fn refusal(&self) -> Option<String> {
        match self {
            MrtSupport::Supported => None,
            MrtSupport::Unsupported { node, version } => Some(format!(
                "multi-record transactions need Aerospike 8.0 or later, but node '{node}' runs \
                 {version}; they also need a strong-consistency namespace, which is worth \
                 checking before upgrading for this"
            )),
            MrtSupport::Unknown => Some(
                "multi-record transactions need a server of 8.0 or later, and this instance has \
                 no connected node to check; retry once the cluster is reachable"
                    .to_string(),
            ),
        }
    }
}

/// One open transaction.
struct Entry {
    txn: Arc<Txn>,
    /// The instance it was opened against. A transaction's reads are verified and
    /// its writes rolled forward on one cluster, so a second one has nothing
    /// coherent to contribute — and a command naming a different instance must not
    /// be able to join it.
    instance: String,
    last_used: Instant,
}

/// The open transactions, keyed by the id the server knows them by.
///
/// A plain `std::sync::Mutex`: every critical section is a map operation with no
/// `.await` inside. The transactions themselves are `Arc<Txn>`, so a command holds
/// one while the lock is long released.
#[derive(Default)]
pub struct Transactions {
    open: Mutex<HashMap<i64, Entry>>,
    idle_timeout: Duration,
    max_open: usize,
}

impl std::fmt::Debug for Transactions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transactions")
            .field("open", &self.open_count())
            .field("idle_timeout", &self.idle_timeout)
            .field("max_open", &self.max_open)
            .finish()
    }
}

impl Transactions {
    /// A registry aborting transactions after `idle_timeout` and holding at most
    /// `max_open`.
    #[must_use]
    pub fn new(idle_timeout: Duration, max_open: usize) -> Transactions {
        Transactions {
            open: Mutex::new(HashMap::new()),
            idle_timeout,
            max_open: max_open.max(1),
        }
    }

    /// Register a transaction, returning the id it is known by.
    ///
    /// The id is `aerospike-core`'s own, so the same number appears here, in a
    /// server log and in PHP.
    ///
    /// # Errors
    /// The number already open, when that is the configured maximum. Refusing is
    /// better than growing without bound: each open transaction is holding locks.
    pub fn open(&self, txn: Arc<Txn>, instance: String) -> Result<i64, usize> {
        let id = txn.id();
        let mut open = self.lock();
        if open.len() >= self.max_open {
            return Err(open.len());
        }
        open.insert(
            id,
            Entry {
                txn,
                instance,
                last_used: Instant::now(),
            },
        );
        Ok(id)
    }

    /// The transaction with this id, if it is open on `instance`.
    ///
    /// Borrowed rather than removed: several commands run inside one transaction,
    /// and it stays open until it is committed or aborted. Touches the idle clock,
    /// so a transaction being used is never reaped underneath its caller.
    ///
    /// `None` when the id is unknown, when it expired, or when it belongs to
    /// another instance — the last of which is a caller mistake worth failing on
    /// rather than silently running the command outside the transaction.
    #[must_use]
    pub fn get(&self, id: i64, instance: &str) -> Option<Arc<Txn>> {
        let mut open = self.lock();
        let entry = open.get_mut(&id)?;
        if entry.instance != instance {
            return None;
        }
        entry.last_used = Instant::now();
        Some(entry.txn.clone())
    }

    /// Forget a transaction, returning whether it was there.
    ///
    /// Called after a commit or an abort has finished: the transaction is over, and
    /// leaving it registered would let a later command believe it was still open.
    pub fn close(&self, id: i64) -> bool {
        self.lock().remove(&id).is_some()
    }

    /// Abort every transaction idle for longer than the configured timeout.
    ///
    /// The important one. An abandoned transaction holds locks on everything it
    /// wrote, so this does not merely forget it — it rolls it back, and only then
    /// drops it. A failed abort is logged and the entry dropped anyway: the
    /// server's own MRT timeout is the backstop, and keeping an entry nobody will
    /// ever finish would mean the registry never drains.
    ///
    /// Returns how many were aborted.
    pub async fn expire(&self, instances: &crate::instance::Instances) -> usize {
        let expired = self.take_expired();
        let count = expired.len();
        for (id, instance, txn) in expired {
            let released = match instances.resolve(&instance) {
                crate::instance::Lookup::Ready(client) => client.abort(&txn).await.map(|_| ()),
                // No client to abort through; the server's timeout will do it.
                _ => Ok(()),
            };
            match released {
                Ok(()) => log::info!(
                    "aborted transaction {id} on '{instance}' after {:?} idle; its record locks \
                     are released",
                    self.idle_timeout
                ),
                Err(e) => log::warn!(
                    "transaction {id} on '{instance}' was idle for {:?} and could not be aborted \
                     ({e}); the server's own transaction timeout will release its locks",
                    self.idle_timeout
                ),
            }
        }
        count
    }

    /// Remove the idle entries, so the aborts happen with no lock held.
    fn take_expired(&self) -> Vec<(i64, String, Arc<Txn>)> {
        let now = Instant::now();
        let mut open = self.lock();
        let idle: Vec<i64> = open
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_used) >= self.idle_timeout)
            .map(|(id, _)| *id)
            .collect();
        idle.into_iter()
            .filter_map(|id| {
                open.remove(&id)
                    .map(|entry| (id, entry.instance, entry.txn))
            })
            .collect()
    }

    /// How many transactions are open.
    #[must_use]
    pub fn open_count(&self) -> usize {
        self.lock().len()
    }

    /// How long a transaction may idle before it is aborted.
    #[must_use]
    pub const fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// The most transactions that may be open at once.
    #[must_use]
    pub const fn max_open(&self) -> usize {
        self.max_open
    }

    /// A poisoned lock cannot happen — nothing here panics while holding it — and
    /// recovering beats propagating a panic into every later request.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Entry>> {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The contract's transaction state.
#[must_use]
pub const fn to_state(state: TxnState) -> WireTxnState {
    match state {
        TxnState::Open => WireTxnState::Open,
        TxnState::Verified => WireTxnState::Verified,
        TxnState::Committed => WireTxnState::Committed,
        TxnState::Aborted => WireTxnState::Aborted,
    }
}

/// The contract's commit status.
#[must_use]
pub const fn to_commit_status(status: &CommitStatus) -> WireCommitStatus {
    match status {
        CommitStatus::Ok => WireCommitStatus::Ok,
        CommitStatus::AlreadyCommitted => WireCommitStatus::AlreadyCommitted,
        CommitStatus::RollForwardAbandoned => WireCommitStatus::RollForwardAbandoned,
        CommitStatus::CloseAbandoned => WireCommitStatus::CloseAbandoned,
    }
}

/// The contract's abort status.
#[must_use]
pub const fn to_abort_status(status: &AbortStatus) -> WireAbortStatus {
    match status {
        AbortStatus::Ok => WireAbortStatus::Ok,
        AbortStatus::AlreadyAborted => WireAbortStatus::AlreadyAborted,
        AbortStatus::RollBackAbandoned => WireAbortStatus::RollBackAbandoned,
        AbortStatus::CloseAbandoned => WireAbortStatus::CloseAbandoned,
    }
}

/// A transaction with `timeout_ms` as its server-side deadline.
///
/// The timeout has to be set before the `Arc`, because `set_timeout` needs `&mut`
/// — which is also a useful constraint: a transaction's deadline cannot change
/// once commands have started joining it.
#[must_use]
pub fn new_txn(timeout_ms: Option<u32>) -> Arc<Txn> {
    let mut txn = Txn::new();
    if let Some(millis) = timeout_ms {
        txn.set_timeout(Duration::from_millis(u64::from(millis)));
    }
    Arc::new(txn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Transactions {
        Transactions::new(Duration::from_secs(30), 8)
    }

    #[test]
    fn a_transaction_is_registered_under_the_id_the_server_knows() {
        let registry = registry();
        let txn = new_txn(None);
        let expected = txn.id();

        let id = registry.open(txn, "default".into()).unwrap();
        assert_eq!(
            id, expected,
            "the registry key must be the client's own transaction id, so one \
             number identifies it everywhere"
        );
        assert_eq!(registry.open_count(), 1);
    }

    /// Borrowed, not checked out: several commands run inside one transaction, so
    /// a lookup must leave it open — the opposite of a scan cursor.
    #[test]
    fn a_transaction_stays_open_while_commands_use_it() {
        let registry = registry();
        let id = registry.open(new_txn(None), "default".into()).unwrap();

        for _ in 0..3 {
            assert!(registry.get(id, "default").is_some());
            assert_eq!(registry.open_count(), 1, "a lookup must not remove it");
        }

        assert!(registry.close(id));
        assert!(registry.get(id, "default").is_none());
        assert!(!registry.close(id), "closing twice is not an error");
    }

    /// A transaction belongs to the cluster it was opened on. A command naming
    /// another instance must fail rather than quietly run outside it.
    #[test]
    fn a_transaction_is_not_visible_from_another_instance() {
        let registry = registry();
        let id = registry.open(new_txn(None), "default".into()).unwrap();

        assert!(registry.get(id, "analytics").is_none());
        assert!(registry.get(id, "default").is_some());
    }

    #[test]
    fn too_many_open_transactions_is_refused_with_the_count() {
        let registry = Transactions::new(Duration::from_secs(30), 2);
        registry.open(new_txn(None), "default".into()).unwrap();
        registry.open(new_txn(None), "default".into()).unwrap();
        assert_eq!(
            registry.open(new_txn(None), "default".into()).unwrap_err(),
            2
        );

        assert_eq!(Transactions::new(Duration::from_secs(1), 0).max_open(), 1);
    }

    /// The idle ones are taken out with no lock held, because aborting them is
    /// async and must not block every other command in the daemon.
    #[test]
    fn expiry_takes_the_idle_ones_and_leaves_the_rest() {
        let immediate = Transactions::new(Duration::from_millis(0), 8);
        let id = immediate.open(new_txn(None), "default".into()).unwrap();
        let taken = immediate.take_expired();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].0, id);
        assert_eq!(taken[0].1, "default");
        assert_eq!(immediate.open_count(), 0, "an expired entry is removed");

        let patient = Transactions::new(Duration::from_secs(3_600), 8);
        patient.open(new_txn(None), "default".into()).unwrap();
        assert!(patient.take_expired().is_empty());
        assert_eq!(patient.open_count(), 1);
    }

    /// Using a transaction resets its idle clock, so one in the middle of a long
    /// sequence of commands is never reaped underneath its caller.
    #[test]
    fn using_a_transaction_keeps_it_alive() {
        let registry = Transactions::new(Duration::from_millis(50), 8);
        let id = registry.open(new_txn(None), "default".into()).unwrap();

        std::thread::sleep(Duration::from_millis(30));
        assert!(registry.get(id, "default").is_some(), "not idle yet");
        std::thread::sleep(Duration::from_millis(30));
        // Idle for 30ms since the touch, not 60ms since it was opened.
        assert!(
            registry.take_expired().is_empty(),
            "the lookup must have reset the clock"
        );
    }

    #[test]
    fn a_timeout_is_applied_before_the_transaction_is_shared() {
        let default = new_txn(None);
        let explicit = new_txn(Some(5_000));
        assert_eq!(explicit.timeout(), Duration::from_millis(5_000));
        // Zero is what the client means by "the server's default".
        assert_eq!(default.timeout(), Duration::from_millis(0));
    }

    #[test]
    fn every_state_and_status_maps_to_the_contracts_own() {
        assert_eq!(to_state(TxnState::Open), WireTxnState::Open);
        assert_eq!(to_state(TxnState::Verified), WireTxnState::Verified);
        assert_eq!(to_state(TxnState::Committed), WireTxnState::Committed);
        assert_eq!(to_state(TxnState::Aborted), WireTxnState::Aborted);

        assert_eq!(to_commit_status(&CommitStatus::Ok), WireCommitStatus::Ok);
        assert_eq!(
            to_commit_status(&CommitStatus::AlreadyCommitted),
            WireCommitStatus::AlreadyCommitted
        );
        assert_eq!(
            to_commit_status(&CommitStatus::RollForwardAbandoned),
            WireCommitStatus::RollForwardAbandoned
        );
        assert_eq!(
            to_commit_status(&CommitStatus::CloseAbandoned),
            WireCommitStatus::CloseAbandoned
        );

        assert_eq!(to_abort_status(&AbortStatus::Ok), WireAbortStatus::Ok);
        assert_eq!(
            to_abort_status(&AbortStatus::AlreadyAborted),
            WireAbortStatus::AlreadyAborted
        );
        assert_eq!(
            to_abort_status(&AbortStatus::RollBackAbandoned),
            WireAbortStatus::RollBackAbandoned
        );
        assert_eq!(
            to_abort_status(&AbortStatus::CloseAbandoned),
            WireAbortStatus::CloseAbandoned
        );
    }

    #[test]
    fn a_cluster_too_old_is_refused_by_name() {
        assert_eq!(MrtSupport::Supported.refusal(), None);

        let old = MrtSupport::Unsupported {
            node: "BB9".into(),
            version: "7.2.0.0".into(),
        }
        .refusal()
        .expect("an old cluster must be refused");
        assert!(old.contains("BB9"), "{old}");
        assert!(old.contains("8.0"), "{old}");
        // The other requirement is easy to miss and produces a confusing failure
        // much later, so the message says it here.
        assert!(old.contains("strong-consistency"), "{old}");

        let unknown = MrtSupport::Unknown.refusal().expect("must be refused");
        assert!(unknown.contains("reachable"), "{unknown}");
    }
}
