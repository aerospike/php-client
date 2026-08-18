// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Translation between PHP values and the contract's [`WireValue`].
//!
//! # Lists and maps
//!
//! PHP has one array type — an ordered hash map — where Aerospike has two
//! collection types, so the direction PHP → Aerospike needs a rule. This
//! module uses the conventional one, the same one `json_encode` applies:
//!
//! - keys exactly `0, 1, .., n-1` **in that order** ⇒ [`WireValue::List`]
//! - anything else (string keys, gaps, a different order) ⇒ [`WireValue::Map`]
//! - the empty array ⇒ [`WireValue::List`]
//!
//! Coming back, [`WireValue::List`] becomes a packed array and
//! [`WireValue::Map`] an associative one, so a list round trips exactly and a
//! map round trips as long as its keys are ones PHP can hold.
//!
//! # Strings, and the values PHP cannot name
//!
//! A PHP string is a byte string; there is no way to ask it whether it was
//! meant as text or as a blob. Every PHP string therefore becomes
//! [`WireValue::Str`] — and a string that is not valid UTF-8 is refused, since
//! writing it as text would corrupt it. To write bytes, wrap them in
//! [`Blob`](crate::types::Blob); the same goes for the other shapes PHP has no
//! literal for, in [`crate::types`].
//!
//! Coming back, those shapes are wrapped again rather than flattened:
//! [`WireValue::Blob`] becomes an `Aerospike\Blob`, not a PHP string. Handing
//! back a bare string would round trip *once* and then silently turn the blob
//! into text on the next write, which is the sort of data loss that surfaces
//! long after the code that caused it.
//!
//! # Result-only shapes
//!
//! [`WireValue::MultiResult`], [`WireValue::KeyValueList`] and
//! [`WireValue::Unknown`] are shapes only a server produces
//! ([`WireValue::is_result_only`]). They are converted on the way in but have no
//! way *out* of PHP, which is deliberate: the contract requires a caller never
//! to send them.

use std::fmt;

use aerospike_php_ipc::{WireKey, WireValue};
use ext_php_rs::boxed::ZBox;
use ext_php_rs::types::{ArrayKey, ZendHashTable, Zval};

use crate::error::{AeroError, AeroResult, php_internal};
use crate::maps;
use crate::types;

/// How deeply nested a value may be before conversion refuses to continue.
///
/// PHP arrays can contain references to themselves (`$a[0] = &$a;`), which
/// would otherwise recurse until the stack ran out. The limit is far above
/// anything a record legitimately nests.
const MAX_DEPTH: u32 = 64;

// ===== Error paths ==========================================================

/// A breadcrumb trail into a nested value, so a rejected bin can be named
/// precisely instead of by type alone.
///
/// Built as a chain of borrowed links rather than a `String` so that walking
/// a large record allocates nothing unless something actually fails.
pub struct Path<'a> {
    step: Step<'a>,
    parent: Option<&'a Path<'a>>,
}

enum Step<'a> {
    Bin(&'a str),
    RecordKey,
    Index(usize),
    StrKey(&'a str),
    IntKey(i64),
    MapKey,
}

impl<'a> Path<'a> {
    /// Root of a trail through the value of one bin.
    #[must_use]
    pub fn bin(name: &'a str) -> Path<'a> {
        Path { step: Step::Bin(name), parent: None }
    }

    /// Root of a trail through a record key.
    #[must_use]
    pub fn record_key() -> Path<'static> {
        Path { step: Step::RecordKey, parent: None }
    }

    /// A link one level deeper.
    ///
    /// Takes the parent by argument rather than as `&self` so the borrow of
    /// the parent and the borrow of whatever the step names are independent.
    fn child(parent: &'a Path<'a>, step: Step<'a>) -> Path<'a> {
        Path { step, parent: Some(parent) }
    }
}

impl fmt::Display for Path<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(parent) = self.parent {
            write!(f, "{parent}")?;
        }
        match self.step {
            Step::Bin(name) => write!(f, "bin \"{name}\""),
            Step::RecordKey => f.write_str("the record key"),
            Step::Index(index) => write!(f, "[{index}]"),
            Step::StrKey(key) => write!(f, "[\"{key}\"]"),
            Step::IntKey(key) => write!(f, "[{key}]"),
            Step::MapKey => f.write_str(" (map key)"),
        }
    }
}

// ===== PHP -> wire ==========================================================

/// Convert a bin value.
///
/// # Errors
/// [`AeroError`] naming `path` when the value is one the contract cannot
/// carry: an object, a resource, a string that is not UTF-8, or nesting
/// past [`MAX_DEPTH`].
pub fn zval_to_wire(zval: &Zval, path: &Path<'_>) -> AeroResult<WireValue> {
    zval_to_wire_at(zval, path, 0)
}

fn zval_to_wire_at(zval: &Zval, path: &Path<'_>, depth: u32) -> AeroResult<WireValue> {
    if depth > MAX_DEPTH {
        return Err(AeroError::client(format!(
            "{path} nests more than {MAX_DEPTH} levels deep; if the array contains a reference \
             to itself, Aerospike cannot store it"
        )));
    }

    // The accessors below dereference a PHP reference for us, but `is_null`
    // does not, so normalise once up front and use the result throughout.
    let zval = zval.dereference();

    if zval.is_null() {
        return Ok(WireValue::Nil);
    }
    if let Some(value) = zval.bool() {
        return Ok(WireValue::Bool(value));
    }
    if let Some(value) = zval.long() {
        return Ok(WireValue::Int(value));
    }
    if let Some(value) = zval.double() {
        return Ok(WireValue::Float(value));
    }
    if let Some(text) = zval.zend_str() {
        return match text.as_str() {
            Ok(text) => Ok(WireValue::Str(text.to_owned())),
            // A bare PHP string is written as text, so one that is not text
            // has to be refused rather than mangled. `Aerospike\Blob` is how a
            // caller says "these are bytes".
            Err(_) => Err(AeroError::client(format!(
                "{path} is a PHP string that is not valid UTF-8, and a bare string is written as \
                 a text value. Wrap it in Aerospike\\Blob to write it as binary"
            ))),
        };
    }
    if let Some(table) = zval.array() {
        return table_to_wire(table, path, depth);
    }
    // One of the wrapper classes: the only way PHP can name a blob, a GeoJSON
    // document, an HLL sketch or a CDT sentinel.
    if let Some(value) = types::object_to_wire(zval) {
        return Ok(value);
    }
    // …or one of the two map kinds a PHP array cannot express.
    if let Some(value) = maps::object_to_wire(zval) {
        return Ok(value);
    }

    Err(AeroError::client(format!(
        "{path} is {}, which Aerospike cannot store. A bin takes null, bool, int, float, string, \
         array, or an Aerospike\\Blob, Aerospike\\GeoJson, Aerospike\\Hll, \
         Aerospike\\OrderedMap or Aerospike\\SortedMap",
        describe(zval)
    )))
}

fn table_to_wire(table: &ZendHashTable, path: &Path<'_>, depth: u32) -> AeroResult<WireValue> {
    // See the module docs: sequential 0..n-1 keys mean a list, and an empty
    // array counts as sequential.
    if table.has_sequential_keys() {
        let mut items = Vec::with_capacity(table.len());
        for (index, value) in table.values().enumerate() {
            let child = Path::child(path, Step::Index(index));
            items.push(zval_to_wire_at(value, &child, depth + 1)?);
        }
        return Ok(WireValue::List(items));
    }

    let mut pairs = Vec::with_capacity(table.len());
    for (key, value) in table.iter() {
        let key = normalize_key(&key, path)?;
        let (wire_key, step) = match &key {
            PhpKey::Int(number) => (WireValue::Int(*number), Step::IntKey(*number)),
            PhpKey::Str(text) => (WireValue::Str(text.clone()), Step::StrKey(text.as_str())),
        };
        let child = Path::child(path, step);
        pairs.push((wire_key, zval_to_wire_at(value, &child, depth + 1)?));
    }
    Ok(WireValue::Map(pairs))
}

/// Convert a record key.
///
/// A key is an int, a string, or an [`Aerospike\Blob`](crate::types::Blob) for a
/// binary key. As with bin values, a bare PHP string is a *text* key: the digest
/// the server computes covers the key's type, so guessing wrong here would place
/// the record under a different digest entirely.
///
/// # Errors
/// [`AeroError`] when the key is none of those three.
pub fn zval_to_wire_key(zval: &Zval) -> AeroResult<WireKey> {
    let path = Path::record_key();
    let zval = zval.dereference();
    if let Some(value) = zval.long() {
        return Ok(WireKey::Int(value));
    }
    if let Some(text) = zval.zend_str() {
        return match text.as_str() {
            Ok(text) => Ok(WireKey::Str(text.to_owned())),
            Err(_) => Err(AeroError::client(format!(
                "{path} is a PHP string that is not valid UTF-8, and a bare string is a text key. \
                 Wrap it in Aerospike\\Blob to use it as a binary key"
            ))),
        };
    }
    if let Some(bytes) = types::blob_bytes(zval) {
        return Ok(WireKey::Blob(bytes));
    }
    Err(AeroError::client(format!(
        "{path} is {}; a key must be an int, a string, or an Aerospike\\Blob",
        describe(zval)
    )))
}

/// A PHP array key reduced to the only two forms PHP actually stores.
/// One PHP array key as an Aerospike value.
///
/// Used where a PHP map is an operation's *argument* rather than a bin value —
/// `MapOp::putItems` — so that the keys go through exactly the same conversion,
/// and the same refusal of a key PHP cannot hold, as a nested map in a bin.
///
/// # Errors
/// [`AeroError`] for a key that is not valid UTF-8, which a PHP array key cannot
/// carry back.
pub fn array_key_to_wire(key: &ArrayKey<'_>) -> AeroResult<WireValue> {
    match normalize_key(key, &Path::bin("map key"))? {
        PhpKey::Int(number) => Ok(WireValue::Int(number)),
        PhpKey::Str(text) => Ok(WireValue::Str(text)),
    }
}

#[derive(Debug)]
enum PhpKey {
    Int(i64),
    Str(String),
}

fn normalize_key(key: &ArrayKey<'_>, path: &Path<'_>) -> AeroResult<PhpKey> {
    let bytes = match key {
        ArrayKey::Long(number) => return Ok(PhpKey::Int(*number)),
        ArrayKey::String(text) => return Ok(PhpKey::Str(text.clone())),
        ArrayKey::Str(text) => return Ok(PhpKey::Str((*text).to_owned())),
        ArrayKey::ZendString(text) => text.as_ref(),
    };
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(PhpKey::Str(text.to_owned())),
        Err(_) => {
            let child = Path::child(path, Step::MapKey);
            // Unlike a bin value or a record key, a map key has no wrapper to
            // reach for: it comes back as a PHP *array* key, which cannot hold
            // arbitrary bytes, so a blob key would not survive the round trip
            // even though the wire could carry it.
            Err(AeroError::client(format!(
                "{child} is not valid UTF-8, and a map key has to come back as a PHP array key, \
                 which only holds ints and text"
            )))
        }
    }
}

fn describe(zval: &Zval) -> String {
    if zval.is_object() {
        return zval
            .object()
            .and_then(|object| object.get_class_name().ok())
            .map_or_else(|| "an object".to_owned(), |name| format!("an instance of {name}"));
    }
    if zval.is_resource() {
        return "a resource".to_owned();
    }
    if zval.is_callable() {
        return "a callable".to_owned();
    }
    format!("of PHP type {:?}", zval.get_type())
}

// ===== wire -> PHP ==========================================================

/// Convert a value the daemon returned.
///
/// # Errors
/// [`AeroError`] if PHP rejects a string or hashtable allocation, or if a
/// map key is one PHP cannot hold (see [`wire_key_to_php`]).
pub fn wire_to_zval(value: &WireValue) -> AeroResult<Zval> {
    let mut zval = Zval::new();
    match value {
        WireValue::Nil => zval.set_null(),
        WireValue::Bool(value) => zval.set_bool(*value),
        WireValue::Int(value) => zval.set_long(*value),
        WireValue::Float(value) => zval.set_double(*value),
        WireValue::Str(text) => zval
            .set_string(text, false)
            .map_err(|error| php_internal(error, "a string value"))?,
        // Wrapped, not flattened to a PHP string: see the module docs.
        WireValue::Blob(bytes) => return types::blob_zval(bytes.clone()),
        WireValue::GeoJson(json) => return types::geojson_zval(json.clone()),
        WireValue::Hll(bytes) => return types::hll_zval(bytes.clone()),
        WireValue::Infinity => return types::infinity_zval(),
        WireValue::Wildcard => return types::wildcard_zval(),
        WireValue::List(items) => zval.set_hashtable(list_to_table(items, "a list element")?),
        // An unordered map is a PHP array: that is what a PHP array means.
        WireValue::Map(pairs) => zval.set_hashtable(map_to_table(pairs)?),
        // The two ordered kinds come back as their class, so a map round trips
        // as itself. Read a key-ordered map into a plain array and write it
        // back, and it would be stored *unordered* — the ordering that makes
        // the rank and key-range operations run in log time would be gone, with
        // nothing said. The same argument as for a blob, and the same answer.
        //
        // In practice only `SortedMap` arrives from the server, since key order
        // is the only one it keeps; `OrderedMap` is handled for completeness.
        WireValue::OrderedMap(pairs) => return maps::ordered_zval(pairs.clone()),
        WireValue::SortedMap(pairs) => return maps::sorted_zval(pairs.clone()),
        // One bin name, several results, in operation order. A packed array,
        // because that is what "in order" means in PHP.
        WireValue::MultiResult(results) => {
            zval.set_hashtable(list_to_table(results, "an operation result")?);
        }
        // A flat key/value *result*, kept distinct from a map on the wire, so
        // it stays distinct here: a list of [key, value] pairs preserves both
        // the order and keys a PHP array could not hold.
        WireValue::KeyValueList(pairs) => {
            let mut table = ZendHashTable::with_capacity(capacity(pairs.len()));
            for (key, item) in pairs {
                let mut pair = ZendHashTable::with_capacity(2);
                pair.push(wire_to_zval(key)?)
                    .map_err(|error| php_internal(error, "a key/value result"))?;
                pair.push(wire_to_zval(item)?)
                    .map_err(|error| php_internal(error, "a key/value result"))?;
                let mut pair_zval = Zval::new();
                pair_zval.set_hashtable(pair);
                table
                    .push(pair_zval)
                    .map_err(|error| php_internal(error, "a key/value result"))?;
            }
            zval.set_hashtable(table);
        }
        // A particle type this build cannot decode. Inspectable rather than
        // fatal: a newer server's data type degrades to an array a caller can
        // look at and log, instead of failing the whole read.
        WireValue::Unknown { particle_type, data } => {
            let mut table = ZendHashTable::with_capacity(2);
            table
                .insert("particle_type", i64::from(*particle_type))
                .map_err(|error| php_internal(error, "an undecoded value"))?;
            table
                .insert("data", types::blob_zval(data.clone())?)
                .map_err(|error| php_internal(error, "an undecoded value"))?;
            zval.set_hashtable(table);
        }
    }
    Ok(zval)
}

fn list_to_table(items: &[WireValue], what: &str) -> AeroResult<ZBox<ZendHashTable>> {
    let mut table = ZendHashTable::with_capacity(capacity(items.len()));
    for item in items {
        table
            .push(wire_to_zval(item)?)
            .map_err(|error| php_internal(error, what))?;
    }
    Ok(table)
}

fn map_to_table(pairs: &[(WireValue, WireValue)]) -> AeroResult<ZBox<ZendHashTable>> {
    let mut table = ZendHashTable::with_capacity(capacity(pairs.len()));
    for (key, item) in pairs {
        let value = wire_to_zval(item)?;
        match wire_key_to_php(key)? {
            PhpKey::Int(number) => table
                .insert_at_index(number, value)
                .map_err(|error| php_internal(error, "a map entry"))?,
            PhpKey::Str(text) => table
                .insert(text.as_str(), value)
                .map_err(|error| php_internal(error, "a map entry"))?,
        }
    }
    Ok(table)
}

/// Reduce a map key to something PHP can use as an array key.
///
/// PHP array keys are only ever `int` or `string`, so the other scalar
/// variants are coerced exactly the way PHP itself coerces them — `null`
/// becomes `""`, a bool becomes `0`/`1`, a float truncates. A collection
/// cannot be a key at all and is reported rather than mangled.
///
/// # Errors
/// [`AeroError`] when the key is a collection or a CDT sentinel.
fn wire_key_to_php(key: &WireValue) -> AeroResult<PhpKey> {
    match key {
        WireValue::Nil => Ok(PhpKey::Str(String::new())),
        WireValue::Bool(value) => Ok(PhpKey::Int(i64::from(*value))),
        WireValue::Int(value) => Ok(PhpKey::Int(*value)),
        // Truncation matches PHP's own float-to-key conversion. Saturating
        // rather than wrapping keeps a NaN or an out-of-range float from
        // becoming an arbitrary index.
        WireValue::Float(value) => Ok(PhpKey::Int(*value as i64)),
        WireValue::Str(text) | WireValue::GeoJson(text) => Ok(PhpKey::Str(text.clone())),
        // A binary map key has to become a PHP string to be a key at all, so
        // the wrapper cannot help here; lossy conversion is the same choice PHP
        // itself makes for a non-UTF-8 array key.
        WireValue::Blob(bytes) | WireValue::Hll(bytes) => {
            Ok(PhpKey::Str(String::from_utf8_lossy(bytes).into_owned()))
        }
        WireValue::List(_)
        | WireValue::Map(_)
        | WireValue::OrderedMap(_)
        | WireValue::SortedMap(_)
        | WireValue::MultiResult(_)
        | WireValue::KeyValueList(_) => Err(AeroError::client(
            "the record contains a map whose key is itself a collection; a PHP array key can \
             only be an int or a string",
        )),
        WireValue::Infinity | WireValue::Wildcard => Err(AeroError::client(
            "the record contains a map keyed by a CDT sentinel (infinity or wildcard), which a \
             PHP array key cannot express",
        )),
        WireValue::Unknown { particle_type, .. } => Err(AeroError::client(format!(
            "the record contains a map whose key is of particle type {particle_type}, which this \
             build cannot decode into a PHP array key"
        ))),
    }
}

fn capacity(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    // Only the parts that do not touch a `Zval` can be exercised here: a
    // `Zval` is allocated by the Zend engine, so anything involving one needs
    // a running PHP. `tests/smoke.php` covers the rest.
    use super::*;

    #[test]
    fn map_keys_follow_php_array_key_coercion() {
        let int = wire_key_to_php(&WireValue::Int(7)).unwrap();
        assert!(matches!(int, PhpKey::Int(7)));

        let text = wire_key_to_php(&WireValue::Str("theme".into())).unwrap();
        assert!(matches!(text, PhpKey::Str(ref s) if s == "theme"));

        // PHP turns `null` into the empty-string key and a bool into 0/1.
        assert!(matches!(wire_key_to_php(&WireValue::Nil).unwrap(), PhpKey::Str(ref s) if s.is_empty()));
        assert!(matches!(wire_key_to_php(&WireValue::Bool(true)).unwrap(), PhpKey::Int(1)));
        assert!(matches!(wire_key_to_php(&WireValue::Bool(false)).unwrap(), PhpKey::Int(0)));

        // A float truncates, as it does in PHP.
        assert!(matches!(wire_key_to_php(&WireValue::Float(2.9)).unwrap(), PhpKey::Int(2)));

        // A collection cannot be an array key at all.
        assert!(wire_key_to_php(&WireValue::List(vec![])).is_err());
        assert!(wire_key_to_php(&WireValue::Map(vec![])).is_err());
        assert!(wire_key_to_php(&WireValue::OrderedMap(vec![])).is_err());
        assert!(wire_key_to_php(&WireValue::SortedMap(vec![])).is_err());
        assert!(wire_key_to_php(&WireValue::MultiResult(vec![])).is_err());
        assert!(wire_key_to_php(&WireValue::KeyValueList(vec![])).is_err());

        // Nor can a sentinel or a value this build cannot decode — and both
        // messages have to say which, since the record is otherwise readable.
        let sentinel = wire_key_to_php(&WireValue::Infinity).unwrap_err().to_string();
        assert!(sentinel.contains("sentinel"), "{sentinel}");
        let unknown = wire_key_to_php(&WireValue::Unknown { particle_type: 42, data: vec![] })
            .unwrap_err()
            .to_string();
        assert!(unknown.contains("42"), "{unknown}");

        // A GeoJSON document is a string on the wire, so it can be a key.
        assert!(matches!(
            wire_key_to_php(&WireValue::GeoJson("{}".into())).unwrap(),
            PhpKey::Str(ref s) if s == "{}"
        ));
    }

    #[test]
    fn paths_name_the_offending_position() {
        let bin = Path::bin("prefs");
        assert_eq!(bin.to_string(), "bin \"prefs\"");

        let nested = Path::child(&bin, Step::StrKey("theme"));
        assert_eq!(nested.to_string(), "bin \"prefs\"[\"theme\"]");

        let deeper = Path::child(&nested, Step::Index(2));
        assert_eq!(deeper.to_string(), "bin \"prefs\"[\"theme\"][2]");

        assert_eq!(Path::record_key().to_string(), "the record key");
    }
}
