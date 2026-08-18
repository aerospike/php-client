// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The contract's operations → the client's.
//!
//! Every arm here is a call to a constructor in `aerospike_core::operations`.
//! That is the point of the contract naming operations rather than encoding
//! them: the sub-opcodes and the argument order stay in the client, where they
//! are already tested, and this file only has to get the *names* right — which
//! the compiler checks.
//!
//! The one place a translation is more than a rename is the range pairs. The
//! contract has a single variant with `count: Option<i64>` where the client has
//! two constructors, so `None` picks the `..._from` or unbounded form; the
//! tests below pin each pair.

use std::fmt;

use aerospike_core::operations::bitwise::{
    self, BitPolicy, BitwiseOverflowActions, BitwiseResizeFlags, BitwiseWriteFlags,
};
use aerospike_core::operations::cdt_context::{
    ctx_all_children, ctx_all_children_with_filter,
    ctx_list_index, ctx_list_index_create, ctx_list_rank, ctx_list_value, ctx_map_index,
    ctx_map_key, ctx_map_key_create, ctx_map_rank, ctx_map_value,
};
use aerospike_core::operations::lists::{
    self, InvertedListReturn, ListOrderType, ListPolicy, ListReturnType,
    ListSortFlags, ListWriteFlags, ToListReturnTypeBitmask,
};
use aerospike_core::expressions::{from_base64, pack_ael_server_filter};
use aerospike_core::operations::exp::{
    self, ExpReadFlags, ExpWriteFlags, ToExpReadFlagBitmask, ToExpWriteFlagBitmask,
};
use aerospike_core::operations::hll::{self, HLLPolicy, HLLWriteFlags};
use aerospike_core::operations::maps::{
    self, InvertedMapReturn, MapOrder, MapPolicy, MapReturnType, MapWriteFlags,
    MapWriteMode, ToMapReturnTypeBitmask,
};
use aerospike_core::operations::{scalar, CdtContext, Operation};
use aerospike_core::{Bin, IndexMap, Value};
use aerospike_php_ipc::op::{
    WireBitOp, WireExpOp, WireExpReadFlags, WireExpWriteFlags, WireExpWriteMode, WireExpression, WireBitOverflow, WireBitPolicy, WireBitResize, WireBitWriteMode, WireCtx,
    WireHllOp, WireHllPolicy, WireHllWriteMode, WireListOp, WireListOrder, WireListPolicy, WireListReturn, WireListReturnKind,
    WireListSort, WireMapOp, WireMapOrder, WireMapPolicy, WireMapReturn, WireMapReturnKind,
    WireMapWriteMode, WireOp,
};

use crate::convert::{self, ResultOnlyValue};
use crate::policy::{AelSupport, Capabilities};

/// Why an operation could not be built.
///
/// Three distinct causes, kept apart because they say different things to a
/// caller: one is a value that should never have been sent, one is a cluster too
/// old for what was asked, and one is an expression that does not decode.
#[derive(Debug)]
pub enum OpError {
    /// A value only the server produces was used as an argument.
    Value(ResultOnlyValue),
    /// The cluster cannot do what the operation needs.
    Unsupported(String),
    /// An expression could not be built from what was sent.
    Expression(String),
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpError::Value(inner) => write!(f, "{inner}"),
            OpError::Unsupported(message) | OpError::Expression(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for OpError {}

impl From<ResultOnlyValue> for OpError {
    fn from(inner: ResultOnlyValue) -> OpError {
        OpError::Value(inner)
    }
}

/// Build the client's operations from the contract's, in order.
///
/// `caps` is the cluster's Aerospike Expression Language support, which only the
/// expression operations consult — asked once by the caller rather than per
/// operation, since it is a property of the cluster and not of the request.
///
/// # Errors
/// [`OpError`] for a result-only value used as an argument, an AEL expression
/// against a cluster too old to compile it, or an expression that does not
/// decode.
pub fn to_operations(ops: &[WireOp], caps: &Capabilities) -> Result<Vec<Operation>, OpError> {
    ops.iter().map(|op| to_operation(op, caps)).collect()
}

/// Build one operation.
///
/// # Errors
/// [`OpError`], as [`to_operations`].
pub fn to_operation(op: &WireOp, caps: &Capabilities) -> Result<Operation, OpError> {
    let built = match op {
        // ----- scalar -----
        WireOp::Get => scalar::get(),
        WireOp::GetHeader => scalar::get_header(),
        WireOp::GetBin { bin } => scalar::get_bin(bin),
        // The client's scalar writes take a `&Bin` and clone what they need, so
        // the bin is built for the call and dropped with it.
        WireOp::Put { bin, value } => scalar::put(&bin_of(bin, value)?),
        WireOp::Append { bin, value } => scalar::append(&bin_of(bin, value)?),
        WireOp::Prepend { bin, value } => scalar::prepend(&bin_of(bin, value)?),
        WireOp::Add { bin, value } => scalar::add(&bin_of(bin, value)?),
        WireOp::Touch => scalar::touch(),
        WireOp::Delete => scalar::delete(),

        // ----- list -----
        WireOp::List { bin, ctx, op } => {
            let built = to_list_operation(bin, op)?;
            if ctx.is_empty() {
                built
            } else {
                built.context(to_contexts(ctx, caps)?)
            }
        }

        // ----- map -----
        //
        // Two map operations take the path as a *constructor argument* rather
        // than through `.context(..)`, because the path is part of what they
        // encode: `maps::create` folds the order flag into the last context
        // element, and degrades to `set_order` when the path is empty. Attaching
        // the path afterwards would leave a `set_order` aimed at a nested map,
        // which is a different operation with the same shape.
        WireOp::Map { bin, ctx, op } => match op {
            WireMapOp::Create {
                order,
                persist_index,
            } => {
                if *persist_index {
                    maps::create_with_index(bin, map_order(*order))
                } else {
                    maps::create(bin, map_order(*order), to_contexts(ctx, caps)?)
                }
            }
            WireMapOp::SetPolicy { policy } => {
                maps::set_policy(&map_policy(*policy), bin, to_contexts(ctx, caps)?)
            }
            other => {
                let built = to_map_operation(bin, other)?;
                if ctx.is_empty() {
                    built
                } else {
                    built.context(to_contexts(ctx, caps)?)
                }
            }
        },

        // ----- bitwise -----
        WireOp::Bit { bin, ctx, op } => {
            let built = to_bit_operation(bin, op)?;
            if ctx.is_empty() {
                built
            } else {
                built.context(to_contexts(ctx, caps)?)
            }
        }

        // ----- HyperLogLog -----
        WireOp::Hll { bin, ctx, op } => {
            let built = to_hll_operation(bin, op)?;
            if ctx.is_empty() {
                built
            } else {
                built.context(to_contexts(ctx, caps)?)
            }
        }

        // ----- expressions -----
        WireOp::Exp { op } => to_exp_operation(op, caps)?,
    };
    Ok(built)
}

/// One expression operation.
///
/// The expression itself is text on the wire — AEL source or a base64 blob —
/// and this is where it becomes the client's `Expression`.
fn to_exp_operation(op: &WireExpOp, caps: &Capabilities) -> Result<Operation, OpError> {
    let expression = to_expression(op.expression(), caps)?;
    Ok(match op {
        WireExpOp::Read { name, flags, .. } => {
            exp::read_exp(name, expression, exp_read_flags(*flags))
        }
        WireExpOp::Write { bin, flags, .. } => {
            exp::write_exp(bin, expression, exp_write_flags(*flags))
        }
    })
}

/// The client's expression, in whichever form the contract carries.
///
/// The one door: every command that takes an expression comes through here, so
/// the three forms are available everywhere one is and no command site has to
/// know there are three. Shared with [`crate::query`], whose expression-based
/// secondary indexes are named the same ways.
///
/// # Errors
/// [`OpError::Unsupported`] when AEL is asked for and a node is too old to
/// compile it — the same check, and the same reason, as the policy `filter`:
/// an old server rejects the text somewhere deep inside with nothing useful to
/// say. [`OpError::Expression`] when base64 does not decode, or when a tree is
/// past its size limits or holds a value that cannot be a literal.
pub fn to_expression(
    expression: &WireExpression,
    caps: &Capabilities,
) -> Result<aerospike_core::expressions::Expression, OpError> {
    match expression {
        WireExpression::Ael(source) => to_ael_expression(source, &caps.ael),
        WireExpression::Base64(packed) => from_base64(packed).map_err(|e| {
            OpError::Expression(format!(
                "that base64 expression could not be decoded: {e}. It must be an expression \
                 another Aerospike client packed, not the expression's source text"
            ))
        }),
        // `crate::exp`, not the `exp` imported above — that one is the client's
        // *operations*::exp. This one is the expression-tree translator.
        WireExpression::Tree(tree) => crate::exp::from_tree(tree, caps),
    }
}

/// A finished expression-flag bitmask, for the client's generic slots.
///
/// The same shape as [`Ret`] and [`MapRet`], and for the same reason: the
/// client's parameter is generic so that a caller can name one flag or several,
/// and this side has already combined them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpFlags(i64);

impl ToExpReadFlagBitmask for ExpFlags {
    fn to_bitmask(self) -> i64 {
        self.0
    }
}

impl ToExpWriteFlagBitmask for ExpFlags {
    fn to_bitmask(self) -> i64 {
        self.0
    }
}

/// An expression from Aerospike Expression Language source.
///
/// Shared with the batch rows, whose per-row filters are the same text with the
/// same version requirement — one gate, one message.
///
/// # Errors
/// [`OpError::Unsupported`] when a node is too old to compile it,
/// [`OpError::Expression`] when it cannot be packed.
pub fn to_ael_expression(
    source: &str,
    ael: &AelSupport,
) -> Result<aerospike_core::expressions::Expression, OpError> {
    // The gate lives on `AelSupport` so that the policy filter and an expression
    // operation cannot disagree about it — including about an *unknown* cluster
    // view, which is not proof of support.
    ael.require().map_err(|e| OpError::Unsupported(e.message().to_string()))?;
    pack_ael_server_filter(source)
        .map_err(|e| OpError::Expression(format!("that expression could not be packed: {e}")))
}

/// The client's read-flag bitmask.
const fn exp_read_flags(flags: WireExpReadFlags) -> ExpFlags {
    ExpFlags(if flags.eval_no_fail {
        ExpReadFlags::EvalNoFail as i64
    } else {
        ExpReadFlags::Default as i64
    })
}

/// The client's write-flag bitmask.
fn exp_write_flags(flags: WireExpWriteFlags) -> ExpFlags {
    let mut bits = match flags.write_mode {
        WireExpWriteMode::Update => ExpWriteFlags::Default as i64,
        WireExpWriteMode::CreateOnly => ExpWriteFlags::CreateOnly as i64,
        WireExpWriteMode::UpdateOnly => ExpWriteFlags::UpdateOnly as i64,
    };
    if flags.allow_delete {
        bits |= ExpWriteFlags::AllowDelete as i64;
    }
    if flags.no_fail {
        bits |= ExpWriteFlags::PolicyNoFail as i64;
    }
    if flags.eval_no_fail {
        bits |= ExpWriteFlags::EvalNoFail as i64;
    }
    ExpFlags(bits)
}

/// One bitwise operation.
///
/// Every one of these takes its policy **last**, and the bit/byte split is in
/// the parameter names rather than in the types — so the only way to get one
/// wrong is to name the wrong constructor, which the compiler catches.
fn to_bit_operation(bin: &str, op: &WireBitOp) -> Result<Operation, ResultOnlyValue> {
    let value = |v| convert::to_value(v);

    Ok(match op {
        WireBitOp::Resize {
            byte_size,
            flags,
            policy,
        } => bitwise::resize(bin, *byte_size, flags.map(bit_resize), &bit_policy(*policy)),
        WireBitOp::Insert {
            byte_offset,
            value: v,
            policy,
        } => bitwise::insert(bin, *byte_offset, value(v)?, &bit_policy(*policy)),
        WireBitOp::Remove {
            byte_offset,
            byte_size,
            policy,
        } => bitwise::remove(bin, *byte_offset, *byte_size, &bit_policy(*policy)),

        WireBitOp::Set {
            bit_offset,
            bit_size,
            value: v,
            policy,
        } => bitwise::set(bin, *bit_offset, *bit_size, value(v)?, &bit_policy(*policy)),
        WireBitOp::Or {
            bit_offset,
            bit_size,
            value: v,
            policy,
        } => bitwise::or(bin, *bit_offset, *bit_size, value(v)?, &bit_policy(*policy)),
        WireBitOp::Xor {
            bit_offset,
            bit_size,
            value: v,
            policy,
        } => bitwise::xor(bin, *bit_offset, *bit_size, value(v)?, &bit_policy(*policy)),
        WireBitOp::And {
            bit_offset,
            bit_size,
            value: v,
            policy,
        } => bitwise::and(bin, *bit_offset, *bit_size, value(v)?, &bit_policy(*policy)),
        WireBitOp::Not {
            bit_offset,
            bit_size,
            policy,
        } => bitwise::not(bin, *bit_offset, *bit_size, &bit_policy(*policy)),
        WireBitOp::LeftShift {
            bit_offset,
            bit_size,
            shift,
            policy,
        } => bitwise::lshift(bin, *bit_offset, *bit_size, *shift, &bit_policy(*policy)),
        WireBitOp::RightShift {
            bit_offset,
            bit_size,
            shift,
            policy,
        } => bitwise::rshift(bin, *bit_offset, *bit_size, *shift, &bit_policy(*policy)),
        WireBitOp::Add {
            bit_offset,
            bit_size,
            value: v,
            signed,
            overflow,
            policy,
        } => bitwise::add(
            bin,
            *bit_offset,
            *bit_size,
            *v,
            *signed,
            bit_overflow(*overflow),
            &bit_policy(*policy),
        ),
        WireBitOp::Subtract {
            bit_offset,
            bit_size,
            value: v,
            signed,
            overflow,
            policy,
        } => bitwise::subtract(
            bin,
            *bit_offset,
            *bit_size,
            *v,
            *signed,
            bit_overflow(*overflow),
            &bit_policy(*policy),
        ),
        WireBitOp::SetInt {
            bit_offset,
            bit_size,
            value: v,
            policy,
        } => bitwise::set_int(bin, *bit_offset, *bit_size, *v, &bit_policy(*policy)),

        WireBitOp::Get {
            bit_offset,
            bit_size,
        } => bitwise::get(bin, *bit_offset, *bit_size),
        WireBitOp::Count {
            bit_offset,
            bit_size,
        } => bitwise::count(bin, *bit_offset, *bit_size),
        WireBitOp::LeftScan {
            bit_offset,
            bit_size,
            value: v,
        } => bitwise::lscan(bin, *bit_offset, *bit_size, *v),
        WireBitOp::RightScan {
            bit_offset,
            bit_size,
            value: v,
        } => bitwise::rscan(bin, *bit_offset, *bit_size, *v),
        WireBitOp::GetInt {
            bit_offset,
            bit_size,
            signed,
        } => bitwise::get_int(bin, *bit_offset, *bit_size, *signed),
    })
}

/// One HyperLogLog operation.
///
/// The client spells "leave this alone" as `-1` for both bit counts, and has a
/// family of constructors that fill it in; the contract spells it `None`, and
/// this is where the two meet.
fn to_hll_operation(bin: &str, op: &WireHllOp) -> Result<Operation, ResultOnlyValue> {
    let values = |vs: &Vec<aerospike_php_ipc::WireValue>| {
        vs.iter().map(convert::to_value).collect::<Result<Vec<_>, _>>()
    };

    Ok(match op {
        WireHllOp::Init {
            index_bit_count,
            min_hash_bit_count,
            policy,
        } => hll::init_with_min_hash(
            &hll_policy(*policy),
            bin,
            *index_bit_count,
            unset_as_minus_one(*min_hash_bit_count),
        ),
        WireHllOp::Add {
            values: vs,
            index_bit_count,
            min_hash_bit_count,
            policy,
        } => hll::add_with_index_and_min_hash(
            &hll_policy(*policy),
            bin,
            values(vs)?,
            unset_as_minus_one(*index_bit_count),
            unset_as_minus_one(*min_hash_bit_count),
        ),
        WireHllOp::SetUnion { sketches, policy } => {
            hll::set_union(&hll_policy(*policy), bin, values(sketches)?)
        }
        WireHllOp::RefreshCount => hll::refresh_count(bin),
        WireHllOp::Fold { index_bit_count } => hll::fold(bin, *index_bit_count),

        WireHllOp::GetCount => hll::get_count(bin),
        WireHllOp::GetUnion { sketches } => hll::get_union(bin, values(sketches)?),
        WireHllOp::GetUnionCount { sketches } => hll::get_union_count(bin, values(sketches)?),
        WireHllOp::GetIntersectCount { sketches } => {
            hll::get_intersect_count(bin, values(sketches)?)
        }
        WireHllOp::GetSimilarity { sketches } => hll::get_similarity(bin, values(sketches)?),
        WireHllOp::Describe => hll::describe(bin),
    })
}

/// The client's "leave it as it is" for a HyperLogLog bit count.
const fn unset_as_minus_one(count: Option<i64>) -> i64 {
    match count {
        Some(count) => count,
        None => -1,
    }
}

pub(crate) const fn bit_resize(flags: WireBitResize) -> BitwiseResizeFlags {
    match flags {
        WireBitResize::Default => BitwiseResizeFlags::Default,
        WireBitResize::FromFront => BitwiseResizeFlags::FromFront,
        WireBitResize::GrowOnly => BitwiseResizeFlags::GrowOnly,
        WireBitResize::ShrinkOnly => BitwiseResizeFlags::ShrinkOnly,
    }
}

pub(crate) const fn bit_overflow(overflow: WireBitOverflow) -> BitwiseOverflowActions {
    match overflow {
        WireBitOverflow::Fail => BitwiseOverflowActions::Fail,
        WireBitOverflow::Saturate => BitwiseOverflowActions::Saturate,
        WireBitOverflow::Wrap => BitwiseOverflowActions::Wrap,
    }
}

/// The client's bitwise policy, which is a bitmask where the contract has one
/// mode and two flags.
pub(crate) fn bit_policy(policy: WireBitPolicy) -> BitPolicy {
    let mut flags = match policy.write_mode {
        WireBitWriteMode::Update => BitwiseWriteFlags::Default as u8,
        WireBitWriteMode::UpdateOnly => BitwiseWriteFlags::UpdateOnly as u8,
        WireBitWriteMode::CreateOnly => BitwiseWriteFlags::CreateOnly as u8,
    };
    if policy.no_fail {
        flags |= BitwiseWriteFlags::NoFail as u8;
    }
    if policy.partial {
        flags |= BitwiseWriteFlags::Partial as u8;
    }
    BitPolicy::new(flags)
}

/// The client's HyperLogLog policy, likewise.
pub(crate) fn hll_policy(policy: WireHllPolicy) -> HLLPolicy {
    let mut flags = match policy.write_mode {
        WireHllWriteMode::Update => HLLWriteFlags::Default as i64,
        WireHllWriteMode::UpdateOnly => HLLWriteFlags::UpdateOnly as i64,
        WireHllWriteMode::CreateOnly => HLLWriteFlags::CreateOnly as i64,
    };
    if policy.no_fail {
        flags |= HLLWriteFlags::NoFail as i64;
    }
    if policy.allow_fold {
        flags |= HLLWriteFlags::AllowFold as i64;
    }
    HLLPolicy { flags }
}

/// A `Bin` for the scalar operations that take one.
fn bin_of(name: &str, value: &aerospike_php_ipc::WireValue) -> Result<Bin, ResultOnlyValue> {
    Ok(Bin::new(name.to_owned(), convert::to_value(value)?))
}

/// The context path for a nested collection.
///
/// Takes the cluster's [`Capabilities`] because a step may carry a **filter
/// expression** — a fan-out over the children the filter accepts — and building
/// that expression needs the same gates any other expression does.
///
/// # Errors
/// [`OpError`] for a result-only value used as a key, an unusable filter
/// expression, or a fan-out against a cluster older than 8.1.1.
pub fn to_contexts(ctx: &[WireCtx], caps: &Capabilities) -> Result<Vec<CdtContext>, OpError> {
    ctx.iter().map(|step| to_context(step, caps)).collect()
}

fn to_context(ctx: &WireCtx, caps: &Capabilities) -> Result<CdtContext, OpError> {
    Ok(match ctx {
        WireCtx::ListIndex { index } => ctx_list_index(*index),
        WireCtx::ListIndexCreate { index, order, pad } => {
            ctx_list_index_create(*index, list_order(*order), *pad)
        }
        WireCtx::ListRank { rank } => ctx_list_rank(*rank),
        WireCtx::ListValue { value } => ctx_list_value(convert::to_value(value)?),
        WireCtx::MapIndex { index } => ctx_map_index(*index),
        WireCtx::MapRank { rank } => ctx_map_rank(*rank),
        WireCtx::MapKey { key } => ctx_map_key(convert::to_value(key)?),
        WireCtx::MapKeyCreate { key, order } => {
            ctx_map_key_create(convert::to_value(key)?, map_order(*order))
        }
        WireCtx::MapValue { value } => ctx_map_value(convert::to_value(value)?),

        // The fan-out steps. Both need server 8.1.1, and the gate is here rather
        // than at each path constructor because a fan-out is what needs it — a
        // path expression is only special because its context contains one.
        WireCtx::AllChildren => {
            caps.paths.require()?;
            ctx_all_children()
        }
        WireCtx::AllChildrenWithFilter { filter } => {
            caps.paths.require()?;
            ctx_all_children_with_filter(to_expression(filter, caps)?)
        }
    })
}

#[allow(clippy::too_many_lines)]
fn to_list_operation(bin: &str, op: &WireListOp) -> Result<Operation, ResultOnlyValue> {
    let value = |v| convert::to_value(v);
    let values = |vs: &Vec<aerospike_php_ipc::WireValue>| {
        vs.iter().map(convert::to_value).collect::<Result<Vec<_>, _>>()
    };

    Ok(match op {
        WireListOp::Create {
            order,
            pad,
            persist_index,
        } => lists::create_persistent(bin, list_order(*order), *pad, *persist_index),
        WireListOp::SetOrder {
            order,
            persist_index,
        } => {
            if *persist_index {
                lists::set_order_with_index(bin, list_order(*order))
            } else {
                lists::set_order(bin, list_order(*order))
            }
        }

        WireListOp::Append { policy, value: v } => {
            lists::append(&list_policy(*policy), bin, value(v)?)
        }
        WireListOp::AppendItems { policy, values: vs } => {
            lists::append_items(&list_policy(*policy), bin, values(vs)?)
        }
        WireListOp::Insert {
            policy,
            index,
            value: v,
        } => lists::insert(&list_policy(*policy), bin, *index, value(v)?),
        WireListOp::InsertItems {
            policy,
            index,
            values: vs,
        } => lists::insert_items(&list_policy(*policy), bin, *index, values(vs)?),

        WireListOp::Pop { index } => lists::pop(bin, *index),
        WireListOp::PopRange { index, count } => match count {
            Some(count) => lists::pop_range(bin, *index, *count),
            None => lists::pop_range_from(bin, *index),
        },
        WireListOp::Remove { index } => lists::remove(bin, *index),
        WireListOp::RemoveRange { index, count } => match count {
            Some(count) => lists::remove_range(bin, *index, *count),
            None => lists::remove_range_from(bin, *index),
        },
        WireListOp::RemoveByValue { value: v, return_type } => {
            lists::remove_by_value(bin, value(v)?, list_return(*return_type))
        }
        WireListOp::RemoveByValueList { values: vs, return_type } => {
            lists::remove_by_value_list(bin, values(vs)?, list_return(*return_type))
        }
        WireListOp::RemoveByValueRange {
            begin,
            end,
            return_type,
        } => lists::remove_by_value_range(
            bin,
            list_return(*return_type),
            bound(begin.as_ref())?,
            bound(end.as_ref())?,
        ),
        WireListOp::RemoveByValueRelativeRankRange {
            value: v,
            rank,
            count,
            return_type,
        } => match count {
            Some(count) => lists::remove_by_value_relative_rank_range_count(
                bin,
                list_return(*return_type),
                value(v)?,
                *rank,
                *count,
            ),
            None => lists::remove_by_value_relative_rank_range(
                bin,
                list_return(*return_type),
                value(v)?,
                *rank,
            ),
        },
        WireListOp::RemoveByIndex { index, return_type } => {
            lists::remove_by_index(bin, *index, list_return(*return_type))
        }
        WireListOp::RemoveByIndexRange {
            index,
            count,
            return_type,
        } => match count {
            Some(count) => {
                lists::remove_by_index_range_count(bin, *index, *count, list_return(*return_type))
            }
            None => lists::remove_by_index_range(bin, *index, list_return(*return_type)),
        },
        WireListOp::RemoveByRank { rank, return_type } => {
            lists::remove_by_rank(bin, *rank, list_return(*return_type))
        }
        WireListOp::RemoveByRankRange {
            rank,
            count,
            return_type,
        } => match count {
            Some(count) => {
                lists::remove_by_rank_range_count(bin, *rank, *count, list_return(*return_type))
            }
            None => lists::remove_by_rank_range(bin, *rank, list_return(*return_type)),
        },

        WireListOp::Set {
            policy,
            index,
            value: v,
        } => match policy {
            Some(policy) => lists::set_with_policy(&list_policy(*policy), bin, *index, value(v)?),
            None => lists::set(bin, *index, value(v)?),
        },
        WireListOp::Trim { index, count } => lists::trim(bin, *index, *count),
        WireListOp::Clear => lists::clear(bin),
        WireListOp::Increment {
            policy,
            index,
            value,
        } => match value {
            Some(delta) => lists::increment(&list_policy(*policy), bin, *index, *delta),
            None => lists::increment_by_one_with_policy(&list_policy(*policy), bin, *index),
        },
        WireListOp::Sort { flags } => lists::sort(bin, list_sort(*flags)),

        WireListOp::Size => lists::size(bin),
        WireListOp::Get { index } => lists::get(bin, *index),
        WireListOp::GetRange { index, count } => match count {
            Some(count) => lists::get_range(bin, *index, *count),
            None => lists::get_range_from(bin, *index),
        },
        WireListOp::GetByValue { value: v, return_type } => {
            lists::get_by_value(bin, value(v)?, list_return(*return_type))
        }
        WireListOp::GetByValueList { values: vs, return_type } => {
            lists::get_by_value_list(bin, values(vs)?, list_return(*return_type))
        }
        WireListOp::GetByValueRange {
            begin,
            end,
            return_type,
        } => lists::get_by_value_range(
            bin,
            bound(begin.as_ref())?,
            bound(end.as_ref())?,
            list_return(*return_type),
        ),
        // Note the argument order: the `get_*` relative-rank functions take the
        // return type **last**, where their `remove_*` counterparts take it
        // second. That asymmetry in the client's own API is the reason this
        // translation exists in exactly one place.
        WireListOp::GetByValueRelativeRankRange {
            value: v,
            rank,
            count,
            return_type,
        } => match count {
            Some(count) => lists::get_by_value_relative_rank_range_count(
                bin,
                value(v)?,
                *rank,
                *count,
                list_return(*return_type),
            ),
            None => lists::get_by_value_relative_rank_range(
                bin,
                value(v)?,
                *rank,
                list_return(*return_type),
            ),
        },
        WireListOp::GetByIndex { index, return_type } => {
            lists::get_by_index(bin, *index, list_return(*return_type))
        }
        WireListOp::GetByIndexRange {
            index,
            count,
            return_type,
        } => match count {
            Some(count) => {
                lists::get_by_index_range_count(bin, *index, *count, list_return(*return_type))
            }
            None => lists::get_by_index_range(bin, *index, list_return(*return_type)),
        },
        WireListOp::GetByRank { rank, return_type } => {
            lists::get_by_rank(bin, *rank, list_return(*return_type))
        }
        WireListOp::GetByRankRange {
            rank,
            count,
            return_type,
        } => match count {
            Some(count) => {
                lists::get_by_rank_range_count(bin, *rank, *count, list_return(*return_type))
            }
            None => lists::get_by_rank_range(bin, *rank, list_return(*return_type)),
        },
    })
}

/// One map operation.
///
/// `Create` and `SetPolicy` are handled by the caller, which owns the path they
/// need; everything else is a plain rename.
///
/// Unlike the list family, every function here takes the return type **last**,
/// which is one fewer thing to get wrong.
#[allow(clippy::too_many_lines)]
fn to_map_operation(bin: &str, op: &WireMapOp) -> Result<Operation, ResultOnlyValue> {
    let value = |v| convert::to_value(v);
    let values = |vs: &Vec<aerospike_php_ipc::WireValue>| {
        vs.iter().map(convert::to_value).collect::<Result<Vec<_>, _>>()
    };

    Ok(match op {
        // Handled by `to_operation`, which has the path these two need.
        WireMapOp::Create { .. } | WireMapOp::SetPolicy { .. } => {
            unreachable!("create and set_policy are built with their context path")
        }

        WireMapOp::SetOrder { order } => maps::set_order(bin, map_order(*order)),

        WireMapOp::Put { policy, key, value: v } => {
            maps::put(&map_policy(*policy), bin, value(key)?, value(v)?)
        }
        WireMapOp::PutItems { policy, items } => {
            // An `IndexMap`, so the order the caller wrote survives: with an
            // unordered policy the client sends it as given, and with an ordered
            // one it pre-sorts and sends the K-ordered header either way. A
            // `HashMap` would throw the order away for no gain.
            let mut map = IndexMap::with_capacity(items.len());
            for (key, value) in items {
                map.insert(convert::to_value(key)?, convert::to_value(value)?);
            }
            maps::put_items(&map_policy(*policy), bin, map)
        }
        WireMapOp::IncrementValue { policy, key, delta } => {
            maps::increment_value(&map_policy(*policy), bin, value(key)?, value(delta)?)
        }
        WireMapOp::DecrementValue { policy, key, delta } => {
            maps::decrement_value(&map_policy(*policy), bin, value(key)?, value(delta)?)
        }
        WireMapOp::Clear => maps::clear(bin),

        WireMapOp::RemoveByKey { key, return_type } => {
            maps::remove_by_key(bin, value(key)?, map_return(*return_type))
        }
        WireMapOp::RemoveByKeyList { keys, return_type } => {
            maps::remove_by_key_list(bin, values(keys)?, map_return(*return_type))
        }
        WireMapOp::RemoveByKeyRange {
            begin,
            end,
            return_type,
        } => maps::remove_by_key_range(
            bin,
            bound(begin.as_ref())?,
            bound(end.as_ref())?,
            map_return(*return_type),
        ),
        WireMapOp::RemoveByKeyRelativeIndexRange {
            key,
            index,
            count,
            return_type,
        } => match count {
            Some(count) => maps::remove_by_key_relative_index_range_count(
                bin,
                value(key)?,
                *index,
                *count,
                map_return(*return_type),
            ),
            None => maps::remove_by_key_relative_index_range(
                bin,
                value(key)?,
                *index,
                map_return(*return_type),
            ),
        },
        WireMapOp::RemoveByValue { value: v, return_type } => {
            maps::remove_by_value(bin, value(v)?, map_return(*return_type))
        }
        WireMapOp::RemoveByValueList { values: vs, return_type } => {
            maps::remove_by_value_list(bin, values(vs)?, map_return(*return_type))
        }
        WireMapOp::RemoveByValueRange {
            begin,
            end,
            return_type,
        } => maps::remove_by_value_range(
            bin,
            bound(begin.as_ref())?,
            bound(end.as_ref())?,
            map_return(*return_type),
        ),
        WireMapOp::RemoveByValueRelativeRankRange {
            value: v,
            rank,
            count,
            return_type,
        } => match count {
            Some(count) => maps::remove_by_value_relative_rank_range_count(
                bin,
                value(v)?,
                *rank,
                *count,
                map_return(*return_type),
            ),
            None => maps::remove_by_value_relative_rank_range(
                bin,
                value(v)?,
                *rank,
                map_return(*return_type),
            ),
        },
        WireMapOp::RemoveByIndex { index, return_type } => {
            maps::remove_by_index(bin, *index, map_return(*return_type))
        }
        WireMapOp::RemoveByIndexRange {
            index,
            count,
            return_type,
        } => match count {
            Some(count) => {
                maps::remove_by_index_range(bin, *index, *count, map_return(*return_type))
            }
            None => maps::remove_by_index_range_from(bin, *index, map_return(*return_type)),
        },
        WireMapOp::RemoveByRank { rank, return_type } => {
            maps::remove_by_rank(bin, *rank, map_return(*return_type))
        }
        WireMapOp::RemoveByRankRange {
            rank,
            count,
            return_type,
        } => match count {
            Some(count) => maps::remove_by_rank_range(bin, *rank, *count, map_return(*return_type)),
            None => maps::remove_by_rank_range_from(bin, *rank, map_return(*return_type)),
        },

        WireMapOp::Size => maps::size(bin),
        WireMapOp::GetByKey { key, return_type } => {
            maps::get_by_key(bin, value(key)?, map_return(*return_type))
        }
        WireMapOp::GetByKeyList { keys, return_type } => {
            maps::get_by_key_list(bin, values(keys)?, map_return(*return_type))
        }
        WireMapOp::GetByKeyRange {
            begin,
            end,
            return_type,
        } => maps::get_by_key_range(
            bin,
            bound(begin.as_ref())?,
            bound(end.as_ref())?,
            map_return(*return_type),
        ),
        WireMapOp::GetByKeyRelativeIndexRange {
            key,
            index,
            count,
            return_type,
        } => match count {
            Some(count) => maps::get_by_key_relative_index_range_count(
                bin,
                value(key)?,
                *index,
                *count,
                map_return(*return_type),
            ),
            None => maps::get_by_key_relative_index_range(
                bin,
                value(key)?,
                *index,
                map_return(*return_type),
            ),
        },
        WireMapOp::GetByValue { value: v, return_type } => {
            maps::get_by_value(bin, value(v)?, map_return(*return_type))
        }
        WireMapOp::GetByValueList { values: vs, return_type } => {
            maps::get_by_value_list(bin, values(vs)?, map_return(*return_type))
        }
        WireMapOp::GetByValueRange {
            begin,
            end,
            return_type,
        } => maps::get_by_value_range(
            bin,
            bound(begin.as_ref())?,
            bound(end.as_ref())?,
            map_return(*return_type),
        ),
        WireMapOp::GetByValueRelativeRankRange {
            value: v,
            rank,
            count,
            return_type,
        } => match count {
            Some(count) => maps::get_by_value_relative_rank_range_count(
                bin,
                value(v)?,
                *rank,
                *count,
                map_return(*return_type),
            ),
            None => maps::get_by_value_relative_rank_range(
                bin,
                value(v)?,
                *rank,
                map_return(*return_type),
            ),
        },
        WireMapOp::GetByIndex { index, return_type } => {
            maps::get_by_index(bin, *index, map_return(*return_type))
        }
        WireMapOp::GetByIndexRange {
            index,
            count,
            return_type,
        } => match count {
            Some(count) => maps::get_by_index_range(bin, *index, *count, map_return(*return_type)),
            None => maps::get_by_index_range_from(bin, *index, map_return(*return_type)),
        },
        WireMapOp::GetByRank { rank, return_type } => {
            maps::get_by_rank(bin, *rank, map_return(*return_type))
        }
        WireMapOp::GetByRankRange {
            rank,
            count,
            return_type,
        } => match count {
            Some(count) => maps::get_by_rank_range(bin, *rank, *count, map_return(*return_type)),
            None => maps::get_by_rank_range_from(bin, *rank, map_return(*return_type)),
        },
    })
}

/// An open end of a value range.
///
/// `None` becomes [`Value::Nil`], which is how the client spells "unbounded" —
/// nil sorts below every other value, and the server reads a missing bound the
/// same way.
fn bound(value: Option<&aerospike_php_ipc::WireValue>) -> Result<Value, ResultOnlyValue> {
    match value {
        Some(value) => convert::to_value(value),
        None => Ok(Value::Nil),
    }
}

const fn list_order(order: WireListOrder) -> ListOrderType {
    match order {
        WireListOrder::Unordered => ListOrderType::Unordered,
        WireListOrder::Ordered => ListOrderType::Ordered,
    }
}

const fn map_order(order: WireMapOrder) -> MapOrder {
    match order {
        WireMapOrder::Unordered => MapOrder::Unordered,
        WireMapOrder::KeyOrdered => MapOrder::KeyOrdered,
        WireMapOrder::KeyValueOrdered => MapOrder::KeyValueOrdered,
    }
}

pub(crate) const fn list_sort(flags: WireListSort) -> ListSortFlags {
    // The client's flags are an enum, not a bitmask, so the two cannot be
    // combined there either; descending wins when both are asked for, which
    // matches the order the flags are documented in.
    match (flags.descending, flags.drop_duplicates) {
        (true, _) => ListSortFlags::Descending,
        (false, true) => ListSortFlags::DropDuplicates,
        (false, false) => ListSortFlags::Default,
    }
}

pub(crate) fn list_policy(policy: WireListPolicy) -> ListPolicy {
    let mut flags = Vec::new();
    if policy.flags.add_unique {
        flags.push(ListWriteFlags::AddUnique);
    }
    if policy.flags.insert_bounded {
        flags.push(ListWriteFlags::InsertBounded);
    }
    if policy.flags.no_fail {
        flags.push(ListWriteFlags::NoFail);
    }
    if policy.flags.partial {
        flags.push(ListWriteFlags::Partial);
    }
    ListPolicy::new_with_flags(list_order(policy.order), flags)
}

/// A return type the client's list functions accept.
///
/// They are generic over `ToListReturnTypeBitmask` so that a caller can write
/// either `ListReturnType::Values` or `InvertedListReturn(..)`. Inversion here
/// is a runtime flag, not a choice of type, so this newtype carries the finished
/// bitmask into that generic slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ret(i64);

impl ToListReturnTypeBitmask for Ret {
    fn to_bitmask(self) -> i64 {
        self.0
    }
}

/// A map return type the client's map functions accept, as [`Ret`] is for
/// lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MapRet(i64);

impl ToMapReturnTypeBitmask for MapRet {
    fn to_bitmask(self) -> i64 {
        self.0
    }
}

/// The client's map return-type bitmask, inversion included.
pub(crate) fn map_return(return_type: WireMapReturn) -> MapRet {
    let kind = match return_type.kind {
        WireMapReturnKind::None => MapReturnType::None,
        WireMapReturnKind::Index => MapReturnType::Index,
        WireMapReturnKind::ReverseIndex => MapReturnType::ReverseIndex,
        WireMapReturnKind::Rank => MapReturnType::Rank,
        WireMapReturnKind::ReverseRank => MapReturnType::ReverseRank,
        WireMapReturnKind::Count => MapReturnType::Count,
        WireMapReturnKind::Key => MapReturnType::Key,
        WireMapReturnKind::Value => MapReturnType::Value,
        WireMapReturnKind::KeyValue => MapReturnType::KeyValue,
        WireMapReturnKind::Exists => MapReturnType::Exists,
        WireMapReturnKind::UnorderedMap => MapReturnType::UnorderedMap,
        WireMapReturnKind::OrderedMap => MapReturnType::OrderedMap,
    };
    MapRet(if return_type.inverted {
        InvertedMapReturn(kind).to_bitmask()
    } else {
        kind.to_bitmask()
    })
}

/// The client's map policy.
///
/// The client carries both a write mode and a flag bitmask, and the flags
/// *replace* the mode when they are non-zero. The contract carries one mode plus
/// the two flags that say something the mode cannot, so this is where they are
/// combined: the flags are only used when one of those two is asked for, and
/// they then have to re-encode the mode as well. Leaving `flags` at zero
/// otherwise keeps a plain `UpdateOnly` working against a server older than 4.3,
/// which is when write flags arrived.
pub(crate) fn map_policy(policy: WireMapPolicy) -> MapPolicy {
    let write_mode = match policy.write_mode {
        WireMapWriteMode::Update => MapWriteMode::Update,
        WireMapWriteMode::UpdateOnly => MapWriteMode::UpdateOnly,
        WireMapWriteMode::CreateOnly => MapWriteMode::CreateOnly,
    };

    let mut flags = 0;
    if policy.no_fail || policy.partial {
        flags = match policy.write_mode {
            WireMapWriteMode::Update => MapWriteFlags::DEFAULT,
            WireMapWriteMode::UpdateOnly => MapWriteFlags::UPDATE_ONLY,
            WireMapWriteMode::CreateOnly => MapWriteFlags::CREATE_ONLY,
        };
        if policy.no_fail {
            flags |= MapWriteFlags::NO_FAIL;
        }
        if policy.partial {
            flags |= MapWriteFlags::PARTIAL;
        }
    }

    MapPolicy {
        order: map_order(policy.order),
        write_mode,
        flags,
        persist_index: policy.persist_index,
    }
}

/// The client's return-type bitmask, inversion included.
pub(crate) fn list_return(return_type: WireListReturn) -> Ret {
    let kind = match return_type.kind {
        WireListReturnKind::None => ListReturnType::None,
        WireListReturnKind::Index => ListReturnType::Index,
        WireListReturnKind::ReverseIndex => ListReturnType::ReverseIndex,
        WireListReturnKind::Rank => ListReturnType::Rank,
        WireListReturnKind::ReverseRank => ListReturnType::ReverseRank,
        WireListReturnKind::Count => ListReturnType::Count,
        WireListReturnKind::Values => ListReturnType::Values,
        WireListReturnKind::Exists => ListReturnType::Exists,
    };
    Ret(if return_type.inverted {
        InvertedListReturn(kind).to_bitmask()
    } else {
        kind.to_bitmask()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::WireValue;

    fn list(op: WireListOp) -> WireOp {
        WireOp::List {
            bin: "items".into(),
            ctx: vec![],
            op,
        }
    }

    /// Every operation the contract can carry must translate. A `todo!()` or a
    /// missing arm would be a panic in the daemon's request path, so this walks
    /// the whole enum rather than a sample of it.
    #[test]
    fn every_operation_translates() {
        let value = || WireValue::Int(1);
        let ret = WireListReturn::of(WireListReturnKind::Values);
        let policy = WireListPolicy::default();

        let ops = vec![
            WireOp::Get,
            WireOp::GetHeader,
            WireOp::GetBin { bin: "a".into() },
            WireOp::Put { bin: "a".into(), value: value() },
            WireOp::Append { bin: "a".into(), value: WireValue::Str("x".into()) },
            WireOp::Prepend { bin: "a".into(), value: WireValue::Str("x".into()) },
            WireOp::Add { bin: "a".into(), value: value() },
            WireOp::Touch,
            WireOp::Delete,
            list(WireListOp::Create {
                order: WireListOrder::Ordered,
                pad: false,
                persist_index: true,
            }),
            list(WireListOp::SetOrder { order: WireListOrder::Ordered, persist_index: false }),
            list(WireListOp::SetOrder { order: WireListOrder::Ordered, persist_index: true }),
            list(WireListOp::Append { policy, value: value() }),
            list(WireListOp::AppendItems { policy, values: vec![value()] }),
            list(WireListOp::Insert { policy, index: 0, value: value() }),
            list(WireListOp::InsertItems { policy, index: 0, values: vec![value()] }),
            list(WireListOp::Pop { index: 0 }),
            list(WireListOp::PopRange { index: 0, count: Some(2) }),
            list(WireListOp::PopRange { index: 0, count: None }),
            list(WireListOp::Remove { index: 0 }),
            list(WireListOp::RemoveRange { index: 0, count: Some(2) }),
            list(WireListOp::RemoveRange { index: 0, count: None }),
            list(WireListOp::RemoveByValue { value: value(), return_type: ret }),
            list(WireListOp::RemoveByValueList { values: vec![value()], return_type: ret }),
            list(WireListOp::RemoveByValueRange {
                begin: Some(value()),
                end: None,
                return_type: ret,
            }),
            list(WireListOp::RemoveByValueRelativeRankRange {
                value: value(),
                rank: 0,
                count: Some(1),
                return_type: ret,
            }),
            list(WireListOp::RemoveByValueRelativeRankRange {
                value: value(),
                rank: 0,
                count: None,
                return_type: ret,
            }),
            list(WireListOp::RemoveByIndex { index: 0, return_type: ret }),
            list(WireListOp::RemoveByIndexRange { index: 0, count: Some(1), return_type: ret }),
            list(WireListOp::RemoveByIndexRange { index: 0, count: None, return_type: ret }),
            list(WireListOp::RemoveByRank { rank: 0, return_type: ret }),
            list(WireListOp::RemoveByRankRange { rank: 0, count: Some(1), return_type: ret }),
            list(WireListOp::RemoveByRankRange { rank: 0, count: None, return_type: ret }),
            list(WireListOp::Set { policy: None, index: 0, value: value() }),
            list(WireListOp::Set { policy: Some(policy), index: 0, value: value() }),
            list(WireListOp::Trim { index: 0, count: 1 }),
            list(WireListOp::Clear),
            list(WireListOp::Increment { policy, index: 0, value: Some(2) }),
            list(WireListOp::Increment { policy, index: 0, value: None }),
            list(WireListOp::Sort {
                flags: WireListSort { descending: true, drop_duplicates: false },
            }),
            list(WireListOp::Size),
            list(WireListOp::Get { index: 0 }),
            list(WireListOp::GetRange { index: 0, count: Some(2) }),
            list(WireListOp::GetRange { index: 0, count: None }),
            list(WireListOp::GetByValue { value: value(), return_type: ret }),
            list(WireListOp::GetByValueList { values: vec![value()], return_type: ret }),
            list(WireListOp::GetByValueRange { begin: None, end: Some(value()), return_type: ret }),
            list(WireListOp::GetByValueRelativeRankRange {
                value: value(),
                rank: 0,
                count: Some(1),
                return_type: ret,
            }),
            list(WireListOp::GetByValueRelativeRankRange {
                value: value(),
                rank: 0,
                count: None,
                return_type: ret,
            }),
            list(WireListOp::GetByIndex { index: 0, return_type: ret }),
            list(WireListOp::GetByIndexRange { index: 0, count: Some(1), return_type: ret }),
            list(WireListOp::GetByIndexRange { index: 0, count: None, return_type: ret }),
            list(WireListOp::GetByRank { rank: 0, return_type: ret }),
            list(WireListOp::GetByRankRange { rank: 0, count: Some(1), return_type: ret }),
            list(WireListOp::GetByRankRange { rank: 0, count: None, return_type: ret }),
        ];

        let built = to_operations(&ops, &Capabilities::all()).expect("every operation must translate");
        assert_eq!(built.len(), ops.len());
    }

    fn map(op: WireMapOp) -> WireOp {
        WireOp::Map {
            bin: "m".into(),
            ctx: vec![],
            op,
        }
    }

    /// Every map operation must translate, for the reason the list test gives:
    /// a missing arm is a panic in the request path.
    #[test]
    fn every_map_operation_translates() {
        let value = || WireValue::Int(1);
        let key = || WireValue::Str("k".into());
        let ret = WireMapReturn::of(WireMapReturnKind::Value);
        let policy = WireMapPolicy::default();

        let ops = vec![
            // Create with a path, which is the form that does not degrade to
            // `set_order`.
            WireOp::Map {
                bin: "m".into(),
                ctx: vec![WireCtx::MapKey { key: key() }],
                op: WireMapOp::Create {
                    order: WireMapOrder::KeyOrdered,
                    persist_index: false,
                },
            },
            map(WireMapOp::Create {
                order: WireMapOrder::KeyOrdered,
                persist_index: true,
            }),
            map(WireMapOp::SetOrder { order: WireMapOrder::KeyValueOrdered }),
            map(WireMapOp::SetPolicy { policy }),
            map(WireMapOp::Put { policy, key: key(), value: value() }),
            map(WireMapOp::PutItems {
                policy,
                items: vec![(key(), value())],
            }),
            map(WireMapOp::IncrementValue { policy, key: key(), delta: value() }),
            map(WireMapOp::DecrementValue { policy, key: key(), delta: value() }),
            map(WireMapOp::Clear),
            map(WireMapOp::RemoveByKey { key: key(), return_type: ret }),
            map(WireMapOp::RemoveByKeyList { keys: vec![key()], return_type: ret }),
            map(WireMapOp::RemoveByKeyRange {
                begin: Some(key()),
                end: None,
                return_type: ret,
            }),
            map(WireMapOp::RemoveByKeyRelativeIndexRange {
                key: key(),
                index: 0,
                count: Some(1),
                return_type: ret,
            }),
            map(WireMapOp::RemoveByKeyRelativeIndexRange {
                key: key(),
                index: 0,
                count: None,
                return_type: ret,
            }),
            map(WireMapOp::RemoveByValue { value: value(), return_type: ret }),
            map(WireMapOp::RemoveByValueList { values: vec![value()], return_type: ret }),
            map(WireMapOp::RemoveByValueRange {
                begin: None,
                end: Some(value()),
                return_type: ret,
            }),
            map(WireMapOp::RemoveByValueRelativeRankRange {
                value: value(),
                rank: 0,
                count: Some(1),
                return_type: ret,
            }),
            map(WireMapOp::RemoveByValueRelativeRankRange {
                value: value(),
                rank: 0,
                count: None,
                return_type: ret,
            }),
            map(WireMapOp::RemoveByIndex { index: 0, return_type: ret }),
            map(WireMapOp::RemoveByIndexRange { index: 0, count: Some(1), return_type: ret }),
            map(WireMapOp::RemoveByIndexRange { index: 0, count: None, return_type: ret }),
            map(WireMapOp::RemoveByRank { rank: 0, return_type: ret }),
            map(WireMapOp::RemoveByRankRange { rank: 0, count: Some(1), return_type: ret }),
            map(WireMapOp::RemoveByRankRange { rank: 0, count: None, return_type: ret }),
            map(WireMapOp::Size),
            map(WireMapOp::GetByKey { key: key(), return_type: ret }),
            map(WireMapOp::GetByKeyList { keys: vec![key()], return_type: ret }),
            map(WireMapOp::GetByKeyRange {
                begin: Some(key()),
                end: Some(WireValue::Str("z".into())),
                return_type: ret,
            }),
            map(WireMapOp::GetByKeyRelativeIndexRange {
                key: key(),
                index: 0,
                count: Some(1),
                return_type: ret,
            }),
            map(WireMapOp::GetByKeyRelativeIndexRange {
                key: key(),
                index: 0,
                count: None,
                return_type: ret,
            }),
            map(WireMapOp::GetByValue { value: value(), return_type: ret }),
            map(WireMapOp::GetByValueList { values: vec![value()], return_type: ret }),
            map(WireMapOp::GetByValueRange {
                begin: None,
                end: None,
                return_type: ret,
            }),
            map(WireMapOp::GetByValueRelativeRankRange {
                value: value(),
                rank: 0,
                count: Some(1),
                return_type: ret,
            }),
            map(WireMapOp::GetByValueRelativeRankRange {
                value: value(),
                rank: 0,
                count: None,
                return_type: ret,
            }),
            map(WireMapOp::GetByIndex { index: 0, return_type: ret }),
            map(WireMapOp::GetByIndexRange { index: 0, count: Some(1), return_type: ret }),
            map(WireMapOp::GetByIndexRange { index: 0, count: None, return_type: ret }),
            map(WireMapOp::GetByRank { rank: 0, return_type: ret }),
            map(WireMapOp::GetByRankRange { rank: 0, count: Some(1), return_type: ret }),
            map(WireMapOp::GetByRankRange { rank: 0, count: None, return_type: ret }),
        ];

        let built = to_operations(&ops, &Capabilities::all()).expect("every map operation must translate");
        assert_eq!(built.len(), ops.len());
    }

    /// The write mode and the two extra flags are one knob set on the wire and
    /// two in the client, where the flags *replace* the mode when non-zero. The
    /// combination has to re-encode the mode, or asking for "do not fail" would
    /// silently turn an `UpdateOnly` into a plain update.
    #[test]
    fn a_map_write_mode_survives_being_combined_with_the_flags() {
        let plain = map_policy(WireMapPolicy {
            write_mode: WireMapWriteMode::UpdateOnly,
            ..WireMapPolicy::default()
        });
        assert_eq!(
            plain.flags, 0,
            "with no extra flags the mode must be carried by write_mode, which works on a server \
             older than 4.3"
        );
        assert!(matches!(plain.write_mode, MapWriteMode::UpdateOnly));

        let with_no_fail = map_policy(WireMapPolicy {
            write_mode: WireMapWriteMode::UpdateOnly,
            no_fail: true,
            ..WireMapPolicy::default()
        });
        assert_eq!(
            with_no_fail.flags,
            MapWriteFlags::UPDATE_ONLY | MapWriteFlags::NO_FAIL,
            "the flags take precedence over the mode, so they must say UpdateOnly too"
        );

        let create_partial = map_policy(WireMapPolicy {
            write_mode: WireMapWriteMode::CreateOnly,
            no_fail: true,
            partial: true,
            ..WireMapPolicy::default()
        });
        assert_eq!(
            create_partial.flags,
            MapWriteFlags::CREATE_ONLY | MapWriteFlags::NO_FAIL | MapWriteFlags::PARTIAL
        );

        let persisted = map_policy(WireMapPolicy {
            persist_index: true,
            ..WireMapPolicy::default()
        });
        assert!(persisted.persist_index);
    }

    fn bit(op: WireBitOp) -> WireOp {
        WireOp::Bit {
            bin: "flags".into(),
            ctx: vec![],
            op,
        }
    }

    fn hll(op: WireHllOp) -> WireOp {
        WireOp::Hll {
            bin: "sketch".into(),
            ctx: vec![],
            op,
        }
    }

    /// Every bitwise operation must translate; a missing arm is a panic in the
    /// request path.
    #[test]
    fn every_bit_operation_translates() {
        let blob = || WireValue::Blob(vec![0xff]);
        let policy = WireBitPolicy::default();

        let ops = vec![
            bit(WireBitOp::Resize {
                byte_size: 4,
                flags: Some(WireBitResize::FromFront),
                policy,
            }),
            bit(WireBitOp::Resize { byte_size: 4, flags: None, policy }),
            bit(WireBitOp::Insert { byte_offset: 0, value: blob(), policy }),
            bit(WireBitOp::Remove { byte_offset: 0, byte_size: 1, policy }),
            bit(WireBitOp::Set { bit_offset: 0, bit_size: 8, value: blob(), policy }),
            bit(WireBitOp::Or { bit_offset: 0, bit_size: 8, value: blob(), policy }),
            bit(WireBitOp::Xor { bit_offset: 0, bit_size: 8, value: blob(), policy }),
            bit(WireBitOp::And { bit_offset: 0, bit_size: 8, value: blob(), policy }),
            bit(WireBitOp::Not { bit_offset: 0, bit_size: 8, policy }),
            bit(WireBitOp::LeftShift { bit_offset: 0, bit_size: 8, shift: 1, policy }),
            bit(WireBitOp::RightShift { bit_offset: 0, bit_size: 8, shift: 1, policy }),
            bit(WireBitOp::Add {
                bit_offset: 0,
                bit_size: 8,
                value: 1,
                signed: false,
                overflow: WireBitOverflow::Saturate,
                policy,
            }),
            bit(WireBitOp::Subtract {
                bit_offset: 0,
                bit_size: 8,
                value: 1,
                signed: true,
                overflow: WireBitOverflow::Wrap,
                policy,
            }),
            bit(WireBitOp::SetInt { bit_offset: 0, bit_size: 8, value: 7, policy }),
            bit(WireBitOp::Get { bit_offset: 0, bit_size: 8 }),
            bit(WireBitOp::Count { bit_offset: 0, bit_size: 8 }),
            bit(WireBitOp::LeftScan { bit_offset: 0, bit_size: 8, value: true }),
            bit(WireBitOp::RightScan { bit_offset: 0, bit_size: 8, value: false }),
            bit(WireBitOp::GetInt { bit_offset: 0, bit_size: 8, signed: true }),
        ];

        let built = to_operations(&ops, &Capabilities::all()).expect("every bit operation must translate");
        assert_eq!(built.len(), ops.len());
    }

    /// Every HyperLogLog operation must translate, including both spellings of
    /// each optional bit count.
    #[test]
    fn every_hll_operation_translates() {
        let sketches = || vec![WireValue::Blob(vec![1, 2])];
        let policy = WireHllPolicy::default();

        let ops = vec![
            hll(WireHllOp::Init {
                index_bit_count: 12,
                min_hash_bit_count: Some(20),
                policy,
            }),
            hll(WireHllOp::Init {
                index_bit_count: 12,
                min_hash_bit_count: None,
                policy,
            }),
            hll(WireHllOp::Add {
                values: vec![WireValue::Str("x".into())],
                index_bit_count: Some(12),
                min_hash_bit_count: Some(20),
                policy,
            }),
            hll(WireHllOp::Add {
                values: vec![WireValue::Str("x".into())],
                index_bit_count: None,
                min_hash_bit_count: None,
                policy,
            }),
            hll(WireHllOp::SetUnion { sketches: sketches(), policy }),
            hll(WireHllOp::RefreshCount),
            hll(WireHllOp::Fold { index_bit_count: 8 }),
            hll(WireHllOp::GetCount),
            hll(WireHllOp::GetUnion { sketches: sketches() }),
            hll(WireHllOp::GetUnionCount { sketches: sketches() }),
            hll(WireHllOp::GetIntersectCount { sketches: sketches() }),
            hll(WireHllOp::GetSimilarity { sketches: sketches() }),
            hll(WireHllOp::Describe),
        ];

        let built = to_operations(&ops, &Capabilities::all()).expect("every HLL operation must translate");
        assert_eq!(built.len(), ops.len());
    }

    /// Both directions and both expression forms must build.
    #[test]
    fn every_expression_operation_translates() {
        let ops = vec![
            WireOp::Exp {
                op: WireExpOp::Read {
                    name: "total".into(),
                    expression: WireExpression::Ael("$.a + $.b".into()),
                    flags: WireExpReadFlags { eval_no_fail: true },
                },
            },
            WireOp::Exp {
                op: WireExpOp::Write {
                    bin: "total".into(),
                    expression: WireExpression::Ael("$.a + $.b".into()),
                    flags: WireExpWriteFlags {
                        write_mode: WireExpWriteMode::CreateOnly,
                        allow_delete: true,
                        no_fail: true,
                        eval_no_fail: true,
                    },
                },
            },
            // A packed expression from another client: valid base64 of a
            // one-element msgpack array.
            WireOp::Exp {
                op: WireExpOp::Read {
                    name: "packed".into(),
                    expression: WireExpression::Base64("kQE=".into()),
                    flags: WireExpReadFlags::default(),
                },
            },
        ];

        let built = to_operations(&ops, &Capabilities::all())
            .expect("every expression operation must translate");
        assert_eq!(built.len(), ops.len());
    }

    /// An AEL expression against a cluster too old to compile it is refused
    /// here, naming the node — the same treatment the policy filter gets, and
    /// for the same reason: the server's own complaint says nothing useful.
    #[test]
    fn an_ael_expression_needs_a_new_enough_cluster() {
        let op = WireOp::Exp {
            op: WireExpOp::Read {
                name: "x".into(),
                expression: WireExpression::Ael("$.a".into()),
                flags: WireExpReadFlags::default(),
            },
        };
        let old = Capabilities {
            ael: AelSupport::Unsupported {
                node: "BB9A".into(),
                version: "7.2.0".into(),
            },
            ..Capabilities::all()
        };

        let error = to_operation(&op, &old).expect_err("an old cluster must be refused");
        let message = error.to_string();
        assert!(message.contains("BB9A"), "{message}");
        assert!(message.contains("7.2.0"), "{message}");
        assert!(message.contains("8.1.3"), "{message}");

        // A packed expression does not need the server to parse anything, so
        // the same cluster takes it.
        let packed = WireOp::Exp {
            op: WireExpOp::Read {
                name: "x".into(),
                expression: WireExpression::Base64("kQE=".into()),
                flags: WireExpReadFlags::default(),
            },
        };
        assert!(to_operation(&packed, &old).is_ok());
    }

    /// Base64 that is not base64 is a mistake worth naming, and the likeliest
    /// version of it is passing the expression's *source* by hand.
    #[test]
    fn a_malformed_packed_expression_says_what_was_expected() {
        let op = WireOp::Exp {
            op: WireExpOp::Read {
                name: "x".into(),
                expression: WireExpression::Base64("$.a == 1".into()),
                flags: WireExpReadFlags::default(),
            },
        };
        let error =
            to_operation(&op, &Capabilities::all()).expect_err("bad base64 must be refused");
        assert!(error.to_string().contains("source text"), "{error}");
    }

    /// The write flags are a bitmask in the client, and the mode has to survive
    /// being combined with the rest of them.
    #[test]
    fn expression_write_flags_combine() {
        assert_eq!(exp_write_flags(WireExpWriteFlags::default()).0, 0);
        assert_eq!(
            exp_write_flags(WireExpWriteFlags {
                write_mode: WireExpWriteMode::UpdateOnly,
                allow_delete: true,
                no_fail: true,
                eval_no_fail: true,
            })
            .0,
            2 | 4 | 8 | 16
        );
        assert_eq!(
            exp_write_flags(WireExpWriteFlags {
                write_mode: WireExpWriteMode::CreateOnly,
                ..WireExpWriteFlags::default()
            })
            .0,
            1
        );
        assert_eq!(exp_read_flags(WireExpReadFlags::default()).0, 0);
        assert_eq!(exp_read_flags(WireExpReadFlags { eval_no_fail: true }).0, 16);
    }

    /// `None` is the client's `-1`, and it must not collide with a real count:
    /// a zero index-bit count is a different request from an absent one.
    #[test]
    fn an_absent_bit_count_becomes_the_clients_sentinel() {
        assert_eq!(unset_as_minus_one(None), -1);
        assert_eq!(unset_as_minus_one(Some(0)), 0);
        assert_eq!(unset_as_minus_one(Some(12)), 12);
    }

    /// Both of these policies are bitmasks in the client and a mode plus flags
    /// on the wire, so the mode has to survive being combined — the same trap
    /// the map policy has.
    #[test]
    fn bit_and_hll_write_modes_survive_the_flags() {
        assert_eq!(bit_policy(WireBitPolicy::default()).flags, 0);
        assert_eq!(
            bit_policy(WireBitPolicy {
                write_mode: WireBitWriteMode::UpdateOnly,
                no_fail: true,
                partial: true,
            })
            .flags,
            2 | 4 | 8
        );
        assert_eq!(
            bit_policy(WireBitPolicy {
                write_mode: WireBitWriteMode::CreateOnly,
                no_fail: false,
                partial: false,
            })
            .flags,
            1
        );

        assert_eq!(hll_policy(WireHllPolicy::default()).flags, 0);
        assert_eq!(
            hll_policy(WireHllPolicy {
                write_mode: WireHllWriteMode::CreateOnly,
                no_fail: true,
                allow_fold: true,
            })
            .flags,
            1 | 4 | 8
        );
    }

    /// Map inversion is the same bit as a list's, and losing it would remove the
    /// wrong half of a map.
    #[test]
    fn map_inversion_reaches_the_bitmask() {
        let plain = map_return(WireMapReturn::of(WireMapReturnKind::KeyValue)).0;
        let inverted = map_return(WireMapReturn {
            kind: WireMapReturnKind::KeyValue,
            inverted: true,
        })
        .0;
        assert_eq!(inverted, plain ^ 0x10000);
    }

    /// A path is attached to the operation, not folded into the bin name, so a
    /// nested operation and a top-level one differ only by the context.
    #[test]
    fn a_context_path_is_attached_to_the_operation() {
        let nested = WireOp::List {
            bin: "profile".into(),
            ctx: vec![
                WireCtx::MapKey { key: WireValue::Str("tags".into()) },
                WireCtx::ListIndex { index: 0 },
            ],
            op: WireListOp::Size,
        };
        let flat = WireOp::List {
            bin: "profile".into(),
            ctx: vec![],
            op: WireListOp::Size,
        };
        assert_ne!(to_operation(&nested, &Capabilities::all()).unwrap(),
            to_operation(&flat, &Capabilities::all()).unwrap());
    }

    /// Every context step must build; a missing arm would panic in the request
    /// path exactly as a missing operation would.
    #[test]
    fn every_context_step_translates() {
        let steps = vec![
            WireCtx::ListIndex { index: 0 },
            WireCtx::ListIndexCreate {
                index: 0,
                order: WireListOrder::Ordered,
                pad: true,
            },
            WireCtx::ListRank { rank: -1 },
            WireCtx::ListValue { value: WireValue::Int(1) },
            WireCtx::MapIndex { index: 0 },
            WireCtx::MapRank { rank: -1 },
            WireCtx::MapKey { key: WireValue::Str("k".into()) },
            WireCtx::MapKeyCreate {
                key: WireValue::Str("k".into()),
                order: WireMapOrder::KeyOrdered,
            },
            WireCtx::MapValue { value: WireValue::Int(1) },
        ];
        assert_eq!(to_contexts(&steps, &Capabilities::all()).unwrap().len(), steps.len());
    }

    /// A value only the server produces cannot be an operation's argument, and
    /// must be refused here rather than turned into something else.
    #[test]
    fn a_result_only_value_is_refused_as_an_argument() {
        let op = list(WireListOp::Append {
            policy: WireListPolicy::default(),
            value: WireValue::MultiResult(vec![WireValue::Int(1)]),
        });
        assert!(to_operation(&op, &Capabilities::all()).is_err());

        let ctx = WireOp::List {
            bin: "items".into(),
            ctx: vec![WireCtx::MapKey {
                key: WireValue::Unknown { particle_type: 42, data: vec![] },
            }],
            op: WireListOp::Size,
        };
        assert!(to_operation(&ctx, &Capabilities::all()).is_err());
    }

    /// The inversion bit has to survive into the client's bitmask, because it is
    /// what turns "these items" into "all the others" — a silent loss would
    /// remove the wrong half of a list.
    #[test]
    fn inversion_reaches_the_bitmask() {
        let plain = list_return(WireListReturn::of(WireListReturnKind::Values)).0;
        let inverted = list_return(WireListReturn {
            kind: WireListReturnKind::Values,
            inverted: true,
        })
        .0;
        assert_ne!(plain, inverted);
        assert_eq!(inverted, plain ^ 0x10000);
    }

    /// The write flags are booleans on the wire and a bitmask in the client, so
    /// each one has to land in its own bit and several must combine.
    #[test]
    fn write_flags_combine_into_the_clients_bitmask() {
        let none = list_policy(WireListPolicy::default());
        assert_eq!(none.flags, 0);

        let all = list_policy(WireListPolicy {
            order: WireListOrder::Ordered,
            flags: aerospike_php_ipc::WireListWriteFlags {
                add_unique: true,
                insert_bounded: true,
                no_fail: true,
                partial: true,
            },
        });
        assert_eq!(all.flags, 1 | 2 | 4 | 8);

        let one = list_policy(WireListPolicy {
            order: WireListOrder::Unordered,
            flags: aerospike_php_ipc::WireListWriteFlags {
                no_fail: true,
                ..aerospike_php_ipc::WireListWriteFlags::default()
            },
        });
        assert_eq!(one.flags, 4);
    }
}
