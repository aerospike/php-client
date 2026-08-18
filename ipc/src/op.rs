// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The operations an `operate()` call carries.
//!
//! # Why this names operations instead of encoding them
//!
//! A collection-data-type operation goes to the server as a sub-opcode and a
//! list of positional arguments, and the contract could simply carry that: an
//! op byte and a vector of values. It deliberately does not. Doing so would put
//! the opcode table and the argument order — the two things easiest to get
//! silently wrong — on the PHP side of the boundary, where nothing can check
//! them, and a mistake would reach the server as a well-formed request that
//! means something else.
//!
//! So each variant here names an operation the way `aerospike-core` names it,
//! and the daemon builds the real one by calling the client's own constructor.
//! The cost is a variant per operation; the benefit is that the argument order
//! lives in exactly one place, and adding an operation is a compiler-checked
//! change on both sides.
//!
//! # Ranges collapse
//!
//! Where the client has a pair of constructors — `get_range` and
//! `get_range_from`, `remove_by_index_range` and `..._range_count` — this has
//! one variant with `count: Option<i64>`, because that is what the pair means:
//! `None` is "to the end of the list". Two spellings of one operation is a
//! distinction worth removing at a boundary.
//!
//! # Adding operations is free
//!
//! The daemon and the extension are matched by version (see [`crate::VERSION`]),
//! so this enum may grow, shrink or be reordered in any release. No variant has
//! to be kept for a peer that might still send it.

use serde::{Deserialize, Serialize};

use crate::WireValue;

/// One operation in an `operate()` call.
///
/// `bin` is a bin name for everything except the whole-record operations
/// ([`WireOp::Get`], [`WireOp::GetHeader`], [`WireOp::Touch`],
/// [`WireOp::Delete`]), which name no bin at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireOp {
    // ===== scalar =====
    /// Read every bin of the record.
    Get,
    /// Read the record's metadata and no bins.
    GetHeader,
    /// Read one bin.
    GetBin {
        /// Bin to read.
        bin: String,
    },
    /// Write one bin. [`WireValue::Nil`] deletes it.
    Put {
        /// Bin to write.
        bin: String,
        /// Value to write.
        value: WireValue,
    },
    /// Append to a string or blob bin.
    Append {
        /// Bin to append to.
        bin: String,
        /// Value to append.
        value: WireValue,
    },
    /// Prepend to a string or blob bin.
    Prepend {
        /// Bin to prepend to.
        bin: String,
        /// Value to prepend.
        value: WireValue,
    },
    /// Add a numeric delta to a bin.
    Add {
        /// Bin to add to.
        bin: String,
        /// Delta; an int for an integer bin, a float for a double bin.
        value: WireValue,
    },
    /// Reset the record's time-to-live.
    Touch,
    /// Delete the record.
    Delete,

    // ===== list =====
    /// An operation on a list bin, or on a list nested inside one.
    List {
        /// Bin holding the list.
        bin: String,
        /// Path to a nested list; empty for the bin itself.
        ctx: Vec<WireCtx>,
        /// What to do to it.
        op: WireListOp,
    },

    // ===== map =====
    /// An operation on a map bin, or on a map nested inside one.
    Map {
        /// Bin holding the map.
        bin: String,
        /// Path to a nested map; empty for the bin itself.
        ctx: Vec<WireCtx>,
        /// What to do to it.
        op: WireMapOp,
    },

    // ===== bitwise =====
    /// An operation on the bits of a blob bin.
    Bit {
        /// Bin holding the blob.
        bin: String,
        /// Path to a nested blob; empty for the bin itself.
        ctx: Vec<WireCtx>,
        /// What to do to it.
        op: WireBitOp,
    },

    // ===== HyperLogLog =====
    /// An operation on a HyperLogLog bin.
    Hll {
        /// Bin holding the sketch.
        bin: String,
        /// Path to a nested sketch; empty for the bin itself.
        ctx: Vec<WireCtx>,
        /// What to do to it.
        op: WireHllOp,
    },

    // ===== expressions =====
    /// Evaluate an expression against the record, and read or write its result.
    ///
    /// No context path: an expression names what it reads for itself, so the
    /// client gives these operations no path either.
    Exp {
        /// What to evaluate, and what to do with the answer.
        op: WireExpOp,
    },
}

impl WireOp {
    /// Whether this operation writes.
    ///
    /// The daemon needs it to decide whether an `operate()` call may run under
    /// a read policy, and the extension uses it to say so before sending.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        match self {
            WireOp::Get | WireOp::GetHeader | WireOp::GetBin { .. } => false,
            WireOp::Put { .. }
            | WireOp::Append { .. }
            | WireOp::Prepend { .. }
            | WireOp::Add { .. }
            | WireOp::Touch
            | WireOp::Delete => true,
            WireOp::List { op, .. } => op.is_write(),
            WireOp::Map { op, .. } => op.is_write(),
            WireOp::Bit { op, .. } => op.is_write(),
            WireOp::Hll { op, .. } => op.is_write(),
            WireOp::Exp { op } => op.is_write(),
        }
    }

    /// The bin this operation names, if it names one.
    #[must_use]
    pub fn bin(&self) -> Option<&str> {
        match self {
            WireOp::Get | WireOp::GetHeader | WireOp::Touch | WireOp::Delete => None,
            // An expression read names the bin its result appears under, and a
            // write names the bin it lands in; both are the operation's bin.
            WireOp::Exp { op } => Some(op.bin()),
            WireOp::GetBin { bin }
            | WireOp::Put { bin, .. }
            | WireOp::Append { bin, .. }
            | WireOp::Prepend { bin, .. }
            | WireOp::Add { bin, .. }
            | WireOp::List { bin, .. }
            | WireOp::Map { bin, .. }
            | WireOp::Bit { bin, .. }
            | WireOp::Hll { bin, .. } => Some(bin),
        }
    }
}

/// A step in the path to a nested collection.
///
/// Mirrors the client's `ctx_*` constructors. Only the list and map steps are
/// here; the expression-filtered contexts belong with the expression operations
/// and arrive with them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireCtx {
    /// The list element at this index.
    ListIndex {
        /// Index; negative counts from the end.
        index: i64,
    },
    /// The list element at this index, creating the list if it is missing.
    ListIndexCreate {
        /// Index; negative counts from the end.
        index: i64,
        /// Order the created list gets.
        order: WireListOrder,
        /// Whether to pad a sparse index.
        pad: bool,
    },
    /// The list element at this rank.
    ListRank {
        /// Rank; negative counts from the largest.
        rank: i64,
    },
    /// The first list element equal to this value.
    ListValue {
        /// Value to look for.
        value: WireValue,
    },
    /// The map entry at this index.
    MapIndex {
        /// Index; negative counts from the end.
        index: i64,
    },
    /// The map entry at this rank.
    MapRank {
        /// Rank; negative counts from the largest.
        rank: i64,
    },
    /// The map entry with this key.
    MapKey {
        /// Key to look for.
        key: WireValue,
    },
    /// The map entry with this key, creating the map if it is missing.
    MapKeyCreate {
        /// Key to look for.
        key: WireValue,
        /// Order the created map gets.
        order: WireMapOrder,
    },
    /// The first map entry with this value.
    MapValue {
        /// Value to look for.
        value: WireValue,
    },

    /// Every child of the current node — every element of a list, every entry of
    /// a map.
    ///
    /// The step that makes a *path expression* a path expression: the ones above
    /// select one node, and this one fans out. What follows applies to each child,
    /// so a path can reach "the price of every book" rather than one book's price.
    ///
    /// Needs server **8.1.1 or later**, which the daemon checks.
    AllChildren,

    /// Every child the filter accepts.
    ///
    /// The filter is evaluated per child, with the loop variable
    /// ([`crate::exp::WireExp::LoopVar`]) standing for the child being tested — so
    /// it can compare against the child's key, value or index.
    ///
    /// Needs server **8.1.1 or later**.
    AllChildrenWithFilter {
        /// The filter to apply to each child.
        ///
        /// Boxed because a [`WireExpression`] may hold a whole tree, and a tree may
        /// hold contexts of its own — the recursion is real, and without a box the
        /// type would have no finite size.
        filter: Box<WireExpression>,
    },
}

impl WireCtx {
    /// The expression tree inside this step's filter, if it has one.
    ///
    /// `None` for every step but
    /// [`AllChildrenWithFilter`](Self::AllChildrenWithFilter), and also for one
    /// whose filter is text or already packed — those hold no tree to walk.
    /// [`crate::exp`] uses this to count a context filter's nesting toward the
    /// expression size limits.
    #[must_use]
    pub fn filter_tree(&self) -> Option<&crate::exp::WireExp> {
        match self {
            WireCtx::AllChildrenWithFilter { filter } => match filter.as_ref() {
                WireExpression::Tree(tree) => Some(tree),
                WireExpression::Ael(_) | WireExpression::Base64(_) => None,
            },
            _ => None,
        }
    }
}

/// List storage order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireListOrder {
    /// Not ordered. The default.
    Unordered,
    /// Ordered by value.
    Ordered,
}

/// Map storage order, for [`WireCtx::MapKeyCreate`].
///
/// The map *operations* arrive with phase 3b; this exists now because a list
/// nested inside a map needs to be able to name the map it creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireMapOrder {
    /// Not ordered.
    Unordered,
    /// Ordered by key.
    KeyOrdered,
    /// Ordered by key, then value.
    KeyValueOrdered,
}

/// What a list operation returns.
///
/// `inverted` flips the selection: the items *outside* the range are the ones
/// returned, and for a remove operation the ones removed. It is a flag rather
/// than a set of doubled variants because that is what it is on the wire — a
/// bit the client ORs into the return type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireListReturn {
    /// What to return.
    pub kind: WireListReturnKind,
    /// Whether to invert the selection.
    pub inverted: bool,
}

impl WireListReturn {
    /// A plain, uninverted return type.
    #[must_use]
    pub const fn of(kind: WireListReturnKind) -> WireListReturn {
        WireListReturn {
            kind,
            inverted: false,
        }
    }
}

/// The values a list operation can return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireListReturnKind {
    /// Nothing.
    None,
    /// Index order.
    Index,
    /// Reverse index order.
    ReverseIndex,
    /// Value order.
    Rank,
    /// Reverse value order.
    ReverseRank,
    /// How many items were selected.
    Count,
    /// The values themselves.
    Values,
    /// Whether anything was selected.
    Exists,
}

/// Sort flags for [`WireListOp::Sort`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireListSort {
    /// Sort descending rather than ascending.
    pub descending: bool,
    /// Drop duplicate values.
    pub drop_duplicates: bool,
}

/// Write flags for the list operations that add items.
///
/// A set of booleans rather than a bitmask: a bitmask crossing a language
/// boundary is a number nobody can read, and the daemon has to translate to the
/// client's own flags anyway.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireListWriteFlags {
    /// Only add values not already present.
    pub add_unique: bool,
    /// Refuse an insert outside the list's current range.
    pub insert_bounded: bool,
    /// Do not fail the operation when a flag rejects an item.
    pub no_fail: bool,
    /// Apply the items that are acceptable when others are rejected.
    pub partial: bool,
}

/// Order and write flags for the list operations that create or extend a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireListPolicy {
    /// Order a created list gets.
    pub order: WireListOrder,
    /// How writes behave.
    pub flags: WireListWriteFlags,
}

impl Default for WireListPolicy {
    fn default() -> WireListPolicy {
        WireListPolicy {
            order: WireListOrder::Unordered,
            flags: WireListWriteFlags::default(),
        }
    }
}

/// An operation on a list.
///
/// One variant per operation `aerospike-core`'s `operations::lists` offers,
/// with the range pairs collapsed as described at the module level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireListOp {
    // ----- create and order -----
    /// Create an empty list.
    Create {
        /// Order it gets.
        order: WireListOrder,
        /// Pad a sparse nested index.
        pad: bool,
        /// Keep a persistent index on it (top-level bins only).
        persist_index: bool,
    },
    /// Change an existing list's order.
    SetOrder {
        /// New order.
        order: WireListOrder,
        /// Keep a persistent index on it.
        persist_index: bool,
    },

    // ----- add -----
    /// Append one value.
    Append {
        /// Order and write flags.
        policy: WireListPolicy,
        /// Value to append.
        value: WireValue,
    },
    /// Append several values.
    AppendItems {
        /// Order and write flags.
        policy: WireListPolicy,
        /// Values to append.
        values: Vec<WireValue>,
    },
    /// Insert one value at an index.
    Insert {
        /// Order and write flags.
        policy: WireListPolicy,
        /// Index to insert at.
        index: i64,
        /// Value to insert.
        value: WireValue,
    },
    /// Insert several values at an index.
    InsertItems {
        /// Order and write flags.
        policy: WireListPolicy,
        /// Index to insert at.
        index: i64,
        /// Values to insert.
        values: Vec<WireValue>,
    },

    // ----- remove and return -----
    /// Remove and return the item at an index.
    Pop {
        /// Index.
        index: i64,
    },
    /// Remove and return a range of items. `count` of `None` means "to the end".
    PopRange {
        /// First index.
        index: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
    },
    /// Remove the item at an index.
    Remove {
        /// Index.
        index: i64,
    },
    /// Remove a range of items. `count` of `None` means "to the end".
    RemoveRange {
        /// First index.
        index: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
    },
    /// Remove items equal to a value.
    RemoveByValue {
        /// Value to match.
        value: WireValue,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Remove items equal to any of these values.
    RemoveByValueList {
        /// Values to match.
        values: Vec<WireValue>,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Remove items in a value range, `begin` inclusive and `end` exclusive.
    RemoveByValueRange {
        /// Lower bound, or `None` for unbounded.
        begin: Option<WireValue>,
        /// Upper bound, or `None` for unbounded.
        end: Option<WireValue>,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Remove items by rank relative to a value.
    RemoveByValueRelativeRankRange {
        /// Value the ranks are relative to.
        value: WireValue,
        /// Starting rank.
        rank: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Remove the item at an index.
    RemoveByIndex {
        /// Index.
        index: i64,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Remove a range of items by index.
    RemoveByIndexRange {
        /// First index.
        index: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Remove the item at a rank.
    RemoveByRank {
        /// Rank.
        rank: i64,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Remove a range of items by rank.
    RemoveByRankRange {
        /// First rank.
        rank: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireListReturn,
    },

    // ----- change in place -----
    /// Replace the item at an index.
    Set {
        /// Order and write flags; `None` uses the list's own.
        policy: Option<WireListPolicy>,
        /// Index.
        index: i64,
        /// New value.
        value: WireValue,
    },
    /// Keep only a range of items, removing the rest.
    Trim {
        /// First index to keep.
        index: i64,
        /// How many to keep.
        count: i64,
    },
    /// Remove every item.
    Clear,
    /// Add a delta to the item at an index.
    Increment {
        /// Order and write flags.
        policy: WireListPolicy,
        /// Index.
        index: i64,
        /// Delta; `None` means one.
        value: Option<i64>,
    },
    /// Sort the list in place.
    Sort {
        /// How to sort.
        flags: WireListSort,
    },

    // ----- read -----
    /// How many items the list holds.
    Size,
    /// The item at an index.
    Get {
        /// Index.
        index: i64,
    },
    /// A range of items. `count` of `None` means "to the end".
    GetRange {
        /// First index.
        index: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
    },
    /// Items equal to a value.
    GetByValue {
        /// Value to match.
        value: WireValue,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Items equal to any of these values.
    GetByValueList {
        /// Values to match.
        values: Vec<WireValue>,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Items in a value range, `begin` inclusive and `end` exclusive.
    GetByValueRange {
        /// Lower bound, or `None` for unbounded.
        begin: Option<WireValue>,
        /// Upper bound, or `None` for unbounded.
        end: Option<WireValue>,
        /// What to return.
        return_type: WireListReturn,
    },
    /// Items by rank relative to a value.
    GetByValueRelativeRankRange {
        /// Value the ranks are relative to.
        value: WireValue,
        /// Starting rank.
        rank: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireListReturn,
    },
    /// The item at an index.
    GetByIndex {
        /// Index.
        index: i64,
        /// What to return.
        return_type: WireListReturn,
    },
    /// A range of items by index.
    GetByIndexRange {
        /// First index.
        index: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireListReturn,
    },
    /// The item at a rank.
    GetByRank {
        /// Rank.
        rank: i64,
        /// What to return.
        return_type: WireListReturn,
    },
    /// A range of items by rank.
    GetByRankRange {
        /// First rank.
        rank: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireListReturn,
    },
}

impl WireListOp {
    /// Whether this operation writes.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        matches!(
            self,
            WireListOp::Create { .. }
                | WireListOp::SetOrder { .. }
                | WireListOp::Append { .. }
                | WireListOp::AppendItems { .. }
                | WireListOp::Insert { .. }
                | WireListOp::InsertItems { .. }
                | WireListOp::Pop { .. }
                | WireListOp::PopRange { .. }
                | WireListOp::Remove { .. }
                | WireListOp::RemoveRange { .. }
                | WireListOp::RemoveByValue { .. }
                | WireListOp::RemoveByValueList { .. }
                | WireListOp::RemoveByValueRange { .. }
                | WireListOp::RemoveByValueRelativeRankRange { .. }
                | WireListOp::RemoveByIndex { .. }
                | WireListOp::RemoveByIndexRange { .. }
                | WireListOp::RemoveByRank { .. }
                | WireListOp::RemoveByRankRange { .. }
                | WireListOp::Set { .. }
                | WireListOp::Trim { .. }
                | WireListOp::Clear
                | WireListOp::Increment { .. }
                | WireListOp::Sort { .. }
        )
    }
}

// ===== map =====

/// What a map operation returns.
///
/// Richer than the list equivalent because a map entry has two halves: an
/// operation can hand back the keys, the values, both, or the map itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireMapReturn {
    /// What to return.
    pub kind: WireMapReturnKind,
    /// Whether to invert the selection — the entries *outside* the range.
    pub inverted: bool,
}

impl WireMapReturn {
    /// A plain, uninverted return type.
    #[must_use]
    pub const fn of(kind: WireMapReturnKind) -> WireMapReturn {
        WireMapReturn {
            kind,
            inverted: false,
        }
    }
}

/// The things a map operation can return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireMapReturnKind {
    /// Nothing.
    None,
    /// Key index order.
    Index,
    /// Reverse key index order.
    ReverseIndex,
    /// Value order.
    Rank,
    /// Reverse value order.
    ReverseRank,
    /// How many entries were selected.
    Count,
    /// The keys.
    Key,
    /// The values.
    Value,
    /// Both, as pairs.
    KeyValue,
    /// Whether anything was selected.
    Exists,
    /// The selection as an unordered map.
    UnorderedMap,
    /// The selection as an ordered map.
    OrderedMap,
}

/// What a map write does when the key is or is not already there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireMapWriteMode {
    /// Create or overwrite.
    Update,
    /// Overwrite only; fail if the key is absent.
    UpdateOnly,
    /// Create only; fail if the key is present.
    CreateOnly,
}

/// Order and write rules for the map operations that write.
///
/// The client has both a `write_mode` and a flag bitmask that *replaces* it when
/// non-zero, which is two ways to say the same thing. This carries the mode plus
/// the two flags that add something the mode cannot say — "do not fail" and
/// "apply what you can" — and the daemon combines them. One knob per idea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireMapPolicy {
    /// Order a created map gets.
    pub order: WireMapOrder,
    /// What a write does about an existing key.
    pub write_mode: WireMapWriteMode,
    /// Do not fail the operation when the mode rejects an entry.
    pub no_fail: bool,
    /// Apply the entries that are acceptable when others are rejected.
    pub partial: bool,
    /// Keep a persistent index on the map.
    pub persist_index: bool,
}

impl Default for WireMapPolicy {
    fn default() -> WireMapPolicy {
        WireMapPolicy {
            order: WireMapOrder::Unordered,
            write_mode: WireMapWriteMode::Update,
            no_fail: false,
            partial: false,
            persist_index: false,
        }
    }
}

/// An operation on a map.
///
/// One variant per operation `aerospike-core`'s `operations::maps` offers, with
/// the range pairs collapsed as described at the module level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireMapOp {
    // ----- create and order -----
    /// Create an empty map.
    Create {
        /// Order it gets.
        order: WireMapOrder,
        /// Keep a persistent index on it (top-level bins only).
        persist_index: bool,
    },
    /// Change an existing map's order.
    SetOrder {
        /// New order.
        order: WireMapOrder,
    },
    /// Replace the map's policy.
    SetPolicy {
        /// The new policy.
        policy: WireMapPolicy,
    },

    // ----- write -----
    /// Write one entry.
    Put {
        /// Order and write rules.
        policy: WireMapPolicy,
        /// Key to write.
        key: WireValue,
        /// Value to write.
        value: WireValue,
    },
    /// Write several entries.
    PutItems {
        /// Order and write rules.
        policy: WireMapPolicy,
        /// Entries to write.
        items: Vec<(WireValue, WireValue)>,
    },
    /// Add a delta to one entry's value.
    IncrementValue {
        /// Order and write rules.
        policy: WireMapPolicy,
        /// Key whose value to change.
        key: WireValue,
        /// Delta.
        delta: WireValue,
    },
    /// Subtract a delta from one entry's value.
    DecrementValue {
        /// Order and write rules.
        policy: WireMapPolicy,
        /// Key whose value to change.
        key: WireValue,
        /// Amount to subtract.
        delta: WireValue,
    },
    /// Remove every entry.
    Clear,

    // ----- remove -----
    /// Remove the entry with this key.
    RemoveByKey {
        /// Key to match.
        key: WireValue,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove the entries with any of these keys.
    RemoveByKeyList {
        /// Keys to match.
        keys: Vec<WireValue>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove entries whose keys are in a range, `begin` inclusive and `end`
    /// exclusive.
    RemoveByKeyRange {
        /// Lower bound, or `None` for unbounded.
        begin: Option<WireValue>,
        /// Upper bound, or `None` for unbounded.
        end: Option<WireValue>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove entries by key index relative to a key.
    RemoveByKeyRelativeIndexRange {
        /// Key the indexes are relative to.
        key: WireValue,
        /// Starting index.
        index: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove entries with this value.
    RemoveByValue {
        /// Value to match.
        value: WireValue,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove entries with any of these values.
    RemoveByValueList {
        /// Values to match.
        values: Vec<WireValue>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove entries whose values are in a range.
    RemoveByValueRange {
        /// Lower bound, or `None` for unbounded.
        begin: Option<WireValue>,
        /// Upper bound, or `None` for unbounded.
        end: Option<WireValue>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove entries by rank relative to a value.
    RemoveByValueRelativeRankRange {
        /// Value the ranks are relative to.
        value: WireValue,
        /// Starting rank.
        rank: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove the entry at this key index.
    RemoveByIndex {
        /// Index.
        index: i64,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove a range of entries by key index.
    RemoveByIndexRange {
        /// First index.
        index: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove the entry at this rank.
    RemoveByRank {
        /// Rank.
        rank: i64,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Remove a range of entries by rank.
    RemoveByRankRange {
        /// First rank.
        rank: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireMapReturn,
    },

    // ----- read -----
    /// How many entries the map holds.
    Size,
    /// The entry with this key.
    GetByKey {
        /// Key to match.
        key: WireValue,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// The entries with any of these keys.
    GetByKeyList {
        /// Keys to match.
        keys: Vec<WireValue>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Entries whose keys are in a range.
    GetByKeyRange {
        /// Lower bound, or `None` for unbounded.
        begin: Option<WireValue>,
        /// Upper bound, or `None` for unbounded.
        end: Option<WireValue>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Entries by key index relative to a key.
    GetByKeyRelativeIndexRange {
        /// Key the indexes are relative to.
        key: WireValue,
        /// Starting index.
        index: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Entries with this value.
    GetByValue {
        /// Value to match.
        value: WireValue,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Entries with any of these values.
    GetByValueList {
        /// Values to match.
        values: Vec<WireValue>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Entries whose values are in a range.
    GetByValueRange {
        /// Lower bound, or `None` for unbounded.
        begin: Option<WireValue>,
        /// Upper bound, or `None` for unbounded.
        end: Option<WireValue>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// Entries by rank relative to a value.
    GetByValueRelativeRankRange {
        /// Value the ranks are relative to.
        value: WireValue,
        /// Starting rank.
        rank: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// The entry at this key index.
    GetByIndex {
        /// Index.
        index: i64,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// A range of entries by key index.
    GetByIndexRange {
        /// First index.
        index: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// The entry at this rank.
    GetByRank {
        /// Rank.
        rank: i64,
        /// What to return.
        return_type: WireMapReturn,
    },
    /// A range of entries by rank.
    GetByRankRange {
        /// First rank.
        rank: i64,
        /// How many, or `None` for the rest.
        count: Option<i64>,
        /// What to return.
        return_type: WireMapReturn,
    },
}

impl WireMapOp {
    /// Whether this operation writes.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        matches!(
            self,
            WireMapOp::Create { .. }
                | WireMapOp::SetOrder { .. }
                | WireMapOp::SetPolicy { .. }
                | WireMapOp::Put { .. }
                | WireMapOp::PutItems { .. }
                | WireMapOp::IncrementValue { .. }
                | WireMapOp::DecrementValue { .. }
                | WireMapOp::Clear
                | WireMapOp::RemoveByKey { .. }
                | WireMapOp::RemoveByKeyList { .. }
                | WireMapOp::RemoveByKeyRange { .. }
                | WireMapOp::RemoveByKeyRelativeIndexRange { .. }
                | WireMapOp::RemoveByValue { .. }
                | WireMapOp::RemoveByValueList { .. }
                | WireMapOp::RemoveByValueRange { .. }
                | WireMapOp::RemoveByValueRelativeRankRange { .. }
                | WireMapOp::RemoveByIndex { .. }
                | WireMapOp::RemoveByIndexRange { .. }
                | WireMapOp::RemoveByRank { .. }
                | WireMapOp::RemoveByRankRange { .. }
        )
    }
}

// ===== bitwise =====

/// What a bitwise write does about the bin already existing.
///
/// The same three-way choice a map write has, spelled the same way, for the same
/// reason: the client carries it as a bitmask and one knob per idea reads better
/// at a boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireBitWriteMode {
    /// Create or overwrite.
    Update,
    /// Overwrite only; fail if the bin is absent.
    UpdateOnly,
    /// Create only; fail if the bin is present.
    CreateOnly,
}

/// Write rules for the bitwise operations that write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireBitPolicy {
    /// What a write does about the bin already existing.
    pub write_mode: WireBitWriteMode,
    /// Do not fail the operation when the mode rejects it.
    pub no_fail: bool,
    /// Let other operations in the same call commit when this one is rejected.
    pub partial: bool,
}

impl Default for WireBitPolicy {
    fn default() -> WireBitPolicy {
        WireBitPolicy {
            write_mode: WireBitWriteMode::Update,
            no_fail: false,
            partial: false,
        }
    }
}

/// How [`WireBitOp::Resize`] changes the blob's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireBitResize {
    /// Add or remove at the end.
    Default,
    /// Add or remove at the front.
    FromFront,
    /// Refuse to shrink.
    GrowOnly,
    /// Refuse to grow.
    ShrinkOnly,
}

/// What an arithmetic bit operation does when the result does not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireBitOverflow {
    /// Fail the operation.
    Fail,
    /// Clamp to the largest or smallest representable value.
    Saturate,
    /// Wrap around.
    Wrap,
}

/// An operation on the bits of a blob.
///
/// Offsets and sizes are in **bits** everywhere except [`WireBitOp::Resize`],
/// [`WireBitOp::Insert`] and [`WireBitOp::Remove`], which work in whole bytes —
/// that split is the client's, and the field names carry it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireBitOp {
    // ----- whole bytes -----
    /// Grow or shrink the blob.
    Resize {
        /// New size in bytes.
        byte_size: i64,
        /// Which end to change, and whether either direction is refused.
        flags: Option<WireBitResize>,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Insert bytes.
    Insert {
        /// Where, in bytes.
        byte_offset: i64,
        /// Bytes to insert.
        value: WireValue,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Remove bytes.
    Remove {
        /// Where, in bytes.
        byte_offset: i64,
        /// How many bytes.
        byte_size: i64,
        /// Write rules.
        policy: WireBitPolicy,
    },

    // ----- bits -----
    /// Overwrite bits.
    Set {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
        /// Bits to write, as a blob.
        value: WireValue,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Bitwise OR.
    Or {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
        /// Operand, as a blob.
        value: WireValue,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Bitwise XOR.
    Xor {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
        /// Operand, as a blob.
        value: WireValue,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Bitwise AND.
    And {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
        /// Operand, as a blob.
        value: WireValue,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Invert bits.
    Not {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Shift bits left.
    LeftShift {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits the field is.
        bit_size: i64,
        /// How far to shift.
        shift: i64,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Shift bits right.
    RightShift {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits the field is.
        bit_size: i64,
        /// How far to shift.
        shift: i64,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Add to the integer held in a bit field.
    Add {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits the integer is.
        bit_size: i64,
        /// Amount to add.
        value: i64,
        /// Whether the field is signed.
        signed: bool,
        /// What to do if the result does not fit.
        overflow: WireBitOverflow,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Subtract from the integer held in a bit field.
    Subtract {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits the integer is.
        bit_size: i64,
        /// Amount to subtract.
        value: i64,
        /// Whether the field is signed.
        signed: bool,
        /// What to do if the result does not fit.
        overflow: WireBitOverflow,
        /// Write rules.
        policy: WireBitPolicy,
    },
    /// Write an integer into a bit field.
    SetInt {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits the integer is.
        bit_size: i64,
        /// Value to write.
        value: i64,
        /// Write rules.
        policy: WireBitPolicy,
    },

    // ----- read -----
    /// Read bits as a blob.
    Get {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
    },
    /// Count the set bits.
    Count {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
    },
    /// Index of the first bit with this value, searching forwards.
    LeftScan {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
        /// The bit value to look for.
        value: bool,
    },
    /// Index of the first bit with this value, searching backwards.
    RightScan {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
        /// The bit value to look for.
        value: bool,
    },
    /// Read a bit field as an integer.
    GetInt {
        /// Where, in bits.
        bit_offset: i64,
        /// How many bits.
        bit_size: i64,
        /// Whether to read it as signed.
        signed: bool,
    },
}

impl WireBitOp {
    /// Whether this operation writes.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        !matches!(
            self,
            WireBitOp::Get { .. }
                | WireBitOp::Count { .. }
                | WireBitOp::LeftScan { .. }
                | WireBitOp::RightScan { .. }
                | WireBitOp::GetInt { .. }
        )
    }
}

// ===== HyperLogLog =====

/// What a HyperLogLog write does about the bin already existing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireHllWriteMode {
    /// Create or update.
    Update,
    /// Update only; fail if the bin is absent.
    UpdateOnly,
    /// Create only; fail if the bin is present.
    CreateOnly,
}

/// Write rules for the HyperLogLog operations that write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireHllPolicy {
    /// What a write does about the bin already existing.
    pub write_mode: WireHllWriteMode,
    /// Do not fail the operation when the mode rejects it.
    pub no_fail: bool,
    /// Allow the result to take the smaller index-bit count when two sketches
    /// disagree, instead of refusing to combine them.
    pub allow_fold: bool,
}

impl Default for WireHllPolicy {
    fn default() -> WireHllPolicy {
        WireHllPolicy {
            write_mode: WireHllWriteMode::Update,
            no_fail: false,
            allow_fold: false,
        }
    }
}

/// An operation on a HyperLogLog sketch.
///
/// The counts these return are **estimates**: that is the point of the
/// structure, which answers "roughly how many distinct things" in a fixed few
/// kilobytes instead of storing the things.
///
/// `index_bit_count` and `min_hash_bit_count` are optional wherever the client
/// has a family of constructors for them, and `None` means "leave it to the
/// sketch that is already there" — the client's `-1` sentinel, named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireHllOp {
    /// Create a sketch, or reset the one that is there.
    Init {
        /// Precision, in index bits.
        index_bit_count: i64,
        /// MinHash bits, for similarity estimates.
        min_hash_bit_count: Option<i64>,
        /// Write rules.
        policy: WireHllPolicy,
    },
    /// Add values to the sketch, creating it if needed.
    Add {
        /// Values to add.
        values: Vec<WireValue>,
        /// Precision to create the sketch with, if it is missing.
        index_bit_count: Option<i64>,
        /// MinHash bits to create it with.
        min_hash_bit_count: Option<i64>,
        /// Write rules.
        policy: WireHllPolicy,
    },
    /// Merge other sketches into this one.
    SetUnion {
        /// The sketches to merge, as blobs.
        sketches: Vec<WireValue>,
        /// Write rules.
        policy: WireHllPolicy,
    },
    /// Recompute and cache the sketch's count.
    RefreshCount,
    /// Reduce the sketch's precision.
    Fold {
        /// The new, smaller index-bit count.
        index_bit_count: i64,
    },

    /// The estimated number of distinct values.
    GetCount,
    /// The union of this sketch and the given ones, as a sketch.
    GetUnion {
        /// The other sketches.
        sketches: Vec<WireValue>,
    },
    /// The estimated size of that union.
    GetUnionCount {
        /// The other sketches.
        sketches: Vec<WireValue>,
    },
    /// The estimated size of the intersection.
    GetIntersectCount {
        /// The other sketches.
        sketches: Vec<WireValue>,
    },
    /// The estimated Jaccard similarity, from 0.0 to 1.0.
    GetSimilarity {
        /// The other sketches.
        sketches: Vec<WireValue>,
    },
    /// The sketch's index-bit and MinHash-bit counts.
    Describe,
}

impl WireHllOp {
    /// Whether this operation writes.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        matches!(
            self,
            WireHllOp::Init { .. }
                | WireHllOp::Add { .. }
                | WireHllOp::SetUnion { .. }
                | WireHllOp::RefreshCount
                | WireHllOp::Fold { .. }
        )
    }
}

// ===== expressions =====

/// How an expression reaches the server.
///
/// Three forms, and which one a caller has depends on how they built it: as text
/// in the Aerospike Expression Language, as bytes another client packed, or as a
/// tree assembled from [`crate::exp`]'s constructors.
///
/// | form | packed by | needs |
/// | --- | --- | --- |
/// | [`Ael`](Self::Ael) | the **server**, from text | server 8.1.3+ |
/// | [`Base64`](Self::Base64) | another client, already | nothing |
/// | [`Tree`](Self::Tree) | the **daemon**, from the tree | nothing |
///
/// The text form is the shortest to write and the server does the parsing, but it
/// only exists from server 8.1.3. The tree form is packed client-side, so it works
/// against every server this client supports — which for expressions matters more
/// than for most things, since a filter is how a caller avoids reading records
/// they did not want.
///
/// Everything that takes an expression takes this, so all three forms are
/// available wherever one is: a policy filter, the expression read and write
/// operations, an expression-based secondary index, and a context filter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireExpression {
    /// Aerospike Expression Language source, compiled by the **server**.
    ///
    /// The same language the policy `filter` takes, and the same requirement:
    /// server 8.1.3 or later. The daemon checks the cluster's versions and
    /// refuses rather than letting an old server reject the text obscurely.
    Ael(String),
    /// A base64-encoded expression somebody else packed.
    ///
    /// For an expression built by another Aerospike client and stored or passed
    /// along. The daemon decodes it and sends the bytes as they are.
    Base64(String),
    /// An expression tree, which the daemon builds with `aerospike-core`'s own
    /// constructors and the client packs.
    ///
    /// No version requirement, because nothing on the server has to parse it.
    Tree(crate::exp::WireExp),
}

/// What an expression *write* does about the bin, and about failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireExpWriteFlags {
    /// Create or overwrite, create only, or update only.
    pub write_mode: WireExpWriteMode,
    /// Delete the bin when the expression evaluates to nil, instead of
    /// reporting that the operation did not apply.
    pub allow_delete: bool,
    /// Do not fail the operation when the mode rejects it.
    pub no_fail: bool,
    /// Do not fail when the expression itself resolves to nothing usable.
    pub eval_no_fail: bool,
}

impl Default for WireExpWriteFlags {
    fn default() -> WireExpWriteFlags {
        WireExpWriteFlags {
            write_mode: WireExpWriteMode::Update,
            allow_delete: false,
            no_fail: false,
            eval_no_fail: false,
        }
    }
}

/// What an expression write does about the bin already existing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireExpWriteMode {
    /// Create or overwrite.
    Update,
    /// Create only; fail if the bin is present.
    CreateOnly,
    /// Overwrite only; fail if the bin is absent.
    UpdateOnly,
}

/// What an expression *read* does about failures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireExpReadFlags {
    /// Do not fail when the expression resolves to nothing usable.
    pub eval_no_fail: bool,
}

/// Evaluate an expression and read or write the result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireExpOp {
    /// Evaluate, and return the result under `name`.
    ///
    /// `name` is not a bin: nothing is written, and it is only the label the
    /// answer comes back under — which is why it can be anything.
    Read {
        /// What to label the result.
        name: String,
        /// What to evaluate.
        expression: WireExpression,
        /// How to treat a failure.
        flags: WireExpReadFlags,
    },
    /// Evaluate, and write the result into `bin`.
    Write {
        /// Bin to write.
        bin: String,
        /// What to evaluate.
        expression: WireExpression,
        /// How to treat the bin and a failure.
        flags: WireExpWriteFlags,
    },
}

impl WireExpOp {
    /// Whether this operation writes.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        matches!(self, WireExpOp::Write { .. })
    }

    /// The bin a write lands in, or the label a read comes back under.
    #[must_use]
    pub fn bin(&self) -> &str {
        match self {
            WireExpOp::Read { name, .. } => name,
            WireExpOp::Write { bin, .. } => bin,
        }
    }

    /// What to evaluate.
    #[must_use]
    pub const fn expression(&self) -> &WireExpression {
        match self {
            WireExpOp::Read { expression, .. } | WireExpOp::Write { expression, .. } => expression,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_body, encode_body};

    fn round_trip(op: WireOp) {
        let bytes = encode_body(&op).unwrap();
        assert_eq!(decode_body::<WireOp>(&bytes).unwrap(), op, "{op:?}");
    }

    #[test]
    fn every_scalar_operation_round_trips() {
        for op in [
            WireOp::Get,
            WireOp::GetHeader,
            WireOp::GetBin { bin: "a".into() },
            WireOp::Put {
                bin: "a".into(),
                value: WireValue::Int(1),
            },
            WireOp::Append {
                bin: "a".into(),
                value: WireValue::Str("x".into()),
            },
            WireOp::Prepend {
                bin: "a".into(),
                value: WireValue::Str("x".into()),
            },
            WireOp::Add {
                bin: "a".into(),
                value: WireValue::Float(1.5),
            },
            WireOp::Touch,
            WireOp::Delete,
        ] {
            round_trip(op);
        }
    }

    #[test]
    fn a_nested_list_operation_round_trips_with_its_path() {
        round_trip(WireOp::List {
            bin: "profile".into(),
            ctx: vec![
                WireCtx::MapKey {
                    key: WireValue::Str("tags".into()),
                },
                WireCtx::ListIndex { index: -1 },
            ],
            op: WireListOp::Append {
                policy: WireListPolicy::default(),
                value: WireValue::Str("new".into()),
            },
        });
    }

    /// The two ends of the range convention, which is the one place this model
    /// differs from the client's: one variant with an optional count stands for
    /// the client's pair of constructors.
    #[test]
    fn a_range_with_no_count_means_to_the_end() {
        round_trip(WireOp::List {
            bin: "items".into(),
            ctx: vec![],
            op: WireListOp::GetRange {
                index: 2,
                count: None,
            },
        });
        round_trip(WireOp::List {
            bin: "items".into(),
            ctx: vec![],
            op: WireListOp::GetRange {
                index: 2,
                count: Some(3),
            },
        });
    }

    /// Whether an operation writes decides which policy an `operate()` runs
    /// under, so a misclassification would send a write out under a read
    /// policy — or refuse a read for no reason.
    #[test]
    fn writes_and_reads_are_told_apart() {
        assert!(!WireOp::Get.is_write());
        assert!(!WireOp::GetHeader.is_write());
        assert!(!WireOp::GetBin { bin: "a".into() }.is_write());
        assert!(WireOp::Touch.is_write());
        assert!(WireOp::Delete.is_write());
        assert!(WireOp::Put {
            bin: "a".into(),
            value: WireValue::Nil
        }
        .is_write());

        let list = |op| WireOp::List {
            bin: "items".into(),
            ctx: vec![],
            op,
        };
        assert!(list(WireListOp::Clear).is_write());
        assert!(list(WireListOp::Pop { index: 0 }).is_write());
        assert!(!list(WireListOp::Size).is_write());
        assert!(!list(WireListOp::Get { index: 0 }).is_write());
        assert!(!list(WireListOp::GetByRank {
            rank: 0,
            return_type: WireListReturn::of(WireListReturnKind::Values),
        })
        .is_write());
    }

    #[test]
    fn an_operation_names_the_bin_it_touches() {
        assert_eq!(WireOp::Get.bin(), None);
        assert_eq!(WireOp::Touch.bin(), None);
        assert_eq!(WireOp::GetBin { bin: "a".into() }.bin(), Some("a"));
        assert_eq!(
            WireOp::List {
                bin: "items".into(),
                ctx: vec![],
                op: WireListOp::Size,
            }
            .bin(),
            Some("items")
        );
    }

    #[test]
    fn every_context_step_round_trips() {
        for ctx in [
            WireCtx::ListIndex { index: 0 },
            WireCtx::ListIndexCreate {
                index: 0,
                order: WireListOrder::Ordered,
                pad: true,
            },
            WireCtx::ListRank { rank: -1 },
            WireCtx::ListValue {
                value: WireValue::Int(7),
            },
            WireCtx::MapIndex { index: 1 },
            WireCtx::MapRank { rank: -1 },
            WireCtx::MapKey {
                key: WireValue::Str("k".into()),
            },
            WireCtx::MapKeyCreate {
                key: WireValue::Str("k".into()),
                order: WireMapOrder::KeyValueOrdered,
            },
            WireCtx::MapValue {
                value: WireValue::Bool(true),
            },
        ] {
            let bytes = encode_body(&ctx).unwrap();
            assert_eq!(decode_body::<WireCtx>(&bytes).unwrap(), ctx, "{ctx:?}");
        }
    }

    #[test]
    fn a_return_type_carries_its_inversion() {
        let plain = WireListReturn::of(WireListReturnKind::Values);
        assert!(!plain.inverted);

        let inverted = WireListReturn {
            kind: WireListReturnKind::Values,
            inverted: true,
        };
        let bytes = encode_body(&inverted).unwrap();
        assert_eq!(decode_body::<WireListReturn>(&bytes).unwrap(), inverted);
        assert_ne!(plain, inverted);
    }

    #[test]
    fn a_map_operation_round_trips_with_its_path() {
        round_trip(WireOp::Map {
            bin: "profile".into(),
            ctx: vec![WireCtx::MapKey {
                key: WireValue::Str("settings".into()),
            }],
            op: WireMapOp::Put {
                policy: WireMapPolicy::default(),
                key: WireValue::Str("theme".into()),
                value: WireValue::Str("dark".into()),
            },
        });
    }

    /// A map entry has two halves, so its return type has more shapes than a
    /// list's — and each has to survive the wire, since the shape is the answer.
    #[test]
    fn every_map_return_kind_round_trips() {
        for kind in [
            WireMapReturnKind::None,
            WireMapReturnKind::Index,
            WireMapReturnKind::ReverseIndex,
            WireMapReturnKind::Rank,
            WireMapReturnKind::ReverseRank,
            WireMapReturnKind::Count,
            WireMapReturnKind::Key,
            WireMapReturnKind::Value,
            WireMapReturnKind::KeyValue,
            WireMapReturnKind::Exists,
            WireMapReturnKind::UnorderedMap,
            WireMapReturnKind::OrderedMap,
        ] {
            let value = WireMapReturn {
                kind,
                inverted: true,
            };
            let bytes = encode_body(&value).unwrap();
            assert_eq!(decode_body::<WireMapReturn>(&bytes).unwrap(), value);
        }
    }

    #[test]
    fn map_writes_and_reads_are_told_apart() {
        let map = |op| WireOp::Map {
            bin: "m".into(),
            ctx: vec![],
            op,
        };
        assert!(map(WireMapOp::Clear).is_write());
        assert!(map(WireMapOp::Put {
            policy: WireMapPolicy::default(),
            key: WireValue::Str("k".into()),
            value: WireValue::Int(1),
        })
        .is_write());
        assert!(!map(WireMapOp::Size).is_write());
        assert!(!map(WireMapOp::GetByKey {
            key: WireValue::Str("k".into()),
            return_type: WireMapReturn::of(WireMapReturnKind::Value),
        })
        .is_write());
        assert_eq!(map(WireMapOp::Size).bin(), Some("m"));
    }

    /// The client has a write mode *and* a flag bitmask that replaces it; this
    /// carries one mode plus the two flags that add something the mode cannot
    /// say, so the default must be the plainest possible write.
    /// The bit family is the one whose units change from operation to
    /// operation — bytes for three of them, bits for the rest — so both kinds
    /// have to survive the wire with their names intact.
    #[test]
    fn bit_operations_round_trip_in_bytes_and_in_bits() {
        round_trip(WireOp::Bit {
            bin: "flags".into(),
            ctx: vec![],
            op: WireBitOp::Resize {
                byte_size: 4,
                flags: Some(WireBitResize::FromFront),
                policy: WireBitPolicy::default(),
            },
        });
        round_trip(WireOp::Bit {
            bin: "flags".into(),
            ctx: vec![WireCtx::MapKey {
                key: WireValue::Str("nested".into()),
            }],
            op: WireBitOp::Add {
                bit_offset: 8,
                bit_size: 8,
                value: 1,
                signed: false,
                overflow: WireBitOverflow::Saturate,
                policy: WireBitPolicy::default(),
            },
        });
    }

    #[test]
    fn bit_writes_and_reads_are_told_apart() {
        let bit = |op| WireOp::Bit {
            bin: "b".into(),
            ctx: vec![],
            op,
        };
        assert!(bit(WireBitOp::Not {
            bit_offset: 0,
            bit_size: 8,
            policy: WireBitPolicy::default()
        })
        .is_write());
        assert!(!bit(WireBitOp::Count {
            bit_offset: 0,
            bit_size: 8
        })
        .is_write());
        assert!(!bit(WireBitOp::GetInt {
            bit_offset: 0,
            bit_size: 8,
            signed: true
        })
        .is_write());
    }

    /// `None` for a bit count is the client's `-1` sentinel with a name, and it
    /// has to stay distinguishable from a real count of zero.
    #[test]
    fn an_absent_hll_bit_count_is_not_a_zero_one() {
        let absent = WireHllOp::Add {
            values: vec![WireValue::Str("x".into())],
            index_bit_count: None,
            min_hash_bit_count: None,
            policy: WireHllPolicy::default(),
        };
        let zero = WireHllOp::Add {
            values: vec![WireValue::Str("x".into())],
            index_bit_count: Some(0),
            min_hash_bit_count: None,
            policy: WireHllPolicy::default(),
        };
        assert_ne!(absent, zero);

        for op in [absent, zero] {
            round_trip(WireOp::Hll {
                bin: "sketch".into(),
                ctx: vec![],
                op,
            });
        }
    }

    #[test]
    fn hll_writes_and_reads_are_told_apart() {
        let hll = |op| WireOp::Hll {
            bin: "sketch".into(),
            ctx: vec![],
            op,
        };
        assert!(hll(WireHllOp::RefreshCount).is_write());
        assert!(hll(WireHllOp::Fold { index_bit_count: 4 }).is_write());
        assert!(!hll(WireHllOp::GetCount).is_write());
        assert!(!hll(WireHllOp::Describe).is_write());
        assert!(!hll(WireHllOp::GetSimilarity { sketches: vec![] }).is_write());
        assert_eq!(hll(WireHllOp::Describe).bin(), Some("sketch"));
    }

    /// Both expression forms are text, and they mean different things, so a
    /// round trip has to keep them apart — sending AEL source where packed
    /// bytes were meant would reach the server as nonsense.
    #[test]
    fn the_two_expression_forms_stay_distinct() {
        let ael = WireExpression::Ael("$.a + $.b".into());
        let packed = WireExpression::Base64("kQE=".into());
        assert_ne!(ael, packed);

        for expression in [ael, packed] {
            round_trip(WireOp::Exp {
                op: WireExpOp::Read {
                    name: "total".into(),
                    expression: expression.clone(),
                    flags: WireExpReadFlags::default(),
                },
            });
            round_trip(WireOp::Exp {
                op: WireExpOp::Write {
                    bin: "total".into(),
                    expression,
                    flags: WireExpWriteFlags::default(),
                },
            });
        }
    }

    /// A read labels its answer and a write names a bin, and both are what
    /// `bin()` reports — the daemon and the extension both rely on it.
    #[test]
    fn an_expression_operation_reports_its_name_and_direction() {
        let read = WireOp::Exp {
            op: WireExpOp::Read {
                name: "total".into(),
                expression: WireExpression::Ael("1".into()),
                flags: WireExpReadFlags::default(),
            },
        };
        assert!(!read.is_write());
        assert_eq!(read.bin(), Some("total"));

        let write = WireOp::Exp {
            op: WireExpOp::Write {
                bin: "total".into(),
                expression: WireExpression::Ael("1".into()),
                flags: WireExpWriteFlags::default(),
            },
        };
        assert!(write.is_write());
        assert_eq!(write.bin(), Some("total"));
    }

    #[test]
    fn the_default_expression_flags_are_a_plain_write_and_a_plain_read() {
        let write = WireExpWriteFlags::default();
        assert_eq!(write.write_mode, WireExpWriteMode::Update);
        assert!(!write.allow_delete && !write.no_fail && !write.eval_no_fail);
        assert!(!WireExpReadFlags::default().eval_no_fail);
    }

    #[test]
    fn the_default_bit_and_hll_policies_are_plain_updates() {
        let bit = WireBitPolicy::default();
        assert_eq!(bit.write_mode, WireBitWriteMode::Update);
        assert!(!bit.no_fail && !bit.partial);

        let hll = WireHllPolicy::default();
        assert_eq!(hll.write_mode, WireHllWriteMode::Update);
        assert!(!hll.no_fail && !hll.allow_fold);
    }

    #[test]
    fn the_default_map_policy_is_an_unordered_update() {
        let policy = WireMapPolicy::default();
        assert_eq!(policy.order, WireMapOrder::Unordered);
        assert_eq!(policy.write_mode, WireMapWriteMode::Update);
        assert!(!policy.no_fail);
        assert!(!policy.partial);
        assert!(!policy.persist_index);
    }

    #[test]
    fn the_default_list_policy_is_unordered_with_no_flags() {
        let policy = WireListPolicy::default();
        assert_eq!(policy.order, WireListOrder::Unordered);
        assert_eq!(policy.flags, WireListWriteFlags::default());
        assert!(!policy.flags.add_unique);
        assert!(!policy.flags.no_fail);
    }
}
