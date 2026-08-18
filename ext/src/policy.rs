// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

// The constructors' parameters are camelCase because PHP named arguments use
// the parameter name exactly as written, and a policy is meant to be built with
// named arguments. `#[php_impl]` copies those identifiers into bindings in code
// it generates *outside* the impl block, so the lint cannot be silenced any
// closer than the module.
#![allow(non_snake_case)]

//! [`ReadPolicy`], [`WritePolicy`] and [`Expiration`].
//!
//! # Why two classes and not one array
//!
//! `aerospike-core` splits these — a read takes a `ReadPolicy`, a write takes a
//! `WritePolicy` — and so does this. The split is not decoration: it means
//! `$client->get($policy, $key)` cannot be handed a `recordExistsAction` at
//! all, because a `ReadPolicy` has nowhere to put one. Under the associative
//! array this replaces, that mistake was a *runtime* error the daemon reported
//! after a round trip; now it is a `TypeError` at the call, or a named-argument
//! error at the constructor, before anything is sent.
//!
//! The one flat [`WirePolicy`] on the wire is unchanged: the daemon knows which
//! verb it is serving, and one type there keeps its handlers uniform. The
//! asymmetry is deliberate — the wire is a place for one shape, an API is a
//! place for the shape that catches mistakes.
//!
//! # Every field is optional, and that means something
//!
//! A field left unset is **not** "the default value". It means *do not
//! override*, and the daemon's own configured default applies — including
//! anything the `[defaults]` section of its configuration file sets. A policy
//! object that filled its fields with plausible defaults would silently defeat
//! that file, which is why every field is nullable and null is the initial
//! state of all of them.
//!
//! ```php
//! // Everything the daemon is configured with, unchanged.
//! $client->put(null, $key, $bins);
//!
//! // The same, but this one write must not create the record, and must not
//! // outlive the hour.
//! $client->put(new Aerospike\WritePolicy(
//!     recordExistsAction: Aerospike\RecordExistsAction::UpdateOnly,
//!     expiration:         Aerospike\Expiration::seconds(3600),
//! ), $key, $bins);
//! ```
//!
//! # Why the objects are immutable
//!
//! There are no public properties and no setters: a policy is built by its
//! constructor and read by its getters. Properties registered from Rust are
//! declared to PHP as untyped, so `$policy->expiration = 3600` would be
//! accepted by the engine and only fail deeper in — the one part of this
//! surface the type system could not check. Named constructor arguments give
//! the same ergonomics with every argument checked, and an immutable policy is
//! safe to keep in a static and reuse across requests, which is exactly what a
//! PHP worker wants to do with one.

use aerospike_php_ipc::op::WireExpression;
use aerospike_php_ipc::policy::WireExpiration;
use aerospike_php_ipc::WirePolicy;
use ext_php_rs::prelude::*;

use crate::arg::Given;
use crate::enums::{
    Case, CommitLevel, GenerationPolicy, ReadModeAp, ReadModeSc, RecordExistsAction, Replica,
};
use crate::error::{AeroError, AeroResult};
use crate::ops::Expression;
use crate::txn::Transaction;

/// How long a record lives after the write that carries this.
///
/// A class rather than an int, because three of the four cases are not
/// durations at all. Other clients encode them as `-1` and `-2`, which is
/// precisely the kind of detail that gets mixed up crossing a language
/// boundary — so here they have names, and a negative number of seconds is an
/// error rather than a sentinel someone meant.
///
/// ```php
/// Aerospike\Expiration::seconds(3600);      // one hour from now
/// Aerospike\Expiration::never();            // never expires
/// Aerospike\Expiration::namespaceDefault(); // the namespace's default-ttl
/// Aerospike\Expiration::dontUpdate();       // leave the current TTL alone
/// ```
#[php_class]
#[php(name = "Aerospike\\Expiration")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expiration {
    wire: WireExpiration,
}

#[php_impl]
impl Expiration {
    /// Expire this many seconds from now.
    ///
    /// `0` is legal and means the namespace default, which is what the server
    /// does with a zero TTL; say [`Expiration::namespace_default`] if that is
    /// what you mean.
    pub fn seconds(seconds: i64) -> PhpResult<Expiration> {
        Ok(Expiration::of_seconds(seconds)?)
    }

    /// Use the namespace's `default-ttl`.
    pub fn namespace_default() -> Expiration {
        Expiration {
            wire: WireExpiration::NamespaceDefault,
        }
    }

    /// Never expire.
    pub fn never() -> Expiration {
        Expiration {
            wire: WireExpiration::Never,
        }
    }

    /// Leave the record's current TTL exactly as it is.
    ///
    /// The difference from [`Expiration::namespace_default`] matters on an
    /// update: this one does not reset the countdown, so a record keeps the
    /// life it had.
    pub fn dont_update() -> Expiration {
        Expiration {
            wire: WireExpiration::DontUpdate,
        }
    }

    /// The number of seconds, or `null` for the three named cases.
    pub fn to_seconds(&self) -> Option<i64> {
        match self.wire {
            WireExpiration::Seconds(seconds) => Some(i64::from(seconds)),
            _ => None,
        }
    }

    /// Whether this is [`Expiration::never`].
    pub fn is_never(&self) -> bool {
        self.wire == WireExpiration::Never
    }

    /// Whether this is [`Expiration::namespace_default`].
    pub fn is_namespace_default(&self) -> bool {
        self.wire == WireExpiration::NamespaceDefault
    }

    /// Whether this is [`Expiration::dont_update`].
    pub fn is_dont_update(&self) -> bool {
        self.wire == WireExpiration::DontUpdate
    }

    /// A readable form, for logs and test failures.
    pub fn __to_string(&self) -> String {
        match self.wire {
            WireExpiration::NamespaceDefault => "namespace default".to_owned(),
            WireExpiration::Never => "never".to_owned(),
            WireExpiration::DontUpdate => "don't update".to_owned(),
            WireExpiration::Seconds(seconds) => format!("{seconds}s"),
        }
    }
}

impl Expiration {
    /// [`Expiration::seconds`] without the PHP wrapping.
    ///
    /// # Errors
    /// `AeroError` for a negative count, or one past what the wire carries.
    /// A negative number is *not* read as one of the named cases: that
    /// reinterpretation is the mistake this type exists to prevent.
    pub fn of_seconds(seconds: i64) -> AeroResult<Expiration> {
        let seconds = u32::try_from(seconds).map_err(|_| {
            AeroError::client(format!(
                "Expiration::seconds() takes 0 to {}, but was given {seconds}. The other cases \
                 have names, not negative numbers: Expiration::never(), \
                 Expiration::namespaceDefault() and Expiration::dontUpdate()",
                u32::MAX
            ))
        })?;
        Ok(Expiration {
            wire: WireExpiration::Seconds(seconds),
        })
    }

    /// The contract's equivalent.
    #[must_use]
    pub const fn to_wire(&self) -> WireExpiration {
        self.wire
    }
}

/// Per-call settings for a read: `get` and `exists`.
///
/// Mirrors `aerospike-core`'s `ReadPolicy`. Pass `null` where one of these is
/// expected to accept the daemon's configuration unchanged, which is what
/// nearly every call should do.
///
/// ```php
/// $policy = new Aerospike\ReadPolicy(
///     totalTimeoutMs: 500,
///     maxRetries:     1,
///     replica:        Aerospike\Replica::PreferRack,
///     readModeSc:     Aerospike\ReadModeSC::Linearize,
/// );
/// $record = $client->get($policy, $key);
/// ```
#[php_class]
#[php(name = "Aerospike\\ReadPolicy")]
#[derive(Debug, Clone, Default)]
pub struct ReadPolicy {
    shared: Shared,
}

#[php_impl]
impl ReadPolicy {
    /// Build a read policy. Every argument is optional; an omitted one leaves
    /// the daemon's own setting alone.
    ///
    /// The parameters are camelCase because **PHP named arguments use the
    /// parameter name verbatim** — `new ReadPolicy(totalTimeoutMs: 500)` — and
    /// a policy is meant to be written with named arguments, since nobody
    /// should be counting nine positions. That is also what makes them match
    /// the getters, which PHP sees as `totalTimeoutMs()`.
    ///
    /// The explicit `= null` defaults are what make skipping possible: PHP
    /// refuses a named argument that leaves an earlier parameter with no
    /// *known* default, so without these `new ReadPolicy(filter: '...')` would
    /// be an `ArgumentCountError`.
    #[php(defaults(
        totalTimeoutMs = None,
        socketTimeoutMs = None,
        maxRetries = None,
        sleepBetweenRetriesMs = None,
        replica = None,
        readModeAp = None,
        readModeSc = None,
        useCompression = None,
        filter = None,
        filterExp = None,
        txn = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn __construct(
        totalTimeoutMs: Option<Given<i64>>,
        socketTimeoutMs: Option<Given<i64>>,
        maxRetries: Option<Given<i64>>,
        sleepBetweenRetriesMs: Option<Given<i64>>,
        replica: Option<Given<Replica>>,
        readModeAp: Option<Given<ReadModeAp>>,
        readModeSc: Option<Given<ReadModeSc>>,
        useCompression: Option<Given<bool>>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
        txn: Option<Given<&Transaction>>,
    ) -> PhpResult<ReadPolicy> {
        Ok(ReadPolicy {
            shared: Shared::new(
                Given::or_none(totalTimeoutMs, "totalTimeoutMs")?,
                Given::or_none(socketTimeoutMs, "socketTimeoutMs")?,
                Given::or_none(maxRetries, "maxRetries")?,
                Given::or_none(sleepBetweenRetriesMs, "sleepBetweenRetriesMs")?,
                Given::or_none(replica, "replica")?,
                Given::or_none(readModeAp, "readModeAp")?,
                Given::or_none(readModeSc, "readModeSc")?,
                Given::or_none(useCompression, "useCompression")?,
                resolve_filter(
                    Given::or_none(filter, "filter")?,
                    Given::or_none(filterExp, "filterExp")?,
                )?,
                Given::or_none(txn, "txn")?.map(Transaction::wire_id),
            )?,
        })
    }

    /// Deadline for the whole command, retries included.
    pub fn total_timeout_ms(&self) -> Option<i64> {
        self.shared.total_timeout_ms.map(i64::from)
    }

    /// Deadline for one socket operation.
    pub fn socket_timeout_ms(&self) -> Option<i64> {
        self.shared.socket_timeout_ms.map(i64::from)
    }

    /// Retries after the first attempt.
    pub fn max_retries(&self) -> Option<i64> {
        self.shared.max_retries.map(i64::from)
    }

    /// Pause between retries.
    pub fn sleep_between_retries_ms(&self) -> Option<i64> {
        self.shared.sleep_between_retries_ms.map(i64::from)
    }

    /// Which node to prefer.
    pub fn replica(&self) -> Option<Case<Replica>> {
        self.shared.replica.map(Case)
    }

    /// AP-namespace read consistency.
    pub fn read_mode_ap(&self) -> Option<Case<ReadModeAp>> {
        self.shared.read_mode_ap.map(Case)
    }

    /// SC-namespace read consistency.
    pub fn read_mode_sc(&self) -> Option<Case<ReadModeSc>> {
        self.shared.read_mode_sc.map(Case)
    }

    /// Whether to compress a request worth compressing.
    pub fn use_compression(&self) -> Option<bool> {
        self.shared.use_compression
    }

    /// The filter as Aerospike Expression Language text.
    ///
    /// `null` when there is no filter **or** when it was built with
    /// `Aerospike\\Exp` — a built expression never was text, and inventing some
    /// would mean writing a serialiser nothing reads. Use
    /// [`filter_exp`](Self::filter_exp) to get the filter whatever its form.
    pub fn filter(&self) -> Option<String> {
        self.shared.filter_text()
    }

    /// The filter, in whichever form it was given.
    pub fn filter_exp(&self) -> Option<Expression> {
        self.shared.filter.clone().map(Expression::from_wire)
    }

    /// The id of the transaction this policy joins, or `null`.
    ///
    /// The id rather than the `Transaction` object: a policy is meant to be
    /// reusable, and holding the object would keep an unfinished transaction alive
    /// past the point its destructor should have aborted it.
    pub fn txn_id(&self) -> Option<i64> {
        self.shared.txn
    }
}

impl ReadPolicy {
    /// The contract's policy. A read has no write-only fields to carry, which
    /// is the whole point of it being a separate type.
    #[must_use]
    pub fn to_wire(&self) -> WirePolicy {
        self.shared.to_wire()
    }
}

/// Per-call settings for a write: `put`, `delete`, `touch`, `add`, `append` and
/// `prepend`.
///
/// Mirrors `aerospike-core`'s `WritePolicy`: everything a [`ReadPolicy`] has,
/// plus the fields that only mean something when a record is being changed.
/// Pass `null` where one of these is expected to accept the daemon's
/// configuration unchanged.
///
/// ```php
/// // A conditional write: only if the record is unchanged since we read it.
/// $policy = new Aerospike\WritePolicy(
///     generationPolicy: Aerospike\GenerationPolicy::ExpectGenEqual,
///     generation:       $record->generation(),
/// );
/// $client->put($policy, $key, $bins);   // result code 3 if someone got there first
/// ```
#[php_class]
#[php(name = "Aerospike\\WritePolicy")]
#[derive(Debug, Clone, Default)]
pub struct WritePolicy {
    shared: Shared,
    record_exists_action: Option<RecordExistsAction>,
    generation_policy: Option<GenerationPolicy>,
    generation: Option<u32>,
    expiration: Option<Expiration>,
    commit_level: Option<CommitLevel>,
    durable_delete: Option<bool>,
    respond_per_each_op: Option<bool>,
    send_key: Option<bool>,
}

#[php_impl]
impl WritePolicy {
    /// Build a write policy. Every argument is optional; an omitted one leaves
    /// the daemon's own setting alone.
    ///
    /// camelCase parameters and `= null` defaults, for the reasons
    /// [`ReadPolicy::__construct`] gives: PHP named arguments use the parameter
    /// name as written, seventeen positional arguments is not an API, and a
    /// named argument cannot skip a parameter whose default PHP does not know.
    #[php(defaults(
        totalTimeoutMs = None,
        socketTimeoutMs = None,
        maxRetries = None,
        sleepBetweenRetriesMs = None,
        replica = None,
        readModeAp = None,
        readModeSc = None,
        useCompression = None,
        filter = None,
        filterExp = None,
        recordExistsAction = None,
        generationPolicy = None,
        generation = None,
        expiration = None,
        commitLevel = None,
        durableDelete = None,
        respondPerEachOp = None,
        sendKey = None,
        txn = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn __construct(
        totalTimeoutMs: Option<Given<i64>>,
        socketTimeoutMs: Option<Given<i64>>,
        maxRetries: Option<Given<i64>>,
        sleepBetweenRetriesMs: Option<Given<i64>>,
        replica: Option<Given<Replica>>,
        readModeAp: Option<Given<ReadModeAp>>,
        readModeSc: Option<Given<ReadModeSc>>,
        useCompression: Option<Given<bool>>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
        txn: Option<Given<&Transaction>>,
        recordExistsAction: Option<Given<RecordExistsAction>>,
        generationPolicy: Option<Given<GenerationPolicy>>,
        generation: Option<Given<i64>>,
        expiration: Option<Given<&Expiration>>,
        commitLevel: Option<Given<CommitLevel>>,
        durableDelete: Option<Given<bool>>,
        respondPerEachOp: Option<Given<bool>>,
        sendKey: Option<Given<bool>>,
    ) -> PhpResult<WritePolicy> {
        let recordExistsAction = Given::or_none(recordExistsAction, "recordExistsAction")?;
        let generationPolicy = Given::or_none(generationPolicy, "generationPolicy")?;
        let expiration = Given::or_none(expiration, "expiration")?;
        let commitLevel = Given::or_none(commitLevel, "commitLevel")?;
        let durableDelete = Given::or_none(durableDelete, "durableDelete")?;
        let respondPerEachOp = Given::or_none(respondPerEachOp, "respondPerEachOp")?;
        let sendKey = Given::or_none(sendKey, "sendKey")?;

        let generation = Given::or_none(generation, "generation")?
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    AeroError::client(format!(
                        "generation must be between 0 and {}, but is {value}",
                        u32::MAX
                    ))
                })
            })
            .transpose()?;

        // A generation with no guard does nothing, and a guard with no
        // generation compares against zero. Either alone is a policy that
        // silently is not the one the caller wrote.
        let guarded = matches!(
            generationPolicy,
            Some(GenerationPolicy::ExpectGenEqual | GenerationPolicy::ExpectGenGreater)
        );
        if guarded && generation.is_none() {
            return Err(AeroError::client(
                "generationPolicy guards a write against a generation, so it needs one: pass \
                 generation: $record->generation() as well, or leave both unset",
            )
            .into());
        }
        if generation.is_some() && !guarded {
            return Err(AeroError::client(
                "a generation only has an effect with a generationPolicy that compares against \
                 it: pass generationPolicy: GenerationPolicy::ExpectGenEqual as well, or leave \
                 both unset",
            )
            .into());
        }

        Ok(WritePolicy {
            shared: Shared::new(
                Given::or_none(totalTimeoutMs, "totalTimeoutMs")?,
                Given::or_none(socketTimeoutMs, "socketTimeoutMs")?,
                Given::or_none(maxRetries, "maxRetries")?,
                Given::or_none(sleepBetweenRetriesMs, "sleepBetweenRetriesMs")?,
                Given::or_none(replica, "replica")?,
                Given::or_none(readModeAp, "readModeAp")?,
                Given::or_none(readModeSc, "readModeSc")?,
                Given::or_none(useCompression, "useCompression")?,
                resolve_filter(
                    Given::or_none(filter, "filter")?,
                    Given::or_none(filterExp, "filterExp")?,
                )?,
                Given::or_none(txn, "txn")?.map(Transaction::wire_id),
            )?,
            record_exists_action: recordExistsAction,
            generation_policy: generationPolicy,
            generation,
            expiration: expiration.cloned(),
            commit_level: commitLevel,
            durable_delete: durableDelete,
            respond_per_each_op: respondPerEachOp,
            send_key: sendKey,
        })
    }

    /// Deadline for the whole command, retries included.
    pub fn total_timeout_ms(&self) -> Option<i64> {
        self.shared.total_timeout_ms.map(i64::from)
    }

    /// Deadline for one socket operation.
    pub fn socket_timeout_ms(&self) -> Option<i64> {
        self.shared.socket_timeout_ms.map(i64::from)
    }

    /// Retries after the first attempt.
    pub fn max_retries(&self) -> Option<i64> {
        self.shared.max_retries.map(i64::from)
    }

    /// Pause between retries.
    pub fn sleep_between_retries_ms(&self) -> Option<i64> {
        self.shared.sleep_between_retries_ms.map(i64::from)
    }

    /// Which node to prefer. Inert on a write, which always goes to the
    /// partition master; accepted so one policy can be shared across verbs.
    pub fn replica(&self) -> Option<Case<Replica>> {
        self.shared.replica.map(Case)
    }

    /// AP-namespace read consistency, for the read a read-modify-write does.
    pub fn read_mode_ap(&self) -> Option<Case<ReadModeAp>> {
        self.shared.read_mode_ap.map(Case)
    }

    /// SC-namespace read consistency.
    pub fn read_mode_sc(&self) -> Option<Case<ReadModeSc>> {
        self.shared.read_mode_sc.map(Case)
    }

    /// Whether to compress a request worth compressing.
    pub fn use_compression(&self) -> Option<bool> {
        self.shared.use_compression
    }

    /// The filter as Aerospike Expression Language text.
    ///
    /// `null` when there is no filter **or** when it was built with
    /// `Aerospike\\Exp` — a built expression never was text, and inventing some
    /// would mean writing a serialiser nothing reads. Use
    /// [`filter_exp`](Self::filter_exp) to get the filter whatever its form.
    pub fn filter(&self) -> Option<String> {
        self.shared.filter_text()
    }

    /// The filter, in whichever form it was given.
    pub fn filter_exp(&self) -> Option<Expression> {
        self.shared.filter.clone().map(Expression::from_wire)
    }

    /// Create/update/replace semantics.
    pub fn record_exists_action(&self) -> Option<Case<RecordExistsAction>> {
        self.record_exists_action.map(Case)
    }

    /// The generation guard.
    pub fn generation_policy(&self) -> Option<Case<GenerationPolicy>> {
        self.generation_policy.map(Case)
    }

    /// The generation the guard compares against.
    pub fn generation(&self) -> Option<i64> {
        self.generation.map(i64::from)
    }

    /// The record's time-to-live after this write.
    pub fn expiration(&self) -> Option<Expiration> {
        self.expiration.clone()
    }

    /// How many replicas must commit before the server answers.
    pub fn commit_level(&self) -> Option<Case<CommitLevel>> {
        self.commit_level.map(Case)
    }

    /// Whether a delete leaves a tombstone (Enterprise only).
    pub fn durable_delete(&self) -> Option<bool> {
        self.durable_delete
    }

    /// Whether every operation reports a result, including ones that normally
    /// report nothing.
    pub fn respond_per_each_op(&self) -> Option<bool> {
        self.respond_per_each_op
    }

    /// Whether the user key is stored alongside the digest.
    pub fn send_key(&self) -> Option<bool> {
        self.send_key
    }

    /// The id of the transaction this policy joins, or `null`.
    ///
    /// The id rather than the `Transaction` object, for the reason
    /// [`ReadPolicy::txn_id`] gives: a policy is meant to be reusable, and holding
    /// the object would keep an unfinished transaction alive past the point its
    /// destructor should have aborted it.
    pub fn txn_id(&self) -> Option<i64> {
        self.shared.txn
    }
}

impl WritePolicy {
    /// The contract's policy, write-only fields included.
    #[must_use]
    pub fn to_wire(&self) -> WirePolicy {
        WirePolicy {
            record_exists_action: self.record_exists_action.map(RecordExistsAction::to_wire),
            generation_policy: self.generation_policy.map(GenerationPolicy::to_wire),
            generation: self.generation,
            expiration: self.expiration.as_ref().map(Expiration::to_wire),
            commit_level: self.commit_level.map(CommitLevel::to_wire),
            durable_delete: self.durable_delete,
            respond_per_each_op: self.respond_per_each_op,
            send_key: self.send_key,
            ..self.shared.to_wire()
        }
    }
}

/// Per-call settings for a cluster-management command.
///
/// Mirrors `aerospike-core`'s `AdminPolicy`, which carries **one** field: the
/// socket timeout for the info command the operation is built on. That is not an
/// oversight to be padded out — registering a UDF or creating an index is one
/// short info exchange with one node, so retries, replica choice and record
/// filters have nothing to act on.
///
/// ```php
/// $client->createIndexOnBin(new Aerospike\AdminPolicy(timeoutMs: 5_000), …);
/// $client->truncate(null, 'test', 'users', null);   // the daemon's default
/// ```
///
/// It is a class rather than a bare `?int` so that every command in this API takes
/// **a policy object in first position**, whichever kind of policy it is — and so
/// that a second field, if the client ever grows one, is not a change to every
/// signature.
#[php_class]
#[php(name = "Aerospike\\AdminPolicy")]
#[derive(Debug, Clone, Default)]
pub struct AdminPolicy {
    timeout_ms: Option<u32>,
}

#[php_impl]
impl AdminPolicy {
    /// Build one. An omitted timeout leaves the daemon's own setting alone.
    #[php(defaults(timeoutMs = None))]
    pub fn __construct(timeoutMs: Option<Given<i64>>) -> PhpResult<AdminPolicy> {
        Ok(AdminPolicy {
            timeout_ms: unsigned("timeoutMs", Given::or_none(timeoutMs, "timeoutMs")?)?,
        })
    }

    /// Socket timeout for the command's info exchange, or `null` for the
    /// daemon's default.
    pub fn timeout_ms(&self) -> Option<i64> {
        self.timeout_ms.map(i64::from)
    }
}

impl AdminPolicy {
    /// The timeout as the contract carries it: `None` means the daemon's
    /// default.
    #[must_use]
    pub const fn wire_timeout_ms(&self) -> Option<u32> {
        self.timeout_ms
    }

    /// What an omitted policy means.
    #[must_use]
    pub const fn none() -> Option<u32> {
        None
    }
}

/// Per-call settings for a scan or a query.
///
/// Mirrors `aerospike-core`'s `QueryPolicy`: everything a [`ReadPolicy`] has —
/// a traversal is a read of many records — plus the four settings that only make
/// sense when there is more than one record and more than one page.
///
/// ```php
/// $policy = new Aerospike\QueryPolicy(
///     maxRecords:       10_000,   // stop after this many, across every page
///     pageSize:         500,      // records per round trip
///     recordsPerSecond: 1_000,    // per node, to leave the cluster room
///     includeBinData:   false,    // keys and metadata only
/// );
/// foreach ($client->query($policy, null, $statement) as $record) { … }
/// ```
///
/// # There is no total timeout by default, and that is deliberate
///
/// A `ReadPolicy` inherits the daemon's `default_timeout` — a second, sized for a
/// single-record command. A scan of a large set legitimately takes longer than
/// that, so this one starts with **no** deadline, exactly as the Rust client's
/// `QueryPolicy` does. Set `totalTimeoutMs` if a page needs a bound; the worker's
/// own `aerospike.timeout_ms` still bounds how long it waits for one.
#[php_class]
#[php(name = "Aerospike\\QueryPolicy")]
#[derive(Debug, Clone, Default)]
pub struct QueryPolicy {
    shared: Shared,
    max_records: Option<u64>,
    records_per_second: Option<u32>,
    page_size: Option<u32>,
    include_bin_data: Option<bool>,
}

#[php_impl]
impl QueryPolicy {
    /// Build a query policy. Every argument is optional; an omitted one leaves
    /// the daemon's own setting alone.
    ///
    /// camelCase parameters and `= null` defaults, for the reasons
    /// [`ReadPolicy::__construct`] gives.
    #[php(defaults(
        totalTimeoutMs = None,
        socketTimeoutMs = None,
        maxRetries = None,
        sleepBetweenRetriesMs = None,
        replica = None,
        readModeAp = None,
        readModeSc = None,
        useCompression = None,
        filter = None,
        filterExp = None,
        maxRecords = None,
        recordsPerSecond = None,
        pageSize = None,
        includeBinData = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn __construct(
        totalTimeoutMs: Option<Given<i64>>,
        socketTimeoutMs: Option<Given<i64>>,
        maxRetries: Option<Given<i64>>,
        sleepBetweenRetriesMs: Option<Given<i64>>,
        replica: Option<Given<Replica>>,
        readModeAp: Option<Given<ReadModeAp>>,
        readModeSc: Option<Given<ReadModeSc>>,
        useCompression: Option<Given<bool>>,
        filter: Option<Given<String>>,
        filterExp: Option<Given<&Expression>>,
        maxRecords: Option<Given<i64>>,
        recordsPerSecond: Option<Given<i64>>,
        pageSize: Option<Given<i64>>,
        includeBinData: Option<Given<bool>>,
    ) -> PhpResult<QueryPolicy> {
        let max_records = Given::or_none(maxRecords, "maxRecords")?
            .map(|value| {
                u64::try_from(value).map_err(|_| {
                    AeroError::client(format!(
                        "maxRecords must not be negative, but is {value}; omit it for no limit"
                    ))
                })
            })
            .transpose()?;

        Ok(QueryPolicy {
            shared: Shared::new(
                Given::or_none(totalTimeoutMs, "totalTimeoutMs")?,
                Given::or_none(socketTimeoutMs, "socketTimeoutMs")?,
                Given::or_none(maxRetries, "maxRetries")?,
                Given::or_none(sleepBetweenRetriesMs, "sleepBetweenRetriesMs")?,
                Given::or_none(replica, "replica")?,
                Given::or_none(readModeAp, "readModeAp")?,
                Given::or_none(readModeSc, "readModeSc")?,
                Given::or_none(useCompression, "useCompression")?,
                resolve_filter(
                    Given::or_none(filter, "filter")?,
                    Given::or_none(filterExp, "filterExp")?,
                )?,
                // A query cannot join a transaction — a traversal names no keys — so
                // a QueryPolicy has nowhere to put one, and the daemon refuses a
                // transaction on a query policy rather than ignoring it.
                None,
            )?,
            max_records,
            records_per_second: unsigned(
                "recordsPerSecond",
                Given::or_none(recordsPerSecond, "recordsPerSecond")?,
            )?,
            page_size: unsigned("pageSize", Given::or_none(pageSize, "pageSize")?)?,
            include_bin_data: Given::or_none(includeBinData, "includeBinData")?,
        })
    }

    /// Deadline for the whole command, retries included.
    pub fn total_timeout_ms(&self) -> Option<i64> {
        self.shared.total_timeout_ms.map(i64::from)
    }

    /// Deadline for one socket operation.
    pub fn socket_timeout_ms(&self) -> Option<i64> {
        self.shared.socket_timeout_ms.map(i64::from)
    }

    /// Retries after the first attempt.
    pub fn max_retries(&self) -> Option<i64> {
        self.shared.max_retries.map(i64::from)
    }

    /// Pause between retries.
    pub fn sleep_between_retries_ms(&self) -> Option<i64> {
        self.shared.sleep_between_retries_ms.map(i64::from)
    }

    /// Which node to prefer.
    pub fn replica(&self) -> Option<Case<Replica>> {
        self.shared.replica.map(Case)
    }

    /// AP-namespace read consistency.
    pub fn read_mode_ap(&self) -> Option<Case<ReadModeAp>> {
        self.shared.read_mode_ap.map(Case)
    }

    /// SC-namespace read consistency.
    pub fn read_mode_sc(&self) -> Option<Case<ReadModeSc>> {
        self.shared.read_mode_sc.map(Case)
    }

    /// Whether to compress a request worth compressing.
    pub fn use_compression(&self) -> Option<bool> {
        self.shared.use_compression
    }

    /// The filter as Aerospike Expression Language text.
    ///
    /// `null` when there is no filter **or** when it was built with
    /// `Aerospike\\Exp` — a built expression never was text, and inventing some
    /// would mean writing a serialiser nothing reads. Use
    /// [`filter_exp`](Self::filter_exp) to get the filter whatever its form.
    pub fn filter(&self) -> Option<String> {
        self.shared.filter_text()
    }

    /// The filter, in whichever form it was given.
    pub fn filter_exp(&self) -> Option<Expression> {
        self.shared.filter.clone().map(Expression::from_wire)
    }

    /// How many records the whole traversal may return, or `null` for no limit.
    pub fn max_records(&self) -> Option<i64> {
        self.max_records.map(|value| i64::try_from(value).unwrap_or(i64::MAX))
    }

    /// Per-node rate limit, or `null` for unlimited.
    pub fn records_per_second(&self) -> Option<i64> {
        self.records_per_second.map(i64::from)
    }

    /// Records per page, or `null` for the daemon's configured default.
    pub fn page_size(&self) -> Option<i64> {
        self.page_size.map(i64::from)
    }

    /// Whether bins come back, or `null` for the default, which is that they do.
    pub fn include_bin_data(&self) -> Option<bool> {
        self.include_bin_data
    }
}

impl QueryPolicy {
    /// The contract's policy. As with a read, there are no write-only fields to
    /// carry.
    #[must_use]
    pub fn to_wire(&self) -> WirePolicy {
        self.shared.to_wire()
    }

    /// The record ceiling as the contract carries it: zero means no ceiling.
    #[must_use]
    pub fn wire_max_records(&self) -> u64 {
        self.max_records.unwrap_or(0)
    }

    /// The rate limit as the contract carries it: zero means unlimited.
    #[must_use]
    pub fn wire_records_per_second(&self) -> u32 {
        self.records_per_second.unwrap_or(0)
    }

    /// The page size as the contract carries it: zero means "the daemon's
    /// default", which is why an unset page size is not a page size of one.
    #[must_use]
    pub fn wire_page_size(&self) -> u32 {
        self.page_size.unwrap_or(0)
    }

    /// Whether bins come back. Unset means they do, which is both the client's
    /// default and the only answer that cannot surprise anybody.
    #[must_use]
    pub fn wire_include_bin_data(&self) -> bool {
        self.include_bin_data.unwrap_or(true)
    }
}

/// Per-call settings for the **verify** half of a commit.
///
/// Mirrors `aerospike-core`'s `TxnVerifyPolicy`. Committing a transaction is two
/// batch commands — check the version of every record the transaction read, then
/// move its writes forward — and this is the policy for the first. Pass `null` to
/// `commitWithPolicies` to accept the tuned defaults, which is what nearly every
/// commit should do.
///
/// ```php
/// // A cluster under load: give the verify batch longer before it gives up.
/// $txn->commitWithPolicies(
///     new Aerospike\TxnVerifyPolicy(totalTimeoutMs: 30_000, maxRetries: 8),
///     null,
/// );
/// ```
///
/// # The defaults are not the daemon's, and should usually be left alone
///
/// Unlike a [`ReadPolicy`], this does not start from the daemon's configured
/// timeout. The Rust client's own defaults apply — linearized SC reads, the
/// partition master, 5 retries, a 3s socket and 10s total timeout, a 1s pause
/// between retries — and they are deliberate: a verify batch that gives up leaves
/// a transaction half-finished, holding record locks, so it is tuned to keep
/// trying rather than to be quick.
///
/// # Fewer fields than a read policy, for two reasons
///
/// There is no `filter`, because this batch reads records the transaction chose
/// and a filter that skipped one would skip verifying it. And there is no `txn`:
/// the transaction being committed is the one whose method this is.
#[php_class]
#[php(name = "Aerospike\\TxnVerifyPolicy")]
#[derive(Debug, Clone, Default)]
pub struct TxnVerifyPolicy {
    shared: Shared,
}

#[php_impl]
impl TxnVerifyPolicy {
    /// Build a verify policy. Every argument is optional; an omitted one leaves
    /// the client's tuned default alone.
    ///
    /// camelCase parameters and `= null` defaults, for the reasons
    /// [`ReadPolicy::__construct`] gives.
    #[php(defaults(
        totalTimeoutMs = None,
        socketTimeoutMs = None,
        maxRetries = None,
        sleepBetweenRetriesMs = None,
        replica = None,
        readModeAp = None,
        readModeSc = None,
        useCompression = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn __construct(
        totalTimeoutMs: Option<Given<i64>>,
        socketTimeoutMs: Option<Given<i64>>,
        maxRetries: Option<Given<i64>>,
        sleepBetweenRetriesMs: Option<Given<i64>>,
        replica: Option<Given<Replica>>,
        readModeAp: Option<Given<ReadModeAp>>,
        readModeSc: Option<Given<ReadModeSc>>,
        useCompression: Option<Given<bool>>,
    ) -> PhpResult<TxnVerifyPolicy> {
        Ok(TxnVerifyPolicy {
            shared: txn_shared(
                totalTimeoutMs,
                socketTimeoutMs,
                maxRetries,
                sleepBetweenRetriesMs,
                replica,
                readModeAp,
                readModeSc,
                useCompression,
            )?,
        })
    }

    /// Deadline for the whole verify batch, retries included.
    pub fn total_timeout_ms(&self) -> Option<i64> {
        self.shared.total_timeout_ms.map(i64::from)
    }

    /// Deadline for one socket operation.
    pub fn socket_timeout_ms(&self) -> Option<i64> {
        self.shared.socket_timeout_ms.map(i64::from)
    }

    /// Retries after the first attempt.
    pub fn max_retries(&self) -> Option<i64> {
        self.shared.max_retries.map(i64::from)
    }

    /// Pause between retries.
    pub fn sleep_between_retries_ms(&self) -> Option<i64> {
        self.shared.sleep_between_retries_ms.map(i64::from)
    }

    /// Which node to prefer.
    pub fn replica(&self) -> Option<Case<Replica>> {
        self.shared.replica.map(Case)
    }

    /// AP-namespace read consistency.
    pub fn read_mode_ap(&self) -> Option<Case<ReadModeAp>> {
        self.shared.read_mode_ap.map(Case)
    }

    /// SC-namespace read consistency.
    pub fn read_mode_sc(&self) -> Option<Case<ReadModeSc>> {
        self.shared.read_mode_sc.map(Case)
    }

    /// Whether to compress a request worth compressing.
    pub fn use_compression(&self) -> Option<bool> {
        self.shared.use_compression
    }
}

impl TxnVerifyPolicy {
    /// The contract's policy.
    #[must_use]
    pub fn to_wire(&self) -> WirePolicy {
        self.shared.to_wire()
    }
}

/// Per-call settings for the **roll** half of a commit or an abort.
///
/// Mirrors `aerospike-core`'s `TxnRollPolicy`: the batch that moves a
/// transaction's writes forward when it commits, or rolls them back when it
/// aborts. Pass `null` to accept the tuned defaults.
///
/// ```php
/// $roll = new Aerospike\TxnRollPolicy(maxRetries: 10);
/// $txn->commitWithPolicies(null, $roll);   // or $txn->abortWithPolicy($roll)
/// ```
///
/// # This is the one policy an abort takes, and the reason is the asymmetry
///
/// An abort has nothing to verify — it is discarding the writes, so the versions
/// the transaction read no longer matter — so `abortWithPolicy` takes one policy
/// where `commitWithPolicies` takes two.
///
/// # It writes, and yet it has no write fields
///
/// Rolling a transaction does change records, so the absence of `expiration`,
/// `recordExistsAction` and the rest looks odd. It is not: this batch does not
/// write records the caller described, it moves writes the transaction already
/// made. A TTL or a create/replace guard would have nothing to apply to, and the
/// daemon refuses one rather than ignoring it.
#[php_class]
#[php(name = "Aerospike\\TxnRollPolicy")]
#[derive(Debug, Clone, Default)]
pub struct TxnRollPolicy {
    shared: Shared,
}

#[php_impl]
impl TxnRollPolicy {
    /// Build a roll policy. Every argument is optional; an omitted one leaves
    /// the client's tuned default alone.
    ///
    /// camelCase parameters and `= null` defaults, for the reasons
    /// [`ReadPolicy::__construct`] gives.
    #[php(defaults(
        totalTimeoutMs = None,
        socketTimeoutMs = None,
        maxRetries = None,
        sleepBetweenRetriesMs = None,
        replica = None,
        readModeAp = None,
        readModeSc = None,
        useCompression = None
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn __construct(
        totalTimeoutMs: Option<Given<i64>>,
        socketTimeoutMs: Option<Given<i64>>,
        maxRetries: Option<Given<i64>>,
        sleepBetweenRetriesMs: Option<Given<i64>>,
        replica: Option<Given<Replica>>,
        readModeAp: Option<Given<ReadModeAp>>,
        readModeSc: Option<Given<ReadModeSc>>,
        useCompression: Option<Given<bool>>,
    ) -> PhpResult<TxnRollPolicy> {
        Ok(TxnRollPolicy {
            shared: txn_shared(
                totalTimeoutMs,
                socketTimeoutMs,
                maxRetries,
                sleepBetweenRetriesMs,
                replica,
                readModeAp,
                readModeSc,
                useCompression,
            )?,
        })
    }

    /// Deadline for the whole roll batch, retries included.
    pub fn total_timeout_ms(&self) -> Option<i64> {
        self.shared.total_timeout_ms.map(i64::from)
    }

    /// Deadline for one socket operation.
    pub fn socket_timeout_ms(&self) -> Option<i64> {
        self.shared.socket_timeout_ms.map(i64::from)
    }

    /// Retries after the first attempt.
    pub fn max_retries(&self) -> Option<i64> {
        self.shared.max_retries.map(i64::from)
    }

    /// Pause between retries.
    pub fn sleep_between_retries_ms(&self) -> Option<i64> {
        self.shared.sleep_between_retries_ms.map(i64::from)
    }

    /// Which node to prefer.
    pub fn replica(&self) -> Option<Case<Replica>> {
        self.shared.replica.map(Case)
    }

    /// AP-namespace read consistency.
    pub fn read_mode_ap(&self) -> Option<Case<ReadModeAp>> {
        self.shared.read_mode_ap.map(Case)
    }

    /// SC-namespace read consistency.
    pub fn read_mode_sc(&self) -> Option<Case<ReadModeSc>> {
        self.shared.read_mode_sc.map(Case)
    }

    /// Whether to compress a request worth compressing.
    pub fn use_compression(&self) -> Option<bool> {
        self.shared.use_compression
    }
}

impl TxnRollPolicy {
    /// The contract's policy.
    #[must_use]
    pub fn to_wire(&self) -> WirePolicy {
        self.shared.to_wire()
    }
}

/// The eight fields a transaction-finishing policy has.
///
/// Its own function rather than a `Shared::new` call in each constructor because
/// the two policies must pass the same `None` for `filter` and `txn` — the point
/// is that neither policy *has* those fields, and a second copy of the call could
/// quietly grow one.
#[allow(clippy::too_many_arguments)]
fn txn_shared(
    total_timeout_ms: Option<Given<i64>>,
    socket_timeout_ms: Option<Given<i64>>,
    max_retries: Option<Given<i64>>,
    sleep_between_retries_ms: Option<Given<i64>>,
    replica: Option<Given<Replica>>,
    read_mode_ap: Option<Given<ReadModeAp>>,
    read_mode_sc: Option<Given<ReadModeSc>>,
    use_compression: Option<Given<bool>>,
) -> PhpResult<Shared> {
    Ok(Shared::new(
        Given::or_none(total_timeout_ms, "totalTimeoutMs")?,
        Given::or_none(socket_timeout_ms, "socketTimeoutMs")?,
        Given::or_none(max_retries, "maxRetries")?,
        Given::or_none(sleep_between_retries_ms, "sleepBetweenRetriesMs")?,
        Given::or_none(replica, "replica")?,
        Given::or_none(read_mode_ap, "readModeAp")?,
        Given::or_none(read_mode_sc, "readModeSc")?,
        Given::or_none(use_compression, "useCompression")?,
        // No filter: this batch reads or writes the records the transaction chose,
        // and skipping one would mean not verifying or not rolling it.
        None,
        // No transaction: the one being finished is the one whose method this is.
        None,
    )?)
}

/// The fields every policy class has, in one place so they cannot drift.
///
/// Not exposed to PHP: PHP sees concrete classes, because an abstract base would
/// let a caller pass a policy that is none of them and leave the extension to
/// work out which fields to honour.
#[derive(Debug, Clone, Default)]
struct Shared {
    total_timeout_ms: Option<u32>,
    socket_timeout_ms: Option<u32>,
    max_retries: Option<u32>,
    sleep_between_retries_ms: Option<u32>,
    replica: Option<Replica>,
    read_mode_ap: Option<ReadModeAp>,
    read_mode_sc: Option<ReadModeSc>,
    use_compression: Option<bool>,
    /// The filter, in whichever of the three forms the caller used.
    ///
    /// A `WireExpression` rather than a string because a filter is an
    /// expression, and the builder in `Aerospike\\Exp` produces one that never
    /// was text. The `filter:` constructor parameter still takes text — that is
    /// the shorthand — and `filterExp:` takes any of the three.
    filter: Option<WireExpression>,
    /// The transaction this command joins, by id.
    ///
    /// The id rather than a reference to the `Transaction` object, because a
    /// policy is meant to be reusable and holding the object would tie the two
    /// lifetimes together — a policy kept in a static would keep a transaction
    /// alive, and the destructor that aborts an unfinished one would never run.
    txn: Option<i64>,
}

/// The filter a policy carries, from the two ways of naming one.
///
/// `filter:` is text and `filterExp:` is a built (or packed) expression. They set
/// the same wire field, so naming both is refused rather than one silently
/// winning: which one won would be invisible, and the two say different things.
///
/// # Errors
/// [`AeroError`] when both are given, or when the text is blank — a filter of
/// whitespace filters nothing, which is never what someone wrote on purpose.
fn resolve_filter(
    text: Option<String>,
    built: Option<&Expression>,
) -> AeroResult<Option<WireExpression>> {
    match (text, built) {
        (Some(_), Some(_)) => Err(AeroError::client(
            "filter: and filterExp: both name the policy's filter, so only one of them can be \
             given. filter: takes Aerospike Expression Language text; filterExp: takes an \
             expression from Aerospike\\Exp, Expression::ael() or Expression::base64()",
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

impl Shared {
    #[allow(clippy::too_many_arguments)]
    fn new(
        total_timeout_ms: Option<i64>,
        socket_timeout_ms: Option<i64>,
        max_retries: Option<i64>,
        sleep_between_retries_ms: Option<i64>,
        replica: Option<Replica>,
        read_mode_ap: Option<ReadModeAp>,
        read_mode_sc: Option<ReadModeSc>,
        use_compression: Option<bool>,
        filter: Option<WireExpression>,
        txn: Option<i64>,
    ) -> AeroResult<Shared> {
        Ok(Shared {
            total_timeout_ms: unsigned("totalTimeoutMs", total_timeout_ms)?,
            socket_timeout_ms: unsigned("socketTimeoutMs", socket_timeout_ms)?,
            max_retries: unsigned("maxRetries", max_retries)?,
            sleep_between_retries_ms: unsigned("sleepBetweenRetriesMs", sleep_between_retries_ms)?,
            replica,
            read_mode_ap,
            read_mode_sc,
            use_compression,
            filter,
            txn,
        })
    }

    fn to_wire(&self) -> WirePolicy {
        WirePolicy {
            total_timeout_ms: self.total_timeout_ms,
            socket_timeout_ms: self.socket_timeout_ms,
            max_retries: self.max_retries,
            sleep_between_retries_ms: self.sleep_between_retries_ms,
            replica: self.replica.map(Replica::to_wire),
            read_mode_ap: self.read_mode_ap.map(ReadModeAp::to_wire),
            read_mode_sc: self.read_mode_sc.map(ReadModeSc::to_wire),
            use_compression: self.use_compression,
            filter: self.filter.clone(),
            txn: self.txn,
            ..WirePolicy::default()
        }
    }

    /// The filter as text, or `None` when there is none or it was built.
    fn filter_text(&self) -> Option<String> {
        match &self.filter {
            Some(WireExpression::Ael(text)) => Some(text.clone()),
            Some(_) | None => None,
        }
    }
}

/// Narrow a PHP int to the unsigned field the contract carries.
///
/// PHP has one integer type and it is signed, so this is the only place a
/// negative count can be caught — and it must be caught: a negative timeout
/// wrapping into four billion milliseconds is a call that never comes back.
fn unsigned(name: &str, value: Option<i64>) -> AeroResult<Option<u32>> {
    value
        .map(|value| {
            u32::try_from(value).map_err(|_| {
                AeroError::client(format!(
                    "{name} must be between 0 and {}, but is {value}",
                    u32::MAX
                ))
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::policy::{
        WireCommitLevel, WireGenerationPolicy, WireReadModeAp, WireReadModeSc,
        WireRecordExistsAction, WireReplica,
    };

    fn shared() -> Shared {
        Shared::new(
            Some(500),
            Some(250),
            Some(1),
            Some(10),
            Some(Replica::PreferRack),
            Some(ReadModeAp::All),
            Some(ReadModeSc::Linearize),
            Some(true),
            Some(WireExpression::Ael("$.age > 21".into())),
            Some(-7),
        )
        .unwrap()
    }

    #[test]
    fn a_read_policy_carries_the_shared_fields_and_nothing_else() {
        let wire = ReadPolicy { shared: shared() }.to_wire();

        assert_eq!(wire.total_timeout_ms, Some(500));
        assert_eq!(wire.socket_timeout_ms, Some(250));
        assert_eq!(wire.max_retries, Some(1));
        assert_eq!(wire.sleep_between_retries_ms, Some(10));
        assert_eq!(wire.replica, Some(WireReplica::PreferRack));
        assert_eq!(wire.read_mode_ap, Some(WireReadModeAp::All));
        assert_eq!(wire.read_mode_sc, Some(WireReadModeSc::Linearize));
        assert_eq!(wire.use_compression, Some(true));
        assert_eq!(
            wire.filter,
            Some(WireExpression::Ael("$.age > 21".into()))
        );

        // The point of the split: there is no way to reach a write-only field
        // through a read policy, so the daemon never has to reject one.
        assert!(
            wire.write_only_fields_set().is_empty(),
            "a ReadPolicy must not be able to set a write-only field: {:?}",
            wire.write_only_fields_set()
        );
    }

    #[test]
    fn a_write_policy_carries_every_field_the_contract_has() {
        let policy = WritePolicy {
            shared: shared(),
            record_exists_action: Some(RecordExistsAction::CreateOnly),
            generation_policy: Some(GenerationPolicy::ExpectGenEqual),
            generation: Some(4),
            expiration: Some(Expiration::of_seconds(3_600).unwrap()),
            commit_level: Some(CommitLevel::CommitMaster),
            durable_delete: Some(true),
            respond_per_each_op: Some(true),
            send_key: Some(false),
        };
        let wire = policy.to_wire();

        // The shared half must survive the merge — a `..` spread that put the
        // struct update in the wrong order would silently drop it.
        assert_eq!(wire.total_timeout_ms, Some(500));
        assert_eq!(
            wire.filter,
            Some(WireExpression::Ael("$.age > 21".into()))
        );

        assert_eq!(
            wire.record_exists_action,
            Some(WireRecordExistsAction::CreateOnly)
        );
        assert_eq!(
            wire.generation_policy,
            Some(WireGenerationPolicy::ExpectGenEqual)
        );
        assert_eq!(wire.generation, Some(4));
        assert_eq!(wire.expiration, Some(WireExpiration::Seconds(3_600)));
        assert_eq!(wire.commit_level, Some(WireCommitLevel::CommitMaster));
        assert_eq!(wire.durable_delete, Some(true));
        assert_eq!(wire.respond_per_each_op, Some(true));
        assert_eq!(wire.send_key, Some(false));

        // Every write-only field the contract knows about must be reachable; if
        // it grows one, this notices.
        assert_eq!(
            wire.write_only_fields_set(),
            vec![
                "record_exists_action",
                "generation_policy",
                "generation",
                "expiration",
                "commit_level",
                "durable_delete",
                "respond_per_each_op",
            ]
        );
    }

    #[test]
    fn an_unset_policy_overrides_nothing() {
        assert!(ReadPolicy::default().to_wire().is_empty());
        assert!(WritePolicy::default().to_wire().is_empty());
        assert!(TxnVerifyPolicy::default().to_wire().is_empty());
        assert!(TxnRollPolicy::default().to_wire().is_empty());
    }

    /// The two transaction-finishing policies carry the shared fields and are
    /// *unable* to carry the two that would be wrong here — a filter that skipped
    /// a record, or a second transaction to be inside.
    #[test]
    fn a_transaction_policy_cannot_carry_a_filter_or_a_transaction() {
        let built = txn_shared(
            Some(Given::Value(30_000)),
            Some(Given::Value(3_000)),
            Some(Given::Value(8)),
            Some(Given::Value(1_000)),
            Some(Given::Value(Replica::Master)),
            None,
            Some(Given::Value(ReadModeSc::Linearize)),
            Some(Given::Value(false)),
        )
        .unwrap();

        for wire in [
            TxnVerifyPolicy {
                shared: built.clone(),
            }
            .to_wire(),
            TxnRollPolicy { shared: built }.to_wire(),
        ] {
            assert_eq!(wire.total_timeout_ms, Some(30_000));
            assert_eq!(wire.socket_timeout_ms, Some(3_000));
            assert_eq!(wire.max_retries, Some(8));
            assert_eq!(wire.sleep_between_retries_ms, Some(1_000));
            assert_eq!(wire.replica, Some(WireReplica::Master));
            assert_eq!(wire.read_mode_ap, None);
            assert_eq!(wire.read_mode_sc, Some(WireReadModeSc::Linearize));
            assert_eq!(wire.use_compression, Some(false));

            // Neither is reachable from these classes, so the daemon's refusal of
            // them is a backstop rather than something a caller can trip over.
            assert_eq!(wire.filter, None);
            assert_eq!(wire.txn, None);
            assert!(wire.write_only_fields_set().is_empty());
        }
    }

    #[test]
    fn the_expiration_sentinels_are_named_and_negatives_are_refused() {
        assert_eq!(
            Expiration::of_seconds(3_600).unwrap().to_wire(),
            WireExpiration::Seconds(3_600)
        );
        assert_eq!(Expiration::never().to_wire(), WireExpiration::Never);
        assert_eq!(
            Expiration::namespace_default().to_wire(),
            WireExpiration::NamespaceDefault
        );
        assert_eq!(Expiration::dont_update().to_wire(), WireExpiration::DontUpdate);

        assert_eq!(Expiration::of_seconds(0).unwrap().to_seconds(), Some(0));
        assert_eq!(Expiration::never().to_seconds(), None);

        // -1 is "never" in some clients' wire encoding. Reading it that way
        // here would silently make a record immortal.
        let error = Expiration::of_seconds(-1).expect_err("a negative TTL must be refused");
        assert!(error.to_string().contains("never()"), "{error}");
    }

    #[test]
    fn a_negative_timeout_is_refused_rather_than_wrapped() {
        let error = unsigned("totalTimeoutMs", Some(-1)).expect_err("must be refused");
        assert!(error.to_string().contains("totalTimeoutMs"), "{error}");

        let too_big =
            unsigned("maxRetries", Some(i64::from(u32::MAX) + 1)).expect_err("must be refused");
        assert!(too_big.to_string().contains("4294967295"), "{too_big}");

        assert_eq!(unsigned("x", Some(0)).unwrap(), Some(0));
        assert_eq!(unsigned("x", None).unwrap(), None);
    }

    /// The rule moved to `resolve_filter` when the field grew from text to a
    /// whole expression, and it still has to hold: a filter of whitespace filters
    /// nothing, which nobody writes on purpose.
    #[test]
    fn a_filter_that_filters_nothing_is_refused() {
        let error = resolve_filter(Some("   ".into()), None)
            .expect_err("a blank filter must be refused");
        assert!(error.to_string().contains("filters nothing"), "{error}");

        assert_eq!(
            resolve_filter(Some("$.age > 21".into()), None).unwrap(),
            Some(WireExpression::Ael("$.age > 21".into()))
        );
        assert_eq!(resolve_filter(None, None).unwrap(), None);
    }

    /// `filter:` and `filterExp:` set one wire field. Naming both is refused,
    /// because whichever won would be invisible and they say different things.
    #[test]
    fn the_two_filter_spellings_cannot_both_be_given() {
        let built = Expression::from_tree(aerospike_php_ipc::exp::WireExp::Value(
            aerospike_php_ipc::WireValue::Bool(true),
        ));
        assert_eq!(
            resolve_filter(None, Some(&built)).unwrap(),
            Some(built.to_wire())
        );

        let error = resolve_filter(Some("$.age > 21".into()), Some(&built))
            .expect_err("both spellings name one filter");
        assert!(error.to_string().contains("only one of them"), "{error}");
        assert!(error.to_string().contains("filterExp"), "{error}");
    }

    /// A built filter has no source text, and `filter()` says so with `null`
    /// rather than inventing something — but `filterExp()` still returns it.
    #[test]
    fn a_built_filter_reads_back_as_an_expression_not_as_text() {
        let built = Expression::from_tree(aerospike_php_ipc::exp::WireExp::Value(
            aerospike_php_ipc::WireValue::Bool(true),
        ));
        let shared = Shared::new(
            None, None, None, None, None, None, None, None,
            resolve_filter(None, Some(&built)).unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(shared.filter_text(), None, "a tree never was text");
        assert_eq!(shared.filter, Some(built.to_wire()));

        // Text still reads back as text.
        let shared = Shared::new(
            None, None, None, None, None, None, None, None,
            resolve_filter(Some("$.age > 21".into()), None).unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(shared.filter_text().as_deref(), Some("$.age > 21"));
    }
}
