// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Scans and queries, one page per round trip.
//!
//! The contract's side of this is [`aerospike_php_ipc::query`]; this is the
//! daemon's. A traversal is a [`Cursor`] — a `PartitionFilter` plus the request
//! it came from — kept in [`Cursors`] between pages, and the number PHP holds is
//! its key.
//!
//! # A page is a whole query, resumed
//!
//! Each page runs `client.query` afresh with `max_records` set to the page size,
//! drains the recordset, and takes the `PartitionFilter` back out of it: that
//! filter *is* the cursor, and handing it to the next query resumes after the
//! last record of each partition. It is the paging pattern
//! `aerospike-core`'s own tests and `aerospike-sdk` use, so this module invents
//! no resumption logic of its own.
//!
//! The consequence worth stating: **a cursor holds no server-side resources
//! between pages.** No connection, no open recordset, no task — only the
//! progress record and the request to repeat. A worker that dies mid-scan
//! therefore strands memory, which [`Cursors::expire`] reclaims, and nothing
//! else.
//!
//! # Why the whole page is collected before answering
//!
//! A page is materialised into a `Vec` rather than streamed into the reply as it
//! arrives, because the reply is one shared-memory frame. That bounds memory at
//! one page, which is what `page_size` is for — and it is why the daemon caps
//! `page_size` rather than trusting the request: a caller asking for a page of
//! ten million records would otherwise be asking the daemon to hold the set.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use aerospike_core::query::Filter;
use aerospike_core::{
    Bins, Client, CollectionIndexType, PartitionFilter, Record, Statement, Value,
};
use aerospike_php_ipc::query::{
    WireCollectionIndex, WireFilter, WireFilterBound, WireFilterKind, WireFilterTarget,
    WirePartitions, WireQueryBody, WireQueryRecord, WireRecordKey, WireStatement,
};
use aerospike_php_ipc::{BinSelector, WirePolicy};
use futures::StreamExt;

use crate::convert;
use crate::ops::{self, OpError};
use crate::policy::{Capabilities, Defaults, PolicyError};

/// Records one page may hold when the request names no size of its own.
///
/// A thousand records is a frame of a few hundred kilobytes for ordinary
/// records — well under the contract's 8 MiB ceiling — and amortises the round
/// trip to about a microsecond per record, which is where the per-record cost
/// stops being the transport's fault.
pub const DEFAULT_PAGE_SIZE: u32 = 1_000;

/// Largest page a request may ask for.
///
/// A page is materialised in the daemon before it is sent, so this is a memory
/// bound, not a preference. A caller wanting more records wants more pages.
pub const MAX_PAGE_SIZE: u32 = 100_000;

/// How long a cursor may sit unused before the daemon reclaims it.
pub const DEFAULT_CURSOR_IDLE: Duration = Duration::from_secs(60);

/// How many cursors may be open at once, across every worker.
pub const DEFAULT_MAX_CURSORS: usize = 1_024;

/// Why a page could not be produced.
#[derive(Debug)]
pub enum PageError {
    /// The request cannot be served as written: an impossible filter, a
    /// statement naming nothing, a policy field that does not apply.
    Invalid(String),
    /// The cluster failed. Classified by [`crate::failure`] like any other
    /// command error, so a timeout on a page reads the same as a timeout on a
    /// read.
    Cluster(aerospike_core::Error),
}

impl From<PolicyError> for PageError {
    fn from(inner: PolicyError) -> PageError {
        PageError::Invalid(inner.message().to_owned())
    }
}

impl From<OpError> for PageError {
    fn from(inner: OpError) -> PageError {
        PageError::Invalid(inner.to_string())
    }
}

/// One traversal in progress: where it has got to, and what to repeat.
///
/// The request is kept in its *wire* form rather than as a built `Statement`,
/// because a `Statement` holds operations that are not `Clone` and rebuilding
/// one costs nothing next to a server round trip. Keeping the wire form also
/// means a cursor is exactly what the request said, with nothing resolved early.
#[derive(Debug)]
pub struct Cursor {
    instance: String,
    policy: WirePolicy,
    statement: WireStatement,
    page_size: u32,
    max_records: u64,
    records_per_second: u32,
    include_bin_data: bool,
    partitions: PartitionFilter,
    delivered: u64,
    last_used: Instant,
}

impl Cursor {
    /// Start a traversal from a request.
    ///
    /// `default_page_size` applies when the request names none; whatever it
    /// names is clamped to [`MAX_PAGE_SIZE`], because the page is held in memory
    /// here before it is sent.
    #[must_use]
    pub fn new(request: WireQueryBody, default_page_size: u32) -> Cursor {
        let page_size = if request.page_size == 0 {
            default_page_size
        } else {
            request.page_size
        }
        .clamp(1, MAX_PAGE_SIZE);

        Cursor {
            partitions: partitions_of(&request.partitions, &request.statement),
            instance: request.instance,
            policy: request.policy,
            statement: request.statement,
            page_size,
            max_records: request.max_records,
            records_per_second: request.records_per_second,
            include_bin_data: request.include_bin_data,
            delivered: 0,
            last_used: Instant::now(),
        }
    }

    /// Which instance this traversal runs against.
    #[must_use]
    pub fn instance(&self) -> &str {
        &self.instance
    }

    /// Whether the traversal has finished, so its cursor must not be handed
    /// back to the caller.
    ///
    /// Two ways to be done, and both matter: the partition filter says every
    /// partition was read, or the caller's own `max_records` ceiling has been
    /// reached. Without the second, a traversal capped at 10 records would
    /// answer a cursor forever, because the partitions were never finished.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.partitions.done() || self.reached_ceiling()
    }

    /// How many records have been handed over so far.
    #[must_use]
    pub const fn delivered(&self) -> u64 {
        self.delivered
    }

    fn reached_ceiling(&self) -> bool {
        self.max_records != 0 && self.delivered >= self.max_records
    }

    /// How many records the next page may hold: the page size, or what is left
    /// of the ceiling if that is smaller.
    fn next_page_limit(&self) -> u64 {
        let page = u64::from(self.page_size);
        if self.max_records == 0 {
            return page;
        }
        page.min(self.max_records.saturating_sub(self.delivered))
    }
}

/// Read the next page of `cursor` from `client`.
///
/// Advances the cursor in place: on success its partition filter has moved on
/// and its delivered count has grown, so the caller only has to decide whether
/// [`Cursor::is_done`].
///
/// # Errors
/// [`PageError::Invalid`] for a request the daemon refuses to send;
/// [`PageError::Cluster`] for a failure of the query itself. On either the
/// caller must drop the cursor: a half-read page cannot be resumed, and
/// pretending otherwise would lose records.
pub async fn page(
    client: &Client,
    defaults: &Defaults,
    cursor: &mut Cursor,
) -> Result<Vec<WireQueryRecord>, PageError> {
    let caps = Capabilities::of(client);
    let mut policy = defaults.query(&cursor.policy, &caps)?;
    policy.max_records = cursor.next_page_limit();
    policy.records_per_second = cursor.records_per_second;
    policy.include_bin_data = cursor.include_bin_data;

    let statement = to_statement(&cursor.statement, &caps)?;

    // The client takes the filter by value and hands a moved-on one back inside
    // the recordset, so the cursor's own is swapped out for the duration. The
    // placeholder is never used: either the real one comes back below, or the
    // caller drops this cursor because the page failed.
    let partitions = std::mem::replace(&mut cursor.partitions, PartitionFilter::all());

    let recordset = client
        .query(&policy, partitions, statement)
        .await
        .map_err(PageError::Cluster)?;

    let mut records = Vec::new();
    let mut stream = recordset.clone().into_stream();
    while let Some(result) = stream.next().await {
        // One bad record fails the page. There is no way to report "records 1..7
        // then a failure" through one reply, and answering with the good ones
        // would be a page that silently lost the rest.
        records.push(to_wire_record(&result.map_err(PageError::Cluster)?));
    }
    drop(stream);
    recordset.close();

    let Some(moved_on) = recordset.partition_filter().await else {
        return Err(PageError::Invalid(
            "the client did not hand back a cursor for this page, so the traversal cannot be \
             resumed without risking records being read twice or not at all"
                .to_owned(),
        ));
    };
    cursor.partitions = moved_on;
    cursor.delivered = cursor.delivered.saturating_add(records.len() as u64);
    cursor.last_used = Instant::now();
    Ok(records)
}

/// The open traversals, keyed by the number PHP holds.
///
/// A plain `std::sync::Mutex`: every critical section is a map operation with no
/// `.await` in it, because a cursor is *checked out* for the duration of a page
/// rather than borrowed across one. That is also what makes a cursor
/// single-use-at-a-time — a second `QUERY_NEXT` for a cursor already being read
/// finds it absent, which is the honest answer.
#[derive(Debug)]
pub struct Cursors {
    open: Mutex<HashMap<u64, Cursor>>,
    next_id: AtomicU64,
    idle_timeout: Duration,
    max_open: usize,
}

impl Cursors {
    /// A registry expiring cursors after `idle_timeout` and holding at most
    /// `max_open`.
    #[must_use]
    pub fn new(idle_timeout: Duration, max_open: usize) -> Cursors {
        Cursors {
            open: Mutex::new(HashMap::new()),
            // Ids start at one so that zero is never a cursor: a body whose
            // field was left unset must not name a real traversal.
            next_id: AtomicU64::new(1),
            idle_timeout,
            max_open: max_open.max(1),
        }
    }

    /// Register `cursor` and return the number that names it.
    ///
    /// Ids are never reused, so a cursor that expired can never be mistaken for
    /// a live one — the caller gets [`StatusCode::CURSOR_EXPIRED`] instead of
    /// somebody else's records.
    ///
    /// [`StatusCode::CURSOR_EXPIRED`]: aerospike_php_ipc::StatusCode::CURSOR_EXPIRED
    ///
    /// # Errors
    /// The number of cursors already open, when that is the configured maximum.
    /// An open cursor is a PHP worker's unfinished scan; too many of them means
    /// workers are abandoning traversals faster than they expire, and refusing
    /// the next one is better than growing without bound.
    pub fn open(&self, cursor: Cursor) -> Result<u64, usize> {
        let mut open = self.lock();
        if open.len() >= self.max_open {
            return Err(open.len());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        open.insert(id, cursor);
        Ok(id)
    }

    /// Take a cursor out for the duration of one page.
    ///
    /// Removed rather than borrowed, so the lock is never held across the
    /// query. The caller must [`check_in`](Self::check_in) it again if the
    /// traversal continues.
    #[must_use]
    pub fn checkout(&self, id: u64) -> Option<Cursor> {
        self.lock().remove(&id)
    }

    /// Put a checked-out cursor back under the same id.
    pub fn check_in(&self, id: u64, mut cursor: Cursor) {
        cursor.last_used = Instant::now();
        self.lock().insert(id, cursor);
    }

    /// Forget a cursor, returning whether it was there.
    ///
    /// `false` is not an error for a close: a traversal that finished, or one
    /// whose cursor expired, is closed already.
    pub fn close(&self, id: u64) -> bool {
        self.lock().remove(&id).is_some()
    }

    /// Drop cursors idle for longer than the configured timeout, returning how
    /// many went.
    ///
    /// This is what makes a killed PHP worker cost nothing permanently. It runs
    /// on a timer rather than on access, because the abandoned cursors are
    /// exactly the ones nobody will touch again.
    pub fn expire(&self) -> usize {
        let now = Instant::now();
        let mut open = self.lock();
        let before = open.len();
        open.retain(|_, cursor| now.duration_since(cursor.last_used) < self.idle_timeout);
        before - open.len()
    }

    /// How many traversals are open.
    #[must_use]
    pub fn open_count(&self) -> usize {
        self.lock().len()
    }

    /// How long a cursor may idle.
    #[must_use]
    pub const fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// The most cursors that may be open at once.
    #[must_use]
    pub const fn max_open(&self) -> usize {
        self.max_open
    }

    /// A poisoned lock cannot happen — nothing here panics while holding it —
    /// and recovering is strictly better than propagating a panic into every
    /// later request.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Cursor>> {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The client's statement for a wire one.
///
/// # Errors
/// [`OpError`] from the filter's context or its expression — a filter on a
/// nested collection, or on an expression-based index, can name things this
/// cluster cannot do.
pub fn to_statement(
    statement: &WireStatement,
    caps: &Capabilities,
) -> Result<Statement, OpError> {
    let bins = match &statement.bins {
        BinSelector::All => Bins::All,
        BinSelector::None => Bins::None,
        BinSelector::Only(names) => Bins::Some(names.clone()),
    };
    let mut built = Statement::new(&statement.namespace, &statement.set, bins);
    if let Some(filter) = &statement.filter {
        built.add_filter(to_filter(filter, caps)?);
    }
    Ok(built)
}

/// The client's filter for a wire one.
///
/// The contract keeps the three independent choices apart — what is compared, how
/// the index is named, what part of a collection it covers — so this is where
/// they are recombined into the client's fourteen constructors. Each arm names
/// exactly one of them, which is what makes a missing combination a compile
/// error rather than a filter that quietly does something else.
///
/// # Errors
/// [`OpError`] from the CDT context or the expression the filter carries.
pub fn to_filter(filter: &WireFilter, caps: &Capabilities) -> Result<Filter, OpError> {
    let cit = collection_index(filter.collection);
    let scalar = matches!(filter.collection, WireCollectionIndex::Default);

    let mut built = match (&filter.target, &filter.kind) {
        // ----- equality, and its collection form: "contains" -----
        (WireFilterTarget::Bin(bin), WireFilterKind::Equal(bound)) => {
            let value = bound_value(bound);
            if scalar {
                Filter::equal(bin, value)
            } else {
                Filter::contains(bin, value, cit)
            }
        }
        (WireFilterTarget::Index(index), WireFilterKind::Equal(bound)) => {
            let value = bound_value(bound);
            if scalar {
                Filter::equal_by_index(index, value)
            } else {
                Filter::contains_by_index(index, value, cit)
            }
        }

        // ----- ranges, and their collection form -----
        (WireFilterTarget::Bin(bin), WireFilterKind::Range { begin, end }) => {
            if scalar {
                Filter::range(bin, *begin, *end)
            } else {
                Filter::contains_range(bin, *begin, *end, cit)
            }
        }
        (WireFilterTarget::Index(index), WireFilterKind::Range { begin, end }) => {
            if scalar {
                Filter::range_by_index(index, *begin, *end)
            } else {
                Filter::contains_range_by_index(index, *begin, *end, cit)
            }
        }

        // ----- geospatial. Each has a `_cit` form for a collection index -----
        (WireFilterTarget::Bin(bin), WireFilterKind::GeoWithinRegion(region)) => {
            if scalar {
                Filter::geo_within_region(bin, region)
            } else {
                Filter::geo_within_region_cit(bin, region, cit)
            }
        }
        (WireFilterTarget::Index(index), WireFilterKind::GeoWithinRegion(region)) => {
            if scalar {
                Filter::geo_within_region_by_index(index, region)
            } else {
                Filter::geo_within_region_by_index_cit(index, region, cit)
            }
        }
        (
            WireFilterTarget::Bin(bin),
            WireFilterKind::GeoWithinRadius {
                longitude,
                latitude,
                radius,
            },
        ) => {
            if scalar {
                Filter::geo_within_radius(bin, *longitude, *latitude, *radius)
            } else {
                Filter::geo_within_radius_cit(bin, *longitude, *latitude, *radius, cit)
            }
        }
        (
            WireFilterTarget::Index(index),
            WireFilterKind::GeoWithinRadius {
                longitude,
                latitude,
                radius,
            },
        ) => {
            if scalar {
                Filter::geo_within_radius_by_index(index, *longitude, *latitude, *radius)
            } else {
                Filter::geo_within_radius_by_index_cit(index, *longitude, *latitude, *radius, cit)
            }
        }
        (WireFilterTarget::Bin(bin), WireFilterKind::GeoContains(point)) => {
            if scalar {
                Filter::geo_contains(bin, point)
            } else {
                Filter::geo_contains_cit(bin, point, cit)
            }
        }
        (WireFilterTarget::Index(index), WireFilterKind::GeoContains(point)) => {
            if scalar {
                Filter::geo_contains_by_index(index, point)
            } else {
                Filter::geo_contains_by_index_cit(index, point, cit)
            }
        }
    };

    if !filter.context.is_empty() {
        built = built.context(ops::to_contexts(&filter.context, caps)?);
    }
    if let Some(expression) = &filter.expression {
        built = built.expression(ops::to_expression(expression, caps)?);
    }
    Ok(built)
}

/// The client's partition filter for a wire selection.
///
/// A `PartitionFilter` cannot fail to be built — an out-of-range partition id is
/// the server's to reject, and inventing a client-side range check here would
/// mean hard-coding a partition count that the server owns.
fn partitions_of(partitions: &WirePartitions, statement: &WireStatement) -> PartitionFilter {
    match partitions {
        WirePartitions::All => PartitionFilter::all(),
        WirePartitions::Id(id) => PartitionFilter::by_id(*id as usize),
        WirePartitions::Range { begin, count } => {
            PartitionFilter::by_range(*begin as usize, *count as usize)
        }
        // The key's namespace and set come from the statement: a scan resuming
        // after a key is a scan of that key's set, so naming them twice would
        // only create a way for them to disagree.
        WirePartitions::AfterKey(key) => {
            match convert::to_key(&statement.namespace, &statement.set, key) {
                Ok(key) => PartitionFilter::by_key(&key),
                // A key that will not build cannot name a partition; covering
                // the whole ring instead would silently scan everything, so the
                // request is left to fail on the key it named. `to_key` only
                // fails for a value that is not a legal user key, which the
                // extension has already refused.
                Err(_) => PartitionFilter::all(),
            }
        }
    }
}

/// One record as the contract carries it.
///
/// The key is the interesting part. A scanned record may or may not carry the
/// *user* key — that depends on `send_key` at write time — but it always carries a
/// digest, which is the only identity there is. So an absent key is not an error
/// and not an empty record: it is a digest with no user key, and a caller resuming
/// a traversal needs exactly that. Losing a record because its key was surprising
/// would be the worst of the available answers.
fn to_wire_record(record: &Record) -> WireQueryRecord {
    let (bins, generation, time_to_live) =
        (&record.bins, record.generation, record.time_to_live());
    let key = record.key.as_ref().map_or_else(
        || WireRecordKey {
            namespace: String::new(),
            set: String::new(),
            digest: [0u8; 20],
            user_key: None,
        },
        |key| WireRecordKey {
            namespace: key.namespace.clone(),
            set: key.set_name.clone(),
            digest: key.digest,
            user_key: key.user_key.as_ref().and_then(user_key_of),
        },
    );

    let ttl = time_to_live.map(clamp_ttl);

    WireQueryRecord {
        key,
        bins: bins
            .iter()
            .map(|(name, value)| (name.clone(), convert::from_value(value)))
            .collect(),
        generation,
        ttl,
    }
}

/// A remaining lifetime as the contract's seconds, never the sentinel.
///
/// [`aerospike_php_ipc::TTL_NEVER_EXPIRES`] is `u32::MAX` and means "no expiry", so
/// a *finite* lifetime must stay below it however large the server's answer was —
/// otherwise a record that expires in 136 years reads as one that never does.
///
/// A real record cannot currently reach that: its expiration is a `u32` count of
/// seconds from 2010, so the largest lifetime it can express is about 3.8e9 seconds,
/// short of `u32::MAX`. The clamp is here for the case where that stops being true,
/// which is exactly why it is a named function with a test rather than an inline
/// `min` no one can exercise.
fn clamp_ttl(remaining: std::time::Duration) -> u32 {
    u32::try_from(remaining.as_secs())
        .unwrap_or(u32::MAX)
        .min(aerospike_php_ipc::TTL_NEVER_EXPIRES - 1)
}

/// The contract's key shape for a stored user key.
///
/// Only the three types a key may be. Anything else cannot have been a key in
/// the first place, so it is reported as no key rather than as a value of some
/// other kind masquerading as one.
fn user_key_of(value: &Value) -> Option<aerospike_php_ipc::WireKey> {
    match value {
        Value::Int(number) => Some(aerospike_php_ipc::WireKey::Int(*number)),
        Value::String(text) => Some(aerospike_php_ipc::WireKey::Str(text.clone())),
        Value::Blob(bytes) => Some(aerospike_php_ipc::WireKey::Blob(bytes.clone())),
        _ => None,
    }
}

/// The client's collection index type.
const fn collection_index(collection: WireCollectionIndex) -> CollectionIndexType {
    match collection {
        WireCollectionIndex::Default => CollectionIndexType::Default,
        WireCollectionIndex::List => CollectionIndexType::List,
        WireCollectionIndex::MapKeys => CollectionIndexType::MapKeys,
        WireCollectionIndex::MapValues => CollectionIndexType::MapValues,
    }
}

/// A filter bound as the client's value.
///
/// `Value` implements both of the client's filter-value traits, so one
/// conversion serves equality and range filters alike — and the particle type
/// the filter sends is derived from the value, which is why the bound's type has
/// to survive rather than being widened to a string.
fn bound_value(bound: &WireFilterBound) -> Value {
    match bound {
        WireFilterBound::Int(number) => Value::Int(*number),
        WireFilterBound::Str(text) => Value::String(text.clone()),
        WireFilterBound::Blob(bytes) => Value::Blob(bytes.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::op::WireCtx;
    use aerospike_php_ipc::WireKey;

    fn body(page_size: u32, max_records: u64) -> WireQueryBody {
        WireQueryBody {
            instance: "default".into(),
            policy: WirePolicy::default(),
            statement: WireStatement {
                namespace: "test".into(),
                set: "users".into(),
                bins: BinSelector::All,
                filter: None,
            },
            partitions: WirePartitions::All,
            page_size,
            max_records,
            records_per_second: 0,
            include_bin_data: true,
        }
    }

    #[test]
    fn a_page_size_of_zero_takes_the_default_and_a_huge_one_is_capped() {
        assert_eq!(Cursor::new(body(0, 0), DEFAULT_PAGE_SIZE).page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(Cursor::new(body(50, 0), DEFAULT_PAGE_SIZE).page_size, 50);
        // The page is held in the daemon before it is sent, so this is a memory
        // bound rather than a preference.
        assert_eq!(
            Cursor::new(body(u32::MAX, 0), DEFAULT_PAGE_SIZE).page_size,
            MAX_PAGE_SIZE
        );
    }

    #[test]
    fn a_page_never_overshoots_the_callers_ceiling() {
        let mut cursor = Cursor::new(body(100, 250), DEFAULT_PAGE_SIZE);
        assert_eq!(cursor.next_page_limit(), 100);

        cursor.delivered = 200;
        // 50 left of the ceiling, so the page must be 50 and not the page size.
        assert_eq!(cursor.next_page_limit(), 50);

        cursor.delivered = 250;
        assert_eq!(cursor.next_page_limit(), 0);
        assert!(cursor.is_done(), "the ceiling was reached");
    }

    /// The ceiling is the *other* way to finish. A capped traversal whose
    /// partitions were never exhausted must still end, or it would hand out a
    /// cursor for ever.
    #[test]
    fn a_ceiling_finishes_a_traversal_the_partitions_did_not() {
        let mut cursor = Cursor::new(body(10, 10), DEFAULT_PAGE_SIZE);
        assert!(!cursor.is_done());
        assert!(!cursor.partitions.done(), "no page has run yet");

        cursor.delivered = 10;
        assert!(cursor.is_done());

        // Without a ceiling, only the partitions can end it.
        let uncapped = Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE);
        assert!(!uncapped.is_done());
        assert_eq!(uncapped.next_page_limit(), 10);
    }

    #[test]
    fn a_registry_hands_out_ids_that_are_never_reused() {
        let cursors = Cursors::new(Duration::from_secs(60), 8);
        let first = cursors.open(Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE)).unwrap();
        let second = cursors.open(Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE)).unwrap();
        assert_ne!(first, second);
        assert_ne!(first, 0, "zero must never name a cursor");
        assert_eq!(cursors.open_count(), 2);

        // Closing frees the slot but not the number: a later cursor must not
        // inherit an id a client might still be holding.
        assert!(cursors.close(first));
        assert!(!cursors.close(first), "closing twice is not an error");
        let third = cursors.open(Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE)).unwrap();
        assert_ne!(third, first);
        assert_ne!(third, second);
    }

    #[test]
    fn a_checked_out_cursor_is_not_findable_until_it_is_checked_back_in() {
        let cursors = Cursors::new(Duration::from_secs(60), 8);
        let id = cursors.open(Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE)).unwrap();

        let cursor = cursors.checkout(id).expect("just opened");
        // A second reader of the same traversal must be told it is not there,
        // rather than both of them reading it and interleaving pages.
        assert!(cursors.checkout(id).is_none());
        assert_eq!(cursors.open_count(), 0);

        cursors.check_in(id, cursor);
        assert_eq!(cursors.open_count(), 1);
        assert!(cursors.checkout(id).is_some());
    }

    #[test]
    fn too_many_open_cursors_is_refused_with_the_count() {
        let cursors = Cursors::new(Duration::from_secs(60), 2);
        cursors.open(Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE)).unwrap();
        cursors.open(Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE)).unwrap();
        assert_eq!(
            cursors.open(Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE)).unwrap_err(),
            2
        );

        // A registry can never be built with room for nothing at all.
        assert_eq!(Cursors::new(Duration::from_secs(1), 0).max_open(), 1);
    }

    #[test]
    fn an_idle_cursor_is_expired_and_a_fresh_one_is_not() {
        let cursors = Cursors::new(Duration::from_millis(0), 8);
        cursors.open(Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE)).unwrap();
        // A zero timeout expires everything, which is the boundary worth
        // pinning: `retain` must use `<`, not `<=`, or nothing would ever go.
        assert_eq!(cursors.expire(), 1);
        assert_eq!(cursors.open_count(), 0);

        let patient = Cursors::new(Duration::from_secs(3_600), 8);
        patient.open(Cursor::new(body(10, 0), DEFAULT_PAGE_SIZE)).unwrap();
        assert_eq!(patient.expire(), 0);
        assert_eq!(patient.open_count(), 1);
    }

    #[test]
    fn a_statement_carries_its_bin_selection() {
        let caps = Capabilities::all();
        for (selector, expected) in [
            (BinSelector::All, Bins::All),
            (BinSelector::None, Bins::None),
            (
                BinSelector::Only(vec!["a".into()]),
                Bins::Some(vec!["a".into()]),
            ),
        ] {
            let wire = WireStatement {
                namespace: "test".into(),
                set: "users".into(),
                bins: selector,
                filter: None,
            };
            let statement = to_statement(&wire, &caps).unwrap();
            assert_eq!(statement.namespace, "test");
            assert_eq!(statement.set_name, "users");
            assert_eq!(format!("{:?}", statement.bins), format!("{expected:?}"));
            assert!(statement.filters.is_none(), "a statement with no filter is a scan");
        }
    }

    /// Every combination of the three independent choices has to build, because
    /// the contract lets a caller ask for any of them.
    #[test]
    fn every_filter_combination_builds() {
        let caps = Capabilities::all();
        for kind in [
            WireFilterKind::Equal(WireFilterBound::Int(1)),
            WireFilterKind::Equal(WireFilterBound::Str("x".into())),
            WireFilterKind::Equal(WireFilterBound::Blob(vec![1, 2])),
            WireFilterKind::Range { begin: 0, end: 10 },
            WireFilterKind::GeoWithinRegion("{\"type\":\"Polygon\"}".into()),
            WireFilterKind::GeoWithinRadius {
                longitude: -122.4,
                latitude: 37.8,
                radius: 100.0,
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
                        context: Vec::new(),
                        expression: None,
                    };
                    to_filter(&filter, &caps)
                        .unwrap_or_else(|e| panic!("{kind:?}/{target:?}/{collection:?}: {e}"));
                }
            }
        }
    }

    #[test]
    fn a_filter_can_carry_a_context_and_an_expression() {
        let filter = WireFilter {
            target: WireFilterTarget::Bin("b".into()),
            kind: WireFilterKind::Equal(WireFilterBound::Int(1)),
            collection: WireCollectionIndex::MapValues,
            context: vec![WireCtx::MapKey {
                key: aerospike_php_ipc::WireValue::Str("inner".into()),
            }],
            expression: Some(aerospike_php_ipc::op::WireExpression::Ael("$.b".into())),
        };
        assert!(to_filter(&filter, &Capabilities::all()).is_ok());

        // The same filter against a cluster too old to compile the expression
        // must be refused by name, not sent and rejected obscurely.
        let error = to_filter(
            &filter,
            &Capabilities {
                ael: crate::policy::AelSupport::Unsupported {
                    node: "BB9".into(),
                    version: "7.1.0.0".into(),
                },
                ..Capabilities::all()
            },
        )
        .expect_err("an old cluster must be refused");
        assert!(error.to_string().contains("BB9"), "{error}");
    }

    #[test]
    fn a_partition_selection_maps_onto_the_clients_filter() {
        let statement = WireStatement {
            namespace: "test".into(),
            set: "users".into(),
            bins: BinSelector::All,
            filter: None,
        };

        let all = partitions_of(&WirePartitions::All, &statement);
        assert_eq!(all.begin, 0);
        assert_eq!(all.count, 4_096);

        let one = partitions_of(&WirePartitions::Id(7), &statement);
        assert_eq!((one.begin, one.count), (7, 1));

        let range = partitions_of(
            &WirePartitions::Range {
                begin: 1_000,
                count: 24,
            },
            &statement,
        );
        assert_eq!((range.begin, range.count), (1_000, 24));

        // A key selection has to produce a digest, since that is the whole point
        // of resuming after one.
        let after = partitions_of(&WirePartitions::AfterKey(WireKey::Str("alice".into())), &statement);
        assert_eq!(after.count, 1);
        assert!(after.digest.is_some());
    }

    /// A record with this key and one bin, as a scan hands one back.
    ///
    /// `expiration` is the server's own encoding — seconds from 2010, `0` for
    /// never — which is what `Record::new` takes.
    fn scanned(key: Option<aerospike_core::Key>, expiration: u32) -> Record {
        Record::new(
            key,
            aerospike_core::IndexMap::from([("name".to_string(), Value::from("Alice"))]),
            None,
            3,
            expiration,
        )
    }

    /// A scanned record's identity: the digest is always there, the user key only
    /// if `send_key` was set on the write. A traversal resuming after a record
    /// needs the digest, so losing it would break paging silently.
    #[test]
    fn a_scanned_record_keeps_its_digest_and_its_stored_key() {
        let key = aerospike_core::Key::new("test", "users", Value::from("alice")).unwrap();
        let digest = key.digest;

        let wire = to_wire_record(&scanned(Some(key.clone()), 0));
        assert_eq!(wire.key.namespace, "test");
        assert_eq!(wire.key.set, "users");
        assert_eq!(wire.key.digest, digest);
        assert_eq!(wire.key.user_key, Some(WireKey::Str("alice".into())));
        assert_eq!(wire.generation, 3);
        assert_eq!(wire.bins.len(), 1);
        assert_eq!(wire.ttl, None, "no expiration means no TTL");

        // A record whose key was not stored keeps its digest, which is the only
        // identity there is; inventing a user key from it is impossible.
        let anonymous = aerospike_core::Key {
            namespace: "test".into(),
            set_name: String::new(),
            user_key: None,
            digest: [9u8; 20],
        };
        let wire = to_wire_record(&scanned(Some(anonymous), 0));
        assert_eq!(wire.key.digest, [9u8; 20]);
        assert_eq!(wire.key.user_key, None);

        // No key at all — a shape the client can produce — is a zero digest and no
        // user key rather than a panic.
        let wire = to_wire_record(&scanned(None, 0));
        assert_eq!(wire.key.digest, [0u8; 20]);
        assert!(wire.key.namespace.is_empty());

        // A record that does expire reports a TTL, and not the sentinel.
        let expires_in_an_hour = u32::try_from(
            std::time::SystemTime::now()
                .duration_since(*aerospike_core::CITRUSLEAF_EPOCH)
                .expect("now is after 2010")
                .as_secs()
                + 3600,
        )
        .expect("2010 plus a u32 of seconds is well past now");
        let wire = to_wire_record(&scanned(Some(key), expires_in_an_hour));
        let ttl = wire.ttl.expect("a record with an expiration has a TTL");
        assert!(
            (3500..=3600).contains(&ttl),
            "about an hour, allowing for the clock: {ttl}"
        );
        assert_ne!(ttl, aerospike_php_ipc::TTL_NEVER_EXPIRES);
    }

    /// A finite TTL must never reach the never-expires sentinel, however large the
    /// server's answer was — a record expiring in 136 years is not one that never
    /// expires. Tested on [`clamp_ttl`] directly because no real record can produce
    /// a lifetime that large; see that function's note.
    #[test]
    fn a_finite_ttl_never_reads_as_never_expires() {
        use std::time::Duration;

        assert_eq!(clamp_ttl(Duration::from_secs(3600)), 3600);
        assert_eq!(
            clamp_ttl(Duration::from_secs(u64::from(u32::MAX))),
            aerospike_php_ipc::TTL_NEVER_EXPIRES - 1
        );
        assert_eq!(
            clamp_ttl(Duration::from_secs(u64::MAX)),
            aerospike_php_ipc::TTL_NEVER_EXPIRES - 1,
            "a lifetime that does not fit in a u32 clamps rather than wrapping"
        );
    }
}
