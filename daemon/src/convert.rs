// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! [`WireValue`] ⇄ [`Value`] and [`WireKey`] → [`Key`].
//!
//! The contract deliberately carries its own value model so the extension
//! never links the database client; this module is the single place the two
//! models meet.
//!
//! Both directions are now total, which is the point of the contract carrying
//! the server's whole value model: nothing a record can hold has to be reported
//! as unrepresentable, and nothing the extension can send has to be guessed at.
//! Three asymmetries remain, and they are all in the *shapes*, not in the
//! coverage:
//!
//! - **Result-only shapes travel one way.** [`WireValue::MultiResult`],
//!   [`WireValue::KeyValueList`] and [`WireValue::Unknown`] are things a server
//!   produces. A caller sending one is a bug in the caller, so [`to_value`]
//!   refuses it — flagged by the contract's own
//!   [`WireValue::is_result_only`] — rather than inventing a client value for
//!   it.
//! - **Maps travel as pairs.** The three map shapes are a vector of pairs, so
//!   the order the server returned survives the trip and non-string keys stay
//!   expressible.
//! - **Only key-ordering is stored.** [`WireValue::Map`] and
//!   [`WireValue::OrderedMap`] both become [`Value::OrderedMap`], which writes
//!   an *unordered* server map whose pairs are packed in the given order; the
//!   server returns such a map in its own canonical key order, so both come
//!   back as [`WireValue::Map`]. [`WireValue::SortedMap`] is different in
//!   storage — it carries the server's K-ordered flag — so it survives as
//!   itself.

use std::fmt;

use aerospike_core::{Bin, FloatValue, IndexMap, Key, Value};
use aerospike_php_ipc::{WireKey, WireValue};

/// A wire value the server produces but a caller must never send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultOnlyValue {
    /// Name of the result-only contract variant that was sent.
    pub shape: &'static str,
}

impl fmt::Display for ResultOnlyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is a result shape the server produces; it cannot be sent to it",
            self.shape
        )
    }
}

impl std::error::Error for ResultOnlyValue {}

/// A bin whose value could not be sent, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadBin {
    /// Name of the offending bin.
    pub bin: String,
    /// What was wrong with its value.
    pub reason: ResultOnlyValue,
}

impl fmt::Display for BadBin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bin '{}': {}", self.bin, self.reason)
    }
}

impl std::error::Error for BadBin {}

/// Convert a wire value into a client value.
///
/// # Errors
/// [`ResultOnlyValue`] when the value — or one nested inside it — is a shape
/// only the server produces.
pub fn to_value(wire: &WireValue) -> Result<Value, ResultOnlyValue> {
    // Asked of the contract rather than re-derived here, so a result-only
    // variant added there cannot quietly become sendable. The match below still
    // has to name them, for exhaustiveness.
    if wire.is_result_only() {
        return Err(ResultOnlyValue {
            shape: result_only_shape(wire),
        });
    }
    match wire {
        WireValue::Nil => Ok(Value::Nil),
        WireValue::Bool(b) => Ok(Value::Bool(*b)),
        WireValue::Int(i) => Ok(Value::Int(*i)),
        WireValue::Float(f) => Ok(Value::Float(FloatValue::from(*f))),
        WireValue::Str(s) => Ok(Value::String(s.clone())),
        WireValue::Blob(b) => Ok(Value::Blob(b.clone())),
        WireValue::GeoJson(json) => Ok(Value::GeoJSON(json.clone())),
        WireValue::Hll(bytes) => Ok(Value::HLL(bytes.clone())),
        WireValue::Infinity => Ok(Value::Infinity),
        WireValue::Wildcard => Ok(Value::Wildcard),
        WireValue::List(items) => Ok(Value::List(
            items.iter().map(to_value).collect::<Result<Vec<_>, _>>()?,
        )),
        // An unordered server map either way; the pair order given is the pair
        // order packed.
        WireValue::Map(pairs) | WireValue::OrderedMap(pairs) => {
            let mut map = IndexMap::with_capacity(pairs.len());
            for (key, value) in pairs {
                map.insert(to_value(key)?, to_value(value)?);
            }
            Ok(Value::OrderedMap(map))
        }
        WireValue::SortedMap(pairs) => {
            let mut map = std::collections::BTreeMap::new();
            for (key, value) in pairs {
                map.insert(to_value(key)?, to_value(value)?);
            }
            Ok(Value::SortedMap(map))
        }
        // Refused above, and deliberately not given a client counterpart.
        WireValue::MultiResult(_) | WireValue::KeyValueList(_) | WireValue::Unknown { .. } => {
            Err(ResultOnlyValue {
                shape: result_only_shape(wire),
            })
        }
    }
}

/// Convert a client value into a wire value. Total: every variant maps.
#[must_use]
pub fn from_value(value: &Value) -> WireValue {
    match value {
        Value::Nil => WireValue::Nil,
        Value::Bool(b) => WireValue::Bool(*b),
        Value::Int(i) => WireValue::Int(*i),
        Value::Float(f) => WireValue::Float(float_to_f64(f)),
        Value::String(s) => WireValue::Str(s.clone()),
        Value::Blob(b) => WireValue::Blob(b.clone()),
        Value::GeoJSON(json) => WireValue::GeoJson(json.clone()),
        Value::HLL(bytes) => WireValue::Hll(bytes.clone()),
        Value::Infinity => WireValue::Infinity,
        Value::Wildcard => WireValue::Wildcard,
        Value::List(items) => WireValue::List(items.iter().map(from_value).collect()),
        Value::MultiResult(items) => {
            WireValue::MultiResult(items.iter().map(from_value).collect())
        }
        // A server map that is not K-ordered comes back in whatever order the
        // server sent, which is what the pair vector is for.
        Value::HashMap(map) => WireValue::Map(pairs(map.iter())),
        Value::OrderedMap(map) => WireValue::Map(pairs(map.iter())),
        // K-ordered in storage, so it stays distinguishable on the wire.
        Value::SortedMap(map) => WireValue::SortedMap(pairs(map.iter())),
        Value::KeyValueList(list) => {
            WireValue::KeyValueList(pairs(list.iter().map(|(k, v)| (k, v))))
        }
        Value::Unknown(particle_type, data) => WireValue::Unknown {
            particle_type: *particle_type,
            data: data.clone(),
        },
    }
}

/// Convert wire bins into client bins, naming the bin that was at fault.
///
/// # Errors
/// [`BadBin`] when a bin's value is a result-only shape.
pub fn to_bins(bins: &[(String, WireValue)]) -> Result<Vec<Bin>, BadBin> {
    let mut out = Vec::with_capacity(bins.len());
    for (name, value) in bins {
        let value = to_value(value).map_err(|reason| BadBin {
            bin: name.clone(),
            reason,
        })?;
        out.push(Bin::new(name.clone(), value));
    }
    Ok(out)
}

fn pairs<'a, I>(entries: I) -> Vec<(WireValue, WireValue)>
where
    I: Iterator<Item = (&'a Value, &'a Value)>,
{
    entries
        .map(|(key, value)| (from_value(key), from_value(value)))
        .collect()
}

/// The contract's name for a result-only shape, for the refusal message.
const fn result_only_shape(wire: &WireValue) -> &'static str {
    match wire {
        WireValue::MultiResult(_) => "a multi-operation result",
        WireValue::KeyValueList(_) => "a key/value result list",
        WireValue::Unknown { .. } => "an undecoded foreign particle",
        // Not reachable: only the three above are result-only.
        _ => "a result-only value",
    }
}

/// Widen a client float to `f64`.
///
/// `FloatValue`'s own `Into<f64>` panics for the single-precision variant, so
/// the widening is done here: a value that reached a record is data, and data
/// must never crash the daemon.
fn float_to_f64(value: &FloatValue) -> f64 {
    match value {
        FloatValue::F32(bits) => f64::from(f32::from_bits(*bits)),
        FloatValue::F64(bits) => f64::from_bits(*bits),
    }
}

/// Build a real key — and therefore compute the digest — from the wire form.
///
/// # Errors
/// Whatever [`Key::new`] rejects (an unsupported user-key type).
pub fn to_key(namespace: &str, set: &str, key: &WireKey) -> aerospike_core::Result<Key> {
    let user_key = match key {
        WireKey::Int(i) => Value::Int(*i),
        WireKey::Str(s) => Value::String(s.clone()),
        WireKey::Blob(b) => Value::Blob(b.clone()),
    };
    Key::new(namespace.to_string(), set.to_string(), user_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, HashMap};

    /// A nested value exercising every variant a caller may send.
    fn nested() -> WireValue {
        WireValue::Map(vec![
            (
                WireValue::Str("scalars".into()),
                WireValue::List(vec![
                    WireValue::Nil,
                    WireValue::Bool(true),
                    WireValue::Bool(false),
                    WireValue::Int(i64::MIN),
                    WireValue::Int(i64::MAX),
                    WireValue::Float(-1.5),
                    WireValue::Str("héllo ☃".into()),
                    WireValue::Blob(vec![0, 1, 254, 255]),
                    WireValue::GeoJson("{\"type\":\"Point\",\"coordinates\":[1,2]}".into()),
                    WireValue::Hll(vec![0, 1, 2]),
                    WireValue::Infinity,
                    WireValue::Wildcard,
                ]),
            ),
            (
                // Sorted in the literal too: the client's sorted map is a
                // BTreeMap, so only key order survives, not the given order.
                WireValue::Str("k-ordered".into()),
                WireValue::SortedMap(vec![
                    (WireValue::Str("a".into()), WireValue::Int(1)),
                    (WireValue::Str("b".into()), WireValue::Infinity),
                ]),
            ),
            (
                WireValue::Int(7),
                WireValue::Map(vec![(
                    WireValue::Blob(vec![9, 9]),
                    WireValue::List(vec![WireValue::Map(vec![(
                        WireValue::Str("deep".into()),
                        WireValue::Int(1),
                    )])]),
                )]),
            ),
        ])
    }

    #[test]
    fn nested_values_round_trip() {
        let wire = nested();
        let value = to_value(&wire).unwrap();
        assert_eq!(from_value(&value), wire);
    }

    #[test]
    fn nil_maps_to_nil_in_both_directions() {
        assert_eq!(to_value(&WireValue::Nil).unwrap(), Value::Nil);
        assert_eq!(from_value(&Value::Nil), WireValue::Nil);
        // Also nested, since a nil bin value and a nil list element are
        // different things to the server but the same conversion here.
        let list = WireValue::List(vec![WireValue::Nil]);
        assert_eq!(from_value(&to_value(&list).unwrap()), list);
    }

    #[test]
    fn scalars_map_to_the_expected_client_variants() {
        assert_eq!(to_value(&WireValue::Bool(true)).unwrap(), Value::Bool(true));
        assert_eq!(to_value(&WireValue::Int(-9)).unwrap(), Value::Int(-9));
        assert_eq!(
            to_value(&WireValue::Str("s".into())).unwrap(),
            Value::String("s".into())
        );
        assert_eq!(
            to_value(&WireValue::Blob(vec![1, 2])).unwrap(),
            Value::Blob(vec![1, 2])
        );
        assert_eq!(
            to_value(&WireValue::Float(2.5)).unwrap(),
            Value::Float(FloatValue::F64(2.5f64.to_bits()))
        );
    }

    #[test]
    fn the_shapes_the_server_treats_specially_keep_their_identity() {
        // A GeoJSON document must not degrade to a string, nor an HLL sketch to
        // a blob: the server indexes and operates on them as their own types.
        let geo = WireValue::GeoJson("{\"type\":\"Point\",\"coordinates\":[0,1]}".into());
        assert_eq!(
            to_value(&geo).unwrap(),
            Value::GeoJSON("{\"type\":\"Point\",\"coordinates\":[0,1]}".into())
        );
        assert_eq!(from_value(&to_value(&geo).unwrap()), geo);

        let hll = WireValue::Hll(vec![0, 4, 8]);
        assert_eq!(to_value(&hll).unwrap(), Value::HLL(vec![0, 4, 8]));
        assert_eq!(from_value(&to_value(&hll).unwrap()), hll);

        for (wire, value) in [
            (WireValue::Infinity, Value::Infinity),
            (WireValue::Wildcard, Value::Wildcard),
        ] {
            assert_eq!(to_value(&wire).unwrap(), value);
            assert_eq!(from_value(&value), wire);
        }

        // And nested, which is where the range sentinels actually appear: as an
        // element of a CDT selection's bounds.
        let range = WireValue::List(vec![WireValue::Int(1), WireValue::Infinity]);
        assert_eq!(from_value(&to_value(&range).unwrap()), range);
    }

    #[test]
    fn a_wire_map_becomes_an_insertion_ordered_client_map() {
        let wire = WireValue::Map(vec![
            (WireValue::Str("z".into()), WireValue::Int(1)),
            (WireValue::Str("a".into()), WireValue::Int(2)),
        ]);
        match to_value(&wire).unwrap() {
            Value::OrderedMap(map) => {
                let keys: Vec<&Value> = map.keys().collect();
                assert_eq!(
                    keys,
                    vec![&Value::String("z".into()), &Value::String("a".into())]
                );
            }
            other => panic!("expected an ordered map, got {other:?}"),
        }
    }

    #[test]
    fn an_ordered_map_is_the_same_storage_as_a_plain_one_but_a_sorted_one_is_not() {
        let pairs = vec![
            (WireValue::Str("z".into()), WireValue::Int(1)),
            (WireValue::Str("a".into()), WireValue::Int(2)),
        ];

        // Both write an unordered server map, with the pairs packed in the
        // order they were given.
        match to_value(&WireValue::OrderedMap(pairs.clone())).unwrap() {
            Value::OrderedMap(map) => {
                let keys: Vec<&Value> = map.keys().collect();
                assert_eq!(
                    keys,
                    vec![&Value::String("z".into()), &Value::String("a".into())]
                );
            }
            other => panic!("expected an ordered map, got {other:?}"),
        }

        // K-ordered is a different thing in storage, so it maps to the client's
        // sorted map — and the keys arrive sorted rather than as given.
        match to_value(&WireValue::SortedMap(pairs)).unwrap() {
            Value::SortedMap(map) => {
                let keys: Vec<&Value> = map.keys().collect();
                assert_eq!(
                    keys,
                    vec![&Value::String("a".into()), &Value::String("z".into())]
                );
            }
            other => panic!("expected a sorted map, got {other:?}"),
        }
    }

    #[test]
    fn every_client_map_variant_comes_back_as_pairs() {
        let mut hash = HashMap::new();
        hash.insert(Value::Int(1), Value::String("one".into()));
        assert_eq!(
            from_value(&Value::HashMap(hash)),
            WireValue::Map(vec![(WireValue::Int(1), WireValue::Str("one".into()))])
        );

        let mut ordered = IndexMap::new();
        ordered.insert(Value::Int(2), Value::Int(20));
        ordered.insert(Value::Int(1), Value::Int(10));
        assert_eq!(
            from_value(&Value::OrderedMap(ordered)),
            WireValue::Map(vec![
                (WireValue::Int(2), WireValue::Int(20)),
                (WireValue::Int(1), WireValue::Int(10)),
            ]),
            "an unordered map keeps the pair order the server sent"
        );

        let mut sorted = BTreeMap::new();
        sorted.insert(Value::Int(2), Value::Int(20));
        sorted.insert(Value::Int(1), Value::Int(10));
        assert_eq!(
            from_value(&Value::SortedMap(sorted)),
            WireValue::SortedMap(vec![
                (WireValue::Int(1), WireValue::Int(10)),
                (WireValue::Int(2), WireValue::Int(20)),
            ])
        );

        assert_eq!(
            from_value(&Value::KeyValueList(vec![(
                Value::String("k".into()),
                Value::Int(1)
            )])),
            WireValue::KeyValueList(vec![(WireValue::Str("k".into()), WireValue::Int(1))])
        );
    }

    #[test]
    fn single_precision_floats_widen_instead_of_panicking() {
        let value = Value::Float(FloatValue::F32(1.5f32.to_bits()));
        assert_eq!(from_value(&value), WireValue::Float(1.5));
    }

    #[test]
    fn result_shapes_come_back_as_themselves() {
        // The server produces these, so a read must carry them rather than fail:
        // what an `operate` returned is exactly what PHP asked for.
        assert_eq!(
            from_value(&Value::MultiResult(vec![Value::Int(1), Value::Nil])),
            WireValue::MultiResult(vec![WireValue::Int(1), WireValue::Nil])
        );
        assert_eq!(
            from_value(&Value::Unknown(7, vec![1, 2])),
            WireValue::Unknown {
                particle_type: 7,
                data: vec![1, 2],
            }
        );
        // Nested, since a multi-op result is what a bin holds, not a bin list.
        assert_eq!(
            from_value(&Value::List(vec![Value::MultiResult(vec![Value::Nil])])),
            WireValue::List(vec![WireValue::MultiResult(vec![WireValue::Nil])])
        );
    }

    #[test]
    fn a_caller_cannot_send_a_result_only_shape() {
        for wire in [
            WireValue::MultiResult(vec![WireValue::Int(1)]),
            WireValue::KeyValueList(vec![(WireValue::Int(1), WireValue::Int(2))]),
            WireValue::Unknown {
                particle_type: 9,
                data: vec![1],
            },
        ] {
            let err = to_value(&wire).unwrap_err();
            assert!(err.to_string().contains("result shape"), "{wire:?}: {err}");
        }

        // And buried in a collection, which is how it would sneak through.
        let in_list =
            WireValue::List(vec![WireValue::List(vec![WireValue::MultiResult(vec![])])]);
        assert!(to_value(&in_list).is_err());
        let in_map = WireValue::Map(vec![(
            WireValue::Str("k".into()),
            WireValue::Unknown {
                particle_type: 1,
                data: vec![],
            },
        )]);
        assert!(to_value(&in_map).is_err());
        // Including as a map key, where the recursion is on the other side.
        let as_key =
            WireValue::SortedMap(vec![(WireValue::KeyValueList(vec![]), WireValue::Int(1))]);
        assert!(to_value(&as_key).is_err());
    }

    #[test]
    fn a_refused_bin_is_named_so_php_can_find_it() {
        let bins = vec![
            ("good".to_string(), WireValue::Int(1)),
            ("bad".to_string(), WireValue::MultiResult(vec![])),
        ];
        let err = to_bins(&bins).unwrap_err();
        assert_eq!(err.bin, "bad");
        assert!(err.to_string().starts_with("bin 'bad':"), "{err}");

        let ok = to_bins(&bins[..1]).unwrap();
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].name, "good");
        assert_eq!(ok[0].value, Value::Int(1));
    }

    #[test]
    fn keys_of_every_wire_type_get_distinct_digests() {
        let int = to_key("test", "s", &WireKey::Int(1)).unwrap();
        let string = to_key("test", "s", &WireKey::Str("1".into())).unwrap();
        let blob = to_key("test", "s", &WireKey::Blob(vec![1])).unwrap();

        assert_eq!(int.namespace, "test");
        assert_eq!(int.set_name, "s");
        assert_eq!(int.user_key, Some(Value::Int(1)));
        assert_eq!(string.user_key, Some(Value::String("1".into())));
        assert_eq!(blob.user_key, Some(Value::Blob(vec![1])));

        assert_ne!(int.digest, string.digest);
        assert_ne!(int.digest, blob.digest);
        assert_ne!(string.digest, blob.digest);

        // The digest covers the set name, so the same user key in another set
        // is another record.
        let other_set = to_key("test", "other", &WireKey::Int(1)).unwrap();
        assert_ne!(int.digest, other_set.digest);

        // An empty set name is the null set, not an error.
        assert!(to_key("test", "", &WireKey::Int(1)).is_ok());
    }
}
