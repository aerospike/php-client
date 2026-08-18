// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! [`WireExp`] → `aerospike-core`'s `Expression`.
//!
//! The tree arrives as a description and leaves as a real expression, built by
//! calling the client's own constructors. Nothing here packs bytes: the client
//! does that, so this side cannot get the wire format wrong — the worst it can do
//! is call the wrong constructor, which is a mistake a test can see.
//!
//! # The tree is measured before it is walked
//!
//! [`from_tree`] checks [`WireExp::check_size`] first and only then recurses.
//! The depth limit is what makes the recursion safe: a tree comes from another
//! process, so its depth is that process's choice, and unbounded recursion in the
//! daemon is a stack overflow that takes down every worker on the host rather than
//! failing one request. Checking as it descends would be the same unbounded
//! recursion, which is why the check is iterative and happens once.
//!
//! # What this refuses
//!
//! Two things, both because the client has no constructor for them rather than
//! because of a rule invented here: a HyperLogLog *literal* (there is no
//! `hll_val`; a sketch is something a bin holds, not something an expression
//! spells out) and the result-only value shapes the contract uses for replies.
//! Both are named in the message, with what to use instead.

use aerospike_core::expressions::{
    self as exps, bitwise as exp_bits, hll as exp_hll, lists as exp_lists, maps as exp_maps,
    string as exp_strings, ExpType, Expression,
};
use aerospike_core::expressions::LoopVarPart;
use aerospike_core::operations::path::{ModifyFlag, SelectFlag};
use aerospike_core::operations::string::{
    StringNumericType, StringPolicy, StringRegexFlags, StringWriteFlags,
};
use aerospike_core::IndexMap;
use aerospike_php_ipc::exp::{
    WireExp, WireExpBinaryOp, WireExpBit, WireExpBitOp, WireExpHll, WireExpHllOp, WireExpList,
    WireExpListOp, WireExpMap, WireExpMapOp, WireExpMeta, WireExpStr, WireExpStrOp, WireExpType,
    WireExpPath, WireExpPathOp, WireExpUnaryOp, WireExpVariadicOp, WireLoopVarPart,
    WireStringNumericType, WireStringPolicy,
};
use aerospike_php_ipc::WireValue;

use crate::convert::to_value;
use crate::policy::Capabilities;
use crate::ops::{
    bit_overflow, bit_policy, bit_resize, hll_policy, list_policy, list_return, list_sort,
    map_policy, map_return, to_contexts, OpError,
};

/// Build a client expression from a wire tree.
///
/// Named `from_tree` rather than `to_expression` because [`crate::ops`] already
/// has a `to_expression` — the one every command goes through, which dispatches
/// on the three wire forms and calls this for one of them.
///
/// # Errors
/// [`OpError::Expression`] for a tree past the size limits, a value shape that
/// cannot be an expression literal, or a context path that cannot be built.
pub fn from_tree(exp: &WireExp, caps: &Capabilities) -> Result<Expression, OpError> {
    exp.check_size().map_err(OpError::Expression)?;
    build(exp, caps)
}

/// The recursive half, after the size check.
///
/// Separate from [`from_tree`] so the check happens once rather than at every
/// level: it walks the whole tree, so doing it per node would be quadratic.
fn build(exp: &WireExp, caps: &Capabilities) -> Result<Expression, OpError> {
    Ok(match exp {
        WireExp::Value(value) => literal(value)?,

        WireExp::Bin { name, exp_type } => exps::bin(name.clone(), exp_type_of(*exp_type)),
        WireExp::BinExists(name) => exps::bin_exists(name.clone()),
        WireExp::BinType(name) => exps::bin_type(name.clone()),
        WireExp::Key(exp_type) => exps::key(exp_type_of(*exp_type)),
        WireExp::DigestModulo(modulo) => exps::digest_modulo(*modulo),

        // `device_size` and `memory_size` are deprecated in the client in favour
        // of `record_size`, but they are still distinct server opcodes and a
        // caller on an older server needs them — so the contract carries all
        // three and this reaches them.
        #[allow(deprecated)]
        WireExp::Meta(meta) => match meta {
            WireExpMeta::KeyExists => exps::key_exists(),
            WireExpMeta::SetName => exps::set_name(),
            WireExpMeta::RecordSize => exps::record_size(),
            WireExpMeta::DeviceSize => exps::device_size(),
            WireExpMeta::MemorySize => exps::memory_size(),
            WireExpMeta::LastUpdate => exps::last_update(),
            WireExpMeta::SinceUpdate => exps::since_update(),
            WireExpMeta::VoidTime => exps::void_time(),
            WireExpMeta::Ttl => exps::ttl(),
            WireExpMeta::IsTombstone => exps::is_tombstone(),
        },

        WireExp::Unary { op, exp } => {
            let operand = build(exp, caps)?;
            match op {
                WireExpUnaryOp::Not => exps::not(operand),
                WireExpUnaryOp::NumAbs => exps::num_abs(operand),
                WireExpUnaryOp::NumFloor => exps::num_floor(operand),
                WireExpUnaryOp::NumCeil => exps::num_ceil(operand),
                WireExpUnaryOp::ToInt => exps::to_int(operand),
                WireExpUnaryOp::ToFloat => exps::to_float(operand),
                WireExpUnaryOp::IntNot => exps::int_not(operand),
                WireExpUnaryOp::IntCount => exps::int_count(operand),
                WireExpUnaryOp::MapKeys => exps::map_keys(operand),
                WireExpUnaryOp::MapValues => exps::map_values(operand),
            }
        }

        WireExp::Binary { op, left, right } => {
            let (left, right) = (build(left, caps)?, build(right, caps)?);
            match op {
                WireExpBinaryOp::Eq => exps::eq(left, right),
                WireExpBinaryOp::Ne => exps::ne(left, right),
                WireExpBinaryOp::Gt => exps::gt(left, right),
                WireExpBinaryOp::Ge => exps::ge(left, right),
                WireExpBinaryOp::Lt => exps::lt(left, right),
                WireExpBinaryOp::Le => exps::le(left, right),
                WireExpBinaryOp::NumPow => exps::num_pow(left, right),
                WireExpBinaryOp::NumLog => exps::num_log(left, right),
                WireExpBinaryOp::NumMod => exps::num_mod(left, right),
                WireExpBinaryOp::IntLshift => exps::int_lshift(left, right),
                WireExpBinaryOp::IntRshift => exps::int_rshift(left, right),
                WireExpBinaryOp::IntArshift => exps::int_arshift(left, right),
                WireExpBinaryOp::IntLscan => exps::int_lscan(left, right),
                WireExpBinaryOp::IntRscan => exps::int_rscan(left, right),
                WireExpBinaryOp::GeoCompare => exps::geo_compare(left, right),
                WireExpBinaryOp::InList => exps::in_list(left, right),
            }
        }

        WireExp::Variadic { op, exps: operands } => {
            let built = operands.iter().map(|e| build(e, caps)).collect::<Result<Vec<_>, _>>()?;
            match op {
                WireExpVariadicOp::And => exps::and(built),
                WireExpVariadicOp::Or => exps::or(built),
                // `exclusive` is the same opcode, which is why the contract has
                // one variant for both spellings.
                WireExpVariadicOp::Xor => exps::xor(built),
                WireExpVariadicOp::NumAdd => exps::num_add(built),
                WireExpVariadicOp::NumSub => exps::num_sub(built),
                WireExpVariadicOp::NumMul => exps::num_mul(built),
                WireExpVariadicOp::NumDiv => exps::num_div(built),
                WireExpVariadicOp::IntAnd => exps::int_and(built),
                WireExpVariadicOp::IntOr => exps::int_or(built),
                WireExpVariadicOp::IntXor => exps::int_xor(built),
                WireExpVariadicOp::Min => exps::min(built),
                WireExpVariadicOp::Max => exps::max(built),
                WireExpVariadicOp::Cond => exps::cond(built),
                WireExpVariadicOp::Let => exps::exp_let(built),
            }
        }

        WireExp::Regex { regex, flags, bin } => {
            exps::regex_compare(regex.clone(), *flags, build(bin, caps)?)
        }

        WireExp::Def { name, value } => exps::def(name.clone(), build(value, caps)?),
        WireExp::Var(name) => exps::var(name.clone()),
        WireExp::Unknown => exps::unknown(),

        WireExp::Path(path) => build_path(path, caps)?,

        // A loop variable and a remove-result are leaves, but only *inside* a
        // fan-out — the server refuses one elsewhere, and it is the only side that
        // knows where "elsewhere" is. The 8.1.1 gate still applies: both opcodes
        // arrived with the path expressions.
        WireExp::LoopVar { exp_type, part } => {
            caps.paths.require()?;
            loop_var(*exp_type, *part)
        }
        WireExp::RemoveResult => {
            caps.paths.require()?;
            exps::exp_remove_result()
        }

        WireExp::List(list) => build_list(list, caps)?,
        WireExp::Map(map) => build_map(map, caps)?,
        WireExp::Bit(bit) => build_bit(bit, caps)?,
        WireExp::Hll(hll) => build_hll(hll, caps)?,
        WireExp::Str(string) => build_str(string, caps)?,
    })
}

/// Build a list expression.
///
/// `bin` and `ctx` are already built by the caller: every one of these
/// constructors takes them last, so building them once here rather than in
/// thirty-one arms is both shorter and impossible to get inconsistent.
fn build_list(node: &WireExpList, caps: &Capabilities) -> Result<Expression, OpError> {
    let bin = build(&node.bin, caps)?;
    let ctx = to_contexts(&node.ctx, caps).map_err(|e| {
        OpError::Expression(format!("that list expression's context path is not usable: {e}"))
    })?;
    let ctx = ctx.as_slice();
    Ok(match &node.op {
            WireExpListOp::Append { policy, value } => {
                let value = build(value, caps)?;
                exp_lists::append(list_policy(*policy), value, bin, ctx)
            }
            WireExpListOp::AppendItems { policy, list } => {
                let list = build(list, caps)?;
                exp_lists::append_items(list_policy(*policy), list, bin, ctx)
            }
            WireExpListOp::Insert { policy, index, value } => {
                let index = build(index, caps)?;
                let value = build(value, caps)?;
                exp_lists::insert(list_policy(*policy), index, value, bin, ctx)
            }
            WireExpListOp::InsertItems { policy, index, list } => {
                let index = build(index, caps)?;
                let list = build(list, caps)?;
                exp_lists::insert_items(list_policy(*policy), index, list, bin, ctx)
            }
            WireExpListOp::Increment { policy, index, value } => {
                let index = build(index, caps)?;
                let value = build(value, caps)?;
                exp_lists::increment(list_policy(*policy), index, value, bin, ctx)
            }
            WireExpListOp::Set { policy, index, value } => {
                let index = build(index, caps)?;
                let value = build(value, caps)?;
                exp_lists::set(list_policy(*policy), index, value, bin, ctx)
            }
            WireExpListOp::Clear => {
                exp_lists::clear(bin, ctx)
            }
            WireExpListOp::Sort { flags } => {
                exp_lists::sort(list_sort(*flags), bin, ctx)
            }
            WireExpListOp::RemoveByValue { return_type, value } => {
                let value = build(value, caps)?;
                exp_lists::remove_by_value(list_return(*return_type), value, bin, ctx)
            }
            WireExpListOp::RemoveByValueList { return_type, values } => {
                let values = build(values, caps)?;
                exp_lists::remove_by_value_list(list_return(*return_type), values, bin, ctx)
            }
            WireExpListOp::RemoveByValueRange { return_type, value_begin, value_end } => {
                let value_begin = value_begin.as_ref().map(|e| build(e, caps)).transpose()?;
                let value_end = value_end.as_ref().map(|e| build(e, caps)).transpose()?;
                exp_lists::remove_by_value_range(list_return(*return_type), value_begin, value_end, bin, ctx)
            }
            WireExpListOp::RemoveByValueRelRankRange { return_type, value, rank } => {
                let value = build(value, caps)?;
                let rank = build(rank, caps)?;
                exp_lists::remove_by_value_relative_rank_range(list_return(*return_type), value, rank, bin, ctx)
            }
            WireExpListOp::RemoveByValueRelRankRangeCount { return_type, value, rank, count } => {
                let value = build(value, caps)?;
                let rank = build(rank, caps)?;
                let count = build(count, caps)?;
                exp_lists::remove_by_value_relative_rank_range_count(list_return(*return_type), value, rank, count, bin, ctx)
            }
            WireExpListOp::RemoveByIndex { return_type, index } => {
                let index = build(index, caps)?;
                exp_lists::remove_by_index(list_return(*return_type), index, bin, ctx)
            }
            WireExpListOp::RemoveByIndexRange { return_type, index } => {
                let index = build(index, caps)?;
                exp_lists::remove_by_index_range(list_return(*return_type), index, bin, ctx)
            }
            WireExpListOp::RemoveByIndexRangeCount { return_type, index, count } => {
                let index = build(index, caps)?;
                let count = build(count, caps)?;
                exp_lists::remove_by_index_range_count(list_return(*return_type), index, count, bin, ctx)
            }
            WireExpListOp::RemoveByRank { return_type, rank } => {
                let rank = build(rank, caps)?;
                exp_lists::remove_by_rank(list_return(*return_type), rank, bin, ctx)
            }
            WireExpListOp::RemoveByRankRange { return_type, rank } => {
                let rank = build(rank, caps)?;
                exp_lists::remove_by_rank_range(list_return(*return_type), rank, bin, ctx)
            }
            WireExpListOp::RemoveByRankRangeCount { return_type, rank, count } => {
                let rank = build(rank, caps)?;
                let count = build(count, caps)?;
                exp_lists::remove_by_rank_range_count(list_return(*return_type), rank, count, bin, ctx)
            }
            WireExpListOp::Size => {
                exp_lists::size(bin, ctx)
            }
            WireExpListOp::GetByValue { return_type, value } => {
                let value = build(value, caps)?;
                exp_lists::get_by_value(list_return(*return_type), value, bin, ctx)
            }
            WireExpListOp::GetByValueRange { return_type, value_begin, value_end } => {
                let value_begin = value_begin.as_ref().map(|e| build(e, caps)).transpose()?;
                let value_end = value_end.as_ref().map(|e| build(e, caps)).transpose()?;
                exp_lists::get_by_value_range(list_return(*return_type), value_begin, value_end, bin, ctx)
            }
            WireExpListOp::GetByValueList { return_type, values } => {
                let values = build(values, caps)?;
                exp_lists::get_by_value_list(list_return(*return_type), values, bin, ctx)
            }
            WireExpListOp::GetByValueRelRankRange { return_type, value, rank } => {
                let value = build(value, caps)?;
                let rank = build(rank, caps)?;
                exp_lists::get_by_value_relative_rank_range(list_return(*return_type), value, rank, bin, ctx)
            }
            WireExpListOp::GetByValueRelRankRangeCount { return_type, value, rank, count } => {
                let value = build(value, caps)?;
                let rank = build(rank, caps)?;
                let count = build(count, caps)?;
                exp_lists::get_by_value_relative_rank_range_count(list_return(*return_type), value, rank, count, bin, ctx)
            }
            WireExpListOp::GetByIndex { return_type, value_type, index } => {
                let index = build(index, caps)?;
                exp_lists::get_by_index(list_return(*return_type), exp_type_of(*value_type), index, bin, ctx)
            }
            WireExpListOp::GetByIndexRange { return_type, index } => {
                let index = build(index, caps)?;
                exp_lists::get_by_index_range(list_return(*return_type), index, bin, ctx)
            }
            WireExpListOp::GetByIndexRangeCount { return_type, index, count } => {
                let index = build(index, caps)?;
                let count = build(count, caps)?;
                exp_lists::get_by_index_range_count(list_return(*return_type), index, count, bin, ctx)
            }
            WireExpListOp::GetByRank { return_type, value_type, rank } => {
                let rank = build(rank, caps)?;
                exp_lists::get_by_rank(list_return(*return_type), exp_type_of(*value_type), rank, bin, ctx)
            }
            WireExpListOp::GetByRankRange { return_type, rank } => {
                let rank = build(rank, caps)?;
                exp_lists::get_by_rank_range(list_return(*return_type), rank, bin, ctx)
            }
            WireExpListOp::GetByRankRangeCount { return_type, rank, count } => {
                let rank = build(rank, caps)?;
                let count = build(count, caps)?;
                exp_lists::get_by_rank_range_count(list_return(*return_type), rank, count, bin, ctx)
            }
    })
}

/// Build a map expression. As [`build_list`], for maps.
fn build_map(node: &WireExpMap, caps: &Capabilities) -> Result<Expression, OpError> {
    let bin = build(&node.bin, caps)?;
    let ctx = to_contexts(&node.ctx, caps).map_err(|e| {
        OpError::Expression(format!("that map expression's context path is not usable: {e}"))
    })?;
    let ctx = ctx.as_slice();
    Ok(match &node.op {
            WireExpMapOp::Put { policy, key, value } => {
                let key = build(key, caps)?;
                let value = build(value, caps)?;
                exp_maps::put(&map_policy(*policy), key, value, bin, ctx)
            }
            WireExpMapOp::PutItems { policy, map } => {
                let map = build(map, caps)?;
                exp_maps::put_items(&map_policy(*policy), map, bin, ctx)
            }
            WireExpMapOp::Increment { policy, key, incr } => {
                let key = build(key, caps)?;
                let incr = build(incr, caps)?;
                exp_maps::increment(&map_policy(*policy), key, incr, bin, ctx)
            }
            WireExpMapOp::Clear => {
                exp_maps::clear(bin, ctx)
            }
            WireExpMapOp::RemoveByKey { return_type, key } => {
                let key = build(key, caps)?;
                exp_maps::remove_by_key(map_return(*return_type), key, bin, ctx)
            }
            WireExpMapOp::RemoveByKeyList { return_type, keys } => {
                let keys = build(keys, caps)?;
                exp_maps::remove_by_key_list(map_return(*return_type), keys, bin, ctx)
            }
            WireExpMapOp::RemoveByKeyRange { return_type, key_begin, key_end } => {
                let key_begin = key_begin.as_ref().map(|e| build(e, caps)).transpose()?;
                let key_end = key_end.as_ref().map(|e| build(e, caps)).transpose()?;
                exp_maps::remove_by_key_range(map_return(*return_type), key_begin, key_end, bin, ctx)
            }
            WireExpMapOp::RemoveByKeyRelIndexRange { return_type, key, index } => {
                let key = build(key, caps)?;
                let index = build(index, caps)?;
                exp_maps::remove_by_key_relative_index_range(map_return(*return_type), key, index, bin, ctx)
            }
            WireExpMapOp::RemoveByKeyRelIndexRangeCount { return_type, key, index, count } => {
                let key = build(key, caps)?;
                let index = build(index, caps)?;
                let count = build(count, caps)?;
                exp_maps::remove_by_key_relative_index_range_count(map_return(*return_type), key, index, count, bin, ctx)
            }
            WireExpMapOp::RemoveByValue { return_type, value } => {
                let value = build(value, caps)?;
                exp_maps::remove_by_value(map_return(*return_type), value, bin, ctx)
            }
            WireExpMapOp::RemoveByValueList { return_type, values } => {
                let values = build(values, caps)?;
                exp_maps::remove_by_value_list(map_return(*return_type), values, bin, ctx)
            }
            WireExpMapOp::RemoveByValueRange { return_type, value_begin, value_end } => {
                let value_begin = value_begin.as_ref().map(|e| build(e, caps)).transpose()?;
                let value_end = value_end.as_ref().map(|e| build(e, caps)).transpose()?;
                exp_maps::remove_by_value_range(map_return(*return_type), value_begin, value_end, bin, ctx)
            }
            WireExpMapOp::RemoveByValueRelRankRange { return_type, value, rank } => {
                let value = build(value, caps)?;
                let rank = build(rank, caps)?;
                exp_maps::remove_by_value_relative_rank_range(map_return(*return_type), value, rank, bin, ctx)
            }
            WireExpMapOp::RemoveByValueRelRankRangeCount { return_type, value, rank, count } => {
                let value = build(value, caps)?;
                let rank = build(rank, caps)?;
                let count = build(count, caps)?;
                exp_maps::remove_by_value_relative_rank_range_count(map_return(*return_type), value, rank, count, bin, ctx)
            }
            WireExpMapOp::RemoveByIndex { return_type, index } => {
                let index = build(index, caps)?;
                exp_maps::remove_by_index(map_return(*return_type), index, bin, ctx)
            }
            WireExpMapOp::RemoveByIndexRange { return_type, index } => {
                let index = build(index, caps)?;
                exp_maps::remove_by_index_range(map_return(*return_type), index, bin, ctx)
            }
            WireExpMapOp::RemoveByIndexRangeCount { return_type, index, count } => {
                let index = build(index, caps)?;
                let count = build(count, caps)?;
                exp_maps::remove_by_index_range_count(map_return(*return_type), index, count, bin, ctx)
            }
            WireExpMapOp::RemoveByRank { return_type, rank } => {
                let rank = build(rank, caps)?;
                exp_maps::remove_by_rank(map_return(*return_type), rank, bin, ctx)
            }
            WireExpMapOp::RemoveByRankRange { return_type, rank } => {
                let rank = build(rank, caps)?;
                exp_maps::remove_by_rank_range(map_return(*return_type), rank, bin, ctx)
            }
            WireExpMapOp::RemoveByRankRangeCount { return_type, rank, count } => {
                let rank = build(rank, caps)?;
                let count = build(count, caps)?;
                exp_maps::remove_by_rank_range_count(map_return(*return_type), rank, count, bin, ctx)
            }
            WireExpMapOp::Size => {
                exp_maps::size(bin, ctx)
            }
            WireExpMapOp::GetByKey { return_type, value_type, key } => {
                let key = build(key, caps)?;
                exp_maps::get_by_key(map_return(*return_type), exp_type_of(*value_type), key, bin, ctx)
            }
            WireExpMapOp::GetByKeyRange { return_type, key_begin, key_end } => {
                let key_begin = key_begin.as_ref().map(|e| build(e, caps)).transpose()?;
                let key_end = key_end.as_ref().map(|e| build(e, caps)).transpose()?;
                exp_maps::get_by_key_range(map_return(*return_type), key_begin, key_end, bin, ctx)
            }
            WireExpMapOp::GetByKeyList { return_type, keys } => {
                let keys = build(keys, caps)?;
                exp_maps::get_by_key_list(map_return(*return_type), keys, bin, ctx)
            }
            WireExpMapOp::GetByKeyRelIndexRange { return_type, key, index } => {
                let key = build(key, caps)?;
                let index = build(index, caps)?;
                exp_maps::get_by_key_relative_index_range(map_return(*return_type), key, index, bin, ctx)
            }
            WireExpMapOp::GetByKeyRelIndexRangeCount { return_type, key, index, count } => {
                let key = build(key, caps)?;
                let index = build(index, caps)?;
                let count = build(count, caps)?;
                exp_maps::get_by_key_relative_index_range_count(map_return(*return_type), key, index, count, bin, ctx)
            }
            WireExpMapOp::GetByValue { return_type, value } => {
                let value = build(value, caps)?;
                exp_maps::get_by_value(map_return(*return_type), value, bin, ctx)
            }
            WireExpMapOp::GetByValueRange { return_type, value_begin, value_end } => {
                let value_begin = value_begin.as_ref().map(|e| build(e, caps)).transpose()?;
                let value_end = value_end.as_ref().map(|e| build(e, caps)).transpose()?;
                exp_maps::get_by_value_range(map_return(*return_type), value_begin, value_end, bin, ctx)
            }
            WireExpMapOp::GetByValueList { return_type, values } => {
                let values = build(values, caps)?;
                exp_maps::get_by_value_list(map_return(*return_type), values, bin, ctx)
            }
            WireExpMapOp::GetByValueRelRankRange { return_type, value, rank } => {
                let value = build(value, caps)?;
                let rank = build(rank, caps)?;
                exp_maps::get_by_value_relative_rank_range(map_return(*return_type), value, rank, bin, ctx)
            }
            WireExpMapOp::GetByValueRelRankRangeCount { return_type, value, rank, count } => {
                let value = build(value, caps)?;
                let rank = build(rank, caps)?;
                let count = build(count, caps)?;
                exp_maps::get_by_value_relative_rank_range_count(map_return(*return_type), value, rank, count, bin, ctx)
            }
            WireExpMapOp::GetByIndex { return_type, value_type, index } => {
                let index = build(index, caps)?;
                exp_maps::get_by_index(map_return(*return_type), exp_type_of(*value_type), index, bin, ctx)
            }
            WireExpMapOp::GetByIndexRange { return_type, index } => {
                let index = build(index, caps)?;
                exp_maps::get_by_index_range(map_return(*return_type), index, bin, ctx)
            }
            WireExpMapOp::GetByIndexRangeCount { return_type, index, count } => {
                let index = build(index, caps)?;
                let count = build(count, caps)?;
                exp_maps::get_by_index_range_count(map_return(*return_type), index, count, bin, ctx)
            }
            WireExpMapOp::GetByRank { return_type, value_type, rank } => {
                let rank = build(rank, caps)?;
                exp_maps::get_by_rank(map_return(*return_type), exp_type_of(*value_type), rank, bin, ctx)
            }
            WireExpMapOp::GetByRankRange { return_type, rank } => {
                let rank = build(rank, caps)?;
                exp_maps::get_by_rank_range(map_return(*return_type), rank, bin, ctx)
            }
            WireExpMapOp::GetByRankRangeCount { return_type, rank, count } => {
                let rank = build(rank, caps)?;
                let count = build(count, caps)?;
                exp_maps::get_by_rank_range_count(map_return(*return_type), rank, count, bin, ctx)
            }
    })
}

/// Build a bitwise expression.
fn build_bit(node: &WireExpBit, caps: &Capabilities) -> Result<Expression, OpError> {
    let bin = build(&node.bin, caps)?;
    Ok(match &node.op {
            WireExpBitOp::Resize { policy, byte_size, resize_flags } => {
                let byte_size = build(byte_size, caps)?;
                exp_bits::resize(&bit_policy(*policy), byte_size, bit_resize(*resize_flags), bin)
            }
            WireExpBitOp::Insert { policy, byte_offset, value } => {
                let byte_offset = build(byte_offset, caps)?;
                let value = build(value, caps)?;
                exp_bits::insert(&bit_policy(*policy), byte_offset, value, bin)
            }
            WireExpBitOp::Remove { policy, byte_offset, byte_size } => {
                let byte_offset = build(byte_offset, caps)?;
                let byte_size = build(byte_size, caps)?;
                exp_bits::remove(&bit_policy(*policy), byte_offset, byte_size, bin)
            }
            WireExpBitOp::Set { policy, bit_offset, bit_size, value } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let value = build(value, caps)?;
                exp_bits::set(&bit_policy(*policy), bit_offset, bit_size, value, bin)
            }
            WireExpBitOp::Or { policy, bit_offset, bit_size, value } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let value = build(value, caps)?;
                exp_bits::or(&bit_policy(*policy), bit_offset, bit_size, value, bin)
            }
            WireExpBitOp::Xor { policy, bit_offset, bit_size, value } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let value = build(value, caps)?;
                exp_bits::xor(&bit_policy(*policy), bit_offset, bit_size, value, bin)
            }
            WireExpBitOp::And { policy, bit_offset, bit_size, value } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let value = build(value, caps)?;
                exp_bits::and(&bit_policy(*policy), bit_offset, bit_size, value, bin)
            }
            WireExpBitOp::Not { policy, bit_offset, bit_size } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                exp_bits::not(&bit_policy(*policy), bit_offset, bit_size, bin)
            }
            WireExpBitOp::Lshift { policy, bit_offset, bit_size, shift } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let shift = build(shift, caps)?;
                exp_bits::lshift(&bit_policy(*policy), bit_offset, bit_size, shift, bin)
            }
            WireExpBitOp::Rshift { policy, bit_offset, bit_size, shift } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let shift = build(shift, caps)?;
                exp_bits::rshift(&bit_policy(*policy), bit_offset, bit_size, shift, bin)
            }
            WireExpBitOp::Add { policy, bit_offset, bit_size, value, signed, action } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let value = build(value, caps)?;
                exp_bits::add(&bit_policy(*policy), bit_offset, bit_size, value, *signed, bit_overflow(*action), bin)
            }
            WireExpBitOp::Subtract { policy, bit_offset, bit_size, value, signed, action } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let value = build(value, caps)?;
                exp_bits::subtract(&bit_policy(*policy), bit_offset, bit_size, value, *signed, bit_overflow(*action), bin)
            }
            WireExpBitOp::SetInt { policy, bit_offset, bit_size, value } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let value = build(value, caps)?;
                exp_bits::set_int(&bit_policy(*policy), bit_offset, bit_size, value, bin)
            }
            WireExpBitOp::Get { bit_offset, bit_size } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                exp_bits::get(bit_offset, bit_size, bin)
            }
            WireExpBitOp::Count { bit_offset, bit_size } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                exp_bits::count(bit_offset, bit_size, bin)
            }
            WireExpBitOp::Lscan { bit_offset, bit_size, value } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let value = build(value, caps)?;
                exp_bits::lscan(bit_offset, bit_size, value, bin)
            }
            WireExpBitOp::Rscan { bit_offset, bit_size, value } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                let value = build(value, caps)?;
                exp_bits::rscan(bit_offset, bit_size, value, bin)
            }
            WireExpBitOp::GetInt { bit_offset, bit_size, signed } => {
                let bit_offset = build(bit_offset, caps)?;
                let bit_size = build(bit_size, caps)?;
                exp_bits::get_int(bit_offset, bit_size, *signed, bin)
            }
    })
}

/// Build a HyperLogLog expression.
fn build_hll(node: &WireExpHll, caps: &Capabilities) -> Result<Expression, OpError> {
    let bin = build(&node.bin, caps)?;
    Ok(match &node.op {
            WireExpHllOp::Init { policy, index_bit_count } => {
                let index_bit_count = build(index_bit_count, caps)?;
                exp_hll::init(hll_policy(*policy), index_bit_count, bin)
            }
            WireExpHllOp::InitWithMinHash { policy, index_bit_count, min_hash_count } => {
                let index_bit_count = build(index_bit_count, caps)?;
                let min_hash_count = build(min_hash_count, caps)?;
                exp_hll::init_with_min_hash(hll_policy(*policy), index_bit_count, min_hash_count, bin)
            }
            WireExpHllOp::Add { policy, list } => {
                let list = build(list, caps)?;
                exp_hll::add(hll_policy(*policy), list, bin)
            }
            WireExpHllOp::AddWithIndex { policy, list, index_bit_count } => {
                let list = build(list, caps)?;
                let index_bit_count = build(index_bit_count, caps)?;
                exp_hll::add_with_index(hll_policy(*policy), list, index_bit_count, bin)
            }
            WireExpHllOp::AddWithIndexAndMinHash { policy, list, index_bit_count, min_hash_count } => {
                let list = build(list, caps)?;
                let index_bit_count = build(index_bit_count, caps)?;
                let min_hash_count = build(min_hash_count, caps)?;
                exp_hll::add_with_index_and_min_hash(hll_policy(*policy), list, index_bit_count, min_hash_count, bin)
            }
            WireExpHllOp::GetCount => {
                exp_hll::get_count(bin)
            }
            WireExpHllOp::GetUnion { list } => {
                let list = build(list, caps)?;
                exp_hll::get_union(list, bin)
            }
            WireExpHllOp::GetUnionCount { list } => {
                let list = build(list, caps)?;
                exp_hll::get_union_count(list, bin)
            }
            WireExpHllOp::GetIntersectCount { list } => {
                let list = build(list, caps)?;
                exp_hll::get_intersect_count(list, bin)
            }
            WireExpHllOp::GetSimilarity { list } => {
                let list = build(list, caps)?;
                exp_hll::get_similarity(list, bin)
            }
            WireExpHllOp::Describe => {
                exp_hll::describe(bin)
            }
            WireExpHllOp::MayContain { list } => {
                let list = build(list, caps)?;
                exp_hll::may_contain(list, bin)
            }
    })
}

/// Build a string expression.
///
/// The only family with a version requirement: these are the string operations,
/// which need server 8.1.3 or later. Gated here rather than at each of the
/// forty-two constructors — the refusal is the same for all of them, and an older
/// server's own answer says nothing about why.
fn build_str(node: &WireExpStr, caps: &Capabilities) -> Result<Expression, OpError> {
    caps.strings.require()?;
    let src = build(&node.src, caps)?;
    Ok(match &node.op {
            WireExpStrOp::Strlen => {
                exp_strings::strlen(src)
            }
            WireExpStrOp::ByteLength => {
                exp_strings::byte_length(src)
            }
            WireExpStrOp::CharAt { index } => {
                let index = build(index, caps)?;
                exp_strings::char_at(index, src)
            }
            WireExpStrOp::Substr { start } => {
                let start = build(start, caps)?;
                exp_strings::substr(start, src)
            }
            WireExpStrOp::SubstrRange { start, end } => {
                let start = build(start, caps)?;
                let end = build(end, caps)?;
                exp_strings::substr_range(start, end, src)
            }
            WireExpStrOp::Find { needle } => {
                let needle = build(needle, caps)?;
                exp_strings::find(needle, src)
            }
            WireExpStrOp::FindNth { needle, occurrence } => {
                let needle = build(needle, caps)?;
                let occurrence = build(occurrence, caps)?;
                exp_strings::find_nth(needle, occurrence, src)
            }
            WireExpStrOp::Contains { needle } => {
                let needle = build(needle, caps)?;
                exp_strings::contains(needle, src)
            }
            WireExpStrOp::StartsWith { prefix } => {
                let prefix = build(prefix, caps)?;
                exp_strings::starts_with(prefix, src)
            }
            WireExpStrOp::EndsWith { suffix } => {
                let suffix = build(suffix, caps)?;
                exp_strings::ends_with(suffix, src)
            }
            WireExpStrOp::IsUpper => {
                exp_strings::is_upper(src)
            }
            WireExpStrOp::IsLower => {
                exp_strings::is_lower(src)
            }
            WireExpStrOp::IsNumeric => {
                exp_strings::is_numeric(src)
            }
            WireExpStrOp::IsNumericTyped { numeric_type } => {
                exp_strings::is_numeric_typed(numeric_type_of(*numeric_type), src)
            }
            WireExpStrOp::ToInteger => {
                exp_strings::to_integer(src)
            }
            WireExpStrOp::ToDouble => {
                exp_strings::to_double(src)
            }
            WireExpStrOp::ToBlob => {
                exp_strings::to_blob(src)
            }
            WireExpStrOp::ToString => {
                exp_strings::to_string(src)
            }
            WireExpStrOp::B64Decode => {
                exp_strings::b64_decode(src)
            }
            WireExpStrOp::Split => {
                exp_strings::split(src)
            }
            WireExpStrOp::SplitBySeparator { separator } => {
                let separator = build(separator, caps)?;
                exp_strings::split_by_separator(separator, src)
            }
            WireExpStrOp::RegexCompare { pattern } => {
                let pattern = build(pattern, caps)?;
                exp_strings::regex_compare(pattern, src)
            }
            WireExpStrOp::RegexCompareWithFlags { pattern, regex_flags } => {
                let pattern = build(pattern, caps)?;
                exp_strings::regex_compare_with_flags(pattern, StringRegexFlags(*regex_flags), src)
            }
            WireExpStrOp::Insert { policy, index, value } => {
                let index = build(index, caps)?;
                let value = build(value, caps)?;
                exp_strings::insert(&string_policy(*policy), index, value, src)
            }
            WireExpStrOp::Overwrite { policy, index, value } => {
                let index = build(index, caps)?;
                let value = build(value, caps)?;
                exp_strings::overwrite(&string_policy(*policy), index, value, src)
            }
            WireExpStrOp::Concat { policy, values } => {
                let values = build(values, caps)?;
                exp_strings::concat(&string_policy(*policy), values, src)
            }
            WireExpStrOp::Append { policy, value } => {
                let value = build(value, caps)?;
                exp_strings::append(&string_policy(*policy), value, src)
            }
            WireExpStrOp::Prepend { policy, value } => {
                let value = build(value, caps)?;
                exp_strings::prepend(&string_policy(*policy), value, src)
            }
            WireExpStrOp::Snip { policy, start, end } => {
                let start = build(start, caps)?;
                let end = build(end, caps)?;
                exp_strings::snip(&string_policy(*policy), start, end, src)
            }
            WireExpStrOp::Replace { policy, needle, replacement } => {
                let needle = build(needle, caps)?;
                let replacement = build(replacement, caps)?;
                exp_strings::replace(&string_policy(*policy), needle, replacement, src)
            }
            WireExpStrOp::ReplaceAll { policy, needle, replacement } => {
                let needle = build(needle, caps)?;
                let replacement = build(replacement, caps)?;
                exp_strings::replace_all(&string_policy(*policy), needle, replacement, src)
            }
            WireExpStrOp::Upper { policy } => {
                exp_strings::upper(&string_policy(*policy), src)
            }
            WireExpStrOp::Lower { policy } => {
                exp_strings::lower(&string_policy(*policy), src)
            }
            WireExpStrOp::CaseFold { policy } => {
                exp_strings::case_fold(&string_policy(*policy), src)
            }
            WireExpStrOp::NormalizeNfc { policy } => {
                exp_strings::normalize_nfc(&string_policy(*policy), src)
            }
            WireExpStrOp::TrimStart { policy } => {
                exp_strings::trim_start(&string_policy(*policy), src)
            }
            WireExpStrOp::TrimEnd { policy } => {
                exp_strings::trim_end(&string_policy(*policy), src)
            }
            WireExpStrOp::Trim { policy } => {
                exp_strings::trim(&string_policy(*policy), src)
            }
            WireExpStrOp::PadStart { policy, target_length, pad_string } => {
                let target_length = build(target_length, caps)?;
                let pad_string = build(pad_string, caps)?;
                exp_strings::pad_start(&string_policy(*policy), target_length, pad_string, src)
            }
            WireExpStrOp::PadEnd { policy, target_length, pad_string } => {
                let target_length = build(target_length, caps)?;
                let pad_string = build(pad_string, caps)?;
                exp_strings::pad_end(&string_policy(*policy), target_length, pad_string, src)
            }
            WireExpStrOp::Repeat { policy, count } => {
                let count = build(count, caps)?;
                exp_strings::repeat(&string_policy(*policy), count, src)
            }
            WireExpStrOp::RegexReplace { policy, pattern, replacement, regex_flags } => {
                let pattern = build(pattern, caps)?;
                let replacement = build(replacement, caps)?;
                exp_strings::regex_replace(&string_policy(*policy), pattern, replacement, StringRegexFlags(*regex_flags), src)
            }
    })
}
/// Whether a cluster can run the string operations.
///
/// The same shape as [`crate::policy::AelSupport`] and for the same reason: the
/// string expressions are the string *operations*, which need server 8.1.3, and an
/// older server answers a request carrying one with a parameter error that says
/// nothing about strings. Asked once per request and passed down, because the
/// answer costs a clone of the cluster's node list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringSupport {
    /// Every node is new enough, or no string expression was asked for.
    Supported,
    /// At least one node is not, named so the message can say which.
    Unsupported {
        /// The node that is too old.
        node: String,
        /// The version it reported.
        version: String,
    },
    /// There is no cluster view to ask.
    Unknown,
}

impl StringSupport {
    /// Ask `client`'s current cluster view.
    ///
    /// All nodes, since the expression travels with the command to whichever node
    /// owns the record — one old node makes the answer no.
    #[must_use]
    pub fn of(client: &aerospike_core::Client) -> StringSupport {
        let nodes = client.nodes();
        if nodes.is_empty() {
            return StringSupport::Unknown;
        }
        for node in nodes {
            // The client's own gate, so the two cannot drift.
            if !node.version().supports_string_operations() {
                let v = node.version();
                return StringSupport::Unsupported {
                    node: node.name().to_string(),
                    version: format!("{}.{}.{}.{}", v.major, v.minor, v.patch, v.build),
                };
            }
        }
        StringSupport::Supported
    }

    fn require(&self) -> Result<(), OpError> {
        match self {
            StringSupport::Supported => Ok(()),
            StringSupport::Unsupported { node, version } => Err(OpError::Unsupported(format!(
                "the string expressions need Aerospike 8.1.3 or later, but node '{node}' runs \
                 {version}; upgrade the cluster or work on the string another way"
            ))),
            StringSupport::Unknown => Err(OpError::Unsupported(
                "the string expressions need a server of 8.1.3 or later, and this instance has no \
                 connected node to check; retry once the cluster is reachable"
                    .to_string(),
            )),
        }
    }
}

/// Whether a cluster can walk a CDT path expression.
///
/// The fan-out step is a server capability — the server visits every child — so a
/// path expression needs server **8.1.1 or later**. The same shape as
/// [`StringSupport`], and gated at the one place path expressions are built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSupport {
    /// Every node is new enough, or no path expression was asked for.
    Supported,
    /// At least one node is not, named so the message can say which.
    Unsupported {
        /// The node that is too old.
        node: String,
        /// The version it reported.
        version: String,
    },
    /// There is no cluster view to ask.
    Unknown,
}

impl PathSupport {
    /// Ask `client`'s current cluster view.
    #[must_use]
    pub fn of(client: &aerospike_core::Client) -> PathSupport {
        let nodes = client.nodes();
        if nodes.is_empty() {
            return PathSupport::Unknown;
        }
        for node in nodes {
            if !node.version().supports_cdt_path_expressions() {
                let v = node.version();
                return PathSupport::Unsupported {
                    node: node.name().to_string(),
                    version: format!("{}.{}.{}.{}", v.major, v.minor, v.patch, v.build),
                };
            }
        }
        PathSupport::Supported
    }

    /// Refuse unless every node can walk a fan-out.
    ///
    /// `pub(crate)` because [`crate::ops::to_contexts`] is where a fan-out is built,
    /// and the gate belongs where the thing it gates is.
    pub(crate) fn require(&self) -> Result<(), OpError> {
        match self {
            PathSupport::Supported => Ok(()),
            PathSupport::Unsupported { node, version } => Err(OpError::Unsupported(format!(
                "a CDT path expression needs Aerospike 8.1.1 or later to walk its fan-out, but \
                 node '{node}' runs {version}; upgrade the cluster, or address one node at a time \
                 with an ordinary collection expression"
            ))),
            PathSupport::Unknown => Err(OpError::Unsupported(
                "a CDT path expression needs a server of 8.1.1 or later, and this instance has no \
                 connected node to check; retry once the cluster is reachable"
                    .to_string(),
            )),
        }
    }
}

/// The contract's numeric-type filter, as the client's.
const fn numeric_type_of(numeric_type: WireStringNumericType) -> StringNumericType {
    match numeric_type {
        WireStringNumericType::Any => StringNumericType::Any,
        WireStringNumericType::Int => StringNumericType::Int,
        WireStringNumericType::Float => StringNumericType::Float,
    }
}

/// The contract's string write flags, as the client's policy.
fn string_policy(policy: WireStringPolicy) -> StringPolicy {
    StringPolicy::new(if policy.no_fail {
        StringWriteFlags::NO_FAIL
    } else {
        StringWriteFlags::DEFAULT
    })
}

/// Build a CDT path expression.
///
/// The gate is on the *context*, not here: a path expression is only special
/// because its context contains a fan-out, and [`crate::ops::to_contexts`] refuses
/// that against a cluster older than 8.1.1. So this needs no version check of its
/// own — building the context already did it.
fn build_path(node: &WireExpPath, caps: &Capabilities) -> Result<Expression, OpError> {
    let bin = build(&node.bin, caps)?;
    let ctx = to_contexts(&node.ctx, caps)?;
    let ctx = ctx.as_slice();
    let return_type = exp_type_of(node.return_type);
    Ok(match &node.op {
        WireExpPathOp::SelectByPath { flag } => {
            exps::exp_select_by_path(return_type, SelectFlag(*flag), bin, ctx)
        }
        WireExpPathOp::SelectValues => exps::exp_select_values(return_type, bin, ctx),
        WireExpPathOp::SelectMapKeys => exps::exp_select_map_keys(return_type, bin, ctx),
        WireExpPathOp::SelectMapEntries => exps::exp_select_map_entries(return_type, bin, ctx),
        WireExpPathOp::SelectMatchingTree => {
            exps::exp_select_matching_tree(return_type, bin, ctx)
        }
        WireExpPathOp::ModifyByPath { flag, modify } => exps::exp_modify_by_path(
            return_type,
            ModifyFlag(*flag),
            bin,
            build(modify, caps)?,
            ctx,
        ),
        WireExpPathOp::Modify { modify } => {
            exps::exp_modify(return_type, bin, build(modify, caps)?, ctx)
        }
        WireExpPathOp::ModifyNoFail { modify } => {
            exps::exp_modify_no_fail(return_type, bin, build(modify, caps)?, ctx)
        }
        WireExpPathOp::Remove => exps::exp_remove(return_type, bin, ctx),
    })
}

/// A loop variable, read as `exp_type`.
///
/// The client has one constructor per type rather than one taking an `ExpType`, so
/// this is the mapping — and the reason it is a `match` here instead of a generic
/// call is that those ten functions are the only way in.
fn loop_var(exp_type: WireExpType, part: WireLoopVarPart) -> Expression {
    let part = LoopVarPart(part.0);
    match exp_type {
        WireExpType::Nil => exps::exp_nil_loop_var(part),
        WireExpType::Bool => exps::exp_bool_loop_var(part),
        WireExpType::Int => exps::exp_int_loop_var(part),
        WireExpType::Str => exps::exp_string_loop_var(part),
        WireExpType::List => exps::exp_list_loop_var(part),
        WireExpType::Map => exps::exp_map_loop_var(part),
        WireExpType::Blob => exps::exp_blob_loop_var(part),
        WireExpType::Float => exps::exp_float_loop_var(part),
        WireExpType::Geo => exps::exp_geo_json_loop_var(part),
        WireExpType::Hll => exps::exp_hll_loop_var(part),
    }
}

/// An expression literal.
///
/// Dispatched on the wire shape rather than built from a converted [`Value`]:
/// `Expression`'s fields are private, so the only way in is the client's own
/// constructors, and they differ per shape — a list literal is packed *quoted*
/// where a scalar is not, and the three map kinds each carry their own ordering.
/// Feeding one constructor a converted `Value` would lose exactly that.
fn literal(value: &WireValue) -> Result<Expression, OpError> {
    Ok(match value {
        WireValue::Nil => exps::nil(),
        WireValue::Bool(b) => exps::bool_val(*b),
        WireValue::Int(i) => exps::int_val(*i),
        WireValue::Float(f) => exps::float_val(*f),
        WireValue::Str(s) => exps::string_val(s.clone()),
        WireValue::Blob(b) => exps::blob_val(b.clone()),
        WireValue::GeoJson(json) => exps::geo_val(json.clone()),
        WireValue::Infinity => exps::infinity(),
        WireValue::Wildcard => exps::wildcard(),

        WireValue::List(items) => exps::list_val(
            items
                .iter()
                .map(|item| to_value(item).map_err(literal_error))
                .collect::<Result<Vec<_>, _>>()?,
        ),

        // The three map kinds are three orderings, and the ordering is what the
        // server compares by — so each goes to the collection type that carries
        // it rather than through one shared conversion.
        WireValue::Map(pairs) | WireValue::OrderedMap(pairs) => {
            let mut map = IndexMap::with_capacity(pairs.len());
            for (key, value) in pairs {
                map.insert(
                    to_value(key).map_err(literal_error)?,
                    to_value(value).map_err(literal_error)?,
                );
            }
            exps::map_val(map)
        }
        WireValue::SortedMap(pairs) => {
            let mut map = std::collections::BTreeMap::new();
            for (key, value) in pairs {
                map.insert(
                    to_value(key).map_err(literal_error)?,
                    to_value(value).map_err(literal_error)?,
                );
            }
            exps::map_val(map)
        }

        // The client has no `hll_val`, and passing the bytes as a blob literal
        // would be worse than refusing: a blob never equals an HLL bin, so the
        // comparison would silently be false rather than wrong-looking.
        WireValue::Hll(_) => {
            return Err(OpError::Expression(
                "a HyperLogLog sketch cannot be an expression literal: the server compares it by \
                 particle type, and a blob of the same bytes is not equal to an HLL bin. Read the \
                 bin with an HLL expression instead — getCount, getUnion, mayContain and the rest \
                 all take the bin"
                    .to_string(),
            ));
        }

        // Reply-only shapes. `to_value` refuses them too; naming them here is
        // what makes the message about expressions rather than about bins.
        WireValue::MultiResult(_) | WireValue::KeyValueList(_) | WireValue::Unknown { .. } => {
            return Err(OpError::Expression(format!(
                "that value shape only ever comes back from the server and cannot be part of an \
                 expression: {}",
                shape_of(value)
            )));
        }
    })
}

/// A converted-value failure, as an expression failure.
fn literal_error(e: impl std::fmt::Display) -> OpError {
    OpError::Expression(format!("that expression literal is not usable: {e}"))
}

/// A name for a value shape, for the refusal above.
fn shape_of(value: &WireValue) -> &'static str {
    match value {
        WireValue::MultiResult(_) => "a multi-operation result",
        WireValue::KeyValueList(_) => "a key/value result list",
        WireValue::Unknown { .. } => "a value of a type this client does not model",
        _ => "that shape",
    }
}

/// The contract's expression type, as the client's.
const fn exp_type_of(exp_type: WireExpType) -> ExpType {
    match exp_type {
        WireExpType::Nil => ExpType::NIL,
        WireExpType::Bool => ExpType::BOOL,
        WireExpType::Int => ExpType::INT,
        WireExpType::Str => ExpType::STRING,
        WireExpType::List => ExpType::LIST,
        WireExpType::Map => ExpType::MAP,
        WireExpType::Blob => ExpType::BLOB,
        WireExpType::Float => ExpType::FLOAT,
        WireExpType::Geo => ExpType::GEO,
        WireExpType::Hll => ExpType::HLL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::exp::{MAX_EXP_DEPTH, MAX_EXP_NODES};
    use aerospike_php_ipc::op::{
        WireCtx, WireListPolicy, WireListReturn, WireListReturnKind, WireListSort, WireMapPolicy,
        WireMapReturn, WireMapReturnKind,
    };

    fn int_bin(name: &str) -> WireExp {
        WireExp::Bin {
            name: name.into(),
            exp_type: WireExpType::Int,
        }
    }

    fn int(value: i64) -> WireExp {
        WireExp::Value(WireValue::Int(value))
    }

    /// The expressions are opaque once built — no accessors, by design — so what
    /// these tests can assert is that every constructor is *reached* and that the
    /// packed form differs where it should. Identical bytes for two different
    /// trees would mean two wire variants collapsed onto one constructor, which is
    /// the mistake this translation can actually make.
    fn packed(exp: &WireExp) -> String {
        let built = from_tree(exp, &Capabilities::all()).expect("that tree should build");
        // `base64` is the only public view of a built expression, and it is
        // exactly the packed bytes the server would receive.
        built.base64().expect("a built expression packs")
    }

    #[test]
    fn a_filter_tree_builds() {
        let exp = WireExp::Variadic {
            op: WireExpVariadicOp::And,
            exps: vec![
                WireExp::Binary {
                    op: WireExpBinaryOp::Gt,
                    left: Box::new(int_bin("a")),
                    right: Box::new(int(5)),
                },
                WireExp::Binary {
                    op: WireExpBinaryOp::Eq,
                    left: Box::new(WireExp::Bin {
                        name: "b".into(),
                        exp_type: WireExpType::Str,
                    }),
                    right: Box::new(WireExp::Value(WireValue::Str("x".into()))),
                },
            ],
        };
        assert!(!packed(&exp).is_empty());
    }

    /// Each comparison opcode has to reach its own constructor. Six wire variants
    /// mapping to `eq` would pass a "does it build" test and be wrong on the wire.
    #[test]
    fn every_comparison_packs_differently() {
        let mut seen = std::collections::HashSet::new();
        for op in [
            WireExpBinaryOp::Eq,
            WireExpBinaryOp::Ne,
            WireExpBinaryOp::Gt,
            WireExpBinaryOp::Ge,
            WireExpBinaryOp::Lt,
            WireExpBinaryOp::Le,
        ] {
            let exp = WireExp::Binary {
                op,
                left: Box::new(int_bin("a")),
                right: Box::new(int(1)),
            };
            assert!(
                seen.insert(packed(&exp)),
                "{op:?} packed the same as an earlier comparison"
            );
        }
    }

    #[test]
    fn every_unary_and_variadic_opcode_packs_differently() {
        let mut seen = std::collections::HashSet::new();
        for op in [
            WireExpUnaryOp::Not,
            WireExpUnaryOp::NumAbs,
            WireExpUnaryOp::NumFloor,
            WireExpUnaryOp::NumCeil,
            WireExpUnaryOp::ToInt,
            WireExpUnaryOp::ToFloat,
            WireExpUnaryOp::IntNot,
            WireExpUnaryOp::IntCount,
            WireExpUnaryOp::MapKeys,
            WireExpUnaryOp::MapValues,
        ] {
            let exp = WireExp::Unary {
                op,
                exp: Box::new(int_bin("a")),
            };
            assert!(seen.insert(packed(&exp)), "{op:?} collided");
        }

        let mut seen = std::collections::HashSet::new();
        for op in [
            WireExpVariadicOp::And,
            WireExpVariadicOp::Or,
            WireExpVariadicOp::NumAdd,
            WireExpVariadicOp::NumSub,
            WireExpVariadicOp::NumMul,
            WireExpVariadicOp::NumDiv,
            WireExpVariadicOp::IntAnd,
            WireExpVariadicOp::IntOr,
            WireExpVariadicOp::IntXor,
            WireExpVariadicOp::Min,
            WireExpVariadicOp::Max,
        ] {
            let exp = WireExp::Variadic {
                op,
                exps: vec![int_bin("a"), int(1)],
            };
            assert!(seen.insert(packed(&exp)), "{op:?} collided");
        }
    }

    /// `xor` and `exclusive` are documented as the same opcode. Asserting it here
    /// is what keeps the contract's single variant honest: if the client ever gave
    /// them different opcodes, one PHP spelling would silently be wrong.
    #[test]
    fn exclusive_really_is_xor() {
        let a = exps::xor(vec![exps::int_val(1), exps::int_val(2)]);
        let b = exps::exclusive(vec![exps::int_val(1), exps::int_val(2)]);
        assert_eq!(a.base64().unwrap(), b.base64().unwrap());
    }

    #[test]
    fn every_metadata_reader_packs_differently() {
        let mut seen = std::collections::HashSet::new();
        for meta in [
            WireExpMeta::KeyExists,
            WireExpMeta::SetName,
            WireExpMeta::RecordSize,
            WireExpMeta::DeviceSize,
            WireExpMeta::MemorySize,
            WireExpMeta::LastUpdate,
            WireExpMeta::SinceUpdate,
            WireExpMeta::VoidTime,
            WireExpMeta::Ttl,
            WireExpMeta::IsTombstone,
        ] {
            assert!(
                seen.insert(packed(&WireExp::Meta(meta))),
                "{meta:?} collided"
            );
        }
    }

    /// A bin's type is part of the packed form, so ten `WireExpType`s must reach
    /// ten different `ExpType`s — a mapping that collapsed two would read a bin as
    /// the wrong type, which is a wrong answer rather than an error.
    #[test]
    fn every_expression_type_packs_differently() {
        let mut seen = std::collections::HashSet::new();
        for exp_type in [
            WireExpType::Nil,
            WireExpType::Bool,
            WireExpType::Int,
            WireExpType::Str,
            WireExpType::List,
            WireExpType::Map,
            WireExpType::Blob,
            WireExpType::Float,
            WireExpType::Geo,
            WireExpType::Hll,
        ] {
            let exp = WireExp::Bin {
                name: "b".into(),
                exp_type,
            };
            assert!(seen.insert(packed(&exp)), "{exp_type:?} collided");
        }
    }

    #[test]
    fn every_literal_shape_builds() {
        let values = [
            WireValue::Nil,
            WireValue::Bool(true),
            WireValue::Int(-1),
            WireValue::Float(1.5),
            WireValue::Str("s".into()),
            WireValue::Blob(vec![1, 2, 3]),
            WireValue::GeoJson("{\"type\":\"Point\",\"coordinates\":[0,0]}".into()),
            WireValue::Infinity,
            WireValue::Wildcard,
            WireValue::List(vec![WireValue::Int(1), WireValue::Str("a".into())]),
            WireValue::Map(vec![(WireValue::Str("k".into()), WireValue::Int(1))]),
            WireValue::OrderedMap(vec![(WireValue::Str("k".into()), WireValue::Int(1))]),
            WireValue::SortedMap(vec![(WireValue::Str("k".into()), WireValue::Int(1))]),
        ];
        for value in values {
            let exp = WireExp::Value(value.clone());
            assert!(
                from_tree(&exp, &Capabilities::all()).is_ok(),
                "{value:?} should be a usable literal"
            );
        }
    }

    /// A sorted map packs with the K-ordered flag and an unordered one does not,
    /// which is what whole-map comparison depends on. One shared conversion would
    /// lose the distinction silently.
    #[test]
    fn a_sorted_map_literal_is_not_an_unordered_one() {
        let pairs = vec![(WireValue::Str("k".into()), WireValue::Int(1))];
        let unordered = packed(&WireExp::Value(WireValue::Map(pairs.clone())));
        let sorted = packed(&WireExp::Value(WireValue::SortedMap(pairs)));
        assert_ne!(
            unordered, sorted,
            "the map orderings must not collapse onto one constructor"
        );
    }

    /// A list literal is packed *quoted* — `list_val` uses a different opcode from
    /// the scalar constructors — so a list of one element must not equal that
    /// element.
    #[test]
    fn a_list_literal_is_quoted() {
        let list = packed(&WireExp::Value(WireValue::List(vec![WireValue::Int(1)])));
        let scalar = packed(&int(1));
        assert_ne!(list, scalar);
    }

    #[test]
    fn an_hll_literal_is_refused_by_name() {
        let exp = WireExp::Value(WireValue::Hll(vec![0, 1, 2]));
        let error = from_tree(&exp, &Capabilities::all()).expect_err("there is no hll_val");
        let message = error.to_string();
        assert!(message.contains("HyperLogLog"), "{message}");
        assert!(
            message.contains("getCount") || message.contains("mayContain"),
            "the refusal should say what to use instead: {message}"
        );
    }

    #[test]
    fn a_reply_only_value_is_refused_by_name() {
        let exp = WireExp::Value(WireValue::MultiResult(vec![WireValue::Int(1)]));
        let error = from_tree(&exp, &Capabilities::all()).expect_err("a result is not a literal");
        assert!(error.to_string().contains("comes back from the server"), "{error}");
    }

    /// Thirty-one list variants must reach thirty-one different constructors. The
    /// mistake this catches is the one a table-driven translation can actually
    /// make: two variants pointing at the same core function, which builds a
    /// perfectly valid expression that does the wrong thing.
    #[test]
    fn every_list_operation_packs_differently() {
        let bin = WireExp::Bin {
            name: "l".into(),
            exp_type: WireExpType::List,
        };
        let ret = WireListReturn::of(WireListReturnKind::Values);
        let e = || int(1);
        let ops = vec![
            WireExpListOp::Append { policy: WireListPolicy::default(), value: e() },
            WireExpListOp::AppendItems { policy: WireListPolicy::default(), list: e() },
            WireExpListOp::Insert { policy: WireListPolicy::default(), index: e(), value: e() },
            WireExpListOp::InsertItems { policy: WireListPolicy::default(), index: e(), list: e() },
            WireExpListOp::Increment { policy: WireListPolicy::default(), index: e(), value: e() },
            WireExpListOp::Set { policy: WireListPolicy::default(), index: e(), value: e() },
            WireExpListOp::Clear,
            WireExpListOp::Sort { flags: WireListSort { descending: false, drop_duplicates: false } },
            WireExpListOp::RemoveByValue { return_type: ret, value: e() },
            WireExpListOp::RemoveByValueList { return_type: ret, values: e() },
            WireExpListOp::RemoveByValueRange { return_type: ret, value_begin: Some(e()), value_end: Some(e()) },
            WireExpListOp::RemoveByValueRelRankRange { return_type: ret, value: e(), rank: e() },
            WireExpListOp::RemoveByValueRelRankRangeCount { return_type: ret, value: e(), rank: e(), count: e() },
            WireExpListOp::RemoveByIndex { return_type: ret, index: e() },
            WireExpListOp::RemoveByIndexRange { return_type: ret, index: e() },
            WireExpListOp::RemoveByIndexRangeCount { return_type: ret, index: e(), count: e() },
            WireExpListOp::RemoveByRank { return_type: ret, rank: e() },
            WireExpListOp::RemoveByRankRange { return_type: ret, rank: e() },
            WireExpListOp::RemoveByRankRangeCount { return_type: ret, rank: e(), count: e() },
            WireExpListOp::Size,
            WireExpListOp::GetByValue { return_type: ret, value: e() },
            WireExpListOp::GetByValueRange { return_type: ret, value_begin: Some(e()), value_end: Some(e()) },
            WireExpListOp::GetByValueList { return_type: ret, values: e() },
            WireExpListOp::GetByValueRelRankRange { return_type: ret, value: e(), rank: e() },
            WireExpListOp::GetByValueRelRankRangeCount { return_type: ret, value: e(), rank: e(), count: e() },
            WireExpListOp::GetByIndex { return_type: ret, value_type: WireExpType::Int, index: e() },
            WireExpListOp::GetByIndexRange { return_type: ret, index: e() },
            WireExpListOp::GetByIndexRangeCount { return_type: ret, index: e(), count: e() },
            WireExpListOp::GetByRank { return_type: ret, value_type: WireExpType::Int, rank: e() },
            WireExpListOp::GetByRankRange { return_type: ret, rank: e() },
            WireExpListOp::GetByRankRangeCount { return_type: ret, rank: e(), count: e() },
        ];
        assert_eq!(ops.len(), 31, "every list constructor the client has");

        let mut seen = std::collections::HashMap::new();
        for op in ops {
            let label = format!("{op:?}");
            let exp = WireExp::List(Box::new(WireExpList {
                op,
                bin: bin.clone(),
                ctx: vec![],
            }));
            if let Some(earlier) = seen.insert(packed(&exp), label.clone()) {
                panic!("{label} packed the same as {earlier}");
            }
        }
    }

    /// The same for the thirty-seven map variants.
    #[test]
    fn every_map_operation_packs_differently() {
        let bin = WireExp::Bin {
            name: "m".into(),
            exp_type: WireExpType::Map,
        };
        let ret = WireMapReturn::of(WireMapReturnKind::Value);
        let pol = WireMapPolicy::default();
        let e = || int(1);
        let ops = vec![
            WireExpMapOp::Put { policy: pol, key: e(), value: e() },
            WireExpMapOp::PutItems { policy: pol, map: e() },
            WireExpMapOp::Increment { policy: pol, key: e(), incr: e() },
            WireExpMapOp::Clear,
            WireExpMapOp::RemoveByKey { return_type: ret, key: e() },
            WireExpMapOp::RemoveByKeyList { return_type: ret, keys: e() },
            WireExpMapOp::RemoveByKeyRange { return_type: ret, key_begin: Some(e()), key_end: Some(e()) },
            WireExpMapOp::RemoveByKeyRelIndexRange { return_type: ret, key: e(), index: e() },
            WireExpMapOp::RemoveByKeyRelIndexRangeCount { return_type: ret, key: e(), index: e(), count: e() },
            WireExpMapOp::RemoveByValue { return_type: ret, value: e() },
            WireExpMapOp::RemoveByValueList { return_type: ret, values: e() },
            WireExpMapOp::RemoveByValueRange { return_type: ret, value_begin: Some(e()), value_end: Some(e()) },
            WireExpMapOp::RemoveByValueRelRankRange { return_type: ret, value: e(), rank: e() },
            WireExpMapOp::RemoveByValueRelRankRangeCount { return_type: ret, value: e(), rank: e(), count: e() },
            WireExpMapOp::RemoveByIndex { return_type: ret, index: e() },
            WireExpMapOp::RemoveByIndexRange { return_type: ret, index: e() },
            WireExpMapOp::RemoveByIndexRangeCount { return_type: ret, index: e(), count: e() },
            WireExpMapOp::RemoveByRank { return_type: ret, rank: e() },
            WireExpMapOp::RemoveByRankRange { return_type: ret, rank: e() },
            WireExpMapOp::RemoveByRankRangeCount { return_type: ret, rank: e(), count: e() },
            WireExpMapOp::Size,
            WireExpMapOp::GetByKey { return_type: ret, value_type: WireExpType::Int, key: e() },
            WireExpMapOp::GetByKeyRange { return_type: ret, key_begin: Some(e()), key_end: Some(e()) },
            WireExpMapOp::GetByKeyList { return_type: ret, keys: e() },
            WireExpMapOp::GetByKeyRelIndexRange { return_type: ret, key: e(), index: e() },
            WireExpMapOp::GetByKeyRelIndexRangeCount { return_type: ret, key: e(), index: e(), count: e() },
            WireExpMapOp::GetByValue { return_type: ret, value: e() },
            WireExpMapOp::GetByValueRange { return_type: ret, value_begin: Some(e()), value_end: Some(e()) },
            WireExpMapOp::GetByValueList { return_type: ret, values: e() },
            WireExpMapOp::GetByValueRelRankRange { return_type: ret, value: e(), rank: e() },
            WireExpMapOp::GetByValueRelRankRangeCount { return_type: ret, value: e(), rank: e(), count: e() },
            WireExpMapOp::GetByIndex { return_type: ret, value_type: WireExpType::Int, index: e() },
            WireExpMapOp::GetByIndexRange { return_type: ret, index: e() },
            WireExpMapOp::GetByIndexRangeCount { return_type: ret, index: e(), count: e() },
            WireExpMapOp::GetByRank { return_type: ret, value_type: WireExpType::Int, rank: e() },
            WireExpMapOp::GetByRankRange { return_type: ret, rank: e() },
            WireExpMapOp::GetByRankRangeCount { return_type: ret, rank: e(), count: e() },
        ];
        assert_eq!(ops.len(), 37, "every map constructor the client has");

        let mut seen = std::collections::HashMap::new();
        for op in ops {
            let label = format!("{op:?}");
            let exp = WireExp::Map(Box::new(WireExpMap {
                op,
                bin: bin.clone(),
                ctx: vec![],
            }));
            if let Some(earlier) = seen.insert(packed(&exp), label.clone()) {
                panic!("{label} packed the same as {earlier}");
            }
        }
    }

    /// A context path is part of the packed form, and an unbounded range is not the
    /// same as a bounded one — two things a translation can drop without failing.
    #[test]
    fn a_collection_expression_carries_its_context_and_its_bounds() {
        let bin = WireExp::Bin {
            name: "l".into(),
            exp_type: WireExpType::List,
        };
        let ret = WireListReturn::of(WireListReturnKind::Count);
        let ranged = |begin: Option<WireExp>, end: Option<WireExp>, ctx: Vec<WireCtx>| {
            WireExp::List(Box::new(WireExpList {
                op: WireExpListOp::GetByValueRange {
                    return_type: ret,
                    value_begin: begin,
                    value_end: end,
                },
                bin: bin.clone(),
                ctx,
            }))
        };
        let both = packed(&ranged(Some(int(1)), Some(int(9)), vec![]));
        let open_end = packed(&ranged(Some(int(1)), None, vec![]));
        let open_start = packed(&ranged(None, Some(int(9)), vec![]));
        let unbounded = packed(&ranged(None, None, vec![]));
        let mut seen = std::collections::HashSet::new();
        for form in [&both, &open_end, &open_start, &unbounded] {
            assert!(seen.insert(form.clone()), "two range shapes packed alike");
        }

        let nested = packed(&ranged(
            Some(int(1)),
            Some(int(9)),
            vec![WireCtx::ListIndex { index: 0 }],
        ));
        assert_ne!(both, nested, "the context path must reach the packed form");
    }

    /// The five convenience selections and the two convenience modifications are
    /// **aliases**: the client defines each as the general form with a flag filled
    /// in, exactly as `exclusive` is `xor`. Asserting it keeps the contract's
    /// separate variants honest — if a flag ever changed, one spelling would
    /// silently mean something else.
    #[test]
    fn the_convenience_path_forms_are_aliases_of_the_general_ones() {
        let bin = WireExp::Bin {
            name: "b".into(),
            exp_type: WireExpType::Map,
        };
        let path = |op: WireExpPathOp| {
            WireExp::Path(Box::new(WireExpPath {
                op,
                return_type: WireExpType::List,
                bin: bin.clone(),
                ctx: vec![WireCtx::AllChildren],
            }))
        };

        // SelectFlag: MATCHING_TREE = 0, VALUE = 1, MAP_KEY = 2, MAP_KEY_VALUE = 3.
        for (convenience, flag) in [
            (WireExpPathOp::SelectMatchingTree, 0),
            (WireExpPathOp::SelectValues, 1),
            (WireExpPathOp::SelectMapKeys, 2),
            (WireExpPathOp::SelectMapEntries, 3),
        ] {
            assert_eq!(
                packed(&path(convenience.clone())),
                packed(&path(WireExpPathOp::SelectByPath { flag })),
                "{convenience:?} should be selectByPath with flag {flag}"
            );
        }

        // ModifyFlag: DEFAULT = 0, NO_FAIL = 0x10.
        for (convenience, flag) in [
            (WireExpPathOp::Modify { modify: int(0) }, 0),
            (WireExpPathOp::ModifyNoFail { modify: int(0) }, 0x10),
        ] {
            assert_eq!(
                packed(&path(convenience.clone())),
                packed(&path(WireExpPathOp::ModifyByPath {
                    flag,
                    modify: int(0)
                })),
                "{convenience:?} should be modifyByPath with flag {flag:#x}"
            );
        }
    }

    /// What *is* distinct: the two general forms across flags, and `remove`.
    #[test]
    fn the_general_path_forms_are_distinct_per_flag() {
        let bin = WireExp::Bin {
            name: "b".into(),
            exp_type: WireExpType::Map,
        };
        let path = |op: WireExpPathOp| {
            WireExp::Path(Box::new(WireExpPath {
                op,
                return_type: WireExpType::List,
                bin: bin.clone(),
                ctx: vec![WireCtx::AllChildren],
            }))
        };

        let mut seen = std::collections::HashSet::new();
        for flag in [0, 1, 2, 3, 0x10] {
            assert!(
                seen.insert(packed(&path(WireExpPathOp::SelectByPath { flag }))),
                "select flag {flag:#x} collided"
            );
        }
        for flag in [0, 0x10] {
            assert!(
                seen.insert(packed(&path(WireExpPathOp::ModifyByPath {
                    flag,
                    modify: int(0)
                }))),
                "modify flag {flag:#x} collided"
            );
        }
        assert!(
            seen.insert(packed(&path(WireExpPathOp::Remove))),
            "remove collided with a select or a modify"
        );

        // A modify expression is part of the packed form: two different new values
        // must not produce the same bytes.
        assert_ne!(
            packed(&path(WireExpPathOp::Modify { modify: int(1) })),
            packed(&path(WireExpPathOp::Modify { modify: int(2) })),
        );
    }

    /// `SelectByPath`'s flag and the return type are both part of the packed form —
    /// a flag dropped on the way through would select the wrong part of each node.
    #[test]
    fn a_path_selection_carries_its_flag_and_return_type() {
        let select = |flag: i64, return_type: WireExpType| {
            WireExp::Path(Box::new(WireExpPath {
                op: WireExpPathOp::SelectByPath { flag },
                return_type,
                bin: WireExp::Bin {
                    name: "b".into(),
                    exp_type: WireExpType::Map,
                },
                ctx: vec![WireCtx::AllChildren],
            }))
        };
        assert_ne!(
            packed(&select(1, WireExpType::List)),
            packed(&select(2, WireExpType::List)),
            "the flag is part of the packed form"
        );
        assert_ne!(
            packed(&select(1, WireExpType::List)),
            packed(&select(1, WireExpType::Int)),
            "the return type is too"
        );
    }

    /// Ten types, ten loop-variable constructors — and three parts each. A mapping
    /// that collapsed two would read the wrong part of the child, silently.
    #[test]
    fn every_loop_variable_type_and_part_packs_differently() {
        let mut seen = std::collections::HashSet::new();
        for exp_type in [
            WireExpType::Nil,
            WireExpType::Bool,
            WireExpType::Int,
            WireExpType::Str,
            WireExpType::List,
            WireExpType::Map,
            WireExpType::Blob,
            WireExpType::Float,
            WireExpType::Geo,
            WireExpType::Hll,
        ] {
            for part in [
                WireLoopVarPart::MAP_KEY,
                WireLoopVarPart::VALUE,
                WireLoopVarPart::INDEX,
            ] {
                let exp = WireExp::LoopVar { exp_type, part };
                assert!(
                    seen.insert(packed(&exp)),
                    "{exp_type:?}/{part:?} collided with an earlier loop variable"
                );
            }
        }
        assert_eq!(seen.len(), 30);
    }

    /// The 8.1.1 gate is on the **fan-out**, which is what needs it — so it fires
    /// for a path expression, for a loop variable, and for an ordinary collection
    /// read whose context happens to fan out.
    #[test]
    fn a_cluster_too_old_for_a_fan_out_is_refused_by_name() {
        let old = Capabilities {
            paths: PathSupport::Unsupported {
                node: "BB9040011AC4202".into(),
                version: "8.1.0.0".into(),
            },
            ..Capabilities::all()
        };

        let path = WireExp::Path(Box::new(WireExpPath {
            op: WireExpPathOp::SelectValues,
            return_type: WireExpType::List,
            bin: WireExp::Bin {
                name: "b".into(),
                exp_type: WireExpType::Map,
            },
            ctx: vec![WireCtx::AllChildren],
        }));
        let error = from_tree(&path, &old).expect_err("8.1.1 is required");
        assert!(error.to_string().contains("8.1.1"), "{error}");
        assert!(error.to_string().contains("BB9040011AC4202"), "{error}");
        assert!(error.to_string().contains("8.1.0.0"), "{error}");

        // A loop variable arrived with the same server release.
        let var = WireExp::LoopVar {
            exp_type: WireExpType::Int,
            part: WireLoopVarPart::VALUE,
        };
        assert!(from_tree(&var, &old).is_err());
        assert!(from_tree(&WireExp::RemoveResult, &old).is_err());

        // And a plain list read whose context fans out — the gate is on the step.
        let fanned = WireExp::List(Box::new(WireExpList {
            op: WireExpListOp::Size,
            bin: WireExp::Bin {
                name: "b".into(),
                exp_type: WireExpType::List,
            },
            ctx: vec![WireCtx::AllChildren],
        }));
        assert!(from_tree(&fanned, &old).is_err());

        // Everything above builds against a new enough cluster.
        for exp in [&path, &var, &WireExp::RemoveResult, &fanned] {
            assert!(from_tree(exp, &Capabilities::all()).is_ok(), "{exp:?}");
        }
    }

    /// A filtered fan-out carries a whole expression, and the filter has to reach
    /// the packed form — otherwise every child would match.
    #[test]
    fn a_filtered_fan_out_carries_its_filter() {
        let fan = |filter: Option<WireExp>| {
            let step = match filter {
                None => WireCtx::AllChildren,
                Some(tree) => WireCtx::AllChildrenWithFilter {
                    filter: Box::new(aerospike_php_ipc::op::WireExpression::Tree(tree)),
                },
            };
            WireExp::Path(Box::new(WireExpPath {
                op: WireExpPathOp::SelectValues,
                return_type: WireExpType::List,
                bin: WireExp::Bin {
                    name: "b".into(),
                    exp_type: WireExpType::Map,
                },
                ctx: vec![step],
            }))
        };
        let cheap = WireExp::Binary {
            op: WireExpBinaryOp::Gt,
            left: Box::new(WireExp::LoopVar {
                exp_type: WireExpType::Int,
                part: WireLoopVarPart::VALUE,
            }),
            right: Box::new(int(10)),
        };
        let dear = WireExp::Binary {
            op: WireExpBinaryOp::Gt,
            left: Box::new(WireExp::LoopVar {
                exp_type: WireExpType::Int,
                part: WireLoopVarPart::VALUE,
            }),
            right: Box::new(int(100)),
        };

        let mut seen = std::collections::HashSet::new();
        for form in [fan(None), fan(Some(cheap)), fan(Some(dear))] {
            assert!(
                seen.insert(packed(&form)),
                "two fan-outs packed alike, so the filter was dropped"
            );
        }
    }

    /// The limits are enforced here, not just measurable — this is the check that
    /// keeps a tree from another process off the daemon's stack.
    #[test]
    fn a_tree_past_the_limits_is_refused_before_it_is_walked() {
        let mut deep = int(0);
        for _ in 0..(MAX_EXP_DEPTH + 5) {
            deep = WireExp::Unary {
                op: WireExpUnaryOp::Not,
                exp: Box::new(deep),
            };
        }
        let error = from_tree(&deep, &Capabilities::all()).expect_err("too deep");
        assert!(error.to_string().contains("nests"), "{error}");

        let wide = WireExp::Variadic {
            op: WireExpVariadicOp::Or,
            exps: (0..=MAX_EXP_NODES).map(|n| int(n as i64)).collect(),
        };
        let error = from_tree(&wide, &Capabilities::all()).expect_err("too many nodes");
        assert!(error.to_string().contains("nodes"), "{error}");
    }

    /// `let`, `def` and `var` have to build together: a variable reference the
    /// client cannot resolve is the server's error to give, but the three
    /// constructors have to be reached at all.
    #[test]
    fn variable_binding_builds() {
        let exp = WireExp::Variadic {
            op: WireExpVariadicOp::Let,
            exps: vec![
                WireExp::Def {
                    name: "x".into(),
                    value: Box::new(int_bin("a")),
                },
                WireExp::Binary {
                    op: WireExpBinaryOp::Gt,
                    left: Box::new(WireExp::Var("x".into())),
                    right: Box::new(int(0)),
                },
            ],
        };
        assert!(!packed(&exp).is_empty());
    }

    /// The flags and the pattern are both part of the packed form, so neither may
    /// be dropped on the way through — a regex compiled without its
    /// case-insensitive flag matches different records.
    #[test]
    fn a_regex_carries_its_pattern_and_its_flags() {
        let regex = |pattern: &str, flags: i64| WireExp::Regex {
            regex: pattern.into(),
            flags,
            bin: Box::new(WireExp::Bin {
                name: "s".into(),
                exp_type: WireExpType::Str,
            }),
        };
        assert_ne!(
            packed(&regex("^a", 1)),
            packed(&regex("^a", 0)),
            "the flags are part of the packed form"
        );
        assert_ne!(
            packed(&regex("^a", 0)),
            packed(&regex("^b", 0)),
            "the pattern is part of the packed form"
        );
    }
}
