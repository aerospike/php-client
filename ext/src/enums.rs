// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The policy enumerations, as real PHP enums.
//!
//! Every one of these is an `enum` in `aerospike-core` too, and a PHP `enum` is
//! the closest thing PHP has: a closed set of named instances the engine
//! type-checks at the call boundary. Passing
//! `Aerospike\Replica::PreferRack` where a `?Replica` is declared either
//! compiles or throws a `TypeError` before this extension sees it — which is
//! the whole reason these are not strings.
//!
//! # Why the cases are named as they are
//!
//! Case names mirror the Rust variant names exactly (`MasterProles`, not
//! `MASTER_PROLES`), so a policy written against the Rust client reads the same
//! in PHP. Each case is *backed* by the upper-snake name the other Aerospike
//! clients and the daemon's configuration file use, so
//! `Replica::from('MASTER_PROLES')` works when a value arrives as
//! configuration, and `->value` produces something an operator recognises.
//!
//! ```php
//! $policy = new Aerospike\ReadPolicy(replica: Aerospike\Replica::PreferRack);
//!
//! Aerospike\Replica::PreferRack->value;          // "PREFER_RACK"
//! Aerospike\Replica::from('MASTER');              // Replica::Master
//! Aerospike\Replica::tryFrom('nonsense');         // null
//! ```

use aerospike_php_ipc::policy::{
    WireCommitLevel, WireGenerationPolicy, WireReadModeAp, WireReadModeSc, WireRecordExistsAction,
    WireReplica,
};
use aerospike_php_ipc::admin::{WireIndexType, WireTaskStatus, WireUdfLang};
use aerospike_php_ipc::security::WirePrivilegeCode;
use aerospike_php_ipc::query::WireCollectionIndex;
use aerospike_php_ipc::txn::{WireAbortStatus, WireCommitStatus, WireTxnState};
use aerospike_php_ipc::op::{
    WireBitOverflow, WireBitResize, WireBitWriteMode, WireHllWriteMode, WireListOrder,
    WireListReturn, WireListReturnKind, WireMapOrder, WireMapReturn, WireMapReturnKind,
    WireMapWriteMode,
};
use aerospike_php_ipc::exp::{WireExpType, WireLoopVarPart, WireStringNumericType};
use aerospike_php_ipc::StatusCode;
use ext_php_rs::boxed::ZBox;
use ext_php_rs::class::RegisteredClass;
use ext_php_rs::convert::{FromZval, IntoZendObject, IntoZval};
use ext_php_rs::enum_::RegisteredEnum;
use ext_php_rs::flags::DataType;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

use crate::error::CLIENT_STATUS_LABEL;

/// A PHP enum case on its way out to PHP, with its reference count handled
/// correctly.
///
/// # Why this exists
///
/// `ext_php_rs` 0.15.15 implements `IntoZval` for every `RegisteredEnum` as
/// "wrap the case in a `ZBox`, decrement it, then set it on the zval". That is
/// right for an object the `ZBox` owns — the decrement cancels the increment
/// `Zval::set_object` performs — but an enum case is **not** owned: PHP's
/// `zend_enum_get_case` returns the one immortal instance held by the enum's
/// own constant table without adding a reference. So the decrement steals the
/// class's reference, and the case is freed the moment the first zval holding
/// it dies. The second read of the same case then reads freed memory.
///
/// That is not a theory: it segfaults with `EXC_BAD_ACCESS` inside
/// `ZEND_FETCH_OBJ_R`, reading a pointer whose bytes are recycled string data,
/// as soon as one exception's `getStatus()` is used twice in one expression.
///
/// So enum cases leave this extension through this wrapper instead, which does
/// only what is correct: `set_object` takes the one reference the zval needs,
/// and the `ZBox` is defused rather than allowed to decrement. The PHP-visible
/// type is unchanged — [`IntoZval::TYPE`] still names the enum class, so a
/// method returning `Case<Replica>` still has the return type
/// `Aerospike\Replica`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Case<T>(pub T);

impl<T: RegisteredEnum + RegisteredClass> IntoZval for Case<T> {
    const TYPE: DataType = DataType::Object(Some(T::CLASS_NAME));
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _persistent: bool) -> ext_php_rs::error::Result<()> {
        let case = self.0.into_zend_object()?;
        // `set_object` increments, which is exactly the reference this zval
        // holds. `into_raw` stops the `ZBox` from decrementing a reference it
        // never owned.
        zv.set_object(ZBox::into_raw(case));
        Ok(())
    }
}

impl<'a, T: RegisteredEnum + RegisteredClass> FromZval<'a> for Case<T> {
    const TYPE: DataType = DataType::Object(Some(T::CLASS_NAME));

    fn from_zval(zval: &'a Zval) -> Option<Case<T>> {
        // Reading a case is a property read, which the upstream implementation
        // gets right; only the outbound direction needed correcting.
        T::from_zval(zval).map(Case)
    }
}

/// Which node a command prefers.
///
/// Inert on a write, which always goes to the partition master. Accepting it
/// there anyway is deliberate: one policy object is often reused across verbs.
#[php_enum]
#[php(name = "Aerospike\\Replica")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replica {
    /// The partition master.
    #[php(value = "MASTER")]
    Master,
    /// Master and proles, chosen at random.
    #[php(value = "MASTER_PROLES")]
    MasterProles,
    /// Any node at random.
    #[php(value = "RANDOM")]
    Random,
    /// Master first, then proles in sequence on failure.
    #[php(value = "SEQUENCE")]
    Sequence,
    /// A node on the client's own rack first, where rack awareness is
    /// configured.
    #[php(value = "PREFER_RACK")]
    PreferRack,
}

impl Replica {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireReplica {
        match self {
            Replica::Master => WireReplica::Master,
            Replica::MasterProles => WireReplica::MasterProles,
            Replica::Random => WireReplica::Random,
            Replica::Sequence => WireReplica::Sequence,
            Replica::PreferRack => WireReplica::PreferRack,
        }
    }
}

/// Read consistency in an availability-mode (AP) namespace.
#[php_enum]
#[php(name = "Aerospike\\ReadModeAP")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadModeAp {
    /// One replica.
    #[php(value = "ONE")]
    One,
    /// All replicas, so a conflict is detected.
    #[php(value = "ALL")]
    All,
}

impl ReadModeAp {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireReadModeAp {
        match self {
            ReadModeAp::One => WireReadModeAp::One,
            ReadModeAp::All => WireReadModeAp::All,
        }
    }
}

/// Read consistency in a strong-consistency (SC) namespace.
#[php_enum]
#[php(name = "Aerospike\\ReadModeSC")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadModeSc {
    /// Session consistency: never read older than this client has written.
    #[php(value = "SESSION")]
    Session,
    /// Linearizable across all clients.
    #[php(value = "LINEARIZE")]
    Linearize,
    /// Allow a possibly stale read from a replica.
    #[php(value = "ALLOW_REPLICA")]
    AllowReplica,
    /// Allow a read from an unavailable partition.
    #[php(value = "ALLOW_UNAVAILABLE")]
    AllowUnavailable,
}

impl ReadModeSc {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireReadModeSc {
        match self {
            ReadModeSc::Session => WireReadModeSc::Session,
            ReadModeSc::Linearize => WireReadModeSc::Linearize,
            ReadModeSc::AllowReplica => WireReadModeSc::AllowReplica,
            ReadModeSc::AllowUnavailable => WireReadModeSc::AllowUnavailable,
        }
    }
}

/// What a write does about the record already existing, or not.
///
/// `put`, `add`, `append` and `prepend` all default to `Update`. Setting this
/// is how a write becomes conditional: `CreateOnly` fails with result code 5
/// when the record is there, `UpdateOnly` with result code 2 when it is not.
#[php_enum]
#[php(name = "Aerospike\\RecordExistsAction")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordExistsAction {
    /// Create or update; merge bins.
    #[php(value = "UPDATE")]
    Update,
    /// Update only; fail if absent.
    #[php(value = "UPDATE_ONLY")]
    UpdateOnly,
    /// Create or replace; drop the bins not named.
    #[php(value = "REPLACE")]
    Replace,
    /// Replace only; fail if absent.
    #[php(value = "REPLACE_ONLY")]
    ReplaceOnly,
    /// Create only; fail if present.
    #[php(value = "CREATE_ONLY")]
    CreateOnly,
}

impl RecordExistsAction {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireRecordExistsAction {
        match self {
            RecordExistsAction::Update => WireRecordExistsAction::Update,
            RecordExistsAction::UpdateOnly => WireRecordExistsAction::UpdateOnly,
            RecordExistsAction::Replace => WireRecordExistsAction::Replace,
            RecordExistsAction::ReplaceOnly => WireRecordExistsAction::ReplaceOnly,
            RecordExistsAction::CreateOnly => WireRecordExistsAction::CreateOnly,
        }
    }
}

/// Whether a write is guarded by the record's generation.
///
/// Only meaningful together with a generation: the guard says how to compare,
/// and the generation says what to compare against.
#[php_enum]
#[php(name = "Aerospike\\GenerationPolicy")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationPolicy {
    /// No guard.
    #[php(value = "NONE")]
    None,
    /// Write only if the generation matches exactly.
    #[php(value = "EXPECT_GEN_EQUAL")]
    ExpectGenEqual,
    /// Write only if the record's generation is greater.
    #[php(value = "EXPECT_GEN_GREATER")]
    ExpectGenGreater,
}

impl GenerationPolicy {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireGenerationPolicy {
        match self {
            GenerationPolicy::None => WireGenerationPolicy::None,
            GenerationPolicy::ExpectGenEqual => WireGenerationPolicy::ExpectGenEqual,
            GenerationPolicy::ExpectGenGreater => WireGenerationPolicy::ExpectGenGreater,
        }
    }
}

/// How many replicas must commit before the server answers.
#[php_enum]
#[php(name = "Aerospike\\CommitLevel")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitLevel {
    /// Master and all replicas.
    #[php(value = "COMMIT_ALL")]
    CommitAll,
    /// Master only.
    #[php(value = "COMMIT_MASTER")]
    CommitMaster,
}

impl CommitLevel {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireCommitLevel {
        match self {
            CommitLevel::CommitAll => WireCommitLevel::CommitAll,
            CommitLevel::CommitMaster => WireCommitLevel::CommitMaster,
        }
    }
}

/// A default privilege the server defines.
///
/// The six administrative codes act on the **cluster**, so they cannot be confined
/// to a namespace — `Aerospike\Privilege` refuses that, because the server's own
/// refusal names neither the privilege nor the reason. The seven data codes can.
///
/// ```php
/// new Aerospike\Privilege(Aerospike\PrivilegeCode::ReadWrite, 'test');   // fine
/// new Aerospike\Privilege(Aerospike\PrivilegeCode::SysAdmin, 'test');    // refused
/// ```
#[php_enum]
#[php(name = "Aerospike\\PrivilegeCode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeCode {
    /// Edit and remove other users. Cluster-wide.
    #[php(value = "USER_ADMIN")]
    UserAdmin,
    /// Systems administration that is not user administration — server
    /// configuration, for instance. Cluster-wide.
    #[php(value = "SYS_ADMIN")]
    SysAdmin,
    /// UDF and secondary-index administration. Cluster-wide.
    #[php(value = "DATA_ADMIN")]
    DataAdmin,
    /// UDF administration alone. Cluster-wide; needs server 6+.
    #[php(value = "UDF_ADMIN")]
    UdfAdmin,
    /// Secondary-index administration alone. Cluster-wide; needs server 6+.
    #[php(value = "SINDEX_ADMIN")]
    SIndexAdmin,
    /// Read data. May be confined to a namespace or a set.
    #[php(value = "READ")]
    Read,
    /// Read and write data. May be confined.
    #[php(value = "READ_WRITE")]
    ReadWrite,
    /// Read and write data through user-defined functions. May be confined.
    #[php(value = "READ_WRITE_UDF")]
    ReadWriteUdf,
    /// Write data. May be confined.
    #[php(value = "WRITE")]
    Write,
    /// Truncate data. May be confined; needs server 6+.
    #[php(value = "TRUNCATE")]
    Truncate,
    /// Data-masking administration. Cluster-wide.
    #[php(value = "MASKING_ADMIN")]
    MaskingAdmin,
    /// Read masked data. May be confined.
    #[php(value = "READ_MASKED")]
    ReadMasked,
    /// Write masked data. May be confined.
    #[php(value = "WRITE_MASKED")]
    WriteMasked,
}

impl PrivilegeCode {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WirePrivilegeCode {
        match self {
            PrivilegeCode::UserAdmin => WirePrivilegeCode::UserAdmin,
            PrivilegeCode::SysAdmin => WirePrivilegeCode::SysAdmin,
            PrivilegeCode::DataAdmin => WirePrivilegeCode::DataAdmin,
            PrivilegeCode::UdfAdmin => WirePrivilegeCode::UdfAdmin,
            PrivilegeCode::SIndexAdmin => WirePrivilegeCode::SIndexAdmin,
            PrivilegeCode::Read => WirePrivilegeCode::Read,
            PrivilegeCode::ReadWrite => WirePrivilegeCode::ReadWrite,
            PrivilegeCode::ReadWriteUdf => WirePrivilegeCode::ReadWriteUdf,
            PrivilegeCode::Write => WirePrivilegeCode::Write,
            PrivilegeCode::Truncate => WirePrivilegeCode::Truncate,
            PrivilegeCode::MaskingAdmin => WirePrivilegeCode::MaskingAdmin,
            PrivilegeCode::ReadMasked => WirePrivilegeCode::ReadMasked,
            PrivilegeCode::WriteMasked => WirePrivilegeCode::WriteMasked,
        }
    }

    /// From the contract's, for a role read back from the server.
    #[must_use]
    pub const fn of(code: WirePrivilegeCode) -> PrivilegeCode {
        match code {
            WirePrivilegeCode::UserAdmin => PrivilegeCode::UserAdmin,
            WirePrivilegeCode::SysAdmin => PrivilegeCode::SysAdmin,
            WirePrivilegeCode::DataAdmin => PrivilegeCode::DataAdmin,
            WirePrivilegeCode::UdfAdmin => PrivilegeCode::UdfAdmin,
            WirePrivilegeCode::SIndexAdmin => PrivilegeCode::SIndexAdmin,
            WirePrivilegeCode::Read => PrivilegeCode::Read,
            WirePrivilegeCode::ReadWrite => PrivilegeCode::ReadWrite,
            WirePrivilegeCode::ReadWriteUdf => PrivilegeCode::ReadWriteUdf,
            WirePrivilegeCode::Write => PrivilegeCode::Write,
            WirePrivilegeCode::Truncate => PrivilegeCode::Truncate,
            WirePrivilegeCode::MaskingAdmin => PrivilegeCode::MaskingAdmin,
            WirePrivilegeCode::ReadMasked => PrivilegeCode::ReadMasked,
            WirePrivilegeCode::WriteMasked => PrivilegeCode::WriteMasked,
        }
    }

    /// The case's backing value, for a message that has to name it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            PrivilegeCode::UserAdmin => "USER_ADMIN",
            PrivilegeCode::SysAdmin => "SYS_ADMIN",
            PrivilegeCode::DataAdmin => "DATA_ADMIN",
            PrivilegeCode::UdfAdmin => "UDF_ADMIN",
            PrivilegeCode::SIndexAdmin => "SINDEX_ADMIN",
            PrivilegeCode::Read => "READ",
            PrivilegeCode::ReadWrite => "READ_WRITE",
            PrivilegeCode::ReadWriteUdf => "READ_WRITE_UDF",
            PrivilegeCode::Write => "WRITE",
            PrivilegeCode::Truncate => "TRUNCATE",
            PrivilegeCode::MaskingAdmin => "MASKING_ADMIN",
            PrivilegeCode::ReadMasked => "READ_MASKED",
            PrivilegeCode::WriteMasked => "WRITE_MASKED",
        }
    }
}

/// Where a multi-record transaction has got to.
#[php_enum]
#[php(name = "Aerospike\\TxnState")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnState {
    /// Accepting commands.
    #[php(value = "OPEN")]
    Open,
    /// Every read has been verified; the commit is part-done.
    #[php(value = "VERIFIED")]
    Verified,
    /// Committed. Its writes are permanent.
    #[php(value = "COMMITTED")]
    Committed,
    /// Aborted. Its writes are gone.
    #[php(value = "ABORTED")]
    Aborted,
}

impl TxnState {
    /// From the contract's state.
    #[must_use]
    pub const fn of(state: WireTxnState) -> TxnState {
        match state {
            WireTxnState::Open => TxnState::Open,
            WireTxnState::Verified => TxnState::Verified,
            WireTxnState::Committed => TxnState::Committed,
            WireTxnState::Aborted => TxnState::Aborted,
        }
    }
}

/// How a commit ended.
///
/// **Every case is a success**: the transaction committed. The three besides `Ok`
/// say the client left some tidying to the server, which is worth logging and is
/// *not* a reason to commit again.
#[php_enum]
#[php(name = "Aerospike\\CommitStatus")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitStatus {
    /// Committed, and everything was tidied up.
    #[php(value = "OK")]
    Ok,
    /// It had already been committed.
    #[php(value = "ALREADY_COMMITTED")]
    AlreadyCommitted,
    /// Committed, but rolling the writes forward was abandoned. The server will
    /// finish.
    #[php(value = "ROLL_FORWARD_ABANDONED")]
    RollForwardAbandoned,
    /// Committed and rolled forward, but closing the transaction's monitor record
    /// was abandoned. The server will.
    #[php(value = "CLOSE_ABANDONED")]
    CloseAbandoned,
}

impl CommitStatus {
    /// From the contract's status.
    #[must_use]
    pub const fn of(status: WireCommitStatus) -> CommitStatus {
        match status {
            WireCommitStatus::Ok => CommitStatus::Ok,
            WireCommitStatus::AlreadyCommitted => CommitStatus::AlreadyCommitted,
            WireCommitStatus::RollForwardAbandoned => CommitStatus::RollForwardAbandoned,
            WireCommitStatus::CloseAbandoned => CommitStatus::CloseAbandoned,
        }
    }
}

/// How an abort ended.
///
/// As with a commit, every case means the transaction's writes are not going to
/// land; the differences are only in how much tidying the server was left.
#[php_enum]
#[php(name = "Aerospike\\AbortStatus")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbortStatus {
    /// Aborted, and everything was tidied up.
    #[php(value = "OK")]
    Ok,
    /// It had already been aborted.
    #[php(value = "ALREADY_ABORTED")]
    AlreadyAborted,
    /// Aborted, but rolling the writes back was abandoned. The server will finish.
    #[php(value = "ROLL_BACK_ABANDONED")]
    RollBackAbandoned,
    /// Rolled back, but closing the monitor record was abandoned.
    #[php(value = "CLOSE_ABANDONED")]
    CloseAbandoned,
}

impl AbortStatus {
    /// From the contract's status.
    #[must_use]
    pub const fn of(status: WireAbortStatus) -> AbortStatus {
        match status {
            WireAbortStatus::Ok => AbortStatus::Ok,
            WireAbortStatus::AlreadyAborted => AbortStatus::AlreadyAborted,
            WireAbortStatus::RollBackAbandoned => AbortStatus::RollBackAbandoned,
            WireAbortStatus::CloseAbandoned => AbortStatus::CloseAbandoned,
        }
    }
}

/// How far a long-running server command has got.
///
/// ```php
/// $task = $client->createIndexOnBin(null, 'test', 'users', 'age', 'age_idx', IndexType::Numeric);
/// while ($task->status() === Aerospike\TaskStatus::InProgress) {
///     usleep(500_000);
/// }
/// ```
///
/// `NotFound` right after issuing a command can mean "not started yet" as much as
/// "never existed" — which is why `Task::waitTillComplete()` sleeps before its
/// first check rather than treating the first answer as final.
#[php_enum]
#[php(name = "Aerospike\\TaskStatus")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// The server has no record of the command.
    #[php(value = "NOT_FOUND")]
    NotFound,
    /// Still running.
    #[php(value = "IN_PROGRESS")]
    InProgress,
    /// Finished on every node.
    #[php(value = "COMPLETE")]
    Complete,
}

impl TaskStatus {
    /// From the contract's status.
    #[must_use]
    pub const fn of(status: WireTaskStatus) -> TaskStatus {
        match status {
            WireTaskStatus::NotFound => TaskStatus::NotFound,
            WireTaskStatus::InProgress => TaskStatus::InProgress,
            WireTaskStatus::Complete => TaskStatus::Complete,
        }
    }

    /// The case's backing value, for a message that has to name it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            TaskStatus::NotFound => "NOT_FOUND",
            TaskStatus::InProgress => "IN_PROGRESS",
            TaskStatus::Complete => "COMPLETE",
        }
    }
}

/// What kind of value a secondary index covers.
///
/// The type has to match the bin's: a numeric index over a string bin indexes
/// nothing, and the server does not complain — the query simply finds no records.
#[php_enum]
#[php(name = "Aerospike\\IndexType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    /// Integers.
    #[php(value = "NUMERIC")]
    Numeric,
    /// Strings.
    ///
    /// Named `Text` in PHP: `String` collides with the reserved type name, and a
    /// case that cannot be written is worse than one that reads differently from
    /// the Rust variant.
    #[php(name = "Text")]
    #[php(value = "STRING")]
    String,
    /// Geospatial points and regions, on a sphere. Required for the geo filters.
    #[php(value = "GEO2DSPHERE")]
    Geo2DSphere,
    /// Byte strings. Needs server 7.0 or later.
    #[php(value = "BLOB")]
    Blob,
}

impl IndexType {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireIndexType {
        match self {
            IndexType::Numeric => WireIndexType::Numeric,
            IndexType::String => WireIndexType::String,
            IndexType::Geo2DSphere => WireIndexType::Geo2DSphere,
            IndexType::Blob => WireIndexType::Blob,
        }
    }
}

/// The language a UDF module is written in.
#[php_enum]
#[php(name = "Aerospike\\UdfLanguage")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UdfLanguage {
    /// Lua, which is the only language Aerospike runs.
    #[default]
    #[php(value = "LUA")]
    Lua,
}

impl UdfLanguage {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireUdfLang {
        match self {
            UdfLanguage::Lua => WireUdfLang::Lua,
        }
    }
}

/// Which part of a collection a secondary index covers.
///
/// The distinction a query has to make and a scan never does: an index on a bin
/// holding a list can be built over its *elements*, and one on a map over its
/// keys or its values. `Default` is a plain scalar bin.
///
/// ```php
/// // Records whose "tags" list contains "urgent".
/// Aerospike\Filter::equal("tags", "urgent", Aerospike\CollectionIndex::List);
/// ```
#[php_enum]
#[php(name = "Aerospike\\CollectionIndex")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CollectionIndex {
    /// A scalar bin: the index covers the bin's own value.
    #[default]
    #[php(value = "DEFAULT")]
    Scalar,
    /// List elements.
    ///
    /// Named `ListElements` in PHP: `list` is a reserved keyword there, so it
    /// cannot be a case name however much it would match the Rust variant.
    #[php(name = "ListElements")]
    #[php(value = "LIST")]
    List,
    /// Map keys.
    #[php(value = "MAP_KEYS")]
    MapKeys,
    /// Map values.
    #[php(value = "MAP_VALUES")]
    MapValues,
}

impl CollectionIndex {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireCollectionIndex {
        match self {
            CollectionIndex::Scalar => WireCollectionIndex::Default,
            CollectionIndex::List => WireCollectionIndex::List,
            CollectionIndex::MapKeys => WireCollectionIndex::MapKeys,
            CollectionIndex::MapValues => WireCollectionIndex::MapValues,
        }
    }
}

/// How a list is stored.
#[php_enum]
#[php(name = "Aerospike\\ListOrder")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListOrder {
    /// Insertion order, which is the default.
    #[php(value = "UNORDERED")]
    Unordered,
    /// Sorted by value, which is what makes the rank and value-range
    /// operations run in log time.
    #[php(value = "ORDERED")]
    Ordered,
}

impl ListOrder {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireListOrder {
        match self {
            ListOrder::Unordered => WireListOrder::Unordered,
            ListOrder::Ordered => WireListOrder::Ordered,
        }
    }
}

/// How a map is stored.
///
/// Only reachable through `Ctx::mapKeyCreate()` for now — the map *operations*
/// arrive with the next phase — because a list nested inside a map has to be
/// able to say what kind of map to create on the way down.
#[php_enum]
#[php(name = "Aerospike\\MapOrder")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapOrder {
    /// Insertion order.
    #[php(value = "UNORDERED")]
    Unordered,
    /// Sorted by key.
    #[php(value = "KEY_ORDERED")]
    KeyOrdered,
    /// Sorted by key, then by value.
    #[php(value = "KEY_VALUE_ORDERED")]
    KeyValueOrdered,
}

impl MapOrder {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireMapOrder {
        match self {
            MapOrder::Unordered => WireMapOrder::Unordered,
            MapOrder::KeyOrdered => WireMapOrder::KeyOrdered,
            MapOrder::KeyValueOrdered => WireMapOrder::KeyValueOrdered,
        }
    }
}

/// What a list operation gives back.
///
/// Every `...By...` operation takes one. `Values` is what most callers want;
/// `None` is what to ask for when the operation is being run for its effect and
/// the result would only be shipped back to be discarded.
#[php_enum]
#[php(name = "Aerospike\\ListReturn")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListReturn {
    /// Nothing at all.
    #[php(value = "NONE")]
    None,
    /// The index of each selected item.
    #[php(value = "INDEX")]
    Index,
    /// The index of each selected item, counted from the end.
    #[php(value = "REVERSE_INDEX")]
    ReverseIndex,
    /// The rank of each selected item — its position in value order.
    #[php(value = "RANK")]
    Rank,
    /// The rank of each selected item, counted from the largest.
    #[php(value = "REVERSE_RANK")]
    ReverseRank,
    /// How many items were selected.
    #[php(value = "COUNT")]
    Count,
    /// The selected values themselves.
    #[php(value = "VALUES")]
    Values,
    /// Whether anything was selected at all.
    #[php(value = "EXISTS")]
    Exists,
}

impl ListReturn {
    /// The contract's equivalent, with the inversion flag applied.
    #[must_use]
    pub const fn to_wire(self, inverted: bool) -> WireListReturn {
        let kind = match self {
            ListReturn::None => WireListReturnKind::None,
            ListReturn::Index => WireListReturnKind::Index,
            ListReturn::ReverseIndex => WireListReturnKind::ReverseIndex,
            ListReturn::Rank => WireListReturnKind::Rank,
            ListReturn::ReverseRank => WireListReturnKind::ReverseRank,
            ListReturn::Count => WireListReturnKind::Count,
            ListReturn::Values => WireListReturnKind::Values,
            ListReturn::Exists => WireListReturnKind::Exists,
        };
        WireListReturn { kind, inverted }
    }
}

/// What a map operation gives back.
///
/// Richer than [`ListReturn`] because a map entry has two halves: an operation
/// can hand back the keys, the values, both as pairs, or the selection as a map.
#[php_enum]
#[php(name = "Aerospike\\MapReturn")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapReturn {
    /// Nothing at all.
    #[php(value = "NONE")]
    None,
    /// The key index of each selected entry.
    #[php(value = "INDEX")]
    Index,
    /// The key index of each selected entry, counted from the end.
    #[php(value = "REVERSE_INDEX")]
    ReverseIndex,
    /// The rank of each selected entry — its position in value order.
    #[php(value = "RANK")]
    Rank,
    /// The rank of each selected entry, counted from the largest.
    #[php(value = "REVERSE_RANK")]
    ReverseRank,
    /// How many entries were selected.
    #[php(value = "COUNT")]
    Count,
    /// The keys.
    #[php(value = "KEY")]
    Key,
    /// The values.
    #[php(value = "VALUE")]
    Value,
    /// Both, as `[key, value]` pairs.
    #[php(value = "KEY_VALUE")]
    KeyValue,
    /// Whether anything was selected at all.
    #[php(value = "EXISTS")]
    Exists,
    /// The selection as an unordered map.
    #[php(value = "UNORDERED_MAP")]
    UnorderedMap,
    /// The selection as an ordered map.
    #[php(value = "ORDERED_MAP")]
    OrderedMap,
}

impl MapReturn {
    /// The contract's equivalent, with the inversion flag applied.
    #[must_use]
    pub const fn to_wire(self, inverted: bool) -> WireMapReturn {
        let kind = match self {
            MapReturn::None => WireMapReturnKind::None,
            MapReturn::Index => WireMapReturnKind::Index,
            MapReturn::ReverseIndex => WireMapReturnKind::ReverseIndex,
            MapReturn::Rank => WireMapReturnKind::Rank,
            MapReturn::ReverseRank => WireMapReturnKind::ReverseRank,
            MapReturn::Count => WireMapReturnKind::Count,
            MapReturn::Key => WireMapReturnKind::Key,
            MapReturn::Value => WireMapReturnKind::Value,
            MapReturn::KeyValue => WireMapReturnKind::KeyValue,
            MapReturn::Exists => WireMapReturnKind::Exists,
            MapReturn::UnorderedMap => WireMapReturnKind::UnorderedMap,
            MapReturn::OrderedMap => WireMapReturnKind::OrderedMap,
        };
        WireMapReturn { kind, inverted }
    }
}

/// What a map write does about a key that is, or is not, already there.
#[php_enum]
#[php(name = "Aerospike\\MapWriteMode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapWriteMode {
    /// Create the entry, or overwrite it. The default.
    #[php(value = "UPDATE")]
    Update,
    /// Overwrite only; fail if the key is not there.
    #[php(value = "UPDATE_ONLY")]
    UpdateOnly,
    /// Create only; fail if the key is already there.
    #[php(value = "CREATE_ONLY")]
    CreateOnly,
}

impl MapWriteMode {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireMapWriteMode {
        match self {
            MapWriteMode::Update => WireMapWriteMode::Update,
            MapWriteMode::UpdateOnly => WireMapWriteMode::UpdateOnly,
            MapWriteMode::CreateOnly => WireMapWriteMode::CreateOnly,
        }
    }
}

/// What a bitwise or HyperLogLog write does about the bin already existing.
///
/// Shared by both families, because both spell the same three-way choice the
/// same way — and because a caller should not have to remember two enums that
/// mean one thing.
#[php_enum]
#[php(name = "Aerospike\\BinWriteMode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinWriteMode {
    /// Create the bin, or overwrite it. The default.
    #[php(value = "UPDATE")]
    Update,
    /// Overwrite only; fail if the bin is not there.
    #[php(value = "UPDATE_ONLY")]
    UpdateOnly,
    /// Create only; fail if the bin is already there.
    #[php(value = "CREATE_ONLY")]
    CreateOnly,
}

impl BinWriteMode {
    /// The contract's bitwise equivalent.
    #[must_use]
    pub const fn to_bit_wire(self) -> WireBitWriteMode {
        match self {
            BinWriteMode::Update => WireBitWriteMode::Update,
            BinWriteMode::UpdateOnly => WireBitWriteMode::UpdateOnly,
            BinWriteMode::CreateOnly => WireBitWriteMode::CreateOnly,
        }
    }

    /// The contract's HyperLogLog equivalent.
    #[must_use]
    pub const fn to_hll_wire(self) -> WireHllWriteMode {
        match self {
            BinWriteMode::Update => WireHllWriteMode::Update,
            BinWriteMode::UpdateOnly => WireHllWriteMode::UpdateOnly,
            BinWriteMode::CreateOnly => WireHllWriteMode::CreateOnly,
        }
    }
}

/// Which end `BitOp::resize()` changes, and whether it may only grow or shrink.
///
/// The first case is the one place a case name does *not* mirror the Rust
/// client's: PHP reserves `Default` as an enum case name. `AtEnd` is what the
/// client's `Default` actually means, and it pairs with `FromFront`.
#[php_enum]
#[php(name = "Aerospike\\BitResize")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitResize {
    /// Add or remove bytes at the end. The client calls this `Default`.
    #[php(name = "AtEnd", value = "DEFAULT")]
    Default,
    /// Add or remove bytes at the front.
    #[php(value = "FROM_FRONT")]
    FromFront,
    /// Refuse to shrink.
    #[php(value = "GROW_ONLY")]
    GrowOnly,
    /// Refuse to grow.
    #[php(value = "SHRINK_ONLY")]
    ShrinkOnly,
}

impl BitResize {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireBitResize {
        match self {
            BitResize::Default => WireBitResize::Default,
            BitResize::FromFront => WireBitResize::FromFront,
            BitResize::GrowOnly => WireBitResize::GrowOnly,
            BitResize::ShrinkOnly => WireBitResize::ShrinkOnly,
        }
    }
}

/// What `BitOp::add()` and `BitOp::subtract()` do when the result does not fit
/// in the bit field.
#[php_enum]
#[php(name = "Aerospike\\BitOverflow")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitOverflow {
    /// Fail the operation. The default, because a number that silently became a
    /// different number is worse than an error.
    #[php(value = "FAIL")]
    Fail,
    /// Clamp to the largest or smallest value the field can hold.
    #[php(value = "SATURATE")]
    Saturate,
    /// Wrap around, so one past the maximum is the minimum.
    #[php(value = "WRAP")]
    Wrap,
}

impl BitOverflow {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireBitOverflow {
        match self {
            BitOverflow::Fail => WireBitOverflow::Fail,
            BitOverflow::Saturate => WireBitOverflow::Saturate,
            BitOverflow::Wrap => WireBitOverflow::Wrap,
        }
    }
}

/// How a failure is classified, from `AerospikeException::getStatus()`.
///
/// This is what an application branches on. It answers "whose problem is
/// this" — the server's, the network's, the daemon's, or this call's arguments
/// — where the server result code answers "what exactly did the server say".
///
/// ```php
/// try {
///     $client->put($policy, $key, $bins);
/// } catch (Aerospike\AerospikeException $e) {
///     match ($e->getStatus()) {
///         Aerospike\Status::Timeout    => $retryLater($e->isInDoubt()),
///         Aerospike\Status::Connection => $failOver(),
///         default                      => throw $e,
///     };
/// }
/// ```
#[php_enum]
#[php(name = "Aerospike\\Status")]
// `Default` is not decoration: Zend creates an exception object *before*
// calling its constructor — `throw` from PHP code does — and ext-php-rs fills
// the Rust half in from `Default` at that point. `Client` is the honest value
// there: an exception PHP built itself did not come from the daemon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Status {
    /// The server returned a non-OK result code; see `getResultCode()`.
    #[php(value = "SERVER")]
    Server,
    /// The record does not exist, where that is a failure rather than an
    /// answer.
    #[php(value = "RECORD_NOT_FOUND")]
    RecordNotFound,
    /// The deadline passed. Check `isInDoubt()` before retrying a write.
    #[php(value = "TIMEOUT")]
    Timeout,
    /// The daemon could not reach the cluster.
    #[php(value = "CONNECTION")]
    Connection,
    /// The daemon has no configuration for the instance this client names.
    #[php(value = "UNKNOWN_INSTANCE")]
    UnknownInstance,
    /// The request was malformed or asked for something unsupported.
    #[php(value = "INVALID_REQUEST")]
    InvalidRequest,
    /// A frame exceeded the protocol's maximum payload.
    #[php(value = "FRAME_TOO_LARGE")]
    FrameTooLarge,
    /// The daemon failed internally.
    #[php(value = "INTERNAL")]
    Internal,
    /// A scan or query cursor is no longer open, so the traversal has to start
    /// again.
    ///
    /// The one status worth catching separately: a `RecordSet` that reports this
    /// mid-iteration has *not* finished, and treating it as the end would drop
    /// every record the scan had not reached.
    #[php(value = "CURSOR_EXPIRED")]
    CursorExpired,
    /// A multi-record transaction is no longer open, so the work has to start
    /// again.
    ///
    /// Worth catching separately: it means the transaction's writes are *not*
    /// applied, and that it was committed, aborted, or aborted for you after
    /// sitting idle. A command that failed *inside* a still-open transaction is an
    /// ordinary failure, and that transaction can still be aborted.
    #[php(value = "TXN_EXPIRED")]
    TxnExpired,
    /// The failure never reached the daemon: it could not be attached to, a
    /// value could not be expressed, or the frame did not validate.
    #[default]
    #[php(value = "CLIENT")]
    Client,
    /// A status this build of the extension does not know.
    ///
    /// Unreachable while the daemon and the extension are the same version,
    /// which they must be — but a status read out of shared memory is data,
    /// and turning unexpected data into a panic would be worse than naming it.
    #[php(value = "UNRECOGNIZED")]
    Unrecognized,
}

impl Status {
    /// Classify a reply's status.
    ///
    /// `None` — a failure that never reached the daemon — is [`Status::Client`];
    /// [`StatusCode::OK`] cannot appear on an exception, and is mapped to
    /// [`Status::Unrecognized`] rather than given a case of its own that could
    /// only ever mean "this should not have thrown".
    #[must_use]
    pub fn of(status: Option<StatusCode>) -> Status {
        let Some(status) = status else {
            return Status::Client;
        };
        match status {
            StatusCode::SERVER => Status::Server,
            StatusCode::RECORD_NOT_FOUND => Status::RecordNotFound,
            StatusCode::TIMEOUT => Status::Timeout,
            StatusCode::CONNECTION => Status::Connection,
            StatusCode::UNKNOWN_INSTANCE => Status::UnknownInstance,
            StatusCode::INVALID_REQUEST => Status::InvalidRequest,
            StatusCode::FRAME_TOO_LARGE => Status::FrameTooLarge,
            StatusCode::INTERNAL => Status::Internal,
            StatusCode::CURSOR_EXPIRED => Status::CursorExpired,
            StatusCode::TXN_EXPIRED => Status::TxnExpired,
            _ => Status::Unrecognized,
        }
    }

    /// The stable label, matching the enum's backing value.
    ///
    /// Equal to `StatusCode::label()` for every status the contract defines,
    /// which the tests below pin: the two must not drift, because an
    /// application may well be matching on the string.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Status::Server => "SERVER",
            Status::RecordNotFound => "RECORD_NOT_FOUND",
            Status::Timeout => "TIMEOUT",
            Status::Connection => "CONNECTION",
            Status::UnknownInstance => "UNKNOWN_INSTANCE",
            Status::InvalidRequest => "INVALID_REQUEST",
            Status::FrameTooLarge => "FRAME_TOO_LARGE",
            Status::Internal => "INTERNAL",
            Status::CursorExpired => "CURSOR_EXPIRED",
            Status::TxnExpired => "TXN_EXPIRED",
            Status::Client => CLIENT_STATUS_LABEL,
            Status::Unrecognized => "UNRECOGNIZED",
        }
    }
}

/// The type an expression evaluates to.
///
/// Aerospike needs this where the expression itself does not settle it — reading
/// a bin, reading the key, or pulling a value out of a collection — because the
/// server has to know how to interpret the bytes it finds. Mirrors
/// `aerospike-core`'s `ExpType`.
///
/// **It is not a hint.** `Exp::bin('age', ExpType::Str)` reads an integer bin's
/// bytes as a string, and the comparison that follows is against nonsense rather
/// than an error. The typed shorthands — `Exp::intBin()`, `Exp::stringBin()` and
/// the rest — exist so that most code never names one of these.
#[php_enum]
#[php(name = "Aerospike\\ExpType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExpType {
    /// Nil, which is what an absent bin reads as.
    #[php(value = "NIL")]
    Nil,
    /// Boolean.
    ///
    /// `Boolean` and not `Bool`: PHP reserves its scalar type names, and a case
    /// nobody can write would be worse than one spelled out.
    #[php(name = "Boolean")]
    #[php(value = "BOOL")]
    Bool,
    /// Integer. `Integer` in PHP, for the reason [`ExpType::Bool`] gives.
    #[php(name = "Integer")]
    #[php(value = "INT")]
    Int,
    /// String. `Text` in PHP, as [`IndexType::String`] is.
    #[php(name = "Text")]
    #[php(value = "STRING")]
    Str,
    /// List. `ListType` in PHP, which reserves `list` for the language construct.
    #[php(name = "ListType")]
    #[php(value = "LIST")]
    List,
    /// Map. `MapType` in PHP, for symmetry with [`ExpType::List`] rather than
    /// because PHP requires it.
    #[php(name = "MapType")]
    #[php(value = "MAP")]
    Map,
    /// Byte string.
    #[php(value = "BLOB")]
    Blob,
    /// Double. `Double` in PHP, for the reason [`ExpType::Bool`] gives.
    #[php(name = "Double")]
    #[php(value = "FLOAT")]
    Float,
    /// GeoJSON.
    #[php(value = "GEO")]
    Geo,
    /// HyperLogLog sketch.
    #[php(value = "HLL")]
    Hll,
}

impl ExpType {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireExpType {
        match self {
            ExpType::Nil => WireExpType::Nil,
            ExpType::Bool => WireExpType::Bool,
            ExpType::Int => WireExpType::Int,
            ExpType::Str => WireExpType::Str,
            ExpType::List => WireExpType::List,
            ExpType::Map => WireExpType::Map,
            ExpType::Blob => WireExpType::Blob,
            ExpType::Float => WireExpType::Float,
            ExpType::Geo => WireExpType::Geo,
            ExpType::Hll => WireExpType::Hll,
        }
    }
}

/// Which numbers `ExpStr::isNumericTyped()` accepts.
#[php_enum]
#[php(name = "Aerospike\\StringNumericType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringNumericType {
    /// An integer or a floating-point number.
    #[php(value = "ANY")]
    Any,
    /// Only integers.
    #[php(name = "Integer")]
    #[php(value = "INT")]
    Int,
    /// Only floating-point numbers.
    #[php(name = "Double")]
    #[php(value = "FLOAT")]
    Float,
}

impl StringNumericType {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireStringNumericType {
        match self {
            StringNumericType::Any => WireStringNumericType::Any,
            StringNumericType::Int => WireStringNumericType::Int,
            StringNumericType::Float => WireStringNumericType::Float,
        }
    }
}

/// Flags for the string expressions that take a regular expression, combined
/// with `|`.
///
/// Constants rather than an enum, for the reason `Aerospike\RegexFlag` gives: they
/// are a bitmask. Distinct from `RegexFlag` because these are **ICU** flags, which
/// the string operations use, where `Exp::regexCompare` uses the server's POSIX
/// ones — same idea, different engine, different values.
#[php_class]
#[php(name = "Aerospike\\StringRegexFlag")]
#[derive(Debug)]
pub struct StringRegexFlag;

#[php_impl]
impl StringRegexFlag {
    /// ICU defaults.
    pub const NONE: i64 = 0;
    /// Case-insensitive matching.
    pub const CASE_INSENSITIVE: i64 = 1;
    /// `^` and `$` match the start and end of any line.
    pub const MULTILINE: i64 = 2;
    /// `.` matches line terminators too.
    pub const DOT_ALL: i64 = 4;
    /// Only `\n` is a line terminator.
    pub const UNIX_LINES: i64 = 8;
    /// Replace every match. Only meaningful for `regexReplace`.
    pub const GLOBAL: i64 = 16;
}

/// Which side of the child a loop variable refers to.
///
/// Inside a fan-out, the loop variable stands for the node being considered. A map
/// child has a key, a value and an index; a list child has a value and an index.
///
/// An enum rather than constants — unlike `SelectFlag` — because these do not
/// combine: a loop variable refers to exactly one part.
#[php_enum]
#[php(name = "Aerospike\\LoopVarPart")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopVarPart {
    /// The child's map key.
    #[php(value = "MAP_KEY")]
    MapKey,
    /// The child's value — a list element, or a map value.
    #[php(value = "VALUE")]
    Value,
    /// The child's list index.
    #[php(value = "INDEX")]
    Index,
}

impl LoopVarPart {
    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(self) -> WireLoopVarPart {
        match self {
            LoopVarPart::MapKey => WireLoopVarPart::MAP_KEY,
            LoopVarPart::Value => WireLoopVarPart::VALUE,
            LoopVarPart::Index => WireLoopVarPart::INDEX,
        }
    }
}

// ===== the 1.x client's enum spellings ======================================

/// The 1.x client's static factories for [`ReadModeAp`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `ReadModeAp::one()`
/// and `ReadModeAp::One` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// The 1.x client spelled these lower-case, and its class was `ReadModeAP` — which
/// is *this* class, because PHP class names are case-insensitive.
#[php_impl]
impl ReadModeAp {
    /// The 1.x spelling of [`ReadModeAp::One`].
    #[php(name = "one")]
    pub fn legacy_one() -> Case<ReadModeAp> {
        Case(ReadModeAp::One)
    }

    /// The 1.x spelling of [`ReadModeAp::All`].
    #[php(name = "all")]
    pub fn legacy_all() -> Case<ReadModeAp> {
        Case(ReadModeAp::All)
    }
}

/// The 1.x client's static factories for [`ReadModeSc`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `ReadModeSc::Session()`
/// and `ReadModeSc::Session` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// 1.x called this `ReadModeSC`, which is this class: PHP class names are
/// case-insensitive.
#[php_impl]
impl ReadModeSc {
    /// The 1.x spelling of [`ReadModeSc::Session`].
    #[php(name = "Session")]
    pub fn legacy_session() -> Case<ReadModeSc> {
        Case(ReadModeSc::Session)
    }

    /// The 1.x spelling of [`ReadModeSc::Linearize`].
    #[php(name = "Linearize")]
    pub fn legacy_linearize() -> Case<ReadModeSc> {
        Case(ReadModeSc::Linearize)
    }

    /// The 1.x spelling of [`ReadModeSc::AllowReplica`].
    #[php(name = "AllowReplica")]
    pub fn legacy_allow_replica() -> Case<ReadModeSc> {
        Case(ReadModeSc::AllowReplica)
    }

    /// The 1.x spelling of [`ReadModeSc::AllowUnavailable`].
    #[php(name = "AllowUnavailable")]
    pub fn legacy_allow_unavailable() -> Case<ReadModeSc> {
        Case(ReadModeSc::AllowUnavailable)
    }
}

/// The 1.x client's static factories for [`CommitLevel`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `CommitLevel::CommitAll()`
/// and `CommitLevel::CommitAll` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
#[php_impl]
impl CommitLevel {
    /// The 1.x spelling of [`CommitLevel::CommitAll`].
    #[php(name = "CommitAll")]
    pub fn legacy_commit_all() -> Case<CommitLevel> {
        Case(CommitLevel::CommitAll)
    }

    /// The 1.x spelling of [`CommitLevel::CommitMaster`].
    #[php(name = "CommitMaster")]
    pub fn legacy_commit_master() -> Case<CommitLevel> {
        Case(CommitLevel::CommitMaster)
    }
}

/// The 1.x client's static factories for [`GenerationPolicy`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `GenerationPolicy::None()`
/// and `GenerationPolicy::None` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
#[php_impl]
impl GenerationPolicy {
    /// The 1.x spelling of [`GenerationPolicy::None`].
    #[php(name = "None")]
    pub fn legacy_none() -> Case<GenerationPolicy> {
        Case(GenerationPolicy::None)
    }

    /// The 1.x spelling of [`GenerationPolicy::ExpectGenEqual`].
    #[php(name = "ExpectGenEqual")]
    pub fn legacy_expect_gen_equal() -> Case<GenerationPolicy> {
        Case(GenerationPolicy::ExpectGenEqual)
    }

    /// The 1.x spelling of [`GenerationPolicy::ExpectGenGreater`].
    #[php(name = "ExpectGenGreater")]
    pub fn legacy_expect_gen_greater() -> Case<GenerationPolicy> {
        Case(GenerationPolicy::ExpectGenGreater)
    }
}

/// The 1.x client's static factories for [`RecordExistsAction`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `RecordExistsAction::Update()`
/// and `RecordExistsAction::Update` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
#[php_impl]
impl RecordExistsAction {
    /// The 1.x spelling of [`RecordExistsAction::Update`].
    #[php(name = "Update")]
    pub fn legacy_update() -> Case<RecordExistsAction> {
        Case(RecordExistsAction::Update)
    }

    /// The 1.x spelling of [`RecordExistsAction::UpdateOnly`].
    #[php(name = "UpdateOnly")]
    pub fn legacy_update_only() -> Case<RecordExistsAction> {
        Case(RecordExistsAction::UpdateOnly)
    }

    /// The 1.x spelling of [`RecordExistsAction::Replace`].
    #[php(name = "Replace")]
    pub fn legacy_replace() -> Case<RecordExistsAction> {
        Case(RecordExistsAction::Replace)
    }

    /// The 1.x spelling of [`RecordExistsAction::ReplaceOnly`].
    #[php(name = "ReplaceOnly")]
    pub fn legacy_replace_only() -> Case<RecordExistsAction> {
        Case(RecordExistsAction::ReplaceOnly)
    }

    /// The 1.x spelling of [`RecordExistsAction::CreateOnly`].
    #[php(name = "CreateOnly")]
    pub fn legacy_create_only() -> Case<RecordExistsAction> {
        Case(RecordExistsAction::CreateOnly)
    }
}

/// The 1.x client's static factories for [`IndexType`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `IndexType::Numeric()`
/// and `IndexType::Numeric` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// `String()` was 1.x's name for what is now the `Text` case — PHP reserves `String`
/// as a type name, so a case cannot use it.
#[php_impl]
impl IndexType {
    /// The 1.x spelling of [`IndexType::Numeric`].
    #[php(name = "Numeric")]
    pub fn legacy_numeric() -> Case<IndexType> {
        Case(IndexType::Numeric)
    }

    /// The 1.x spelling of [`IndexType::Text`].
    #[php(name = "String")]
    pub fn legacy_string() -> Case<IndexType> {
        Case(IndexType::String)
    }

    /// The 1.x spelling of [`IndexType::Blob`].
    #[php(name = "Blob")]
    pub fn legacy_blob() -> Case<IndexType> {
        Case(IndexType::Blob)
    }

    /// The 1.x spelling of [`IndexType::Geo2DSphere`].
    #[php(name = "Geo2DSphere")]
    pub fn legacy_geo2_d_sphere() -> Case<IndexType> {
        Case(IndexType::Geo2DSphere)
    }
}

/// The 1.x client's static factories for [`UdfLanguage`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `UdfLanguage::Lua()`
/// and `UdfLanguage::Lua` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
#[php_impl]
impl UdfLanguage {
    /// The 1.x spelling of [`UdfLanguage::Lua`].
    #[php(name = "Lua")]
    pub fn legacy_lua() -> Case<UdfLanguage> {
        Case(UdfLanguage::Lua)
    }
}

/// The 1.x client's static factories for [`MapWriteMode`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `MapWriteMode::Update()`
/// and `MapWriteMode::Update` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
#[php_impl]
impl MapWriteMode {
    /// The 1.x spelling of [`MapWriteMode::Update`].
    #[php(name = "Update")]
    pub fn legacy_update() -> Case<MapWriteMode> {
        Case(MapWriteMode::Update)
    }

    /// The 1.x spelling of [`MapWriteMode::UpdateOnly`].
    #[php(name = "UpdateOnly")]
    pub fn legacy_update_only() -> Case<MapWriteMode> {
        Case(MapWriteMode::UpdateOnly)
    }

    /// The 1.x spelling of [`MapWriteMode::CreateOnly`].
    #[php(name = "CreateOnly")]
    pub fn legacy_create_only() -> Case<MapWriteMode> {
        Case(MapWriteMode::CreateOnly)
    }
}

/// The 1.x client's static factories for [`ExpType`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `ExpType::Nil()`
/// and `ExpType::Nil` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// Six are renamed cases: PHP reserves `Bool`, `Int`, `String`, `Float` and `List`,
/// so the cases are `Boolean`, `Integer`, `Text`, `Double`, `ListType` and `MapType`.
#[php_impl]
impl ExpType {
    /// The 1.x spelling of [`ExpType::Nil`].
    #[php(name = "Nil")]
    pub fn legacy_nil() -> Case<ExpType> {
        Case(ExpType::Nil)
    }

    /// The 1.x spelling of [`ExpType::Boolean`].
    #[php(name = "Bool")]
    pub fn legacy_bool() -> Case<ExpType> {
        Case(ExpType::Bool)
    }

    /// The 1.x spelling of [`ExpType::Integer`].
    #[php(name = "Int")]
    pub fn legacy_int() -> Case<ExpType> {
        Case(ExpType::Int)
    }

    /// The 1.x spelling of [`ExpType::Text`].
    #[php(name = "String")]
    pub fn legacy_string() -> Case<ExpType> {
        Case(ExpType::Str)
    }

    /// The 1.x spelling of [`ExpType::ListType`].
    #[php(name = "List")]
    pub fn legacy_list() -> Case<ExpType> {
        Case(ExpType::List)
    }

    /// The 1.x spelling of [`ExpType::MapType`].
    #[php(name = "Map")]
    pub fn legacy_map() -> Case<ExpType> {
        Case(ExpType::Map)
    }

    /// The 1.x spelling of [`ExpType::Blob`].
    #[php(name = "Blob")]
    pub fn legacy_blob() -> Case<ExpType> {
        Case(ExpType::Blob)
    }

    /// The 1.x spelling of [`ExpType::Double`].
    #[php(name = "Float")]
    pub fn legacy_float() -> Case<ExpType> {
        Case(ExpType::Float)
    }

    /// The 1.x spelling of [`ExpType::Geo`].
    #[php(name = "Geo")]
    pub fn legacy_geo() -> Case<ExpType> {
        Case(ExpType::Geo)
    }

    /// The 1.x spelling of [`ExpType::Hll`].
    #[php(name = "Hll")]
    pub fn legacy_hll() -> Case<ExpType> {
        Case(ExpType::Hll)
    }
}

/// The 1.x client's static factories for [`ListOrder`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `ListOrder::Ordered()`
/// and `ListOrder::Ordered` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// 1.x called this class `ListOrderType`.
#[php_impl]
impl ListOrder {
    /// The 1.x spelling of [`ListOrder::Ordered`].
    #[php(name = "Ordered")]
    pub fn legacy_ordered() -> Case<ListOrder> {
        Case(ListOrder::Ordered)
    }

    /// The 1.x spelling of [`ListOrder::Unordered`].
    #[php(name = "Unordered")]
    pub fn legacy_unordered() -> Case<ListOrder> {
        Case(ListOrder::Unordered)
    }
}

/// The 1.x client's static factories for [`MapOrder`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `MapOrder::Unordered()`
/// and `MapOrder::Unordered` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// 1.x called this class `MapOrderType`.
#[php_impl]
impl MapOrder {
    /// The 1.x spelling of [`MapOrder::Unordered`].
    #[php(name = "Unordered")]
    pub fn legacy_unordered() -> Case<MapOrder> {
        Case(MapOrder::Unordered)
    }

    /// The 1.x spelling of [`MapOrder::KeyOrdered`].
    #[php(name = "KeyOrdered")]
    pub fn legacy_key_ordered() -> Case<MapOrder> {
        Case(MapOrder::KeyOrdered)
    }

    /// The 1.x spelling of [`MapOrder::KeyValueOrdered`].
    #[php(name = "KeyValueOrdered")]
    pub fn legacy_key_value_ordered() -> Case<MapOrder> {
        Case(MapOrder::KeyValueOrdered)
    }
}

/// The 1.x client's static factories for [`ListReturn`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `ListReturn::None()`
/// and `ListReturn::None` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// 1.x called this class `ListReturnType`, and its `Value` is this one's `Values` —
/// plural, because an operation returns as many as it selected. Its `count` was
/// lower-case.
#[php_impl]
impl ListReturn {
    /// The 1.x spelling of [`ListReturn::None`].
    #[php(name = "None")]
    pub fn legacy_none() -> Case<ListReturn> {
        Case(ListReturn::None)
    }

    /// The 1.x spelling of [`ListReturn::Index`].
    #[php(name = "Index")]
    pub fn legacy_index() -> Case<ListReturn> {
        Case(ListReturn::Index)
    }

    /// The 1.x spelling of [`ListReturn::ReverseIndex`].
    #[php(name = "ReverseIndex")]
    pub fn legacy_reverse_index() -> Case<ListReturn> {
        Case(ListReturn::ReverseIndex)
    }

    /// The 1.x spelling of [`ListReturn::Rank`].
    #[php(name = "Rank")]
    pub fn legacy_rank() -> Case<ListReturn> {
        Case(ListReturn::Rank)
    }

    /// The 1.x spelling of [`ListReturn::ReverseRank`].
    #[php(name = "ReverseRank")]
    pub fn legacy_reverse_rank() -> Case<ListReturn> {
        Case(ListReturn::ReverseRank)
    }

    /// The 1.x spelling of [`ListReturn::Count`].
    #[php(name = "count")]
    pub fn legacy_count() -> Case<ListReturn> {
        Case(ListReturn::Count)
    }

    /// The 1.x spelling of [`ListReturn::Values`].
    #[php(name = "Value")]
    pub fn legacy_value() -> Case<ListReturn> {
        Case(ListReturn::Values)
    }

    /// The 1.x spelling of [`ListReturn::Exists`].
    #[php(name = "Exists")]
    pub fn legacy_exists() -> Case<ListReturn> {
        Case(ListReturn::Exists)
    }
}

/// The 1.x client's static factories for [`MapReturn`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `MapReturn::None()`
/// and `MapReturn::None` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// 1.x called this class `MapReturnType`.
#[php_impl]
impl MapReturn {
    /// The 1.x spelling of [`MapReturn::None`].
    #[php(name = "None")]
    pub fn legacy_none() -> Case<MapReturn> {
        Case(MapReturn::None)
    }

    /// The 1.x spelling of [`MapReturn::Index`].
    #[php(name = "Index")]
    pub fn legacy_index() -> Case<MapReturn> {
        Case(MapReturn::Index)
    }

    /// The 1.x spelling of [`MapReturn::ReverseIndex`].
    #[php(name = "ReverseIndex")]
    pub fn legacy_reverse_index() -> Case<MapReturn> {
        Case(MapReturn::ReverseIndex)
    }

    /// The 1.x spelling of [`MapReturn::Rank`].
    #[php(name = "Rank")]
    pub fn legacy_rank() -> Case<MapReturn> {
        Case(MapReturn::Rank)
    }

    /// The 1.x spelling of [`MapReturn::ReverseRank`].
    #[php(name = "ReverseRank")]
    pub fn legacy_reverse_rank() -> Case<MapReturn> {
        Case(MapReturn::ReverseRank)
    }

    /// The 1.x spelling of [`MapReturn::Count`].
    #[php(name = "Count")]
    pub fn legacy_count() -> Case<MapReturn> {
        Case(MapReturn::Count)
    }

    /// The 1.x spelling of [`MapReturn::Key`].
    #[php(name = "Key")]
    pub fn legacy_key() -> Case<MapReturn> {
        Case(MapReturn::Key)
    }

    /// The 1.x spelling of [`MapReturn::Value`].
    #[php(name = "Value")]
    pub fn legacy_value() -> Case<MapReturn> {
        Case(MapReturn::Value)
    }

    /// The 1.x spelling of [`MapReturn::KeyValue`].
    #[php(name = "KeyValue")]
    pub fn legacy_key_value() -> Case<MapReturn> {
        Case(MapReturn::KeyValue)
    }

    /// The 1.x spelling of [`MapReturn::Exists`].
    #[php(name = "Exists")]
    pub fn legacy_exists() -> Case<MapReturn> {
        Case(MapReturn::Exists)
    }

    /// The 1.x spelling of [`MapReturn::UnorderedMap`].
    #[php(name = "UnorderedMap")]
    pub fn legacy_unordered_map() -> Case<MapReturn> {
        Case(MapReturn::UnorderedMap)
    }

    /// The 1.x spelling of [`MapReturn::OrderedMap`].
    #[php(name = "OrderedMap")]
    pub fn legacy_ordered_map() -> Case<MapReturn> {
        Case(MapReturn::OrderedMap)
    }
}

/// The 1.x client's static factories for [`CollectionIndex`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `CollectionIndex::Default()`
/// and `CollectionIndex::Scalar` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// 1.x called this class `IndexCollectionType`, and its `Default` is this one's
/// `Scalar` — the case means "the bin's own value, not a collection in it", which
/// `Default` did not say.
#[php_impl]
impl CollectionIndex {
    /// The 1.x spelling of [`CollectionIndex::Scalar`].
    #[php(name = "Default")]
    pub fn legacy_default() -> Case<CollectionIndex> {
        Case(CollectionIndex::Scalar)
    }

    /// The 1.x spelling of [`CollectionIndex::ListElements`].
    #[php(name = "List")]
    pub fn legacy_list() -> Case<CollectionIndex> {
        Case(CollectionIndex::List)
    }

    /// The 1.x spelling of [`CollectionIndex::MapKeys`].
    #[php(name = "MapKeys")]
    pub fn legacy_map_keys() -> Case<CollectionIndex> {
        Case(CollectionIndex::MapKeys)
    }

    /// The 1.x spelling of [`CollectionIndex::MapValues`].
    #[php(name = "MapValues")]
    pub fn legacy_map_values() -> Case<CollectionIndex> {
        Case(CollectionIndex::MapValues)
    }
}

/// The 1.x client's static factories for [`BitResize`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `BitResize::Default()`
/// and `BitResize::AtEnd` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// 1.x called this class `BitwiseResizeFlags`, and its `Default` is `AtEnd`.
#[php_impl]
impl BitResize {
    /// The 1.x spelling of [`BitResize::AtEnd`].
    #[php(name = "Default")]
    pub fn legacy_default() -> Case<BitResize> {
        Case(BitResize::Default)
    }
}

/// The 1.x client's static factories for [`BitOverflow`].
///
/// That client had no PHP enums: each of these was a class whose static methods
/// returned an instance. These return the matching case, so `BitOverflow::Fail()`
/// and `BitOverflow::Fail` are the same value and old code needs no edit.
///
/// The Rust names are prefixed because an associated function may not share a
/// unit variant's name; `#[php(name)]` gives PHP the 1.x spelling.
///
/// 1.x called this class `BitwiseOverflowAction`.
#[php_impl]
impl BitOverflow {
    /// The 1.x spelling of [`BitOverflow::Fail`].
    #[php(name = "Fail")]
    pub fn legacy_fail() -> Case<BitOverflow> {
        Case(BitOverflow::Fail)
    }

    /// The 1.x spelling of [`BitOverflow::Saturate`].
    #[php(name = "Saturate")]
    pub fn legacy_saturate() -> Case<BitOverflow> {
        Case(BitOverflow::Saturate)
    }

    /// The 1.x spelling of [`BitOverflow::Wrap`].
    #[php(name = "Wrap")]
    pub fn legacy_wrap() -> Case<BitOverflow> {
        Case(BitOverflow::Wrap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every status the contract can put in a reply must map onto a case of
    /// its own — otherwise an application branching on `getStatus()` would see
    /// two different failures as the same one.
    #[test]
    fn every_contract_status_has_its_own_case() {
        let mapped = [
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
        ]
        .map(|status| Status::of(Some(status)));

        for (index, status) in mapped.iter().enumerate() {
            assert_ne!(
                *status,
                Status::Unrecognized,
                "status at index {index} fell through to Unrecognized"
            );
            assert_eq!(mapped.iter().filter(|other| *other == status).count(), 1);
        }
    }

    /// The labels are the enum's backing values, and applications may match on
    /// them, so they have to stay equal to the contract's own labels.
    #[test]
    fn labels_agree_with_the_contract() {
        for status in [
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
            assert_eq!(Status::of(Some(status)).label(), status.label());
        }
    }

    #[test]
    fn a_failure_that_never_reached_the_daemon_is_a_client_failure() {
        assert_eq!(Status::of(None), Status::Client);
        assert_eq!(Status::of(None).label(), CLIENT_STATUS_LABEL);
        // A status from outside the contract's range must be named, not
        // panicked over: it arrived as data out of shared memory.
        assert_eq!(Status::of(Some(StatusCode(999))), Status::Unrecognized);
    }
}
