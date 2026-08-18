// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Decoding a request body, running the operation, and producing the reply.
//!
//! This module knows nothing about iceoryx2. It takes an opcode plus the
//! request payload and returns an [`Outcome`] — the reply header's fields and
//! the reply payload — which keeps every operation testable without a
//! transport, and keeps the transport loop free of database concerns.
//!
//! # What each verb answers
//!
//! | opcode | request body | reply payload |
//! |---|---|---|
//! | `PING` | none | [`PongBody`] |
//! | `PUT` | [`PutBody`] | none |
//! | `GET` | [`GetBody`] | [`RecordBody`], generation and TTL in the header |
//! | `DELETE` | [`TargetBody`] | [`WireValue::Bool`] — whether the record existed |
//! | `TOUCH` | [`TargetBody`] | none |
//! | `EXISTS` | [`TargetBody`] | [`WireValue::Bool`] — whether the record exists |
//! | `ADD`, `APPEND`, `PREPEND` | [`ModifyBody`] | none |
//! | `OPERATE` | [`OperateBody`] | [`RecordBody`] — the results of the operations that produced one |
//! | `BATCH` | [`WireBatchBody`] | [`WireBatchReply`] — one answer per row, in order |
//! | `QUERY` | [`WireQueryBody`] | [`WireQueryPage`] — the first page, plus a cursor if more follow |
//! | `QUERY_NEXT` | [`WireCursorBody`] | [`WireQueryPage`] |
//! | `QUERY_CLOSE` | [`WireCursorBody`] | none |
//! | any failure | — | [`ErrorBody`] |
//!
//! ## Why a bare `WireValue::Bool` for `DELETE` and `EXISTS`
//!
//! Both verbs answer a question rather than fetching a record, and the contract
//! has no body for "one boolean". Rather than overload
//! [`StatusCode::RECORD_NOT_FOUND`] — which would make `exists() == false`
//! arrive as an error and force PHP to catch an exception for an ordinary answer
//! — the reply is `OK` with a single postcard-encoded [`WireValue::Bool`] as its
//! whole payload. It is a contract type, so both ends already agree on it, and
//! adding a named body later is a compatible change to make.
//!
//! A `DELETE` of a record that was not there is therefore `OK` + `false`, not
//! `RECORD_NOT_FOUND`: the client's `delete` reports it that way because after
//! the call the record provably does not exist, which is what was asked for.

use std::sync::Arc;
use std::time::Duration;

use aerospike_core::{Bins, Client, Key, ReadPolicy, WritePolicy};
use aerospike_php_ipc::admin::{
    WireIndexCreateBody, WireIndexDropBody, WireIndexOn, WireInfoBody, WireInfoReply, WireNodes,
    WireNodesBody, WireQueryUdfBody, WireTaskHandle, WireTaskStatusBody, WireTruncateBody,
    WireUdfExecuteBody, WireUdfList, WireUdfListBody, WireUdfRegisterBody, WireUdfRemoveBody,
    WireUdfResult,
};
use aerospike_php_ipc::batch::{WireBatchBody, WireBatchReply};
use aerospike_php_ipc::query::{WireCursorBody, WireQueryBody, WireQueryPage, WireQueryRecord};
use aerospike_php_ipc::security::{
    WireAdminTarget, WireRoleAllowlistBody, WireRoleCreateBody, WireRoleNameBody,
    WireRolePrivilegesBody, WireRoleQuotasBody, WireRoleQueryBody, WireRoles, WireUserCreateBody,
    WireUserNameBody, WireUserPasswordBody, WireUserQueryBody, WireUserRolesBody, WireUsers,
};
use aerospike_php_ipc::txn::{
    WireTxnAbortBody, WireTxnBeginBody, WireTxnBody, WireTxnCommitBody, WireTxnHandle,
};
use aerospike_php_ipc::{
    decode_body, encode_body, opcode, BinSelector, CodecError, ErrorBody, GetBody, ModifyBody,
    OperateBody, PongBody, PutBody, RecordBody, ReplyHeader, StatusCode, Target, TargetBody,
    WirePolicy,
    WireValue, MAX_PAYLOAD_LEN,
};
use serde::Serialize;

use crate::admin;
use crate::batch;
use crate::convert;
use crate::failure::{classify, Failure};
use crate::instance::{Instances, Lookup};
use crate::ops;
use crate::policy::{Capabilities, Defaults};
use crate::query::{self, Cursor, Cursors, PageError};
use crate::security;
use crate::txn::{self, MrtSupport, Transactions};

/// Everything the transport needs to answer one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    status: StatusCode,
    body: Vec<u8>,
    server: Option<(i32, bool)>,
    metadata: Option<(u32, Option<u32>)>,
}

impl Outcome {
    /// A success with no payload — what a write returns.
    #[must_use]
    pub fn ok() -> Outcome {
        Outcome {
            status: StatusCode::OK,
            body: Vec::new(),
            server: None,
            metadata: None,
        }
    }

    /// A success whose payload is `body`, encoded.
    #[must_use]
    pub fn encoded<T: Serialize>(body: &T) -> Outcome {
        match encode_body(body) {
            Ok(bytes) => Outcome {
                status: StatusCode::OK,
                body: bytes,
                server: None,
                metadata: None,
            },
            Err(e) => Outcome::from_codec_error(&e),
        }
    }

    /// A success whose whole answer is one boolean — see the module docs for why
    /// this is a bare [`WireValue::Bool`] rather than a named body.
    #[must_use]
    pub fn flag(value: bool) -> Outcome {
        Outcome::encoded(&WireValue::Bool(value))
    }

    /// A successful read: bins in the payload, generation and TTL in the
    /// header.
    #[must_use]
    pub fn record(body: &RecordBody, generation: u32, ttl: Option<u32>) -> Outcome {
        let mut outcome = Outcome::encoded(body);
        if outcome.status.is_ok() {
            outcome.metadata = Some((generation, ttl));
        }
        outcome
    }

    /// A failure, with its detail in the payload.
    #[must_use]
    pub fn failed(failure: &Failure) -> Outcome {
        // An `ErrorBody` is a single string; encoding it cannot realistically
        // fail, but if it ever did, an empty body still carries the status.
        let body = encode_body(&ErrorBody {
            message: failure.message.clone(),
        })
        .unwrap_or_default();
        Outcome {
            status: failure.status,
            body,
            server: failure.server,
            metadata: None,
        }
    }

    /// Shorthand for a daemon-diagnosed failure.
    #[must_use]
    pub fn error(status: StatusCode, message: impl Into<String>) -> Outcome {
        Outcome::failed(&Failure::new(status, message))
    }

    /// A framing failure, mapped to the status it deserves.
    #[must_use]
    pub fn from_codec_error(err: &CodecError) -> Outcome {
        let status = match err {
            CodecError::TooLarge { .. } => StatusCode::FRAME_TOO_LARGE,
            _ => StatusCode::INVALID_REQUEST,
        };
        Outcome::error(status, err.to_string())
    }

    /// The payload to write into the response slot.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// This outcome's status.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// The typed reply header for sequence number `seq`.
    #[must_use]
    pub fn header(&self, seq: u64) -> ReplyHeader {
        let mut header = ReplyHeader::new(self.status, seq, self.body.len() as u32);
        if let Some((result_code, in_doubt)) = self.server {
            header = header.with_server_error(result_code, in_doubt);
        }
        if let Some((generation, ttl)) = self.metadata {
            header = header.with_metadata(generation, ttl);
        }
        header
    }
}

/// A single-record command, resolved: which cluster, which record, and the
/// per-call policy.
///
/// A struct rather than a tuple because it grew a fourth field — the instance name
/// — and that one is not decoration: a transaction belongs to the cluster it was
/// opened on, so joining one needs to know which cluster the command is for.
struct Aimed {
    client: Arc<Client>,
    key: Key,
    policy: WirePolicy,
    instance: String,
}

/// Which of the three bin-wise modifications a [`ModifyBody`] is for.
///
/// One decode path and one policy path for all three: they differ only in the
/// client method they end in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modify {
    Add,
    Append,
    Prepend,
}

impl Modify {
    const fn verb(self) -> &'static str {
        match self {
            Modify::Add => "ADD",
            Modify::Append => "APPEND",
            Modify::Prepend => "PREPEND",
        }
    }
}

/// Runs operations against the configured clusters.
pub struct Dispatcher {
    instances: Arc<Instances>,
    default_timeout: Duration,
    defaults: Defaults,
    cursors: Cursors,
    default_page_size: u32,
    transactions: Transactions,
}

impl Dispatcher {
    /// Serve `instances`, applying `default_timeout` to requests that declare
    /// no deadline of their own — as the abandon-work deadline *and* as the
    /// total timeout every resolved policy starts from.
    ///
    /// Scan and query paging gets the module defaults; [`with_paging`] replaces
    /// them from configuration.
    ///
    /// [`with_paging`]: Self::with_paging
    #[must_use]
    pub fn new(instances: Arc<Instances>, default_timeout: Duration) -> Dispatcher {
        Dispatcher {
            instances,
            default_timeout,
            defaults: Defaults::with_total_timeout(default_timeout),
            cursors: Cursors::new(query::DEFAULT_CURSOR_IDLE, query::DEFAULT_MAX_CURSORS),
            default_page_size: query::DEFAULT_PAGE_SIZE,
            transactions: Transactions::new(txn::DEFAULT_TXN_IDLE, txn::DEFAULT_MAX_TXNS),
        }
    }

    /// Replace the transaction settings.
    ///
    /// Separate from [`with_paging`](Self::with_paging) because the two bound
    /// different things: a cursor's idle timeout bounds memory, a transaction's
    /// bounds how long other writers wait behind its locks.
    #[must_use]
    pub fn with_transactions(mut self, idle: Duration, max_open: usize) -> Dispatcher {
        self.transactions = Transactions::new(idle, max_open);
        self
    }

    /// The open transactions, for the loop that aborts idle ones.
    #[must_use]
    pub const fn transactions(&self) -> &Transactions {
        &self.transactions
    }

    /// The instances this dispatcher serves, for the same loop: aborting a
    /// transaction needs the client it was opened against.
    #[must_use]
    pub fn instances(&self) -> &Instances {
        &self.instances
    }

    /// Replace the scan and query paging settings.
    ///
    /// A builder step rather than three more constructor arguments: every
    /// existing caller wants the defaults, and a `new` with six positional
    /// parameters would be a worse way to say so.
    #[must_use]
    pub fn with_paging(
        mut self,
        default_page_size: u32,
        cursor_idle: Duration,
        max_cursors: usize,
    ) -> Dispatcher {
        self.default_page_size = default_page_size.clamp(1, query::MAX_PAGE_SIZE);
        self.cursors = Cursors::new(cursor_idle, max_cursors);
        self
    }

    /// The open scan and query cursors, for the loop that expires idle ones.
    #[must_use]
    pub const fn cursors(&self) -> &Cursors {
        &self.cursors
    }

    /// The policies a request with no overrides of its own gets.
    #[must_use]
    pub const fn defaults(&self) -> &Defaults {
        &self.defaults
    }

    /// The instances this daemon serves.
    #[must_use]
    pub fn instance_names(&self) -> Vec<String> {
        self.instances.names()
    }

    /// How long an operation may run, given what the request asked for.
    #[must_use]
    pub fn deadline(&self, timeout_ms: u32) -> Duration {
        if timeout_ms == 0 {
            self.default_timeout
        } else {
            Duration::from_millis(u64::from(timeout_ms))
        }
    }

    /// Answer a `PING`. Synchronous: liveness must not queue behind database
    /// work.
    #[must_use]
    pub fn ping(&self) -> Outcome {
        Outcome::encoded(&PongBody {
            // The contract's version, not this crate's: they are asserted equal
            // at compile time, and the contract is what the extension matches
            // against.
            daemon_version: crate::VERSION.to_owned(),
            instances: self.instances.names(),
        })
    }

    /// Run one request.
    pub async fn execute(&self, opcode: u16, payload: &[u8]) -> Outcome {
        if payload.len() > MAX_PAYLOAD_LEN {
            return Outcome::from_codec_error(&CodecError::TooLarge { len: payload.len() });
        }
        match opcode {
            opcode::PING => self.ping(),
            opcode::PUT => self.put(payload).await,
            opcode::GET => self.get(payload).await,
            opcode::DELETE => self.delete(payload).await,
            opcode::TOUCH => self.touch(payload).await,
            opcode::EXISTS => self.exists(payload).await,
            opcode::ADD => self.modify(payload, Modify::Add).await,
            opcode::APPEND => self.modify(payload, Modify::Append).await,
            opcode::PREPEND => self.modify(payload, Modify::Prepend).await,
            opcode::OPERATE => self.operate(payload).await,
            opcode::BATCH => self.batch(payload).await,
            opcode::QUERY => self.query(payload).await,
            opcode::QUERY_NEXT => self.query_next(payload).await,
            opcode::QUERY_CLOSE => self.query_close(payload),
            opcode::UDF_REGISTER => self.udf_register(payload).await,
            opcode::UDF_REMOVE => self.udf_remove(payload).await,
            opcode::UDF_LIST => self.udf_list(payload).await,
            opcode::UDF_EXECUTE => self.udf_execute(payload).await,
            opcode::QUERY_UDF => self.query_udf(payload).await,
            opcode::INDEX_CREATE => self.index_create(payload).await,
            opcode::INDEX_DROP => self.index_drop(payload).await,
            opcode::TASK_STATUS => self.task_status(payload).await,
            opcode::TRUNCATE => self.truncate(payload).await,
            opcode::INFO => self.info(payload).await,
            opcode::NODES => self.nodes(payload),
            opcode::TXN_BEGIN => self.txn_begin(payload),
            opcode::TXN_COMMIT => self.txn_commit(payload).await,
            opcode::TXN_ABORT => self.txn_abort(payload).await,
            opcode::TXN_STATE => self.txn_state(payload),
            opcode::USER_CREATE => self.user_create(payload).await,
            opcode::USER_CREATE_PKI => self.user_create_pki(payload).await,
            opcode::USER_DROP => self.user_drop(payload).await,
            opcode::USER_PASSWORD => self.user_password(payload).await,
            opcode::USER_QUERY => self.user_query(payload).await,
            opcode::USER_GRANT_ROLES => self.user_roles(payload, true).await,
            opcode::USER_REVOKE_ROLES => self.user_roles(payload, false).await,
            opcode::ROLE_CREATE => self.role_create(payload).await,
            opcode::ROLE_DROP => self.role_drop(payload).await,
            opcode::ROLE_QUERY => self.role_query(payload).await,
            opcode::ROLE_GRANT_PRIVILEGES => self.role_privileges(payload, true).await,
            opcode::ROLE_REVOKE_PRIVILEGES => self.role_privileges(payload, false).await,
            opcode::ROLE_ALLOWLIST => self.role_allowlist(payload).await,
            opcode::ROLE_QUOTAS => self.role_quotas(payload).await,
            other => Outcome::from_codec_error(&CodecError::UnknownOpcode { opcode: other }),
        }
    }

    async fn put(&self, payload: &[u8]) -> Outcome {
        let request: PutBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let client = match self.client(&request.target.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let key = match convert::to_key(
            &request.target.namespace,
            &request.target.set,
            &request.target.key,
        ) {
            Ok(key) => key,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };
        let bins = match convert::to_bins(&request.bins) {
            Ok(bins) => bins,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };
        let policy = match self.write_policy(&client, &request.target.policy, &request.target.instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };

        match client.put(&policy, &key, &bins).await {
            Ok(()) => Outcome::ok(),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    async fn get(&self, payload: &[u8]) -> Outcome {
        let request: GetBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let client = match self.client(&request.target.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let key = match convert::to_key(
            &request.target.namespace,
            &request.target.set,
            &request.target.key,
        ) {
            Ok(key) => key,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };
        let bins = match request.bins {
            BinSelector::All => Bins::All,
            BinSelector::None => Bins::None,
            BinSelector::Only(names) => Bins::Some(names),
        };
        let policy = match self.read_policy(&client, &request.target.policy, &request.target.instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };

        let record = match client.get(&policy, &key, bins).await {
            Ok(record) => record,
            Err(e) => return Outcome::failed(&classify(&e)),
        };

        Self::record_reply(&record)
    }

    /// One record as a reply: bins in the payload, metadata in the header.
    ///
    /// Shared by `GET` and `OPERATE`, which answer with the same shape — an
    /// `operate` that read nothing simply has no bins.
    fn record_reply(record: &aerospike_core::Record) -> Outcome {
        let wire_bins = record
            .bins
            .iter()
            .map(|(name, value)| (name.clone(), convert::from_value(value)))
            .collect();

        // `None` from the client means "never expires", which the reply header
        // encodes as its own sentinel — so a finite TTL must never reach that
        // value, however absurdly large the server's answer was.
        let ttl = record.time_to_live().map(|d| {
            u32::try_from(d.as_secs())
                .unwrap_or(u32::MAX)
                .min(aerospike_php_ipc::TTL_NEVER_EXPIRES - 1)
        });
        Outcome::record(&RecordBody { bins: wire_bins }, record.generation, ttl)
    }

    async fn delete(&self, payload: &[u8]) -> Outcome {
        let Aimed { client, key, policy: wire, instance } = match self.aim(payload) {
            Ok(aimed) => aimed,
            Err(outcome) => return outcome,
        };
        let policy = match self.write_policy(&client, &wire, &instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };
        match client.delete(&policy, &key).await {
            // Whether it was there is the answer, not an error: after this call
            // the record is gone either way.
            Ok(existed) => Outcome::flag(existed),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    async fn touch(&self, payload: &[u8]) -> Outcome {
        let Aimed { client, key, policy: wire, instance } = match self.aim(payload) {
            Ok(aimed) => aimed,
            Err(outcome) => return outcome,
        };
        let policy = match self.write_policy(&client, &wire, &instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };
        match client.touch(&policy, &key).await {
            Ok(()) => Outcome::ok(),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    async fn exists(&self, payload: &[u8]) -> Outcome {
        let Aimed { client, key, policy: wire, instance } = match self.aim(payload) {
            Ok(aimed) => aimed,
            Err(outcome) => return outcome,
        };
        let policy = match self.read_policy(&client, &wire, &instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };
        match client.exists(&policy, &key).await {
            Ok(exists) => Outcome::flag(exists),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    async fn modify(&self, payload: &[u8], op: Modify) -> Outcome {
        let request: ModifyBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if request.bins.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                format!("{} needs at least one bin to modify", op.verb()),
            );
        }
        let bins = match convert::to_bins(&request.bins) {
            Ok(bins) => bins,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };
        let Aimed { client, key, policy: wire, instance } = match self.resolve(&request.target) {
            Ok(aimed) => aimed,
            Err(outcome) => return outcome,
        };
        let policy = match self.write_policy(&client, &wire, &instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };

        let result = match op {
            Modify::Add => client.add(&policy, &key, &bins).await,
            Modify::Append => client.append(&policy, &key, &bins).await,
            Modify::Prepend => client.prepend(&policy, &key, &bins).await,
        };
        match result {
            Ok(()) => Outcome::ok(),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Run a list of operations against one record, in order.
    ///
    /// The client's `operate` takes a write policy whatever the operations are,
    /// so this does too — but a call whose operations only *read* is checked
    /// against the read rules first. Otherwise a caller could set an
    /// `expiration` on a read-only `operate`, have the server ignore it, and be
    /// told nothing; the daemon is the only side that knows the difference,
    /// which is exactly when it should be the one to speak up.
    async fn operate(&self, payload: &[u8]) -> Outcome {
        let request: OperateBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if request.ops.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                "operate needs at least one operation; the server has no answer for a request \
                 that asks nothing"
                    .to_owned(),
            );
        }

        let Aimed { client, key, policy: wire, instance } = match self.resolve(&request.target) {
            Ok(aimed) => aimed,
            Err(outcome) => return outcome,
        };
        if !request.has_write() {
            if let Err(outcome) = self.read_policy(&client, &wire, &instance) {
                return outcome;
            }
        }
        let policy = match self.write_policy(&client, &wire, &instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };

        // The cluster's AEL support is asked once, here, because it is a
        // property of the cluster rather than of any one operation — and only
        // the expression operations consult it.
        let ops = match ops::to_operations(&request.ops, &Capabilities::of(&client)) {
            Ok(ops) => ops,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };

        match client.operate(&policy, &key, &ops).await {
            Ok(record) => Self::record_reply(&record),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Run a batch: many rows, of several kinds, in one round trip per node.
    ///
    /// The reply is `OK` whenever the batch *ran*, even if rows failed: a row's
    /// result code belongs to that row, and a caller that had to catch an
    /// exception to read nine successes would be worse off than one reading ten
    /// answers. Only a failure that stops the batch being sent — an unknown
    /// instance, an unbuildable row, a cluster too old for a row filter — is a
    /// failed reply.
    async fn batch(&self, payload: &[u8]) -> Outcome {
        let request: WireBatchBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if request.rows.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                "a batch needs at least one row; the server has no answer for a request that asks \
                 nothing"
                    .to_owned(),
            );
        }

        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let policy = match self.batch_policy(&client, &request.policy, &request.instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };

        let rows = match batch::to_operations(&request.rows, &Capabilities::of(&client)) {
            Ok(rows) => rows,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };

        match client.batch(&policy, &rows).await {
            Ok(records) => Outcome::encoded(&WireBatchReply {
                rows: records.iter().map(batch::to_result).collect(),
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Start a scan or query and answer with its first page.
    ///
    /// A traversal whose first page finishes it — every query with a selective
    /// filter — registers no cursor at all: the page comes back with `cursor:
    /// None` and the whole thing cost one round trip and no daemon state. Only a
    /// traversal that has more to give is registered, and only then can it need
    /// closing.
    async fn query(&self, payload: &[u8]) -> Outcome {
        let request: WireQueryBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if request.statement.namespace.trim().is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                "a scan or query needs a namespace; every record lives in one".to_owned(),
            );
        }

        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };

        let mut cursor = Cursor::new(request, self.default_page_size);
        let records = match query::page(&client, &self.defaults, &mut cursor).await {
            Ok(records) => records,
            Err(e) => return Self::page_failure(&e),
        };
        self.answer_page(cursor, records)
    }

    /// Answer with the next page of an open traversal.
    ///
    /// An unknown cursor is [`StatusCode::CURSOR_EXPIRED`] and never an empty
    /// page: a scan that stops early without saying so is the failure the whole
    /// paging design exists to prevent, so a caller must be able to tell "there
    /// were no more records" from "your cursor is gone".
    async fn query_next(&self, payload: &[u8]) -> Outcome {
        let request: WireCursorBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let Some(mut cursor) = self.cursors.checkout(request.cursor) else {
            return Outcome::error(
                StatusCode::CURSOR_EXPIRED,
                format!(
                    "scan cursor {} is not open: it was closed, it was idle for more than {:?} \
                     and expired, or the daemon restarted. The traversal has to start again — \
                     the records it had not reached are still there, but where it had got to \
                     is not",
                    request.cursor,
                    self.cursors.idle_timeout()
                ),
            );
        };

        // Resolved through the cursor's own instance, not the caller's word for
        // it: a traversal belongs to the cluster it started on, and a second
        // request naming a different one must not continue it.
        let client = match self.client(cursor.instance()) {
            Ok(client) => client,
            // The cursor goes back: nothing was read, so an instance that is
            // briefly unreachable must not also destroy the traversal. This is
            // the one failure here that is safely retryable.
            Err(outcome) => {
                self.cursors.check_in(request.cursor, cursor);
                return outcome;
            }
        };
        let records = match query::page(&client, &self.defaults, &mut cursor).await {
            Ok(records) => records,
            // The cursor is deliberately *not* checked back in: a page that
            // failed part-way cannot be resumed without risking records read
            // twice or not at all, so the traversal ends here and says so.
            Err(e) => return Self::page_failure(&e),
        };
        self.answer_page(cursor, records)
    }

    /// Abandon a traversal.
    ///
    /// Answering `OK` for a cursor that is not open is not a lie: it says the
    /// traversal is closed, which it is — it finished, or it expired, or this is
    /// a second close. A PHP object's destructor runs whether or not the scan
    /// was read to the end, so a close that had to be conditional would be a
    /// close nobody could call safely.
    fn query_close(&self, payload: &[u8]) -> Outcome {
        let request: WireCursorBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        self.cursors.close(request.cursor);
        Outcome::ok()
    }

    /// Register `cursor` if the traversal continues, and encode the page.
    fn answer_page(&self, cursor: Cursor, records: Vec<WireQueryRecord>) -> Outcome {
        if cursor.is_done() {
            return Outcome::encoded(&WireQueryPage {
                cursor: None,
                records,
            });
        }
        match self.cursors.open(cursor) {
            Ok(id) => Outcome::encoded(&WireQueryPage {
                cursor: Some(id),
                records,
            }),
            // The records were read and are being thrown away, which is the
            // point of failing rather than answering with them and no cursor:
            // that would be a silently truncated scan, and this is a caller
            // whose scan can be retried when the host is less busy.
            Err(open) => Outcome::error(
                StatusCode::INTERNAL,
                format!(
                    "this daemon already has {open} scan or query cursors open, which is its \
                     configured maximum, so this traversal cannot be continued. Cursors are \
                     released when a scan finishes, when it is closed, and when it has been idle \
                     for {:?} — a host that reaches this limit has workers abandoning scans",
                    self.cursors.idle_timeout()
                ),
            ),
        }
    }

    /// The reply a failed page deserves.
    ///
    /// A cluster failure is classified exactly as any other command's is, so a
    /// timeout on page four reads like a timeout on a `get`; a request the daemon
    /// refuses is an `INVALID_REQUEST` naming what was wrong with it.
    fn page_failure(error: &PageError) -> Outcome {
        match error {
            PageError::Invalid(message) => {
                Outcome::error(StatusCode::INVALID_REQUEST, message.clone())
            }
            PageError::Cluster(e) => Outcome::failed(&classify(e)),
        }
    }

    // ===== Cluster management ===============================================

    /// Register a UDF module on every node.
    ///
    /// Answers with a task handle rather than waiting: the command returns once
    /// one node has taken the module, and propagation is the server's business.
    async fn udf_register(&self, payload: &[u8]) -> Outcome {
        let request: WireUdfRegisterBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if let Err(outcome) = Self::require_named("a UDF module", &request.server_path) {
            return outcome;
        }
        if request.source.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                "a UDF module needs a source; registering an empty one would leave the cluster \
                 holding a module with no functions in it"
                    .to_owned(),
            );
        }
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let policy = self.admin_policy(request.timeout_ms);

        match client
            .register_udf(
                &policy,
                &request.source,
                &request.server_path,
                admin::udf_lang(request.language),
            )
            .await
        {
            Ok(_) => Outcome::encoded(&WireTaskHandle::UdfRegister {
                package: request.server_path,
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Remove a UDF module from every node.
    async fn udf_remove(&self, payload: &[u8]) -> Outcome {
        let request: WireUdfRemoveBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if let Err(outcome) = Self::require_named("a UDF module", &request.server_path) {
            return outcome;
        }
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let policy = self.admin_policy(request.timeout_ms);

        match client.remove_udf(&policy, &request.server_path).await {
            Ok(_) => Outcome::encoded(&WireTaskHandle::UdfRemove {
                package: request.server_path,
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// List the UDF modules the cluster holds.
    ///
    /// An info command, because the client has no typed `list_udf` — so the
    /// parsing of the server's record format happens here, once, rather than in
    /// every caller. See [`admin::parse_udf_list`].
    async fn udf_list(&self, payload: &[u8]) -> Outcome {
        let request: WireUdfListBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let policy = self.admin_policy(request.timeout_ms);

        match client.info(&policy, &["udf-list"]).await {
            Ok(response) => Outcome::encoded(&WireUdfList {
                modules: response
                    .get("udf-list")
                    .map(|text| admin::parse_udf_list(text))
                    .unwrap_or_default(),
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Run a UDF against one record.
    ///
    /// A write, whatever the function does: the server takes a write lock on the
    /// record because it cannot know in advance whether the Lua will change it.
    /// So this resolves a write policy, and a read-only UDF still bumps nothing
    /// only because the function itself wrote nothing.
    async fn udf_execute(&self, payload: &[u8]) -> Outcome {
        let request: WireUdfExecuteBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if let Err(outcome) = Self::require_named("a UDF module", &request.package) {
            return outcome;
        }
        if let Err(outcome) = Self::require_named("a UDF function", &request.function) {
            return outcome;
        }

        let Aimed { client, key, policy: wire, instance } = match self.resolve(&request.target) {
            Ok(aimed) => aimed,
            Err(outcome) => return outcome,
        };
        let policy = match self.write_policy(&client, &wire, &instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };
        let args = match admin::to_udf_args(&request.args) {
            Ok(args) => args,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };

        match client
            .execute_udf(
                &policy,
                &key,
                &request.package,
                &request.function,
                args.as_deref(),
            )
            .await
        {
            Ok(value) => Outcome::encoded(&WireUdfResult {
                value: value.as_ref().map(convert::from_value),
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Apply a UDF to every record a statement matches, in the background.
    ///
    /// Returns as soon as the server has accepted the job, with a task handle for
    /// its progress — the work itself may touch the whole set and take as long as
    /// that needs.
    async fn query_udf(&self, payload: &[u8]) -> Outcome {
        let request: WireQueryUdfBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if let Err(outcome) = Self::require_named("a UDF module", &request.package) {
            return outcome;
        }
        if let Err(outcome) = Self::require_named("a UDF function", &request.function) {
            return outcome;
        }
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let policy = match self.write_policy(&client, &request.policy, &request.instance) {
            Ok(policy) => policy,
            Err(outcome) => return outcome,
        };
        let statement = match query::to_statement(&request.statement, &Capabilities::of(&client)) {
            Ok(statement) => statement,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };
        let args = match admin::to_udf_args(&request.args) {
            Ok(args) => args,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };
        // A statement with no filter is a scan, and the server tracks a
        // background scan under a different info command than a background
        // query — so the handle has to record which this was.
        let scan = request.statement.filter.is_none();

        match client
            .query_execute_udf(
                &policy,
                statement,
                &request.package,
                &request.function,
                args.as_deref(),
            )
            .await
        {
            Ok(task) => Outcome::encoded(&WireTaskHandle::Execute {
                task_id: task.task_id(),
                scan,
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Create a secondary index.
    async fn index_create(&self, payload: &[u8]) -> Outcome {
        let request: WireIndexCreateBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if let Err(outcome) = Self::require_named("an index", &request.index_name) {
            return outcome;
        }
        if let Err(outcome) = Self::require_named("a namespace", &request.namespace) {
            return outcome;
        }
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let policy = self.admin_policy(request.timeout_ms);
        let index_type = admin::index_type(request.index_type);
        let collection = admin::collection_index(request.collection);

        let created = match &request.on {
            WireIndexOn::Bin { name, ctx } => {
                if name.trim().is_empty() {
                    return Outcome::error(
                        StatusCode::INVALID_REQUEST,
                        "an index on a bin needs a bin name; \"\" is not one".to_owned(),
                    );
                }
                // A secondary index on a nested bin takes a path, and a path step
                // may carry a filter expression — so the cluster's capabilities are
                // consulted here as they are anywhere else a context is built.
                let context = match ops::to_contexts(ctx, &Capabilities::of(&client)) {
                    Ok(context) => context,
                    Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
                };
                client
                    .create_index_on_bin(
                        &policy,
                        &request.namespace,
                        &request.set,
                        name,
                        &request.index_name,
                        index_type,
                        collection,
                        // An empty path is *no* path: passing `Some(&[])` would
                        // send an empty context field, which is not the same
                        // request as sending none.
                        if context.is_empty() {
                            None
                        } else {
                            Some(context.as_slice())
                        },
                    )
                    .await
            }
            WireIndexOn::Expression(expression) => {
                let expression =
                    match ops::to_expression(expression, &Capabilities::of(&client)) {
                        Ok(expression) => expression,
                        Err(e) => {
                            return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string())
                        }
                    };
                client
                    .create_index_using_expression(
                        &policy,
                        &request.namespace,
                        &request.set,
                        &request.index_name,
                        index_type,
                        collection,
                        &expression,
                    )
                    .await
            }
        };

        match created {
            Ok(_) => Outcome::encoded(&WireTaskHandle::Index {
                namespace: request.namespace,
                index_name: request.index_name,
                dropping: false,
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Drop a secondary index.
    async fn index_drop(&self, payload: &[u8]) -> Outcome {
        let request: WireIndexDropBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if let Err(outcome) = Self::require_named("an index", &request.index_name) {
            return outcome;
        }
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let policy = self.admin_policy(request.timeout_ms);

        match client
            .drop_index(
                &policy,
                &request.namespace,
                &request.set,
                &request.index_name,
            )
            .await
        {
            Ok(_) => Outcome::encoded(&WireTaskHandle::Index {
                namespace: request.namespace,
                index_name: request.index_name,
                // The same question, read the other way round: for a drop, the
                // index being gone is completion.
                dropping: true,
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Report how far a long-running command has got.
    ///
    /// Never blocks. The handle is a description, so the task is rebuilt here and
    /// asked once; waiting is the caller's loop, which is what keeps a wait for a
    /// minutes-long index build from being a request a worker holds open. See
    /// [`crate::admin`].
    async fn task_status(&self, payload: &[u8]) -> Outcome {
        let request: WireTaskStatusBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        // The instance comes from the request rather than the handle: a task
        // belongs to the cluster its command was issued against, and a daemon
        // serving two clusters could otherwise be asked about an index name that
        // exists in both.
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };

        match admin::task_of(&client, &request.handle).query_status().await {
            Ok(status) => Outcome::encoded(&admin::to_status(status)),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Delete every record of a set, or of a whole namespace.
    async fn truncate(&self, payload: &[u8]) -> Outcome {
        let request: WireTruncateBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if let Err(outcome) = Self::require_named("a namespace", &request.namespace) {
            return outcome;
        }
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let policy = self.admin_policy(request.timeout_ms);

        match client
            .truncate(
                &policy,
                &request.namespace,
                &request.set,
                // The client reads 0 as "every record", which is exactly what
                // `None` means here.
                request.before_nanos.unwrap_or(0),
            )
            .await
        {
            Ok(()) => Outcome::ok(),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Send info commands to a node and return its answers in order.
    async fn info(&self, payload: &[u8]) -> Outcome {
        let request: WireInfoBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        if request.commands.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                "an info request needs at least one command; the server has no answer for a \
                 request that asks nothing"
                    .to_owned(),
            );
        }
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        let policy = self.admin_policy(request.timeout_ms);
        let commands: Vec<&str> = request.commands.iter().map(String::as_str).collect();

        let answered = match &request.node {
            // A named node, for the commands whose answer is per node —
            // statistics and latencies — where any-node would be the wrong one.
            Some(name) => match client.get_node(name) {
                Ok(node) => node.info(&policy, &commands).await,
                Err(e) => return Outcome::failed(&classify(&e)),
            },
            None => client.info(&policy, &commands).await,
        };

        match answered {
            // The client answers with an order-preserving map, and the contract
            // carries pairs, so the server's order survives to PHP.
            Ok(values) => Outcome::encoded(&WireInfoReply {
                values: values.into_iter().collect(),
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// The cluster's nodes, as the daemon's tend loop last saw them.
    ///
    /// Synchronous: this is the client's own view, not a question for the
    /// cluster, so there is nothing to await.
    fn nodes(&self, payload: &[u8]) -> Outcome {
        let request: WireNodesBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        Outcome::encoded(&WireNodes {
            nodes: client.nodes().iter().map(admin::to_node).collect(),
        })
    }

    // ===== Multi-record transactions ========================================

    /// Open a transaction.
    ///
    /// Synchronous: a transaction is created locally and the server learns of it
    /// when its first command arrives, so there is nothing to await. The
    /// capability gate is checked here rather than on the first write, because
    /// "this cluster cannot do transactions" is much clearer at the point the
    /// caller asked for one.
    fn txn_begin(&self, payload: &[u8]) -> Outcome {
        let request: WireTxnBeginBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let client = match self.client(&request.instance) {
            Ok(client) => client,
            Err(outcome) => return outcome,
        };
        if let Some(refusal) = MrtSupport::of(&client).refusal() {
            return Outcome::error(StatusCode::INVALID_REQUEST, refusal);
        }

        let transaction = txn::new_txn(request.timeout_ms);
        match self.transactions.open(transaction, request.instance) {
            Ok(id) => Outcome::encoded(&WireTxnHandle { id }),
            // Every open transaction is holding locks, so refusing the next one is
            // the kinder failure.
            Err(open) => Outcome::error(
                StatusCode::INTERNAL,
                format!(
                    "this daemon already has {open} transactions open, which is its configured \
                     maximum. Each one holds record locks until it is committed, aborted, or \
                     aborted for being idle for {:?} — a host that reaches this limit has workers \
                     abandoning transactions",
                    self.transactions.idle_timeout()
                ),
            ),
        }
    }

    /// Commit a transaction, and forget it.
    ///
    /// Every [`WireCommitStatus`] is a success: the transaction committed. The
    /// registry entry goes whatever the status, because a committed transaction
    /// cannot take another command — leaving it registered would let one try.
    ///
    /// [`WireCommitStatus`]: aerospike_php_ipc::txn::WireCommitStatus
    async fn txn_commit(&self, payload: &[u8]) -> Outcome {
        let request: WireTxnCommitBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, transaction) = match self.transaction_of(&request.instance, request.id) {
            Ok(found) => found,
            Err(outcome) => return outcome,
        };
        // Neither policy carries filter text, so AEL support is never consulted —
        // but the resolvers take it, so the "not applicable" value is what they get.
        let verify = match self
            .defaults
            .txn_verify(&request.verify.unwrap_or_default(), &Capabilities::all())
        {
            Ok(policy) => policy,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };
        let roll = match self
            .defaults
            .txn_roll(&request.roll.unwrap_or_default(), &Capabilities::all())
        {
            Ok(policy) => policy,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };
        match client.commit_with_policies(&verify, &roll, &transaction).await {
            Ok(status) => {
                self.transactions.close(request.id);
                Outcome::encoded(&txn::to_commit_status(&status))
            }
            // Deliberately left open on failure: a commit that did not happen can
            // still be aborted, and dropping the entry here would leave the
            // caller holding an id for a transaction the daemon had forgotten
            // while the server still held its locks.
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Abort a transaction, and forget it.
    async fn txn_abort(&self, payload: &[u8]) -> Outcome {
        let request: WireTxnAbortBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, transaction) = match self.transaction_of(&request.instance, request.id) {
            Ok(found) => found,
            Err(outcome) => return outcome,
        };
        let roll = match self
            .defaults
            .txn_roll(&request.roll.unwrap_or_default(), &Capabilities::all())
        {
            Ok(policy) => policy,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };
        match client.abort_with_policy(&roll, &transaction).await {
            Ok(status) => {
                self.transactions.close(request.id);
                Outcome::encoded(&txn::to_abort_status(&status))
            }
            // As with a commit: an abort that failed leaves something to retry.
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Where a transaction has got to. Local state; no round trip to the cluster.
    fn txn_state(&self, payload: &[u8]) -> Outcome {
        let (_, transaction, _) = match self.transaction(payload) {
            Ok(found) => found,
            Err(outcome) => return outcome,
        };
        Outcome::encoded(&txn::to_state(transaction.state()))
    }

    /// Decode a [`WireTxnBody`] and find the transaction it names.
    fn transaction(
        &self,
        payload: &[u8],
    ) -> Result<(Arc<Client>, Arc<aerospike_core::Txn>, i64), Outcome> {
        let request: WireTxnBody =
            decode_body(payload).map_err(|e| Outcome::from_codec_error(&e))?;
        let (client, transaction) = self.transaction_of(&request.instance, request.id)?;
        Ok((client, transaction, request.id))
    }

    /// Find an open transaction by instance and id.
    ///
    /// The instance comes from the request and has to match the one the
    /// transaction was opened on: a transaction's reads are verified and its
    /// writes rolled forward on one cluster, so joining it from another would be a
    /// command that quietly ran outside it.
    ///
    /// Separate from [`transaction`](Self::transaction) because commit and abort
    /// carry their own bodies — a commit needs two policies and an abort one — so
    /// they have already decoded by the time they need the lookup.
    fn transaction_of(
        &self,
        instance: &str,
        id: i64,
    ) -> Result<(Arc<Client>, Arc<aerospike_core::Txn>), Outcome> {
        let client = self.client(instance)?;
        let transaction = self
            .transactions
            .get(id, instance)
            .ok_or_else(|| self.txn_gone(id, instance))?;
        Ok((client, transaction))
    }

    /// The reply for a transaction that is not open.
    fn txn_gone(&self, id: i64, instance: &str) -> Outcome {
        Outcome::error(
            StatusCode::TXN_EXPIRED,
            format!(
                "transaction {id} is not open on instance '{instance}': it was committed, it was \
                 aborted, it was idle for more than {:?} and was aborted for you, or it belongs to \
                 a different instance. Its writes are not applied, and the work has to start again",
                self.transactions.idle_timeout()
            ),
        )
    }

    // ===== Users, roles and privileges ======================================

    /// Create a user with a password.
    async fn user_create(&self, payload: &[u8]) -> Outcome {
        let request: WireUserCreateBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a user", &request.user) {
            return outcome;
        }
        // The server would refuse an empty password too, with a code that says
        // nothing about which field it meant.
        if request.password.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                "a user needs a password; for a user the client certificate identifies, create a \
                 PKI user instead"
                    .to_owned(),
            );
        }

        Self::done(
            client
                .create_user(
                    &policy,
                    &request.user,
                    &request.password,
                    &security::as_str_slice(&request.roles),
                )
                .await,
        )
    }

    /// Create a user the client certificate identifies, with no password.
    async fn user_create_pki(&self, payload: &[u8]) -> Outcome {
        let request: WireUserRolesBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a user", &request.user) {
            return outcome;
        }

        Self::done(
            client
                .create_pki_user(
                    &policy,
                    &request.user,
                    &security::as_str_slice(&request.roles),
                )
                .await,
        )
    }

    /// Remove a user.
    async fn user_drop(&self, payload: &[u8]) -> Outcome {
        let request: WireUserNameBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a user", &request.user) {
            return outcome;
        }
        Self::done(client.drop_user(&policy, &request.user).await)
    }

    /// Change a user's password.
    async fn user_password(&self, payload: &[u8]) -> Outcome {
        let request: WireUserPasswordBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a user", &request.user) {
            return outcome;
        }
        if request.password.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                "a password change needs a new password; \"\" is not one".to_owned(),
            );
        }
        Self::done(
            client
                .change_password(&policy, &request.user, &request.password)
                .await,
        )
    }

    /// Describe one user, or every user.
    async fn user_query(&self, payload: &[u8]) -> Outcome {
        let request: WireUserQueryBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };

        match client.query_users(&policy, request.user.as_deref()).await {
            Ok(users) => Outcome::encoded(&WireUsers {
                users: users.iter().map(security::from_user).collect(),
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Assign roles to a user, or take them away.
    async fn user_roles(&self, payload: &[u8], grant: bool) -> Outcome {
        let request: WireUserRolesBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a user", &request.user) {
            return outcome;
        }
        // Granting nothing is a request the server cannot act on, and almost
        // always a list that was computed and came out empty.
        if request.roles.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                format!(
                    "{} needs at least one role",
                    if grant { "granting roles" } else { "revoking roles" }
                ),
            );
        }

        let roles = security::as_str_slice(&request.roles);
        Self::done(if grant {
            client.grant_roles(&policy, &request.user, &roles).await
        } else {
            client.revoke_roles(&policy, &request.user, &roles).await
        })
    }

    /// Create a role.
    async fn role_create(&self, payload: &[u8]) -> Outcome {
        let request: WireRoleCreateBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a role", &request.role) {
            return outcome;
        }
        // A role with no privileges permits nothing, which the server accepts and
        // nobody means: privileges can be granted later, but creating one empty is
        // overwhelmingly a mistake worth naming.
        if request.privileges.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                "a role needs at least one privilege; one with none permits nothing".to_owned(),
            );
        }
        let privileges = match security::to_privileges(&request.privileges) {
            Ok(privileges) => privileges,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };

        Self::done(
            client
                .create_role(
                    &policy,
                    &request.role,
                    &privileges,
                    &security::as_str_slice(&request.allowlist),
                    request.read_quota,
                    request.write_quota,
                )
                .await,
        )
    }

    /// Remove a role.
    async fn role_drop(&self, payload: &[u8]) -> Outcome {
        let request: WireRoleNameBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a role", &request.role) {
            return outcome;
        }
        Self::done(client.drop_role(&policy, &request.role).await)
    }

    /// Describe one role, or every role.
    async fn role_query(&self, payload: &[u8]) -> Outcome {
        let request: WireRoleQueryBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };

        match client.query_roles(&policy, request.role.as_deref()).await {
            Ok(roles) => Outcome::encoded(&WireRoles {
                roles: roles.iter().map(security::from_role).collect(),
            }),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// Add privileges to a role, or take them away.
    async fn role_privileges(&self, payload: &[u8], grant: bool) -> Outcome {
        let request: WireRolePrivilegesBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a role", &request.role) {
            return outcome;
        }
        if request.privileges.is_empty() {
            return Outcome::error(
                StatusCode::INVALID_REQUEST,
                format!(
                    "{} needs at least one privilege",
                    if grant { "granting" } else { "revoking" }
                ),
            );
        }
        let privileges = match security::to_privileges(&request.privileges) {
            Ok(privileges) => privileges,
            Err(e) => return Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()),
        };

        Self::done(if grant {
            client
                .grant_privileges(&policy, &request.role, &privileges)
                .await
        } else {
            client
                .revoke_privileges(&policy, &request.role, &privileges)
                .await
        })
    }

    /// Replace a role's address allowlist.
    async fn role_allowlist(&self, payload: &[u8]) -> Outcome {
        let request: WireRoleAllowlistBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a role", &request.role) {
            return outcome;
        }
        // An empty list is *not* refused here: clearing the allowlist is how a
        // restriction is removed, and there is no other way to say it.
        Self::done(
            client
                .set_allowlist(
                    &policy,
                    &request.role,
                    &security::as_str_slice(&request.allowlist),
                )
                .await,
        )
    }

    /// Replace a role's rate quotas.
    async fn role_quotas(&self, payload: &[u8]) -> Outcome {
        let request: WireRoleQuotasBody = match decode_body(payload) {
            Ok(body) => body,
            Err(e) => return Outcome::from_codec_error(&e),
        };
        let (client, policy) = match self.admin(&request.target) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        if let Err(outcome) = Self::require_named("a role", &request.role) {
            return outcome;
        }
        // Zero is not refused either: it lifts the quota.
        Self::done(
            client
                .set_quotas(
                    &policy,
                    &request.role,
                    request.read_quota,
                    request.write_quota,
                )
                .await,
        )
    }

    /// Resolve the client and admin policy every security verb needs.
    ///
    /// One helper because all fourteen take the same two things, and none of them
    /// takes anything else — an `AdminPolicy` is a timeout and nothing more.
    fn admin(
        &self,
        target: &WireAdminTarget,
    ) -> Result<(Arc<Client>, aerospike_core::AdminPolicy), Outcome> {
        let client = self.client(&target.instance)?;
        Ok((client, self.admin_policy(target.timeout_ms)))
    }

    /// The reply for a command whose whole answer is "it worked".
    ///
    /// Ten of the fourteen security verbs answer with nothing, so this is where
    /// their one line of error handling lives instead of ten copies.
    fn done(result: aerospike_core::Result<()>) -> Outcome {
        match result {
            Ok(()) => Outcome::ok(),
            Err(e) => Outcome::failed(&classify(&e)),
        }
    }

    /// The admin policy for an info-based command.
    fn admin_policy(&self, timeout_ms: Option<u32>) -> aerospike_core::AdminPolicy {
        admin::admin_policy(timeout_ms, self.default_timeout)
    }

    /// Refuse a blank name before the cluster is asked.
    ///
    /// The server answers a blank name with a parameter error naming neither the
    /// command nor the field, and a blank name is nearly always a variable that
    /// was empty.
    fn require_named(what: &str, name: &str) -> Result<(), Outcome> {
        if name.trim().is_empty() {
            return Err(Outcome::error(
                StatusCode::INVALID_REQUEST,
                format!("{what} needs a name; \"\" is not one"),
            ));
        }
        Ok(())
    }

    /// Decode a [`TargetBody`] and resolve the record it names — the whole
    /// request for `DELETE`, `TOUCH` and `EXISTS`.
    fn aim(&self, payload: &[u8]) -> Result<Aimed, Outcome> {
        let body: TargetBody =
            decode_body(payload).map_err(|e| Outcome::from_codec_error(&e))?;
        self.resolve(&body.target)
    }

    /// The client, key, policy and instance one [`Target`] stands for.
    fn resolve(&self, target: &Target) -> Result<Aimed, Outcome> {
        let client = self.client(&target.instance)?;
        let key = convert::to_key(&target.namespace, &target.set, &target.key)
            .map_err(|e| Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()))?;
        Ok(Aimed {
            client,
            key,
            policy: target.policy.clone(),
            instance: target.instance.clone(),
        })
    }

    /// A read policy for this request, or the reply naming what was wrong with
    /// the one it asked for.
    fn read_policy(
        &self,
        client: &Client,
        wire: &WirePolicy,
        instance: &str,
    ) -> Result<ReadPolicy, Outcome> {
        let mut policy = self
            .defaults
            .for_read(client, wire)
            .map_err(|e| Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()))?;
        policy.base_policy.txn = self.joined_transaction(wire, instance)?;
        Ok(policy)
    }

    /// A batch policy for this request, or the reply naming what was wrong with
    /// the one it asked for.
    fn batch_policy(
        &self,
        client: &Client,
        wire: &WirePolicy,
        instance: &str,
    ) -> Result<aerospike_core::BatchPolicy, Outcome> {
        let mut policy = self
            .defaults
            .batch(wire, &Capabilities::of(client))
            .map_err(|e| Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()))?;
        policy.base_policy.txn = self.joined_transaction(wire, instance)?;
        Ok(policy)
    }

    /// A write policy for this request, or the reply naming what was wrong with
    /// the one it asked for.
    fn write_policy(
        &self,
        client: &Client,
        wire: &WirePolicy,
        instance: &str,
    ) -> Result<WritePolicy, Outcome> {
        let mut policy = self
            .defaults
            .for_write(client, wire)
            .map_err(|e| Outcome::error(StatusCode::INVALID_REQUEST, e.to_string()))?;
        policy.base_policy.txn = self.joined_transaction(wire, instance)?;
        Ok(policy)
    }

    /// The transaction this request asked to run inside, if any.
    ///
    /// Resolved here, in the three policy helpers, rather than at each command
    /// site: every single-record verb and `batch` already goes through one of
    /// them, so a transaction reaches all of them at once and none of them can
    /// forget it.
    ///
    /// A transaction opened on another instance is *not* found, which is why the
    /// instance travels this far down: joining it would be a command that quietly
    /// ran outside the transaction it named.
    fn joined_transaction(
        &self,
        wire: &WirePolicy,
        instance: &str,
    ) -> Result<Option<Arc<aerospike_core::Txn>>, Outcome> {
        let Some(id) = wire.txn else {
            return Ok(None);
        };
        match self.transactions.get(id, instance) {
            Some(transaction) => Ok(Some(transaction)),
            None => Err(self.txn_gone(id, instance)),
        }
    }

    /// Resolve the instance a request named, or the reply explaining why not.
    fn client(&self, name: &str) -> Result<Arc<aerospike_core::Client>, Outcome> {
        match self.instances.resolve(name) {
            Lookup::Ready(client) => Ok(client.clone()),
            Lookup::Unavailable(reason) => Err(Outcome::error(
                StatusCode::CONNECTION,
                format!("instance '{name}' is configured but unavailable: {reason}"),
            )),
            Lookup::Unknown => Err(Outcome::error(
                StatusCode::UNKNOWN_INSTANCE,
                format!(
                    "no instance named '{name}'; this daemon serves: {}",
                    self.instances.names().join(", ")
                ),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::{flags, WireKey, WireValue, TTL_NEVER_EXPIRES};

    fn dispatcher() -> Dispatcher {
        Dispatcher::new(
            Arc::new(Instances::from_clients(Vec::new())),
            Duration::from_secs(1),
        )
    }

    #[test]
    fn a_write_reply_is_a_bare_ok() {
        let outcome = Outcome::ok();
        let header = outcome.header(11);
        assert!(header.status().is_ok());
        assert_eq!(header.seq, 11);
        assert_eq!(header.body_len, 0);
        assert_eq!(header.result_code(), None);
        assert!(header.validate(outcome.body().len()).is_ok());
    }

    #[test]
    fn a_read_reply_carries_metadata_in_the_header() {
        let body = RecordBody {
            bins: vec![("n".into(), WireValue::Int(1))],
        };
        let outcome = Outcome::record(&body, 4, Some(120));
        let header = outcome.header(2);
        assert_eq!(header.generation, 4);
        assert_eq!(header.time_to_live(), Some(120));
        assert!(header.validate(outcome.body().len()).is_ok());
        assert_eq!(
            aerospike_php_ipc::decode_body::<RecordBody>(outcome.body()).unwrap(),
            body
        );

        let never = Outcome::record(&body, 1, None);
        assert_eq!(never.header(0).ttl, TTL_NEVER_EXPIRES);
    }

    #[test]
    fn a_failure_reply_carries_its_message_and_server_detail() {
        let outcome = Outcome::failed(&Failure {
            status: StatusCode::SERVER,
            message: "no can do".into(),
            server: Some((5, true)),
        });
        let header = outcome.header(3);
        assert_eq!(header.status(), StatusCode::SERVER);
        assert_eq!(header.result_code(), Some(5));
        assert!(header.in_doubt());
        assert_eq!(header.flags & flags::HAS_RESULT_CODE, flags::HAS_RESULT_CODE);
        assert_eq!(
            aerospike_php_ipc::decode_body::<ErrorBody>(outcome.body())
                .unwrap()
                .message,
            "no can do"
        );
    }

    #[test]
    fn ping_reports_this_build_and_its_instances() {
        let outcome = dispatcher().ping();
        assert!(outcome.status().is_ok());
        let pong: PongBody = aerospike_php_ipc::decode_body(outcome.body()).unwrap();
        assert_eq!(pong.daemon_version, aerospike_php_ipc::VERSION);
        assert!(pong.instances.is_empty());
    }

    #[tokio::test]
    async fn an_unknown_opcode_is_an_invalid_request() {
        let outcome = dispatcher().execute(4242, &[]).await;
        assert_eq!(outcome.status(), StatusCode::INVALID_REQUEST);
        let body: ErrorBody = aerospike_php_ipc::decode_body(outcome.body()).unwrap();
        assert!(body.message.contains("4242"), "{}", body.message);
    }

    #[tokio::test]
    async fn a_malformed_body_is_an_invalid_request() {
        let outcome = dispatcher().execute(opcode::PUT, &[0xFF; 8]).await;
        assert_eq!(outcome.status(), StatusCode::INVALID_REQUEST);
        let outcome = dispatcher().execute(opcode::GET, &[]).await;
        assert_eq!(outcome.status(), StatusCode::INVALID_REQUEST);
    }

    #[tokio::test]
    async fn naming_an_instance_that_does_not_exist_says_which_ones_do() {
        let body = encode_body(&GetBody {
            target: Target {
                instance: "nope".into(),
                namespace: "test".into(),
                set: "s".into(),
                key: WireKey::Int(1),
                policy: WirePolicy::default(),
            },
            bins: BinSelector::All,
        })
        .unwrap();
        let outcome = dispatcher().execute(opcode::GET, &body).await;
        assert_eq!(outcome.status(), StatusCode::UNKNOWN_INSTANCE);
        let error: ErrorBody = aerospike_php_ipc::decode_body(outcome.body()).unwrap();
        assert!(error.message.contains("nope"), "{}", error.message);

        // A PUT for an unknown instance must not be attempted either.
        let body = encode_body(&PutBody {
            target: Target {
                instance: "nope".into(),
                namespace: "test".into(),
                set: "s".into(),
                key: WireKey::Int(1),
                policy: WirePolicy::default(),
            },
            bins: vec![("b".into(), WireValue::Int(1))],
        })
        .unwrap();
        let outcome = dispatcher().execute(opcode::PUT, &body).await;
        assert_eq!(outcome.status(), StatusCode::UNKNOWN_INSTANCE);
    }

    #[tokio::test]
    async fn every_phase_two_verb_resolves_its_instance_before_running() {
        // The `Target` shape is shared, so one wrong instance name must be
        // reported the same way by every verb that takes one.
        let target = Target {
            instance: "nope".into(),
            namespace: "test".into(),
            set: "s".into(),
            key: WireKey::Int(1),
            policy: WirePolicy::default(),
        };
        let body = encode_body(&TargetBody {
            target: target.clone(),
        })
        .unwrap();
        for op in [opcode::DELETE, opcode::TOUCH, opcode::EXISTS] {
            let outcome = dispatcher().execute(op, &body).await;
            assert_eq!(outcome.status(), StatusCode::UNKNOWN_INSTANCE, "opcode {op}");
        }

        let body = encode_body(&ModifyBody {
            target,
            bins: vec![("b".into(), WireValue::Int(1))],
        })
        .unwrap();
        for op in [opcode::ADD, opcode::APPEND, opcode::PREPEND] {
            let outcome = dispatcher().execute(op, &body).await;
            assert_eq!(outcome.status(), StatusCode::UNKNOWN_INSTANCE, "opcode {op}");
        }
    }

    #[tokio::test]
    async fn a_modification_with_no_bins_is_refused_before_the_cluster_is_asked() {
        let body = encode_body(&ModifyBody {
            target: Target {
                instance: "nope".into(),
                namespace: "test".into(),
                set: "s".into(),
                key: WireKey::Int(1),
                policy: WirePolicy::default(),
            },
            bins: Vec::new(),
        })
        .unwrap();
        let outcome = dispatcher().execute(opcode::APPEND, &body).await;
        assert_eq!(outcome.status(), StatusCode::INVALID_REQUEST);
        let error: ErrorBody = aerospike_php_ipc::decode_body(outcome.body()).unwrap();
        assert!(error.message.contains("APPEND"), "{}", error.message);
    }

    #[test]
    fn a_boolean_reply_is_a_bare_wire_bool() {
        for value in [true, false] {
            let outcome = Outcome::flag(value);
            assert!(outcome.status().is_ok());
            assert_eq!(
                aerospike_php_ipc::decode_body::<WireValue>(outcome.body()).unwrap(),
                WireValue::Bool(value)
            );
            assert!(outcome.header(1).validate(outcome.body().len()).is_ok());
        }
    }

    #[test]
    fn a_request_without_a_timeout_gets_the_configured_default() {
        let dispatcher = dispatcher();
        assert_eq!(dispatcher.deadline(0), Duration::from_secs(1));
        assert_eq!(dispatcher.deadline(250), Duration::from_millis(250));
    }

    fn query_body(namespace: &str) -> Vec<u8> {
        use aerospike_php_ipc::query::{WirePartitions, WireStatement};
        encode_body(&WireQueryBody {
            instance: "nope".into(),
            policy: WirePolicy::default(),
            statement: WireStatement {
                namespace: namespace.into(),
                set: "users".into(),
                bins: BinSelector::All,
                filter: None,
            },
            partitions: WirePartitions::All,
            page_size: 10,
            max_records: 0,
            records_per_second: 0,
            include_bin_data: true,
        })
        .unwrap()
    }

    #[tokio::test]
    async fn a_query_resolves_its_instance_and_its_namespace_before_running() {
        let outcome = dispatcher().execute(opcode::QUERY, &query_body("test")).await;
        assert_eq!(outcome.status(), StatusCode::UNKNOWN_INSTANCE);

        // A blank namespace is refused before the cluster is even looked up:
        // there is no record anywhere that is not in a namespace, and the
        // server's own complaint would not say which argument was empty.
        let outcome = dispatcher().execute(opcode::QUERY, &query_body("  ")).await;
        assert_eq!(outcome.status(), StatusCode::INVALID_REQUEST);
        let error: ErrorBody = aerospike_php_ipc::decode_body(outcome.body()).unwrap();
        assert!(error.message.contains("namespace"), "{}", error.message);
    }

    /// The distinction the whole paging design rests on: a cursor that is gone
    /// must be an error a caller can see, never an empty page that reads like
    /// the end of the scan.
    #[tokio::test]
    async fn an_unknown_cursor_is_expired_and_not_an_empty_page() {
        let body = encode_body(&WireCursorBody { cursor: 12_345 }).unwrap();
        let outcome = dispatcher().execute(opcode::QUERY_NEXT, &body).await;
        assert_eq!(outcome.status(), StatusCode::CURSOR_EXPIRED);

        let error: ErrorBody = aerospike_php_ipc::decode_body(outcome.body()).unwrap();
        assert!(error.message.contains("12345"), "{}", error.message);
        // The message has to say what to do about it, because there is exactly
        // one thing that can be done: start again.
        assert!(error.message.contains("start again"), "{}", error.message);
    }

    /// A PHP destructor runs whether or not the scan was read to the end, so
    /// closing something already closed cannot be a failure.
    #[tokio::test]
    async fn closing_a_cursor_that_is_not_open_succeeds() {
        let body = encode_body(&WireCursorBody { cursor: 999 }).unwrap();
        for _ in 0..2 {
            let outcome = dispatcher().execute(opcode::QUERY_CLOSE, &body).await;
            assert!(outcome.status().is_ok());
            assert_eq!(outcome.body().len(), 0);
        }
    }

    #[test]
    fn paging_settings_come_from_configuration_and_are_bounded() {
        let configured = dispatcher().with_paging(50, Duration::from_secs(5), 4);
        assert_eq!(configured.default_page_size, 50);
        assert_eq!(configured.cursors().idle_timeout(), Duration::from_secs(5));
        assert_eq!(configured.cursors().max_open(), 4);

        // A page is materialised here before it is sent, so a configured size is
        // clamped rather than trusted.
        let absurd = dispatcher().with_paging(u32::MAX, Duration::from_secs(5), 4);
        assert_eq!(absurd.default_page_size, crate::query::MAX_PAGE_SIZE);
        let zero = dispatcher().with_paging(0, Duration::from_secs(5), 4);
        assert_eq!(zero.default_page_size, 1);

        // The defaults are the module's, so a daemon with no configuration for
        // this still pages.
        assert_eq!(dispatcher().default_page_size, crate::query::DEFAULT_PAGE_SIZE);
        assert_eq!(
            dispatcher().cursors().idle_timeout(),
            crate::query::DEFAULT_CURSOR_IDLE
        );
    }

    #[test]
    fn an_oversized_payload_is_refused_as_frame_too_large() {
        let outcome = Outcome::from_codec_error(&CodecError::TooLarge {
            len: MAX_PAYLOAD_LEN + 1,
        });
        assert_eq!(outcome.status(), StatusCode::FRAME_TOO_LARGE);
    }
}
