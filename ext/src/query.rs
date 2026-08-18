// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

// `Filter`'s and `Statement`'s parameters are camelCase because PHP named
// arguments use the parameter name exactly as written; see `crate::policy` for
// why the lint cannot be scoped more tightly than the module.
#![allow(non_snake_case)]

//! Scans and queries: [`Statement`], [`Filter`], [`PartitionFilter`] and the
//! [`RecordSet`] a traversal is read through.
//!
//! # A scan is a query with no filter
//!
//! There is one method, `Client::query`, exactly as there is one in
//! `aerospike-core`. A [`Statement`] with no [`Filter`] visits every record of a
//! set — a scan — and one with a filter uses a secondary index. Two methods
//! would suggest two mechanisms; there is one, and the filter is what varies.
//!
//! ```php
//! // A scan.
//! $statement = new Aerospike\Statement("test", "users");
//! foreach ($client->query(null, null, $statement) as $record) {
//!     echo $record->bin("name"), "\n";
//! }
//!
//! // A query: the same thing with an index filter.
//! $statement = new Aerospike\Statement("test", "users", filter: Aerospike\Filter::range("age", 20, 30));
//! foreach ($client->query(null, null, $statement) as $record) { … }
//! ```
//!
//! # Records arrive a page at a time, and that is visible
//!
//! [`RecordSet`] is an `Iterator`, so `foreach` reads a whole traversal without
//! the caller ever thinking about pages. Underneath, each page is one round trip
//! to the daemon: `pageSize` on the policy sets how many records that is, and the
//! iterator fetches the next page when it runs off the end of the current one.
//!
//! Two consequences worth knowing:
//!
//! - **A `RecordSet` is readable once.** It is a position in a traversal, not a
//!   collection; `foreach`-ing one twice throws rather than silently continuing
//!   from wherever the first loop stopped.
//! - **An abandoned traversal should be closed**, and is: `close()` does it
//!   explicitly, and dropping the object does it too, so `break` out of a
//!   `foreach` releases the daemon's cursor rather than leaving it to expire.

use aerospike_php_ipc::query::{
    WireCursorBody, WireFilter, WireFilterBound, WireFilterKind, WireFilterTarget, WirePartitions,
    WireQueryBody, WireQueryPage, WireQueryRecord, WireStatement,
};
use aerospike_php_ipc::{decode_body, encode_body, opcode, StatusCode, WireValue};
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;
use ext_php_rs::zend::ce;

use crate::arg::Given;
use crate::enums::CollectionIndex;
use crate::error::{AeroError, AeroResult};
use crate::ops::{self, Expression};
use crate::record::{Bins, Key, Record};
use crate::settings::Settings;
use crate::transport;
use crate::value;

/// A secondary-index filter.
///
/// One class for all of the Rust client's filter constructors, because they vary
/// in three independent ways and a class can express that: what is compared (the
/// static method), whether a collection index is meant (the `$collection`
/// argument), and whether the index is named by bin or by its own name
/// (`…ByIndex`).
///
/// ```php
/// Aerospike\Filter::equal("name", "Alice");
/// Aerospike\Filter::range("age", 20, 30);
/// Aerospike\Filter::equal("tags", "urgent", Aerospike\CollectionIndex::List);
/// Aerospike\Filter::equalByIndex("users_by_name", "Alice");
///
/// // A filter on a value nested inside a map, and one on an expression index.
/// Aerospike\Filter::equal("meta", 1, Aerospike\CollectionIndex::MapValues)
///     ->context([Aerospike\Ctx::mapKey("inner")]);
/// Aerospike\Filter::range("computed", 0, 10)
///     ->expression(Aerospike\Expression::ael('$.a + $.b'));
/// ```
///
/// **A query needs the index to exist.** A filter naming a bin with no secondary
/// index on it is a server error (result code 201, `INDEX_NOTFOUND`), not an
/// empty result: the server cannot answer a query it has no index for, and
/// silently scanning instead would turn a fast query into a full-set read.
#[php_class]
#[php(name = "Aerospike\\Filter")]
#[derive(Debug, Clone)]
pub struct Filter {
    filter: WireFilter,
}

#[php_impl]
impl Filter {
    /// Records whose indexed value equals `$value`.
    ///
    /// `$value` is an `int`, a `string`, or an `Aerospike\Blob` for a blob index
    /// — the three types Aerospike indexes. Anything else is refused here rather
    /// than sent as a filter the server would reject without saying which
    /// argument was wrong.
    #[php(defaults(collection = None))]
    pub fn equal(
        bin: String,
        value: &Zval,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        let kind = WireFilterKind::Equal(bound(value, &bin)?);
        Filter::built(WireFilterTarget::Bin(bin), kind, collection)
    }

    /// [`Filter::equal`], against a named secondary index.
    ///
    /// For a bin with more than one index on it, where the bin name alone would
    /// not say which to use.
    #[php(defaults(collection = None))]
    pub fn equal_by_index(
        index: String,
        value: &Zval,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        let kind = WireFilterKind::Equal(bound(value, &index)?);
        Filter::built(WireFilterTarget::Index(index), kind, collection)
    }

    /// Records whose indexed integer is between `$begin` and `$end`, inclusive.
    ///
    /// Integers only, which is the Rust client's rule and the server's: a string
    /// index has no ordering to range over, so a string range would be quietly
    /// wrong rather than refused.
    #[php(defaults(collection = None))]
    pub fn range(
        bin: String,
        begin: i64,
        end: i64,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        Filter::built(
            WireFilterTarget::Bin(bin),
            range_kind(begin, end)?,
            collection,
        )
    }

    /// [`Filter::range`], against a named secondary index.
    #[php(defaults(collection = None))]
    pub fn range_by_index(
        index: String,
        begin: i64,
        end: i64,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        Filter::built(
            WireFilterTarget::Index(index),
            range_kind(begin, end)?,
            collection,
        )
    }

    /// Points inside a GeoJSON region.
    ///
    /// `$region` is a GeoJSON polygon or a circle, as a document — the same text
    /// an `Aerospike\GeoJson` wraps.
    #[php(defaults(collection = None))]
    pub fn geo_within_region(
        bin: String,
        region: String,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        Filter::built(
            WireFilterTarget::Bin(bin),
            WireFilterKind::GeoWithinRegion(geojson(region, "region")?),
            collection,
        )
    }

    /// `Filter::geoWithinRegion()`, against a named secondary index.
    #[php(defaults(collection = None))]
    pub fn geo_within_region_by_index(
        index: String,
        region: String,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        Filter::built(
            WireFilterTarget::Index(index),
            WireFilterKind::GeoWithinRegion(geojson(region, "region")?),
            collection,
        )
    }

    /// Points within `$radius` metres of a longitude and latitude.
    ///
    /// Longitude first, as GeoJSON orders coordinates — not latitude first, as a
    /// map application might. Getting them the wrong way round produces a query
    /// that runs and finds nothing, so the order is worth reading twice.
    #[php(defaults(collection = None))]
    pub fn geo_within_radius(
        bin: String,
        longitude: f64,
        latitude: f64,
        radius: f64,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        Filter::built(
            WireFilterTarget::Bin(bin),
            radius_kind(longitude, latitude, radius)?,
            collection,
        )
    }

    /// `Filter::geoWithinRadius()`, against a named secondary index.
    #[php(defaults(collection = None))]
    #[allow(clippy::too_many_arguments)]
    pub fn geo_within_radius_by_index(
        index: String,
        longitude: f64,
        latitude: f64,
        radius: f64,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        Filter::built(
            WireFilterTarget::Index(index),
            radius_kind(longitude, latitude, radius)?,
            collection,
        )
    }

    /// Regions that contain a GeoJSON point.
    ///
    /// The inverse of `Filter::geoWithinRegion()`: there the bin holds points and
    /// the filter is a region, here the bin holds regions and the filter is a
    /// point.
    #[php(defaults(collection = None))]
    pub fn geo_contains(
        bin: String,
        point: String,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        Filter::built(
            WireFilterTarget::Bin(bin),
            WireFilterKind::GeoContains(geojson(point, "point")?),
            collection,
        )
    }

    /// `Filter::geoContains()`, against a named secondary index.
    #[php(defaults(collection = None))]
    pub fn geo_contains_by_index(
        index: String,
        point: String,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        Filter::built(
            WireFilterTarget::Index(index),
            WireFilterKind::GeoContains(geojson(point, "point")?),
            collection,
        )
    }

    /// The same filter, applied inside a nested collection.
    ///
    /// For an index built on a path — a list inside a map, a map inside a list —
    /// the path has to travel with the filter, because it is part of what the
    /// index covers. Returns a new filter; this class is immutable, as the
    /// policies are.
    pub fn context(&self, ctx: Vec<&Zval>) -> PhpResult<Filter> {
        let mut filter = self.filter.clone();
        filter.context = ops::context_list(&ctx)?;
        Ok(Filter { filter })
    }

    /// The same filter, against an expression-based secondary index.
    ///
    /// The expression is the one the *index* was created with, not a record
    /// filter: it identifies which index to use. A record filter belongs on the
    /// policy, where it applies to every record the query returns.
    pub fn expression(&self, expression: Given<&Expression>) -> PhpResult<Filter> {
        let expression = expression.required("expression")?;
        let mut filter = self.filter.clone();
        filter.expression = Some(expression.to_wire());
        Ok(Filter { filter })
    }

    /// The bin or index this filter names.
    pub fn target(&self) -> String {
        match &self.filter.target {
            WireFilterTarget::Bin(name) | WireFilterTarget::Index(name) => name.clone(),
        }
    }

    /// Whether the filter names a secondary index rather than a bin.
    pub fn is_by_index(&self) -> bool {
        matches!(self.filter.target, WireFilterTarget::Index(_))
    }

    /// How many context steps the filter carries.
    pub fn context_depth(&self) -> i64 {
        i64::try_from(self.filter.context.len()).unwrap_or(i64::MAX)
    }
}

impl Filter {
    /// The three independent choices, combined.
    ///
    /// # Errors
    /// [`AeroError`] for a blank bin or index name — the server would reject it
    /// as a parameter error that names neither the filter nor the field.
    fn built(
        target: WireFilterTarget,
        kind: WireFilterKind,
        collection: Option<Given<CollectionIndex>>,
    ) -> PhpResult<Filter> {
        let collection = Given::or_none(collection, "collection")?.unwrap_or_default();
        Ok(Filter::checked(target, kind, collection)?)
    }

    /// `built` without the PHP argument wrapper, so the naming
    /// rules are testable without a running PHP.
    ///
    /// # Errors
    /// An `AeroError` for a blank bin or index name.
    pub fn checked(
        target: WireFilterTarget,
        kind: WireFilterKind,
        collection: CollectionIndex,
    ) -> AeroResult<Filter> {
        let (what, name) = match &target {
            WireFilterTarget::Bin(name) => ("bin", name),
            WireFilterTarget::Index(name) => ("index", name),
        };
        if name.trim().is_empty() {
            return Err(AeroError::client(format!(
                "a filter needs a {what} name; \"\" is not one"
            )));
        }
        Ok(Filter {
            filter: WireFilter {
                target,
                kind,
                collection: collection.to_wire(),
                context: Vec::new(),
                expression: None,
            },
        })
    }

    /// The contract's filter.
    #[must_use]
    pub fn to_wire(&self) -> WireFilter {
        self.filter.clone()
    }
}

/// Which records a scan or query visits: a namespace, a set, which bins, and
/// optionally a filter.
///
/// ```php
/// new Aerospike\Statement("test", "users");                       // a scan
/// new Aerospike\Statement("test", "users", Aerospike\Bins::some(["name"]));
/// new Aerospike\Statement("test", "users", filter: Aerospike\Filter::equal("age", 30));
/// new Aerospike\Statement("test");                                // every set
/// ```
///
/// Immutable, like the policies and for the same reason: a statement is worth
/// building once and reusing, including across requests, and one that could be
/// mutated afterwards would be one whose meaning depends on when it was read.
#[php_class]
#[php(name = "Aerospike\\Statement")]
#[derive(Debug, Clone)]
pub struct Statement {
    statement: WireStatement,
}

#[php_impl]
impl Statement {
    /// Name what to visit.
    ///
    /// An empty or omitted `$set` means **every set in the namespace**, which is
    /// not the same as the null set — a single-record `Key` with an empty set
    /// names the null set, and this names all of them. That asymmetry is the
    /// server's; naming it here is better than the surprise.
    #[php(defaults(set = None, bins = None, filter = None))]
    pub fn __construct(
        namespace: String,
        set: Option<Given<String>>,
        bins: Option<Given<&Bins>>,
        filter: Option<Given<&Filter>>,
    ) -> PhpResult<Statement> {
        let set = Given::or_none(set, "set")?.unwrap_or_default();
        if namespace.trim().is_empty() {
            return Err(AeroError::client(
                "a Statement needs a namespace; \"\" is not one. An empty set is meaningful — it \
                 means every set in the namespace — but an empty namespace is not",
            )
            .into());
        }
        Ok(Statement {
            statement: WireStatement {
                namespace,
                set,
                bins: Given::or_none(bins, "bins")?.map_or_else(Bins::all_wire, Bins::to_wire),
                filter: Given::or_none(filter, "filter")?.map(Filter::to_wire),
            },
        })
    }

    /// The namespace.
    pub fn namespace(&self) -> String {
        self.statement.namespace.clone()
    }

    /// The set name; `""` for every set in the namespace.
    pub fn set_name(&self) -> String {
        self.statement.set.clone()
    }

    /// Whether this is a scan — a statement with no filter.
    pub fn is_scan(&self) -> bool {
        self.statement.filter.is_none()
    }
}

impl Statement {
    /// A filter-less statement, for [`crate::client::Client::scan`].
    ///
    /// `pub(crate)` and not a PHP method: PHP already has the constructor, and a
    /// second way to build the same thing would be a second thing to document.
    ///
    /// # Errors
    /// [`AeroError`] for a blank namespace, as the constructor does.
    pub(crate) fn of_scan(
        namespace: String,
        set: String,
        bin_names: Option<Vec<String>>,
    ) -> AeroResult<Statement> {
        if namespace.trim().is_empty() {
            return Err(AeroError::client(
                "scan() needs a namespace; \"\" is not one. An empty set is meaningful — it means \
                 every set in the namespace — but an empty namespace is not",
            ));
        }
        Ok(Statement {
            statement: WireStatement {
                namespace,
                set,
                bins: match bin_names {
                    None => Bins::all_wire(),
                    Some(names) => Bins::only(names)?.to_wire(),
                },
                filter: None,
            },
        })
    }

    /// The contract's statement.
    #[must_use]
    pub fn to_wire(&self) -> WireStatement {
        self.statement.clone()
    }
}

/// Which partitions a scan or query covers.
///
/// The whole ring unless told otherwise. The narrower forms are how one scan is
/// divided between several workers: give each a partition range and no record is
/// seen twice, without any coordination between them.
///
/// ```php
/// Aerospike\PartitionFilter::all();
/// Aerospike\PartitionFilter::byRange(0, 1024);      // a quarter of the ring
/// Aerospike\PartitionFilter::byId(7);
/// Aerospike\PartitionFilter::after($key);           // resume after a key
/// ```
#[php_class]
#[php(name = "Aerospike\\PartitionFilter")]
#[derive(Debug, Clone)]
pub struct PartitionFilter {
    partitions: WirePartitions,
}

#[php_impl]
impl PartitionFilter {
    /// Every partition. The same as passing `null`.
    pub fn all() -> PartitionFilter {
        PartitionFilter {
            partitions: WirePartitions::All,
        }
    }

    /// One partition, by id.
    ///
    /// Ids run from 0 to 4095. A record's partition is a function of its digest,
    /// so this is only useful for dividing a scan up, never for finding a
    /// particular record.
    pub fn by_id(id: i64) -> PhpResult<PartitionFilter> {
        Ok(PartitionFilter {
            partitions: WirePartitions::Id(partition_id(id, "id")?),
        })
    }

    /// A contiguous range of partitions: `$count` of them, from `$begin`.
    pub fn by_range(begin: i64, count: i64) -> PhpResult<PartitionFilter> {
        if count <= 0 {
            return Err(AeroError::client(format!(
                "a partition range needs at least one partition, but count is {count}"
            ))
            .into());
        }
        Ok(PartitionFilter {
            partitions: WirePartitions::Range {
                begin: partition_id(begin, "begin")?,
                count: partition_id(count, "count")?,
            },
        })
    }

    /// Records after this key's digest, in the partition that holds it.
    ///
    /// Digest order, which is not key order and not insertion order — so this
    /// resumes a scan that stopped at a known record, and is not a way to page
    /// through a set in any meaningful sequence.
    ///
    /// Primary-index scans only: a digest is not enough to resume a
    /// secondary-index query, so a statement with a filter must not use this.
    pub fn after(key: Given<&Key>) -> PhpResult<PartitionFilter> {
        let (_, _, user_key) = key.required("key")?.parts();
        Ok(PartitionFilter {
            partitions: WirePartitions::AfterKey(user_key),
        })
    }

    /// Whether this covers the whole ring.
    pub fn is_all(&self) -> bool {
        matches!(self.partitions, WirePartitions::All)
    }
}

impl PartitionFilter {
    /// The contract's selection.
    #[must_use]
    pub fn to_wire(&self) -> WirePartitions {
        self.partitions.clone()
    }

    /// What an omitted selection means: the whole ring.
    #[must_use]
    pub const fn all_wire() -> WirePartitions {
        WirePartitions::All
    }
}

/// The records a scan or query returns, read one page at a time.
///
/// An `Iterator`, so `foreach` reads the whole traversal:
///
/// ```php
/// foreach ($client->query(null, null, $statement) as $record) {
///     echo $record->key()?->userKey(), " ", $record->bin("name"), "\n";
/// }
/// ```
///
/// # Read once, and close what you abandon
///
/// This is a *position* in a traversal, not a collection of records. Reading it
/// twice throws: the second `foreach` cannot start from the beginning — the
/// records are on the server, and the first loop consumed them — and silently
/// continuing from the middle would be worse than saying so.
///
/// Abandoning one is fine. `break` out of a `foreach`, or let the variable go out
/// of scope, and the daemon's cursor is released — the object closes itself when
/// it is destroyed. `close()` does it explicitly, for a traversal held in a
/// long-lived variable.
///
/// # A page boundary is invisible except in timing
///
/// Every `pageSize` records, `next()` makes a round trip. Nothing about the
/// iteration changes; the only visible effect is that one step in every page
/// takes as long as a database call. A larger `pageSize` trades memory — the page
/// is held in the daemon and then in this process — for fewer round trips.
#[php_class]
#[php(name = "Aerospike\\RecordSet")]
#[php(implements(ce = ce::iterator, stub = "\\Iterator"))]
#[derive(Debug)]
pub struct RecordSet {
    instance: String,
    settings: Settings,
    /// `None` once the traversal has finished, so there is nothing to close and
    /// nothing more to ask for.
    cursor: Option<u64>,
    /// The page being read.
    page: Vec<WireQueryRecord>,
    /// Position within [`page`](Self::page).
    index: usize,
    /// Position across the whole traversal, which is what `key()` reports.
    position: i64,
    /// Whether `rewind()` has been seen, so a second `foreach` can be refused.
    started: bool,
}

#[php_impl]
impl RecordSet {
    /// The record at the current position, or `null` past the end.
    pub fn current(&self) -> Option<Record> {
        self.page.get(self.index).map(Record::from_query)
    }

    /// The current position, counted across the whole traversal rather than
    /// within the page — so it keeps rising past a page boundary, as a caller
    /// would expect and as the pages themselves do not.
    pub fn key(&self) -> i64 {
        self.position
    }

    /// Advance, fetching the next page when the current one runs out.
    pub fn next(&mut self) -> PhpResult<()> {
        if self.index < self.page.len() {
            self.index += 1;
            self.position += 1;
        }
        Ok(self.fill()?)
    }

    /// Start iterating.
    ///
    /// A no-op the first time, because the first page was already fetched when
    /// the traversal opened. A *second* time it throws: see the class docs — a
    /// position cannot be rewound to a beginning that no longer exists.
    pub fn rewind(&mut self) -> PhpResult<()> {
        if self.started {
            return Err(AeroError::client(
                "a RecordSet can only be iterated once: it is a position in a scan, not a \
                 collection, and the records it has already returned are gone from it. Run the \
                 query again for a second pass, or collect what you need on the first",
            )
            .into());
        }
        self.started = true;
        Ok(())
    }

    /// Whether there is a record at the current position.
    pub fn valid(&self) -> bool {
        self.index < self.page.len()
    }

    /// Release the daemon's cursor, ending the traversal here.
    ///
    /// Idempotent, and safe on a traversal that already finished — a finished one
    /// has no cursor to release. Whatever is left of the current page is
    /// discarded, so `valid()` is `false` afterwards.
    pub fn close(&mut self) -> PhpResult<()> {
        self.page.clear();
        self.index = 0;
        Ok(self.release()?)
    }

    /// Whether the traversal still has a cursor open on the daemon.
    ///
    /// `false` for one that finished on its own, as much as for one that was
    /// closed: neither is holding anything.
    pub fn is_open(&self) -> bool {
        self.cursor.is_some()
    }

    /// How many records this traversal has produced so far.
    ///
    /// Not `Countable`: how many a scan *will* produce is not knowable without
    /// running it, and a `count()` that answered "so far" while looking like a
    /// total would be the most misleading thing this class could offer.
    pub fn seen(&self) -> i64 {
        self.position + i64::from(self.valid())
    }

    /// The next record, or `null` at the end — a pull rather than an advance.
    ///
    /// ```php
    /// while (($record = $recordSet->nextRecord()) !== null) { … }
    /// ```
    ///
    /// This is what the 1.x client's `Recordset::next()` did, and it is spelled
    /// differently here because [`RecordSet::next`] is `Iterator::next`, which PHP
    /// declares as returning nothing. Old code that reads `next()`'s return value
    /// gets `null` and iterates zero times, so it has to change one way or
    /// another; this is the smaller of the two changes, `foreach` being the other.
    ///
    /// Do not mix this with `foreach` on one traversal: each advances the same
    /// position, so together they skip records.
    pub fn next_record(&mut self) -> PhpResult<Option<Record>> {
        if !self.started {
            self.started = true;
        } else if self.index < self.page.len() {
            self.index += 1;
            self.position += 1;
            self.fill()?;
        }
        Ok(self.current())
    }

    /// Whether the traversal is still producing records.
    ///
    /// The 1.x spelling of [`RecordSet::valid`].
    #[php(name = "getActive")]
    pub fn legacy_get_active(&self) -> bool {
        self.valid()
    }
}

impl RecordSet {
    /// Open a traversal from its first page.
    ///
    /// The first page is fetched by `Client::query` itself, so a traversal that
    /// fits in one page never registers a cursor and this never has to ask for
    /// anything.
    ///
    /// # Errors
    /// Whatever fetching a further page fails with, which can happen here: a
    /// first page may legitimately be empty with more to come.
    pub fn open(
        instance: String,
        settings: Settings,
        page: WireQueryPage,
    ) -> AeroResult<RecordSet> {
        let mut set = RecordSet {
            instance,
            settings,
            cursor: page.cursor,
            page: page.records,
            index: 0,
            position: 0,
            started: false,
        };
        set.fill()?;
        Ok(set)
    }

    /// Make sure the current position has a record, or the traversal is over.
    ///
    /// A loop, not a single fetch, because **an empty page is not the end**: the
    /// server divides a page's record budget between nodes, so a page can come
    /// back empty while partitions remain unread. Stopping at the first empty one
    /// would truncate the scan — silently, which is the failure this whole design
    /// exists to prevent.
    fn fill(&mut self) -> AeroResult<()> {
        while self.index >= self.page.len() {
            let Some(cursor) = self.cursor else {
                return Ok(());
            };
            let page = self.fetch(cursor)?;
            self.cursor = page.cursor;
            self.page = page.records;
            self.index = 0;
        }
        Ok(())
    }

    /// Ask the daemon for the next page.
    ///
    /// A failure here means the traversal ends part-way, which is worth saying
    /// out loud: the records already read are good, and the ones not reached were
    /// never returned. That is the difference between this and a failure of the
    /// *first* page, which returned nothing at all.
    fn fetch(&self, cursor: u64) -> AeroResult<WireQueryPage> {
        let payload = encode_body(&WireCursorBody { cursor }).map_err(AeroError::codec)?;
        let (header, reply) = transport::call(
            &self.instance,
            &self.settings,
            opcode::QUERY_NEXT,
            "query",
            &payload,
        )?;
        if !header.status().is_ok() {
            let failure = AeroError::from_reply(&header, &reply, "query");
            return Err(if is_expired(header.status()) {
                failure.context(&format!(
                    "the scan stopped after {} record(s) and cannot be resumed",
                    self.position
                ))
            } else {
                failure.context(&format!(
                    "the scan stopped after {} record(s)",
                    self.position
                ))
            });
        }
        decode_body(&reply).map_err(AeroError::codec)
    }

    /// Tell the daemon to forget the cursor, if there is one.
    fn release(&mut self) -> AeroResult<()> {
        let Some(cursor) = self.cursor.take() else {
            return Ok(());
        };
        let payload = encode_body(&WireCursorBody { cursor }).map_err(AeroError::codec)?;
        let (header, reply) = transport::call(
            &self.instance,
            &self.settings,
            opcode::QUERY_CLOSE,
            "query close",
            &payload,
        )?;
        if !header.status().is_ok() {
            return Err(AeroError::from_reply(&header, &reply, "query close"));
        }
        Ok(())
    }
}

/// Close an abandoned traversal.
///
/// This is what makes `break` out of a `foreach` release the daemon's cursor
/// rather than leaving it to expire. A `__destruct` method would be the PHP way
/// to say it, but `Drop` is the one that cannot be missed: it runs when the
/// object is freed however that happens, including when a request ends with the
/// traversal still in a variable.
///
/// A failure here is deliberately swallowed. There is nothing a destructor could
/// do with it — throwing from one is a fatal error in PHP — and the cursor
/// expires on its own regardless, which is exactly the case expiry exists for.
impl Drop for RecordSet {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

/// A filter bound from a PHP value.
///
/// Narrower than a bin value on purpose: Aerospike indexes integers, strings and
/// blobs. Refusing the rest here names the argument, where the server's own
/// refusal would be a parameter error with nothing in it.
fn bound(value: &Zval, target: &str) -> AeroResult<WireFilterBound> {
    match value::zval_to_wire(value, &value::Path::bin(target))? {
        WireValue::Int(number) => Ok(WireFilterBound::Int(number)),
        WireValue::Str(text) => Ok(WireFilterBound::Str(text)),
        WireValue::Blob(bytes) => Ok(WireFilterBound::Blob(bytes)),
        other => Err(AeroError::client(format!(
            "a filter compares an int, a string or an Aerospike\\Blob — the three types Aerospike \
             builds an index over — and this is {}. A filter on anything else would have to be an \
             expression on the policy instead",
            describe(&other)
        ))),
    }
}

/// A checked range.
///
/// # Errors
/// [`AeroError`] when the bounds are the wrong way round, which the server
/// answers with an empty result rather than a complaint — so it looks like "no
/// matching records" and is really "no possible records".
fn range_kind(begin: i64, end: i64) -> AeroResult<WireFilterKind> {
    if begin > end {
        return Err(AeroError::client(format!(
            "a range filter's begin must not be above its end, but {begin} > {end}; the server \
             would answer this with no records rather than an error, which reads like an empty \
             set"
        )));
    }
    Ok(WireFilterKind::Range { begin, end })
}

/// A checked radius.
fn radius_kind(longitude: f64, latitude: f64, radius: f64) -> AeroResult<WireFilterKind> {
    if !(-180.0..=180.0).contains(&longitude) {
        return Err(AeroError::client(format!(
            "longitude must be between -180 and 180 degrees, but is {longitude}. Note the order: \
             longitude first, as GeoJSON writes coordinates"
        )));
    }
    if !(-90.0..=90.0).contains(&latitude) {
        return Err(AeroError::client(format!(
            "latitude must be between -90 and 90 degrees, but is {latitude}. Note the order: \
             longitude first, as GeoJSON writes coordinates"
        )));
    }
    if !(radius > 0.0) {
        return Err(AeroError::client(format!(
            "a radius must be a positive number of metres, but is {radius}"
        )));
    }
    Ok(WireFilterKind::GeoWithinRadius {
        longitude,
        latitude,
        radius,
    })
}

/// A GeoJSON document that is at least not blank.
///
/// The document itself is the server's to parse — reimplementing that here would
/// mean two parsers that must agree — but an empty one is worth catching, because
/// it is almost always a variable that was empty.
fn geojson(document: String, what: &str) -> AeroResult<String> {
    if document.trim().is_empty() {
        return Err(AeroError::client(format!(
            "a geospatial filter needs a GeoJSON {what}; \"\" is not one"
        )));
    }
    Ok(document)
}

/// A partition id or count that fits the ring.
///
/// PHP has one integer type and it is signed, so a negative id would otherwise
/// wrap into a partition number the server has no answer for.
fn partition_id(value: i64, what: &str) -> AeroResult<u32> {
    // 4096 is the count, so a valid *id* is 0..=4095 and a valid *count* is
    // 1..=4096; both fit here, and the server rejects the combination that does
    // not. Bounding it at all is what stops a negative from wrapping.
    if !(0..=4_096).contains(&value) {
        return Err(AeroError::client(format!(
            "a partition {what} must be between 0 and 4096, but is {value}; Aerospike has 4096 \
             partitions"
        )));
    }
    u32::try_from(value).map_err(|_| {
        AeroError::client(format!("a partition {what} of {value} is not a partition"))
    })
}

/// What kind of value this is, for a message that has to name it.
fn describe(value: &WireValue) -> &'static str {
    match value {
        WireValue::Nil => "null",
        WireValue::Bool(_) => "a bool",
        WireValue::Float(_) => "a float",
        WireValue::List(_) => "a list",
        WireValue::Map(_) | WireValue::OrderedMap(_) | WireValue::SortedMap(_) => "a map",
        WireValue::GeoJson(_) => "an Aerospike\\GeoJson",
        WireValue::Hll(_) => "an Aerospike\\Hll",
        WireValue::Infinity => "Aerospike\\Infinity",
        WireValue::Wildcard => "Aerospike\\Wildcard",
        _ => "not a value a filter can compare",
    }
}

/// The contract's query request, from the pieces PHP gave.
///
/// Here rather than in `client.rs` so that the whole request shape lives beside
/// the classes it is built from.
#[must_use]
pub fn request(
    instance: String,
    policy: Option<&crate::policy::QueryPolicy>,
    partitions: Option<&PartitionFilter>,
    statement: &Statement,
) -> WireQueryBody {
    WireQueryBody {
        instance,
        policy: policy.map(crate::policy::QueryPolicy::to_wire).unwrap_or_default(),
        statement: statement.to_wire(),
        partitions: partitions.map_or_else(PartitionFilter::all_wire, PartitionFilter::to_wire),
        page_size: policy.map_or(0, crate::policy::QueryPolicy::wire_page_size),
        max_records: policy.map_or(0, crate::policy::QueryPolicy::wire_max_records),
        records_per_second: policy.map_or(0, crate::policy::QueryPolicy::wire_records_per_second),
        include_bin_data: policy.is_none_or(crate::policy::QueryPolicy::wire_include_bin_data),
    }
}

/// Whether a reply says the cursor is gone, which a caller may want to tell
/// apart from any other failure: the records this traversal had not reached are
/// still there, and only the place it had got to is lost.
#[must_use]
pub const fn is_expired(status: StatusCode) -> bool {
    status.0 == StatusCode::CURSOR_EXPIRED.0
}

/// The key a scan result carried, for [`Record`].
///
/// A digest is not reversible, so a record whose user key was never stored has
/// none — and this returns `None` rather than inventing one.
#[must_use]
pub fn stored_key(record: &WireQueryRecord) -> Option<Key> {
    record.key.user_key.as_ref().map(|user_key| {
        // `from_wire`, not `from_parts`: a scanned key arrives with the server's
        // digest, and that is the only way this extension ever learns one — it
        // does not link `aerospike-core`, so it cannot compute a digest itself.
        Key::from_wire(
            record.key.namespace.clone(),
            record.key.set.clone(),
            user_key.clone(),
            record.key.digest.to_vec(),
        )
    })
}

/// The digest a scan result carried. Always present for a scanned record.
#[must_use]
pub fn digest_of(record: &WireQueryRecord) -> Vec<u8> {
    record.key.digest.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::query::WireCollectionIndex;
    use aerospike_php_ipc::WireKey;

    #[test]
    fn a_filter_needs_a_name_for_whatever_it_targets() {
        for target in [
            WireFilterTarget::Bin("  ".into()),
            WireFilterTarget::Index(String::new()),
        ] {
            let expected = match &target {
                WireFilterTarget::Bin(_) => "bin",
                WireFilterTarget::Index(_) => "index",
            };
            let error = Filter::checked(
                target,
                WireFilterKind::Equal(WireFilterBound::Int(1)),
                CollectionIndex::Scalar,
            )
            .expect_err("a blank name must be refused");
            assert!(error.to_string().contains(expected), "{error}");
        }

        let ok = Filter::checked(
            WireFilterTarget::Bin("age".into()),
            WireFilterKind::Range { begin: 1, end: 2 },
            CollectionIndex::List,
        )
        .unwrap();
        assert_eq!(ok.to_wire().collection, WireCollectionIndex::List);
        assert!(ok.to_wire().context.is_empty());
        assert!(ok.to_wire().expression.is_none());
    }

    /// A backwards range is answered by the server with no records, which reads
    /// exactly like "nothing matched" — so it has to be caught here.
    #[test]
    fn a_backwards_range_is_refused_rather_than_answered_with_nothing() {
        let error = range_kind(30, 20).expect_err("a backwards range must be refused");
        assert!(error.to_string().contains("30 > 20"), "{error}");

        assert_eq!(
            range_kind(20, 30).unwrap(),
            WireFilterKind::Range { begin: 20, end: 30 }
        );
        // A single-value range is legal, and is how an equality on an integer
        // index is sometimes written.
        assert_eq!(
            range_kind(7, 7).unwrap(),
            WireFilterKind::Range { begin: 7, end: 7 }
        );
    }

    #[test]
    fn a_radius_filter_checks_its_coordinates_and_says_which_order_they_go_in() {
        // The mistake this catches: latitude and longitude the wrong way round,
        // which for most of the world produces a query that runs and finds
        // nothing.
        let error = radius_kind(37.8, -122.4, 100.0).expect_err("latitude out of range");
        assert!(error.to_string().contains("latitude"), "{error}");
        assert!(error.to_string().contains("longitude first"), "{error}");

        let error = radius_kind(200.0, 0.0, 100.0).expect_err("longitude out of range");
        assert!(error.to_string().contains("longitude"), "{error}");

        for bad in [0.0, -1.0] {
            let error = radius_kind(0.0, 0.0, bad).expect_err("a radius must be positive");
            assert!(error.to_string().contains("metres"), "{error}");
        }

        assert!(radius_kind(-122.4, 37.8, 1_000.0).is_ok());
        // The extremes are legal coordinates.
        assert!(radius_kind(180.0, 90.0, 1.0).is_ok());
        assert!(radius_kind(-180.0, -90.0, 1.0).is_ok());
    }

    #[test]
    fn a_blank_geojson_document_is_refused() {
        let error = geojson("   ".into(), "region").expect_err("a blank document must be refused");
        assert!(error.to_string().contains("region"), "{error}");
        assert_eq!(geojson("{}".into(), "point").unwrap(), "{}");
    }

    #[test]
    fn a_partition_number_cannot_be_negative_or_off_the_ring() {
        assert_eq!(partition_id(0, "id").unwrap(), 0);
        assert_eq!(partition_id(4_095, "id").unwrap(), 4_095);
        assert_eq!(partition_id(4_096, "count").unwrap(), 4_096);

        for bad in [-1, 4_097, i64::MIN, i64::MAX] {
            let error = partition_id(bad, "id").expect_err("must be refused");
            assert!(error.to_string().contains("4096"), "{error}");
        }
    }

    #[test]
    fn a_filter_bound_is_one_of_the_three_types_an_index_covers() {
        // The conversions themselves need real zvals; what is testable here is
        // that the *rejection* names the type and points at the alternative.
        let error =
            bound_of(WireValue::Float(1.5)).expect_err("a float is not an indexable bound");
        assert!(error.to_string().contains("a float"), "{error}");
        assert!(error.to_string().contains("expression"), "{error}");

        assert_eq!(
            bound_of(WireValue::Int(7)).unwrap(),
            WireFilterBound::Int(7)
        );
        assert_eq!(
            bound_of(WireValue::Str("x".into())).unwrap(),
            WireFilterBound::Str("x".into())
        );
        assert_eq!(
            bound_of(WireValue::Blob(vec![1])).unwrap(),
            WireFilterBound::Blob(vec![1])
        );
    }

    /// [`bound`] without the zval, so the rule is testable without PHP.
    fn bound_of(value: WireValue) -> AeroResult<WireFilterBound> {
        match value {
            WireValue::Int(number) => Ok(WireFilterBound::Int(number)),
            WireValue::Str(text) => Ok(WireFilterBound::Str(text)),
            WireValue::Blob(bytes) => Ok(WireFilterBound::Blob(bytes)),
            other => Err(AeroError::client(format!(
                "a filter compares an int, a string or an Aerospike\\Blob — the three types \
                 Aerospike builds an index over — and this is {}. A filter on anything else would \
                 have to be an expression on the policy instead",
                describe(&other)
            ))),
        }
    }

    #[test]
    fn a_request_with_no_policy_asks_for_the_daemons_defaults() {
        let statement = Statement {
            statement: WireStatement {
                namespace: "test".into(),
                set: "users".into(),
                bins: aerospike_php_ipc::BinSelector::All,
                filter: None,
            },
        };
        let body = request("default".into(), None, None, &statement);

        assert!(body.policy.is_empty(), "no policy means override nothing");
        assert_eq!(body.page_size, 0, "zero means the daemon's configured page size");
        assert_eq!(body.max_records, 0, "zero means no ceiling");
        assert_eq!(body.records_per_second, 0, "zero means unlimited");
        // The one that is not zero-as-unset: bins come back unless asked
        // otherwise, because that is what every other read does.
        assert!(body.include_bin_data);
        assert_eq!(body.partitions, WirePartitions::All);
    }

    #[test]
    fn the_expired_status_is_recognised_and_nothing_else_is() {
        assert!(is_expired(StatusCode::CURSOR_EXPIRED));
        for other in [
            StatusCode::OK,
            StatusCode::SERVER,
            StatusCode::TIMEOUT,
            StatusCode::INVALID_REQUEST,
            StatusCode::INTERNAL,
        ] {
            assert!(!is_expired(other), "{}", other.label());
        }
    }

    fn scanned(user_key: Option<WireKey>) -> WireQueryRecord {
        WireQueryRecord {
            key: aerospike_php_ipc::query::WireRecordKey {
                namespace: "test".into(),
                set: "users".into(),
                digest: [3u8; 20],
                user_key,
            },
            bins: Vec::new(),
            generation: 1,
            ttl: None,
        }
    }

    #[test]
    fn a_scanned_record_has_a_digest_and_a_key_only_if_one_was_stored() {
        let with_key = scanned(Some(WireKey::Str("alice".into())));
        assert_eq!(digest_of(&with_key), vec![3u8; 20]);
        let key = stored_key(&with_key).expect("the key was stored");
        assert_eq!(key.parts().0, "test");
        assert_eq!(key.parts().1, "users");

        // Without `sendKey` the digest is the only identity there is, and a
        // digest cannot be turned back into a key.
        let anonymous = scanned(None);
        assert_eq!(digest_of(&anonymous), vec![3u8; 20]);
        assert!(stored_key(&anonymous).is_none());
    }
}
