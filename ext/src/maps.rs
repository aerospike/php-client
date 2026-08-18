// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! `Aerospike\OrderedMap` and `Aerospike\SortedMap`: the two map kinds a PHP
//! array cannot express.
//!
//! Aerospike has three map kinds and PHP has one array, so writing an array has
//! always meant writing an *unordered* map. These two classes are how a caller
//! says otherwise. They are accepted anywhere a map is — as a bin value, nested
//! inside a list or another map, as an operation argument, and as the items of
//! `MapOp::putItems` — and a map that comes back from the server ordered comes
//! back **as one of these**, so it round trips as itself.
//!
//! ```php
//! use Aerospike\{Bin, SortedMap};
//!
//! $scores = new SortedMap(['bob' => 20, 'alice' => 10]);
//! $client->put(null, $key, [new Bin('scores', $scores)]);
//!
//! $stored = $client->get(null, $key)->bin('scores');   // a SortedMap
//! foreach ($stored as $name => $score) { /* in the server's key order */ }
//! $stored->get('alice');   // 10
//! $stored['alice'];        // the same, through ArrayAccess
//! ```
//!
//! # Why reading one back gives an object, not an array
//!
//! For the same reason a blob does. A key-ordered map read into a plain array
//! and written back would be stored *unordered*: the ordering — which is what
//! makes the rank and key-range operations run in log time — would be gone, and
//! nothing would have said so. Handing back the class keeps the round trip
//! honest, and `toArray()` is there when an array is what you actually want.
//!
//! Only [`SortedMap`] can come back from the server, because key order is the
//! only one the server keeps. See [`OrderedMap`] for what that means for it.
//!
//! # Keys PHP arrays cannot hold
//!
//! A PHP array key is an int or a string. An Aerospike map key can also be a
//! blob, a float, or a boolean — and `foreach` over one of these classes hands
//! those back **as they are**, because `Iterator::key()` is not restricted the
//! way an array key is. That is the second thing these classes can express that
//! an array cannot.

use aerospike_php_ipc::WireValue;
use ext_php_rs::boxed::ZBox;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendClassObject, ZendHashTable, Zval};
use ext_php_rs::zend::ce;

use crate::error::{php_internal, AeroError, AeroResult};
use crate::value::{self, Path};

/// The entries of one map, and where an iteration has got to.
///
/// Shared by both classes: they differ in what the *server* does with the
/// order, not in anything they do themselves.
#[derive(Debug, Clone, Default)]
struct Entries {
    /// In the order given, which for a map read from the server is the order
    /// the server sent.
    pairs: Vec<(WireValue, WireValue)>,
    /// Where `foreach` has got to. Part of the object, as PHP's `Iterator`
    /// requires; nested `foreach` over one instance therefore shares a cursor,
    /// exactly as it does for any other `Iterator` implementation.
    cursor: usize,
}

impl Entries {
    /// Build from a PHP array, converting keys and values as a bin value would.
    fn from_table(table: &ZendHashTable) -> AeroResult<Entries> {
        let mut pairs = Vec::with_capacity(table.len());
        for (key, value) in table.iter() {
            pairs.push((
                value::array_key_to_wire(&key)?,
                value::zval_to_wire(value, &Path::bin("map value"))?,
            ));
        }
        Ok(Entries { pairs, cursor: 0 })
    }

    fn position(&self, key: &WireValue) -> Option<usize> {
        self.pairs.iter().position(|(existing, _)| existing == key)
    }

    /// Insert or replace, keeping an existing key in its place.
    ///
    /// Replacing in place rather than moving the entry to the end is what makes
    /// this a map rather than a log: `set` on a key that is already there
    /// changes a value, and changing a value is not a reordering.
    fn set(&mut self, key: WireValue, value: WireValue) {
        match self.position(&key) {
            Some(index) => self.pairs[index].1 = value,
            None => self.pairs.push((key, value)),
        }
    }

    fn get(&self, key: &WireValue) -> Option<&WireValue> {
        self.position(key).map(|index| &self.pairs[index].1)
    }

    fn remove(&mut self, key: &WireValue) -> bool {
        match self.position(key) {
            Some(index) => {
                self.pairs.remove(index);
                true
            }
            None => false,
        }
    }

    /// The PHP array this map would be if its keys all fitted in one.
    ///
    /// Strict where reading a plain map bin is lenient. A map bin arrives as an
    /// array and has nowhere else to go, so a blob key is flattened to a lossy
    /// string there; here the caller is holding an object that carries the key
    /// faithfully and has asked for an array anyway, so quietly mangling it
    /// would be throwing away something they still have.
    ///
    /// # Errors
    /// [`AeroError`] for a key a PHP array cannot hold — a blob, a float, a
    /// bool, a nested collection.
    fn to_table(&self) -> AeroResult<ZBox<ZendHashTable>> {
        let mut table = ZendHashTable::with_capacity(capacity(self.pairs.len()));
        for (key, value) in &self.pairs {
            let value = value::wire_to_zval(value)?;
            match key {
                WireValue::Int(number) => table
                    .insert_at_index(*number, value)
                    .map_err(|error| php_internal(error, "a map entry"))?,
                WireValue::Str(text) => table
                    .insert(text.as_str(), value)
                    .map_err(|error| php_internal(error, "a map entry"))?,
                other => {
                    return Err(AeroError::client(format!(
                        "this map has a {} key, which a PHP array cannot hold — that is one of \
                         the reasons this class exists. Iterate it instead of calling toArray()",
                        describe_key(other)
                    )))
                }
            }
        }
        Ok(table)
    }
}

/// A map whose entries keep the order they were given.
///
/// **The server does not store this order.** Aerospike has no insertion-ordered
/// map, so an `OrderedMap` is written as an unordered map and reads back as a
/// plain PHP array. What it buys is the *sending* side: the entries reach the
/// server in the order you wrote them, which is what the client's own
/// insertion-ordered map type means, and the order is preserved locally for as
/// long as you hold the object.
///
/// If you want an order the server keeps, use [`SortedMap`].
#[php_class]
#[php(name = "Aerospike\\OrderedMap")]
#[php(implements(ce = ce::arrayaccess, stub = "\\ArrayAccess"))]
#[php(implements(ce = ce::iterator, stub = "\\Iterator"))]
#[php(implements(ce = ce::countable, stub = "\\Countable"))]
#[derive(Debug, Clone, Default)]
pub struct OrderedMap {
    entries: Entries,
}

#[php_impl]
impl OrderedMap {
    /// Build one, optionally from a PHP array.
    #[php(defaults(entries = None))]
    pub fn __construct(entries: Option<&ZendHashTable>) -> PhpResult<OrderedMap> {
        Ok(OrderedMap {
            entries: match entries {
                Some(table) => Entries::from_table(table)?,
                None => Entries::default(),
            },
        })
    }

    /// Set one entry, replacing an existing key in place.
    pub fn set(&mut self, key: &Zval, value: &Zval) -> PhpResult<()> {
        self.entries.set(map_key(key)?, map_value(value)?);
        Ok(())
    }

    /// One entry's value, or `null` if the key is not there.
    pub fn get(&self, key: &Zval) -> PhpResult<Zval> {
        lookup(&self.entries, key)
    }

    /// Whether the map has this key.
    pub fn has(&self, key: &Zval) -> PhpResult<bool> {
        Ok(self.entries.position(&map_key(key)?).is_some())
    }

    /// Remove one entry, reporting whether it was there.
    pub fn remove(&mut self, key: &Zval) -> PhpResult<bool> {
        Ok(self.entries.remove(&map_key(key)?))
    }

    /// The keys, in order.
    pub fn keys(&self) -> PhpResult<ZBox<ZendHashTable>> {
        Ok(keys_of(&self.entries)?)
    }

    /// The values, in order.
    pub fn values(&self) -> PhpResult<ZBox<ZendHashTable>> {
        Ok(values_of(&self.entries)?)
    }

    /// The same entries as a PHP array.
    ///
    /// Throws for a key a PHP array cannot hold — a blob or a float — since
    /// carrying those is one of the two reasons this class exists.
    pub fn to_array(&self) -> PhpResult<ZBox<ZendHashTable>> {
        Ok(self.entries.to_table()?)
    }

    /// How many entries the map holds.
    pub fn count(&self) -> i64 {
        count_of(&self.entries)
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.pairs.is_empty()
    }

    // ----- Iterator -----

    /// Start an iteration.
    pub fn rewind(&mut self) {
        self.entries.cursor = 0;
    }

    /// Whether the iteration has an entry to give.
    pub fn valid(&self) -> bool {
        self.entries.cursor < self.entries.pairs.len()
    }

    /// The value at the cursor.
    pub fn current(&self) -> PhpResult<Zval> {
        current_of(&self.entries)
    }

    /// The key at the cursor. Not restricted to what a PHP array key can hold.
    pub fn key(&self) -> PhpResult<Zval> {
        key_of(&self.entries)
    }

    /// Advance the cursor.
    pub fn next(&mut self) {
        self.entries.cursor += 1;
    }

    // ----- ArrayAccess -----

    /// `isset($map[$key])`.
    pub fn offset_exists(&self, offset: &Zval) -> PhpResult<bool> {
        self.has(offset)
    }

    /// `$map[$key]`.
    pub fn offset_get(&self, offset: &Zval) -> PhpResult<Zval> {
        lookup(&self.entries, offset)
    }

    /// `$map[$key] = $value`.
    pub fn offset_set(&mut self, offset: &Zval, value: &Zval) -> PhpResult<()> {
        appendless(offset)?;
        self.set(offset, value)
    }

    /// `unset($map[$key])`.
    pub fn offset_unset(&mut self, offset: &Zval) -> PhpResult<()> {
        self.remove(offset)?;
        Ok(())
    }
}

impl OrderedMap {
    /// The contract's value.
    #[must_use]
    pub fn to_wire(&self) -> WireValue {
        WireValue::OrderedMap(self.entries.pairs.clone())
    }

    /// Build one from entries the server sent.
    #[must_use]
    pub fn from_wire(pairs: Vec<(WireValue, WireValue)>) -> OrderedMap {
        OrderedMap {
            entries: Entries { pairs, cursor: 0 },
        }
    }

    /// The entries, for the code that needs them without a PHP round trip.
    #[must_use]
    pub fn pairs(&self) -> &[(WireValue, WireValue)] {
        &self.entries.pairs
    }
}

/// A map the server stores sorted by key.
///
/// Unlike [`OrderedMap`], this order is real storage: the server keeps the map
/// key-ordered, which is what lets its rank, index and key-range operations run
/// in log time instead of scanning. A key-ordered map read back from the server
/// therefore arrives **as a `SortedMap`**, in the server's order.
///
/// The entries are not re-sorted locally. The server is the authority on how
/// values order — its rule spans every type, `nil < bool < int < string < list <
/// map < blob < float < GeoJSON` — and a second implementation of that here
/// could only ever disagree with it. So a `SortedMap` you build iterates in the
/// order you built it, and one the server sent iterates in the server's order,
/// which is the one that matters.
#[php_class]
#[php(name = "Aerospike\\SortedMap")]
#[php(implements(ce = ce::arrayaccess, stub = "\\ArrayAccess"))]
#[php(implements(ce = ce::iterator, stub = "\\Iterator"))]
#[php(implements(ce = ce::countable, stub = "\\Countable"))]
#[derive(Debug, Clone, Default)]
pub struct SortedMap {
    entries: Entries,
}

#[php_impl]
impl SortedMap {
    /// Build one, optionally from a PHP array.
    #[php(defaults(entries = None))]
    pub fn __construct(entries: Option<&ZendHashTable>) -> PhpResult<SortedMap> {
        Ok(SortedMap {
            entries: match entries {
                Some(table) => Entries::from_table(table)?,
                None => Entries::default(),
            },
        })
    }

    /// Set one entry, replacing an existing key in place.
    pub fn set(&mut self, key: &Zval, value: &Zval) -> PhpResult<()> {
        self.entries.set(map_key(key)?, map_value(value)?);
        Ok(())
    }

    /// One entry's value, or `null` if the key is not there.
    pub fn get(&self, key: &Zval) -> PhpResult<Zval> {
        lookup(&self.entries, key)
    }

    /// Whether the map has this key.
    pub fn has(&self, key: &Zval) -> PhpResult<bool> {
        Ok(self.entries.position(&map_key(key)?).is_some())
    }

    /// Remove one entry, reporting whether it was there.
    pub fn remove(&mut self, key: &Zval) -> PhpResult<bool> {
        Ok(self.entries.remove(&map_key(key)?))
    }

    /// The keys, in order.
    pub fn keys(&self) -> PhpResult<ZBox<ZendHashTable>> {
        Ok(keys_of(&self.entries)?)
    }

    /// The values, in order.
    pub fn values(&self) -> PhpResult<ZBox<ZendHashTable>> {
        Ok(values_of(&self.entries)?)
    }

    /// The same entries as a PHP array.
    ///
    /// Throws for a key a PHP array cannot hold — a blob or a float — since
    /// carrying those is one of the two reasons this class exists.
    pub fn to_array(&self) -> PhpResult<ZBox<ZendHashTable>> {
        Ok(self.entries.to_table()?)
    }

    /// How many entries the map holds.
    pub fn count(&self) -> i64 {
        count_of(&self.entries)
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.pairs.is_empty()
    }

    // ----- Iterator -----

    /// Start an iteration.
    pub fn rewind(&mut self) {
        self.entries.cursor = 0;
    }

    /// Whether the iteration has an entry to give.
    pub fn valid(&self) -> bool {
        self.entries.cursor < self.entries.pairs.len()
    }

    /// The value at the cursor.
    pub fn current(&self) -> PhpResult<Zval> {
        current_of(&self.entries)
    }

    /// The key at the cursor. Not restricted to what a PHP array key can hold.
    pub fn key(&self) -> PhpResult<Zval> {
        key_of(&self.entries)
    }

    /// Advance the cursor.
    pub fn next(&mut self) {
        self.entries.cursor += 1;
    }

    // ----- ArrayAccess -----

    /// `isset($map[$key])`.
    pub fn offset_exists(&self, offset: &Zval) -> PhpResult<bool> {
        self.has(offset)
    }

    /// `$map[$key]`.
    pub fn offset_get(&self, offset: &Zval) -> PhpResult<Zval> {
        lookup(&self.entries, offset)
    }

    /// `$map[$key] = $value`.
    pub fn offset_set(&mut self, offset: &Zval, value: &Zval) -> PhpResult<()> {
        appendless(offset)?;
        self.set(offset, value)
    }

    /// `unset($map[$key])`.
    pub fn offset_unset(&mut self, offset: &Zval) -> PhpResult<()> {
        self.remove(offset)?;
        Ok(())
    }
}

impl SortedMap {
    /// The contract's value.
    #[must_use]
    pub fn to_wire(&self) -> WireValue {
        WireValue::SortedMap(self.entries.pairs.clone())
    }

    /// Build one from entries the server sent.
    #[must_use]
    pub fn from_wire(pairs: Vec<(WireValue, WireValue)>) -> SortedMap {
        SortedMap {
            entries: Entries { pairs, cursor: 0 },
        }
    }

    /// The entries, for the code that needs them without a PHP round trip.
    #[must_use]
    pub fn pairs(&self) -> &[(WireValue, WireValue)] {
        &self.entries.pairs
    }
}

// ===== shared helpers =======================================================

/// A PHP value as a map key.
///
/// The same conversion a bin value gets, so a key can be anything Aerospike can
/// store one as — including the blobs and floats a PHP array key cannot hold.
fn map_key(key: &Zval) -> AeroResult<WireValue> {
    value::zval_to_wire(key, &Path::bin("map key"))
}

/// A PHP value as a map value.
fn map_value(value: &Zval) -> AeroResult<WireValue> {
    value::zval_to_wire(value, &Path::bin("map value"))
}

/// One entry's value as PHP sees it, or `null`.
fn lookup(entries: &Entries, key: &Zval) -> PhpResult<Zval> {
    match entries.get(&map_key(key)?) {
        Some(value) => Ok(value::wire_to_zval(value)?),
        None => Ok(null_zval()),
    }
}

fn keys_of(entries: &Entries) -> AeroResult<ZBox<ZendHashTable>> {
    let mut table = ZendHashTable::with_capacity(capacity(entries.pairs.len()));
    for (key, _) in &entries.pairs {
        table
            .push(value::wire_to_zval(key)?)
            .map_err(|error| php_internal(error, "a map key"))?;
    }
    Ok(table)
}

fn values_of(entries: &Entries) -> AeroResult<ZBox<ZendHashTable>> {
    let mut table = ZendHashTable::with_capacity(capacity(entries.pairs.len()));
    for (_, value) in &entries.pairs {
        table
            .push(value::wire_to_zval(value)?)
            .map_err(|error| php_internal(error, "a map value"))?;
    }
    Ok(table)
}

fn count_of(entries: &Entries) -> i64 {
    i64::try_from(entries.pairs.len()).unwrap_or(i64::MAX)
}

/// The value at the cursor, or null past the end.
///
/// Null rather than an error: PHP calls `current()` on an invalid position in
/// ordinary code, and `valid()` is what says whether there is anything there.
fn current_of(entries: &Entries) -> PhpResult<Zval> {
    match entries.pairs.get(entries.cursor) {
        Some((_, value)) => Ok(value::wire_to_zval(value)?),
        None => Ok(null_zval()),
    }
}

/// The key at the cursor, or null past the end.
fn key_of(entries: &Entries) -> PhpResult<Zval> {
    match entries.pairs.get(entries.cursor) {
        Some((key, _)) => Ok(value::wire_to_zval(key)?),
        None => Ok(null_zval()),
    }
}

/// Refuse `$map[] = $value`.
///
/// A map has no "next key", and PHP signals the append form with a null offset —
/// which is also a legal Aerospike map key. Refusing it is the only way to keep
/// `$map[null] = 1` meaning what it says.
fn appendless(offset: &Zval) -> PhpResult<()> {
    if offset.is_null() {
        return Err(AeroError::client(
            "a map has no next key, so $map[] = $value is not meaningful; give the key you mean",
        )
        .into());
    }
    Ok(())
}

/// What a key is, for the message when a PHP array cannot hold it.
fn describe_key(key: &WireValue) -> &'static str {
    match key {
        WireValue::Nil => "null",
        WireValue::Bool(_) => "boolean",
        WireValue::Float(_) => "float",
        WireValue::Blob(_) => "binary",
        WireValue::GeoJson(_) => "GeoJSON",
        WireValue::Hll(_) => "HyperLogLog",
        WireValue::List(_) => "list",
        WireValue::Map(_) | WireValue::OrderedMap(_) | WireValue::SortedMap(_) => "map",
        _ => "non-scalar",
    }
}

fn null_zval() -> Zval {
    let mut zval = Zval::new();
    zval.set_null();
    zval
}

fn capacity(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

// ===== conversion for the value model =======================================

/// Recognise one of these classes in a PHP value.
#[must_use]
pub fn object_to_wire(zval: &Zval) -> Option<WireValue> {
    if let Some(map) = zval.extract::<&ZendClassObject<OrderedMap>>() {
        return Some(map.to_wire());
    }
    if let Some(map) = zval.extract::<&ZendClassObject<SortedMap>>() {
        return Some(map.to_wire());
    }
    None
}

/// The entries of any PHP map argument: an array, an `OrderedMap` or a
/// `SortedMap`.
///
/// This is what "accepted wherever a map is" means for the places that take a
/// map *as such* rather than as a value — `MapOp::putItems`, most of all.
///
/// # Errors
/// [`AeroError`] when the value is none of those three, or when converting an
/// entry fails.
pub fn entries_of(zval: &Zval) -> AeroResult<Vec<(WireValue, WireValue)>> {
    if let Some(map) = zval.extract::<&ZendClassObject<OrderedMap>>() {
        return Ok(map.pairs().to_vec());
    }
    if let Some(map) = zval.extract::<&ZendClassObject<SortedMap>>() {
        return Ok(map.pairs().to_vec());
    }
    if let Some(table) = zval.array() {
        return Ok(Entries::from_table(table)?.pairs);
    }
    Err(AeroError::client(
        "expected a map: a PHP array, an Aerospike\\OrderedMap or an Aerospike\\SortedMap",
    ))
}

/// Build an `Aerospike\OrderedMap` around entries the server sent.
///
/// # Errors
/// [`AeroError`] if PHP could not allocate the object.
pub fn ordered_zval(pairs: Vec<(WireValue, WireValue)>) -> AeroResult<Zval> {
    wrap(OrderedMap::from_wire(pairs), "Aerospike\\OrderedMap")
}

/// Build an `Aerospike\SortedMap` around entries the server sent.
///
/// # Errors
/// [`AeroError`] if PHP could not allocate the object.
pub fn sorted_zval(pairs: Vec<(WireValue, WireValue)>) -> AeroResult<Zval> {
    wrap(SortedMap::from_wire(pairs), "Aerospike\\SortedMap")
}

fn wrap<T: ext_php_rs::class::RegisteredClass>(value: T, class: &str) -> AeroResult<Zval> {
    ZendClassObject::new(value).into_zval(false).map_err(|error| {
        AeroError::client(format!(
            "could not build a PHP {class} for a value the server returned: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Entries {
        Entries {
            pairs: vec![
                (WireValue::Str("b".into()), WireValue::Int(2)),
                (WireValue::Str("a".into()), WireValue::Int(1)),
            ],
            cursor: 0,
        }
    }

    /// Setting a key that is already there changes its value and leaves it
    /// where it was: changing a value is not a reordering, and for an
    /// insertion-ordered map the difference is the whole point.
    #[test]
    fn setting_an_existing_key_replaces_it_in_place() {
        let mut map = entries();
        map.set(WireValue::Str("b".into()), WireValue::Int(20));

        assert_eq!(map.pairs.len(), 2);
        assert_eq!(map.pairs[0].0, WireValue::Str("b".into()));
        assert_eq!(map.pairs[0].1, WireValue::Int(20));
        assert_eq!(map.get(&WireValue::Str("a".into())), Some(&WireValue::Int(1)));
    }

    #[test]
    fn a_new_key_goes_on_the_end() {
        let mut map = entries();
        map.set(WireValue::Str("c".into()), WireValue::Int(3));
        assert_eq!(map.pairs[2].0, WireValue::Str("c".into()));
    }

    #[test]
    fn removing_reports_whether_the_key_was_there() {
        let mut map = entries();
        assert!(map.remove(&WireValue::Str("a".into())));
        assert!(!map.remove(&WireValue::Str("a".into())));
        assert_eq!(map.pairs.len(), 1);
        assert_eq!(map.get(&WireValue::Str("a".into())), None);
    }

    /// A key an array cannot hold is the second reason these classes exist, so
    /// it has to survive being stored and looked up.
    #[test]
    fn a_key_a_php_array_could_not_hold_works() {
        let mut map = Entries::default();
        let blob = WireValue::Blob(vec![0, 255]);
        map.set(blob.clone(), WireValue::Str("bytes".into()));

        assert_eq!(map.get(&blob), Some(&WireValue::Str("bytes".into())));
        assert_eq!(map.get(&WireValue::Blob(vec![1])), None);
    }

    /// The two classes differ only in the wire variant they produce — which is
    /// the whole distinction the server acts on.
    #[test]
    fn each_class_produces_its_own_wire_shape() {
        let pairs = entries().pairs;
        assert!(matches!(
            OrderedMap::from_wire(pairs.clone()).to_wire(),
            WireValue::OrderedMap(_)
        ));
        assert!(matches!(
            SortedMap::from_wire(pairs).to_wire(),
            WireValue::SortedMap(_)
        ));
    }

    /// Order is preserved exactly: an `OrderedMap` that sorted itself would be
    /// a `SortedMap`, and a `SortedMap` that sorted itself would be guessing at
    /// the server's ordering rules.
    #[test]
    fn neither_class_reorders_what_it_was_given() {
        let map = SortedMap::from_wire(entries().pairs);
        let keys: Vec<_> = map.pairs().iter().map(|(key, _)| key.clone()).collect();
        assert_eq!(
            keys,
            vec![WireValue::Str("b".into()), WireValue::Str("a".into())],
            "the order given must survive; the server is what sorts"
        );
    }
}
