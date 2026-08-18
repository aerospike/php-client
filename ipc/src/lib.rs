// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The IPC contract between the Aerospike PHP extension and the local
//! daemon that owns the real client.
//!
//! Both ends of this channel are Rust, so the contract is expressed as
//! ordinary types rather than an IDL: the extension and the daemon both
//! depend on this crate and the compiler enforces that they agree. This
//! crate depends on neither `aerospike-core` nor `ext-php-rs`, so the
//! whole transport can be exercised from a plain Rust test with no PHP
//! and no database.
//!
//! # Transport
//!
//! Every request and reply — payload *and* metadata — crosses through
//! iceoryx2 shared memory. What varies is only how a waiting side learns
//! that something arrived, and that is a compile-time choice:
//!
//! - **Default:** the worker blocks on an iceoryx2 event port, so waiting
//!   costs essentially no CPU. Under `ipc::Service` those events are a
//!   unix datagram socket, so the wakeup path — not the payload path —
//!   leaves shared memory.
//! - **With the `shm-poll` feature:** the worker polls its response port
//!   with the tiered [`Backoff`] below, and nothing but shared memory is
//!   involved. Measured, that is worth ~10µs of latency for ~17× the CPU
//!   in the waiting process, so it is the opt-in.
//!
//! [`wakeup`] documents the trade in full, including why the
//! shared-memory event backend iceoryx2 ships is not a portable option.
//!
//! # Versioning: lock-step, not compatible
//!
//! The extension and the daemon must be **exactly the same version**. There is
//! no negotiation, no compatibility window, and no separately bumped protocol
//! counter: [`VERSION`] is this crate's package version, both ends compile it
//! in from here, and three independent mechanisms keep mismatched peers apart.
//!
//! | Mechanism | Catches |
//! | --- | --- |
//! | [`rpc_service_name`] embeds [`VERSION`] | a mismatched pair *meeting at all* — the extension cannot open a service another version created |
//! | [`VERSION_FINGERPRINT`] in every header | a stale mapping, or a daemon restarted from a different build under a name that already resolved |
//! | [`PongBody::daemon_version`] | reporting the *readable* versions once something is already wrong |
//!
//! This is what makes the rest of the contract cheap to change. Anything here
//! may be renamed, reordered or removed in a release, because no build ever
//! talks to a different one — so nothing below has to carry a compatibility
//! shim, and no field has to be kept alive for a peer that might still send
//! it. The cost is an operational rule, stated once: **upgrade both halves
//! together and restart the daemon.**
//!
//! # Typed frames
//!
//! The fixed part of every frame is a `#[repr(C)]`, [`ZeroCopySend`] type
//! carried as an iceoryx2 *user header* — so opcode, sequence, result
//! code, generation and TTL are typed fields written straight into shared
//! memory with no serialization step at all.
//!
//! Only the genuinely variable tail — namespaces, keys, bin names and
//! recursively nested bin values — travels in the `[u8]` payload, encoded
//! with [postcard]. Aerospike values nest arbitrarily, so they cannot be
//! a fixed-size POD; splitting the frame this way keeps everything that
//! *can* be typed typed.
//!
//! ```text
//!   user header (typed, zero-copy)        payload slice (postcard)
//! ┌────────────────────────────────┐   ┌──────────────────────────────┐
//! │ RequestHeader { opcode, seq,   │   │ PutBody { instance, ns, set, │
//! │   client_id, timeout_ms, .. }  │   │   key, bins }                │
//! └────────────────────────────────┘   └──────────────────────────────┘
//! ```
//!
//! [postcard]: https://docs.rs/postcard
//! [`ZeroCopySend`]: iceoryx2::prelude::ZeroCopySend

#![warn(missing_docs)]

pub mod admin;
pub mod batch;
pub mod exp;
pub mod op;
pub mod policy;
pub mod query;
pub mod security;
pub mod txn;
pub mod wakeup;

pub use admin::{
    WireIndexCreateBody, WireIndexDropBody, WireIndexOn, WireIndexType, WireInfoBody,
    WireInfoReply, WireNode, WireNodes, WireNodesBody, WireQueryUdfBody, WireTaskHandle,
    WireTaskStatus, WireTaskStatusBody, WireTruncateBody, WireUdfExecuteBody, WireUdfLang,
    WireUdfList, WireUdfListBody, WireUdfModule, WireUdfRegisterBody, WireUdfRemoveBody,
    WireUdfResult,
};
pub use exp::{
    MAX_EXP_DEPTH, MAX_EXP_NODES, WireExp, WireExpBinaryOp, WireExpBit, WireExpBitOp, WireExpHll,
    WireExpHllOp, WireExpList, WireExpListOp, WireExpMap, WireExpMapOp, WireExpMeta, WireExpStr,
    WireExpPath, WireExpPathOp, WireExpStrOp, WireExpType, WireExpUnaryOp, WireExpVariadicOp,
    WireLoopVarPart, WireStringNumericType, WireStringPolicy,
};
pub use batch::{
    WireBatchBody, WireBatchDeletePolicy, WireBatchKey, WireBatchReadPolicy, WireBatchReply,
    WireBatchResult, WireBatchRow, WireBatchUdfPolicy, WireBatchWritePolicy,
};
pub use query::{
    WireCollectionIndex, WireCursorBody, WireFilter, WireFilterBound, WireFilterKind,
    WireFilterTarget, WirePartitions, WireQueryBody, WireQueryPage, WireQueryRecord, WireRecordKey,
    WireStatement,
};
pub use op::{
    WireBitOp, WireBitOverflow, WireBitPolicy, WireBitResize, WireBitWriteMode, WireCtx,
    WireExpOp, WireExpReadFlags, WireExpWriteFlags, WireExpWriteMode, WireExpression, WireHllOp,
    WireHllPolicy, WireHllWriteMode, WireListOp, WireListOrder, WireListPolicy,
    WireListReturn, WireListReturnKind, WireListSort, WireListWriteFlags, WireMapOp, WireMapOrder,
    WireMapPolicy, WireMapReturn, WireMapReturnKind, WireMapWriteMode, WireOp,
};
pub use policy::WirePolicy;
pub use security::{
    WireAdminTarget, WirePrivilege, WirePrivilegeCode, WireRole, WireRoleAllowlistBody,
    WireRoleCreateBody, WireRoleNameBody, WireRolePrivilegesBody, WireRoleQuotasBody,
    WireRoleQueryBody, WireRoles, WireUser, WireUserCreateBody, WireUserNameBody,
    WireUserPasswordBody, WireUserQueryBody, WireUserRolesBody, WireUsers,
};
pub use txn::{
    WireAbortStatus, WireCommitStatus, WireTxnAbortBody, WireTxnBeginBody, WireTxnBody,
    WireTxnCommitBody, WireTxnHandle, WireTxnState,
};

use core::time::Duration;

use iceoryx2::prelude::ZeroCopySend;
use serde::{Deserialize, Serialize};

// ===== Protocol constants ===================================================

/// Frame marker (`"ASPH"`), guarding against an unrelated protocol
/// appearing on the same service name.
pub const MAGIC: u32 = 0x4153_5048;

/// The build that both ends of the channel must share.
///
/// This is *this crate's* package version, so the extension and the daemon
/// cannot disagree about what it is: both compile it in from here, and
/// [`rpc_service_name`] embeds it, so two peers of different versions never
/// meet in the first place.
///
/// Tying the wire format to the release version rather than to a separately
/// bumped protocol counter is deliberate. A counter only helps if the two
/// sides are *allowed* to differ, which buys backwards compatibility at the
/// price of every change having to reason about it. These two components ship
/// together, from one repository, and are installed together — so the simpler
/// rule is the honest one: **same version or no conversation.** The payoff is
/// that a wire change costs nothing more than changing it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// [`VERSION`] reduced to a fixed-width value the typed headers can carry.
///
/// A `#[repr(C)]` header cannot hold a `&str`, and the version is worth
/// checking on *every* frame rather than only at attach time: a service name
/// resolves once, but a stale mapping or a daemon restarted from a different
/// build would otherwise go unnoticed until it produced nonsense.
pub const VERSION_FINGERPRINT: u64 = fingerprint(VERSION);

/// Whether `candidate` is exactly [`VERSION`].
///
/// A `const fn` so that a crate on either end can assert its *own* package
/// version against the contract's at compile time:
///
/// ```
/// const _: () = assert!(
///     aerospike_php_ipc::is_version(env!("CARGO_PKG_VERSION")),
///     "this crate's version must equal aerospike-php-ipc's"
/// );
/// ```
///
/// The PHP extension is outside the cargo workspace — it links the Zend API,
/// so a plain `cargo build` at the repository root must not need a PHP
/// toolchain — which means it cannot inherit `version.workspace`. That
/// assertion is what keeps its hand-written version from drifting.
#[must_use]
pub const fn is_version(candidate: &str) -> bool {
    let (mine, theirs) = (VERSION.as_bytes(), candidate.as_bytes());
    if mine.len() != theirs.len() {
        return false;
    }
    let mut index = 0;
    while index < mine.len() {
        if mine[index] != theirs[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// FNV-1a over the version string.
///
/// Not a cryptographic hash and not required to be one: it distinguishes
/// release strings that were chosen by a human, not inputs chosen by an
/// attacker, and [`rpc_service_name`] already keeps mismatched peers apart.
const fn fingerprint(text: &str) -> u64 {
    let bytes = text.as_bytes();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

/// Initial slice length both ends request for payload loans. Larger
/// frames still work — the ports grow by powers of two — but staying
/// under this avoids a reallocation.
pub const INITIAL_MAX_SLICE_LEN: usize = 64 * 1024;

/// Hard ceiling on one payload. Reported as [`StatusCode::FRAME_TOO_LARGE`]
/// rather than tearing the channel down; 8 MiB is the largest record an
/// Aerospike server will return.
pub const MAX_PAYLOAD_LEN: usize = 8 * 1024 * 1024;

/// Instance assumed when PHP names none. Matches `[cluster.default]` in
/// the daemon's configuration file.
pub const DEFAULT_INSTANCE: &str = "default";

/// Sentinel in [`ReplyHeader::ttl`] for a record that never expires.
pub const TTL_NEVER_EXPIRES: u32 = u32::MAX;

/// Operation selectors for [`RequestHeader::opcode`].
///
/// # Why result sets will not stream
///
/// iceoryx2 lets a server answer one request with many responses, which looks
/// like a natural fit for a scan or a query — but measured behaviour rules it
/// out. With a response buffer of 16, a server that sent 50 replies had **every
/// send return `Ok` while the client received only the last 16**: the buffer is
/// a ring that overwrites its oldest entry, and the producer is never told.
/// Dropping the client's pending response is equally quiet — the server sees
/// `is_connected() == false` but `send()` still succeeds into the void.
///
/// Silently losing records from a scan is the worst failure mode a database
/// client can have, so paging is what [`query`] does instead: a scan or query
/// request returns a bounded page plus a continuation cursor, and the caller
/// asks for the next one. The cost is a round trip per page (~190us) amortised
/// over hundreds of records; the benefit is that flow control is inherent rather
/// than something the client has to keep up with.
/// # Request and reply shapes
///
/// Both halves belong here. Documenting only the request side once left the
/// extension to guess what `DELETE` answers with — it guessed a
/// [`StatusCode::RECORD_NOT_FOUND`] status and was wrong, which is exactly the
/// class of mistake a shared contract exists to prevent.
///
/// | Opcode | Request payload | Success payload |
/// | --- | --- | --- |
/// | [`PING`](opcode::PING) | empty | [`PongBody`] |
/// | [`PUT`](opcode::PUT) | [`PutBody`] | empty |
/// | [`GET`](opcode::GET) | [`GetBody`] | [`RecordBody`]; generation and TTL in [`ReplyHeader`] |
/// | [`DELETE`](opcode::DELETE) | [`TargetBody`] | [`WireValue::Bool`] — did the record exist? |
/// | [`TOUCH`](opcode::TOUCH) | [`TargetBody`] | empty |
/// | [`EXISTS`](opcode::EXISTS) | [`TargetBody`] | [`WireValue::Bool`] |
/// | [`ADD`](opcode::ADD), [`APPEND`](opcode::APPEND), [`PREPEND`](opcode::PREPEND) | [`ModifyBody`] | empty |
/// | [`OPERATE`](opcode::OPERATE) | [`OperateBody`] | [`RecordBody`]; generation and TTL in [`ReplyHeader`] |
/// | [`BATCH`](opcode::BATCH) | [`WireBatchBody`] | [`WireBatchReply`]; per-row metadata in the payload |
/// | [`QUERY`](opcode::QUERY) | [`WireQueryBody`] | [`WireQueryPage`] — the first page, and a cursor if more follow |
/// | [`QUERY_NEXT`](opcode::QUERY_NEXT) | [`WireCursorBody`] | [`WireQueryPage`] |
/// | [`QUERY_CLOSE`](opcode::QUERY_CLOSE) | [`WireCursorBody`] | empty |
/// | [`UDF_REGISTER`](opcode::UDF_REGISTER) | [`WireUdfRegisterBody`] | [`WireTaskHandle`] |
/// | [`UDF_REMOVE`](opcode::UDF_REMOVE) | [`WireUdfRemoveBody`] | [`WireTaskHandle`] |
/// | [`UDF_LIST`](opcode::UDF_LIST) | [`WireUdfListBody`] | [`WireUdfList`] |
/// | [`UDF_EXECUTE`](opcode::UDF_EXECUTE) | [`WireUdfExecuteBody`] | [`WireUdfResult`] |
/// | [`QUERY_UDF`](opcode::QUERY_UDF) | [`WireQueryUdfBody`] | [`WireTaskHandle`] |
/// | [`INDEX_CREATE`](opcode::INDEX_CREATE) | [`WireIndexCreateBody`] | [`WireTaskHandle`] |
/// | [`INDEX_DROP`](opcode::INDEX_DROP) | [`WireIndexDropBody`] | [`WireTaskHandle`] |
/// | [`TASK_STATUS`](opcode::TASK_STATUS) | [`WireTaskStatusBody`] | [`WireTaskStatus`] |
/// | [`TRUNCATE`](opcode::TRUNCATE) | [`WireTruncateBody`] | empty |
/// | [`INFO`](opcode::INFO) | [`WireInfoBody`] | [`WireInfoReply`] |
/// | [`NODES`](opcode::NODES) | [`WireNodesBody`] | [`WireNodes`] |
/// | [`TXN_BEGIN`](opcode::TXN_BEGIN) | [`WireTxnBeginBody`] | [`WireTxnHandle`] |
/// | [`TXN_COMMIT`](opcode::TXN_COMMIT) | [`WireTxnCommitBody`] | [`WireCommitStatus`] |
/// | [`TXN_ABORT`](opcode::TXN_ABORT) | [`WireTxnAbortBody`] | [`WireAbortStatus`] |
/// | [`TXN_STATE`](opcode::TXN_STATE) | [`WireTxnBody`] | [`WireTxnState`] |
/// | [`USER_CREATE`](opcode::USER_CREATE) | [`WireUserCreateBody`] | empty |
/// | [`USER_CREATE_PKI`](opcode::USER_CREATE_PKI) | [`WireUserRolesBody`] | empty |
/// | [`USER_DROP`](opcode::USER_DROP) | [`WireUserNameBody`] | empty |
/// | [`USER_PASSWORD`](opcode::USER_PASSWORD) | [`WireUserPasswordBody`] | empty |
/// | [`USER_QUERY`](opcode::USER_QUERY) | [`WireUserQueryBody`] | [`WireUsers`] |
/// | [`USER_GRANT_ROLES`](opcode::USER_GRANT_ROLES), [`USER_REVOKE_ROLES`](opcode::USER_REVOKE_ROLES) | [`WireUserRolesBody`] | empty |
/// | [`ROLE_CREATE`](opcode::ROLE_CREATE) | [`WireRoleCreateBody`] | empty |
/// | [`ROLE_DROP`](opcode::ROLE_DROP) | [`WireRoleNameBody`] | empty |
/// | [`ROLE_QUERY`](opcode::ROLE_QUERY) | [`WireRoleQueryBody`] | [`WireRoles`] |
/// | [`ROLE_GRANT_PRIVILEGES`](opcode::ROLE_GRANT_PRIVILEGES), [`ROLE_REVOKE_PRIVILEGES`](opcode::ROLE_REVOKE_PRIVILEGES) | [`WireRolePrivilegesBody`] | empty |
/// | [`ROLE_ALLOWLIST`](opcode::ROLE_ALLOWLIST) | [`WireRoleAllowlistBody`] | empty |
/// | [`ROLE_QUOTAS`](opcode::ROLE_QUOTAS) | [`WireRoleQuotasBody`] | empty |
///
/// A failure always carries [`ErrorBody`], whatever the opcode.
///
/// `OPERATE` answers with the same [`RecordBody`] a read does, holding one entry
/// per operation that produced a result — so a call with no read operations
/// answers with an empty one rather than with nothing. A record the operations
/// did not find is [`StatusCode::RECORD_NOT_FOUND`], as on a `GET`.
///
/// `BATCH` is the one reply whose status says nothing about the *work*: a batch
/// where some rows failed is still [`StatusCode::OK`], because a row's failure
/// belongs to that row. Its metadata is per row and therefore in the payload —
/// there is no one generation for a batch.
///
/// Note what `DELETE` and `EXISTS` deliberately do *not* do: a record that was
/// not there is reported as `OK` with `false`, not as
/// [`StatusCode::RECORD_NOT_FOUND`]. "It wasn't there" is the answer to both
/// questions, not a failure to answer them.
///
/// The three query opcodes are one conversation, described in [`query`]: `QUERY`
/// opens and answers with the first page, `QUERY_NEXT` continues, `QUERY_CLOSE`
/// abandons. A page whose cursor is `None` has finished and needs neither of the
/// other two.
pub mod opcode {
    /// Liveness and version handshake.
    pub const PING: u16 = 1;
    /// Write bins to one record.
    pub const PUT: u16 = 2;
    /// Read one record.
    pub const GET: u16 = 3;
    /// Delete one record.
    pub const DELETE: u16 = 4;
    /// Reset one record's time-to-live.
    pub const TOUCH: u16 = 5;
    /// Test for one record's existence.
    pub const EXISTS: u16 = 6;
    /// Add integer or float deltas to bins.
    pub const ADD: u16 = 7;
    /// Append to string or blob bins.
    pub const APPEND: u16 = 8;
    /// Prepend to string or blob bins.
    pub const PREPEND: u16 = 9;
    /// Run a list of operations against one record, in order.
    pub const OPERATE: u16 = 10;
    /// Run many rows — reads, writes, deletes and UDF calls — in one round
    /// trip per node.
    pub const BATCH: u16 = 11;
    /// Start a scan or query, and return its first page.
    pub const QUERY: u16 = 12;
    /// Return the next page of a scan or query.
    pub const QUERY_NEXT: u16 = 13;
    /// Abandon a scan or query before it finished.
    pub const QUERY_CLOSE: u16 = 14;
    /// Register a UDF module on every node.
    pub const UDF_REGISTER: u16 = 15;
    /// Remove a UDF module from every node.
    pub const UDF_REMOVE: u16 = 16;
    /// List the UDF modules the cluster holds.
    pub const UDF_LIST: u16 = 17;
    /// Run a UDF against one record.
    pub const UDF_EXECUTE: u16 = 18;
    /// Run a UDF against every record a statement matches, in the background.
    pub const QUERY_UDF: u16 = 19;
    /// Create a secondary index.
    pub const INDEX_CREATE: u16 = 20;
    /// Drop a secondary index.
    pub const INDEX_DROP: u16 = 21;
    /// Ask how far a long-running command has got.
    pub const TASK_STATUS: u16 = 22;
    /// Delete every record of a set, or of a namespace.
    pub const TRUNCATE: u16 = 23;
    /// Send info commands to a node.
    pub const INFO: u16 = 24;
    /// List the cluster's nodes as the daemon sees them.
    pub const NODES: u16 = 25;
    /// Open a multi-record transaction.
    pub const TXN_BEGIN: u16 = 26;
    /// Commit a multi-record transaction.
    pub const TXN_COMMIT: u16 = 27;
    /// Abort a multi-record transaction.
    pub const TXN_ABORT: u16 = 28;
    /// Ask where a multi-record transaction has got to.
    pub const TXN_STATE: u16 = 29;
    /// Create a user with a password.
    pub const USER_CREATE: u16 = 30;
    /// Create a user the client certificate identifies.
    pub const USER_CREATE_PKI: u16 = 31;
    /// Remove a user.
    pub const USER_DROP: u16 = 32;
    /// Change a user's password.
    pub const USER_PASSWORD: u16 = 33;
    /// Describe one user, or every user.
    pub const USER_QUERY: u16 = 34;
    /// Assign roles to a user.
    pub const USER_GRANT_ROLES: u16 = 35;
    /// Take roles away from a user.
    pub const USER_REVOKE_ROLES: u16 = 36;
    /// Create a role.
    pub const ROLE_CREATE: u16 = 37;
    /// Remove a role.
    pub const ROLE_DROP: u16 = 38;
    /// Describe one role, or every role.
    pub const ROLE_QUERY: u16 = 39;
    /// Add privileges to a role.
    pub const ROLE_GRANT_PRIVILEGES: u16 = 40;
    /// Take privileges away from a role.
    pub const ROLE_REVOKE_PRIVILEGES: u16 = 41;
    /// Replace a role's address allowlist.
    pub const ROLE_ALLOWLIST: u16 = 42;
    /// Replace a role's rate quotas.
    pub const ROLE_QUOTAS: u16 = 43;
}

/// Bit flags for [`ReplyHeader::flags`].
pub mod flags {
    /// A write may have been applied despite the error: PHP must not
    /// blindly retry a non-idempotent operation.
    pub const IN_DOUBT: u32 = 1 << 0;
    /// [`crate::ReplyHeader::result_code`] carries a meaningful server code.
    pub const HAS_RESULT_CODE: u32 = 1 << 1;
}

/// Outcome classification in [`ReplyHeader::status`].
///
/// Deliberately a set of constants rather than an enum: the header is a
/// `#[repr(C)]` type read straight out of shared memory, and a peer
/// sending an out-of-range discriminant must be a recoverable validation
/// error, not undefined behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusCode(pub u16);

impl StatusCode {
    /// Succeeded.
    pub const OK: StatusCode = StatusCode(0);
    /// The server returned a non-OK result code.
    pub const SERVER: StatusCode = StatusCode(1);
    /// The record does not exist.
    pub const RECORD_NOT_FOUND: StatusCode = StatusCode(2);
    /// Timed out.
    pub const TIMEOUT: StatusCode = StatusCode(3);
    /// The daemon could not reach the cluster.
    pub const CONNECTION: StatusCode = StatusCode(4);
    /// No configuration for the requested instance.
    pub const UNKNOWN_INSTANCE: StatusCode = StatusCode(5);
    /// Malformed or unsupported request.
    pub const INVALID_REQUEST: StatusCode = StatusCode(6);
    /// A frame exceeded [`MAX_PAYLOAD_LEN`].
    pub const FRAME_TOO_LARGE: StatusCode = StatusCode(7);
    /// Daemon-internal failure.
    pub const INTERNAL: StatusCode = StatusCode(8);
    /// The scan or query cursor is no longer open: it was closed, it was
    /// expired for being idle, or the daemon restarted under it.
    ///
    /// Its own status rather than an [`INVALID_REQUEST`](Self::INVALID_REQUEST)
    /// because it is the one failure a caller can act on — restart the
    /// traversal — and because the alternative, answering with an empty page,
    /// would truncate a scan silently.
    pub const CURSOR_EXPIRED: StatusCode = StatusCode(9);
    /// The multi-record transaction is no longer open: it was committed, aborted,
    /// or aborted *for the caller* after sitting idle too long.
    ///
    /// Its own status, like [`CURSOR_EXPIRED`](Self::CURSOR_EXPIRED), because it
    /// is the one transaction failure a caller can act on — start again — and
    /// because it must not be confused with a command that failed *inside* a
    /// still-open transaction, which leaves the transaction abortable.
    pub const TXN_EXPIRED: StatusCode = StatusCode(10);

    /// Whether this is the success status.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        self.0 == Self::OK.0
    }

    /// A short, stable label, used in messages and to name PHP exception
    /// classes.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self.0 {
            0 => "OK",
            1 => "SERVER",
            2 => "RECORD_NOT_FOUND",
            3 => "TIMEOUT",
            4 => "CONNECTION",
            5 => "UNKNOWN_INSTANCE",
            6 => "INVALID_REQUEST",
            7 => "FRAME_TOO_LARGE",
            8 => "INTERNAL",
            9 => "CURSOR_EXPIRED",
            10 => "TXN_EXPIRED",
            _ => "UNRECOGNIZED",
        }
    }
}

// ===== Service naming =======================================================

/// Name of the per-worker event service the daemon notifies after
/// writing a reply for `client_id`.
///
/// Unused when the `shm-poll` feature is enabled; see
/// [`wakeup`]. Naming it per worker is what makes a reply wake exactly
/// one process instead of every waiting one.
#[must_use]
pub fn event_service_name(instance: &str, client_id: ClientId) -> String {
    format!("aerospike/{instance}/v{VERSION}/evt/{}", client_id.0)
}

/// Name of the shared request/response service for `instance`.
///
/// One service carries every worker's traffic: iceoryx2 request/response
/// supports many clients against one server, and each client's pending
/// response is its own, so no multiplexing key is needed.
///
/// The name embeds [`VERSION`], which is what makes the lock-step rule
/// self-enforcing: an extension simply cannot open a service a differently
/// versioned daemon created. [`parse_rpc_service_name`] reads the name back,
/// so "no daemon" and "the wrong daemon" can be told apart in the message.
#[must_use]
pub fn rpc_service_name(instance: &str) -> String {
    format!("aerospike/{instance}/v{VERSION}/rpc")
}

/// The instance and version encoded in an RPC service name.
///
/// `None` for a name that is not one of ours. Used to turn a failed attach
/// into a diagnosis: a daemon *is* running for this instance, but it is
/// version X and this build is [`VERSION`].
#[must_use]
pub fn parse_rpc_service_name(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("aerospike/")?;
    let rest = rest.strip_suffix("/rpc")?;
    let (instance, version) = rest.rsplit_once("/v")?;
    if instance.is_empty() || version.is_empty() {
        return None;
    }
    Some((instance, version))
}

// ===== Typed headers ========================================================

/// Identifies one attached PHP worker for the life of its process.
///
/// PHP-FPM forks its workers, so this must be minted *after* the fork.
/// The pid alone will not do: pids are recycled, and a stale id could
/// collide with resources the daemon has not yet reaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u64);

impl ClientId {
    /// Combine the process id with a per-process nonce.
    #[must_use]
    pub const fn new(pid: u32, nonce: u32) -> ClientId {
        ClientId(((pid as u64) << 32) | nonce as u64)
    }
}

/// Typed, zero-copy fixed part of a request.
///
/// `Default` is required by iceoryx2: `loan_slice_uninit` default-initialises
/// the user header before handing it over. A defaulted header is deliberately
/// *invalid* — its marker is zero, so [`validate`](Self::validate) rejects it
/// and a forgotten initialisation fails loudly instead of travelling as a
/// plausible-looking frame.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, ZeroCopySend)]
pub struct RequestHeader {
    /// Must equal [`MAGIC`].
    pub magic: u32,
    /// One of [`opcode`].
    pub opcode: u16,
    /// Explicit padding, so no uninitialised byte of this header is ever
    /// published into shared memory.
    pub reserved: u16,
    /// Must equal [`VERSION_FINGERPRINT`].
    pub version: u64,
    /// Monotonic per-worker counter. PHP is synchronous, so a worker has
    /// at most one request outstanding and this is not a multiplexing
    /// key — it exists so a reply arriving after the worker stopped
    /// waiting is recognised as stale rather than mistaken for the answer
    /// to the *next* call.
    pub seq: u64,
    /// Which worker sent this.
    pub client_id: u64,
    /// How long the worker intends to wait, so the daemon can abandon
    /// work nobody is listening for.
    pub timeout_ms: u32,
    /// Length of the postcard payload, cross-checked against the slice.
    pub body_len: u32,
}

impl RequestHeader {
    /// Build a header for `opcode`.
    #[must_use]
    pub const fn new(
        opcode: u16,
        seq: u64,
        client_id: ClientId,
        timeout_ms: u32,
        body_len: u32,
    ) -> RequestHeader {
        RequestHeader {
            magic: MAGIC,
            version: VERSION_FINGERPRINT,
            reserved: 0,
            opcode,
            seq,
            client_id: client_id.0,
            timeout_ms,
            body_len,
        }
    }

    /// Check the marker, version and declared body length.
    ///
    /// # Errors
    /// [`CodecError`] describing which check failed.
    pub fn validate(&self, payload_len: usize) -> Result<(), CodecError> {
        validate_preamble(self.magic, self.version)?;
        if self.body_len as usize != payload_len {
            return Err(CodecError::BodyLenMismatch {
                declared: self.body_len as usize,
                actual: payload_len,
            });
        }
        Ok(())
    }
}

/// Typed, zero-copy fixed part of a reply.
///
/// Record metadata lives here rather than in the payload: generation and
/// TTL are fixed-width, so there is no reason to serialize them.
///
/// As with [`RequestHeader`], `Default` exists because iceoryx2
/// default-initialises user headers on loan, and a defaulted value fails
/// [`validate`](Self::validate) by design.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, ZeroCopySend)]
pub struct ReplyHeader {
    /// Must equal [`MAGIC`].
    pub magic: u32,
    /// A [`StatusCode`] value.
    pub status: u16,
    /// Explicit padding, as on [`RequestHeader::reserved`].
    pub reserved: u16,
    /// Must equal [`VERSION_FINGERPRINT`].
    pub version: u64,
    /// Echoes [`RequestHeader::seq`].
    pub seq: u64,
    /// Server result code; meaningful only with
    /// [`flags::HAS_RESULT_CODE`].
    pub result_code: i32,
    /// See [`flags`].
    pub flags: u32,
    /// Record generation, for a successful read.
    pub generation: u32,
    /// Remaining seconds to live, or [`TTL_NEVER_EXPIRES`].
    pub ttl: u32,
    /// Length of the postcard payload, cross-checked against the slice.
    pub body_len: u32,
}

impl ReplyHeader {
    /// A reply carrying only a status (a write, or an error whose detail
    /// is in the payload).
    #[must_use]
    pub const fn new(status: StatusCode, seq: u64, body_len: u32) -> ReplyHeader {
        ReplyHeader {
            magic: MAGIC,
            version: VERSION_FINGERPRINT,
            reserved: 0,
            status: status.0,
            seq,
            result_code: 0,
            flags: 0,
            generation: 0,
            ttl: 0,
            body_len,
        }
    }

    /// The status this reply carries.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        StatusCode(self.status)
    }

    /// Whether a failed write may still have been applied.
    #[must_use]
    pub const fn in_doubt(&self) -> bool {
        self.flags & flags::IN_DOUBT != 0
    }

    /// The server result code, when one was attached.
    #[must_use]
    pub const fn result_code(&self) -> Option<i32> {
        if self.flags & flags::HAS_RESULT_CODE != 0 {
            Some(self.result_code)
        } else {
            None
        }
    }

    /// Remaining time to live; `None` means the record never expires.
    #[must_use]
    pub const fn time_to_live(&self) -> Option<u32> {
        if self.ttl == TTL_NEVER_EXPIRES {
            None
        } else {
            Some(self.ttl)
        }
    }

    /// Attach a server result code and in-doubt marker.
    #[must_use]
    pub const fn with_server_error(mut self, result_code: i32, in_doubt: bool) -> ReplyHeader {
        self.result_code = result_code;
        self.flags |= flags::HAS_RESULT_CODE;
        if in_doubt {
            self.flags |= flags::IN_DOUBT;
        }
        self
    }

    /// Attach record metadata.
    #[must_use]
    pub const fn with_metadata(mut self, generation: u32, ttl: Option<u32>) -> ReplyHeader {
        self.generation = generation;
        self.ttl = match ttl {
            Some(seconds) => seconds,
            None => TTL_NEVER_EXPIRES,
        };
        self
    }

    /// Check the marker, version and declared body length.
    ///
    /// # Errors
    /// [`CodecError`] describing which check failed.
    pub fn validate(&self, payload_len: usize) -> Result<(), CodecError> {
        validate_preamble(self.magic, self.version)?;
        if self.body_len as usize != payload_len {
            return Err(CodecError::BodyLenMismatch {
                declared: self.body_len as usize,
                actual: payload_len,
            });
        }
        Ok(())
    }
}

fn validate_preamble(magic: u32, version: u64) -> Result<(), CodecError> {
    if magic != MAGIC {
        return Err(CodecError::BadMagic { found: magic });
    }
    if version != VERSION_FINGERPRINT {
        return Err(CodecError::VersionMismatch {
            found: version,
            expected: VERSION_FINGERPRINT,
        });
    }
    Ok(())
}

// ===== Variable-length bodies ==============================================

/// Where a single-record operation is aimed. Shared by every one-key verb so
/// the daemon has one place to resolve an instance and build a key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target {
    /// Cluster instance from the daemon's configuration.
    pub instance: String,
    /// Namespace.
    pub namespace: String,
    /// Set name; empty means the null set.
    pub set: String,
    /// Record key.
    pub key: WireKey,
    /// Per-call overrides; default means "use the daemon's settings".
    pub policy: WirePolicy,
}

/// Payload of [`opcode::DELETE`], [`opcode::TOUCH`] and [`opcode::EXISTS`] —
/// verbs that name a record and carry nothing else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetBody {
    /// The record and its policy.
    pub target: Target,
}

/// Payload of [`opcode::ADD`], [`opcode::APPEND`] and [`opcode::PREPEND`]:
/// bin-wise modifications whose shape is identical to a write.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModifyBody {
    /// The record and its policy.
    pub target: Target,
    /// Bins and the values to add, append or prepend.
    pub bins: Vec<(String, WireValue)>,
}

/// Payload of an [`opcode::PUT`] request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PutBody {
    /// The record and its per-call policy.
    pub target: Target,
    /// Bins to write, in order. [`WireValue::Nil`] deletes a bin.
    pub bins: Vec<(String, WireValue)>,
}

/// Payload of an [`opcode::GET`] request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetBody {
    /// The record and its per-call policy.
    pub target: Target,
    /// Which bins to return.
    pub bins: BinSelector,
}

/// Payload of an [`opcode::OPERATE`] request.
///
/// The operations run **in the order given**, on one record, atomically — that
/// ordering is the whole point of `operate`, so the list is a vector and the
/// daemon must not reorder it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperateBody {
    /// The record and its per-call policy.
    pub target: Target,
    /// What to do, in order.
    pub ops: Vec<WireOp>,
}

impl OperateBody {
    /// Whether any operation writes.
    ///
    /// Decides which of the client's policies the daemon resolves: a call that
    /// only reads must not be sent as a write, or it would take a write lock and
    /// bump the generation of a record nobody meant to change.
    #[must_use]
    pub fn has_write(&self) -> bool {
        self.ops.iter().any(WireOp::is_write)
    }
}

/// Payload of a successful read. Generation and TTL travel in
/// [`ReplyHeader`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordBody {
    /// Bins in the order the server returned them.
    pub bins: Vec<(String, WireValue)>,
}

/// Payload of a failed reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Human-readable detail.
    pub message: String,
}

/// Payload of a [`opcode::PING`] reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PongBody {
    /// Daemon package version.
    pub daemon_version: String,
    /// Instances the daemon is configured for.
    pub instances: Vec<String>,
}

/// Which bins a read should return.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinSelector {
    /// Every bin.
    All,
    /// No bins — metadata only.
    None,
    /// Just these bins.
    Only(Vec<String>),
}

/// A record key as it crosses the channel. The daemon builds the real key
/// — and therefore computes the digest — on its side.
// `Eq` as well as `PartialEq`: a key holds an integer, a string or bytes and
// never a float, so equality really is an equivalence — and the query contract's
// `WireRecordKey` wants to be `Eq` too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireKey {
    /// Integer key.
    Int(i64),
    /// String key.
    Str(String),
    /// Binary key.
    Blob(Vec<u8>),
}

/// The subset of Aerospike's value model phase one carries.
///
/// Intentionally *not* the client's own value type: keeping the contract
/// independent means the extension never links the database client, and
/// adding a variant stays a deliberate protocol change rather than a
/// side effect of an upstream refactor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireValue {
    /// Nil. Writing this deletes the bin.
    Nil,
    /// Boolean.
    Bool(bool),
    /// 64-bit signed integer.
    Int(i64),
    /// Double.
    Float(f64),
    /// UTF-8 string.
    Str(String),
    /// Byte string.
    Blob(Vec<u8>),
    /// Ordered list.
    List(Vec<WireValue>),
    /// Key/value pairs, stored unordered.
    ///
    /// A vector rather than a Rust map, so server-side ordering survives
    /// the round trip and non-string keys stay expressible.
    Map(Vec<(WireValue, WireValue)>),
    /// Key/value pairs whose given order is preserved *on the wire*.
    ///
    /// The server has no insertion-ordered map type, so this and
    /// [`WireValue::Map`] both store an unordered map and both read back as
    /// `Map`. Only [`WireValue::SortedMap`] is distinguishable once stored.
    OrderedMap(Vec<(WireValue, WireValue)>),
    /// Key/value pairs stored sorted by key, which is what the server
    /// needs for the ordered-map operations to work in log time.
    SortedMap(Vec<(WireValue, WireValue)>),
    /// A GeoJSON document. Distinct from [`WireValue::Str`] because the
    /// server indexes and queries it as geometry.
    GeoJson(String),
    /// A HyperLogLog sketch. Opaque bytes: only the server's HLL
    /// operations may interpret them.
    Hll(Vec<u8>),
    /// The value that sorts above every other, for open-ended CDT range
    /// selections.
    Infinity,
    /// Matches any value, for CDT selections by example.
    Wildcard,

    // ----- read-only shapes the server produces -----
    /// Several results for one bin name, in operation order. Produced when
    /// a single `operate` call reads the same bin more than once; never
    /// sent by a caller.
    MultiResult(Vec<WireValue>),
    /// The flat key/value result of a map read that asked for both. Kept
    /// distinct from [`WireValue::Map`] because it is a *result* shape,
    /// not a stored one.
    KeyValueList(Vec<(WireValue, WireValue)>),
    /// A particle type this build does not know how to decode, passed
    /// through verbatim.
    ///
    /// Forwarding the bytes rather than failing means a newer server's
    /// data type degrades to something inspectable instead of breaking the
    /// whole read.
    Unknown {
        /// The server's particle type byte.
        particle_type: u8,
        /// The undecoded payload.
        data: Vec<u8>,
    },
}

impl WireValue {
    /// Whether this shape is only ever produced by the server, and so must
    /// be rejected if a caller sends it.
    ///
    /// Enforcing this at the edge keeps the daemon's conversion total: it
    /// never has to invent an `aerospike-core` value for a result-only
    /// shape.
    #[must_use]
    pub const fn is_result_only(&self) -> bool {
        matches!(
            self,
            WireValue::MultiResult(_) | WireValue::KeyValueList(_) | WireValue::Unknown { .. }
        )
    }
}

// ===== Body codec ===========================================================

/// Failures of the framing itself, as opposed to of an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The marker did not match [`MAGIC`].
    BadMagic {
        /// The marker that was present.
        found: u32,
    },
    /// The peer was built from a different version of this contract.
    ///
    /// Fingerprints rather than version strings, because that is all a
    /// fixed-width header can carry; the peer's readable version comes from
    /// [`PongBody::daemon_version`] or from the service name it created.
    VersionMismatch {
        /// Fingerprint the peer sent.
        found: u64,
        /// [`VERSION_FINGERPRINT`], for [`VERSION`].
        expected: u64,
    },
    /// The header's declared body length disagreed with the payload.
    BodyLenMismatch {
        /// Length the header declared.
        declared: usize,
        /// Length actually received.
        actual: usize,
    },
    /// The payload would exceed [`MAX_PAYLOAD_LEN`].
    TooLarge {
        /// Size that was attempted.
        len: usize,
    },
    /// The opcode is not one this build serves.
    UnknownOpcode {
        /// The opcode received.
        opcode: u16,
    },
    /// The body did not decode.
    Malformed,
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CodecError::BadMagic { found } => {
                write!(f, "bad frame marker {found:#010x}, expected {MAGIC:#010x}")
            }
            CodecError::VersionMismatch { found, expected } => write!(
                f,
                "the peer was built from a different version of aerospike-php-ipc (its \
                 fingerprint is {found:#018x}, this build is {VERSION}, {expected:#018x}); the \
                 daemon and the extension must be the same version"
            ),
            CodecError::BodyLenMismatch { declared, actual } => write!(
                f,
                "header declared a {declared}-byte body but the payload is {actual} bytes"
            ),
            CodecError::TooLarge { len } => write!(
                f,
                "payload of {len} bytes exceeds the {MAX_PAYLOAD_LEN}-byte maximum"
            ),
            CodecError::UnknownOpcode { opcode } => write!(f, "unknown opcode {opcode}"),
            CodecError::Malformed => f.write_str("frame body did not decode"),
        }
    }
}

impl core::error::Error for CodecError {}

/// Encode a body for the payload slice.
///
/// # Errors
/// [`CodecError::TooLarge`] past [`MAX_PAYLOAD_LEN`];
/// [`CodecError::Malformed`] if the value cannot be serialized.
pub fn encode_body<T: Serialize>(body: &T) -> Result<Vec<u8>, CodecError> {
    let bytes = postcard::to_allocvec(body).map_err(|_| CodecError::Malformed)?;
    if bytes.len() > MAX_PAYLOAD_LEN {
        return Err(CodecError::TooLarge { len: bytes.len() });
    }
    Ok(bytes)
}

/// Decode a body from a payload slice.
///
/// # Errors
/// [`CodecError::Malformed`] when the bytes do not decode to `T`.
pub fn decode_body<T: for<'de> Deserialize<'de>>(payload: &[u8]) -> Result<T, CodecError> {
    postcard::from_bytes(payload).map_err(|_| CodecError::Malformed)
}

// ===== Waiting ==============================================================

/// Tiered wait used while polling a shared-memory port.
///
/// Shared memory is the only transport here, so a waiting side polls
/// rather than blocking on an event. Spinning first keeps the fast path
/// at cache-line latency; escalating to yields and then to short sleeps
/// stops an idle worker from burning a core while the daemon waits on the
/// network. Both tiers are configurable because the right trade depends
/// on how many workers a host runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    /// Iterations to spin with a busy hint before yielding.
    pub spin_iters: u32,
    /// Iterations to yield the thread before sleeping.
    pub yield_iters: u32,
    /// First sleep once yielding is exhausted.
    pub initial_sleep: Duration,
    /// Ceiling the sleep grows to.
    pub max_sleep: Duration,
}

impl Default for Backoff {
    /// Tuned against a measured round trip.
    ///
    /// A single-record operation against a cluster on the same host takes
    /// roughly 180µs, so the ladder has to stay in its cheap tiers for at
    /// least that long or it sleeps straight through the reply. An earlier
    /// version escalated after ~1µs of spinning and let sleeps double to
    /// 500µs; because *both* the daemon's receive loop and the waiting
    /// worker climbed it, the two overshoots compounded and added ~1.4ms
    /// per operation — six times the cost of the event build, while using
    /// *less* CPU, which is the signature of sleeping rather than polling.
    ///
    /// So: spin long enough to cover a local round trip, yield for a
    /// while after that, and treat sleeping as a last resort with a low
    /// ceiling that bounds how long an idle daemon can take to notice a
    /// request. Deployments with a remote cluster (milliseconds, not
    /// microseconds) or many idle workers per core should raise the sleep
    /// tiers, or drop the `shm-poll` feature and let the event build wait
    /// for them instead.
    fn default() -> Backoff {
        Backoff {
            spin_iters: 50_000,
            yield_iters: 100,
            initial_sleep: Duration::from_micros(10),
            max_sleep: Duration::from_micros(50),
        }
    }
}

impl Backoff {
    /// Start a wait governed by this policy.
    #[must_use]
    pub const fn start(self) -> BackoffState {
        BackoffState {
            policy: self,
            step: 0,
            sleep: self.initial_sleep,
        }
    }
}

/// In-progress wait produced by [`Backoff::start`].
#[derive(Debug, Clone, Copy)]
pub struct BackoffState {
    policy: Backoff,
    step: u32,
    sleep: Duration,
}

impl BackoffState {
    /// Pause once, escalating from spinning to yielding to sleeping.
    pub fn snooze(&mut self) {
        if self.step < self.policy.spin_iters {
            core::hint::spin_loop();
        } else if self.step < self.policy.spin_iters + self.policy.yield_iters {
            std::thread::yield_now();
        } else {
            std::thread::sleep(self.sleep);
            let doubled = self.sleep.saturating_mul(2);
            self.sleep = if doubled > self.policy.max_sleep {
                self.policy.max_sleep
            } else {
                doubled
            };
        }
        self.step = self.step.saturating_add(1);
    }

    /// Count a wait that an external mechanism performed — an event
    /// block, rather than one of this policy's own pauses.
    ///
    /// This keeps [`steps`](Self::steps) meaning "waits performed" in
    /// either wakeup mode, so diagnostics read the same whichever
    /// strategy was compiled in.
    pub fn note_wait(&mut self) {
        self.step = self.step.saturating_add(1);
    }

    /// How many waits this attempt has taken — useful in diagnostics to
    /// tell a spin-served reply from a slept-through one.
    #[must_use]
    pub const fn steps(&self) -> u32 {
        self.step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put_body() -> PutBody {
        PutBody {
            target: Target {
                instance: DEFAULT_INSTANCE.into(),
                namespace: "test".into(),
                set: "users".into(),
                key: WireKey::Str("alice".into()),
                policy: WirePolicy {
                    expiration: Some(policy::WireExpiration::Seconds(60)),
                    ..WirePolicy::default()
                },
            },
            bins: vec![
                ("name".into(), WireValue::Str("Alice".into())),
                ("age".into(), WireValue::Int(30)),
                ("scores".into(), WireValue::List(vec![WireValue::Int(1)])),
                (
                    "prefs".into(),
                    WireValue::Map(vec![(
                        WireValue::Str("theme".into()),
                        WireValue::Str("dark".into()),
                    )]),
                ),
                ("retired".into(), WireValue::Nil),
            ],
        }
    }

    #[test]
    fn body_round_trips_through_postcard() {
        let body = put_body();
        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<PutBody>(&bytes).unwrap(), body);
    }

    #[test]
    fn headers_stay_pod_and_zero_copy_sized() {
        // A ZeroCopySend user header must be plain data: no pointers, and no
        // implicit padding, which is why both headers name their filler
        // explicitly. The sizes are pinned so a field added without thinking
        // about the layout is a failing test rather than a silent change to
        // what crosses shared memory.
        assert_eq!(core::mem::size_of::<RequestHeader>(), 40);
        assert_eq!(core::mem::size_of::<ReplyHeader>(), 48);
        assert!(core::mem::size_of::<RequestHeader>() % core::mem::align_of::<u64>() == 0);
        assert!(core::mem::size_of::<ReplyHeader>() % core::mem::align_of::<u64>() == 0);
    }

    /// The three mechanisms that keep two builds from talking to each other,
    /// each checked where it actually applies.
    #[test]
    fn one_version_governs_the_whole_channel() {
        // The fingerprint is derived from the version, so it cannot be bumped
        // independently of it — that is the point of deriving it.
        assert_eq!(VERSION_FINGERPRINT, fingerprint(VERSION));
        assert_ne!(VERSION_FINGERPRINT, fingerprint("0.0.0-not-this-build"));
        // A version string is never empty, so the seed must never survive
        // unchanged: an all-zero header would otherwise validate.
        assert_ne!(VERSION_FINGERPRINT, 0xcbf2_9ce4_8422_2325);

        // Both headers carry it, so a frame from another build is rejected
        // even if it somehow reaches a port this one owns.
        assert_eq!(
            RequestHeader::new(opcode::PING, 1, ClientId::new(1, 1), 0, 0).version,
            VERSION_FINGERPRINT
        );
        assert_eq!(ReplyHeader::new(StatusCode::OK, 1, 0).version, VERSION_FINGERPRINT);

        // And the service name embeds it, which is what stops the two from
        // meeting at all.
        assert!(rpc_service_name("default").contains(VERSION));
        assert!(event_service_name("default", ClientId::new(1, 2)).contains(VERSION));
    }

    /// The version travels in the service name as a plain string, so it has to
    /// survive iceoryx2's own name validation — release versions contain dots
    /// and hyphens, which nothing else in these names does.
    #[test]
    fn service_names_are_ones_iceoryx2_accepts() {
        use iceoryx2::prelude::ServiceName;

        for instance in [DEFAULT_INSTANCE, "analytics", "a"] {
            let rpc = rpc_service_name(instance);
            assert!(
                ServiceName::new(&rpc).is_ok(),
                "{rpc} is not a usable iceoryx2 service name"
            );
            let event = event_service_name(instance, ClientId::new(u32::MAX, u32::MAX));
            assert!(
                ServiceName::new(&event).is_ok(),
                "{event} is not a usable iceoryx2 service name"
            );
        }
    }

    /// Reading the name back is what lets a failed attach say "the wrong
    /// daemon" instead of "no daemon".
    #[test]
    fn an_rpc_service_name_parses_back_into_instance_and_version() {
        for instance in [DEFAULT_INSTANCE, "analytics"] {
            assert_eq!(
                parse_rpc_service_name(&rpc_service_name(instance)),
                Some((instance, VERSION))
            );
        }

        // A daemon of another version, which is precisely the case worth
        // diagnosing: recognised as ours, and reported with its version.
        assert_eq!(
            parse_rpc_service_name("aerospike/default/v2.7.1/rpc"),
            Some(("default", "2.7.1"))
        );

        for foreign in [
            "aerospike/default/v1/evt/7",
            "some/other/service",
            "aerospike//v1/rpc",
            "aerospike/default/v/rpc",
            "",
        ] {
            assert_eq!(parse_rpc_service_name(foreign), None, "{foreign}");
        }
    }

    #[test]
    fn request_header_validates_marker_version_and_length() {
        let body = encode_body(&put_body()).unwrap();
        let header = RequestHeader::new(
            opcode::PUT,
            7,
            ClientId::new(4242, 9),
            1_000,
            body.len() as u32,
        );
        assert!(header.validate(body.len()).is_ok());

        let mut bad = header;
        bad.magic ^= 0xFFFF;
        assert!(matches!(
            bad.validate(body.len()),
            Err(CodecError::BadMagic { .. })
        ));

        let mut other_build = header;
        other_build.version = fingerprint("0.0.0-some-other-build");
        assert_eq!(
            other_build.validate(body.len()),
            Err(CodecError::VersionMismatch {
                found: fingerprint("0.0.0-some-other-build"),
                expected: VERSION_FINGERPRINT,
            })
        );
        // The message has to name this build, because the fingerprints alone
        // tell an operator nothing they can act on.
        let complaint = CodecError::VersionMismatch {
            found: 1,
            expected: VERSION_FINGERPRINT,
        }
        .to_string();
        assert!(complaint.contains(VERSION), "{complaint}");

        assert_eq!(
            header.validate(body.len() - 1),
            Err(CodecError::BodyLenMismatch {
                declared: body.len(),
                actual: body.len() - 1,
            })
        );
    }

    #[test]
    fn reply_header_carries_metadata_and_error_detail() {
        let ok = ReplyHeader::new(StatusCode::OK, 3, 0).with_metadata(5, Some(600));
        assert!(ok.status().is_ok());
        assert_eq!(ok.generation, 5);
        assert_eq!(ok.time_to_live(), Some(600));
        assert_eq!(ok.result_code(), None);
        assert!(!ok.in_doubt());

        let never = ReplyHeader::new(StatusCode::OK, 3, 0).with_metadata(1, None);
        assert_eq!(never.time_to_live(), None);

        let failed =
            ReplyHeader::new(StatusCode::SERVER, 4, 12).with_server_error(-3, true);
        assert_eq!(failed.status(), StatusCode::SERVER);
        assert_eq!(failed.result_code(), Some(-3));
        assert!(failed.in_doubt());
    }

    #[test]
    fn status_labels_cover_every_code_and_degrade_gracefully() {
        for status in [
            StatusCode::OK,
            StatusCode::SERVER,
            StatusCode::RECORD_NOT_FOUND,
            StatusCode::TIMEOUT,
            StatusCode::CONNECTION,
            StatusCode::UNKNOWN_INSTANCE,
            StatusCode::INVALID_REQUEST,
            StatusCode::FRAME_TOO_LARGE,
            StatusCode::INTERNAL,
            StatusCode::CURSOR_EXPIRED,
            StatusCode::TXN_EXPIRED,
        ] {
            assert_ne!(status.label(), "UNRECOGNIZED");
        }
        // An out-of-range code from a future peer must not be UB.
        assert_eq!(StatusCode(999).label(), "UNRECOGNIZED");
    }

    #[test]
    fn oversized_body_is_refused_rather_than_sent() {
        let huge = WireValue::Blob(vec![0u8; MAX_PAYLOAD_LEN + 1]);
        assert!(matches!(
            encode_body(&huge),
            Err(CodecError::TooLarge { .. })
        ));
    }

    #[test]
    fn garbage_payload_is_malformed_not_a_panic() {
        assert_eq!(
            decode_body::<PutBody>(&[0xFF; 16]),
            Err(CodecError::Malformed)
        );
    }

    #[test]
    fn every_value_shape_round_trips() {
        let pair = || {
            vec![(
                WireValue::Str("k".into()),
                WireValue::Int(1),
            )]
        };
        for value in [
            WireValue::Nil,
            WireValue::Bool(true),
            WireValue::Int(i64::MIN),
            WireValue::Float(-0.0),
            WireValue::Str(String::new()),
            WireValue::Blob(vec![0, 255]),
            WireValue::List(vec![WireValue::Nil]),
            WireValue::Map(pair()),
            WireValue::OrderedMap(pair()),
            WireValue::SortedMap(pair()),
            WireValue::GeoJson("{\"type\":\"Point\",\"coordinates\":[0,0]}".into()),
            WireValue::Hll(vec![1, 2, 3]),
            WireValue::Infinity,
            WireValue::Wildcard,
            WireValue::MultiResult(vec![WireValue::Int(1), WireValue::Nil]),
            WireValue::KeyValueList(pair()),
            WireValue::Unknown {
                particle_type: 42,
                data: vec![9],
            },
        ] {
            let bytes = encode_body(&value).unwrap();
            assert_eq!(decode_body::<WireValue>(&bytes).unwrap(), value, "{value:?}");
        }
    }

    #[test]
    fn result_only_shapes_are_flagged_so_callers_cannot_send_them() {
        // The daemon relies on this to keep its conversion total: there is no
        // core value to build for a MultiResult a caller invented.
        assert!(WireValue::MultiResult(vec![]).is_result_only());
        assert!(WireValue::KeyValueList(vec![]).is_result_only());
        assert!(WireValue::Unknown { particle_type: 1, data: vec![] }.is_result_only());

        for sendable in [
            WireValue::Nil,
            WireValue::Int(1),
            WireValue::Map(vec![]),
            WireValue::OrderedMap(vec![]),
            WireValue::SortedMap(vec![]),
            WireValue::GeoJson("{}".into()),
            WireValue::Hll(vec![]),
            WireValue::Infinity,
            WireValue::Wildcard,
        ] {
            assert!(!sendable.is_result_only(), "{sendable:?}");
        }
    }

    #[test]
    fn defaulted_headers_are_invalid_by_design() {
        // iceoryx2 default-initialises user headers on loan, so a frame whose
        // header was never filled in must be rejected rather than mistaken
        // for a valid one.
        assert!(matches!(
            RequestHeader::default().validate(0),
            Err(CodecError::BadMagic { found: 0 })
        ));
        assert!(matches!(
            ReplyHeader::default().validate(0),
            Err(CodecError::BadMagic { found: 0 })
        ));
    }

    #[test]
    fn service_name_embeds_instance_and_version() {
        assert_eq!(
            rpc_service_name("default"),
            format!("aerospike/default/v{VERSION}/rpc")
        );
        assert_eq!(
            rpc_service_name("analytics"),
            format!("aerospike/analytics/v{VERSION}/rpc")
        );
    }

    #[test]
    fn client_ids_survive_pid_reuse() {
        assert_ne!(ClientId::new(100, 1), ClientId::new(100, 2));
        assert_ne!(ClientId::new(100, 1), ClientId::new(101, 1));
    }

    #[test]
    fn backoff_escalates_spin_then_yield_then_sleep() {
        let policy = Backoff {
            spin_iters: 2,
            yield_iters: 2,
            initial_sleep: Duration::from_micros(1),
            max_sleep: Duration::from_micros(4),
        };
        let mut state = policy.start();
        for _ in 0..8 {
            state.snooze();
        }
        assert_eq!(state.steps(), 8);
        // Sleep must saturate at the ceiling rather than grow unbounded.
        assert_eq!(state.sleep, Duration::from_micros(4));
    }
}
