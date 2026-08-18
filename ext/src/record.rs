// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

// `Key::__construct`'s `userKey` parameter is camelCase because PHP named
// arguments use the parameter name exactly as written; see `crate::policy` for
// why the lint cannot be scoped more tightly than the module.
#![allow(non_snake_case)]

//! The things a command names and returns: [`Key`], [`Bin`], [`Bins`],
//! [`Record`] and [`DaemonInfo`].
//!
//! Each one is a type in `aerospike-core`, so each one is a class here. What
//! that buys is not ceremony — it is that a mistake is caught where it was
//! made:
//!
//! - `new Key("test", "users", "alice")` validates the key *once*, at the point
//!   the caller wrote it, and can then be passed to any number of commands.
//!   The alternative — three loose arguments per call — has to be re-validated
//!   every time and cannot be given a name.
//! - `new Bin("age", 30)` converts the value immediately, so a value PHP cannot
//!   express is reported against the bin that holds it rather than somewhere
//!   inside a nested array.
//! - [`Bins`] makes "which bins to read" a closed choice. As a nullable array
//!   it had a trap in it: `[]` meant *no bins*, so an empty list of bin names
//!   computed at runtime silently turned a read into a metadata-only round
//!   trip. `Bins::none()` says that on purpose and `Bins::some([])` is refused.
//! - [`Record`] and [`DaemonInfo`] are only ever produced by the daemon, so
//!   neither has a constructor: `new Aerospike\Record()` is a `TypeError`
//!   rather than an empty record that looks like a real one.

use aerospike_php_ipc::{BinSelector, ReplyHeader, RecordBody, WireKey, WireValue};
use ext_php_rs::boxed::ZBox;
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendHashTable, Zval};

use crate::arg;
use crate::error::{php_internal, AeroError, AeroResult};
use crate::value::{self, Path};

/// Which record a command is aimed at: a namespace, a set, and a user key.
///
/// The user key is an `int`, a `string`, or an `Aerospike\Blob` for a binary
/// key. Nothing else: a float key would compare by a representation the server
/// does not define an ordering for, and an array has no digest.
///
/// ```php
/// $key = new Aerospike\Key("test", "users", "alice");
/// $key = new Aerospike\Key("test", "users", 42);
/// $key = new Aerospike\Key("test", "", "in-the-null-set");
/// ```
///
/// The digest is **not** available here. The daemon computes it, because the
/// daemon is what builds the real key; exposing a digest this side would mean
/// reimplementing RIPEMD-160 in the extension and having two implementations
/// that must agree.
#[php_class]
#[php(name = "Aerospike\\Key")]
#[derive(Debug, Clone)]
pub struct Key {
    namespace: String,
    set: String,
    user_key: WireKey,
    /// The server's 20-byte digest, for a key that came back from a scan or a
    /// query.
    ///
    /// `None` for a key the caller constructed: the digest is a RIPEMD-160 of the
    /// namespace, set and key, computed by `aerospike-core` inside the **daemon**
    /// — this extension does not link the client and so cannot compute one. That
    /// is why [`Key::digest`] is nullable rather than always available, and it is
    /// the one place the 1.x client could answer and this cannot.
    digest: Option<Vec<u8>>,
}

#[php_impl]
impl Key {
    /// Name a record.
    ///
    /// An empty `$set` is the null set, which is legal. An empty `$namespace`
    /// is not: every record lives in one, and the server would reject the
    /// command with a result code that does not say which argument was blank.
    pub fn __construct(namespace: String, set: String, userKey: &Zval) -> PhpResult<Key> {
        if namespace.trim().is_empty() {
            return Err(AeroError::client(
                "a Key needs a namespace; \"\" is not one. The empty string is only meaningful for \
                 the set, where it means the null set",
            )
            .into());
        }
        Ok(Key {
            namespace,
            set,
            user_key: value::zval_to_wire_key(userKey)?,
            // A key built here has no digest: computing one needs
            // `aerospike-core`'s RIPEMD-160, which lives in the daemon.
            digest: None,
        })
    }

    /// The server's 20-byte digest, for a key a scan or query returned.
    ///
    /// `null` for a key the caller constructed — see the field's own note. The
    /// digest is what a scan identifies a record by whether or not its user key
    /// was stored, so this is the identity that always exists *on the server*;
    /// it just is not computed on this side.
    pub fn digest(&self) -> Option<crate::types::Blob> {
        self.digest
            .as_ref()
            .map(|bytes| crate::types::Blob::of(bytes.clone()))
    }

    /// Which of the 4096 partitions this key falls in, or `null`.
    ///
    /// Derived from the digest, so it is available exactly when [`Key::digest`]
    /// is: the server takes the first two bytes little-endian and masks off the
    /// top four bits. Useful for splitting a scan across workers by partition.
    pub fn partition_id(&self) -> Option<i64> {
        self.digest.as_ref().and_then(|bytes| {
            let (low, high) = (*bytes.first()?, *bytes.get(1)?);
            Some(i64::from(u16::from_le_bytes([low, high]) & 0x0FFF))
        })
    }

    /// The namespace.
    pub fn namespace(&self) -> String {
        self.namespace.clone()
    }

    /// The set name; `""` for the null set.
    pub fn set_name(&self) -> String {
        self.set.clone()
    }

    /// The user key, as the `int`, `string` or `Aerospike\Blob` it was given
    /// as.
    pub fn user_key(&self) -> PhpResult<Zval> {
        let value = match &self.user_key {
            WireKey::Int(number) => WireValue::Int(*number),
            WireKey::Str(text) => WireValue::Str(text.clone()),
            WireKey::Blob(bytes) => WireValue::Blob(bytes.clone()),
        };
        Ok(value::wire_to_zval(&value)?)
    }

    /// `namespace:set:key`, for logs and test failures.
    ///
    /// A binary key is rendered as its length rather than its bytes, which are
    /// not printable and would corrupt whatever is reading the log.
    pub fn __to_string(&self) -> String {
        let user_key = match &self.user_key {
            WireKey::Int(number) => number.to_string(),
            WireKey::Str(text) => text.clone(),
            WireKey::Blob(bytes) => format!("<{} bytes>", bytes.len()),
        };
        format!("{}:{}:{}", self.namespace, self.set, user_key)
    }

    // ===== the 1.x client's spellings ======================================

    /// Alias of [`Key::namespace`], as the 1.x client spelled it.
    pub fn get_namespace(&self) -> String {
        self.namespace()
    }

    /// Alias of [`Key::set_name`], as the 1.x client spelled it — `setname`, one
    /// word.
    ///
    /// There is deliberately no `getSetName()` beside it: **PHP method names are
    /// case-insensitive**, so the two would be the same method and the second
    /// registration fails at load time with "duplicate name". This client's own
    /// spelling is [`Key::set_name`], which reads `setName()`.
    pub fn get_setname(&self) -> String {
        self.set_name()
    }

    /// Alias of [`Key::user_key`].
    pub fn get_value(&self) -> PhpResult<Zval> {
        self.user_key()
    }

    /// Alias of [`Key::digest`], as the 1.x client spelled it — but see the
    /// difference in [`Key::digest`]: this is `null` for a key built here.
    pub fn get_digest(&self) -> Option<crate::types::Blob> {
        self.digest()
    }

    /// Alias of [`Key::digest`]. The 1.x client returned an array of bytes; this
    /// returns an `Aerospike\Blob`, whose `(string)` cast is those bytes.
    pub fn get_digest_bytes(&self) -> Option<crate::types::Blob> {
        self.digest()
    }

    /// Alias of [`Key::partition_id`].
    pub fn get_partition_id(&self) -> Option<i64> {
        self.partition_id()
    }

    /// Property access, for the 1.x client's `$key->namespace` style.
    ///
    /// Read-only by construction, for the reason [`Record::__get`] gives.
    pub fn __get(&self, name: &str) -> PhpResult<Zval> {
        match name {
            "namespace" => Ok(zval_of_string(self.namespace())),
            // Both spellings: `setname` is the 1.x property, `set` reads better.
            "setname" | "set" => Ok(zval_of_string(self.set_name())),
            "value" => self.user_key(),
            "digest" => match self.digest() {
                None => Ok(Zval::new()),
                Some(digest) => into_zval(digest),
            },
            "partition_id" | "partitionId" => {
                Ok(self.partition_id().map_or_else(Zval::new, zval_of_int))
            }
            other => Err(AeroError::client(format!(
                "Aerospike\\Key has no property ${other}. It has $namespace, $setname, $value, \
                 $digest and $partition_id — or call namespace(), setName(), userKey(), digest() \
                 and partitionId(), which is the spelling this client documents"
            ))
            .into()),
        }
    }
}

impl Key {
    /// Rebuild one from the parts a batch row carried.
    ///
    /// The key came from PHP in the first place, so it needs no re-validation —
    /// this is the same key travelling back out with its answer.
    #[must_use]
    pub fn from_parts(namespace: String, set: String, user_key: WireKey) -> Key {
        Key {
            namespace,
            set,
            user_key,
            digest: None,
        }
    }

    /// A key a scan or query returned, digest included.
    #[must_use]
    pub fn from_wire(namespace: String, set: String, user_key: WireKey, digest: Vec<u8>) -> Key {
        Key {
            namespace,
            set,
            user_key,
            digest: Some(digest),
        }
    }

    /// The parts the contract's `Target` needs.
    #[must_use]
    pub fn parts(&self) -> (String, String, WireKey) {
        (
            self.namespace.clone(),
            self.set.clone(),
            self.user_key.clone(),
        )
    }
}

/// One bin: a name and a value.
///
/// The value is converted when the bin is constructed, not when the command
/// runs, so `new Bin("avatar", $resource)` fails naming `avatar` instead of
/// failing later with a position inside a list of bins.
///
/// ```php
/// $client->put(null, $key, [
///     new Aerospike\Bin("name", "Alice"),
///     new Aerospike\Bin("age", 30),
///     new Aerospike\Bin("tags", ["a", "b"]),          // a list
///     new Aerospike\Bin("prefs", ["theme" => "dark"]), // a map
///     new Aerospike\Bin("retired", null),              // deletes the bin
/// ]);
/// ```
#[php_class]
#[php(name = "Aerospike\\Bin")]
#[derive(Debug, Clone)]
pub struct Bin {
    name: String,
    value: WireValue,
}

#[php_impl]
impl Bin {
    /// Name a bin and its value.
    ///
    /// A `null` value is not an omission: writing it **deletes** the bin.
    pub fn __construct(name: String, value: &Zval) -> PhpResult<Bin> {
        let converted = value::zval_to_wire(value, &Path::bin(&name))?;
        Ok(Bin {
            name,
            value: converted,
        })
    }

    /// The bin name.
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// The value, converted back to PHP.
    pub fn value(&self) -> PhpResult<Zval> {
        Ok(value::wire_to_zval(&self.value)?)
    }
}

impl Bin {
    /// The pair the contract's bodies carry.
    #[must_use]
    pub fn to_wire(&self) -> (String, WireValue) {
        (self.name.clone(), self.value.clone())
    }

    /// The bin name, for error messages that have to name it.
    #[must_use]
    pub fn name_str(&self) -> &str {
        &self.name
    }

    /// The value, for the checks `add`, `append` and `prepend` apply.
    #[must_use]
    pub const fn wire_value(&self) -> &WireValue {
        &self.value
    }
}

/// Which bins a read returns: all of them, none of them, or a named few.
///
/// `aerospike-core` has an enum for this and so does this class, for one
/// reason: the three cases are genuinely different requests, and the shape
/// that used to express them — a nullable array — could not tell "no bins"
/// apart from "an empty list of bins I computed". A read that quietly returns
/// no bins is a bug that surfaces as missing data much later.
///
/// ```php
/// $client->get(null, $key);                                // every bin
/// $client->get(null, $key, Aerospike\Bins::some(["name", "age"]));
/// $client->get(null, $key, Aerospike\Bins::none());        // metadata only
/// ```
#[php_class]
#[php(name = "Aerospike\\Bins")]
#[derive(Debug, Clone)]
pub struct Bins {
    selector: BinSelector,
}

#[php_impl]
impl Bins {
    /// Every bin in the record. The same as passing no selector at all.
    pub fn all() -> Bins {
        Bins {
            selector: BinSelector::All,
        }
    }

    /// No bins: generation and TTL only.
    ///
    /// Cheaper than reading bins and discarding them, and the reason `exists()`
    /// is not the only way to ask a metadata question.
    pub fn none() -> Bins {
        Bins {
            selector: BinSelector::None,
        }
    }

    /// Just these bins, given as a list of names.
    ///
    /// A list rather than PHP variadics: an `int|string ...$names` variadic
    /// cannot be typed by the engine — the only variadic form available here is
    /// untyped — so an array keeps every name checked as a `string`.
    ///
    /// Naming no bins is refused rather than treated as [`Bins::none`]: an
    /// empty list here is overwhelmingly a list that was *computed* and came
    /// out empty, and silently answering with no bins would hide that.
    pub fn some(names: Vec<String>) -> PhpResult<Bins> {
        Ok(Bins::only(names)?)
    }

    /// The named bins, or `null` for [`Bins::all`] and [`Bins::none`].
    pub fn names(&self) -> Option<Vec<String>> {
        match &self.selector {
            BinSelector::Only(names) => Some(names.clone()),
            BinSelector::All | BinSelector::None => None,
        }
    }

    /// Whether this asks for every bin.
    pub fn is_all(&self) -> bool {
        matches!(self.selector, BinSelector::All)
    }

    /// Whether this asks for no bins at all.
    pub fn is_none(&self) -> bool {
        matches!(self.selector, BinSelector::None)
    }
}

impl Bins {
    /// [`Bins::some`] without the PHP wrapping, so the rules it enforces are
    /// testable without a running PHP.
    ///
    /// # Errors
    /// `AeroError` for an empty selection, or a name that is blank.
    pub fn only(names: Vec<String>) -> AeroResult<Bins> {
        if names.is_empty() {
            return Err(AeroError::client(
                "Bins::some() needs at least one bin name; if you meant to read no bins at all, \
                 say so with Bins::none()",
            ));
        }
        if let Some(position) = names.iter().position(|name| name.trim().is_empty()) {
            return Err(AeroError::client(format!(
                "Bins::some() was given an empty bin name at position {position}; a bin has a name"
            )));
        }
        Ok(Bins {
            selector: BinSelector::Only(names),
        })
    }

    /// The contract's selector.
    #[must_use]
    pub fn to_wire(&self) -> BinSelector {
        self.selector.clone()
    }

    /// What an omitted selector means: every bin.
    ///
    /// Exists so a caller that passed no selector at all does not have to
    /// allocate a PHP object to say the ordinary thing.
    #[must_use]
    pub const fn all_wire() -> BinSelector {
        BinSelector::All
    }
}

/// One record as the server returned it.
///
/// Produced by a read; there is deliberately no constructor, because a
/// `Record` that no server produced would report a generation and a TTL that
/// mean nothing.
///
/// ```php
/// $record = $client->get(null, $key);
/// if ($record !== null) {
///     $record->bins();          // ["name" => "Alice", "age" => 30]
///     $record->bin("age");      // 30, or null if the bin is not there
///     $record->generation();    // 1
///     $record->ttl();           // 2592000, or null when it never expires
/// }
/// ```
#[php_class]
#[php(name = "Aerospike\\Record")]
#[derive(Debug, Clone)]
pub struct Record {
    /// In the order the server returned them.
    bins: Vec<(String, WireValue)>,
    generation: u32,
    /// `None` means the record never expires.
    ttl: Option<u32>,
    /// The user key, for a record a scan or query returned whose key was
    /// *stored*. A single-record read has none: the caller already has the key
    /// it asked with, and the server does not send it back.
    key: Option<Key>,
    /// The server's digest, for a record a scan or query returned. Always
    /// present for one of those, and never for a single-record read.
    digest: Option<Vec<u8>>,
}

#[php_impl]
impl Record {
    /// Every bin, as a name-to-value map.
    ///
    /// In the server's order, which for a record read as a whole is its own
    /// bin order and not the order they were written in.
    pub fn bins(&self) -> PhpResult<ZBox<ZendHashTable>> {
        let mut table = ZendHashTable::with_capacity(
            u32::try_from(self.bins.len()).unwrap_or(u32::MAX),
        );
        for (name, wire) in &self.bins {
            table
                .insert(name.as_str(), value::wire_to_zval(wire)?)
                .map_err(|error| php_internal(error, "a record bin"))?;
        }
        Ok(table)
    }

    /// One bin's value, or `null` if the record does not have it.
    ///
    /// A bin holding `null` and an absent bin are indistinguishable through
    /// this, because Aerospike does not store a nil bin — writing nil deletes
    /// it. Use [`Record::has`] when the difference matters to the caller.
    pub fn bin(&self, name: &str) -> PhpResult<Zval> {
        match self.bins.iter().find(|(bin, _)| bin == name) {
            Some((_, wire)) => Ok(value::wire_to_zval(wire)?),
            None => {
                let mut null = Zval::new();
                null.set_null();
                Ok(null)
            }
        }
    }

    /// Whether the record has this bin.
    pub fn has(&self, name: &str) -> bool {
        self.bins.iter().any(|(bin, _)| bin == name)
    }

    /// The bin names, in the order the server returned them.
    pub fn bin_names(&self) -> Vec<String> {
        self.bins.iter().map(|(name, _)| name.clone()).collect()
    }

    /// How many bins the record has.
    pub fn count(&self) -> i64 {
        i64::try_from(self.bins.len()).unwrap_or(i64::MAX)
    }

    /// The record's generation, which every write increments.
    pub fn generation(&self) -> i64 {
        i64::from(self.generation)
    }

    /// Seconds until the record expires, or `null` when it never does.
    ///
    /// `null` rather than a magic number: "never" is not a quantity of
    /// seconds, and a sentinel would invite arithmetic on it.
    pub fn ttl(&self) -> Option<i64> {
        self.ttl.map(i64::from)
    }

    /// Which record this is, for one a scan or query returned — or `null`.
    ///
    /// `null` in two quite different situations, which is why [`Record::digest`]
    /// exists beside it:
    ///
    /// - a single-record read, where the caller already has the key it asked
    ///   with and the server does not send it back;
    /// - a scanned record whose write did not store its key, because
    ///   `sendKey` was not set. The record has a digest and no key, and a digest
    ///   cannot be turned back into one.
    ///
    /// So a scan that needs keys has to have been written with `sendKey: true`.
    /// That is the server's rule, not this client's.
    pub fn key(&self) -> Option<Key> {
        self.key.clone()
    }

    /// The server's 20-byte digest, for a record a scan or query returned.
    ///
    /// An `Aerospike\Blob`, because it is bytes rather than text —
    /// `bin2hex((string) $record->digest())` is the readable form. `null` for a
    /// single-record read, whose digest was never sent.
    ///
    /// This is the identity every scanned record has, whether or not its key was
    /// stored, which makes it what a scan can rely on.
    pub fn digest(&self) -> Option<crate::types::Blob> {
        self.digest.as_ref().map(|bytes| crate::types::Blob::of(bytes.clone()))
    }

    /// When the record expires, as an `Aerospike\Expiration`.
    ///
    /// The same fact [`Record::ttl`] reports, in the shape a `WritePolicy` takes —
    /// so a read-modify-write can carry the record's own lifetime forward without
    /// converting anything. `null` for a record that never expires, matching
    /// `ttl()`.
    pub fn expiration(&self) -> Option<crate::policy::Expiration> {
        self.ttl
            .map(|seconds| crate::policy::Expiration::of_seconds(i64::from(seconds)))
            .transpose()
            .ok()
            .flatten()
    }

    // ===== the 1.x client's spellings ======================================
    //
    // The previous PHP client named these `get*()` and also exposed them as
    // public properties. The methods are aliases here — one line each, and they
    // mean the difference between an old script running and not. Property access
    // goes through [`Record::__get`].

    /// Alias of [`Record::bins`], as the 1.x client spelled it.
    pub fn get_bins(&self) -> PhpResult<ZBox<ZendHashTable>> {
        self.bins()
    }

    /// Alias of [`Record::generation`].
    pub fn get_generation(&self) -> i64 {
        self.generation()
    }

    /// Alias of [`Record::ttl`].
    pub fn get_ttl(&self) -> Option<i64> {
        self.ttl()
    }

    /// Alias of [`Record::key`].
    pub fn get_key(&self) -> Option<Key> {
        self.key()
    }

    /// Alias of [`Record::expiration`].
    pub fn get_expiration(&self) -> Option<crate::policy::Expiration> {
        self.expiration()
    }

    /// Property access, for the 1.x client's `$record->bins` style.
    ///
    /// A magic getter rather than registered properties: a property registered
    /// from Rust is declared to PHP as untyped, so `$record->bins = [...]` would
    /// be accepted and silently do nothing. `__get` is read-only by construction,
    /// and an unknown name is an error naming what is available rather than
    /// `null` — which is what PHP would otherwise hand back.
    pub fn __get(&self, name: &str) -> PhpResult<Zval> {
        match name {
            "bins" => {
                let mut zval = Zval::new();
                zval.set_hashtable(self.bins()?);
                Ok(zval)
            }
            "generation" => Ok(zval_of_int(self.generation())),
            "ttl" => Ok(self.ttl().map_or_else(Zval::new, zval_of_int)),
            "key" => match self.key() {
                None => Ok(Zval::new()),
                Some(key) => into_zval(key),
            },
            "expiration" => match self.expiration() {
                None => Ok(Zval::new()),
                Some(expiration) => into_zval(expiration),
            },
            other => Err(AeroError::client(format!(
                "Aerospike\\Record has no property ${other}. It has $bins, $generation, $ttl, \
                 $key and $expiration — or call bins(), generation(), ttl(), key() and \
                 expiration(), which is the spelling this client documents"
            ))
            .into()),
        }
    }
}

/// A PHP string as a `Zval`, for the magic getters.
fn zval_of_string(value: String) -> Zval {
    let mut zval = Zval::new();
    // Infallible for an owned `String`; a failure here would mean the engine
    // could not allocate, which is not a case this can report usefully.
    let _ = zval.set_string(&value, false);
    zval
}

/// A PHP int as a `Zval`, for the magic getters.
fn zval_of_int(value: i64) -> Zval {
    let mut zval = Zval::new();
    zval.set_long(value);
    zval
}

/// A registered class instance as a `Zval`, for the magic getters.
fn into_zval<T: ext_php_rs::convert::IntoZval>(value: T) -> PhpResult<Zval> {
    value
        .into_zval(false)
        .map_err(|e| AeroError::client(format!("that property could not be returned: {e}")).into())
}

impl Record {
    /// Build one from a batch row's answer, whose metadata travels in the
    /// payload rather than in the reply header — a batch has no single
    /// generation.
    #[must_use]
    pub fn from_parts(body: RecordBody, generation: u32, ttl: Option<u32>) -> Record {
        Record {
            bins: body.bins,
            generation,
            ttl,
            key: None,
            digest: None,
        }
    }

    /// Build one from a reply. Metadata comes from the typed header, which is
    /// where the contract puts it.
    #[must_use]
    pub fn from_reply(header: &ReplyHeader, body: RecordBody) -> Record {
        Record {
            bins: body.bins,
            generation: header.generation,
            ttl: header.time_to_live(),
            key: None,
            digest: None,
        }
    }

    /// Build one from a page of a scan or query.
    ///
    /// The only shape that carries its own identity and its own metadata: a page
    /// has no single generation, and a caller that scanned a set needs to know
    /// *which* record each answer is.
    #[must_use]
    pub fn from_query(record: &aerospike_php_ipc::query::WireQueryRecord) -> Record {
        Record {
            bins: record.bins.clone(),
            generation: record.generation,
            ttl: record.ttl,
            key: crate::query::stored_key(record),
            digest: Some(crate::query::digest_of(record)),
        }
    }
}

/// What the daemon answers `ping()` with.
///
/// As with [`Record`], there is no constructor: this describes a daemon, and
/// one that no daemon reported would be a fiction.
#[php_class]
#[php(name = "Aerospike\\DaemonInfo")]
#[derive(Debug, Clone)]
pub struct DaemonInfo {
    version: String,
    instances: Vec<String>,
}

#[php_impl]
impl DaemonInfo {
    /// The daemon's version.
    ///
    /// Always equal to this extension's, because a daemon of any other version
    /// could not have answered: the service name they meet on embeds it.
    pub fn version(&self) -> String {
        self.version.clone()
    }

    /// Every instance the daemon is configured for.
    ///
    /// The names of the `[cluster.*]` sections in its configuration file, which
    /// are what `new Aerospike\Client($instance)` accepts.
    pub fn instances(&self) -> Vec<String> {
        self.instances.clone()
    }

    /// Whether the daemon serves `$instance`.
    pub fn serves(&self, instance: &str) -> bool {
        self.instances.iter().any(|name| name == instance)
    }
}

impl DaemonInfo {
    /// Build one from a `PING` reply.
    #[must_use]
    pub fn new(version: String, instances: Vec<String>) -> DaemonInfo {
        DaemonInfo { version, instances }
    }
}

/// Read a `Bin[]` argument, checking every element.
///
/// PHP can type the parameter as `array` but not as *an array of `Bin`*, so this
/// is where that half of the signature is enforced — and it is enforced by
/// position, because "item 2 must be an Aerospike\Bin, string given" is
/// actionable where "invalid value given for argument `bins`" is not.
///
/// # Errors
/// A PHP `TypeError` for an element that is not a `Bin`, and an
/// [`AeroError`]-derived failure when the list is empty: every write verb needs
/// at least one bin, and the server's own complaint would name neither the verb
/// nor the record.
pub fn bin_list<'a>(bins: &[&'a Zval], operation: &str) -> PhpResult<Vec<&'a Bin>> {
    at_least_one(bins.len(), operation)?;

    let mut list = Vec::with_capacity(bins.len());
    for (index, zval) in bins.iter().enumerate() {
        let bin = zval.extract::<&Bin>().ok_or_else(|| {
            arg::type_error(format!(
                "$bins must be a list of Aerospike\\Bin; item {index} is {}",
                arg::type_of(zval)
            ))
        })?;
        list.push(bin);
    }
    Ok(list)
}

/// Refuse a write with no bins.
///
/// Its own function so the rule is testable without a running PHP: turning an
/// [`AeroError`] into a PHP exception needs the exception class's entry, which
/// only exists inside a PHP process.
///
/// # Errors
/// [`AeroError`] naming the verb.
fn at_least_one(count: usize, operation: &str) -> AeroResult<()> {
    if count > 0 {
        return Ok(());
    }
    Err(AeroError::client(format!(
        "{operation} needs at least one Aerospike\\Bin; a write with no bins would reach the \
         server as a request it cannot answer"
    )))
}

/// Convert a checked bin list into the contract's ordered pairs.
///
/// The values were converted when each [`Bin`] was constructed, so this only
/// flattens them — which is why a bad *value* is reported against its own bin
/// rather than against a position in this list.
#[must_use]
pub fn wire_bins(bins: &[&Bin]) -> Vec<(String, WireValue)> {
    bins.iter().map(|bin| bin.to_wire()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bin_selector_maps_onto_the_contracts_three_cases() {
        assert_eq!(Bins::all().to_wire(), BinSelector::All);
        assert_eq!(Bins::none().to_wire(), BinSelector::None);
        assert_eq!(
            Bins::only(vec!["name".into(), "age".into()]).unwrap().to_wire(),
            BinSelector::Only(vec!["name".into(), "age".into()])
        );

        assert!(Bins::all().is_all() && !Bins::all().is_none());
        assert!(Bins::none().is_none() && !Bins::none().is_all());
        assert_eq!(Bins::all().names(), None);
        assert_eq!(Bins::none().names(), None);
    }

    /// The trap this class exists to remove: an empty selection used to mean
    /// "no bins", so a computed list that came out empty silently changed the
    /// request. It must be refused, and the message must point at the verb
    /// that does mean it.
    #[test]
    fn naming_no_bins_is_refused_and_points_at_the_alternative() {
        let error = Bins::only(vec![]).expect_err("an empty selection must be refused");
        assert!(error.to_string().contains("Bins::none()"), "{error}");

        let blank = Bins::only(vec!["name".into(), "  ".into()])
            .expect_err("a blank bin name must be refused");
        assert!(blank.to_string().contains("position 1"), "{blank}");
    }

    /// The empty case is the one reachable without a running PHP; the
    /// element-type check needs real zvals, and `tests/smoke.php` covers it.
    #[test]
    fn a_write_with_no_bins_is_refused_before_it_reaches_the_daemon() {
        let error = at_least_one(0, "put").expect_err("a write needs bins");
        assert!(error.to_string().contains("put"), "{error}");
        assert!(at_least_one(1, "put").is_ok());
    }
}
