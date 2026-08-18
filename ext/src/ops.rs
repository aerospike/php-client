// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

// The PHP-facing parameters are camelCase because PHP named arguments use the
// parameter name exactly as written — `ListOp::getByIndexRange('items', 0,
// returnType: ListReturn::Count)` — and `#[php_impl]` copies those identifiers
// into code it generates outside the impl block, so the lint cannot be silenced
// any closer than the module. See `crate::policy` for the same note.
#![allow(non_snake_case)]

//! Building the operations an `operate()` call runs.
//!
//! `aerospike-core` puts its operation constructors in a module per family —
//! `operations::scalar`, `operations::lists`, `operations::maps` — and this
//! mirrors that with a class per family whose static methods have the same
//! names:
//!
//! ```php
//! use Aerospike\{Op, ListOp, Ctx, ListReturn};
//!
//! $record = $client->operate(null, $key, [
//!     Op::add(new Aerospike\Bin('views', 1)),
//!     ListOp::append('history', $event),
//!     ListOp::getByIndexRange('history', -5, null, ListReturn::Values),
//!     Op::getBin('name'),
//! ]);
//! ```
//!
//! The operations run **in the order given**, on one record, atomically. That
//! is the whole reason `operate` exists: the increment, the append and the read
//! above cannot interleave with anyone else's write.
//!
//! # Nesting
//!
//! Any list or map operation can be aimed at a collection *inside* another one
//! by attaching a path, exactly as the Rust client's `.context(..)` does:
//!
//! ```php
//! ListOp::append('profile', 'admin')->context([Ctx::mapKey('roles')]);
//! ```
//!
//! # Reading the result
//!
//! `operate()` answers with a [`Record`](crate::Record) holding one entry per
//! operation that produced a result. Run two read operations against the same
//! bin and that bin's value is a **list of results in operation order** — the
//! server's own shape for it, reported faithfully rather than flattened.

use aerospike_php_ipc::op::{
    WireBitOp, WireBitOverflow, WireBitPolicy, WireCtx, WireExpOp, WireExpReadFlags,
    WireExpWriteFlags, WireExpWriteMode, WireExpression, WireHllOp, WireHllPolicy, WireListOp,
    WireListPolicy, WireListSort, WireListWriteFlags, WireMapOp, WireMapPolicy, WireMapReturn,
    WireOp,
};
use aerospike_php_ipc::WireValue;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

use crate::arg::Given;
use crate::enums::ExpType;
use crate::exp::Exp;
use crate::enums::{
    BinWriteMode, BitOverflow, BitResize, ListOrder, ListReturn, MapOrder, MapReturn,
    MapWriteMode,
};
use crate::error::{AeroError, AeroResult};
use crate::record::Bin;
use crate::value::{self, Path};

/// One operation in an `operate()` call.
///
/// Built by the static methods of [`Op`], [`ListOp`], [`MapOp`], [`BitOp`],
/// [`HllOp`] and [`ExpOp`]; there is no
/// constructor, because an operation with no operation in it is not a thing.
#[php_class]
#[php(name = "Aerospike\\Operation")]
#[derive(Debug, Clone)]
pub struct Operation {
    op: WireOp,
}

#[php_impl]
impl Operation {
    /// Aim this operation at a collection nested inside the bin.
    ///
    /// Returns a **new** operation: an operation is a value, and one that
    /// mutated when a path was attached could not be shared between calls.
    ///
    /// Only the collection operations can be nested. Attaching a path to a
    /// scalar operation is refused rather than ignored — the server would run
    /// the operation on the bin itself, which is not what the path asked for.
    pub fn context(&self, ctx: Vec<&Zval>) -> PhpResult<Operation> {
        if !matches!(
            self.op,
            WireOp::List { .. } | WireOp::Map { .. } | WireOp::Bit { .. } | WireOp::Hll { .. }
        ) {
            return Err(AeroError::client(format!(
                "a context path can only be attached to a collection operation, and this one is \
                 {}",
                self.describe()
            ))
            .into());
        }
        let path = context_list(&ctx)?;

        Ok(Operation {
            op: match &self.op {
                WireOp::List { bin, op, .. } => WireOp::List {
                    bin: bin.clone(),
                    ctx: path,
                    op: op.clone(),
                },
                WireOp::Map { bin, op, .. } => WireOp::Map {
                    bin: bin.clone(),
                    ctx: path,
                    op: op.clone(),
                },
                WireOp::Bit { bin, op, .. } => WireOp::Bit {
                    bin: bin.clone(),
                    ctx: path,
                    op: op.clone(),
                },
                WireOp::Hll { bin, op, .. } => WireOp::Hll {
                    bin: bin.clone(),
                    ctx: path,
                    op: op.clone(),
                },
                // Refused above; every other operation names no collection.
                other => other.clone(),
            },
        })
    }

    /// The bin this operation reads or writes, or `null` for the whole-record
    /// operations.
    pub fn bin(&self) -> Option<String> {
        self.op.bin().map(ToOwned::to_owned)
    }

    /// Whether this operation writes.
    ///
    /// An `operate()` call with any write in it takes a write lock on the
    /// record and bumps its generation, so this is worth being able to check.
    pub fn is_write(&self) -> bool {
        self.op.is_write()
    }
}

impl Operation {
    /// The contract's operation.
    #[must_use]
    pub fn to_wire(&self) -> WireOp {
        self.op.clone()
    }

    /// A short description, for the message when a path cannot be attached.
    fn describe(&self) -> String {
        match &self.op {
            WireOp::Get => "a whole-record read".to_owned(),
            WireOp::GetHeader => "a metadata read".to_owned(),
            WireOp::Touch => "a touch".to_owned(),
            WireOp::Delete => "a delete".to_owned(),
            // An expression names what it reads for itself, so the client gives
            // these operations no path either.
            WireOp::Exp { .. } => "an expression operation".to_owned(),
            other => format!(
                "a scalar operation on bin \"{}\"",
                other.bin().unwrap_or_default()
            ),
        }
    }

    fn scalar(op: WireOp) -> Operation {
        Operation { op }
    }

    fn list(bin: String, op: WireListOp) -> Operation {
        Operation {
            op: WireOp::List {
                bin,
                ctx: Vec::new(),
                op,
            },
        }
    }

    fn map(bin: String, op: WireMapOp) -> Operation {
        Operation {
            op: WireOp::Map {
                bin,
                ctx: Vec::new(),
                op,
            },
        }
    }

    fn bit(bin: String, op: WireBitOp) -> Operation {
        Operation {
            op: WireOp::Bit {
                bin,
                ctx: Vec::new(),
                op,
            },
        }
    }

    fn hll(bin: String, op: WireHllOp) -> Operation {
        Operation {
            op: WireOp::Hll {
                bin,
                ctx: Vec::new(),
                op,
            },
        }
    }

    fn exp(op: WireExpOp) -> Operation {
        Operation {
            op: WireOp::Exp { op },
        }
    }
}

/// A step in the path to a collection nested inside a bin.
///
/// Mirrors the Rust client's `ctx_*` constructors. Pass a list of these to
/// [`Operation::context`].
///
/// ```php
/// // The list at $record["profile"]["roles"]
/// ListOp::size('profile')->context([Ctx::mapKey('roles')]);
///
/// // The list at $record["matrix"][0]
/// ListOp::append('matrix', 9)->context([Ctx::listIndex(0)]);
/// ```
#[php_class]
#[php(name = "Aerospike\\Ctx")]
#[derive(Debug, Clone)]
pub struct Ctx {
    ctx: WireCtx,
}

#[php_impl]
impl Ctx {
    /// The list element at `index`. Negative counts from the end.
    pub fn list_index(index: i64) -> Ctx {
        Ctx {
            ctx: WireCtx::ListIndex { index },
        }
    }

    /// The list element at `index`, creating the list if the path is missing.
    ///
    /// `pad` fills the gap with nils when the index is past the end; without
    /// it, an index outside the list is an error.
    #[php(defaults(pad = false))]
    pub fn list_index_create(index: i64, order: ListOrder, pad: bool) -> Ctx {
        Ctx {
            ctx: WireCtx::ListIndexCreate {
                index,
                order: order.to_wire(),
                pad,
            },
        }
    }

    /// The list element at `rank` in value order. Negative counts from the
    /// largest.
    pub fn list_rank(rank: i64) -> Ctx {
        Ctx {
            ctx: WireCtx::ListRank { rank },
        }
    }

    /// The first list element equal to `value`.
    pub fn list_value(value: &Zval) -> PhpResult<Ctx> {
        Ok(Ctx {
            ctx: WireCtx::ListValue {
                value: wire_value(value, "value")?,
            },
        })
    }

    /// The map entry at `index` in key order.
    pub fn map_index(index: i64) -> Ctx {
        Ctx {
            ctx: WireCtx::MapIndex { index },
        }
    }

    /// The map entry at `rank` in value order.
    pub fn map_rank(rank: i64) -> Ctx {
        Ctx {
            ctx: WireCtx::MapRank { rank },
        }
    }

    /// The map entry with this key.
    pub fn map_key(key: &Zval) -> PhpResult<Ctx> {
        Ok(Ctx {
            ctx: WireCtx::MapKey {
                key: wire_value(key, "key")?,
            },
        })
    }

    /// The map entry with this key, creating the map if the path is missing.
    pub fn map_key_create(key: &Zval, order: MapOrder) -> PhpResult<Ctx> {
        Ok(Ctx {
            ctx: WireCtx::MapKeyCreate {
                key: wire_value(key, "key")?,
                order: order.to_wire(),
            },
        })
    }

    /// The first map entry whose value is this.
    /// Every child of the current node — every list element, every map entry.
    ///
    /// The step that makes a path expression one: the others select a single node,
    /// and this fans out so that what follows applies to each child. Needs server
    /// 8.1.1 or later, which the daemon checks.
    ///
    /// ```php
    /// // The price of every book, rather than of one book.
    /// ExpPath::selectValues(ExpType::ListType, Exp::mapBin('books'),
    ///     [Ctx::allChildren(), Ctx::mapKey('price')]);
    /// ```
    pub fn all_children() -> Ctx {
        Ctx {
            ctx: WireCtx::AllChildren,
        }
    }

    /// Every child a filter accepts.
    ///
    /// The filter runs per child, with `Exp::loopVar()` standing for the child
    /// being tested — so it can compare against the child's key, value or index.
    /// Needs server 8.1.1 or later.
    ///
    /// The filter must be a **built** expression: it is evaluated inside a packed
    /// tree, and there is nowhere there to put text for the server to parse.
    pub fn all_children_with_filter(filter: &Expression) -> PhpResult<Ctx> {
        let wire = filter.to_wire();
        if !matches!(wire, WireExpression::Tree(_)) {
            return Err(AeroError::client(
                "a context filter must be built with Aerospike\\Exp: it is evaluated inside a \
                 packed expression tree, and text the server would have to parse cannot go there",
            )
            .into());
        }
        Ok(Ctx {
            ctx: WireCtx::AllChildrenWithFilter {
                filter: Box::new(wire),
            },
        })
    }

    pub fn map_value(value: &Zval) -> PhpResult<Ctx> {
        Ok(Ctx {
            ctx: WireCtx::MapValue {
                value: wire_value(value, "value")?,
            },
        })
    }
}

impl Ctx {
    /// The contract's context step.
    #[must_use]
    pub fn to_wire(&self) -> WireCtx {
        self.ctx.clone()
    }
}

/// Order and write rules for the list operations that add items.
///
/// ```php
/// // A sorted list that refuses duplicates, and says so rather than failing
/// // the whole operate() when one turns up.
/// $policy = new Aerospike\ListPolicy(
///     order:     Aerospike\ListOrder::Ordered,
///     addUnique: true,
///     noFail:    true,
/// );
/// ```
#[php_class]
#[php(name = "Aerospike\\ListPolicy")]
#[derive(Debug, Clone, Copy, Default)]
pub struct ListPolicy {
    policy: WireListPolicy,
}

#[php_impl]
impl ListPolicy {
    /// Build a list policy. Everything is optional and defaults to the
    /// server's own behaviour: an unordered list that accepts anything.
    #[php(defaults(
        order = None,
        addUnique = false,
        insertBounded = false,
        noFail = false,
        partial = false
    ))]
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn __construct(
        order: Option<Given<ListOrder>>,
        addUnique: bool,
        insertBounded: bool,
        noFail: bool,
        partial: bool,
    ) -> PhpResult<ListPolicy> {
        let order = Given::or_none(order, "order")?;
        Ok(ListPolicy {
            policy: WireListPolicy {
                order: order.unwrap_or(ListOrder::Unordered).to_wire(),
                flags: WireListWriteFlags {
                    add_unique: addUnique,
                    insert_bounded: insertBounded,
                    no_fail: noFail,
                    partial,
                },
            },
        })
    }

    /// Whether the list this policy creates is value-ordered.
    pub fn is_ordered(&self) -> bool {
        self.policy.order == aerospike_php_ipc::op::WireListOrder::Ordered
    }

    /// Whether duplicate values are rejected.
    pub fn add_unique(&self) -> bool {
        self.policy.flags.add_unique
    }

    /// Whether an insert outside the list's range is refused.
    pub fn insert_bounded(&self) -> bool {
        self.policy.flags.insert_bounded
    }

    /// Whether a rejected item leaves the operation successful.
    pub fn no_fail(&self) -> bool {
        self.policy.flags.no_fail
    }

    /// Whether acceptable items are applied when others are rejected.
    pub fn partial(&self) -> bool {
        self.policy.flags.partial
    }
}

impl ListPolicy {
    /// The contract's policy, or the default when PHP passed none.
    pub(crate) fn wire(policy: Option<&ListPolicy>) -> WireListPolicy {
        policy.map_or_else(WireListPolicy::default, |policy| policy.policy)
    }
}

/// The operations that work on any bin: `aerospike-core`'s
/// `operations::scalar`, method for method.
#[php_class]
#[php(name = "Aerospike\\Op")]
#[derive(Debug, Clone, Copy)]
pub struct Op;

#[php_impl]
impl Op {
    /// Read every bin of the record.
    pub fn get() -> Operation {
        Operation::scalar(WireOp::Get)
    }

    /// Read the record's metadata and no bins.
    pub fn get_header() -> Operation {
        Operation::scalar(WireOp::GetHeader)
    }

    /// Read one bin.
    pub fn get_bin(bin: String) -> Operation {
        Operation::scalar(WireOp::GetBin { bin })
    }

    /// Write one bin. A `null` value deletes it.
    pub fn put(bin: &Bin) -> Operation {
        let (name, value) = bin.to_wire();
        Operation::scalar(WireOp::Put { bin: name, value })
    }

    /// Append to a string or blob bin.
    pub fn append(bin: &Bin) -> Operation {
        let (name, value) = bin.to_wire();
        Operation::scalar(WireOp::Append { bin: name, value })
    }

    /// Prepend to a string or blob bin.
    pub fn prepend(bin: &Bin) -> Operation {
        let (name, value) = bin.to_wire();
        Operation::scalar(WireOp::Prepend { bin: name, value })
    }

    /// Add a numeric delta to a bin. Negative subtracts.
    pub fn add(bin: &Bin) -> Operation {
        let (name, value) = bin.to_wire();
        Operation::scalar(WireOp::Add { bin: name, value })
    }

    /// Reset the record's time-to-live, using the policy's `expiration`.
    pub fn touch() -> Operation {
        Operation::scalar(WireOp::Touch)
    }

    /// Delete the record.
    ///
    /// Operations after this one in the same call still run, which is how a
    /// record is replaced rather than merged.
    pub fn delete() -> Operation {
        Operation::scalar(WireOp::Delete)
    }
}

/// The list operations: `aerospike-core`'s `operations::lists`, method for
/// method.
///
/// Every index and rank may be negative, counting from the end of the list and
/// from the largest value respectively. Where the Rust client has a pair of
/// functions — `get_range` and `get_range_from` — this has one method with a
/// nullable `$count`, and `null` means "to the end of the list".
///
/// The `...By...` methods take a [`ListReturn`](crate::ListReturn) saying what
/// to give back, and `$inverted` to select everything *except* what was named —
/// which for a remove operation removes the rest.
#[php_class]
#[php(name = "Aerospike\\ListOp")]
#[derive(Debug, Clone, Copy)]
pub struct ListOp;

#[php_impl]
impl ListOp {
    // ----- create and order -----

    /// Create an empty list in this bin.
    ///
    /// `persistIndex` keeps an index on a top-level list, which makes index and
    /// rank operations cheaper at the cost of some storage. `pad` applies to a
    /// nested list reached by an index past the end.
    #[php(defaults(pad = false, persistIndex = false))]
    pub fn create(bin: String, order: ListOrder, pad: bool, persistIndex: bool) -> Operation {
        Operation::list(
            bin,
            WireListOp::Create {
                order: order.to_wire(),
                pad,
                persist_index: persistIndex,
            },
        )
    }

    /// Change an existing list's order.
    #[php(defaults(persistIndex = false))]
    pub fn set_order(bin: String, order: ListOrder, persistIndex: bool) -> Operation {
        Operation::list(
            bin,
            WireListOp::SetOrder {
                order: order.to_wire(),
                persist_index: persistIndex,
            },
        )
    }

    // ----- add -----

    /// Append one value.
    #[php(defaults(policy = None))]
    pub fn append(bin: String, value: &Zval, policy: Option<Given<&ListPolicy>>) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::Append {
                policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
                value: wire_value(value, "value")?,
            },
        ))
    }

    /// Append several values.
    #[php(defaults(policy = None))]
    pub fn append_items(
        bin: String,
        values: Vec<&Zval>,
        policy: Option<Given<&ListPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::AppendItems {
                policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
                values: wire_values(&values)?,
            },
        ))
    }

    /// Insert one value at `index`.
    #[php(defaults(policy = None))]
    pub fn insert(
        bin: String,
        index: i64,
        value: &Zval,
        policy: Option<Given<&ListPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::Insert {
                policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
                index,
                value: wire_value(value, "value")?,
            },
        ))
    }

    /// Insert several values at `index`.
    #[php(defaults(policy = None))]
    pub fn insert_items(
        bin: String,
        index: i64,
        values: Vec<&Zval>,
        policy: Option<Given<&ListPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::InsertItems {
                policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
                index,
                values: wire_values(&values)?,
            },
        ))
    }

    // ----- remove -----

    /// Remove and return the item at `index`.
    pub fn pop(bin: String, index: i64) -> Operation {
        Operation::list(bin, WireListOp::Pop { index })
    }

    /// Remove and return `count` items from `index`; `null` for the rest.
    #[php(defaults(count = None))]
    pub fn pop_range(bin: String, index: i64, count: Option<Given<i64>>) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::PopRange {
                index,
                count: Given::or_none(count, "count")?,
            },
        ))
    }

    /// Remove the item at `index`.
    pub fn remove(bin: String, index: i64) -> Operation {
        Operation::list(bin, WireListOp::Remove { index })
    }

    /// Remove `count` items from `index`; `null` for the rest.
    #[php(defaults(count = None))]
    pub fn remove_range(bin: String, index: i64, count: Option<Given<i64>>) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::RemoveRange {
                index,
                count: Given::or_none(count, "count")?,
            },
        ))
    }

    /// Remove every item equal to `value`.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_value(
        bin: String,
        value: &Zval,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::RemoveByValue {
                value: wire_value(value, "value")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove every item equal to any of `values`.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_value_list(
        bin: String,
        values: Vec<&Zval>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::RemoveByValueList {
                values: wire_values(&values)?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove items from `begin` inclusive to `end` exclusive; `null` for an
    /// open end.
    #[php(defaults(begin = None, end = None, returnType = None, inverted = false))]
    pub fn remove_by_value_range(
        bin: String,
        begin: Option<&Zval>,
        end: Option<&Zval>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::RemoveByValueRange {
                begin: optional_value(begin, "begin")?,
                end: optional_value(end, "end")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove items by rank relative to `value`.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn remove_by_value_relative_rank_range(
        bin: String,
        value: &Zval,
        rank: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::RemoveByValueRelativeRankRange {
                value: wire_value(value, "value")?,
                rank,
                count: Given::or_none(count, "count")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove the item at `index`.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_index(
        bin: String,
        index: i64,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::RemoveByIndex {
                index,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove `count` items from `index`; `null` for the rest.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn remove_by_index_range(
        bin: String,
        index: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::RemoveByIndexRange {
                index,
                count: Given::or_none(count, "count")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove the item at `rank`.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_rank(
        bin: String,
        rank: i64,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::RemoveByRank {
                rank,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove `count` items from `rank`; `null` for the rest.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn remove_by_rank_range(
        bin: String,
        rank: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::RemoveByRankRange {
                rank,
                count: Given::or_none(count, "count")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    // ----- change in place -----

    /// Replace the item at `index`.
    #[php(defaults(policy = None))]
    pub fn set(
        bin: String,
        index: i64,
        value: &Zval,
        policy: Option<Given<&ListPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::Set {
                policy: Given::or_none(policy, "policy")?.map(|policy| policy.policy),
                index,
                value: wire_value(value, "value")?,
            },
        ))
    }

    /// Keep `count` items from `index` and remove the rest.
    pub fn trim(bin: String, index: i64, count: i64) -> Operation {
        Operation::list(bin, WireListOp::Trim { index, count })
    }

    /// Remove every item.
    pub fn clear(bin: String) -> Operation {
        Operation::list(bin, WireListOp::Clear)
    }

    /// Add `value` to the item at `index`; `null` adds one.
    #[php(defaults(value = None, policy = None))]
    pub fn increment(
        bin: String,
        index: i64,
        value: Option<Given<i64>>,
        policy: Option<Given<&ListPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::Increment {
                policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
                index,
                value: Given::or_none(value, "value")?,
            },
        ))
    }

    /// Sort the list in place.
    #[php(defaults(descending = false, dropDuplicates = false))]
    pub fn sort(bin: String, descending: bool, dropDuplicates: bool) -> Operation {
        Operation::list(
            bin,
            WireListOp::Sort {
                flags: WireListSort {
                    descending,
                    drop_duplicates: dropDuplicates,
                },
            },
        )
    }

    // ----- read -----

    /// How many items the list holds.
    pub fn size(bin: String) -> Operation {
        Operation::list(bin, WireListOp::Size)
    }

    /// The item at `index`.
    pub fn get(bin: String, index: i64) -> Operation {
        Operation::list(bin, WireListOp::Get { index })
    }

    /// `count` items from `index`; `null` for the rest.
    #[php(defaults(count = None))]
    pub fn get_range(bin: String, index: i64, count: Option<Given<i64>>) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::GetRange {
                index,
                count: Given::or_none(count, "count")?,
            },
        ))
    }

    /// Items equal to `value`.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_value(
        bin: String,
        value: &Zval,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::GetByValue {
                value: wire_value(value, "value")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Items equal to any of `values`.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_value_list(
        bin: String,
        values: Vec<&Zval>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::GetByValueList {
                values: wire_values(&values)?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Items from `begin` inclusive to `end` exclusive; `null` for an open end.
    #[php(defaults(begin = None, end = None, returnType = None, inverted = false))]
    pub fn get_by_value_range(
        bin: String,
        begin: Option<&Zval>,
        end: Option<&Zval>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::GetByValueRange {
                begin: optional_value(begin, "begin")?,
                end: optional_value(end, "end")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// Items by rank relative to `value`.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn get_by_value_relative_rank_range(
        bin: String,
        value: &Zval,
        rank: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::GetByValueRelativeRankRange {
                value: wire_value(value, "value")?,
                rank,
                count: Given::or_none(count, "count")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// The item at `index`.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_index(
        bin: String,
        index: i64,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::GetByIndex {
                index,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// `count` items from `index`; `null` for the rest.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn get_by_index_range(
        bin: String,
        index: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::GetByIndexRange {
                index,
                count: Given::or_none(count, "count")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// The item at `rank`.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_rank(
        bin: String,
        rank: i64,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::GetByRank {
                rank,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }

    /// `count` items from `rank`; `null` for the rest.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn get_by_rank_range(
        bin: String,
        rank: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<ListReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::list(
            bin,
            WireListOp::GetByRankRange {
                rank,
                count: Given::or_none(count, "count")?,
                return_type: returns(returnType, inverted)?,
            },
        ))
    }
}

/// Order and write rules for the map operations that write.
///
/// ```php
/// // A key-ordered map whose writes must not create new keys, and which
/// // silently skips the ones that would.
/// $policy = new Aerospike\MapPolicy(
///     order:     Aerospike\MapOrder::KeyOrdered,
///     writeMode: Aerospike\MapWriteMode::UpdateOnly,
///     noFail:    true,
///     partial:   true,
/// );
/// ```
///
/// The Rust client has a write *mode* and a flag bitmask that replaces the mode
/// when it is non-zero — two ways to say the same thing. This carries the mode
/// plus the two flags that say something the mode cannot, and the daemon
/// combines them.
#[php_class]
#[php(name = "Aerospike\\MapPolicy")]
#[derive(Debug, Clone, Copy, Default)]
pub struct MapPolicy {
    policy: WireMapPolicy,
}

#[php_impl]
impl MapPolicy {
    /// Build a map policy. Everything is optional; the default is an unordered
    /// map whose writes create or overwrite.
    #[php(defaults(
        order = None,
        writeMode = None,
        noFail = false,
        partial = false,
        persistIndex = false
    ))]
    pub fn __construct(
        order: Option<Given<MapOrder>>,
        writeMode: Option<Given<MapWriteMode>>,
        noFail: bool,
        partial: bool,
        persistIndex: bool,
    ) -> PhpResult<MapPolicy> {
        let order = Given::or_none(order, "order")?;
        let write_mode = Given::or_none(writeMode, "writeMode")?;
        Ok(MapPolicy {
            policy: WireMapPolicy {
                order: order.unwrap_or(MapOrder::Unordered).to_wire(),
                write_mode: write_mode.unwrap_or(MapWriteMode::Update).to_wire(),
                no_fail: noFail,
                partial,
                persist_index: persistIndex,
            },
        })
    }

    /// Whether a created map is key-ordered or key/value-ordered.
    pub fn is_ordered(&self) -> bool {
        self.policy.order != aerospike_php_ipc::op::WireMapOrder::Unordered
    }

    /// Whether a rejected entry leaves the operation successful.
    pub fn no_fail(&self) -> bool {
        self.policy.no_fail
    }

    /// Whether acceptable entries are applied when others are rejected.
    pub fn partial(&self) -> bool {
        self.policy.partial
    }

    /// Whether the map keeps a persistent index.
    pub fn persist_index(&self) -> bool {
        self.policy.persist_index
    }
}

impl MapPolicy {
    /// The contract's policy, or the default when PHP passed none.
    pub(crate) fn wire(policy: Option<&MapPolicy>) -> WireMapPolicy {
        policy.map_or_else(WireMapPolicy::default, |policy| policy.policy)
    }
}

/// The map operations: `aerospike-core`'s `operations::maps`, method for method.
///
/// A map entry has a key *and* a value, so most operations come in both flavours
/// — `getByKey` and `getByValue`, `removeByKeyRange` and `removeByValueRange` —
/// and `$returnType` decides which half comes back. It defaults to
/// `MapReturn::Value`.
///
/// As with lists, indexes and ranks may be negative, and a `null` `$count` means
/// "to the end".
#[php_class]
#[php(name = "Aerospike\\MapOp")]
#[derive(Debug, Clone, Copy)]
pub struct MapOp;

#[php_impl]
impl MapOp {
    // ----- create and order -----

    /// Create an empty map in this bin.
    ///
    /// With a path attached this creates the map *at that path*; without one it
    /// is the same as `setOrder`. `persistIndex` applies to a top-level map only.
    #[php(defaults(persistIndex = false))]
    pub fn create(bin: String, order: MapOrder, persistIndex: bool) -> Operation {
        Operation::map(
            bin,
            WireMapOp::Create {
                order: order.to_wire(),
                persist_index: persistIndex,
            },
        )
    }

    /// Change an existing map's order.
    pub fn set_order(bin: String, order: MapOrder) -> Operation {
        Operation::map(
            bin,
            WireMapOp::SetOrder {
                order: order.to_wire(),
            },
        )
    }

    /// Replace the map's policy.
    pub fn set_policy(bin: String, policy: &MapPolicy) -> Operation {
        Operation::map(bin, WireMapOp::SetPolicy { policy: policy.policy })
    }

    // ----- write -----

    /// Write one entry.
    #[php(defaults(policy = None))]
    pub fn put(
        bin: String,
        key: &Zval,
        value: &Zval,
        policy: Option<Given<&MapPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::Put {
                policy: MapPolicy::wire(Given::or_none(policy, "policy")?),
                key: wire_value(key, "key")?,
                value: wire_value(value, "value")?,
            },
        ))
    }

    /// Write several entries, given as a PHP array, an `Aerospike\\OrderedMap`
    /// or an `Aerospike\\SortedMap`.
    #[php(defaults(policy = None))]
    pub fn put_items(
        bin: String,
        items: &Zval,
        policy: Option<Given<&MapPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::PutItems {
                policy: MapPolicy::wire(Given::or_none(policy, "policy")?),
                // An array, an `OrderedMap` or a `SortedMap`: a map argument
                // accepts every shape a map bin value does.
                items: crate::maps::entries_of(items)?,
            },
        ))
    }

    /// Add a delta to one entry's value, creating the entry if it is missing.
    #[php(defaults(policy = None))]
    pub fn increment_value(
        bin: String,
        key: &Zval,
        delta: &Zval,
        policy: Option<Given<&MapPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::IncrementValue {
                policy: MapPolicy::wire(Given::or_none(policy, "policy")?),
                key: wire_value(key, "key")?,
                delta: wire_value(delta, "delta")?,
            },
        ))
    }

    /// Subtract a delta from one entry's value.
    #[php(defaults(policy = None))]
    pub fn decrement_value(
        bin: String,
        key: &Zval,
        delta: &Zval,
        policy: Option<Given<&MapPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::DecrementValue {
                policy: MapPolicy::wire(Given::or_none(policy, "policy")?),
                key: wire_value(key, "key")?,
                delta: wire_value(delta, "delta")?,
            },
        ))
    }

    /// Remove every entry.
    pub fn clear(bin: String) -> Operation {
        Operation::map(bin, WireMapOp::Clear)
    }

    // ----- remove -----

    /// Remove the entry with this key.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_key(
        bin: String,
        key: &Zval,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByKey {
                key: wire_value(key, "key")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove the entries with any of these keys.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_key_list(
        bin: String,
        keys: Vec<&Zval>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByKeyList {
                keys: wire_values(&keys)?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove entries whose keys run from `begin` inclusive to `end` exclusive;
    /// `null` for an open end.
    #[php(defaults(begin = None, end = None, returnType = None, inverted = false))]
    pub fn remove_by_key_range(
        bin: String,
        begin: Option<&Zval>,
        end: Option<&Zval>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByKeyRange {
                begin: optional_value(begin, "begin")?,
                end: optional_value(end, "end")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove entries by key index relative to a key.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn remove_by_key_relative_index_range(
        bin: String,
        key: &Zval,
        index: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByKeyRelativeIndexRange {
                key: wire_value(key, "key")?,
                index,
                count: Given::or_none(count, "count")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove entries with this value.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_value(
        bin: String,
        value: &Zval,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByValue {
                value: wire_value(value, "value")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove entries with any of these values.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_value_list(
        bin: String,
        values: Vec<&Zval>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByValueList {
                values: wire_values(&values)?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove entries whose values are in a range.
    #[php(defaults(begin = None, end = None, returnType = None, inverted = false))]
    pub fn remove_by_value_range(
        bin: String,
        begin: Option<&Zval>,
        end: Option<&Zval>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByValueRange {
                begin: optional_value(begin, "begin")?,
                end: optional_value(end, "end")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove entries by rank relative to a value.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn remove_by_value_relative_rank_range(
        bin: String,
        value: &Zval,
        rank: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByValueRelativeRankRange {
                value: wire_value(value, "value")?,
                rank,
                count: Given::or_none(count, "count")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove the entry at this key index.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_index(
        bin: String,
        index: i64,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByIndex {
                index,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove `count` entries from this key index; `null` for the rest.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn remove_by_index_range(
        bin: String,
        index: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByIndexRange {
                index,
                count: Given::or_none(count, "count")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove the entry at this rank.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn remove_by_rank(
        bin: String,
        rank: i64,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByRank {
                rank,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Remove `count` entries from this rank; `null` for the rest.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn remove_by_rank_range(
        bin: String,
        rank: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::RemoveByRankRange {
                rank,
                count: Given::or_none(count, "count")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    // ----- read -----

    /// How many entries the map holds.
    pub fn size(bin: String) -> Operation {
        Operation::map(bin, WireMapOp::Size)
    }

    /// The entry with this key.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_key(
        bin: String,
        key: &Zval,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByKey {
                key: wire_value(key, "key")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// The entries with any of these keys.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_key_list(
        bin: String,
        keys: Vec<&Zval>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByKeyList {
                keys: wire_values(&keys)?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Entries whose keys run from `begin` inclusive to `end` exclusive; `null`
    /// for an open end.
    #[php(defaults(begin = None, end = None, returnType = None, inverted = false))]
    pub fn get_by_key_range(
        bin: String,
        begin: Option<&Zval>,
        end: Option<&Zval>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByKeyRange {
                begin: optional_value(begin, "begin")?,
                end: optional_value(end, "end")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Entries by key index relative to a key.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn get_by_key_relative_index_range(
        bin: String,
        key: &Zval,
        index: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByKeyRelativeIndexRange {
                key: wire_value(key, "key")?,
                index,
                count: Given::or_none(count, "count")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Entries with this value.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_value(
        bin: String,
        value: &Zval,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByValue {
                value: wire_value(value, "value")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Entries with any of these values.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_value_list(
        bin: String,
        values: Vec<&Zval>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByValueList {
                values: wire_values(&values)?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Entries whose values are in a range.
    #[php(defaults(begin = None, end = None, returnType = None, inverted = false))]
    pub fn get_by_value_range(
        bin: String,
        begin: Option<&Zval>,
        end: Option<&Zval>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByValueRange {
                begin: optional_value(begin, "begin")?,
                end: optional_value(end, "end")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// Entries by rank relative to a value.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn get_by_value_relative_rank_range(
        bin: String,
        value: &Zval,
        rank: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByValueRelativeRankRange {
                value: wire_value(value, "value")?,
                rank,
                count: Given::or_none(count, "count")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// The entry at this key index.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_index(
        bin: String,
        index: i64,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByIndex {
                index,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// `count` entries from this key index; `null` for the rest.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn get_by_index_range(
        bin: String,
        index: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByIndexRange {
                index,
                count: Given::or_none(count, "count")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// The entry at this rank.
    #[php(defaults(returnType = None, inverted = false))]
    pub fn get_by_rank(
        bin: String,
        rank: i64,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByRank {
                rank,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }

    /// `count` entries from this rank; `null` for the rest.
    #[php(defaults(count = None, returnType = None, inverted = false))]
    pub fn get_by_rank_range(
        bin: String,
        rank: i64,
        count: Option<Given<i64>>,
        returnType: Option<Given<MapReturn>>,
        inverted: bool,
    ) -> PhpResult<Operation> {
        Ok(Operation::map(
            bin,
            WireMapOp::GetByRankRange {
                rank,
                count: Given::or_none(count, "count")?,
                return_type: map_returns(returnType, inverted)?,
            },
        ))
    }
}

/// Write rules for the bitwise and HyperLogLog operations that write.
///
/// One class for both families: they carry the same three-way write mode, and
/// differ only in their third flag — `partial` for bits, `allowFold` for
/// sketches. Setting the one that does not apply to the family you are using is
/// simply ignored by that family, which is the one place this collapses two of
/// the client's types into one.
#[php_class]
#[php(name = "Aerospike\\BinPolicy")]
#[derive(Debug, Clone, Copy, Default)]
pub struct BinPolicy {
    write_mode: Option<BinWriteMode>,
    no_fail: bool,
    partial: bool,
    allow_fold: bool,
}

#[php_impl]
impl BinPolicy {
    /// Build one. Everything is optional; the default creates or overwrites.
    #[php(defaults(writeMode = None, noFail = false, partial = false, allowFold = false))]
    pub fn __construct(
        writeMode: Option<Given<BinWriteMode>>,
        noFail: bool,
        partial: bool,
        allowFold: bool,
    ) -> PhpResult<BinPolicy> {
        Ok(BinPolicy {
            write_mode: Given::or_none(writeMode, "writeMode")?,
            no_fail: noFail,
            partial,
            allow_fold: allowFold,
        })
    }

    /// Whether a rejected write leaves the operation successful.
    pub fn no_fail(&self) -> bool {
        self.no_fail
    }

    /// Whether other operations may commit when this one is rejected. Bitwise
    /// only.
    pub fn partial(&self) -> bool {
        self.partial
    }

    /// Whether sketches of different precision may be combined by folding down
    /// to the smaller. HyperLogLog only.
    pub fn allow_fold(&self) -> bool {
        self.allow_fold
    }
}

impl BinPolicy {
    fn mode(self) -> BinWriteMode {
        self.write_mode.unwrap_or(BinWriteMode::Update)
    }

    /// The contract's bitwise policy, or the default when PHP passed none.
    pub(crate) fn bit_wire(policy: Option<&BinPolicy>) -> WireBitPolicy {
        policy.map_or_else(WireBitPolicy::default, |policy| WireBitPolicy {
            write_mode: policy.mode().to_bit_wire(),
            no_fail: policy.no_fail,
            partial: policy.partial,
        })
    }

    /// The contract's HyperLogLog policy, likewise.
    pub(crate) fn hll_wire(policy: Option<&BinPolicy>) -> WireHllPolicy {
        policy.map_or_else(WireHllPolicy::default, |policy| WireHllPolicy {
            write_mode: policy.mode().to_hll_wire(),
            no_fail: policy.no_fail,
            allow_fold: policy.allow_fold,
        })
    }
}

/// The bitwise operations: `aerospike-core`'s `operations::bitwise`, method for
/// method.
///
/// They work on a **blob** bin — `Aerospike\Blob`, not a string — and treat it
/// as a flat field of bits.
///
/// Watch the units. `resize`, `insert` and `remove` work in whole **bytes**;
/// everything else works in **bits**. The parameter names say which
/// (`$byteOffset` against `$bitOffset`), because that is the mistake this API
/// can least afford: a byte offset used as a bit offset addresses the right
/// blob at the wrong place and succeeds.
#[php_class]
#[php(name = "Aerospike\\BitOp")]
#[derive(Debug, Clone, Copy)]
pub struct BitOp;

#[php_impl]
impl BitOp {
    /// Grow or shrink the blob to `$byteSize` bytes.
    #[php(defaults(flags = None, policy = None))]
    pub fn resize(
        bin: String,
        byteSize: i64,
        flags: Option<Given<BitResize>>,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::Resize {
                byte_size: byteSize,
                flags: Given::or_none(flags, "flags")?.map(BitResize::to_wire),
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Insert bytes at `$byteOffset`.
    #[php(defaults(policy = None))]
    pub fn insert(
        bin: String,
        byteOffset: i64,
        value: &Zval,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::Insert {
                byte_offset: byteOffset,
                value: wire_value(value, "value")?,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Remove `$byteSize` bytes at `$byteOffset`.
    #[php(defaults(policy = None))]
    pub fn remove(
        bin: String,
        byteOffset: i64,
        byteSize: i64,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::Remove {
                byte_offset: byteOffset,
                byte_size: byteSize,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Overwrite `$bitSize` bits at `$bitOffset`.
    #[php(defaults(policy = None))]
    pub fn set(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        value: &Zval,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::Set {
                bit_offset: bitOffset,
                bit_size: bitSize,
                value: wire_value(value, "value")?,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Bitwise OR into `$bitSize` bits at `$bitOffset`.
    #[php(defaults(policy = None))]
    pub fn or(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        value: &Zval,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::Or {
                bit_offset: bitOffset,
                bit_size: bitSize,
                value: wire_value(value, "value")?,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Bitwise XOR.
    #[php(defaults(policy = None))]
    pub fn xor(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        value: &Zval,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::Xor {
                bit_offset: bitOffset,
                bit_size: bitSize,
                value: wire_value(value, "value")?,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Bitwise AND.
    #[php(defaults(policy = None))]
    pub fn and(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        value: &Zval,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::And {
                bit_offset: bitOffset,
                bit_size: bitSize,
                value: wire_value(value, "value")?,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Invert `$bitSize` bits at `$bitOffset`.
    #[php(defaults(policy = None))]
    pub fn not(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::Not {
                bit_offset: bitOffset,
                bit_size: bitSize,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Shift a bit field left.
    #[php(defaults(policy = None))]
    pub fn lshift(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        shift: i64,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::LeftShift {
                bit_offset: bitOffset,
                bit_size: bitSize,
                shift,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Shift a bit field right.
    #[php(defaults(policy = None))]
    pub fn rshift(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        shift: i64,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::RightShift {
                bit_offset: bitOffset,
                bit_size: bitSize,
                shift,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Add to the integer held in a bit field.
    ///
    /// `$overflow` defaults to `BitOverflow::Fail`: a counter that silently
    /// wrapped or stuck is worse than one that says it could not.
    #[php(defaults(signed = false, overflow = None, policy = None))]
    pub fn add(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        value: i64,
        signed: bool,
        overflow: Option<Given<BitOverflow>>,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::Add {
                bit_offset: bitOffset,
                bit_size: bitSize,
                value,
                signed,
                overflow: overflows(overflow)?,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Subtract from the integer held in a bit field.
    #[php(defaults(signed = false, overflow = None, policy = None))]
    pub fn subtract(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        value: i64,
        signed: bool,
        overflow: Option<Given<BitOverflow>>,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::Subtract {
                bit_offset: bitOffset,
                bit_size: bitSize,
                value,
                signed,
                overflow: overflows(overflow)?,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Write an integer into a bit field.
    #[php(defaults(policy = None))]
    pub fn set_int(
        bin: String,
        bitOffset: i64,
        bitSize: i64,
        value: i64,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::bit(
            bin,
            WireBitOp::SetInt {
                bit_offset: bitOffset,
                bit_size: bitSize,
                value,
                policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Read `$bitSize` bits at `$bitOffset` as a blob.
    pub fn get(bin: String, bitOffset: i64, bitSize: i64) -> Operation {
        Operation::bit(
            bin,
            WireBitOp::Get {
                bit_offset: bitOffset,
                bit_size: bitSize,
            },
        )
    }

    /// Count the set bits in a range.
    pub fn count(bin: String, bitOffset: i64, bitSize: i64) -> Operation {
        Operation::bit(
            bin,
            WireBitOp::Count {
                bit_offset: bitOffset,
                bit_size: bitSize,
            },
        )
    }

    /// The offset of the first bit equal to `$value`, searching forwards; `-1`
    /// if there is none.
    #[php(defaults(value = true))]
    pub fn lscan(bin: String, bitOffset: i64, bitSize: i64, value: bool) -> Operation {
        Operation::bit(
            bin,
            WireBitOp::LeftScan {
                bit_offset: bitOffset,
                bit_size: bitSize,
                value,
            },
        )
    }

    /// The same, searching backwards.
    #[php(defaults(value = true))]
    pub fn rscan(bin: String, bitOffset: i64, bitSize: i64, value: bool) -> Operation {
        Operation::bit(
            bin,
            WireBitOp::RightScan {
                bit_offset: bitOffset,
                bit_size: bitSize,
                value,
            },
        )
    }

    /// Read a bit field as an integer.
    #[php(defaults(signed = false))]
    pub fn get_int(bin: String, bitOffset: i64, bitSize: i64, signed: bool) -> Operation {
        Operation::bit(
            bin,
            WireBitOp::GetInt {
                bit_offset: bitOffset,
                bit_size: bitSize,
                signed,
            },
        )
    }
}

/// The HyperLogLog operations: `aerospike-core`'s `operations::hll`, method for
/// method.
///
/// A HyperLogLog sketch answers "roughly how many *distinct* things have I
/// seen" in a fixed few kilobytes, however many things there were. **Every
/// count it gives back is an estimate** — that is the trade the structure
/// makes, and `$indexBitCount` is what buys accuracy with space.
///
/// ```php
/// $client->operate(null, $key, [
///     HllOp::add('visitors', [$userId], indexBitCount: 12),
///     HllOp::getCount('visitors'),
/// ]);
/// ```
///
/// The bit counts are nullable wherever the Rust client has a family of
/// constructors for them, and `null` means "leave it to the sketch that is
/// already there".
#[php_class]
#[php(name = "Aerospike\\HllOp")]
#[derive(Debug, Clone, Copy)]
pub struct HllOp;

#[php_impl]
impl HllOp {
    /// Create a sketch, or reset the one that is there.
    #[php(defaults(minHashBitCount = None, policy = None))]
    pub fn init(
        bin: String,
        indexBitCount: i64,
        minHashBitCount: Option<Given<i64>>,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::hll(
            bin,
            WireHllOp::Init {
                index_bit_count: indexBitCount,
                min_hash_bit_count: Given::or_none(minHashBitCount, "minHashBitCount")?,
                policy: BinPolicy::hll_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Add values to the sketch, creating it if it is missing.
    ///
    /// Returns how many of them caused the sketch to change — not how many were
    /// new, which a sketch cannot know.
    #[php(defaults(indexBitCount = None, minHashBitCount = None, policy = None))]
    pub fn add(
        bin: String,
        values: Vec<&Zval>,
        indexBitCount: Option<Given<i64>>,
        minHashBitCount: Option<Given<i64>>,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::hll(
            bin,
            WireHllOp::Add {
                values: wire_values(&values)?,
                index_bit_count: Given::or_none(indexBitCount, "indexBitCount")?,
                min_hash_bit_count: Given::or_none(minHashBitCount, "minHashBitCount")?,
                policy: BinPolicy::hll_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Merge other sketches into this one.
    #[php(defaults(policy = None))]
    pub fn set_union(
        bin: String,
        sketches: Vec<&Zval>,
        policy: Option<Given<&BinPolicy>>,
    ) -> PhpResult<Operation> {
        Ok(Operation::hll(
            bin,
            WireHllOp::SetUnion {
                sketches: wire_values(&sketches)?,
                policy: BinPolicy::hll_wire(Given::or_none(policy, "policy")?),
            },
        ))
    }

    /// Recompute and cache the sketch's count.
    pub fn refresh_count(bin: String) -> Operation {
        Operation::hll(bin, WireHllOp::RefreshCount)
    }

    /// Reduce the sketch's precision to `$indexBitCount`.
    pub fn fold(bin: String, indexBitCount: i64) -> Operation {
        Operation::hll(
            bin,
            WireHllOp::Fold {
                index_bit_count: indexBitCount,
            },
        )
    }

    /// The estimated number of distinct values.
    pub fn get_count(bin: String) -> Operation {
        Operation::hll(bin, WireHllOp::GetCount)
    }

    /// The union of this sketch and the given ones, as a sketch.
    pub fn get_union(bin: String, sketches: Vec<&Zval>) -> PhpResult<Operation> {
        Ok(Operation::hll(
            bin,
            WireHllOp::GetUnion {
                sketches: wire_values(&sketches)?,
            },
        ))
    }

    /// The estimated size of that union.
    pub fn get_union_count(bin: String, sketches: Vec<&Zval>) -> PhpResult<Operation> {
        Ok(Operation::hll(
            bin,
            WireHllOp::GetUnionCount {
                sketches: wire_values(&sketches)?,
            },
        ))
    }

    /// The estimated size of the intersection.
    pub fn get_intersect_count(bin: String, sketches: Vec<&Zval>) -> PhpResult<Operation> {
        Ok(Operation::hll(
            bin,
            WireHllOp::GetIntersectCount {
                sketches: wire_values(&sketches)?,
            },
        ))
    }

    /// The estimated Jaccard similarity, from 0.0 to 1.0.
    pub fn get_similarity(bin: String, sketches: Vec<&Zval>) -> PhpResult<Operation> {
        Ok(Operation::hll(
            bin,
            WireHllOp::GetSimilarity {
                sketches: wire_values(&sketches)?,
            },
        ))
    }

    /// The sketch's index-bit and MinHash-bit counts, as a two-element list.
    pub fn describe(bin: String) -> Operation {
        Operation::hll(bin, WireHllOp::Describe)
    }
}

/// An expression to evaluate against a record.
///
/// Two ways to say one, both text:
///
/// ```php
/// Aerospike\Expression::ael('$.first + $.second');   // the server compiles it
/// Aerospike\Expression::base64($packedFromAnotherClient);
/// ```
///
/// Three ways to get one, and they are interchangeable wherever an expression is
/// used — a policy filter, an expression operation, an expression-based index:
///
/// | how | needs | notes |
/// | --- | --- | --- |
/// | `Aerospike\\Exp::*` | nothing | composed here, packed by the client |
/// | `Expression::ael($text)` | server 8.1.3+ | shortest to write; the server parses it |
/// | `Expression::base64($packed)` | nothing | one another client packed |
///
/// **Prefer the builder** unless the text form is clearly more readable for what
/// you are writing: it works on every server this client supports, and a wrong
/// operand is a `TypeError` at the call site rather than a server error a round
/// trip later. See `Aerospike\\Exp`.
///
/// Only a built expression can be an operand of another. The other two are
/// already whole expressions — there is nowhere in the wire format to put text
/// inside a packed tree.
#[php_class]
#[php(name = "Aerospike\\Expression")]
#[derive(Debug, Clone)]
pub struct Expression {
    expression: WireExpression,
}

#[php_impl]
impl Expression {
    /// An expression written in the Aerospike Expression Language.
    pub fn ael(source: String) -> PhpResult<Expression> {
        if source.trim().is_empty() {
            return Err(AeroError::client(
                "an expression needs a source; \"\" is not one",
            )
            .into());
        }
        Ok(Expression {
            expression: WireExpression::Ael(source),
        })
    }

    /// An expression another Aerospike client packed, base64-encoded.
    ///
    /// This is the packed *form*, not the source text — passing source here
    /// reaches the daemon as base64 that does not decode, and it says so.
    pub fn base64(packed: String) -> PhpResult<Expression> {
        if packed.trim().is_empty() {
            return Err(AeroError::client(
                "a packed expression needs bytes; \"\" is not any",
            )
            .into());
        }
        Ok(Expression {
            expression: WireExpression::Base64(packed),
        })
    }

    /// Whether this is Aerospike Expression Language source.
    pub fn is_ael(&self) -> bool {
        matches!(self.expression, WireExpression::Ael(_))
    }

    /// Whether this was composed by `Aerospike\\Exp`'s builder.
    ///
    /// The three forms are interchangeable wherever an expression is *used*, so
    /// this is not something a caller normally has to ask. It matters in one
    /// place: only a built expression can be an **operand** of another, because
    /// the other two forms are already whole expressions.
    pub fn is_built(&self) -> bool {
        matches!(self.expression, WireExpression::Tree(_))
    }

    /// Attach a path into a nested collection.
    ///
    /// Only meaningful on a list or map expression — those are the ones that take
    /// a context — so anything else is refused by name rather than accepting a
    /// path that would be dropped. The same spelling as `Operation::context()`, so
    /// a caller learns one convention for both.
    ///
    /// ```php
    /// use Aerospike\{Exp, ExpList, Ctx, ListReturn};
    ///
    /// // The size of the list at attrs["history"].
    /// ExpList::size(Exp::mapBin('attrs'))->context([Ctx::mapKey('history')]);
    /// ```
    pub fn context(&self, ctx: Vec<&Zval>) -> PhpResult<Expression> {
        let path = context_list(&ctx)?;
        let tree = match &self.expression {
            WireExpression::Tree(tree) => tree.clone(),
            _ => {
                return Err(AeroError::client(
                    "context() applies to a list or map expression built with Aerospike\\ExpList \
                     or Aerospike\\ExpMap. An expression written as text carries its own path in \
                     the text, and a packed one already has whatever path it was built with",
                )
                .into())
            }
        };
        let tree = match tree {
            aerospike_php_ipc::exp::WireExp::List(mut list) => {
                list.ctx = path;
                aerospike_php_ipc::exp::WireExp::List(list)
            }
            aerospike_php_ipc::exp::WireExp::Map(mut map) => {
                map.ctx = path;
                aerospike_php_ipc::exp::WireExp::Map(map)
            }
            other => {
                return Err(AeroError::client(format!(
                    "context() applies to a list or map expression, and this is not one: {}. Put \
                     the path on the ExpList or ExpMap call that reads the nested collection",
                    describe_tree(&other)
                ))
                .into())
            }
        };
        Ok(Expression::from_tree(tree))
    }

    /// The text this was built from, or `""` for a built expression.
    ///
    /// A tree has no source text — it was never text — and inventing one would
    /// mean writing an Aerospike Expression Language serialiser whose output
    /// nothing reads.
    pub fn source(&self) -> String {
        match &self.expression {
            WireExpression::Ael(source) | WireExpression::Base64(source) => source.clone(),
            WireExpression::Tree(_) => String::new(),
        }
    }

    // ===== the 1.x client's expression builder ==============================
    //
    // The previous PHP client put its whole expression builder on this class as
    // static methods, and the signatures happen to match `Aerospike\Exp`'s
    // exactly — same names, same argument order, arrays for the variadic ones. So
    // these are one-line forwarders, and they are what make an old script's
    // expressions run unchanged.
    //
    // The builder itself lives on `Exp`, which is where new code should look: it
    // has these plus the list, map, bitwise, HLL, string and path families that
    // the 1.x client had no equivalent for. Deliberately absent is the 1.x
    // `Expression::new(...)`, the raw node constructor — it took the client's
    // internal opcode numbering, which is not a thing this contract exposes.
    /// The 1.x spelling of [`Exp::key_exists`].
    pub fn key_exists() -> Expression {
        Exp::key_exists()
    }

    /// The 1.x spelling of [`Exp::set_name`].
    pub fn set_name() -> Expression {
        Exp::set_name()
    }

    /// The 1.x spelling of [`Exp::device_size`].
    pub fn device_size() -> Expression {
        Exp::device_size()
    }

    /// The 1.x spelling of [`Exp::memory_size`].
    pub fn memory_size() -> Expression {
        Exp::memory_size()
    }

    /// The 1.x spelling of [`Exp::last_update`].
    pub fn last_update() -> Expression {
        Exp::last_update()
    }

    /// The 1.x spelling of [`Exp::since_update`].
    pub fn since_update() -> Expression {
        Exp::since_update()
    }

    /// The 1.x spelling of [`Exp::void_time`].
    pub fn void_time() -> Expression {
        Exp::void_time()
    }

    /// The 1.x spelling of [`Exp::ttl`].
    pub fn ttl() -> Expression {
        Exp::ttl()
    }

    /// The 1.x spelling of [`Exp::is_tombstone`].
    pub fn is_tombstone() -> Expression {
        Exp::is_tombstone()
    }

    /// The 1.x spelling of [`Exp::nil`].
    pub fn nil() -> Expression {
        Exp::nil()
    }

    /// The 1.x spelling of [`Exp::infinity`].
    pub fn infinity() -> Expression {
        Exp::infinity()
    }

    /// The 1.x spelling of [`Exp::wildcard`].
    pub fn wildcard() -> Expression {
        Exp::wildcard()
    }

    /// The 1.x spelling of [`Exp::unknown`].
    pub fn unknown() -> Expression {
        Exp::unknown()
    }

    /// The 1.x spelling of [`Exp::int_bin`].
    pub fn int_bin(name: String) -> Expression {
        Exp::int_bin(name)
    }

    /// The 1.x spelling of [`Exp::string_bin`].
    pub fn string_bin(name: String) -> Expression {
        Exp::string_bin(name)
    }

    /// The 1.x spelling of [`Exp::blob_bin`].
    pub fn blob_bin(name: String) -> Expression {
        Exp::blob_bin(name)
    }

    /// The 1.x spelling of [`Exp::float_bin`].
    pub fn float_bin(name: String) -> Expression {
        Exp::float_bin(name)
    }

    /// The 1.x spelling of [`Exp::geo_bin`].
    pub fn geo_bin(name: String) -> Expression {
        Exp::geo_bin(name)
    }

    /// The 1.x spelling of [`Exp::list_bin`].
    pub fn list_bin(name: String) -> Expression {
        Exp::list_bin(name)
    }

    /// The 1.x spelling of [`Exp::map_bin`].
    pub fn map_bin(name: String) -> Expression {
        Exp::map_bin(name)
    }

    /// The 1.x spelling of [`Exp::hll_bin`].
    pub fn hll_bin(name: String) -> Expression {
        Exp::hll_bin(name)
    }

    /// The 1.x spelling of [`Exp::bin_exists`].
    pub fn bin_exists(name: String) -> Expression {
        Exp::bin_exists(name)
    }

    /// The 1.x spelling of [`Exp::bin_type`].
    pub fn bin_type(name: String) -> Expression {
        Exp::bin_type(name)
    }

    /// The 1.x spelling of [`Exp::not`].
    pub fn not(exp: &Expression) -> PhpResult<Expression> {
        Exp::not(exp)
    }

    /// The 1.x spelling of [`Exp::num_abs`].
    pub fn num_abs(value: &Expression) -> PhpResult<Expression> {
        Exp::num_abs(value)
    }

    /// The 1.x spelling of [`Exp::num_floor`].
    pub fn num_floor(num: &Expression) -> PhpResult<Expression> {
        Exp::num_floor(num)
    }

    /// The 1.x spelling of [`Exp::num_ceil`].
    pub fn num_ceil(num: &Expression) -> PhpResult<Expression> {
        Exp::num_ceil(num)
    }

    /// The 1.x spelling of [`Exp::to_int`].
    pub fn to_int(num: &Expression) -> PhpResult<Expression> {
        Exp::to_int(num)
    }

    /// The 1.x spelling of [`Exp::to_float`].
    pub fn to_float(num: &Expression) -> PhpResult<Expression> {
        Exp::to_float(num)
    }

    /// The 1.x spelling of [`Exp::int_not`].
    pub fn int_not(exp: &Expression) -> PhpResult<Expression> {
        Exp::int_not(exp)
    }

    /// The 1.x spelling of [`Exp::int_count`].
    pub fn int_count(exp: &Expression) -> PhpResult<Expression> {
        Exp::int_count(exp)
    }

    /// The 1.x spelling of [`Exp::geo_compare`].
    pub fn geo_compare(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Exp::geo_compare(left, right)
    }

    /// The 1.x spelling of [`Exp::eq`].
    pub fn eq(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Exp::eq(left, right)
    }

    /// The 1.x spelling of [`Exp::ne`].
    pub fn ne(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Exp::ne(left, right)
    }

    /// The 1.x spelling of [`Exp::gt`].
    pub fn gt(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Exp::gt(left, right)
    }

    /// The 1.x spelling of [`Exp::ge`].
    pub fn ge(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Exp::ge(left, right)
    }

    /// The 1.x spelling of [`Exp::lt`].
    pub fn lt(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Exp::lt(left, right)
    }

    /// The 1.x spelling of [`Exp::le`].
    pub fn le(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Exp::le(left, right)
    }

    /// The 1.x spelling of [`Exp::num_pow`].
    pub fn num_pow(base: &Expression, exponent: &Expression) -> PhpResult<Expression> {
        Exp::num_pow(base, exponent)
    }

    /// The 1.x spelling of [`Exp::num_log`].
    pub fn num_log(num: &Expression, base: &Expression) -> PhpResult<Expression> {
        Exp::num_log(num, base)
    }

    /// The 1.x spelling of [`Exp::num_mod`].
    pub fn num_mod(numerator: &Expression, denominator: &Expression) -> PhpResult<Expression> {
        Exp::num_mod(numerator, denominator)
    }

    /// The 1.x spelling of [`Exp::int_lshift`].
    pub fn int_lshift(value: &Expression, shift: &Expression) -> PhpResult<Expression> {
        Exp::int_lshift(value, shift)
    }

    /// The 1.x spelling of [`Exp::int_rshift`].
    pub fn int_rshift(value: &Expression, shift: &Expression) -> PhpResult<Expression> {
        Exp::int_rshift(value, shift)
    }

    /// The 1.x spelling of [`Exp::int_arshift`].
    pub fn int_arshift(value: &Expression, shift: &Expression) -> PhpResult<Expression> {
        Exp::int_arshift(value, shift)
    }

    /// The 1.x spelling of [`Exp::int_lscan`].
    pub fn int_lscan(value: &Expression, search: &Expression) -> PhpResult<Expression> {
        Exp::int_lscan(value, search)
    }

    /// The 1.x spelling of [`Exp::int_rscan`].
    pub fn int_rscan(value: &Expression, search: &Expression) -> PhpResult<Expression> {
        Exp::int_rscan(value, search)
    }

    /// The 1.x spelling of [`Exp::and`].
    pub fn and(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::and(exps)
    }

    /// The 1.x spelling of [`Exp::or`].
    pub fn or(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::or(exps)
    }

    /// The 1.x spelling of [`Exp::xor`].
    pub fn xor(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::xor(exps)
    }

    /// The 1.x spelling of [`Exp::num_add`].
    pub fn num_add(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::num_add(exps)
    }

    /// The 1.x spelling of [`Exp::num_sub`].
    pub fn num_sub(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::num_sub(exps)
    }

    /// The 1.x spelling of [`Exp::num_mul`].
    pub fn num_mul(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::num_mul(exps)
    }

    /// The 1.x spelling of [`Exp::num_div`].
    pub fn num_div(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::num_div(exps)
    }

    /// The 1.x spelling of [`Exp::int_and`].
    pub fn int_and(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::int_and(exps)
    }

    /// The 1.x spelling of [`Exp::int_or`].
    pub fn int_or(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::int_or(exps)
    }

    /// The 1.x spelling of [`Exp::int_xor`].
    pub fn int_xor(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::int_xor(exps)
    }

    /// The 1.x spelling of [`Exp::min`].
    pub fn min(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::min(exps)
    }

    /// The 1.x spelling of [`Exp::max`].
    pub fn max(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::max(exps)
    }

    /// The 1.x spelling of [`Exp::cond`].
    pub fn cond(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::cond(exps)
    }

    /// The 1.x spelling of [`Exp::let_`]. The 1.x name for `let()`, which is what this client calls it.
    pub fn exp_let(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::let_(exps)
    }

    /// The 1.x spelling of [`Exp::key`].
    pub fn key(expType: ExpType) -> Expression {
        Exp::key(expType)
    }

    /// The 1.x spelling of [`Exp::digest_modulo`].
    pub fn digest_modulo(modulo: i64) -> PhpResult<Expression> {
        Exp::digest_modulo(modulo)
    }

    /// The 1.x spelling of [`Exp::regex_compare`].
    pub fn regex_compare(regex: String, flags: i64, bin: &Expression) -> PhpResult<Expression> {
        Exp::regex_compare(regex, flags, bin)
    }

    /// The 1.x spelling of [`Exp::int_val`].
    pub fn int_val(value: i64) -> Expression {
        Exp::int_val(value)
    }

    /// The 1.x spelling of [`Exp::bool_val`].
    pub fn bool_val(value: bool) -> Expression {
        Exp::bool_val(value)
    }

    /// The 1.x spelling of [`Exp::string_val`].
    pub fn string_val(value: String) -> Expression {
        Exp::string_val(value)
    }

    /// The 1.x spelling of [`Exp::float_val`].
    pub fn float_val(value: f64) -> Expression {
        Exp::float_val(value)
    }

    /// The 1.x spelling of [`Exp::blob_val`].
    pub fn blob_val(value: Vec<u8>) -> Expression {
        Exp::blob_val(value)
    }

    /// The 1.x spelling of [`Exp::geo_val`].
    pub fn geo_val(value: String) -> Expression {
        Exp::geo_val(value)
    }

    /// The 1.x spelling of [`Exp::list_val`].
    pub fn list_val(value: &Zval) -> PhpResult<Expression> {
        Exp::list_val(value)
    }

    /// The 1.x spelling of [`Exp::map_val`].
    pub fn map_val(value: &Zval) -> PhpResult<Expression> {
        Exp::map_val(value)
    }

    /// The 1.x spelling of [`Exp::def`].
    pub fn def(name: String, value: &Expression) -> PhpResult<Expression> {
        Exp::def(name, value)
    }

    /// The 1.x spelling of [`Exp::var`].
    pub fn var(name: String) -> PhpResult<Expression> {
        Exp::var(name)
    }
}

impl Expression {
    /// The contract's expression.
    #[must_use]
    pub fn to_wire(&self) -> WireExpression {
        self.expression.clone()
    }

    /// An expression from whatever form the contract carries.
    ///
    /// For handing a policy's filter back to PHP: the policy stored a
    /// `WireExpression`, and this is how it becomes the class again.
    #[must_use]
    pub(crate) fn from_wire(expression: WireExpression) -> Expression {
        Expression { expression }
    }

    /// An expression built from a tree by [`crate::exp::Exp`].
    ///
    /// `pub(crate)` and not a PHP method: a tree is composed by the builder's
    /// static methods, and a constructor taking one from PHP would have to accept
    /// the tree in some hand-written form — which is the text API that already
    /// exists.
    #[must_use]
    pub(crate) fn from_tree(tree: aerospike_php_ipc::exp::WireExp) -> Expression {
        Expression {
            expression: WireExpression::Tree(tree),
        }
    }
}

/// The expression operations: `aerospike-core`'s `operations::exp`.
///
/// An expression is evaluated against the record by the **server**, so it sees
/// the record as it is at that moment — inside the same atomic `operate()` as
/// everything else in the call.
///
/// ```php
/// $client->operate(null, $key, [
///     // Compute a bin from two others, and store it
///     ExpOp::write('total', Expression::ael('$.price * $.quantity')),
///     // Compute something and just read it back, storing nothing
///     ExpOp::read('discounted', Expression::ael('$.total * 0.9')),
/// ]);
/// ```
#[php_class]
#[php(name = "Aerospike\\ExpOp")]
#[derive(Debug, Clone, Copy)]
pub struct ExpOp;

#[php_impl]
impl ExpOp {
    /// Evaluate, and return the result labelled `$name`.
    ///
    /// `$name` is **not a bin**: nothing is written, and it is only what the
    /// answer comes back under — so it can be anything that reads well.
    ///
    /// `$evalNoFail` swallows the failure when the expression resolves to
    /// nothing usable, leaving the operation out of the result instead.
    #[php(defaults(evalNoFail = false))]
    pub fn read(name: String, expression: &Expression, evalNoFail: bool) -> Operation {
        Operation::exp(WireExpOp::Read {
            name,
            expression: expression.to_wire(),
            flags: WireExpReadFlags {
                eval_no_fail: evalNoFail,
            },
        })
    }

    /// Evaluate, and write the result into `$bin`.
    ///
    /// `$allowDelete` makes an expression that evaluates to null **delete** the
    /// bin; without it, that is reported as an operation that did not apply.
    #[php(defaults(writeMode = None, allowDelete = false, noFail = false, evalNoFail = false))]
    pub fn write(
        bin: String,
        expression: &Expression,
        writeMode: Option<Given<BinWriteMode>>,
        allowDelete: bool,
        noFail: bool,
        evalNoFail: bool,
    ) -> PhpResult<Operation> {
        let write_mode = match Given::or_none(writeMode, "writeMode")?.unwrap_or(BinWriteMode::Update)
        {
            BinWriteMode::Update => WireExpWriteMode::Update,
            BinWriteMode::UpdateOnly => WireExpWriteMode::UpdateOnly,
            BinWriteMode::CreateOnly => WireExpWriteMode::CreateOnly,
        };
        Ok(Operation::exp(WireExpOp::Write {
            bin,
            expression: expression.to_wire(),
            flags: WireExpWriteFlags {
                write_mode,
                allow_delete: allowDelete,
                no_fail: noFail,
                eval_no_fail: evalNoFail,
            },
        }))
    }
}

/// The overflow action an arithmetic bit operation was given.
///
/// Defaults to `Fail`, which is the client's default too: a number that
/// silently became a different number is worse than an error.
fn overflows(overflow: Option<Given<BitOverflow>>) -> PhpResult<WireBitOverflow> {
    Ok(Given::or_none(overflow, "overflow")?
        .unwrap_or(BitOverflow::Fail)
        .to_wire())
}

/// The map return type an operation was given, defaulting to the values.
///
/// `Value` for the same reason a list operation defaults to its values: a result
/// that vanished silently is the more surprising outcome.
fn map_returns(
    return_type: Option<Given<MapReturn>>,
    inverted: bool,
) -> PhpResult<WireMapReturn> {
    let kind = Given::or_none(return_type, "returnType")?.unwrap_or(MapReturn::Value);
    Ok(kind.to_wire(inverted))
}

/// The return type an operation was given, defaulting to the values.
///
/// `Values` rather than `None`: an operation whose result was silently dropped
/// is the more surprising of the two, and a caller who wants nothing back can
/// say `ListReturn::None` and mean it.
fn returns(
    return_type: Option<Given<ListReturn>>,
    inverted: bool,
) -> PhpResult<aerospike_php_ipc::op::WireListReturn> {
    let kind = Given::or_none(return_type, "returnType")?.unwrap_or(ListReturn::Values);
    Ok(kind.to_wire(inverted))
}

/// One PHP value as an operation argument.
fn wire_value(zval: &Zval, argument: &str) -> AeroResult<WireValue> {
    value::zval_to_wire(zval, &Path::bin(argument))
}

/// A name for an expression node, for `context()`'s refusal.
///
/// Only has to be good enough to tell the caller which part of their expression
/// the path landed on, so it names the shape rather than reconstructing the call.
fn describe_tree(tree: &aerospike_php_ipc::exp::WireExp) -> &'static str {
    use aerospike_php_ipc::exp::WireExp;
    match tree {
        WireExp::Value(_) => "a literal",
        WireExp::Bin { .. } => "a bin read",
        WireExp::BinExists(_) => "a binExists()",
        WireExp::BinType(_) => "a binType()",
        WireExp::Key(_) => "a key read",
        WireExp::Meta(_) => "a record-metadata read",
        WireExp::DigestModulo(_) => "a digestModulo()",
        WireExp::Unary { .. } | WireExp::Binary { .. } | WireExp::Variadic { .. } => {
            "an operator"
        }
        WireExp::Regex { .. } => "a regexCompare()",
        WireExp::Def { .. } => "a def()",
        WireExp::Var(_) => "a var()",
        WireExp::Unknown => "an unknown()",
        WireExp::List(_) | WireExp::Map(_) => "a collection expression",
        WireExp::Bit(_) => "a bitwise expression",
        WireExp::Hll(_) => "a HyperLogLog expression",
        WireExp::Str(_) => "a string expression",
        WireExp::Path(_) => "a path expression, which takes its path as an argument",
        WireExp::LoopVar { .. } => "a loop variable",
        WireExp::RemoveResult => "a removeResult()",
    }
}

/// A list of PHP values as operation arguments.
fn wire_values(values: &[&Zval]) -> AeroResult<Vec<WireValue>> {
    values
        .iter()
        .enumerate()
        .map(|(index, zval)| value::zval_to_wire(zval, &Path::bin(&format!("values[{index}]"))))
        .collect()
}

/// An optional bound of a value range.
fn optional_value(zval: Option<&Zval>, argument: &str) -> AeroResult<Option<WireValue>> {
    match zval {
        // An explicit null and an omitted argument both mean "unbounded"; a
        // range open at one end is the ordinary case, not an error.
        None => Ok(None),
        Some(zval) if zval.is_null() => Ok(None),
        Some(zval) => Ok(Some(wire_value(zval, argument)?)),
    }
}

/// Read a `Ctx[]` argument — a path into a nested collection — checking every
/// element.
///
/// Three places want this: an operation being aimed at a nested collection, a
/// secondary-index filter over one, and an index being *created* over one. One
/// function so the three cannot disagree about what a path is or how a wrong step
/// is reported.
///
/// Checked by position, as with `$bins` and `$ops`: PHP can type the parameter as
/// `array` but not as an array *of* `Ctx`, so this is the only place the element
/// type can be enforced — and the position is what a caller needs in order to find
/// the wrong step.
///
/// # Errors
/// A PHP `TypeError` for an element that is not an `Aerospike\Ctx`, and a client
/// failure for an empty path: a path with no steps names nothing, and is
/// overwhelmingly one that was computed and came out empty.
pub fn context_list(ctx: &[&Zval]) -> PhpResult<Vec<WireCtx>> {
    if ctx.is_empty() {
        return Err(AeroError::client(
            "a context path needs at least one step; omit it rather than passing an empty path",
        )
        .into());
    }
    let mut path = Vec::with_capacity(ctx.len());
    for (index, zval) in ctx.iter().enumerate() {
        let step = zval.extract::<&Ctx>().ok_or_else(|| {
            crate::arg::type_error(format!(
                "$ctx must be a list of Aerospike\\Ctx; item {index} is {}",
                crate::arg::type_of(zval)
            ))
        })?;
        path.push(step.to_wire());
    }
    Ok(path)
}

/// Read an `Operation[]` argument, checking every element.
///
/// As with a list of bins, PHP can type the parameter as `array` but not as an
/// array *of operations*, so this is where that half is enforced — by position,
/// because the position is what a caller needs to find the wrong one.
///
/// # Errors
/// A PHP `TypeError` for an element that is not an `Aerospike\Operation`, and a
/// client failure when the list is empty.
pub fn operation_list(ops: &[&Zval]) -> PhpResult<Vec<WireOp>> {
    if ops.is_empty() {
        return Err(AeroError::client(
            "operate() needs at least one operation; the server has no answer for a request that \
             asks nothing",
        )
        .into());
    }

    let mut wire = Vec::with_capacity(ops.len());
    for (index, zval) in ops.iter().enumerate() {
        let op = zval.extract::<&Operation>().ok_or_else(|| {
            crate::arg::type_error(format!(
                "$ops must be a list of Aerospike\\Operation; item {index} is {}",
                crate::arg::type_of(zval)
            ))
        })?;
        wire.push(op.to_wire());
    }
    Ok(wire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::op::WireListReturnKind;

    #[test]
    fn a_scalar_operation_names_its_bin_and_says_whether_it_writes() {
        let read = Operation::scalar(WireOp::GetBin { bin: "a".into() });
        assert_eq!(read.op.bin(), Some("a"));
        assert!(!read.op.is_write());

        let write = Operation::scalar(WireOp::Touch);
        assert_eq!(write.op.bin(), None);
        assert!(write.op.is_write());
    }

    /// The default return type is the values, not nothing: an operation whose
    /// result vanished silently is the more surprising outcome.
    #[test]
    fn the_default_return_type_is_the_values() {
        let plain = returns(None, false).unwrap();
        assert_eq!(plain.kind, WireListReturnKind::Values);
        assert!(!plain.inverted);

        let inverted = returns(None, true).unwrap();
        assert!(inverted.inverted);
    }

    /// An omitted bound and an explicit null both mean "unbounded", because a
    /// half-open range is an ordinary thing to ask for.
    #[test]
    fn an_absent_range_bound_is_unbounded() {
        assert_eq!(optional_value(None, "begin").unwrap(), None);
    }

    /// A path only means something on a collection operation; attaching one to
    /// a scalar would run against the bin instead, silently.
    #[test]
    fn a_path_cannot_be_attached_to_a_scalar_operation() {
        let scalar = Operation::scalar(WireOp::Get);
        assert_eq!(scalar.describe(), "a whole-record read");
        assert_eq!(
            Operation::scalar(WireOp::Add {
                bin: "n".into(),
                value: WireValue::Int(1)
            })
            .describe(),
            "a scalar operation on bin \"n\""
        );
    }

    #[test]
    fn a_list_operation_starts_with_no_path() {
        let op = Operation::list("items".into(), WireListOp::Size);
        match op.to_wire() {
            WireOp::List { bin, ctx, .. } => {
                assert_eq!(bin, "items");
                assert!(ctx.is_empty());
            }
            other => panic!("expected a list operation, got {other:?}"),
        }
    }
}
