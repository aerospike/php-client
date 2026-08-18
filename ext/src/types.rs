// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The small wrapper classes that let PHP name a value shape it cannot express
//! on its own.
//!
//! PHP has five scalar types and one array type; Aerospike has rather more. A
//! PHP string in particular is a byte string with no way to ask it whether it
//! was meant as text, as binary, or as a GeoJSON document — so the *outbound*
//! direction needs a way for a caller to say which. That is what these classes
//! are: each one wraps a payload and stands for exactly one [`WireValue`]
//! variant.
//!
//! They are deliberately dumb — a constructor and an accessor each. Anything
//! smarter would be a value model of its own, and the contract already is one.
//!
//! ```php
//! $client->put("test", "users", "alice", [
//!     "avatar" => new Aerospike\Blob(file_get_contents("a.png")),
//!     "where"  => new Aerospike\GeoJson('{"type":"Point","coordinates":[13.4,52.5]}'),
//! ]);
//! ```
//!
//! Reading goes back through the same classes, so a value **round trips as
//! itself**: a blob read back is an `Aerospike\Blob`, not a PHP string that
//! would silently be rewritten as text on the next write. That is the whole
//! reason the read direction wraps rather than unwraps.

use aerospike_php_ipc::WireValue;
use ext_php_rs::binary::Binary;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendClassObject, Zval};

use crate::error::{AeroError, AeroResult};

/// A binary string, written as an Aerospike blob rather than as text.
///
/// PHP cannot distinguish binary from UTF-8, so a bare PHP string is always
/// written as a text value; wrap it in this to write bytes.
///
/// ```php
/// $client->put("test", "files", "logo", ["data" => new Aerospike\Blob($bytes)]);
/// $record = $client->get("test", "files", "logo");
/// $bytes  = $record["bins"]["data"]->bytes();   // an Aerospike\Blob
/// ```
#[php_class]
#[php(name = "Aerospike\\Blob")]
#[derive(Debug, Default, Clone)]
pub struct Blob {
    bytes: Vec<u8>,
}

#[php_impl]
impl Blob {
    /// Wrap a PHP string, binary or not.
    ///
    /// Taking the bytes as a packed binary string rather than as a Rust
    /// `String` is the point: a `String` would have to be valid UTF-8, which is
    /// exactly the constraint this class exists to escape.
    pub fn __construct(bytes: Binary<u8>) -> Blob {
        Blob { bytes: bytes.to_vec() }
    }

    /// The wrapped bytes, as a PHP string.
    pub fn bytes(&self) -> Binary<u8> {
        Binary::new(self.bytes.clone())
    }

    /// How many bytes are wrapped.
    pub fn length(&self) -> i64 {
        i64::try_from(self.bytes.len()).unwrap_or(i64::MAX)
    }

    /// The wrapped bytes, so `(string) $blob` and string interpolation work.
    pub fn __to_string(&self) -> Binary<u8> {
        self.bytes()
    }
}

impl Blob {
    /// The wrapped bytes, for this crate's own conversion code.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Wrap bytes this crate produced, with no PHP string in between.
    ///
    /// For values the *daemon* sends that are bytes rather than text — a record
    /// digest — where going through `__construct` would mean building a PHP
    /// string only to read it back out.
    #[must_use]
    pub const fn of(bytes: Vec<u8>) -> Blob {
        Blob { bytes }
    }
}

/// A GeoJSON document.
///
/// Distinct from a plain string because the server indexes and queries it as
/// geometry: written as text it is inert, and a geospatial index or query will
/// not see it.
///
/// The document is **not** validated here. The server parses GeoJSON and
/// reports precisely what it did not like; a second, weaker parser in the
/// extension could only disagree with it.
#[php_class]
#[php(name = "Aerospike\\GeoJson")]
#[derive(Debug, Default, Clone)]
pub struct GeoJson {
    json: String,
}

#[php_impl]
impl GeoJson {
    /// Wrap a GeoJSON document.
    pub fn __construct(json: String) -> PhpResult<GeoJson> {
        if json.trim().is_empty() {
            return Err(AeroError::client(
                "Aerospike\\GeoJson needs a GeoJSON document; the string is empty",
            )
            .into());
        }
        Ok(GeoJson { json })
    }

    /// The wrapped document.
    pub fn json(&self) -> String {
        self.json.clone()
    }

    /// The wrapped document, so `(string) $geo` works.
    pub fn __to_string(&self) -> String {
        self.json.clone()
    }
}

impl GeoJson {
    /// The wrapped document, for this crate's own conversion code.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.json
    }
}

/// A HyperLogLog sketch: opaque bytes only the server's HLL operations may
/// interpret.
///
/// Wrapped rather than handed over as a PHP string for the same reason as
/// [`Blob`], plus one more: a sketch that was accidentally written back as text
/// is no longer a sketch, and the failure would only surface later, in a
/// cardinality estimate that is quietly wrong.
#[php_class]
#[php(name = "Aerospike\\Hll")]
#[derive(Debug, Default, Clone)]
pub struct Hll {
    bytes: Vec<u8>,
}

#[php_impl]
impl Hll {
    /// Wrap the bytes of a sketch the server produced.
    pub fn __construct(bytes: Binary<u8>) -> Hll {
        Hll { bytes: bytes.to_vec() }
    }

    /// The sketch's bytes, as a PHP string.
    pub fn bytes(&self) -> Binary<u8> {
        Binary::new(self.bytes.clone())
    }

    /// How many bytes the sketch occupies.
    pub fn length(&self) -> i64 {
        i64::try_from(self.bytes.len()).unwrap_or(i64::MAX)
    }
}

impl Hll {
    /// The wrapped bytes, for this crate's own conversion code.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// The value that sorts above every other, for open-ended CDT range
/// selections.
///
/// A class rather than a constant on purpose: a PHP class constant can only
/// hold a scalar, and any scalar sentinel — `PHP_INT_MAX`, a magic string — is
/// a value someone will one day store for real, at which point the sentinel
/// silently changes what their range selection means. An object of a dedicated
/// class cannot be confused with data.
///
/// Nothing consumes it yet: CDT operations are a later phase, and writing one
/// into a bin is rejected by the server. It exists so that the value model in
/// the contract and the one in PHP do not have a hole in the same place.
#[php_class]
#[php(name = "Aerospike\\Infinity")]
#[derive(Debug, Default, Clone)]
pub struct Infinity;

#[php_impl]
impl Infinity {
    /// The sentinel carries no state.
    pub fn __construct() -> Infinity {
        Infinity
    }
}

/// Matches any value, for CDT selections by example. See [`Infinity`] for why
/// this is a class and not a constant.
#[php_class]
#[php(name = "Aerospike\\Wildcard")]
#[derive(Debug, Default, Clone)]
pub struct Wildcard;

#[php_impl]
impl Wildcard {
    /// The sentinel carries no state.
    pub fn __construct() -> Wildcard {
        Wildcard
    }
}

// ===== PHP object -> wire ===================================================

/// Recognise one of the wrapper classes above.
///
/// Returns `None` for anything else — including an object of an unrelated
/// class, which the caller reports with its own, more informative message.
#[must_use]
pub fn object_to_wire(zval: &Zval) -> Option<WireValue> {
    if !zval.is_object() {
        return None;
    }
    if let Some(blob) = zval.extract::<&ZendClassObject<Blob>>() {
        return Some(WireValue::Blob(blob.as_bytes().to_vec()));
    }
    if let Some(geo) = zval.extract::<&ZendClassObject<GeoJson>>() {
        return Some(WireValue::GeoJson(geo.as_str().to_owned()));
    }
    if let Some(hll) = zval.extract::<&ZendClassObject<Hll>>() {
        return Some(WireValue::Hll(hll.as_bytes().to_vec()));
    }
    if zval.extract::<&ZendClassObject<Infinity>>().is_some() {
        return Some(WireValue::Infinity);
    }
    if zval.extract::<&ZendClassObject<Wildcard>>().is_some() {
        return Some(WireValue::Wildcard);
    }
    None
}

/// The bytes of an `Aerospike\Blob`, for the places that accept a binary
/// string but not an arbitrary value — a record key, most of all.
#[must_use]
pub fn blob_bytes(zval: &Zval) -> Option<Vec<u8>> {
    zval.extract::<&ZendClassObject<Blob>>()
        .map(|blob| blob.as_bytes().to_vec())
}

// ===== wire -> PHP object ===================================================

/// Build an `Aerospike\Blob` around `bytes`.
///
/// # Errors
/// [`AeroError`] if PHP could not allocate the object.
pub fn blob_zval(bytes: Vec<u8>) -> AeroResult<Zval> {
    wrap(Blob { bytes }, "Aerospike\\Blob")
}

/// Build an `Aerospike\GeoJson` around `json`.
///
/// # Errors
/// [`AeroError`] if PHP could not allocate the object.
pub fn geojson_zval(json: String) -> AeroResult<Zval> {
    wrap(GeoJson { json }, "Aerospike\\GeoJson")
}

/// Build an `Aerospike\Hll` around `bytes`.
///
/// # Errors
/// [`AeroError`] if PHP could not allocate the object.
pub fn hll_zval(bytes: Vec<u8>) -> AeroResult<Zval> {
    wrap(Hll { bytes }, "Aerospike\\Hll")
}

/// Build an `Aerospike\Infinity`.
///
/// # Errors
/// [`AeroError`] if PHP could not allocate the object.
pub fn infinity_zval() -> AeroResult<Zval> {
    wrap(Infinity, "Aerospike\\Infinity")
}

/// Build an `Aerospike\Wildcard`.
///
/// # Errors
/// [`AeroError`] if PHP could not allocate the object.
pub fn wildcard_zval() -> AeroResult<Zval> {
    wrap(Wildcard, "Aerospike\\Wildcard")
}

/// Move a Rust value into a fresh PHP object of its registered class.
fn wrap<T: ext_php_rs::class::RegisteredClass>(value: T, class: &str) -> AeroResult<Zval> {
    ZendClassObject::new(value).into_zval(false).map_err(|error| {
        AeroError::client(format!("could not build a PHP {class} for a value the server returned: {error}"))
    })
}
