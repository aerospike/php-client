// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! `Aerospike\Exp`: the expression builder.
//!
//! Every method here returns an [`Expression`](crate::ops::Expression), and every
//! place that takes an expression takes one of these — a policy `filter`, the
//! expression read and write operations, an expression-based secondary index, a
//! query filter. So the builder is not a parallel API to
//! `Aerospike\Expression::ael()`: it is a third way to build the same thing, and
//! the three are interchangeable at the point of use.
//!
//! ```php
//! use Aerospike\Exp;
//!
//! // age > 21 && status == "active"
//! $filter = Exp::and([
//!     Exp::gt(Exp::intBin('age'), Exp::intVal(21)),
//!     Exp::eq(Exp::stringBin('status'), Exp::stringVal('active')),
//! ]);
//! $record = $client->get(new Aerospike\ReadPolicy(filterExp: $filter), $key);
//! ```
//!
//! # Why this exists alongside the text form
//!
//! `Aerospike\Expression::ael('$.age > 21')` is shorter, and the server does the
//! parsing. But the Aerospike Expression Language only exists from **server
//! 8.1.3**, and a filter is not a nicety — it is how a caller avoids reading
//! records they did not ask for. A builder packs client-side, so it works against
//! every server this client supports.
//!
//! It is also checked earlier. `Exp::gt(Exp::intBin('age'), Exp::stringVal('x'))`
//! is a type error the *server* will report for the text form, one round trip
//! later; here the pieces are objects, and a wrong one is a `TypeError` at the
//! call site.
//!
//! # Lists of expressions are arrays
//!
//! `and`, `or`, `cond` and the rest take an **array**, not variadic arguments:
//! `Exp::and([$a, $b])`. That is the same shape `operate()` takes for its
//! operations and `batch()` for its rows, and consistency inside this API is
//! worth more than saving two characters. A wrong element is a `TypeError` naming
//! its position.
//!
//! # Static methods, no instances
//!
//! `Exp` is a namespace, not a value. It has no constructor: an `Exp` object would
//! hold nothing and do nothing, and the thing worth holding is the `Expression` a
//! method returns.

#![allow(non_snake_case)]

use aerospike_php_ipc::exp::{
    WireExp, WireExpBinaryOp, WireExpBit, WireExpBitOp, WireExpHll, WireExpHllOp, WireExpList,
    WireExpListOp, WireExpMap, WireExpMapOp, WireExpMeta, WireExpPath, WireExpPathOp, WireExpStr,
    WireExpStrOp, WireExpType, WireExpUnaryOp, WireExpVariadicOp, WireStringPolicy,
};
use aerospike_php_ipc::op::{WireExpression, WireListSort};
use aerospike_php_ipc::WireValue;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

use crate::arg::{type_error, type_of, Given};
use crate::enums::{
    BitOverflow, BitResize, ExpType, ListReturn, LoopVarPart, MapReturn, StringNumericType,
};
use crate::error::{AeroError, AeroResult};
use crate::ops::{context_list, BinPolicy, Expression, ListPolicy, MapPolicy};
use crate::value::{zval_to_wire, Path};

/// The expression builder: `aerospike-core`'s `expressions` module.
///
/// A namespace of static methods, each returning an `Aerospike\Expression`. See
/// the module documentation for why lists are arrays and why there is no
/// constructor.
///
/// ```php
/// use Aerospike\Exp;
///
/// // A record written in the last hour, whose "score" bin is in the top band.
/// Exp::and([
///     Exp::lt(Exp::sinceUpdate(), Exp::intVal(3_600_000_000_000)),
///     Exp::ge(Exp::intBin('score'), Exp::intVal(900)),
/// ]);
/// ```
#[php_class]
#[php(name = "Aerospike\\Exp")]
#[derive(Debug)]
pub struct Exp;

#[php_impl]
impl Exp {
    // ===== Literals =========================================================

    /// A literal, converted the way a bin value is.
    ///
    /// The one method here that is not in `aerospike-core`: PHP is dynamically
    /// typed, so the mapping from a PHP value to an Aerospike one already exists
    /// for every bin written by this client, and an expression literal is the
    /// same question. Accepts an int, float, string, bool, `null`, an array, or
    /// any of `Blob`, `GeoJson`, `Hll`, `Infinity`, `Wildcard`, `OrderedMap` and
    /// `SortedMap`.
    ///
    /// Use the typed constructors below where the type matters and the PHP value
    /// does not settle it — a PHP string is a string, never GeoJSON, so
    /// `Exp::geoVal()` is how a region is written.
    pub fn val(value: &Zval) -> PhpResult<Expression> {
        Ok(tree(WireExp::Value(zval_to_wire(
            value,
            &Path::bin("the expression value"),
        )?)))
    }

    /// An integer literal.
    pub fn int_val(value: i64) -> Expression {
        tree(WireExp::Value(WireValue::Int(value)))
    }

    /// A boolean literal.
    pub fn bool_val(value: bool) -> Expression {
        tree(WireExp::Value(WireValue::Bool(value)))
    }

    /// A string literal.
    pub fn string_val(value: String) -> Expression {
        tree(WireExp::Value(WireValue::Str(value)))
    }

    /// A float literal.
    pub fn float_val(value: f64) -> Expression {
        tree(WireExp::Value(WireValue::Float(value)))
    }

    /// A byte-string literal.
    pub fn blob_val(value: Vec<u8>) -> Expression {
        tree(WireExp::Value(WireValue::Blob(value)))
    }

    /// A GeoJSON literal, for [`geo_compare`](Self::geo_compare).
    pub fn geo_val(value: String) -> Expression {
        tree(WireExp::Value(WireValue::GeoJson(value)))
    }

    /// A list literal, from a PHP array.
    ///
    /// The array's *values* are used and its keys ignored, which is what makes a
    /// PHP list a list. Pass a `SortedMap` or `OrderedMap` for a map literal, or
    /// use [`map_val`](Self::map_val).
    pub fn list_val(value: &Zval) -> PhpResult<Expression> {
        let wire = zval_to_wire(value, &Path::bin("the list value"))?;
        match wire {
            WireValue::List(_) => Ok(tree(WireExp::Value(wire))),
            other => Err(AeroError::client(format!(
                "listVal() needs a PHP list, but that is {}. A PHP array with string keys is a \
                 map, not a list — use mapVal() for one",
                describe_value(&other)
            ))
            .into()),
        }
    }

    /// A map literal.
    ///
    /// A plain PHP array with string keys, an `OrderedMap`, or a `SortedMap`.
    /// **Pass a `SortedMap` to compare whole maps**: the server compares maps in
    /// key order, and only a sorted map is packed with the ordering flag that
    /// makes such a comparison meaningful.
    pub fn map_val(value: &Zval) -> PhpResult<Expression> {
        let wire = zval_to_wire(value, &Path::bin("the map value"))?;
        match wire {
            WireValue::Map(_) | WireValue::OrderedMap(_) | WireValue::SortedMap(_) => {
                Ok(tree(WireExp::Value(wire)))
            }
            other => Err(AeroError::client(format!(
                "mapVal() needs a map, but that is {}. A PHP list is a list — use listVal() for \
                 one",
                describe_value(&other)
            ))
            .into()),
        }
    }

    /// Nil, the value an absent bin reads as.
    pub fn nil() -> Expression {
        tree(WireExp::Value(WireValue::Nil))
    }

    /// The value that sorts above every other, for an open-ended range.
    pub fn infinity() -> Expression {
        tree(WireExp::Value(WireValue::Infinity))
    }

    /// The value that matches any other, for a selection by example.
    pub fn wildcard() -> Expression {
        tree(WireExp::Value(WireValue::Wildcard))
    }

    // ===== Bins =============================================================

    /// A bin, read as `expType`.
    ///
    /// The type is not a hint: the server reads the bin's bytes as the type
    /// named, so naming the wrong one compares against nonsense rather than
    /// failing. The typed shorthands below are this with the type filled in, and
    /// are what most code should use.
    pub fn bin(name: String, expType: ExpType) -> Expression {
        tree(WireExp::Bin {
            name,
            exp_type: expType.to_wire(),
        })
    }

    /// An integer bin.
    pub fn int_bin(name: String) -> Expression {
        typed_bin(name, WireExpType::Int)
    }

    /// A boolean bin.
    pub fn bool_bin(name: String) -> Expression {
        typed_bin(name, WireExpType::Bool)
    }

    /// A string bin.
    pub fn string_bin(name: String) -> Expression {
        typed_bin(name, WireExpType::Str)
    }

    /// A byte-string bin.
    pub fn blob_bin(name: String) -> Expression {
        typed_bin(name, WireExpType::Blob)
    }

    /// A float bin.
    pub fn float_bin(name: String) -> Expression {
        typed_bin(name, WireExpType::Float)
    }

    /// A GeoJSON bin.
    pub fn geo_bin(name: String) -> Expression {
        typed_bin(name, WireExpType::Geo)
    }

    /// A list bin.
    pub fn list_bin(name: String) -> Expression {
        typed_bin(name, WireExpType::List)
    }

    /// A map bin.
    pub fn map_bin(name: String) -> Expression {
        typed_bin(name, WireExpType::Map)
    }

    /// A HyperLogLog bin.
    pub fn hll_bin(name: String) -> Expression {
        typed_bin(name, WireExpType::Hll)
    }

    /// Whether a bin is present.
    pub fn bin_exists(name: String) -> Expression {
        tree(WireExp::BinExists(name))
    }

    /// A bin's particle type, as the integer the server uses.
    pub fn bin_type(name: String) -> Expression {
        tree(WireExp::BinType(name))
    }

    // ===== The record itself ================================================

    /// The record's key, read as `expType`.
    ///
    /// Only present when the record was written with `sendKey`, so
    /// [`key_exists`](Self::key_exists) is worth asking first.
    pub fn key(expType: ExpType) -> Expression {
        tree(WireExp::Key(expType.to_wire()))
    }

    /// Whether the record's key was stored with it.
    ///
    /// A fact about the *write* — `sendKey` on the policy that created the record
    /// — not about the record's identity. Every record has a key; not every
    /// record has it stored.
    pub fn key_exists() -> Expression {
        meta(WireExpMeta::KeyExists)
    }

    /// The record's set name, or `""` for a record in no set.
    pub fn set_name() -> Expression {
        meta(WireExpMeta::SetName)
    }

    /// The record's size in bytes.
    pub fn record_size() -> Expression {
        meta(WireExpMeta::RecordSize)
    }

    /// The record's size on device in bytes. Zero for an in-memory namespace.
    ///
    /// Superseded by [`record_size`](Self::record_size), which does not depend on
    /// where the namespace stores its data. Kept because it is a distinct server
    /// opcode and the only one available on older servers.
    pub fn device_size() -> Expression {
        meta(WireExpMeta::DeviceSize)
    }

    /// The record's size in memory in bytes. Zero for an on-device namespace.
    ///
    /// Superseded by [`record_size`](Self::record_size), as
    /// [`device_size`](Self::device_size) is.
    pub fn memory_size() -> Expression {
        meta(WireExpMeta::MemorySize)
    }

    /// When the record was last written, in nanoseconds since the Unix epoch.
    pub fn last_update() -> Expression {
        meta(WireExpMeta::LastUpdate)
    }

    /// Nanoseconds since the record was last written.
    pub fn since_update() -> Expression {
        meta(WireExpMeta::SinceUpdate)
    }

    /// When the record expires, in nanoseconds since the Unix epoch. Zero for a
    /// record that never expires.
    pub fn void_time() -> Expression {
        meta(WireExpMeta::VoidTime)
    }

    /// Seconds until the record expires. `-1` for a record that never does.
    pub fn ttl() -> Expression {
        meta(WireExpMeta::Ttl)
    }

    /// Whether this is a tombstone left by a durable delete.
    pub fn is_tombstone() -> Expression {
        meta(WireExpMeta::IsTombstone)
    }

    /// The record digest modulo `modulo`, for sampling a fraction of a set.
    ///
    /// `Exp::eq(Exp::digestModulo(100), Exp::intVal(0))` matches about one
    /// record in a hundred, and the same records every time — the digest does not
    /// change, so a sample taken this way is stable across runs.
    pub fn digest_modulo(modulo: i64) -> PhpResult<Expression> {
        Ok(tree(digest_modulo_tree(modulo)?))
    }

    // ===== Comparison =======================================================

    /// Equal.
    pub fn eq(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::Eq, left, right)?)
    }

    /// Not equal.
    pub fn ne(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::Ne, left, right)?)
    }

    /// Greater than.
    pub fn gt(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::Gt, left, right)?)
    }

    /// Greater than or equal.
    pub fn ge(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::Ge, left, right)?)
    }

    /// Less than.
    pub fn lt(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::Lt, left, right)?)
    }

    /// Less than or equal.
    pub fn le(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::Le, left, right)?)
    }

    /// Match a string bin against a regular expression.
    ///
    /// The pattern is a literal string rather than an expression, because the
    /// server compiles it once. `flags` combines `Aerospike\RegexFlag` cases with
    /// `|`; pass `0` for the defaults.
    ///
    /// ```php
    /// use Aerospike\{Exp, RegexFlag};
    /// Exp::regexCompare('^a.*z$', RegexFlag::Icase->value, Exp::stringBin('name'));
    /// ```
    pub fn regex_compare(regex: String, flags: i64, bin: &Expression) -> PhpResult<Expression> {
        Ok(tree(regex_tree(regex, flags, bin)?))
    }

    /// Whether two GeoJSON regions relate — the documents say how.
    pub fn geo_compare(left: &Expression, right: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::GeoCompare, left, right)?)
    }

    /// Whether `value` appears in `list`.
    ///
    /// One comparison against a list, rather than an `or` of one comparison per
    /// element — cheaper on the server and much shorter to write.
    pub fn in_list(value: &Expression, list: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::InList, value, list)?)
    }

    // ===== Logic ============================================================

    /// Negation.
    pub fn not(exp: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::Not, exp)?)
    }

    /// All of them.
    pub fn and(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::And, &exps, "and")
    }

    /// Any of them.
    pub fn or(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::Or, &exps, "or")
    }

    /// An odd number of them.
    pub fn xor(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::Xor, &exps, "xor")
    }

    /// Exactly one of them — the Java client's spelling of
    /// [`xor`](Self::xor).
    ///
    /// The same server opcode as `xor`, kept because the two names appear in
    /// different clients' documentation and a reader of either should find what
    /// they expect.
    pub fn exclusive(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::Xor, &exps, "exclusive")
    }

    // ===== Arithmetic =======================================================

    /// Sum.
    pub fn num_add(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::NumAdd, &exps, "numAdd")
    }

    /// Left-to-right difference.
    pub fn num_sub(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::NumSub, &exps, "numSub")
    }

    /// Product.
    pub fn num_mul(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::NumMul, &exps, "numMul")
    }

    /// Left-to-right quotient.
    pub fn num_div(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::NumDiv, &exps, "numDiv")
    }

    /// `base` raised to `exponent`.
    pub fn num_pow(base: &Expression, exponent: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::NumPow, base, exponent)?)
    }

    /// Logarithm of `num` in `base`.
    pub fn num_log(num: &Expression, base: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::NumLog, num, base)?)
    }

    /// Remainder of `numerator` divided by `denominator`.
    pub fn num_mod(numerator: &Expression, denominator: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::NumMod, numerator, denominator)?)
    }

    /// Absolute value.
    pub fn num_abs(value: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::NumAbs, value)?)
    }

    /// Round down.
    pub fn num_floor(num: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::NumFloor, num)?)
    }

    /// Round up.
    pub fn num_ceil(num: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::NumCeil, num)?)
    }

    /// Truncate a float to an integer.
    pub fn to_int(num: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::ToInt, num)?)
    }

    /// Widen an integer to a float.
    pub fn to_float(num: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::ToFloat, num)?)
    }

    /// Smallest.
    pub fn min(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::Min, &exps, "min")
    }

    /// Largest.
    pub fn max(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::Max, &exps, "max")
    }

    // ===== Integers, bit by bit =============================================

    /// Bitwise AND.
    pub fn int_and(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::IntAnd, &exps, "intAnd")
    }

    /// Bitwise OR.
    pub fn int_or(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::IntOr, &exps, "intOr")
    }

    /// Bitwise XOR.
    pub fn int_xor(exps: Vec<&Zval>) -> PhpResult<Expression> {
        variadic(WireExpVariadicOp::IntXor, &exps, "intXor")
    }

    /// Bitwise complement.
    pub fn int_not(exp: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::IntNot, exp)?)
    }

    /// Shift left.
    pub fn int_lshift(value: &Expression, shift: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::IntLshift, value, shift)?)
    }

    /// Shift right, filling with zeroes.
    pub fn int_rshift(value: &Expression, shift: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::IntRshift, value, shift)?)
    }

    /// Shift right, preserving the sign bit.
    pub fn int_arshift(value: &Expression, shift: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::IntArshift, value, shift)?)
    }

    /// Count the set bits.
    pub fn int_count(exp: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::IntCount, exp)?)
    }

    /// Index of the left-most bit matching `search`.
    pub fn int_lscan(value: &Expression, search: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::IntLscan, value, search)?)
    }

    /// Index of the right-most bit matching `search`.
    pub fn int_rscan(value: &Expression, search: &Expression) -> PhpResult<Expression> {
        Ok(binary(WireExpBinaryOp::IntRscan, value, search)?)
    }

    // ===== Collections, in passing ==========================================

    /// The keys of a map, as a list.
    pub fn map_keys(map: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::MapKeys, map)?)
    }

    /// The values of a map, as a list.
    pub fn map_values(map: &Expression) -> PhpResult<Expression> {
        Ok(unary(WireExpUnaryOp::MapValues, map)?)
    }

    // ===== Control flow =====================================================

    /// Condition, result, condition, result, …, default.
    ///
    /// An **odd** number of expressions: pairs of condition and result, then the
    /// default that applies when none matched. An even number is refused here
    /// rather than by the server, because the mistake is always the same one — a
    /// forgotten default — and the server's answer does not say so.
    ///
    /// ```php
    /// Exp::cond([
    ///     Exp::ge(Exp::intBin('score'), Exp::intVal(900)), Exp::stringVal('gold'),
    ///     Exp::ge(Exp::intBin('score'), Exp::intVal(500)), Exp::stringVal('silver'),
    ///     Exp::stringVal('bronze'),                        // the default
    /// ]);
    /// ```
    pub fn cond(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Ok(tree(cond_tree(expression_list(&exps, "cond")?)?))
    }

    /// Variable definitions, then the expression that uses them.
    ///
    /// Every element but the last must be a [`def`](Self::def), and the last is
    /// the expression evaluated with those definitions in scope. Worth reaching
    /// for when a sub-expression appears more than once: the server evaluates a
    /// bound variable once.
    ///
    /// ```php
    /// Exp::let([
    ///     Exp::def('total', Exp::numAdd([Exp::intBin('a'), Exp::intBin('b')])),
    ///     Exp::gt(Exp::var('total'), Exp::intVal(100)),
    /// ]);
    /// ```
    ///
    /// Called `let` here and `exp_let` in the Rust client, where `let` is a
    /// keyword. PHP allows it as a method name, so this is the name that reads.
    #[php(name = "let")]
    pub fn let_(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Ok(tree(let_tree(expression_list(&exps, "let")?)?))
    }

    /// The 1.x client's spelling of [`let`](Self::let_).
    ///
    /// `let` is what this client calls it — PHP allows the keyword as a method
    /// name where Rust does not — and `expLet` is what the previous client had to
    /// call it. Both are here so old code reads and new code reads well.
    pub fn exp_let(exps: Vec<&Zval>) -> PhpResult<Expression> {
        Exp::let_(exps)
    }

    /// Define a variable, for use inside a [`let`](Self::let_).
    pub fn def(name: String, value: &Expression) -> PhpResult<Expression> {
        Ok(tree(def_tree(name, value)?))
    }

    /// A variable an enclosing [`let`](Self::let_) defined.
    pub fn var(name: String) -> PhpResult<Expression> {
        Ok(tree(var_tree(name)?))
    }

    /// The node a fan-out is currently considering, read as `expType`.
    ///
    /// Only meaningful inside a `Ctx::allChildrenWithFilter()` filter or an
    /// `ExpPath` modify expression — outside one there is no node being
    /// considered, and the server says so. `part` picks which side of the child is
    /// meant: its map key, its value, or its list index.
    ///
    /// ```php
    /// use Aerospike\{Exp, ExpType, LoopVarPart};
    ///
    /// // Children whose own value exceeds 20.
    /// Exp::gt(Exp::loopVar(ExpType::Integer, LoopVarPart::Value), Exp::intVal(20));
    /// ```
    ///
    /// Needs server 8.1.1 or later, like the path expressions it belongs to.
    pub fn loop_var(expType: ExpType, part: LoopVarPart) -> Expression {
        tree(WireExp::LoopVar {
            exp_type: expType.to_wire(),
            part: part.to_wire(),
        })
    }

    /// "Delete this node", as the result of an `ExpPath` modify expression.
    ///
    /// A modify expression normally produces the node's new value. Returning this
    /// instead removes the node, which is how a conditional removal is written
    /// without a second pass over the collection.
    ///
    /// Needs server 8.1.1 or later.
    pub fn remove_result() -> Expression {
        tree(WireExp::RemoveResult)
    }

    /// The value a *write* would have produced.
    ///
    /// For the expression write operations, where it stands for "whatever the
    /// expression evaluates to". Meaningless as a filter, and the server says so.
    pub fn unknown() -> Expression {
        tree(WireExp::Unknown)
    }
}

/// Flags for [`Exp::regex_compare`], combined with `|`.
///
/// Constants rather than an enum, because they are a **bitmask**: PHP enum cases
/// cannot be OR-ed together, and the parameter takes one integer.
///
/// ```php
/// use Aerospike\{Exp, RegexFlag};
///
/// // Case-insensitive, POSIX extended syntax.
/// Exp::regexCompare('^a(b|c)z$', RegexFlag::ICASE | RegexFlag::EXTENDED, Exp::stringBin('name'));
/// ```
#[php_class]
#[php(name = "Aerospike\\RegexFlag")]
#[derive(Debug)]
pub struct RegexFlag;

#[php_impl]
impl RegexFlag {
    /// The regex engine's defaults.
    pub const NONE: i64 = 0;
    /// POSIX Extended Regular Expression syntax.
    pub const EXTENDED: i64 = 1;
    /// Ignore case.
    pub const ICASE: i64 = 2;
    /// Do not report the position of matches. Faster when only the yes/no answer
    /// is wanted, which for a filter is always.
    pub const NOSUB: i64 = 4;
    /// Match-any-character operators do not match a newline.
    pub const NEWLINE: i64 = 8;
}

// ===== collections ==========================================================

/// List expressions: `aerospike-core`'s `expressions::lists`.
///
/// Every argument is an expression, which is the difference from
/// `Aerospike\ListOp`: an operation takes a literal index, and a list *expression*
/// takes an expression for it. That is what lets a filter say "the element whose
/// index is in another bin".
///
/// The `bin` argument is the list operated on — usually `Exp::listBin('name')`,
/// but any expression evaluating to a list, including another list expression.
/// Modify operations return the whole modified list, so they compose.
///
/// ```php
/// use Aerospike\{Exp, ExpList, ListReturn};
///
/// // A record whose "scores" list contains a value above 900.
/// $filter = Exp::gt(
///     ExpList::getByValueRange(ListReturn::Count, Exp::intVal(900), null,
///         Exp::listBin('scores')),
///     Exp::intVal(0),
/// );
/// ```
///
/// Add a path into a nested list with `->context([...])`, exactly as an
/// `Operation` does.
#[php_class]
#[php(name = "Aerospike\\ExpList")]
#[derive(Debug)]
pub struct ExpList;

#[php_impl]
impl ExpList {
    /// Append one value.
    ///
    /// Returns the whole modified list, which is what makes these composable: the
    /// result is a list expression another operation can read.
    pub fn append(policy: Option<Given<&ListPolicy>>, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::Append {
            policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
            value: unwrap_tree(value, "value")?,
        },
            bin,
        )?)
    }

    /// Append every element of a list.
    pub fn append_items(policy: Option<Given<&ListPolicy>>, list: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::AppendItems {
            policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
            list: unwrap_tree(list, "list")?,
        },
            bin,
        )?)
    }

    /// Insert one value at an index.
    pub fn insert(policy: Option<Given<&ListPolicy>>, index: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::Insert {
            policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
            index: unwrap_tree(index, "index")?,
            value: unwrap_tree(value, "value")?,
        },
            bin,
        )?)
    }

    /// Insert every element of a list at an index.
    pub fn insert_items(policy: Option<Given<&ListPolicy>>, index: &Expression, list: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::InsertItems {
            policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
            index: unwrap_tree(index, "index")?,
            list: unwrap_tree(list, "list")?,
        },
            bin,
        )?)
    }

    /// Add to the number at an index.
    pub fn increment(policy: Option<Given<&ListPolicy>>, index: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::Increment {
            policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
            index: unwrap_tree(index, "index")?,
            value: unwrap_tree(value, "value")?,
        },
            bin,
        )?)
    }

    /// Replace the value at an index.
    pub fn set(policy: Option<Given<&ListPolicy>>, index: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::Set {
            policy: ListPolicy::wire(Given::or_none(policy, "policy")?),
            index: unwrap_tree(index, "index")?,
            value: unwrap_tree(value, "value")?,
        },
            bin,
        )?)
    }

    /// Remove every element.
    pub fn clear(bin: &Expression) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::Clear,
            bin,
        )?)
    }

    /// Sort in place.
    pub fn sort(descending: bool, dropDuplicates: bool, bin: &Expression) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::Sort {
            flags: WireListSort { descending, drop_duplicates: dropDuplicates },
        },
            bin,
        )?)
    }

    /// Remove elements equal to a value.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value(returnType: ListReturn, value: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByValue {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
        },
            bin,
        )?)
    }

    /// Remove elements equal to any value in a list.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value_list(returnType: ListReturn, values: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByValueList {
            return_type: returnType.to_wire(inverted),
            values: unwrap_tree(values, "values")?,
        },
            bin,
        )?)
    }

    /// Remove elements in a value range.
    ///
    /// `null` for either bound means unbounded on that side — not a missing
    /// argument.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value_range(returnType: ListReturn, valueBegin: Option<Given<&Expression>>, valueEnd: Option<Given<&Expression>>, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByValueRange {
            return_type: returnType.to_wire(inverted),
            value_begin: optional_operand(Given::or_none(valueBegin, "valueBegin")?, "valueBegin")?,
            value_end: optional_operand(Given::or_none(valueEnd, "valueEnd")?, "valueEnd")?,
        },
            bin,
        )?)
    }

    /// Remove elements by rank relative to a value.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value_relative_rank_range(returnType: ListReturn, value: &Expression, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByValueRelRankRange {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Remove a bounded number of elements by rank relative to a value.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value_relative_rank_range_count(returnType: ListReturn, value: &Expression, rank: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByValueRelRankRangeCount {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
            rank: unwrap_tree(rank, "rank")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// Remove the element at an index.
    #[php(defaults(inverted = false))]
    pub fn remove_by_index(returnType: ListReturn, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByIndex {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// Remove from an index to the end.
    #[php(defaults(inverted = false))]
    pub fn remove_by_index_range(returnType: ListReturn, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByIndexRange {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// Remove a bounded number of elements from an index.
    #[php(defaults(inverted = false))]
    pub fn remove_by_index_range_count(returnType: ListReturn, index: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByIndexRangeCount {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// Remove the element at a rank.
    #[php(defaults(inverted = false))]
    pub fn remove_by_rank(returnType: ListReturn, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByRank {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Remove from a rank to the highest.
    #[php(defaults(inverted = false))]
    pub fn remove_by_rank_range(returnType: ListReturn, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByRankRange {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Remove a bounded number of elements from a rank.
    #[php(defaults(inverted = false))]
    pub fn remove_by_rank_range_count(returnType: ListReturn, rank: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::RemoveByRankRangeCount {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// How many elements the list has.
    pub fn size(bin: &Expression) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::Size,
            bin,
        )?)
    }

    /// Select elements equal to a value.
    #[php(defaults(inverted = false))]
    pub fn get_by_value(returnType: ListReturn, value: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByValue {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
        },
            bin,
        )?)
    }

    /// Select elements in a value range. `null` bounds are unbounded.
    #[php(defaults(inverted = false))]
    pub fn get_by_value_range(returnType: ListReturn, valueBegin: Option<Given<&Expression>>, valueEnd: Option<Given<&Expression>>, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByValueRange {
            return_type: returnType.to_wire(inverted),
            value_begin: optional_operand(Given::or_none(valueBegin, "valueBegin")?, "valueBegin")?,
            value_end: optional_operand(Given::or_none(valueEnd, "valueEnd")?, "valueEnd")?,
        },
            bin,
        )?)
    }

    /// Select elements equal to any value in a list.
    #[php(defaults(inverted = false))]
    pub fn get_by_value_list(returnType: ListReturn, values: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByValueList {
            return_type: returnType.to_wire(inverted),
            values: unwrap_tree(values, "values")?,
        },
            bin,
        )?)
    }

    /// Select by rank relative to a value.
    #[php(defaults(inverted = false))]
    pub fn get_by_value_relative_rank_range(returnType: ListReturn, value: &Expression, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByValueRelRankRange {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Select a bounded number by rank relative to a value.
    #[php(defaults(inverted = false))]
    pub fn get_by_value_relative_rank_range_count(returnType: ListReturn, value: &Expression, rank: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByValueRelRankRangeCount {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
            rank: unwrap_tree(rank, "rank")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// Select the element at an index.
    ///
    /// Takes a `valueType` because one element comes back as itself rather than as
    /// a list, so the server has to be told how to read it.
    #[php(defaults(inverted = false))]
    pub fn get_by_index(returnType: ListReturn, valueType: ExpType, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByIndex {
            return_type: returnType.to_wire(inverted),
            value_type: valueType.to_wire(),
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// Select from an index to the end.
    #[php(defaults(inverted = false))]
    pub fn get_by_index_range(returnType: ListReturn, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByIndexRange {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// Select a bounded number of elements from an index.
    #[php(defaults(inverted = false))]
    pub fn get_by_index_range_count(returnType: ListReturn, index: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByIndexRangeCount {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// Select the element at a rank.
    #[php(defaults(inverted = false))]
    pub fn get_by_rank(returnType: ListReturn, valueType: ExpType, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByRank {
            return_type: returnType.to_wire(inverted),
            value_type: valueType.to_wire(),
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Select from a rank to the highest.
    #[php(defaults(inverted = false))]
    pub fn get_by_rank_range(returnType: ListReturn, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByRankRange {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Select a bounded number of elements from a rank.
    #[php(defaults(inverted = false))]
    pub fn get_by_rank_range_count(returnType: ListReturn, rank: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(node(
            WireExpListOp::GetByRankRangeCount {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }
}

/// Map expressions: `aerospike-core`'s `expressions::maps`.
///
/// As [`ExpList`], for maps — with the four ways a map is addressed (key, value,
/// index, rank) rather than a list's two.
///
/// ```php
/// use Aerospike\{Exp, ExpMap, ExpType, MapReturn};
///
/// // A record whose "attrs" map has tier == "gold".
/// $filter = Exp::eq(
///     ExpMap::getByKey(MapReturn::Value, ExpType::Text, Exp::stringVal('tier'),
///         Exp::mapBin('attrs')),
///     Exp::stringVal('gold'),
/// );
/// ```
#[php_class]
#[php(name = "Aerospike\\ExpMap")]
#[derive(Debug)]
pub struct ExpMap;

#[php_impl]
impl ExpMap {
    /// Set one key.
    pub fn put(policy: Option<Given<&MapPolicy>>, key: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::Put {
            policy: MapPolicy::wire(Given::or_none(policy, "policy")?),
            key: unwrap_tree(key, "key")?,
            value: unwrap_tree(value, "value")?,
        },
            bin,
        )?)
    }

    /// Set every key of another map.
    pub fn put_items(policy: Option<Given<&MapPolicy>>, map: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::PutItems {
            policy: MapPolicy::wire(Given::or_none(policy, "policy")?),
            map: unwrap_tree(map, "map")?,
        },
            bin,
        )?)
    }

    /// Add to the number at a key.
    pub fn increment(policy: Option<Given<&MapPolicy>>, key: &Expression, incr: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::Increment {
            policy: MapPolicy::wire(Given::or_none(policy, "policy")?),
            key: unwrap_tree(key, "key")?,
            incr: unwrap_tree(incr, "incr")?,
        },
            bin,
        )?)
    }

    /// Remove every entry.
    pub fn clear(bin: &Expression) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::Clear,
            bin,
        )?)
    }

    /// Remove one key.
    #[php(defaults(inverted = false))]
    pub fn remove_by_key(returnType: MapReturn, key: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByKey {
            return_type: returnType.to_wire(inverted),
            key: unwrap_tree(key, "key")?,
        },
            bin,
        )?)
    }

    /// Remove every key in a list.
    #[php(defaults(inverted = false))]
    pub fn remove_by_key_list(returnType: MapReturn, keys: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByKeyList {
            return_type: returnType.to_wire(inverted),
            keys: unwrap_tree(keys, "keys")?,
        },
            bin,
        )?)
    }

    /// Remove keys in a range. `null` bounds are unbounded.
    #[php(defaults(inverted = false))]
    pub fn remove_by_key_range(returnType: MapReturn, keyBegin: Option<Given<&Expression>>, keyEnd: Option<Given<&Expression>>, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByKeyRange {
            return_type: returnType.to_wire(inverted),
            key_begin: optional_operand(Given::or_none(keyBegin, "keyBegin")?, "keyBegin")?,
            key_end: optional_operand(Given::or_none(keyEnd, "keyEnd")?, "keyEnd")?,
        },
            bin,
        )?)
    }

    /// Remove keys by index relative to a key.
    #[php(defaults(inverted = false))]
    pub fn remove_by_key_relative_index_range(returnType: MapReturn, key: &Expression, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByKeyRelIndexRange {
            return_type: returnType.to_wire(inverted),
            key: unwrap_tree(key, "key")?,
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// Remove a bounded number of keys by index relative to a key.
    #[php(defaults(inverted = false))]
    pub fn remove_by_key_relative_index_range_count(returnType: MapReturn, key: &Expression, index: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByKeyRelIndexRangeCount {
            return_type: returnType.to_wire(inverted),
            key: unwrap_tree(key, "key")?,
            index: unwrap_tree(index, "index")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// Remove entries with a value.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value(returnType: MapReturn, value: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByValue {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
        },
            bin,
        )?)
    }

    /// Remove entries whose value is in a list.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value_list(returnType: MapReturn, values: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByValueList {
            return_type: returnType.to_wire(inverted),
            values: unwrap_tree(values, "values")?,
        },
            bin,
        )?)
    }

    /// Remove entries whose value is in a range.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value_range(returnType: MapReturn, valueBegin: Option<Given<&Expression>>, valueEnd: Option<Given<&Expression>>, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByValueRange {
            return_type: returnType.to_wire(inverted),
            value_begin: optional_operand(Given::or_none(valueBegin, "valueBegin")?, "valueBegin")?,
            value_end: optional_operand(Given::or_none(valueEnd, "valueEnd")?, "valueEnd")?,
        },
            bin,
        )?)
    }

    /// Remove entries by rank relative to a value.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value_relative_rank_range(returnType: MapReturn, value: &Expression, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByValueRelRankRange {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Remove a bounded number of entries by rank relative to a value.
    #[php(defaults(inverted = false))]
    pub fn remove_by_value_relative_rank_range_count(returnType: MapReturn, value: &Expression, rank: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByValueRelRankRangeCount {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
            rank: unwrap_tree(rank, "rank")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// Remove the entry at an index.
    #[php(defaults(inverted = false))]
    pub fn remove_by_index(returnType: MapReturn, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByIndex {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// Remove from an index to the end.
    #[php(defaults(inverted = false))]
    pub fn remove_by_index_range(returnType: MapReturn, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByIndexRange {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// Remove a bounded number of entries from an index.
    #[php(defaults(inverted = false))]
    pub fn remove_by_index_range_count(returnType: MapReturn, index: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByIndexRangeCount {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// Remove the entry at a rank.
    #[php(defaults(inverted = false))]
    pub fn remove_by_rank(returnType: MapReturn, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByRank {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Remove from a rank to the highest.
    #[php(defaults(inverted = false))]
    pub fn remove_by_rank_range(returnType: MapReturn, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByRankRange {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Remove a bounded number of entries from a rank.
    #[php(defaults(inverted = false))]
    pub fn remove_by_rank_range_count(returnType: MapReturn, rank: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::RemoveByRankRangeCount {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// How many entries the map has.
    pub fn size(bin: &Expression) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::Size,
            bin,
        )?)
    }

    /// The value at a key.
    ///
    /// Takes a `valueType` because one value comes back as itself rather than as a
    /// collection.
    #[php(defaults(inverted = false))]
    pub fn get_by_key(returnType: MapReturn, valueType: ExpType, key: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByKey {
            return_type: returnType.to_wire(inverted),
            value_type: valueType.to_wire(),
            key: unwrap_tree(key, "key")?,
        },
            bin,
        )?)
    }

    /// Entries whose key is in a range. `null` bounds are unbounded.
    #[php(defaults(inverted = false))]
    pub fn get_by_key_range(returnType: MapReturn, keyBegin: Option<Given<&Expression>>, keyEnd: Option<Given<&Expression>>, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByKeyRange {
            return_type: returnType.to_wire(inverted),
            key_begin: optional_operand(Given::or_none(keyBegin, "keyBegin")?, "keyBegin")?,
            key_end: optional_operand(Given::or_none(keyEnd, "keyEnd")?, "keyEnd")?,
        },
            bin,
        )?)
    }

    /// Entries whose key is in a list.
    #[php(defaults(inverted = false))]
    pub fn get_by_key_list(returnType: MapReturn, keys: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByKeyList {
            return_type: returnType.to_wire(inverted),
            keys: unwrap_tree(keys, "keys")?,
        },
            bin,
        )?)
    }

    /// Entries by index relative to a key.
    #[php(defaults(inverted = false))]
    pub fn get_by_key_relative_index_range(returnType: MapReturn, key: &Expression, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByKeyRelIndexRange {
            return_type: returnType.to_wire(inverted),
            key: unwrap_tree(key, "key")?,
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// A bounded number of entries by index relative to a key.
    #[php(defaults(inverted = false))]
    pub fn get_by_key_relative_index_range_count(returnType: MapReturn, key: &Expression, index: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByKeyRelIndexRangeCount {
            return_type: returnType.to_wire(inverted),
            key: unwrap_tree(key, "key")?,
            index: unwrap_tree(index, "index")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// Entries with a value.
    #[php(defaults(inverted = false))]
    pub fn get_by_value(returnType: MapReturn, value: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByValue {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
        },
            bin,
        )?)
    }

    /// Entries whose value is in a range.
    #[php(defaults(inverted = false))]
    pub fn get_by_value_range(returnType: MapReturn, valueBegin: Option<Given<&Expression>>, valueEnd: Option<Given<&Expression>>, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByValueRange {
            return_type: returnType.to_wire(inverted),
            value_begin: optional_operand(Given::or_none(valueBegin, "valueBegin")?, "valueBegin")?,
            value_end: optional_operand(Given::or_none(valueEnd, "valueEnd")?, "valueEnd")?,
        },
            bin,
        )?)
    }

    /// Entries whose value is in a list.
    #[php(defaults(inverted = false))]
    pub fn get_by_value_list(returnType: MapReturn, values: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByValueList {
            return_type: returnType.to_wire(inverted),
            values: unwrap_tree(values, "values")?,
        },
            bin,
        )?)
    }

    /// Entries by rank relative to a value.
    #[php(defaults(inverted = false))]
    pub fn get_by_value_relative_rank_range(returnType: MapReturn, value: &Expression, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByValueRelRankRange {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// A bounded number of entries by rank relative to a value.
    #[php(defaults(inverted = false))]
    pub fn get_by_value_relative_rank_range_count(returnType: MapReturn, value: &Expression, rank: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByValueRelRankRangeCount {
            return_type: returnType.to_wire(inverted),
            value: unwrap_tree(value, "value")?,
            rank: unwrap_tree(rank, "rank")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// The entry at an index.
    #[php(defaults(inverted = false))]
    pub fn get_by_index(returnType: MapReturn, valueType: ExpType, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByIndex {
            return_type: returnType.to_wire(inverted),
            value_type: valueType.to_wire(),
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// Entries from an index to the end.
    #[php(defaults(inverted = false))]
    pub fn get_by_index_range(returnType: MapReturn, index: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByIndexRange {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
        },
            bin,
        )?)
    }

    /// A bounded number of entries from an index.
    #[php(defaults(inverted = false))]
    pub fn get_by_index_range_count(returnType: MapReturn, index: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByIndexRangeCount {
            return_type: returnType.to_wire(inverted),
            index: unwrap_tree(index, "index")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }

    /// The entry at a rank.
    #[php(defaults(inverted = false))]
    pub fn get_by_rank(returnType: MapReturn, valueType: ExpType, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByRank {
            return_type: returnType.to_wire(inverted),
            value_type: valueType.to_wire(),
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// Entries from a rank to the highest.
    #[php(defaults(inverted = false))]
    pub fn get_by_rank_range(returnType: MapReturn, rank: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByRankRange {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
        },
            bin,
        )?)
    }

    /// A bounded number of entries from a rank.
    #[php(defaults(inverted = false))]
    pub fn get_by_rank_range_count(returnType: MapReturn, rank: &Expression, count: &Expression, bin: &Expression, inverted: bool) -> PhpResult<Expression> {
        Ok(map_node(
            WireExpMapOp::GetByRankRangeCount {
            return_type: returnType.to_wire(inverted),
            rank: unwrap_tree(rank, "rank")?,
            count: unwrap_tree(count, "count")?,
        },
            bin,
        )?)
    }
}

/// Bitwise expressions: `aerospike-core`'s `expressions::bitwise`.
///
/// Operations on a blob bin, bit by bit. Offsets and sizes are expressions, so a
/// filter can read a field whose position is itself stored in the record.
///
/// No `context()`: a blob has no nested structure to point into, which is why
/// these take no path where the collection families do.
///
/// ```php
/// use Aerospike\{Exp, ExpBit};
///
/// // Records whose flags blob has bit 3 set.
/// Exp::eq(
///     ExpBit::count(Exp::intVal(3), Exp::intVal(1), Exp::blobBin('flags')),
///     Exp::intVal(1),
/// );
/// ```
#[php_class]
#[php(name = "Aerospike\\ExpBit")]
#[derive(Debug)]
pub struct ExpBit;

#[php_impl]
impl ExpBit {
    /// Grow or shrink to a byte size.
    pub fn resize(policy: Option<Given<&BinPolicy>>, byteSize: &Expression, resizeFlags: BitResize, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Resize {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            byte_size: unwrap_tree(byteSize, "byteSize")?,
            resize_flags: resizeFlags.to_wire(),
        }, bin)?)
    }

    /// Insert bytes at a byte offset.
    pub fn insert(policy: Option<Given<&BinPolicy>>, byteOffset: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Insert {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            byte_offset: unwrap_tree(byteOffset, "byteOffset")?,
            value: unwrap_tree(value, "value")?,
        }, bin)?)
    }

    /// Remove bytes at a byte offset.
    pub fn remove(policy: Option<Given<&BinPolicy>>, byteOffset: &Expression, byteSize: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Remove {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            byte_offset: unwrap_tree(byteOffset, "byteOffset")?,
            byte_size: unwrap_tree(byteSize, "byteSize")?,
        }, bin)?)
    }

    /// Overwrite a bit range.
    pub fn set(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Set {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            value: unwrap_tree(value, "value")?,
        }, bin)?)
    }

    /// OR a bit range with a value.
    pub fn or(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Or {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            value: unwrap_tree(value, "value")?,
        }, bin)?)
    }

    /// XOR a bit range with a value.
    pub fn xor(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Xor {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            value: unwrap_tree(value, "value")?,
        }, bin)?)
    }

    /// AND a bit range with a value.
    pub fn and(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::And {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            value: unwrap_tree(value, "value")?,
        }, bin)?)
    }

    /// Invert a bit range.
    pub fn not(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Not {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
        }, bin)?)
    }

    /// Shift a bit range left.
    pub fn lshift(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, shift: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Lshift {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            shift: unwrap_tree(shift, "shift")?,
        }, bin)?)
    }

    /// Shift a bit range right.
    pub fn rshift(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, shift: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Rshift {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            shift: unwrap_tree(shift, "shift")?,
        }, bin)?)
    }

    /// Add to the integer in a bit range.
    pub fn add(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, value: &Expression, signed: bool, action: BitOverflow, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Add {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            value: unwrap_tree(value, "value")?,
            signed,
            action: action.to_wire(),
        }, bin)?)
    }

    /// Subtract from the integer in a bit range.
    pub fn subtract(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, value: &Expression, signed: bool, action: BitOverflow, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Subtract {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            value: unwrap_tree(value, "value")?,
            signed,
            action: action.to_wire(),
        }, bin)?)
    }

    /// Write an integer into a bit range.
    pub fn set_int(policy: Option<Given<&BinPolicy>>, bitOffset: &Expression, bitSize: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::SetInt {
            policy: BinPolicy::bit_wire(Given::or_none(policy, "policy")?),
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            value: unwrap_tree(value, "value")?,
        }, bin)?)
    }

    /// Read a bit range as a blob.
    pub fn get(bitOffset: &Expression, bitSize: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Get {
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
        }, bin)?)
    }

    /// Count the set bits in a bit range.
    pub fn count(bitOffset: &Expression, bitSize: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Count {
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
        }, bin)?)
    }

    /// Index of the left-most bit in a range matching `value`.
    pub fn lscan(bitOffset: &Expression, bitSize: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Lscan {
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            value: unwrap_tree(value, "value")?,
        }, bin)?)
    }

    /// Index of the right-most bit in a range matching `value`.
    pub fn rscan(bitOffset: &Expression, bitSize: &Expression, value: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::Rscan {
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            value: unwrap_tree(value, "value")?,
        }, bin)?)
    }

    /// Read a bit range as an integer.
    pub fn get_int(bitOffset: &Expression, bitSize: &Expression, signed: bool, bin: &Expression) -> PhpResult<Expression> {
        Ok(bit_node(WireExpBitOp::GetInt {
            bit_offset: unwrap_tree(bitOffset, "bitOffset")?,
            bit_size: unwrap_tree(bitSize, "bitSize")?,
            signed,
        }, bin)?)
    }
}

/// HyperLogLog expressions: `aerospike-core`'s `expressions::hll`.
///
/// A sketch answers "how many distinct values" in constant space, with an error
/// bound rather than exactly. The `list` arguments are lists of values to add, or
/// lists of *other sketches* to combine with — each method says which.
///
/// ```php
/// use Aerospike\{Exp, ExpHll};
///
/// // Records whose "visitors" sketch estimates more than a thousand.
/// Exp::gt(ExpHll::getCount(Exp::hllBin('visitors')), Exp::intVal(1000));
/// ```
#[php_class]
#[php(name = "Aerospike\\ExpHll")]
#[derive(Debug)]
pub struct ExpHll;

#[php_impl]
impl ExpHll {
    /// Create or reset a sketch.
    pub fn init(policy: Option<Given<&BinPolicy>>, indexBitCount: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::Init {
            policy: BinPolicy::hll_wire(Given::or_none(policy, "policy")?),
            index_bit_count: unwrap_tree(indexBitCount, "indexBitCount")?,
        }, bin)?)
    }

    /// Create or reset a sketch that also supports similarity estimates.
    pub fn init_with_min_hash(policy: Option<Given<&BinPolicy>>, indexBitCount: &Expression, minHashCount: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::InitWithMinHash {
            policy: BinPolicy::hll_wire(Given::or_none(policy, "policy")?),
            index_bit_count: unwrap_tree(indexBitCount, "indexBitCount")?,
            min_hash_count: unwrap_tree(minHashCount, "minHashCount")?,
        }, bin)?)
    }

    /// Add values to a sketch.
    pub fn add(policy: Option<Given<&BinPolicy>>, list: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::Add {
            policy: BinPolicy::hll_wire(Given::or_none(policy, "policy")?),
            list: unwrap_tree(list, "list")?,
        }, bin)?)
    }

    /// Add values, creating the sketch with a precision if it does not exist.
    pub fn add_with_index(policy: Option<Given<&BinPolicy>>, list: &Expression, indexBitCount: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::AddWithIndex {
            policy: BinPolicy::hll_wire(Given::or_none(policy, "policy")?),
            list: unwrap_tree(list, "list")?,
            index_bit_count: unwrap_tree(indexBitCount, "indexBitCount")?,
        }, bin)?)
    }

    /// Add values, creating the sketch with a precision and MinHash count.
    pub fn add_with_index_and_min_hash(policy: Option<Given<&BinPolicy>>, list: &Expression, indexBitCount: &Expression, minHashCount: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::AddWithIndexAndMinHash {
            policy: BinPolicy::hll_wire(Given::or_none(policy, "policy")?),
            list: unwrap_tree(list, "list")?,
            index_bit_count: unwrap_tree(indexBitCount, "indexBitCount")?,
            min_hash_count: unwrap_tree(minHashCount, "minHashCount")?,
        }, bin)?)
    }

    /// The estimated number of distinct values.
    pub fn get_count(bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::GetCount, bin)?)
    }

    /// A sketch that is the union of this one and the others.
    pub fn get_union(list: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::GetUnion {
            list: unwrap_tree(list, "list")?,
        }, bin)?)
    }

    /// The estimated size of that union, without building it.
    pub fn get_union_count(list: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::GetUnionCount {
            list: unwrap_tree(list, "list")?,
        }, bin)?)
    }

    /// The estimated size of the intersection.
    pub fn get_intersect_count(list: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::GetIntersectCount {
            list: unwrap_tree(list, "list")?,
        }, bin)?)
    }

    /// The estimated Jaccard similarity, from 0.0 to 1.0.
    pub fn get_similarity(list: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::GetSimilarity {
            list: unwrap_tree(list, "list")?,
        }, bin)?)
    }

    /// The sketch's own parameters: index bit count and MinHash count.
    pub fn describe(bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::Describe, bin)?)
    }

    /// Whether a value may be in the sketch.
    ///
    /// False positives, no false negatives: `false` is certain and `true` is
    /// probable, which is what makes it cheap.
    pub fn may_contain(list: &Expression, bin: &Expression) -> PhpResult<Expression> {
        Ok(hll_node(WireExpHllOp::MayContain {
            list: unwrap_tree(list, "list")?,
        }, bin)?)
    }
}

/// String expressions: `aerospike-core`'s `expressions::string`.
///
/// **Needs server 8.1.3 or later** — these are the string operations, and the
/// daemon checks the cluster's versions rather than letting an older server refuse
/// them obscurely.
///
/// Offsets and lengths are in **characters**, not bytes, except where the name says
/// otherwise (`byteLength`, `toBlob`): the server works in Unicode code points, so
/// a multi-byte character counts once.
///
/// `src` is the string operated on — usually `Exp::stringBin('name')`, but any
/// expression evaluating to a string, including another string expression, which is
/// what lets these chain. The modify operations return the whole modified string.
///
/// ```php
/// use Aerospike\{Exp, ExpStr};
///
/// // Records whose trimmed, folded name starts with "ali".
/// ExpStr::startsWith(
///     Exp::stringVal('ali'),
///     ExpStr::caseFold(false, ExpStr::trim(false, Exp::stringBin('name'))),
/// );
/// ```
///
/// # `noFail` instead of a policy class
///
/// The string write policy carries exactly one flag, so the modify methods take a
/// `bool $noFail` rather than an object. A class holding one boolean would be
/// ceremony; `BinPolicy` exists for the bitwise and HLL families because those
/// carry a write *mode* as well.
#[php_class]
#[php(name = "Aerospike\\ExpStr")]
#[derive(Debug)]
pub struct ExpStr;

#[php_impl]
impl ExpStr {
    /// Length in characters.
    pub fn strlen(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Strlen, src)?)
    }

    /// Length in bytes, which differs from `strlen` outside ASCII.
    pub fn byte_length(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::ByteLength, src)?)
    }

    /// The character at an index.
    pub fn char_at(index: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::CharAt {
            index: unwrap_tree(index, "index")?,
        }, src)?)
    }

    /// From an index to the end.
    pub fn substr(start: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Substr {
            start: unwrap_tree(start, "start")?,
        }, src)?)
    }

    /// A bounded range of characters.
    pub fn substr_range(start: &Expression, end: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::SubstrRange {
            start: unwrap_tree(start, "start")?,
            end: unwrap_tree(end, "end")?,
        }, src)?)
    }

    /// Index of the first occurrence of a needle, or -1.
    pub fn find(needle: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Find {
            needle: unwrap_tree(needle, "needle")?,
        }, src)?)
    }

    /// Index of the nth occurrence, counting from zero, or -1.
    pub fn find_nth(needle: &Expression, occurrence: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::FindNth {
            needle: unwrap_tree(needle, "needle")?,
            occurrence: unwrap_tree(occurrence, "occurrence")?,
        }, src)?)
    }

    /// Whether a needle appears at all.
    pub fn contains(needle: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Contains {
            needle: unwrap_tree(needle, "needle")?,
        }, src)?)
    }

    /// Whether the string starts with a prefix.
    pub fn starts_with(prefix: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::StartsWith {
            prefix: unwrap_tree(prefix, "prefix")?,
        }, src)?)
    }

    /// Whether the string ends with a suffix.
    pub fn ends_with(suffix: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::EndsWith {
            suffix: unwrap_tree(suffix, "suffix")?,
        }, src)?)
    }

    /// Whether every character is upper case.
    pub fn is_upper(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::IsUpper, src)?)
    }

    /// Whether every character is lower case.
    pub fn is_lower(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::IsLower, src)?)
    }

    /// Whether the string reads as a number.
    pub fn is_numeric(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::IsNumeric, src)?)
    }

    /// Whether the string reads as a number of a particular kind.
    pub fn is_numeric_typed(numericType: StringNumericType, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::IsNumericTyped {
            numeric_type: numericType.to_wire(),
        }, src)?)
    }

    /// Parse as an integer.
    pub fn to_integer(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::ToInteger, src)?)
    }

    /// Parse as a double.
    pub fn to_double(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::ToDouble, src)?)
    }

    /// The string's bytes, as a blob.
    pub fn to_blob(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::ToBlob, src)?)
    }

    /// Anything as its string form.
    pub fn to_string(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::ToString, src)?)
    }

    /// Decode base64 into a blob.
    pub fn b64_decode(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::B64Decode, src)?)
    }

    /// Split on whitespace.
    pub fn split(src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Split, src)?)
    }

    /// Split on a separator.
    pub fn split_by_separator(separator: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::SplitBySeparator {
            separator: unwrap_tree(separator, "separator")?,
        }, src)?)
    }

    /// Whether a pattern matches.
    ///
    /// The pattern is an expression here, so it is recompiled per record — unlike
    /// `Exp::regexCompare`, where it is a literal the server compiles once.
    pub fn regex_compare(pattern: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::RegexCompare {
            pattern: unwrap_tree(pattern, "pattern")?,
        }, src)?)
    }

    /// Whether a pattern matches, with `StringRegexFlag` flags OR-ed together.
    pub fn regex_compare_with_flags(pattern: &Expression, regexFlags: i64, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::RegexCompareWithFlags {
            pattern: unwrap_tree(pattern, "pattern")?,
            regex_flags: regexFlags,
        }, src)?)
    }

    /// Insert a value at an index.
    pub fn insert(noFail: bool, index: &Expression, value: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Insert {
            policy: WireStringPolicy { no_fail: noFail },
            index: unwrap_tree(index, "index")?,
            value: unwrap_tree(value, "value")?,
        }, src)?)
    }

    /// Overwrite from an index.
    pub fn overwrite(noFail: bool, index: &Expression, value: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Overwrite {
            policy: WireStringPolicy { no_fail: noFail },
            index: unwrap_tree(index, "index")?,
            value: unwrap_tree(value, "value")?,
        }, src)?)
    }

    /// Join a list of strings onto the end.
    pub fn concat(noFail: bool, values: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Concat {
            policy: WireStringPolicy { no_fail: noFail },
            values: unwrap_tree(values, "values")?,
        }, src)?)
    }

    /// Append one string.
    pub fn append(noFail: bool, value: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Append {
            policy: WireStringPolicy { no_fail: noFail },
            value: unwrap_tree(value, "value")?,
        }, src)?)
    }

    /// Prepend one string.
    pub fn prepend(noFail: bool, value: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Prepend {
            policy: WireStringPolicy { no_fail: noFail },
            value: unwrap_tree(value, "value")?,
        }, src)?)
    }

    /// Cut out a range of characters.
    pub fn snip(noFail: bool, start: &Expression, end: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Snip {
            policy: WireStringPolicy { no_fail: noFail },
            start: unwrap_tree(start, "start")?,
            end: unwrap_tree(end, "end")?,
        }, src)?)
    }

    /// Replace the first occurrence.
    pub fn replace(noFail: bool, needle: &Expression, replacement: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Replace {
            policy: WireStringPolicy { no_fail: noFail },
            needle: unwrap_tree(needle, "needle")?,
            replacement: unwrap_tree(replacement, "replacement")?,
        }, src)?)
    }

    /// Replace every occurrence.
    pub fn replace_all(noFail: bool, needle: &Expression, replacement: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::ReplaceAll {
            policy: WireStringPolicy { no_fail: noFail },
            needle: unwrap_tree(needle, "needle")?,
            replacement: unwrap_tree(replacement, "replacement")?,
        }, src)?)
    }

    /// Replace by pattern. Include the global flag to replace every match.
    pub fn regex_replace(noFail: bool, pattern: &Expression, replacement: &Expression, regexFlags: i64, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::RegexReplace {
            policy: WireStringPolicy { no_fail: noFail },
            pattern: unwrap_tree(pattern, "pattern")?,
            replacement: unwrap_tree(replacement, "replacement")?,
            regex_flags: regexFlags,
        }, src)?)
    }

    /// Upper case.
    pub fn upper(noFail: bool, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Upper {
            policy: WireStringPolicy { no_fail: noFail },
        }, src)?)
    }

    /// Lower case.
    pub fn lower(noFail: bool, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Lower {
            policy: WireStringPolicy { no_fail: noFail },
        }, src)?)
    }

    /// Case-fold, for case-insensitive comparison.
    ///
    /// Not the same as lower-casing: folding is defined for scripts where case does
    /// not map one-to-one, which is what makes it the right basis for comparison
    /// rather than for display.
    pub fn case_fold(noFail: bool, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::CaseFold {
            policy: WireStringPolicy { no_fail: noFail },
        }, src)?)
    }

    /// Normalise to Unicode NFC.
    pub fn normalize_nfc(noFail: bool, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::NormalizeNfc {
            policy: WireStringPolicy { no_fail: noFail },
        }, src)?)
    }

    /// Trim leading whitespace.
    pub fn trim_start(noFail: bool, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::TrimStart {
            policy: WireStringPolicy { no_fail: noFail },
        }, src)?)
    }

    /// Trim trailing whitespace.
    pub fn trim_end(noFail: bool, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::TrimEnd {
            policy: WireStringPolicy { no_fail: noFail },
        }, src)?)
    }

    /// Trim both ends.
    pub fn trim(noFail: bool, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Trim {
            policy: WireStringPolicy { no_fail: noFail },
        }, src)?)
    }

    /// Pad the start to a length.
    pub fn pad_start(noFail: bool, targetLength: &Expression, padString: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::PadStart {
            policy: WireStringPolicy { no_fail: noFail },
            target_length: unwrap_tree(targetLength, "targetLength")?,
            pad_string: unwrap_tree(padString, "padString")?,
        }, src)?)
    }

    /// Pad the end to a length.
    pub fn pad_end(noFail: bool, targetLength: &Expression, padString: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::PadEnd {
            policy: WireStringPolicy { no_fail: noFail },
            target_length: unwrap_tree(targetLength, "targetLength")?,
            pad_string: unwrap_tree(padString, "padString")?,
        }, src)?)
    }

    /// Repeat the string.
    pub fn repeat(noFail: bool, count: &Expression, src: &Expression) -> PhpResult<Expression> {
        Ok(str_node(WireExpStrOp::Repeat {
            policy: WireStringPolicy { no_fail: noFail },
            count: unwrap_tree(count, "count")?,
        }, src)?)
    }
}

/// CDT path expressions: `aerospike-core`'s `exp_select_*` and `exp_modify_*`.
///
/// Every other expression class addresses **one** node — a list element, a map
/// entry. These address **many**, because the path handed to them contains a
/// fan-out step: `Ctx::allChildren()`, or `Ctx::allChildrenWithFilter()` to visit
/// only the children a filter accepts. "The price of every book" is a path
/// expression; "the price of the first book" is an `ExpMap` read.
///
/// **Needs server 8.1.1 or later.** The fan-out is the server walking the
/// collection, not a loop here, and the daemon refuses against an older cluster
/// rather than letting it fail obscurely.
///
/// ```php
/// use Aerospike\{Exp, ExpPath, ExpType, Ctx, LoopVarPart};
///
/// // Every book priced over 20 — the filter runs per child, and the loop
/// // variable is the child being tested.
/// $dear = ExpPath::selectValues(ExpType::ListType, Exp::mapBin('books'), [
///     Ctx::allChildrenWithFilter(
///         Exp::gt(
///             ExpMap::getByKey(MapReturn::Value, ExpType::Integer,
///                 Exp::stringVal('price'), Exp::loopVar(ExpType::MapType, LoopVarPart::Value)),
///             Exp::intVal(20),
///         ),
///     ),
/// ]);
/// ```
///
/// # The path is an argument here, not `->context()`
///
/// The collection classes take their path fluently, because it is optional there —
/// most list reads have none. For a path expression the path *is* the operation:
/// without a fan-out step it selects one node and there was no reason to use this
/// class. So it is a required argument, in `aerospike-core`'s position (last).
#[php_class]
#[php(name = "Aerospike\\ExpPath")]
#[derive(Debug)]
pub struct ExpPath;

#[php_impl]
impl ExpPath {
    /// Select from each node the path reached, with an explicit `SelectFlag`.
    ///
    /// The general form. The four below are this with the flag filled in, and are
    /// what most code should use — they are literally the same server opcode.
    pub fn select_by_path(
        returnType: ExpType,
        flag: i64,
        bin: &Expression,
        ctx: Vec<&Zval>,
    ) -> PhpResult<Expression> {
        Ok(path_node(
            WireExpPathOp::SelectByPath { flag },
            returnType,
            bin,
            &ctx,
        )?)
    }

    /// The value of each selected node.
    pub fn select_values(
        returnType: ExpType,
        bin: &Expression,
        ctx: Vec<&Zval>,
    ) -> PhpResult<Expression> {
        Ok(path_node(WireExpPathOp::SelectValues, returnType, bin, &ctx)?)
    }

    /// The map key of each selected node.
    pub fn select_map_keys(
        returnType: ExpType,
        bin: &Expression,
        ctx: Vec<&Zval>,
    ) -> PhpResult<Expression> {
        Ok(path_node(
            WireExpPathOp::SelectMapKeys,
            returnType,
            bin,
            &ctx,
        )?)
    }

    /// The key and value of each selected node, as pairs.
    pub fn select_map_entries(
        returnType: ExpType,
        bin: &Expression,
        ctx: Vec<&Zval>,
    ) -> PhpResult<Expression> {
        Ok(path_node(
            WireExpPathOp::SelectMapEntries,
            returnType,
            bin,
            &ctx,
        )?)
    }

    /// The original structure with everything the path did not match pruned away.
    ///
    /// The others return a flat collection of what matched; this keeps the shape,
    /// so the answer still says *where* each match was.
    pub fn select_matching_tree(
        returnType: ExpType,
        bin: &Expression,
        ctx: Vec<&Zval>,
    ) -> PhpResult<Expression> {
        Ok(path_node(
            WireExpPathOp::SelectMatchingTree,
            returnType,
            bin,
            &ctx,
        )?)
    }

    /// Replace each selected node, with an explicit `ModifyFlag`.
    ///
    /// `modify` produces the new value and sees the node through
    /// [`Exp::loop_var`]. Return [`Exp::remove_result`] from it to delete that
    /// node instead, which is how a conditional removal is written in one pass.
    pub fn modify_by_path(
        returnType: ExpType,
        flag: i64,
        bin: &Expression,
        modify: &Expression,
        ctx: Vec<&Zval>,
    ) -> PhpResult<Expression> {
        Ok(path_node(
            WireExpPathOp::ModifyByPath {
                flag,
                modify: unwrap_tree(modify, "modify")?,
            },
            returnType,
            bin,
            &ctx,
        )?)
    }

    /// Replace each selected node, failing on a type mismatch.
    pub fn modify(
        returnType: ExpType,
        bin: &Expression,
        modify: &Expression,
        ctx: Vec<&Zval>,
    ) -> PhpResult<Expression> {
        Ok(path_node(
            WireExpPathOp::Modify {
                modify: unwrap_tree(modify, "modify")?,
            },
            returnType,
            bin,
            &ctx,
        )?)
    }

    /// Replace each selected node, ignoring type mismatches.
    pub fn modify_no_fail(
        returnType: ExpType,
        bin: &Expression,
        modify: &Expression,
        ctx: Vec<&Zval>,
    ) -> PhpResult<Expression> {
        Ok(path_node(
            WireExpPathOp::ModifyNoFail {
                modify: unwrap_tree(modify, "modify")?,
            },
            returnType,
            bin,
            &ctx,
        )?)
    }

    /// Remove each selected node.
    pub fn remove(
        returnType: ExpType,
        bin: &Expression,
        ctx: Vec<&Zval>,
    ) -> PhpResult<Expression> {
        Ok(path_node(WireExpPathOp::Remove, returnType, bin, &ctx)?)
    }
}

/// Flags for [`ExpPath::select_by_path`], combined with `|`.
///
/// Constants rather than an enum because `NO_FAIL` combines with the others —
/// PHP enum cases cannot be OR-ed. The four selections have named methods on
/// `ExpPath`, so these are for `NO_FAIL` and for the flag an application computes.
#[php_class]
#[php(name = "Aerospike\\SelectFlag")]
#[derive(Debug)]
pub struct SelectFlag;

#[php_impl]
impl SelectFlag {
    /// The tree from the root down, pruned to what matched.
    pub const MATCHING_TREE: i64 = 0;
    /// The value of each selected node.
    pub const VALUE: i64 = 1;
    /// Synonym for `VALUE`, when the nodes are list elements.
    pub const LIST_VALUE: i64 = 1;
    /// Synonym for `VALUE`, when the nodes are map values.
    pub const MAP_VALUE: i64 = 1;
    /// The map key of each selected node.
    pub const MAP_KEY: i64 = 2;
    /// The key and value of each selected node.
    pub const MAP_KEY_VALUE: i64 = 3;
    /// Skip nodes of the wrong type instead of failing.
    pub const NO_FAIL: i64 = 0x10;
}

/// Flags for [`ExpPath::modify_by_path`], combined with `|`.
#[php_class]
#[php(name = "Aerospike\\ModifyFlag")]
#[derive(Debug)]
pub struct ModifyFlag;

#[php_impl]
impl ModifyFlag {
    /// Fail on a type mismatch.
    pub const DEFAULT: i64 = 0;
    /// Skip nodes of the wrong type instead of failing.
    pub const NO_FAIL: i64 = 0x10;
}

// ===== helpers ==============================================================
//
// The guards live here rather than in the `#[php_impl]` methods above, and return
// [`AeroResult`] rather than `PhpResult`, for one practical reason: turning an
// `AeroError` into a `PhpException` needs the registered exception class, which
// only exists inside PHP. Keeping the rules in plain functions is what lets
// `cargo test` exercise them at all — the same split `src/query.rs` uses.

/// A `cond` node, arity checked.
///
/// An even list means the default is missing, which is the mistake this catches —
/// the server's answer for an even list says nothing about a default.
fn cond_tree(exps: Vec<WireExp>) -> AeroResult<WireExp> {
    if exps.len().is_multiple_of(2) {
        return Err(AeroError::client(format!(
            "cond() takes pairs of condition and result followed by one default, so an odd number \
             of expressions — {} is even, which means the default is missing",
            exps.len()
        )));
    }
    Ok(WireExp::Variadic {
        op: WireExpVariadicOp::Cond,
        exps,
    })
}

/// A `let` node, shape checked.
///
/// Everything but the last element must be a `def`, and the last must not be —
/// otherwise nothing uses the definitions. The server would refuse both too, with
/// a message about the packed form rather than about the call.
fn let_tree(exps: Vec<WireExp>) -> AeroResult<WireExp> {
    if exps.len() < 2 {
        return Err(AeroError::client(
            "let() needs at least one def() and the expression that uses it, so at least two \
             elements",
        ));
    }
    for (index, exp) in exps.iter().enumerate().take(exps.len() - 1) {
        if !matches!(exp, WireExp::Def { .. }) {
            return Err(AeroError::client(format!(
                "let() takes def() definitions followed by one expression; element {index} is not \
                 a def(). Only the last element may be something else"
            )));
        }
    }
    if matches!(exps.last(), Some(WireExp::Def { .. })) {
        return Err(AeroError::client(
            "let()'s last element is the expression that uses the definitions, and that one is a \
             def() too — so nothing uses them",
        ));
    }
    Ok(WireExp::Variadic {
        op: WireExpVariadicOp::Let,
        exps,
    })
}

/// A regex node, pattern checked.
fn regex_tree(regex: String, flags: i64, bin: &Expression) -> AeroResult<WireExp> {
    if regex.is_empty() {
        return Err(AeroError::client(
            "regexCompare() needs a pattern; \"\" matches everything, which a filter never means",
        ));
    }
    Ok(WireExp::Regex {
        regex,
        flags,
        bin: Box::new(unwrap_tree(bin, "the expression to match")?),
    })
}

/// A digest-modulo node, divisor checked.
fn digest_modulo_tree(modulo: i64) -> AeroResult<WireExp> {
    if modulo == 0 {
        return Err(AeroError::client(
            "digestModulo(0) would divide by zero; pass the number of buckets to divide the \
             keyspace into",
        ));
    }
    Ok(WireExp::DigestModulo(modulo))
}

/// A variable definition, name checked.
fn def_tree(name: String, value: &Expression) -> AeroResult<WireExp> {
    if name.trim().is_empty() {
        return Err(AeroError::client("a def() needs a name to bind to"));
    }
    Ok(WireExp::Def {
        name,
        value: Box::new(unwrap_tree(value, "the value")?),
    })
}

/// A variable reference, name checked.
fn var_tree(name: String) -> AeroResult<WireExp> {
    if name.trim().is_empty() {
        return Err(AeroError::client("a var() needs the name a def() bound"));
    }
    Ok(WireExp::Var(name))
}

/// A list-expression node, with no context path yet.
///
/// The path is attached afterwards with `Expression::context()`, which is how the
/// operation classes already do it — so a caller learns one convention rather
/// than two, and the thirty-one constructors keep `aerospike-core`'s argument
/// order without a trailing array on every one of them.
fn node(op: WireExpListOp, bin: &Expression) -> AeroResult<Expression> {
    Ok(Expression::from_tree(WireExp::List(Box::new(WireExpList {
        op,
        bin: unwrap_tree(bin, "the list")?,
        ctx: Vec::new(),
    }))))
}

/// A map-expression node. As [`node`], for maps.
fn map_node(op: WireExpMapOp, bin: &Expression) -> AeroResult<Expression> {
    Ok(Expression::from_tree(WireExp::Map(Box::new(WireExpMap {
        op,
        bin: unwrap_tree(bin, "the map")?,
        ctx: Vec::new(),
    }))))
}

/// An optional expression operand, for the range bounds.
///
/// `None` means unbounded, which is a *value* here rather than a missing
/// argument — so an omitted bound and an explicit `null` mean the same thing, as
/// they do everywhere else in this API.
fn optional_operand(
    exp: Option<&Expression>,
    position: &str,
) -> AeroResult<Option<WireExp>> {
    exp.map(|exp| unwrap_tree(exp, position)).transpose()
}

/// A bitwise-expression node.
fn bit_node(op: WireExpBitOp, bin: &Expression) -> AeroResult<Expression> {
    Ok(Expression::from_tree(WireExp::Bit(Box::new(WireExpBit {
        op,
        bin: unwrap_tree(bin, "the blob")?,
    }))))
}

/// A HyperLogLog-expression node.
fn hll_node(op: WireExpHllOp, bin: &Expression) -> AeroResult<Expression> {
    Ok(Expression::from_tree(WireExp::Hll(Box::new(WireExpHll {
        op,
        bin: unwrap_tree(bin, "the sketch")?,
    }))))
}

/// A string-expression node.
fn str_node(op: WireExpStrOp, src: &Expression) -> AeroResult<Expression> {
    Ok(Expression::from_tree(WireExp::Str(Box::new(WireExpStr {
        op,
        src: unwrap_tree(src, "the string")?,
    }))))
}

/// A path-expression node.
///
/// The path is required rather than fluent, for the reason [`ExpPath`] gives, and
/// it is checked for emptiness here: a path expression with no steps selects the
/// bin itself, which every other class does more directly.
fn path_node(
    op: WireExpPathOp,
    return_type: ExpType,
    bin: &Expression,
    ctx: &[&Zval],
) -> AeroResult<Expression> {
    if ctx.is_empty() {
        return Err(AeroError::client(
            "a path expression needs a path; with no steps it selects the bin itself, which \
             Exp::bin() already does. Add at least Ctx::allChildren() — the fan-out is the point",
        ));
    }
    Ok(Expression::from_tree(WireExp::Path(Box::new(WireExpPath {
        op,
        return_type: return_type.to_wire(),
        bin: unwrap_tree(bin, "the collection")?,
        ctx: context_list(ctx).map_err(|_| {
            AeroError::client("a path expression's path must be a list of Aerospike\\Ctx steps")
        })?,
    }))))
}

/// Wrap a tree as the `Expression` PHP holds.
fn tree(exp: WireExp) -> Expression {
    Expression::from_tree(exp)
}

/// A bin read as a fixed type — the ten typed shorthands.
fn typed_bin(name: String, exp_type: WireExpType) -> Expression {
    tree(WireExp::Bin { name, exp_type })
}

/// A metadata reader.
fn meta(meta: WireExpMeta) -> Expression {
    tree(WireExp::Meta(meta))
}

/// One-operand node.
fn unary(op: WireExpUnaryOp, exp: &Expression) -> AeroResult<Expression> {
    Ok(tree(WireExp::Unary {
        op,
        exp: Box::new(unwrap_tree(exp, "the operand")?),
    }))
}

/// Two-operand node, in `aerospike-core`'s argument order.
fn binary(
    op: WireExpBinaryOp,
    left: &Expression,
    right: &Expression,
) -> AeroResult<Expression> {
    Ok(tree(WireExp::Binary {
        op,
        left: Box::new(unwrap_tree(left, "the left operand")?),
        right: Box::new(unwrap_tree(right, "the right operand")?),
    }))
}

/// Any-operand node.
fn variadic(
    op: WireExpVariadicOp,
    exps: &[&Zval],
    method: &str,
) -> PhpResult<Expression> {
    Ok(tree(WireExp::Variadic {
        op,
        exps: expression_list(exps, method)?,
    }))
}

/// Extract a list of built expressions from a PHP array.
///
/// The same shape as [`crate::ops::operation_list`], and the same reason: a
/// wrong element is reported with its *position*, because a list of five
/// expressions with one mistake in it is otherwise a message about nothing in
/// particular.
fn expression_list(exps: &[&Zval], method: &str) -> PhpResult<Vec<WireExp>> {
    if exps.is_empty() {
        return Err(AeroError::client(format!(
            "{method}() needs at least one expression; an empty list has no meaning the server \
             would agree with. Write a literal — boolVal(true) or boolVal(false) — if a constant \
             is what you meant"
        ))
        .into());
    }
    let mut built = Vec::with_capacity(exps.len());
    for (index, zval) in exps.iter().enumerate() {
        let exp = zval.extract::<&Expression>().ok_or_else(|| {
            type_error(format!(
                "{method}()'s argument must be a list of Aerospike\\Expression; item {index} is {}",
                type_of(zval)
            ))
        })?;
        built.push(unwrap_tree(exp, &format!("item {index}"))?);
    }
    Ok(built)
}

/// The tree inside an `Expression`, or a refusal naming why there is none.
///
/// An expression built from text or from base64 is not a tree and cannot be an
/// *operand*: the builder composes trees, and the other two forms are already
/// whole expressions the server or another client produced. Mixing them would
/// mean asking the server to parse text in the middle of a packed tree, which is
/// not a thing the wire format can express.
fn unwrap_tree(exp: &Expression, position: &str) -> AeroResult<WireExp> {
    match exp.to_wire() {
        WireExpression::Tree(tree) => Ok(tree),
        WireExpression::Ael(_) => Err(AeroError::client(format!(
            "{position} is an Aerospike Expression Language expression, and those cannot be \
             combined with built ones: the server parses the text as a whole, so there is nowhere \
             to put a sub-expression. Write the whole expression in one form or the other"
        ))),
        WireExpression::Base64(_) => Err(AeroError::client(format!(
            "{position} is a packed expression from another client, and a packed expression is \
             already complete — it cannot be an operand of another. Use it on its own"
        ))),
    }
}

/// A name for a value shape, for the list/map refusals.
fn describe_value(value: &WireValue) -> &'static str {
    match value {
        WireValue::Nil => "null",
        WireValue::Bool(_) => "a bool",
        WireValue::Int(_) => "an int",
        WireValue::Float(_) => "a float",
        WireValue::Str(_) => "a string",
        WireValue::Blob(_) => "a Blob",
        WireValue::GeoJson(_) => "a GeoJson",
        WireValue::Hll(_) => "an Hll",
        WireValue::Infinity => "Infinity",
        WireValue::Wildcard => "Wildcard",
        WireValue::List(_) => "a list",
        WireValue::Map(_) | WireValue::OrderedMap(_) | WireValue::SortedMap(_) => "a map",
        _ => "a value this client only ever reads back",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::exp::WireLoopVarPart;

    /// The tree inside, for the assertions below.
    fn wire(exp: &Expression) -> WireExp {
        match exp.to_wire() {
            WireExpression::Tree(tree) => tree,
            other => panic!("the builder must produce a tree, got {other:?}"),
        }
    }

    #[test]
    fn a_typed_bin_carries_its_type() {
        assert_eq!(
            wire(&Exp::int_bin("age".into())),
            WireExp::Bin {
                name: "age".into(),
                exp_type: WireExpType::Int
            }
        );
        assert_eq!(
            wire(&Exp::geo_bin("region".into())),
            WireExp::Bin {
                name: "region".into(),
                exp_type: WireExpType::Geo
            }
        );
    }

    /// Ten typed shorthands, ten distinct types — a copy-paste slip here would be
    /// a bin read as the wrong type, which is a wrong answer and not an error.
    #[test]
    fn every_typed_bin_shorthand_has_its_own_type() {
        let bins = [
            (Exp::int_bin("b".into()), WireExpType::Int),
            (Exp::bool_bin("b".into()), WireExpType::Bool),
            (Exp::string_bin("b".into()), WireExpType::Str),
            (Exp::blob_bin("b".into()), WireExpType::Blob),
            (Exp::float_bin("b".into()), WireExpType::Float),
            (Exp::geo_bin("b".into()), WireExpType::Geo),
            (Exp::list_bin("b".into()), WireExpType::List),
            (Exp::map_bin("b".into()), WireExpType::Map),
            (Exp::hll_bin("b".into()), WireExpType::Hll),
        ];
        let mut seen = std::collections::HashSet::new();
        for (exp, expected) in bins {
            match wire(&exp) {
                WireExp::Bin { exp_type, .. } => {
                    assert_eq!(exp_type, expected);
                    assert!(seen.insert(exp_type), "{exp_type:?} appears twice");
                }
                other => panic!("expected a bin, got {other:?}"),
            }
        }
    }

    #[test]
    fn comparisons_keep_their_operand_order() {
        let exp = Exp::gt(&Exp::int_bin("a".into()), &Exp::int_val(5)).unwrap();
        assert_eq!(
            wire(&exp),
            WireExp::Binary {
                op: WireExpBinaryOp::Gt,
                left: Box::new(WireExp::Bin {
                    name: "a".into(),
                    exp_type: WireExpType::Int
                }),
                right: Box::new(WireExp::Value(WireValue::Int(5))),
            }
        );
    }

    #[test]
    fn every_literal_constructor_names_its_shape() {
        assert_eq!(wire(&Exp::int_val(1)), WireExp::Value(WireValue::Int(1)));
        assert_eq!(
            wire(&Exp::bool_val(true)),
            WireExp::Value(WireValue::Bool(true))
        );
        assert_eq!(
            wire(&Exp::string_val("s".into())),
            WireExp::Value(WireValue::Str("s".into()))
        );
        assert_eq!(
            wire(&Exp::float_val(1.5)),
            WireExp::Value(WireValue::Float(1.5))
        );
        assert_eq!(
            wire(&Exp::blob_val(vec![1, 2])),
            WireExp::Value(WireValue::Blob(vec![1, 2]))
        );
        assert_eq!(
            wire(&Exp::geo_val("{}".into())),
            WireExp::Value(WireValue::GeoJson("{}".into()))
        );
        assert_eq!(wire(&Exp::nil()), WireExp::Value(WireValue::Nil));
        assert_eq!(wire(&Exp::infinity()), WireExp::Value(WireValue::Infinity));
        assert_eq!(wire(&Exp::wildcard()), WireExp::Value(WireValue::Wildcard));
    }

    /// A text or packed expression is a whole expression, not an operand. Letting
    /// one through would produce a tree the daemon cannot build.
    #[test]
    fn a_text_expression_cannot_be_an_operand() {
        let ael = Expression::ael("$.a > 1".into()).unwrap();
        let error = unwrap_tree(&ael, "the operand").expect_err("AEL is not a tree");
        assert!(error.to_string().contains("cannot be combined"), "{error}");

        let packed = Expression::base64("kQE=".into()).unwrap();
        let error = unwrap_tree(&packed, "the operand").expect_err("packed is not a tree");
        assert!(error.to_string().contains("already complete"), "{error}");

        // And a built one is accepted, so the guard is not simply refusing
        // everything.
        assert!(unwrap_tree(&Exp::int_val(1), "the operand").is_ok());
    }

    /// `cond` without a default is the mistake this guard exists to catch: the
    /// server's own answer for an even list says nothing about a missing default.
    #[test]
    fn cond_requires_a_default() {
        let cond = WireExp::Value(WireValue::Bool(true));
        let result = WireExp::Value(WireValue::Int(1));
        let default = WireExp::Value(WireValue::Int(0));

        // condition, result, default.
        assert!(cond_tree(vec![cond.clone(), result.clone(), default]).is_ok());
        // A bare default on its own is legal — an odd list of one.
        assert!(cond_tree(vec![result.clone()]).is_ok());

        let error = cond_tree(vec![cond, result]).expect_err("no default");
        assert!(error.to_string().contains("default is missing"), "{error}");
    }

    /// `let` has two shape rules, and both catch a real mistake: definitions that
    /// are not definitions, and a body that is one — so nothing uses them.
    #[test]
    fn let_needs_definitions_then_one_expression() {
        let def = || WireExp::Def {
            name: "x".into(),
            value: Box::new(WireExp::Value(WireValue::Int(1))),
        };
        let body = || WireExp::Var("x".into());

        assert!(let_tree(vec![def(), body()]).is_ok());
        assert!(let_tree(vec![def(), def(), body()]).is_ok());

        let error = let_tree(vec![body()]).expect_err("one element is not a let");
        assert!(error.to_string().contains("at least two"), "{error}");

        let error = let_tree(vec![body(), body()]).expect_err("element 0 is not a def");
        assert!(error.to_string().contains("element 0 is not"), "{error}");

        let error = let_tree(vec![def(), def()]).expect_err("nothing uses the defs");
        assert!(error.to_string().contains("nothing uses them"), "{error}");
    }

    /// A path expression with no path selects the bin, which every other class does
    /// more directly — so an empty path is a mistake worth naming.
    #[test]
    fn a_path_expression_needs_a_path() {
        let bin = Exp::map_bin("books".into());
        let error = path_node(WireExpPathOp::SelectValues, ExpType::List, &bin, &[])
            .expect_err("an empty path is not a path expression");
        assert!(error.to_string().contains("the fan-out is the point"), "{error}");
    }

    /// The loop variable and the remove-result are leaves that carry their meaning
    /// in the node itself — three parts and ten types, all distinct.
    #[test]
    fn a_loop_variable_carries_its_type_and_part() {
        assert_eq!(
            wire(&Exp::loop_var(ExpType::Int, LoopVarPart::Value)),
            WireExp::LoopVar {
                exp_type: WireExpType::Int,
                part: WireLoopVarPart::VALUE,
            }
        );
        assert_eq!(
            wire(&Exp::loop_var(ExpType::Str, LoopVarPart::MapKey)),
            WireExp::LoopVar {
                exp_type: WireExpType::Str,
                part: WireLoopVarPart::MAP_KEY,
            }
        );

        let mut seen = std::collections::HashSet::new();
        for part in [LoopVarPart::MapKey, LoopVarPart::Value, LoopVarPart::Index] {
            assert!(seen.insert(part.to_wire()), "{part:?} maps to a used part");
        }
        assert_eq!(seen.len(), 3);

        assert_eq!(wire(&Exp::remove_result()), WireExp::RemoveResult);
    }

    /// The flag constants must match the server's numbers — the whole point of
    /// naming them is that nobody writes `2` by hand, so a wrong value here would
    /// silently select the wrong part of each node.
    #[test]
    fn the_path_flags_are_the_servers_numbers() {
        assert_eq!(SelectFlag::MATCHING_TREE, 0);
        assert_eq!(SelectFlag::VALUE, 1);
        assert_eq!(SelectFlag::LIST_VALUE, 1, "a synonym for VALUE");
        assert_eq!(SelectFlag::MAP_VALUE, 1, "and so is this");
        assert_eq!(SelectFlag::MAP_KEY, 2);
        assert_eq!(SelectFlag::MAP_KEY_VALUE, 3);
        assert_eq!(SelectFlag::NO_FAIL, 0x10);

        assert_eq!(ModifyFlag::DEFAULT, 0);
        assert_eq!(ModifyFlag::NO_FAIL, 0x10);

        // NO_FAIL is a bit, so it combines — which is why these are constants and
        // not an enum.
        assert_eq!(SelectFlag::VALUE | SelectFlag::NO_FAIL, 0x11);
    }

    #[test]
    fn digest_modulo_refuses_zero() {
        assert_eq!(digest_modulo_tree(100).unwrap(), WireExp::DigestModulo(100));
        let error = digest_modulo_tree(0).expect_err("that divides by zero");
        assert!(error.to_string().contains("divide by zero"), "{error}");
        // Negative is the server's business, not this guard's.
        assert!(digest_modulo_tree(-1).is_ok());
    }

    #[test]
    fn an_empty_regex_is_refused() {
        let bin = Exp::string_bin("s".into());
        assert!(regex_tree("^a".into(), 0, &bin).is_ok());
        let error = regex_tree(String::new(), 0, &bin).expect_err("empty pattern");
        assert!(error.to_string().contains("matches everything"), "{error}");
    }

    #[test]
    fn a_variable_needs_a_name() {
        assert_eq!(var_tree("x".into()).unwrap(), WireExp::Var("x".into()));
        let error = var_tree("  ".into()).expect_err("whitespace is not a name");
        assert!(error.to_string().contains("a def() bound"), "{error}");

        assert!(def_tree("x".into(), &Exp::int_val(1)).is_ok());
        let error = def_tree(" ".into(), &Exp::int_val(1)).expect_err("no name");
        assert!(error.to_string().contains("needs a name"), "{error}");
    }

    /// The metadata readers are ten distinct opcodes; two spelled the same way
    /// would be a silent wrong answer.
    #[test]
    fn every_metadata_reader_is_distinct() {
        let all = [
            wire(&Exp::key_exists()),
            wire(&Exp::set_name()),
            wire(&Exp::record_size()),
            wire(&Exp::device_size()),
            wire(&Exp::memory_size()),
            wire(&Exp::last_update()),
            wire(&Exp::since_update()),
            wire(&Exp::void_time()),
            wire(&Exp::ttl()),
            wire(&Exp::is_tombstone()),
        ];
        let mut seen = std::collections::HashSet::new();
        for exp in all {
            match exp {
                WireExp::Meta(meta) => {
                    assert!(seen.insert(meta), "{meta:?} appears twice");
                }
                other => panic!("expected metadata, got {other:?}"),
            }
        }
        assert_eq!(seen.len(), 10);
    }

    /// `exclusive` is documented as another spelling of `xor`, so the two must
    /// produce the same node — the daemon has one variant for both.
    #[test]
    fn exclusive_is_the_same_node_as_xor() {
        // Both go through `variadic` with the same op, which is the claim.
        let by_xor = WireExpVariadicOp::Xor;
        let by_exclusive = WireExpVariadicOp::Xor;
        assert_eq!(by_xor, by_exclusive);
    }
}
