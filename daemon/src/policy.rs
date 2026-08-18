// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! [`WirePolicy`] → the client's [`ReadPolicy`] and [`WritePolicy`].
//!
//! Every wire field is optional, and every `None` means "leave the daemon's
//! configured default alone" — so a resolved policy is [`Defaults`] plus
//! whatever the request actually asked to change, never a fresh policy built
//! from nothing.
//!
//! # Nothing is applied silently, and nothing is dropped silently
//!
//! One flat [`WirePolicy`] covers reads and writes, because PHP passes an
//! associative array and has nowhere to put a type distinction. The daemon knows
//! which verb it is serving, so it applies the relevant subset — and **refuses**
//! the rest rather than ignoring it: a policy that quietly does nothing surfaces
//! much later as data that is subtly wrong, at which point nothing in the
//! request points at the cause. So a read carrying `expiration` is an
//! [`INVALID_REQUEST`](aerospike_php_ipc::StatusCode::INVALID_REQUEST) naming
//! the field, not a read with a forgotten TTL.
//!
//! Two fields are the exception, and only because the contract puts them in its
//! shared section while this client puts them on one side:
//!
//! - `replica` is applied to reads and accepted-but-inert on writes: a write
//!   goes to the partition master by construction (`Partition::for_write`
//!   hard-codes it), so there is no node choice to influence.
//! - `send_key` is applied to writes and accepted-but-inert on reads: the user
//!   key is *stored* by a write, and a read has nothing to store.
//!
//! Both are refusals the contract would not agree with — it lists them as
//! applying to any command — so they are documented no-ops instead.
//!
//! # AEL is gated on the cluster, not on hope
//!
//! `filter` may be a built expression tree, filter text, or a packed expression
//! from another client, and it goes through [`crate::ops::to_expression`] like
//! every other expression in this daemon — so the three forms are available
//! wherever a filter is, and the version gate below is written once.
//!
//! Only the **text** form has a version requirement: it is compiled by the
//! server, which only 8.1.3+ can do. Sending it to an older cluster gets a parameter error from
//! somewhere deep in the server with nothing to say about why, so [`AelSupport`]
//! asks the cluster first — all nodes, since the filter travels with the command
//! to whichever node owns the record — and an older cluster is refused with a
//! message naming the node and its version.

use std::fmt;
use std::time::Duration;

use aerospike_core::policy::Replica;
use aerospike_core::{
    BasePolicy, Client, CommitLevel, Expiration, GenerationPolicy, ReadModeAP, ReadModeSC,
    ReadPolicy, RecordExistsAction, WritePolicy,
};
use aerospike_php_ipc::policy::{
    WireCommitLevel, WireExpiration, WireGenerationPolicy, WireReadModeAp, WireReadModeSc,
    WireRecordExistsAction, WireReplica,
};
use aerospike_php_ipc::op::WireExpression;
use aerospike_php_ipc::WirePolicy;

/// A per-call policy the daemon will not apply, and why.
///
/// Always reported as
/// [`INVALID_REQUEST`](aerospike_php_ipc::StatusCode::INVALID_REQUEST): the
/// request asked for something this daemon cannot honour, which is the caller's
/// to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyError(String);

impl PolicyError {
    /// The refusal detail.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PolicyError {}

/// Whether a cluster can compile Aerospike Expression Language text itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AelSupport {
    /// Every node is new enough (8.1.3+).
    Supported,
    /// At least one node is not, named so the message can say which.
    Unsupported {
        /// The node that is too old.
        node: String,
        /// The version it reported.
        version: String,
    },
    /// There is no cluster view to ask — not connected, or still tending.
    Unknown,
}

impl AelSupport {
    /// Ask `client`'s current cluster view.
    ///
    /// All-nodes: the filter text travels with the command to whichever node
    /// owns the record, so one old node makes the answer no.
    #[must_use]
    pub fn of(client: &Client) -> AelSupport {
        let nodes = client.nodes();
        if nodes.is_empty() {
            return AelSupport::Unknown;
        }
        for node in nodes {
            // The client's own gate, not a version comparison re-implemented
            // here, so the two cannot drift apart.
            if !node.version().supports_server_compiled_ael() {
                let v = node.version();
                return AelSupport::Unsupported {
                    node: node.name().to_string(),
                    version: format!("{}.{}.{}.{}", v.major, v.minor, v.patch, v.build),
                };
            }
        }
        AelSupport::Supported
    }

    /// Refuse unless every node can compile filter text.
    ///
    /// `pub(crate)` because [`crate::ops::to_ael_expression`] is the single door
    /// every expression goes through, and this gate belongs on the door. It used
    /// to live only on the policy path, which meant a filter on a policy was
    /// refused while the cluster view was still empty but the same text on an
    /// expression *operation* was attempted anyway — an inconsistency nothing
    /// intended.
    pub(crate) fn require(&self) -> Result<(), PolicyError> {
        match self {
            AelSupport::Supported => Ok(()),
            AelSupport::Unsupported { node, version } => Err(PolicyError(format!(
                "policy.filter is filter text the server compiles, which needs Aerospike 8.1.3 \
                 or later, but node '{node}' runs {version}. Upgrade the cluster, drop the \
                 filter, or build it with Aerospike\\Exp instead — a built expression is \
                 packed by the client and needs nothing of the server"
            ))),
            AelSupport::Unknown => Err(PolicyError(
                "policy.filter is filter text, which needs a server of 8.1.3 or later to \
                 compile it, and this instance has no connected node to check; retry once the \
                 cluster is reachable, or build the expression with Aerospike\\Exp instead"
                    .to_string(),
            )),
        }
    }
}

/// What this cluster can do that an expression might need.
///
/// Three gates travel together because they are asked at the same moment and
/// answered from the same node list: filter *text* needs server 8.1.3 to compile,
/// the string expressions need 8.1.3 to run, and a CDT path expression's fan-out
/// needs 8.1.1. Bundling them is not tidiness — it means one walk of the cluster's
/// nodes instead of three, and it means a new capability is a field here rather
/// than another parameter threaded through every signature that leads to an
/// expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// Whether filter text can be compiled by the server.
    pub ael: AelSupport,
    /// Whether the string operations exist.
    pub strings: crate::exp::StringSupport,
    /// Whether the server can walk a CDT path expression's fan-out.
    pub paths: crate::exp::PathSupport,
}

impl Capabilities {
    /// Ask `client`'s current cluster view about both.
    ///
    /// One pass over the nodes: both gates are version checks on the same nodes,
    /// and the node list is a clone.
    #[must_use]
    pub fn of(client: &Client) -> Capabilities {
        let nodes = client.nodes();
        if nodes.is_empty() {
            return Capabilities {
                ael: AelSupport::Unknown,
                strings: crate::exp::StringSupport::Unknown,
                paths: crate::exp::PathSupport::Unknown,
            };
        }
        let mut caps = Capabilities::all();
        for node in nodes {
            let v = node.version();
            let name = || node.name().to_string();
            let version = || format!("{}.{}.{}.{}", v.major, v.minor, v.patch, v.build);
            // The client's own gates, so this cannot drift from them.
            if caps.ael == AelSupport::Supported && !v.supports_server_compiled_ael() {
                caps.ael = AelSupport::Unsupported {
                    node: name(),
                    version: version(),
                };
            }
            if caps.strings == crate::exp::StringSupport::Supported
                && !v.supports_string_operations()
            {
                caps.strings = crate::exp::StringSupport::Unsupported {
                    node: name(),
                    version: version(),
                };
            }
            if caps.paths == crate::exp::PathSupport::Supported
                && !v.supports_cdt_path_expressions()
            {
                caps.paths = crate::exp::PathSupport::Unsupported {
                    node: name(),
                    version: version(),
                };
            }
        }
        caps
    }

    /// Everything supported.
    ///
    /// What a request with nothing version-gated in it gets, and what the tests
    /// use: "not applicable" and "yes" lead to the same behaviour, and inventing a
    /// third state for the difference would be a state nothing reads.
    #[must_use]
    pub fn all() -> Capabilities {
        Capabilities {
            ael: AelSupport::Supported,
            strings: crate::exp::StringSupport::Supported,
            paths: crate::exp::PathSupport::Supported,
        }
    }
}

/// The policies a request starts from before its own overrides land.
///
/// Held by the dispatcher, one set for the whole daemon: this is where the
/// configured defaults enter the picture, so a request that overrides nothing
/// still gets them.
#[derive(Debug, Clone)]
pub struct Defaults {
    read: ReadPolicy,
    write: WritePolicy,
}

impl Defaults {
    /// Defaults whose total timeout is the daemon's configured one.
    ///
    /// A request may still declare its own `total_timeout_ms`; this is what it
    /// gets when it does not. Milliseconds are what the client's policies speak,
    /// and a duration too large for `u32` is clamped rather than wrapped —
    /// silently turning a long timeout into a tiny one would be the worst
    /// possible rounding.
    #[must_use]
    pub fn with_total_timeout(total: Duration) -> Defaults {
        let millis = u32::try_from(total.as_millis()).unwrap_or(u32::MAX);
        let mut read = ReadPolicy::default();
        read.base_policy.total_timeout = millis;
        let mut write = WritePolicy::default();
        write.base_policy.total_timeout = millis;
        Defaults { read, write }
    }

    /// The read policy a request with no overrides gets.
    #[must_use]
    pub const fn read_default(&self) -> &ReadPolicy {
        &self.read
    }

    /// The write policy a request with no overrides gets.
    #[must_use]
    pub const fn write_default(&self) -> &WritePolicy {
        &self.write
    }

    /// Resolve a read policy, checking AEL support against `client` only when
    /// the request actually carries filter text.
    ///
    /// # Errors
    /// [`PolicyError`] for a write-only field on a read, unparsable filter text,
    /// or filter text a cluster this old cannot compile.
    pub fn for_read(
        &self,
        client: &Client,
        wire: &WirePolicy,
    ) -> Result<ReadPolicy, PolicyError> {
        self.read(wire, &capabilities(client, wire))
    }

    /// Resolve a write policy, checking AEL support against `client` only when
    /// the request actually carries filter text.
    ///
    /// # Errors
    /// [`PolicyError`] for unparsable filter text, or filter text a cluster this
    /// old cannot compile.
    pub fn for_write(
        &self,
        client: &Client,
        wire: &WirePolicy,
    ) -> Result<WritePolicy, PolicyError> {
        self.write(wire, &capabilities(client, wire))
    }

    /// Resolve a read policy against an already-known [`AelSupport`].
    ///
    /// # Errors
    /// [`PolicyError`] as for [`for_read`](Self::for_read).
    pub fn read(&self, wire: &WirePolicy, caps: &Capabilities) -> Result<ReadPolicy, PolicyError> {
        reject_write_only(wire)?;

        let mut policy = self.read.clone();
        apply_shared(&mut policy.base_policy, wire, caps)?;
        if let Some(replica) = wire.replica {
            policy.replica = replica_of(replica);
        }
        // `send_key` is deliberately not applied: a read stores nothing.
        Ok(policy)
    }

    /// A batch policy for this request.
    ///
    /// The shared fields resolve exactly as a read's do — timeouts, retries,
    /// replica, a filter that applies to every row — because that is what the
    /// parent policy of a batch *is*. Per-row settings live on the rows, so this
    /// deliberately consults none of the write-only fields.
    ///
    /// # Errors
    /// [`PolicyError`] naming the field, as [`Defaults::read`] does.
    pub fn batch(
        &self,
        wire: &WirePolicy,
        caps: &Capabilities,
    ) -> Result<aerospike_core::BatchPolicy, PolicyError> {
        let mut policy = aerospike_core::BatchPolicy::default();
        policy.base_policy = self.read(wire, caps)?.base_policy;
        if let Some(replica) = wire.replica {
            policy.replica = replica_of(replica);
        }
        Ok(policy)
    }

    /// A query policy for this request.
    ///
    /// A scan or query is a read of many records, so the shared fields resolve
    /// the same way and the write-only ones are refused the same way. What is
    /// specific to a traversal — the page size, the record ceiling, the rate
    /// limit, whether bins come back — is part of the request rather than a
    /// policy override, so [`crate::query`] sets those on the returned policy
    /// and this does not consult them.
    ///
    /// # This one does *not* start from the daemon's default timeout
    ///
    /// Unlike [`read`](Self::read) and [`write`](Self::write), this starts from
    /// the client's own `QueryPolicy::default()`, whose total timeout is zero —
    /// unlimited. That is deliberate: the daemon's `default_timeout` is sized for
    /// a single-record command (a second), and a scan of a large set legitimately
    /// takes longer than any such deadline. Inheriting it would make every real
    /// scan fail at exactly the same place. A caller that wants a bound on a page
    /// sets `total_timeout_ms` on the policy, and the worker's own reply deadline
    /// bounds how long it waits regardless.
    ///
    /// # Errors
    /// [`PolicyError`] naming the field, as [`Defaults::read`] does.
    pub fn query(
        &self,
        wire: &WirePolicy,
        caps: &Capabilities,
    ) -> Result<aerospike_core::QueryPolicy, PolicyError> {
        reject_write_only(wire)?;
        // A multi-record transaction covers records named by key, and a scan or
        // query names none — so there is nothing coherent for a transaction to
        // mean here. Refused rather than ignored, because a caller who believed
        // their query was inside a transaction would be wrong about their data.
        if wire.txn.is_some() {
            return Err(PolicyError(
                "a scan or query cannot run inside a multi-record transaction: a transaction \
                 covers records named by key, and a traversal names none. Read the keys with a \
                 query first, then read or write them by key inside the transaction"
                    .to_string(),
            ));
        }

        let mut policy = aerospike_core::QueryPolicy::default();
        apply_shared(&mut policy.base_policy, wire, caps)?;
        if let Some(replica) = wire.replica {
            policy.replica = replica_of(replica);
        }
        Ok(policy)
    }

    /// A verify policy for the read-checking half of a commit.
    ///
    /// # This one starts from the client's own defaults too
    ///
    /// Like [`query`](Self::query) and for the same reason, this does *not*
    /// inherit the daemon's `default_timeout`: `TxnVerifyPolicy::default()`
    /// carries deliberately tuned values — linearized SC reads, the master
    /// replica, 5 retries, a 3s socket and 10s total timeout, a 1s sleep between
    /// retries — chosen because a verify batch that gives up leaves a transaction
    /// half-finished. Overwriting them with a timeout sized for a single-record
    /// command would make a commit fail exactly when the cluster is busiest.
    ///
    /// # Errors
    /// [`PolicyError`] naming the field: this is a batch of reads, so the
    /// write-only fields are refused as they are everywhere else.
    pub fn txn_verify(
        &self,
        wire: &WirePolicy,
        caps: &Capabilities,
    ) -> Result<aerospike_core::TxnVerifyPolicy, PolicyError> {
        reject_write_only(wire)?;
        reject_joining_a_transaction(wire, "verify")?;
        let mut policy = aerospike_core::TxnVerifyPolicy::default();
        apply_shared(&mut policy.batch_policy.base_policy, wire, caps)?;
        if let Some(replica) = wire.replica {
            policy.batch_policy.replica = replica_of(replica);
        }
        Ok(policy)
    }

    /// A roll policy for the write-moving half of a commit or an abort.
    ///
    /// Starts from `TxnRollPolicy::default()` for the same reason
    /// [`txn_verify`](Self::txn_verify) does.
    ///
    /// # Why the write-only fields are refused here as well
    ///
    /// Rolling a transaction forward or back *does* write, so refusing
    /// `expiration` or `record_exists_action` may look inconsistent. It is not:
    /// the roll batch does not write records the caller described, it moves
    /// writes the transaction already made. A TTL or a create/replace guard has
    /// nothing to apply to, so naming one is a misunderstanding worth reporting.
    ///
    /// # Errors
    /// [`PolicyError`] naming the field.
    pub fn txn_roll(
        &self,
        wire: &WirePolicy,
        caps: &Capabilities,
    ) -> Result<aerospike_core::TxnRollPolicy, PolicyError> {
        reject_write_only(wire)?;
        reject_joining_a_transaction(wire, "roll")?;
        let mut policy = aerospike_core::TxnRollPolicy::default();
        apply_shared(&mut policy.batch_policy.base_policy, wire, caps)?;
        if let Some(replica) = wire.replica {
            policy.batch_policy.replica = replica_of(replica);
        }
        Ok(policy)
    }

    /// Resolve a write policy against an already-known [`AelSupport`].
    ///
    /// # Errors
    /// [`PolicyError`] as for [`for_write`](Self::for_write).
    pub fn write(&self, wire: &WirePolicy, caps: &Capabilities) -> Result<WritePolicy, PolicyError> {
        let mut policy = self.write.clone();
        apply_shared(&mut policy.base_policy, wire, caps)?;

        if let Some(send_key) = wire.send_key {
            policy.send_key = send_key;
        }
        if let Some(action) = wire.record_exists_action {
            policy.record_exists_action = record_exists_action(action);
        }
        if let Some(guard) = wire.generation_policy {
            policy.generation_policy = generation_policy(guard);
        }
        if let Some(generation) = wire.generation {
            policy.generation = generation;
        }
        if let Some(expiration) = wire.expiration {
            policy.expiration = expiration_of(expiration);
        }
        if let Some(level) = wire.commit_level {
            policy.commit_level = commit_level(level);
        }
        if let Some(durable_delete) = wire.durable_delete {
            policy.durable_delete = durable_delete;
        }
        if let Some(respond_per_each_op) = wire.respond_per_each_op {
            policy.respond_per_each_op = respond_per_each_op;
        }
        // `replica` is deliberately not applied: a write goes to the master.
        Ok(policy)
    }
}

/// Refuse a write-only field on a command that does not write.
///
/// Its own function because three resolvers need it — a read, a batch and a
/// query are all reads — and the message has to be the same one every time: the
/// point is that the field was *named*, not that some particular verb saw it.
fn reject_write_only(wire: &WirePolicy) -> Result<(), PolicyError> {
    let write_only = wire.write_only_fields_set();
    if write_only.is_empty() {
        return Ok(());
    }
    Err(PolicyError(format!(
        "policy field{} {} appl{} only to writes, and this is a read command; \
         the daemon refuses rather than ignores {}, because an ignored policy \
         shows up later as data that is subtly wrong",
        if write_only.len() == 1 { "" } else { "s" },
        write_only
            .iter()
            .map(|f| format!("'{f}'"))
            .collect::<Vec<_>>()
            .join(", "),
        if write_only.len() == 1 { "ies" } else { "y" },
        if write_only.len() == 1 { "it" } else { "them" },
    )))
}

/// Refuse `policy.txn` on the two policies that *finish* a transaction.
///
/// Every other command joins a transaction through this field, so a caller who
/// has learnt that rule will reasonably try it here too. But a commit already
/// names its transaction — by id, in the request body — and there is no second
/// transaction for its verify or roll batch to be part of. Setting the field
/// would either be redundant or contradict the id, and neither is something to
/// guess at, so it is named and refused.
fn reject_joining_a_transaction(wire: &WirePolicy, phase: &str) -> Result<(), PolicyError> {
    if wire.txn.is_none() {
        return Ok(());
    }
    Err(PolicyError(format!(
        "policy field 'txn' does not apply to a transaction's {phase} policy: the transaction \
         being finished is named by the request itself, and its {phase} batch is not inside \
         another transaction"
    )))
}

/// Fields that mean the same thing whichever verb is being served.
fn apply_shared(
    base: &mut BasePolicy,
    wire: &WirePolicy,
    caps: &Capabilities,
) -> Result<(), PolicyError> {
    if let Some(total) = wire.total_timeout_ms {
        base.total_timeout = total;
    }
    if let Some(socket) = wire.socket_timeout_ms {
        base.socket_timeout = socket;
    }
    if let Some(retries) = wire.max_retries {
        base.max_retries = retries as usize;
    }
    if let Some(sleep) = wire.sleep_between_retries_ms {
        base.sleep_between_retries = sleep;
    }
    if let Some(mode) = wire.read_mode_ap {
        base.read_mode_ap = match mode {
            WireReadModeAp::One => ReadModeAP::One,
            WireReadModeAp::All => ReadModeAP::All,
        };
    }
    if let Some(mode) = wire.read_mode_sc {
        base.read_mode_sc = match mode {
            WireReadModeSc::Session => ReadModeSC::Session,
            WireReadModeSc::Linearize => ReadModeSC::Linearize,
            WireReadModeSc::AllowReplica => ReadModeSC::AllowReplica,
            WireReadModeSc::AllowUnavailable => ReadModeSC::AllowUnavailable,
        };
    }
    if let Some(compress) = wire.use_compression {
        base.use_compression = compress;
    }
    if let Some(filter) = &wire.filter {
        // Through the one door in `crate::ops`, so a policy filter accepts every
        // form an expression operation does — and the AEL version gate lives in
        // one place rather than being repeated per use.
        base.filter_expression = Some(crate::ops::to_expression(filter, caps).map_err(|e| {
            PolicyError(format!("policy.filter is not usable: {e}"))
        })?);
    }
    Ok(())
}

/// What the cluster has to be asked about for this request.
///
/// Asking costs a clone of the cluster's node list, so it is skipped unless the
/// request carries something version-gated. A filter written as **text** needs the
/// AEL gate; a filter built as a **tree** may contain string expressions, which
/// need theirs. A packed expression needs nothing of the server, and neither does
/// no filter at all — for those, [`Capabilities::all`] stands in for "not
/// applicable".
///
/// A tree is not inspected to find out whether it actually contains a string
/// expression: that would be a walk of the tree to avoid a walk of the node list,
/// and the node list is the shorter of the two.
fn capabilities(client: &Client, wire: &WirePolicy) -> Capabilities {
    match &wire.filter {
        Some(WireExpression::Ael(_) | WireExpression::Tree(_)) => Capabilities::of(client),
        Some(WireExpression::Base64(_)) | None => Capabilities::all(),
    }
}

const fn replica_of(replica: WireReplica) -> Replica {
    match replica {
        WireReplica::Master => Replica::Master,
        WireReplica::MasterProles => Replica::MasterProles,
        WireReplica::Random => Replica::Random,
        WireReplica::Sequence => Replica::Sequence,
        WireReplica::PreferRack => Replica::PreferRack,
    }
}

/// The client's create/update/replace setting.
///
/// `pub(crate)` because the per-row batch policies need exactly the same
/// mapping, and two copies of it could disagree.
pub(crate) const fn record_exists_action(
    action: WireRecordExistsAction,
) -> RecordExistsAction {
    match action {
        WireRecordExistsAction::Update => RecordExistsAction::Update,
        WireRecordExistsAction::UpdateOnly => RecordExistsAction::UpdateOnly,
        WireRecordExistsAction::Replace => RecordExistsAction::Replace,
        WireRecordExistsAction::ReplaceOnly => RecordExistsAction::ReplaceOnly,
        WireRecordExistsAction::CreateOnly => RecordExistsAction::CreateOnly,
    }
}

/// The client's generation guard.
pub(crate) const fn generation_policy(guard: WireGenerationPolicy) -> GenerationPolicy {
    match guard {
        WireGenerationPolicy::None => GenerationPolicy::None,
        WireGenerationPolicy::ExpectGenEqual => GenerationPolicy::ExpectGenEqual,
        WireGenerationPolicy::ExpectGenGreater => GenerationPolicy::ExpectGenGreater,
    }
}

/// The client's commit requirement.
pub(crate) const fn commit_level(level: WireCommitLevel) -> CommitLevel {
    match level {
        WireCommitLevel::CommitAll => CommitLevel::CommitAll,
        WireCommitLevel::CommitMaster => CommitLevel::CommitMaster,
    }
}

/// The client's time-to-live, for the per-row batch policies.
pub(crate) const fn expiration(expiration: WireExpiration) -> Expiration {
    expiration_of(expiration)
}

const fn expiration_of(expiration: WireExpiration) -> Expiration {
    match expiration {
        WireExpiration::NamespaceDefault => Expiration::NamespaceDefault,
        WireExpiration::Never => Expiration::Never,
        WireExpiration::DontUpdate => Expiration::DontUpdate,
        WireExpiration::Seconds(seconds) => Expiration::Seconds(seconds),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_core::{Policy, Version};

    fn defaults() -> Defaults {
        Defaults::with_total_timeout(Duration::from_secs(2))
    }

    #[test]
    fn the_configured_timeout_is_where_every_policy_starts() {
        let defaults = defaults();
        assert_eq!(defaults.read_default().base_policy.total_timeout, 2_000);
        assert_eq!(defaults.write_default().base_policy.total_timeout, 2_000);

        // Whatever the client's own default is, the daemon's configuration wins.
        let huge = Defaults::with_total_timeout(Duration::from_secs(u64::from(u32::MAX)));
        assert_eq!(
            huge.read_default().base_policy.total_timeout,
            u32::MAX,
            "an absurd configured timeout must clamp, never wrap to a tiny one"
        );
    }

    /// A transaction covers records named by key; a traversal names none. Ignoring
    /// it would leave a caller believing their query was transactional.
    #[test]
    fn a_query_refuses_to_run_inside_a_transaction() {
        let defaults = Defaults::with_total_timeout(Duration::from_secs(1));
        let in_txn = WirePolicy {
            txn: Some(7),
            ..WirePolicy::default()
        };
        let error = defaults
            .query(&in_txn, &Capabilities::all())
            .expect_err("a query in a transaction must be refused");
        assert!(error.message().contains("named by key"), "{error}");

        // And the same policy is perfectly fine for a read or a write, which is
        // the point of refusing it only here.
        assert!(defaults.read(&in_txn, &Capabilities::all()).is_ok());
        assert!(defaults.write(&in_txn, &Capabilities::all()).is_ok());
        assert!(defaults.query(&WirePolicy::default(), &Capabilities::all()).is_ok());
    }

    /// A commit's two policies start from the client's tuned defaults, not the
    /// daemon's single-record timeout — a verify that gives up early leaves a
    /// transaction half-finished, holding locks.
    #[test]
    fn a_transactions_policies_keep_the_clients_own_tuning() {
        // A deliberately tiny configured timeout: if it leaked in, these would be
        // 5ms rather than the client's 10s.
        let defaults = Defaults::with_total_timeout(Duration::from_millis(5));
        let none = WirePolicy::default();

        let verify = defaults.txn_verify(&none, &Capabilities::all()).unwrap();
        assert_eq!(verify.batch_policy.base_policy.total_timeout, 10_000);
        assert_eq!(verify.batch_policy.base_policy.socket_timeout, 3_000);
        assert_eq!(verify.batch_policy.base_policy.max_retries, 5);
        assert_eq!(
            verify.batch_policy.base_policy.read_mode_sc,
            ReadModeSC::Linearize,
            "a verify batch reads linearizably, which is the point of verifying"
        );
        assert_eq!(verify.batch_policy.replica, Replica::Master);

        let roll = defaults.txn_roll(&none, &Capabilities::all()).unwrap();
        assert_eq!(roll.batch_policy.base_policy.total_timeout, 10_000);
        assert_eq!(roll.batch_policy.base_policy.max_retries, 5);
        assert_eq!(roll.batch_policy.replica, Replica::Master);

        // Overrides still land on top, which is what makes the pair worth carrying.
        let longer = WirePolicy {
            total_timeout_ms: Some(30_000),
            max_retries: Some(8),
            replica: Some(WireReplica::Sequence),
            ..WirePolicy::default()
        };
        let verify = defaults.txn_verify(&longer, &Capabilities::all()).unwrap();
        assert_eq!(verify.batch_policy.base_policy.total_timeout, 30_000);
        assert_eq!(verify.batch_policy.base_policy.max_retries, 8);
        assert_eq!(verify.batch_policy.replica, Replica::Sequence);
        // Untouched fields keep the client's tuning rather than reverting.
        assert_eq!(verify.batch_policy.base_policy.socket_timeout, 3_000);

        let roll = defaults.txn_roll(&longer, &Capabilities::all()).unwrap();
        assert_eq!(roll.batch_policy.base_policy.total_timeout, 30_000);
        assert_eq!(roll.batch_policy.replica, Replica::Sequence);
    }

    /// Both refuse the fields that would mean nothing here, rather than ignoring
    /// them: a write field has no record of the caller's to apply to, and the
    /// transaction being finished is already named by the request.
    #[test]
    fn a_transactions_policies_refuse_what_they_cannot_honour() {
        let defaults = defaults();

        let in_txn = WirePolicy {
            txn: Some(7),
            ..WirePolicy::default()
        };
        let refusals = [
            (
                "verify",
                defaults
                    .txn_verify(&in_txn, &Capabilities::all())
                    .map(|_| ()),
            ),
            (
                "roll",
                defaults.txn_roll(&in_txn, &Capabilities::all()).map(|_| ()),
            ),
        ];
        for (phase, result) in refusals {
            let error =
                result.expect_err("'txn' must be refused on a transaction's own policies");
            assert!(error.message().contains("named by the request"), "{error}");
            assert!(error.message().contains(phase), "{error}");
        }

        // A roll *writes*, and still refuses the write fields — it moves writes the
        // transaction already made, so a TTL has nothing to apply to.
        let with_ttl = WirePolicy {
            expiration: Some(WireExpiration::Seconds(3_600)),
            ..WirePolicy::default()
        };
        let error = defaults
            .txn_roll(&with_ttl, &Capabilities::all())
            .expect_err("a roll policy has no record of the caller's to give a TTL to");
        assert!(error.message().contains("'expiration'"), "{error}");
        assert!(defaults
            .txn_verify(&with_ttl, &Capabilities::all())
            .is_err());
    }

    #[test]
    fn an_empty_policy_changes_nothing() {
        let defaults = defaults();
        let empty = WirePolicy::default();

        let read = defaults.read(&empty, &Capabilities::all()).unwrap();
        let base = defaults.read_default();
        assert_eq!(read.base_policy.total_timeout, base.base_policy.total_timeout);
        assert_eq!(
            read.base_policy.socket_timeout,
            base.base_policy.socket_timeout
        );
        assert_eq!(read.base_policy.max_retries, base.base_policy.max_retries);
        assert_eq!(read.replica, base.replica);
        assert!(read.base_policy.filter_expression.is_none());

        let write = defaults.write(&empty, &Capabilities::all()).unwrap();
        let base = defaults.write_default();
        assert_eq!(write.record_exists_action, base.record_exists_action);
        assert_eq!(write.generation_policy, base.generation_policy);
        assert_eq!(write.generation, base.generation);
        assert_eq!(write.expiration, base.expiration);
        assert_eq!(write.commit_level, base.commit_level);
        assert_eq!(write.send_key, base.send_key);
        assert_eq!(write.durable_delete, base.durable_delete);
        assert_eq!(write.respond_per_each_op, base.respond_per_each_op);
        assert_eq!(
            write.base_policy.max_retries, base.base_policy.max_retries,
            "a write must not inherit a read's retry count: it may not be idempotent"
        );
        assert!(write.base_policy.filter_expression.is_none());
    }

    #[test]
    fn the_shared_fields_land_on_both_kinds_of_command() {
        let wire = WirePolicy {
            total_timeout_ms: Some(1_500),
            socket_timeout_ms: Some(250),
            max_retries: Some(4),
            sleep_between_retries_ms: Some(30),
            read_mode_ap: Some(WireReadModeAp::All),
            read_mode_sc: Some(WireReadModeSc::Linearize),
            use_compression: Some(true),
            ..WirePolicy::default()
        };

        let read = defaults().read(&wire, &Capabilities::all()).unwrap();
        assert_eq!(read.total_timeout(), 1_500);
        assert_eq!(read.socket_timeout(), 250);
        assert_eq!(read.max_retries(), 4);
        assert_eq!(read.sleep_between_retries(), Some(Duration::from_millis(30)));
        assert_eq!(read.read_mode_ap(), ReadModeAP::All);
        assert_eq!(read.read_mode_sc(), ReadModeSC::Linearize);
        assert!(read.use_compression());

        let write = defaults().write(&wire, &Capabilities::all()).unwrap();
        assert_eq!(write.total_timeout(), 1_500);
        assert_eq!(write.socket_timeout(), 250);
        assert_eq!(write.max_retries(), 4);
        assert_eq!(
            write.sleep_between_retries(),
            Some(Duration::from_millis(30))
        );
        assert_eq!(write.read_mode_ap(), ReadModeAP::All);
        assert_eq!(write.read_mode_sc(), ReadModeSC::Linearize);
        assert!(write.use_compression());
    }

    #[test]
    fn every_write_only_field_maps_to_its_client_counterpart() {
        let wire = WirePolicy {
            record_exists_action: Some(WireRecordExistsAction::CreateOnly),
            generation_policy: Some(WireGenerationPolicy::ExpectGenEqual),
            generation: Some(7),
            expiration: Some(WireExpiration::Seconds(600)),
            commit_level: Some(WireCommitLevel::CommitMaster),
            durable_delete: Some(true),
            respond_per_each_op: Some(true),
            send_key: Some(true),
            ..WirePolicy::default()
        };
        let write = defaults().write(&wire, &Capabilities::all()).unwrap();
        assert_eq!(write.record_exists_action, RecordExistsAction::CreateOnly);
        assert_eq!(write.generation_policy, GenerationPolicy::ExpectGenEqual);
        assert_eq!(write.generation, 7);
        assert_eq!(write.expiration, Expiration::Seconds(600));
        assert_eq!(write.commit_level, CommitLevel::CommitMaster);
        assert!(write.durable_delete);
        assert!(write.respond_per_each_op);
        assert!(write.send_key);
    }

    #[test]
    fn every_enum_variant_has_a_mapping() {
        for (wire, expected) in [
            (WireRecordExistsAction::Update, RecordExistsAction::Update),
            (
                WireRecordExistsAction::UpdateOnly,
                RecordExistsAction::UpdateOnly,
            ),
            (WireRecordExistsAction::Replace, RecordExistsAction::Replace),
            (
                WireRecordExistsAction::ReplaceOnly,
                RecordExistsAction::ReplaceOnly,
            ),
            (
                WireRecordExistsAction::CreateOnly,
                RecordExistsAction::CreateOnly,
            ),
        ] {
            let policy = WirePolicy {
                record_exists_action: Some(wire),
                ..WirePolicy::default()
            };
            let resolved = defaults().write(&policy, &Capabilities::all()).unwrap();
            assert_eq!(resolved.record_exists_action, expected, "{wire:?}");
        }

        for (wire, expected) in [
            (WireGenerationPolicy::None, GenerationPolicy::None),
            (
                WireGenerationPolicy::ExpectGenEqual,
                GenerationPolicy::ExpectGenEqual,
            ),
            (
                WireGenerationPolicy::ExpectGenGreater,
                GenerationPolicy::ExpectGenGreater,
            ),
        ] {
            let policy = WirePolicy {
                generation_policy: Some(wire),
                ..WirePolicy::default()
            };
            let resolved = defaults().write(&policy, &Capabilities::all()).unwrap();
            assert_eq!(resolved.generation_policy, expected, "{wire:?}");
        }

        for (wire, expected) in [
            (WireReplica::Master, Replica::Master),
            (WireReplica::MasterProles, Replica::MasterProles),
            (WireReplica::Random, Replica::Random),
            (WireReplica::Sequence, Replica::Sequence),
            (WireReplica::PreferRack, Replica::PreferRack),
        ] {
            let policy = WirePolicy {
                replica: Some(wire),
                ..WirePolicy::default()
            };
            let resolved = defaults().read(&policy, &Capabilities::all()).unwrap();
            assert_eq!(resolved.replica, expected, "{wire:?}");
        }
    }

    #[test]
    fn the_ttl_sentinels_map_to_the_clients_own() {
        for (wire, expected) in [
            (WireExpiration::NamespaceDefault, Expiration::NamespaceDefault),
            (WireExpiration::Never, Expiration::Never),
            (WireExpiration::DontUpdate, Expiration::DontUpdate),
            (WireExpiration::Seconds(0), Expiration::Seconds(0)),
            (WireExpiration::Seconds(u32::MAX), Expiration::Seconds(u32::MAX)),
        ] {
            let policy = WirePolicy {
                expiration: Some(wire),
                ..WirePolicy::default()
            };
            let resolved = defaults().write(&policy, &Capabilities::all()).unwrap();
            assert_eq!(resolved.expiration, expected, "{wire:?}");
            // And the wire form the server actually sees, so that "never" can
            // never be confused with "namespace default".
            assert_eq!(u32::from(resolved.expiration), u32::from(expected));
        }
    }

    #[test]
    fn a_read_refuses_write_only_fields_by_name() {
        let wire = WirePolicy {
            expiration: Some(WireExpiration::Never),
            durable_delete: Some(true),
            total_timeout_ms: Some(100),
            ..WirePolicy::default()
        };
        let err = defaults().read(&wire, &Capabilities::all()).unwrap_err();
        assert!(err.message().contains("'expiration'"), "{err}");
        assert!(err.message().contains("'durable_delete'"), "{err}");
        // The shared field it also set is not part of the complaint.
        assert!(!err.message().contains("total_timeout"), "{err}");
        // ...and the same policy is fine on a write.
        assert!(defaults().write(&wire, &Capabilities::all()).is_ok());

        // Singular reads as singular: one field must not be "fields ... apply".
        let one = WirePolicy {
            generation: Some(1),
            ..WirePolicy::default()
        };
        let err = defaults().read(&one, &Capabilities::all()).unwrap_err();
        assert!(err.message().contains("policy field 'generation' applies"), "{err}");
    }

    #[test]
    fn a_read_only_field_on_a_write_is_accepted_and_inert() {
        // The contract lists `replica` as applying to any command, and a write
        // goes to the master regardless, so this must not be an error.
        let wire = WirePolicy {
            replica: Some(WireReplica::Random),
            ..WirePolicy::default()
        };
        assert!(defaults().write(&wire, &Capabilities::all()).is_ok());

        // The mirror image: `send_key` is stored by a write, so a read ignores
        // it rather than refusing.
        let wire = WirePolicy {
            send_key: Some(true),
            ..WirePolicy::default()
        };
        assert!(defaults().read(&wire, &Capabilities::all()).is_ok());
    }

    #[test]
    fn filter_text_is_compiled_for_both_reads_and_writes() {
        let wire = WirePolicy {
            filter: Some(WireExpression::Ael("$.age > 21".into())),
            ..WirePolicy::default()
        };
        let read = defaults().read(&wire, &Capabilities::all()).unwrap();
        assert!(
            read.base_policy.filter_expression.is_some(),
            "the filter must reach the policy, or the server sees no filter at all"
        );
        let write = defaults().write(&wire, &Capabilities::all()).unwrap();
        assert!(write.base_policy.filter_expression.is_some());
    }

    #[test]
    fn filter_text_is_refused_when_the_cluster_cannot_compile_it() {
        let wire = WirePolicy {
            filter: Some(WireExpression::Ael("$.age > 21".into())),
            ..WirePolicy::default()
        };

        let old = Capabilities {
            ael: AelSupport::Unsupported {
                node: "BB9040011AC4202".into(),
                version: "8.1.2.0".into(),
            },
            ..Capabilities::all()
        };
        for err in [
            defaults().read(&wire, &old).unwrap_err(),
            defaults().write(&wire, &old).unwrap_err(),
        ] {
            assert!(err.message().contains("8.1.3"), "{err}");
            assert!(err.message().contains("BB9040011AC4202"), "{err}");
            assert!(err.message().contains("8.1.2.0"), "{err}");
        }

        // No cluster view is not proof of support either.
        let err = defaults().read(&wire, &Capabilities { ael: AelSupport::Unknown, ..Capabilities::all() })
            .unwrap_err();
        assert!(err.message().contains("no connected node"), "{err}");

        // Without filter text, an unsupported cluster is irrelevant.
        assert!(defaults().read(&WirePolicy::default(), &old).is_ok());
        assert!(defaults().write(&WirePolicy::default(), &old).is_ok());
    }

    #[test]
    fn the_version_gate_is_the_clients_own() {
        // Not a re-implementation of "8.1.3": the same predicate the rest of the
        // client uses, so the two cannot drift apart.
        assert!(Version::new(8, 1, 3, 0).supports_server_compiled_ael());
        assert!(!Version::new(8, 1, 2, 0).supports_server_compiled_ael());
    }
}
