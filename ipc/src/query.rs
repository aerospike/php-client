// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Scans and queries, in pages.
//!
//! A scan or a query answers with an unbounded number of records, and the one
//! thing this contract may not do is lose any of them. iceoryx2 can answer one
//! request with many responses, but measured it drops the oldest silently once
//! the response buffer fills — see [`crate::opcode`] — so records do not stream.
//!
//! # The shape: open, next, close
//!
//! [`QUERY`](crate::opcode::QUERY) carries the whole request and answers with
//! the **first page** plus, if more may follow, a cursor.
//! [`QUERY_NEXT`](crate::opcode::QUERY_NEXT) exchanges that cursor for the next
//! page, and [`QUERY_CLOSE`](crate::opcode::QUERY_CLOSE) abandons one early.
//!
//! ```text
//!   QUERY      { statement, partitions, page_size }  ->  { records, cursor: Some(7) }
//!   QUERY_NEXT { cursor: 7 }                         ->  { records, cursor: Some(7) }
//!   QUERY_NEXT { cursor: 7 }                         ->  { records, cursor: None  }   // done
//! ```
//!
//! A `cursor` of `None` means the traversal finished: there is nothing left to
//! ask for and nothing left to close. A request whose whole result fits in one
//! page therefore costs one round trip and registers nothing — the common case
//! for a query with a selective filter.
//!
//! # Why the cursor is a number and not the state itself
//!
//! `aerospike-core`'s cursor is a `PartitionFilter`, carrying per-partition
//! progress for up to 4096 partitions. It could be serialized and handed to PHP,
//! which would make paging stateless — but it is ~100 KB, so it would cross
//! shared memory twice per page, and rebuilding it on this side means
//! reimplementing a resumption format that the client already treats as private.
//!
//! So the daemon keeps it and PHP holds a number. The cost is state with an
//! owner that can die: a PHP worker that is killed mid-scan never sends
//! `QUERY_CLOSE`. That is bounded two ways — the daemon expires an idle cursor,
//! and it caps how many may be open at once — and a cursor holds no server-side
//! resources between pages, only the progress record. A worker's cursor is
//! therefore a leak of memory for a while, never of connections.
//!
//! An expired cursor is [`StatusCode::CURSOR_EXPIRED`](crate::StatusCode),
//! **never** an empty page: a scan that silently stops early is the failure this
//! whole design exists to avoid.

use serde::{Deserialize, Serialize};

use crate::op::{WireCtx, WireExpression};
use crate::{BinSelector, WireKey, WirePolicy, WireValue};

/// Which records a scan or query visits.
///
/// A statement with no [`filter`](WireStatement::filter) is a **scan**: every
/// record of the set. With one, it is a secondary-index query. That is the same
/// distinction `aerospike-core`'s `Statement` makes, and the reason there is one
/// opcode rather than two.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireStatement {
    /// Namespace to visit.
    pub namespace: String,
    /// Set name; empty means every set in the namespace.
    pub set: String,
    /// Which bins to return.
    pub bins: BinSelector,
    /// The secondary-index filter, or `None` for a scan.
    pub filter: Option<WireFilter>,
}

/// A secondary-index filter.
///
/// One type for all of `aerospike-core`'s filter constructors, because they
/// differ in only three independent ways: what is compared
/// ([`kind`](Self::kind)), whether the index is named by bin or by index name
/// ([`target`](Self::target)), and whether it indexes a collection
/// ([`collection`](Self::collection)). Spelling out the cross product as
/// fourteen variants would let a caller ask for a combination that does not
/// exist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireFilter {
    /// Which index to use, by bin name or by index name.
    pub target: WireFilterTarget,
    /// What to compare.
    pub kind: WireFilterKind,
    /// Which part of a collection the index covers.
    pub collection: WireCollectionIndex,
    /// Path into a nested collection, for an index built on one.
    pub context: Vec<WireCtx>,
    /// The expression an expression-based index was built on.
    pub expression: Option<WireExpression>,
}

/// How a filter names the index it uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireFilterTarget {
    /// By bin name; the server picks the index built on that bin.
    Bin(String),
    /// By index name, when a bin has more than one index on it.
    Index(String),
}

/// Which part of a collection a secondary index covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireCollectionIndex {
    /// A scalar bin.
    Default,
    /// List elements.
    List,
    /// Map keys.
    MapKeys,
    /// Map values.
    MapValues,
}

/// What a filter compares.
///
/// Ranges are integers only, which is `aerospike-core`'s rule too: the server
/// has no ordering for a string index, so a string range would be silently
/// wrong rather than refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireFilterKind {
    /// Records whose indexed value equals this.
    Equal(WireFilterBound),
    /// Records whose indexed integer is in `[begin, end]`, inclusive.
    Range {
        /// Lowest value that matches.
        begin: i64,
        /// Highest value that matches.
        end: i64,
    },
    /// Points inside a GeoJSON region.
    GeoWithinRegion(String),
    /// Points inside a circle on the earth's surface.
    GeoWithinRadius {
        /// Centre longitude, in degrees.
        longitude: f64,
        /// Centre latitude, in degrees.
        latitude: f64,
        /// Radius in metres.
        radius: f64,
    },
    /// Regions that contain a GeoJSON point.
    GeoContains(String),
}

/// A value an equality filter may compare against.
///
/// Deliberately narrower than [`WireValue`]: an index covers integers, strings
/// and blobs, and nothing else can be a filter bound. Naming the three keeps the
/// daemon's conversion total instead of rejecting most of a wider type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireFilterBound {
    /// An integer index.
    Int(i64),
    /// A string index.
    Str(String),
    /// A blob index.
    Blob(Vec<u8>),
}

/// Which partitions a scan or query covers.
///
/// The whole ring by default. The narrower forms exist so that several workers
/// can divide one scan between them, each taking a partition range — which is
/// how a scan is parallelised without any of them seeing another's records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WirePartitions {
    /// Every partition.
    All,
    /// One partition, by id.
    Id(u32),
    /// A contiguous range of partitions.
    Range {
        /// First partition id.
        begin: u32,
        /// How many partitions, from `begin`.
        count: u32,
    },
    /// The partition holding this key, and only records after its digest.
    ///
    /// Primary-index scans only: a digest is not enough to resume a
    /// secondary-index query, which is why `aerospike-core` says the same.
    AfterKey(WireKey),
}

/// Payload of a [`QUERY`](crate::opcode::QUERY) request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireQueryBody {
    /// Cluster instance from the daemon's configuration.
    pub instance: String,
    /// Shared per-call overrides: timeouts, retries, replica, a filter
    /// expression applied to every record.
    pub policy: WirePolicy,
    /// What to visit.
    pub statement: WireStatement,
    /// Which partitions to cover.
    pub partitions: WirePartitions,
    /// How many records one page may hold. `0` means the daemon's configured
    /// default.
    pub page_size: u32,
    /// Ceiling on the records the whole traversal returns, across every page.
    /// `0` means no ceiling.
    pub max_records: u64,
    /// Per-node rate limit, in records per second. `0` means unlimited.
    pub records_per_second: u32,
    /// Whether to return bins. `false` returns keys and metadata only, which is
    /// how a set is counted or its digests collected without moving its data.
    pub include_bin_data: bool,
}

/// Payload of a [`QUERY_NEXT`](crate::opcode::QUERY_NEXT) or
/// [`QUERY_CLOSE`](crate::opcode::QUERY_CLOSE) request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireCursorBody {
    /// The cursor a previous page answered with.
    pub cursor: u64,
}

/// One page of a scan or query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireQueryPage {
    /// The cursor to ask for the next page with, or `None` when the traversal
    /// is finished.
    ///
    /// `None` is the *only* end-of-traversal signal. An empty `records` with a
    /// cursor still set is legal and means "this page found nothing, keep
    /// going": the server divides `max_records` between nodes, so a page can
    /// come back short — or empty — while records remain.
    pub cursor: Option<u64>,
    /// The records this page holds, in the order the cluster produced them,
    /// which is not an order a caller may rely on.
    pub records: Vec<WireQueryRecord>,
}

/// One record from a scan or query.
///
/// Unlike a single-record read, this carries its own key and metadata: there is
/// no one generation for a page, and a caller that scanned a set needs to know
/// *which* record each answer is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireQueryRecord {
    /// Which record this is.
    pub key: WireRecordKey,
    /// Bins, in the order the server returned them. Empty when the request
    /// asked for no bin data.
    pub bins: Vec<(String, WireValue)>,
    /// Generation, which every write increments.
    pub generation: u32,
    /// Remaining seconds to live; `None` means it never expires.
    pub ttl: Option<u32>,
}

/// The identity of a record a scan returned.
///
/// The digest is always there; the user key only when the write that created the
/// record stored it (`send_key`). That asymmetry is the server's, and reporting
/// it faithfully is better than inventing a key from a digest that cannot be
/// reversed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRecordKey {
    /// Namespace.
    pub namespace: String,
    /// Set name; empty for the null set.
    pub set: String,
    /// The server's 20-byte digest.
    pub digest: [u8; 20],
    /// The original user key, when the record has one stored.
    pub user_key: Option<WireKey>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::WireListOrder;
    use crate::{decode_body, encode_body};

    fn statement() -> WireStatement {
        WireStatement {
            namespace: "test".into(),
            set: "users".into(),
            bins: BinSelector::Only(vec!["name".into()]),
            filter: Some(WireFilter {
                target: WireFilterTarget::Bin("age".into()),
                kind: WireFilterKind::Range { begin: 20, end: 30 },
                collection: WireCollectionIndex::Default,
                context: vec![WireCtx::ListIndex { index: 0 }],
                expression: Some(WireExpression::Ael("$.age".into())),
            }),
        }
    }

    #[test]
    fn a_query_request_round_trips() {
        let body = WireQueryBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            policy: WirePolicy::default(),
            statement: statement(),
            partitions: WirePartitions::Range {
                begin: 1_000,
                count: 24,
            },
            page_size: 500,
            max_records: 10_000,
            records_per_second: 100,
            include_bin_data: true,
        };
        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<WireQueryBody>(&bytes).unwrap(), body);
    }

    #[test]
    fn every_filter_shape_round_trips() {
        for kind in [
            WireFilterKind::Equal(WireFilterBound::Int(1)),
            WireFilterKind::Equal(WireFilterBound::Str("x".into())),
            WireFilterKind::Equal(WireFilterBound::Blob(vec![0, 255])),
            WireFilterKind::Range {
                begin: i64::MIN,
                end: i64::MAX,
            },
            WireFilterKind::GeoWithinRegion("{\"type\":\"Polygon\"}".into()),
            WireFilterKind::GeoWithinRadius {
                longitude: -122.4,
                latitude: 37.8,
                radius: 1_000.0,
            },
            WireFilterKind::GeoContains("{\"type\":\"Point\"}".into()),
        ] {
            for target in [
                WireFilterTarget::Bin("b".into()),
                WireFilterTarget::Index("b_idx".into()),
            ] {
                for collection in [
                    WireCollectionIndex::Default,
                    WireCollectionIndex::List,
                    WireCollectionIndex::MapKeys,
                    WireCollectionIndex::MapValues,
                ] {
                    let filter = WireFilter {
                        target: target.clone(),
                        kind: kind.clone(),
                        collection,
                        context: vec![WireCtx::ListIndexCreate {
                            index: 1,
                            order: WireListOrder::Ordered,
                            pad: false,
                        }],
                        expression: None,
                    };
                    let bytes = encode_body(&filter).unwrap();
                    assert_eq!(decode_body::<WireFilter>(&bytes).unwrap(), filter);
                }
            }
        }
    }

    #[test]
    fn every_partition_selection_round_trips() {
        for partitions in [
            WirePartitions::All,
            WirePartitions::Id(4_095),
            WirePartitions::Range {
                begin: 0,
                count: 4_096,
            },
            WirePartitions::AfterKey(WireKey::Str("alice".into())),
        ] {
            let bytes = encode_body(&partitions).unwrap();
            assert_eq!(decode_body::<WirePartitions>(&bytes).unwrap(), partitions);
        }
    }

    #[test]
    fn a_page_round_trips_with_and_without_a_cursor() {
        let record = WireQueryRecord {
            key: WireRecordKey {
                namespace: "test".into(),
                set: "users".into(),
                digest: [7u8; 20],
                user_key: Some(WireKey::Int(1)),
            },
            bins: vec![("name".into(), WireValue::Str("Alice".into()))],
            generation: 3,
            ttl: Some(600),
        };
        for cursor in [Some(42), None] {
            let page = WireQueryPage {
                cursor,
                records: vec![record.clone()],
            };
            let bytes = encode_body(&page).unwrap();
            assert_eq!(decode_body::<WireQueryPage>(&bytes).unwrap(), page);
        }

        // A record whose key was never stored: the digest is all there is, and
        // that must survive rather than becoming an invented key.
        let anonymous = WireQueryRecord {
            key: WireRecordKey {
                namespace: "test".into(),
                set: String::new(),
                digest: [0u8; 20],
                user_key: None,
            },
            bins: Vec::new(),
            generation: 1,
            ttl: None,
        };
        let bytes = encode_body(&anonymous).unwrap();
        assert_eq!(decode_body::<WireQueryRecord>(&bytes).unwrap(), anonymous);
    }

    /// The end of a traversal is the absent cursor, and nothing else. An empty
    /// page with a cursor still set has to remain askable — the server hands
    /// `max_records` out per node, so a short or empty page does not mean the
    /// end.
    #[test]
    fn an_empty_page_is_not_the_end_unless_the_cursor_is_gone() {
        let more = WireQueryPage {
            cursor: Some(1),
            records: Vec::new(),
        };
        let done = WireQueryPage {
            cursor: None,
            records: Vec::new(),
        };
        assert!(more.cursor.is_some());
        assert!(done.cursor.is_none());
    }

    #[test]
    fn a_cursor_request_is_just_the_number() {
        let body = WireCursorBody { cursor: u64::MAX };
        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<WireCursorBody>(&bytes).unwrap(), body);
        // Small enough that a page request is never the expensive part of a
        // page: postcard varints a u64 in at most ten bytes.
        assert!(bytes.len() <= 10, "{} bytes", bytes.len());
    }
}
