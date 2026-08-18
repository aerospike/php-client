// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Expressions as a tree, built in PHP and assembled by the daemon.
//!
//! An Aerospike expression is evaluated by the server: as a policy filter it
//! decides whether a command applies to a record, and as an operation it computes
//! a value from one. There are three ways to say one over this contract, and
//! [`crate::op::WireExpression`] carries whichever was used:
//!
//! | form | built by | needs |
//! | --- | --- | --- |
//! | [`WireExpression::Ael`](crate::op::WireExpression::Ael) | the **server**, from text | server 8.1.3+ |
//! | [`WireExpression::Base64`](crate::op::WireExpression::Base64) | another client, already packed | nothing |
//! | [`WireExpression::Tree`](crate::op::WireExpression::Tree) | the **daemon**, from this module | nothing |
//!
//! # Why a tree and not more text
//!
//! The text form is smaller and the server does the parsing, so it was the only
//! form this contract had at first. But it is only available from server 8.1.3,
//! and expressions are the one part of the API where that matters most: a filter
//! is how a caller avoids reading records they do not want, and needing a very
//! recent server to express one at all is a hard limit rather than a missing
//! convenience.
//!
//! A tree is packed by the **client**, so it works against every server the rest
//! of this client supports. It is also checkable: a malformed tree is a Rust
//! `enum` that will not build, where malformed text is a parse error from a
//! server round trip away.
//!
//! # Why the tree is not one variant per function
//!
//! `aerospike-core` exposes around 240 expression constructors. Almost all of
//! them are the same few shapes with a different opcode — `eq`, `ne`, `gt`, `ge`,
//! `lt` and `le` differ only in which opcode a two-operand node carries — so this
//! module models the *shapes* and names the opcode as data:
//! [`WireExp::Unary`], [`WireExp::Binary`] and [`WireExp::Variadic`] cover about
//! ninety of those constructors between them.
//!
//! That is not a shortcut. It is what makes the daemon's translation a match on a
//! small enum rather than 240 arms that could each be wrong in its own way, and
//! it means adding an opcode is one variant on one enum on both sides.
//!
//! # Where a context path lives
//!
//! It does not live here. `aerospike-core` has no constructor that applies a
//! context to an arbitrary expression — a context is always an argument to a
//! collection operation — so a context travels on the list, map, bitwise, HLL or
//! string node that uses it, not as a node of its own.
//!
//! # Nesting is bounded, deliberately
//!
//! A tree arrives from another process, so its depth is that process's choice.
//! Recursion over untrusted depth is a stack overflow, which in a daemon serving
//! every worker on the host is not an acceptable failure. [`WireExp::depth`]
//! measures it and [`MAX_EXP_DEPTH`] is the limit the daemon enforces before it
//! recurses at all.

use serde::{Deserialize, Serialize};

use crate::op::{
    WireBitOverflow, WireBitPolicy, WireBitResize, WireCtx, WireHllPolicy, WireListPolicy,
    WireListReturn, WireListSort, WireMapPolicy, WireMapReturn,
};
use crate::WireValue;

/// How deeply expressions may nest.
///
/// Generous for anything hand-written — the expressions in Aerospike's own
/// documentation reach four or five — and small enough that the daemon's
/// recursive translation cannot exhaust its stack. Checked once, on the whole
/// tree, before translation starts: checking as it recurses would be the same
/// unbounded recursion it is meant to prevent.
pub const MAX_EXP_DEPTH: usize = 64;

/// How many nodes an expression may have.
///
/// A separate limit from the depth, because a wide tree is as expensive as a deep
/// one and neither bounds the other: 100 000 siblings is a flat tree of depth 2.
pub const MAX_EXP_NODES: usize = 4096;

/// The type an expression evaluates to.
///
/// Aerospike needs this where the type cannot be inferred from the expression
/// itself — reading a bin, reading the key, or pulling a value out of a
/// collection — because the server has to know how to interpret the bytes.
/// Mirrors `aerospike-core`'s `ExpType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WireExpType {
    /// Nil.
    Nil,
    /// Boolean.
    Bool,
    /// Integer.
    Int,
    /// String.
    Str,
    /// List.
    List,
    /// Map.
    Map,
    /// Byte string.
    Blob,
    /// Double.
    Float,
    /// GeoJSON.
    Geo,
    /// HyperLogLog sketch.
    Hll,
}

/// A record's metadata, as an expression.
///
/// These take no arguments and read something the server knows about the record
/// rather than something stored in it, which is why they are one enum rather than
/// ten variants of [`WireExp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WireExpMeta {
    /// Whether the record's key was stored with it. Needs `send_key` on the
    /// write that created it, so `false` is about the *write*, not the record.
    KeyExists,
    /// The record's set name, or `""` for a record in no set.
    SetName,
    /// Total record size in bytes, on device or in memory as configured.
    RecordSize,
    /// Record size on device in bytes. Zero for a namespace stored in memory.
    DeviceSize,
    /// Record size in memory in bytes. Zero for a namespace stored on device.
    MemorySize,
    /// When the record was last written, as nanoseconds since the Unix epoch.
    LastUpdate,
    /// Nanoseconds since the record was last written.
    SinceUpdate,
    /// When the record expires, as nanoseconds since the Unix epoch. Zero for a
    /// record that never expires.
    VoidTime,
    /// Seconds until the record expires. `-1` for a record that never does.
    Ttl,
    /// Whether this is a tombstone left by a durable delete.
    IsTombstone,
}

/// An operator taking exactly one expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WireExpUnaryOp {
    /// Logical negation.
    Not,
    /// Absolute value.
    NumAbs,
    /// Round down to an integer-valued float.
    NumFloor,
    /// Round up to an integer-valued float.
    NumCeil,
    /// Convert a float to an integer, truncating.
    ToInt,
    /// Convert an integer to a float.
    ToFloat,
    /// Bitwise complement of an integer.
    IntNot,
    /// Count the set bits in an integer.
    IntCount,
    /// The keys of a map, as a list.
    MapKeys,
    /// The values of a map, as a list.
    MapValues,
}

/// An operator taking exactly two expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WireExpBinaryOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Raise the left operand to the power of the right.
    NumPow,
    /// Logarithm of the left operand in the right operand's base.
    NumLog,
    /// Remainder of the left operand divided by the right.
    NumMod,
    /// Shift the left operand left by the right operand's bits.
    IntLshift,
    /// Shift right, filling with zeroes.
    IntRshift,
    /// Shift right, preserving the sign bit.
    IntArshift,
    /// Index of the left-most bit in the left operand matching the right.
    IntLscan,
    /// Index of the right-most bit in the left operand matching the right.
    IntRscan,
    /// Whether two GeoJSON regions relate — containment or intersection, as the
    /// documents themselves describe.
    GeoCompare,
    /// Whether the left operand appears in the list on the right.
    InList,
}

/// An operator taking any number of expressions.
///
/// Every one of these has a meaning for an empty list and for a single element,
/// and neither is checked here: `aerospike-core` passes the vector through and
/// the server decides. A caller that means "always true" should say so with a
/// boolean value rather than an empty `and`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WireExpVariadicOp {
    /// All of them.
    And,
    /// Any of them.
    Or,
    /// An odd number of them. Also what `exclusive` compiles to — the two are
    /// the same server opcode, which is why there is one variant.
    Xor,
    /// Sum.
    NumAdd,
    /// Left-to-right difference.
    NumSub,
    /// Product.
    NumMul,
    /// Left-to-right quotient.
    NumDiv,
    /// Bitwise AND of integers.
    IntAnd,
    /// Bitwise OR of integers.
    IntOr,
    /// Bitwise XOR of integers.
    IntXor,
    /// Smallest.
    Min,
    /// Largest.
    Max,
    /// Condition, result, condition, result, …, default. An odd number of
    /// expressions, ending in the default.
    Cond,
    /// Variable definitions followed by the expression that uses them. Every
    /// element but the last must be a [`WireExp::Def`].
    Let,
}

/// An expression, as a tree.
///
/// Recursive: an operator's operands are expressions, which is what lets the
/// three operator shapes cover most of `aerospike-core`'s surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireExp {
    /// A literal.
    ///
    /// Covers every `*_val` constructor plus `nil`, `infinity` and `wildcard` —
    /// [`WireValue`] already distinguishes all of them, and a second encoding of
    /// "an Aerospike value" would be one that could disagree with the first.
    Value(WireValue),

    /// A bin, read as `exp_type`.
    ///
    /// The type is required and not a hint: the server reads the bin's bytes as
    /// the type named, so naming the wrong one is a comparison against nonsense
    /// rather than an error. `aerospike-core`'s `int_bin`, `string_bin` and the
    /// rest are this with the type filled in.
    Bin {
        /// Bin name.
        name: String,
        /// How to read it.
        exp_type: WireExpType,
    },

    /// Whether a bin is present.
    BinExists(String),

    /// A bin's particle type, as the integer the server uses.
    BinType(String),

    /// The record's key, read as `exp_type`.
    ///
    /// Only present when the record was written with `send_key`, which is why
    /// [`WireExpMeta::KeyExists`] exists to ask first.
    Key(WireExpType),

    /// Something the server knows about the record rather than something in it.
    Meta(WireExpMeta),

    /// The record digest modulo `modulo`, for sampling a fraction of a set.
    DigestModulo(i64),

    /// One operand.
    Unary {
        /// What to do.
        op: WireExpUnaryOp,
        /// To what.
        exp: Box<WireExp>,
    },

    /// Two operands, in `aerospike-core`'s order.
    Binary {
        /// What to do.
        op: WireExpBinaryOp,
        /// Left operand.
        left: Box<WireExp>,
        /// Right operand.
        right: Box<WireExp>,
    },

    /// Any number of operands.
    Variadic {
        /// What to do.
        op: WireExpVariadicOp,
        /// To what, in order.
        exps: Vec<WireExp>,
    },

    /// Match a string bin against a regular expression.
    ///
    /// Not a [`WireExp::Binary`] because the pattern is *not* an expression: the
    /// server compiles it once, so it has to be a literal string, and the flags
    /// belong to the compilation rather than to the comparison.
    Regex {
        /// The pattern, as the server's regex engine reads it.
        regex: String,
        /// Compilation flags, OR-ed together.
        flags: i64,
        /// The expression to match, which must evaluate to a string.
        bin: Box<WireExp>,
    },

    /// Define a variable, for use inside a [`WireExpVariadicOp::Let`].
    Def {
        /// Variable name, as [`WireExp::Var`] refers to it.
        name: String,
        /// What it stands for.
        value: Box<WireExp>,
    },

    /// A variable defined by an enclosing `let`.
    Var(String),

    /// The value a *write* would have produced, for the expression operations.
    ///
    /// Meaningless as a filter, and the server says so rather than this contract:
    /// where it is legal depends on the command, which this module does not know.
    Unknown,

    /// An operation on a list. Boxed because it holds expressions of its own, and
    /// an unboxed variant would make every [`WireExp`] as large as the largest.
    List(Box<WireExpList>),

    /// An operation on a map.
    Map(Box<WireExpMap>),

    /// An operation on a byte string, bit by bit.
    Bit(Box<WireExpBit>),

    /// An operation on a HyperLogLog sketch.
    Hll(Box<WireExpHll>),

    /// An operation on a string. Needs server 8.1.3+, which the daemon checks.
    Str(Box<WireExpStr>),

    /// An operation over *every* node a path selects. Needs server 8.1.1+.
    Path(Box<WireExpPath>),

    /// Part of the node a fan-out is currently considering.
    ///
    /// Only meaningful inside a context filter or a modify expression — outside
    /// one there is no node being considered, and the server says so.
    LoopVar {
        /// How to read the part.
        exp_type: WireExpType,
        /// Which part.
        part: WireLoopVarPart,
    },

    /// Stand-in for "remove this node", as a modify expression's result.
    ///
    /// A modify expression normally produces the node's new value; this produces
    /// the instruction to delete it instead, which is how a conditional removal is
    /// written without a second pass.
    RemoveResult,
}

impl WireExp {
    /// How deeply this tree nests. A leaf is 1.
    ///
    /// Iterative, not recursive: this is the check that makes recursion safe, so
    /// it cannot be the thing that overflows the stack. It walks an explicit
    /// stack of `(node, depth)` and keeps the largest depth it reaches.
    #[must_use]
    pub fn depth(&self) -> usize {
        let mut deepest = 0;
        let mut stack = vec![(self, 1usize)];
        while let Some((node, depth)) = stack.pop() {
            deepest = deepest.max(depth);
            // Bail out once the answer can only be "too deep". Without this a
            // tree built to be pathological would still be walked in full.
            if deepest > MAX_EXP_DEPTH {
                return deepest;
            }
            node.children(&mut |child| stack.push((child, depth + 1)));
        }
        deepest
    }

    /// How many nodes this tree has.
    ///
    /// Iterative for the same reason as [`depth`](Self::depth), and stops
    /// counting once the answer can only be "too many".
    #[must_use]
    pub fn nodes(&self) -> usize {
        let mut counted = 0;
        let mut stack = vec![self];
        while let Some(node) = stack.pop() {
            counted += 1;
            if counted > MAX_EXP_NODES {
                return counted;
            }
            node.children(&mut |child| stack.push(child));
        }
        counted
    }

    /// Whether this tree is within both limits.
    ///
    /// # Errors
    /// The offending limit and the measurement, as a sentence naming what to do:
    /// a caller who hit either wrote the expression, and nothing else on the host
    /// can shorten it for them.
    pub fn check_size(&self) -> Result<(), String> {
        let depth = self.depth();
        if depth > MAX_EXP_DEPTH {
            return Err(format!(
                "that expression nests {depth} levels deep, and the limit is {MAX_EXP_DEPTH}. \
                 Bind a repeated sub-expression to a variable with a `let` instead of nesting it \
                 again"
            ));
        }
        let nodes = self.nodes();
        if nodes > MAX_EXP_NODES {
            return Err(format!(
                "that expression has {nodes} nodes, and the limit is {MAX_EXP_NODES}. An \
                 expression this large is usually a loop that should be a single collection \
                 operation — comparing against a list with `in_list`, for instance, rather than \
                 an `or` of one comparison per element"
            ));
        }
        Ok(())
    }

    /// Visit this node's immediate children.
    ///
    /// One place that knows the shape of every variant, so [`depth`](Self::depth)
    /// and [`nodes`](Self::nodes) cannot disagree about it, and a new variant is
    /// one addition rather than three.
    fn children<'a>(&'a self, visit: &mut impl FnMut(&'a WireExp)) {
        match self {
            WireExp::Value(_)
            | WireExp::Bin { .. }
            | WireExp::BinExists(_)
            | WireExp::BinType(_)
            | WireExp::Key(_)
            | WireExp::Meta(_)
            | WireExp::DigestModulo(_)
            | WireExp::Var(_)
            | WireExp::Unknown => {}
            WireExp::Unary { exp, .. } => visit(exp),
            WireExp::Binary { left, right, .. } => {
                visit(left);
                visit(right);
            }
            WireExp::Variadic { exps, .. } => {
                for exp in exps {
                    visit(exp);
                }
            }
            WireExp::Regex { bin, .. } => visit(bin),
            WireExp::Def { value, .. } => visit(value),
            // The collection families hold expressions in their arguments as well
            // as in `bin`, and every one of them counts: a tree that hid its depth
            // inside a list operation would defeat the limit entirely.
            WireExp::List(list) => {
                visit(&list.bin);
                list.op.args(visit);
                visit_ctx(&list.ctx, visit);
            }
            WireExp::Map(map) => {
                visit(&map.bin);
                map.op.args(visit);
                visit_ctx(&map.ctx, visit);
            }
            WireExp::Bit(bit) => {
                visit(&bit.bin);
                bit.op.args(visit);
            }
            WireExp::Hll(hll) => {
                visit(&hll.bin);
                hll.op.args(visit);
            }
            WireExp::Str(string) => {
                visit(&string.src);
                string.op.args(visit);
            }
            WireExp::Path(path) => {
                visit(&path.bin);
                path.op.args(visit);
                visit_ctx(&path.ctx, visit);
            }
            WireExp::LoopVar { .. } | WireExp::RemoveResult => {}
        }
    }
}

/// Visit the filter expression of every context step that has one.
///
/// A [`crate::op::WireCtx::AllChildrenWithFilter`] carries a whole expression, and
/// that expression is part of the tree for the purposes of [`WireExp::depth`] and
/// [`WireExp::nodes`]. Without this, a caller could bury unbounded nesting in a
/// context filter and the size check would not see it — the same hole the
/// collection families' `args()` walkers close, in the one place a *context* opens
/// it.
fn visit_ctx<'a>(ctx: &'a [WireCtx], visit: &mut impl FnMut(&'a WireExp)) {
    for step in ctx {
        if let Some(tree) = step.filter_tree() {
            visit(tree);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::{WireListReturnKind, WireMapReturnKind};
    use crate::{decode_body, encode_body};

    /// `a > 5 && b == "x"`, the shape almost every real filter has.
    fn filter() -> WireExp {
        WireExp::Variadic {
            op: WireExpVariadicOp::And,
            exps: vec![
                WireExp::Binary {
                    op: WireExpBinaryOp::Gt,
                    left: Box::new(WireExp::Bin {
                        name: "a".into(),
                        exp_type: WireExpType::Int,
                    }),
                    right: Box::new(WireExp::Value(WireValue::Int(5))),
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
        }
    }

    #[test]
    fn a_tree_round_trips() {
        let exp = filter();
        let bytes = encode_body(&exp).unwrap();
        assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
    }

    /// Every leaf is a leaf, and every operator counts its operands — the two
    /// facts the size check rests on.
    #[test]
    fn depth_and_node_counts_follow_the_shape() {
        let leaf = WireExp::Value(WireValue::Int(1));
        assert_eq!(leaf.depth(), 1);
        assert_eq!(leaf.nodes(), 1);

        // and( gt(bin, val), eq(bin, val) ): and=1, gt/eq=2, operands=3
        let exp = filter();
        assert_eq!(exp.depth(), 3);
        assert_eq!(exp.nodes(), 7);
        assert!(exp.check_size().is_ok());

        for meta in [WireExpMeta::Ttl, WireExpMeta::SetName] {
            assert_eq!(WireExp::Meta(meta).depth(), 1);
        }
    }

    /// The limits exist so that a tree from another process cannot overflow the
    /// daemon's stack, so they have to hold for a tree built to be pathological.
    #[test]
    fn a_tree_too_deep_is_refused_without_recursing_into_it() {
        let mut exp = WireExp::Value(WireValue::Int(0));
        for _ in 0..(MAX_EXP_DEPTH + 10) {
            exp = WireExp::Unary {
                op: WireExpUnaryOp::Not,
                exp: Box::new(exp),
            };
        }
        let error = exp.check_size().expect_err("that nests past the limit");
        assert!(error.contains("nests"), "{error}");
        assert!(error.contains(&MAX_EXP_DEPTH.to_string()), "{error}");

        // Depth and width bound each other not at all, so a flat tree has to be
        // refused on its own count.
        let wide = WireExp::Variadic {
            op: WireExpVariadicOp::Or,
            exps: (0..=MAX_EXP_NODES)
                .map(|n| WireExp::Value(WireValue::Int(n as i64)))
                .collect(),
        };
        assert_eq!(wide.depth(), 2, "a wide tree is shallow");
        let error = wide.check_size().expect_err("that is too many nodes");
        assert!(error.contains("nodes"), "{error}");
    }

    /// A tree exactly at each limit is allowed: the check is a ceiling, and an
    /// off-by-one here would refuse an expression that is fine.
    #[test]
    fn a_tree_exactly_at_the_limits_is_allowed() {
        let mut exp = WireExp::Value(WireValue::Int(0));
        for _ in 1..MAX_EXP_DEPTH {
            exp = WireExp::Unary {
                op: WireExpUnaryOp::Not,
                exp: Box::new(exp),
            };
        }
        assert_eq!(exp.depth(), MAX_EXP_DEPTH);
        assert!(exp.check_size().is_ok());

        let wide = WireExp::Variadic {
            op: WireExpVariadicOp::Or,
            exps: (0..MAX_EXP_NODES - 1)
                .map(|n| WireExp::Value(WireValue::Int(n as i64)))
                .collect(),
        };
        assert_eq!(wide.nodes(), MAX_EXP_NODES);
        assert!(wide.check_size().is_ok());
    }

    #[test]
    fn every_leaf_and_operator_round_trips() {
        let leaves = [
            WireExp::Value(WireValue::Nil),
            WireExp::Value(WireValue::Infinity),
            WireExp::Value(WireValue::Wildcard),
            WireExp::Value(WireValue::GeoJson("{\"type\":\"Point\"}".into())),
            WireExp::BinExists("a".into()),
            WireExp::BinType("a".into()),
            WireExp::Key(WireExpType::Blob),
            WireExp::DigestModulo(3),
            WireExp::Var("v".into()),
            WireExp::Unknown,
        ];
        for leaf in leaves {
            let bytes = encode_body(&leaf).unwrap();
            assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), leaf);
        }

        // Naming the opcode as data is the point of the three operator shapes, so
        // every opcode has to survive the trip.
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
                exp: Box::new(WireExp::Value(WireValue::Int(1))),
            };
            let bytes = encode_body(&exp).unwrap();
            assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
        }
        for op in [
            WireExpBinaryOp::Eq,
            WireExpBinaryOp::Ne,
            WireExpBinaryOp::Gt,
            WireExpBinaryOp::Ge,
            WireExpBinaryOp::Lt,
            WireExpBinaryOp::Le,
            WireExpBinaryOp::NumPow,
            WireExpBinaryOp::NumLog,
            WireExpBinaryOp::NumMod,
            WireExpBinaryOp::IntLshift,
            WireExpBinaryOp::IntRshift,
            WireExpBinaryOp::IntArshift,
            WireExpBinaryOp::IntLscan,
            WireExpBinaryOp::IntRscan,
            WireExpBinaryOp::GeoCompare,
            WireExpBinaryOp::InList,
        ] {
            let exp = WireExp::Binary {
                op,
                left: Box::new(WireExp::Value(WireValue::Int(1))),
                right: Box::new(WireExp::Value(WireValue::Int(2))),
            };
            let bytes = encode_body(&exp).unwrap();
            assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
        }
        for op in [
            WireExpVariadicOp::And,
            WireExpVariadicOp::Or,
            WireExpVariadicOp::Xor,
            WireExpVariadicOp::NumAdd,
            WireExpVariadicOp::NumSub,
            WireExpVariadicOp::NumMul,
            WireExpVariadicOp::NumDiv,
            WireExpVariadicOp::IntAnd,
            WireExpVariadicOp::IntOr,
            WireExpVariadicOp::IntXor,
            WireExpVariadicOp::Min,
            WireExpVariadicOp::Max,
            WireExpVariadicOp::Cond,
            WireExpVariadicOp::Let,
        ] {
            let exp = WireExp::Variadic {
                op,
                exps: vec![WireExp::Value(WireValue::Int(1))],
            };
            let bytes = encode_body(&exp).unwrap();
            assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
        }
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
            let exp = WireExp::Meta(meta);
            let bytes = encode_body(&exp).unwrap();
            assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
        }
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
            let bytes = encode_body(&exp).unwrap();
            assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
        }
    }

    /// A `let` and the `def`s it needs, and a `cond`: the two variadic shapes
    /// with rules about what their elements are.
    #[test]
    fn variable_binding_and_conditionals_round_trip() {
        let exp = WireExp::Variadic {
            op: WireExpVariadicOp::Let,
            exps: vec![
                WireExp::Def {
                    name: "x".into(),
                    value: Box::new(WireExp::Bin {
                        name: "a".into(),
                        exp_type: WireExpType::Int,
                    }),
                },
                WireExp::Variadic {
                    op: WireExpVariadicOp::Cond,
                    exps: vec![
                        WireExp::Binary {
                            op: WireExpBinaryOp::Gt,
                            left: Box::new(WireExp::Var("x".into())),
                            right: Box::new(WireExp::Value(WireValue::Int(0))),
                        },
                        WireExp::Var("x".into()),
                        WireExp::Value(WireValue::Int(0)),
                    ],
                },
            ],
        };
        let bytes = encode_body(&exp).unwrap();
        assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
        assert!(exp.check_size().is_ok());
    }

    /// A list operation and a map operation both round trip, and both are found by
    /// the size walk — the second matters more: an operation whose arguments the
    /// walker missed would be a hole in the depth limit.
    #[test]
    fn a_collection_operation_round_trips_and_its_arguments_are_counted() {
        let list = WireExp::List(Box::new(WireExpList {
            op: WireExpListOp::GetByIndex {
                return_type: WireListReturn::of(WireListReturnKind::Values),
                value_type: WireExpType::Int,
                index: WireExp::Value(WireValue::Int(0)),
            },
            bin: WireExp::Bin {
                name: "scores".into(),
                exp_type: WireExpType::List,
            },
            ctx: vec![],
        }));
        let bytes = encode_body(&list).unwrap();
        assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), list);
        // bin + index, plus the node itself.
        assert_eq!(list.nodes(), 3);
        assert_eq!(list.depth(), 2);

        let map = WireExp::Map(Box::new(WireExpMap {
            op: WireExpMapOp::GetByKey {
                return_type: WireMapReturn::of(WireMapReturnKind::Value),
                value_type: WireExpType::Str,
                key: WireExp::Value(WireValue::Str("k".into())),
            },
            bin: WireExp::Bin {
                name: "attrs".into(),
                exp_type: WireExpType::Map,
            },
            ctx: vec![WireCtx::MapKey {
                key: WireValue::Str("nested".into()),
            }],
        }));
        let bytes = encode_body(&map).unwrap();
        assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), map);
        assert_eq!(map.nodes(), 3);
    }

    /// The hole this closes: nesting *inside* a collection operation's arguments.
    /// If `args` missed a field, a caller could bury a thousand levels in an index
    /// expression and the depth check would report 2.
    #[test]
    fn nesting_inside_a_collection_argument_is_visible_to_the_limit() {
        let mut index = WireExp::Value(WireValue::Int(0));
        for _ in 0..(MAX_EXP_DEPTH + 5) {
            index = WireExp::Unary {
                op: WireExpUnaryOp::NumAbs,
                exp: Box::new(index),
            };
        }
        let hidden = WireExp::List(Box::new(WireExpList {
            op: WireExpListOp::GetByIndex {
                return_type: WireListReturn::of(WireListReturnKind::Values),
                value_type: WireExpType::Int,
                index,
            },
            bin: WireExp::Bin {
                name: "scores".into(),
                exp_type: WireExpType::List,
            },
            ctx: vec![],
        }));
        assert!(
            hidden.depth() > MAX_EXP_DEPTH,
            "depth inside an operation argument must count: got {}",
            hidden.depth()
        );
        assert!(hidden.check_size().is_err());
    }

    /// Both bounds of a range are optional and both are expressions, so both have
    /// to be walked — the `Option` arms are the easiest ones to get wrong.
    #[test]
    fn both_optional_range_bounds_are_counted() {
        let bounded = |begin: Option<WireExp>, end: Option<WireExp>| {
            WireExp::List(Box::new(WireExpList {
                op: WireExpListOp::GetByValueRange {
                    return_type: WireListReturn::of(WireListReturnKind::Count),
                    value_begin: begin,
                    value_end: end,
                },
                bin: WireExp::Bin {
                    name: "l".into(),
                    exp_type: WireExpType::List,
                },
                ctx: vec![],
            }))
        };
        let int = |n| WireExp::Value(WireValue::Int(n));
        assert_eq!(bounded(None, None).nodes(), 2, "just the node and its bin");
        assert_eq!(bounded(Some(int(1)), None).nodes(), 3);
        assert_eq!(bounded(None, Some(int(9))).nodes(), 3);
        assert_eq!(bounded(Some(int(1)), Some(int(9))).nodes(), 4);
    }

    /// A path expression round trips, and a fan-out step is what distinguishes it
    /// from an ordinary collection read.
    #[test]
    fn a_path_expression_round_trips() {
        let exp = WireExp::Path(Box::new(WireExpPath {
            op: WireExpPathOp::SelectValues,
            return_type: WireExpType::List,
            bin: WireExp::Bin {
                name: "books".into(),
                exp_type: WireExpType::Map,
            },
            ctx: vec![
                WireCtx::AllChildren,
                WireCtx::MapKey {
                    key: WireValue::Str("price".into()),
                },
            ],
        }));
        let bytes = encode_body(&exp).unwrap();
        assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
        assert!(exp.check_size().is_ok());

        for part in [
            WireLoopVarPart::MAP_KEY,
            WireLoopVarPart::VALUE,
            WireLoopVarPart::INDEX,
        ] {
            let var = WireExp::LoopVar {
                exp_type: WireExpType::Int,
                part,
            };
            let bytes = encode_body(&var).unwrap();
            assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), var);
            assert_eq!(var.depth(), 1, "a loop variable is a leaf");
        }

        let bytes = encode_body(&WireExp::RemoveResult).unwrap();
        assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), WireExp::RemoveResult);
    }

    /// Every path operation round trips, including the modify forms that carry an
    /// expression of their own.
    #[test]
    fn every_path_operation_round_trips() {
        let modify = || WireExp::Value(WireValue::Int(0));
        let ops = vec![
            WireExpPathOp::SelectByPath { flag: 1 },
            WireExpPathOp::SelectValues,
            WireExpPathOp::SelectMapKeys,
            WireExpPathOp::SelectMapEntries,
            WireExpPathOp::SelectMatchingTree,
            WireExpPathOp::ModifyByPath {
                flag: 0x10,
                modify: modify(),
            },
            WireExpPathOp::Modify { modify: modify() },
            WireExpPathOp::ModifyNoFail { modify: modify() },
            WireExpPathOp::Remove,
        ];
        assert_eq!(ops.len(), 9, "every path constructor the client has");
        for op in ops {
            let exp = WireExp::Path(Box::new(WireExpPath {
                op,
                return_type: WireExpType::List,
                bin: WireExp::Bin {
                    name: "b".into(),
                    exp_type: WireExpType::Map,
                },
                ctx: vec![WireCtx::AllChildren],
            }));
            let bytes = encode_body(&exp).unwrap();
            assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
        }
    }

    /// **The hole a context filter opens.** A filter is a whole expression living
    /// inside a *context*, so nesting buried there has to count toward the limits
    /// like nesting anywhere else. Before `visit_ctx` this tree measured 2.
    #[test]
    fn nesting_inside_a_context_filter_is_visible_to_the_limit() {
        let mut filter = WireExp::Value(WireValue::Bool(true));
        for _ in 0..(MAX_EXP_DEPTH + 5) {
            filter = WireExp::Unary {
                op: WireExpUnaryOp::Not,
                exp: Box::new(filter),
            };
        }
        let filtered = WireCtx::AllChildrenWithFilter {
            filter: Box::new(crate::op::WireExpression::Tree(filter)),
        };

        // On a path expression...
        let path = WireExp::Path(Box::new(WireExpPath {
            op: WireExpPathOp::SelectValues,
            return_type: WireExpType::List,
            bin: WireExp::Bin {
                name: "b".into(),
                exp_type: WireExpType::Map,
            },
            ctx: vec![filtered.clone()],
        }));
        assert!(
            path.depth() > MAX_EXP_DEPTH,
            "a context filter's depth must count: got {}",
            path.depth()
        );
        assert!(path.check_size().is_err());

        // ...and on an ordinary collection read, which takes contexts too.
        let list = WireExp::List(Box::new(WireExpList {
            op: WireExpListOp::Size,
            bin: WireExp::Bin {
                name: "b".into(),
                exp_type: WireExpType::List,
            },
            ctx: vec![filtered],
        }));
        assert!(list.depth() > MAX_EXP_DEPTH, "got {}", list.depth());
        assert!(list.check_size().is_err());
    }

    /// A filter written as text or already packed holds no tree, so there is
    /// nothing to walk — and `filter_tree` has to say so rather than pretending.
    #[test]
    fn a_non_tree_context_filter_has_no_nesting_to_count() {
        for filter in [
            crate::op::WireExpression::Ael("$.price > 1".into()),
            crate::op::WireExpression::Base64("kQE=".into()),
        ] {
            let step = WireCtx::AllChildrenWithFilter {
                filter: Box::new(filter),
            };
            assert!(step.filter_tree().is_none());

            let exp = WireExp::List(Box::new(WireExpList {
                op: WireExpListOp::Size,
                bin: WireExp::Bin {
                    name: "b".into(),
                    exp_type: WireExpType::List,
                },
                ctx: vec![step],
            }));
            assert_eq!(exp.nodes(), 2, "the node and its bin");
        }

        // An unfiltered fan-out has none either.
        assert!(WireCtx::AllChildren.filter_tree().is_none());
        assert!(WireCtx::ListIndex { index: 0 }.filter_tree().is_none());
    }

    #[test]
    fn a_regex_carries_its_flags_not_an_expression() {
        let exp = WireExp::Regex {
            regex: "^prefix".into(),
            flags: 1 | 2,
            bin: Box::new(WireExp::Bin {
                name: "name".into(),
                exp_type: WireExpType::Str,
            }),
        };
        let bytes = encode_body(&exp).unwrap();
        assert_eq!(decode_body::<WireExp>(&bytes).unwrap(), exp);
        assert_eq!(exp.depth(), 2, "the pattern is not a node");
        assert_eq!(exp.nodes(), 2);
    }
}

// ===== list expressions =====================================================

/// A list expression: an operation on a list, evaluated by the server.
///
/// Every argument is itself an expression — an index, a rank, a count, a value —
/// which is the whole difference from [`crate::op::WireListOp`], where those are
/// literals. That is what lets a filter say "the third element of the list in the
/// bin named by *another* bin", and it is why these cannot reuse the operation
/// enum.
///
/// `bin` is the list operated on. Usually [`WireExp::Bin`], but any expression
/// evaluating to a list will do — including another list expression, which is how
/// nested collection work composes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireExpList {
    /// What to do.
    pub op: WireExpListOp,
    /// The list to do it to.
    pub bin: WireExp,
    /// A path into a nested list, empty for the bin itself.
    pub ctx: Vec<WireCtx>,
}

/// What a [`WireExpList`] does.
///
/// Split by what the operation *needs*, which is also how `aerospike-core` splits
/// them: the ones that change the list take a [`WireListPolicy`], and the ones
/// that select from it take a [`WireListReturn`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireExpListOp {
    // ----- modify: these return the whole modified list -----
    /// Append one value.
    Append {
        /// Order and write flags.
        policy: WireListPolicy,
        /// What to append.
        value: WireExp,
    },
    /// Append every element of a list.
    AppendItems {
        /// Order and write flags.
        policy: WireListPolicy,
        /// The list whose elements are appended.
        list: WireExp,
    },
    /// Insert one value at an index.
    Insert {
        /// Order and write flags.
        policy: WireListPolicy,
        /// Where.
        index: WireExp,
        /// What.
        value: WireExp,
    },
    /// Insert every element of a list at an index.
    InsertItems {
        /// Order and write flags.
        policy: WireListPolicy,
        /// Where.
        index: WireExp,
        /// The list whose elements are inserted.
        list: WireExp,
    },
    /// Add to the number at an index.
    Increment {
        /// Order and write flags.
        policy: WireListPolicy,
        /// Which element.
        index: WireExp,
        /// How much to add.
        value: WireExp,
    },
    /// Replace the value at an index.
    Set {
        /// Order and write flags.
        policy: WireListPolicy,
        /// Which element.
        index: WireExp,
        /// The new value.
        value: WireExp,
    },
    /// Remove every element.
    Clear,
    /// Sort in place.
    Sort {
        /// Direction, and whether duplicates survive.
        flags: WireListSort,
    },

    // ----- remove: these return the whole list, minus what was selected -----
    /// Remove elements equal to a value.
    RemoveByValue {
        /// What to return.
        return_type: WireListReturn,
        /// The value to match.
        value: WireExp,
    },
    /// Remove elements equal to any value in a list.
    RemoveByValueList {
        /// What to return.
        return_type: WireListReturn,
        /// The values to match.
        values: WireExp,
    },
    /// Remove elements in a value range.
    ///
    /// `None` for either bound means unbounded on that side, which is the same
    /// thing `aerospike-core` means by `None` — not a missing argument.
    RemoveByValueRange {
        /// What to return.
        return_type: WireListReturn,
        /// Inclusive start, or unbounded.
        value_begin: Option<WireExp>,
        /// Exclusive end, or unbounded.
        value_end: Option<WireExp>,
    },
    /// Remove elements by rank relative to a value.
    RemoveByValueRelRankRange {
        /// What to return.
        return_type: WireListReturn,
        /// The value ranks are relative to.
        value: WireExp,
        /// Where to start, relative to that value's rank.
        rank: WireExp,
    },
    /// Remove a bounded number of elements by rank relative to a value.
    RemoveByValueRelRankRangeCount {
        /// What to return.
        return_type: WireListReturn,
        /// The value ranks are relative to.
        value: WireExp,
        /// Where to start.
        rank: WireExp,
        /// How many.
        count: WireExp,
    },
    /// Remove the element at an index.
    RemoveByIndex {
        /// What to return.
        return_type: WireListReturn,
        /// Which one.
        index: WireExp,
    },
    /// Remove from an index to the end.
    RemoveByIndexRange {
        /// What to return.
        return_type: WireListReturn,
        /// Where to start.
        index: WireExp,
    },
    /// Remove a bounded number of elements from an index.
    RemoveByIndexRangeCount {
        /// What to return.
        return_type: WireListReturn,
        /// Where to start.
        index: WireExp,
        /// How many.
        count: WireExp,
    },
    /// Remove the element at a rank.
    RemoveByRank {
        /// What to return.
        return_type: WireListReturn,
        /// Which one.
        rank: WireExp,
    },
    /// Remove from a rank to the highest.
    RemoveByRankRange {
        /// What to return.
        return_type: WireListReturn,
        /// Where to start.
        rank: WireExp,
    },
    /// Remove a bounded number of elements from a rank.
    RemoveByRankRangeCount {
        /// What to return.
        return_type: WireListReturn,
        /// Where to start.
        rank: WireExp,
        /// How many.
        count: WireExp,
    },

    // ----- read -----
    /// How many elements the list has.
    Size,
    /// Select elements equal to a value.
    GetByValue {
        /// What to return.
        return_type: WireListReturn,
        /// The value to match.
        value: WireExp,
    },
    /// Select elements in a value range.
    GetByValueRange {
        /// What to return.
        return_type: WireListReturn,
        /// Inclusive start, or unbounded.
        value_begin: Option<WireExp>,
        /// Exclusive end, or unbounded.
        value_end: Option<WireExp>,
    },
    /// Select elements equal to any value in a list.
    GetByValueList {
        /// What to return.
        return_type: WireListReturn,
        /// The values to match.
        values: WireExp,
    },
    /// Select by rank relative to a value.
    GetByValueRelRankRange {
        /// What to return.
        return_type: WireListReturn,
        /// The value ranks are relative to.
        value: WireExp,
        /// Where to start.
        rank: WireExp,
    },
    /// Select a bounded number by rank relative to a value.
    GetByValueRelRankRangeCount {
        /// What to return.
        return_type: WireListReturn,
        /// The value ranks are relative to.
        value: WireExp,
        /// Where to start.
        rank: WireExp,
        /// How many.
        count: WireExp,
    },
    /// Select the element at an index.
    ///
    /// Carries `value_type` because a single element comes back as itself rather
    /// than as a list, so the server has to be told how to read it — the same
    /// reason [`WireExp::Bin`] carries one.
    GetByIndex {
        /// What to return.
        return_type: WireListReturn,
        /// How to read the element.
        value_type: WireExpType,
        /// Which one.
        index: WireExp,
    },
    /// Select from an index to the end.
    GetByIndexRange {
        /// What to return.
        return_type: WireListReturn,
        /// Where to start.
        index: WireExp,
    },
    /// Select a bounded number of elements from an index.
    GetByIndexRangeCount {
        /// What to return.
        return_type: WireListReturn,
        /// Where to start.
        index: WireExp,
        /// How many.
        count: WireExp,
    },
    /// Select the element at a rank.
    GetByRank {
        /// What to return.
        return_type: WireListReturn,
        /// How to read the element.
        value_type: WireExpType,
        /// Which one.
        rank: WireExp,
    },
    /// Select from a rank to the highest.
    GetByRankRange {
        /// What to return.
        return_type: WireListReturn,
        /// Where to start.
        rank: WireExp,
    },
    /// Select a bounded number of elements from a rank.
    GetByRankRangeCount {
        /// What to return.
        return_type: WireListReturn,
        /// Where to start.
        rank: WireExp,
        /// How many.
        count: WireExp,
    },
}

// ===== map expressions ======================================================

/// A map expression: an operation on a map, evaluated by the server.
///
/// The same shape as [`WireExpList`] and for the same reasons — every argument is
/// an expression, and `bin` is whatever evaluates to the map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireExpMap {
    /// What to do.
    pub op: WireExpMapOp,
    /// The map to do it to.
    pub bin: WireExp,
    /// A path into a nested map, empty for the bin itself.
    pub ctx: Vec<WireCtx>,
}

/// What a [`WireExpMap`] does.
///
/// Where a list is addressed by index and rank, a map is addressed four ways —
/// key, value, index and rank — which is why there are more of these than there
/// are list operations rather than because maps do more.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireExpMapOp {
    // ----- modify -----
    /// Set one key.
    Put {
        /// Order and write mode.
        policy: WireMapPolicy,
        /// Which key.
        key: WireExp,
        /// The value.
        value: WireExp,
    },
    /// Set every key of another map.
    PutItems {
        /// Order and write mode.
        policy: WireMapPolicy,
        /// The map whose entries are written.
        map: WireExp,
    },
    /// Add to the number at a key.
    Increment {
        /// Order and write mode.
        policy: WireMapPolicy,
        /// Which key.
        key: WireExp,
        /// How much to add.
        incr: WireExp,
    },
    /// Remove every entry.
    Clear,

    // ----- remove by key -----
    /// Remove one key.
    RemoveByKey {
        /// What to return.
        return_type: WireMapReturn,
        /// Which key.
        key: WireExp,
    },
    /// Remove every key in a list.
    RemoveByKeyList {
        /// What to return.
        return_type: WireMapReturn,
        /// The keys.
        keys: WireExp,
    },
    /// Remove keys in a range.
    RemoveByKeyRange {
        /// What to return.
        return_type: WireMapReturn,
        /// Inclusive start, or unbounded.
        key_begin: Option<WireExp>,
        /// Exclusive end, or unbounded.
        key_end: Option<WireExp>,
    },
    /// Remove keys by index relative to a key.
    RemoveByKeyRelIndexRange {
        /// What to return.
        return_type: WireMapReturn,
        /// The key indexes are relative to.
        key: WireExp,
        /// Where to start.
        index: WireExp,
    },
    /// Remove a bounded number of keys by index relative to a key.
    RemoveByKeyRelIndexRangeCount {
        /// What to return.
        return_type: WireMapReturn,
        /// The key indexes are relative to.
        key: WireExp,
        /// Where to start.
        index: WireExp,
        /// How many.
        count: WireExp,
    },

    // ----- remove by value -----
    /// Remove entries with a value.
    RemoveByValue {
        /// What to return.
        return_type: WireMapReturn,
        /// The value to match.
        value: WireExp,
    },
    /// Remove entries whose value is in a list.
    RemoveByValueList {
        /// What to return.
        return_type: WireMapReturn,
        /// The values to match.
        values: WireExp,
    },
    /// Remove entries whose value is in a range.
    RemoveByValueRange {
        /// What to return.
        return_type: WireMapReturn,
        /// Inclusive start, or unbounded.
        value_begin: Option<WireExp>,
        /// Exclusive end, or unbounded.
        value_end: Option<WireExp>,
    },
    /// Remove entries by rank relative to a value.
    RemoveByValueRelRankRange {
        /// What to return.
        return_type: WireMapReturn,
        /// The value ranks are relative to.
        value: WireExp,
        /// Where to start.
        rank: WireExp,
    },
    /// Remove a bounded number of entries by rank relative to a value.
    RemoveByValueRelRankRangeCount {
        /// What to return.
        return_type: WireMapReturn,
        /// The value ranks are relative to.
        value: WireExp,
        /// Where to start.
        rank: WireExp,
        /// How many.
        count: WireExp,
    },

    // ----- remove by position -----
    /// Remove the entry at an index.
    RemoveByIndex {
        /// What to return.
        return_type: WireMapReturn,
        /// Which one.
        index: WireExp,
    },
    /// Remove from an index to the end.
    RemoveByIndexRange {
        /// What to return.
        return_type: WireMapReturn,
        /// Where to start.
        index: WireExp,
    },
    /// Remove a bounded number of entries from an index.
    RemoveByIndexRangeCount {
        /// What to return.
        return_type: WireMapReturn,
        /// Where to start.
        index: WireExp,
        /// How many.
        count: WireExp,
    },
    /// Remove the entry at a rank.
    RemoveByRank {
        /// What to return.
        return_type: WireMapReturn,
        /// Which one.
        rank: WireExp,
    },
    /// Remove from a rank to the highest.
    RemoveByRankRange {
        /// What to return.
        return_type: WireMapReturn,
        /// Where to start.
        rank: WireExp,
    },
    /// Remove a bounded number of entries from a rank.
    RemoveByRankRangeCount {
        /// What to return.
        return_type: WireMapReturn,
        /// Where to start.
        rank: WireExp,
        /// How many.
        count: WireExp,
    },

    // ----- read -----
    /// How many entries the map has.
    Size,
    /// The value at a key.
    ///
    /// Carries `value_type` because one value comes back as itself rather than as
    /// a collection, so the server has to be told how to read it.
    GetByKey {
        /// What to return.
        return_type: WireMapReturn,
        /// How to read the value.
        value_type: WireExpType,
        /// Which key.
        key: WireExp,
    },
    /// Entries whose key is in a range.
    GetByKeyRange {
        /// What to return.
        return_type: WireMapReturn,
        /// Inclusive start, or unbounded.
        key_begin: Option<WireExp>,
        /// Exclusive end, or unbounded.
        key_end: Option<WireExp>,
    },
    /// Entries whose key is in a list.
    GetByKeyList {
        /// What to return.
        return_type: WireMapReturn,
        /// The keys.
        keys: WireExp,
    },
    /// Entries by index relative to a key.
    GetByKeyRelIndexRange {
        /// What to return.
        return_type: WireMapReturn,
        /// The key indexes are relative to.
        key: WireExp,
        /// Where to start.
        index: WireExp,
    },
    /// A bounded number of entries by index relative to a key.
    GetByKeyRelIndexRangeCount {
        /// What to return.
        return_type: WireMapReturn,
        /// The key indexes are relative to.
        key: WireExp,
        /// Where to start.
        index: WireExp,
        /// How many.
        count: WireExp,
    },
    /// Entries with a value.
    GetByValue {
        /// What to return.
        return_type: WireMapReturn,
        /// The value to match.
        value: WireExp,
    },
    /// Entries whose value is in a range.
    GetByValueRange {
        /// What to return.
        return_type: WireMapReturn,
        /// Inclusive start, or unbounded.
        value_begin: Option<WireExp>,
        /// Exclusive end, or unbounded.
        value_end: Option<WireExp>,
    },
    /// Entries whose value is in a list.
    GetByValueList {
        /// What to return.
        return_type: WireMapReturn,
        /// The values to match.
        values: WireExp,
    },
    /// Entries by rank relative to a value.
    GetByValueRelRankRange {
        /// What to return.
        return_type: WireMapReturn,
        /// The value ranks are relative to.
        value: WireExp,
        /// Where to start.
        rank: WireExp,
    },
    /// A bounded number of entries by rank relative to a value.
    GetByValueRelRankRangeCount {
        /// What to return.
        return_type: WireMapReturn,
        /// The value ranks are relative to.
        value: WireExp,
        /// Where to start.
        rank: WireExp,
        /// How many.
        count: WireExp,
    },
    /// The entry at an index.
    GetByIndex {
        /// What to return.
        return_type: WireMapReturn,
        /// How to read the value.
        value_type: WireExpType,
        /// Which one.
        index: WireExp,
    },
    /// Entries from an index to the end.
    GetByIndexRange {
        /// What to return.
        return_type: WireMapReturn,
        /// Where to start.
        index: WireExp,
    },
    /// A bounded number of entries from an index.
    GetByIndexRangeCount {
        /// What to return.
        return_type: WireMapReturn,
        /// Where to start.
        index: WireExp,
        /// How many.
        count: WireExp,
    },
    /// The entry at a rank.
    GetByRank {
        /// What to return.
        return_type: WireMapReturn,
        /// How to read the value.
        value_type: WireExpType,
        /// Which one.
        rank: WireExp,
    },
    /// Entries from a rank to the highest.
    GetByRankRange {
        /// What to return.
        return_type: WireMapReturn,
        /// Where to start.
        rank: WireExp,
    },
    /// A bounded number of entries from a rank.
    GetByRankRangeCount {
        /// What to return.
        return_type: WireMapReturn,
        /// Where to start.
        rank: WireExp,
        /// How many.
        count: WireExp,
    },
}

impl WireExpListOp {
    /// Visit every expression this operation carries as an argument.
    ///
    /// Written out per variant and matched exhaustively, so a new variant with an
    /// expression argument does not compile until it appears here — which matters
    /// because [`WireExp::depth`] and [`WireExp::nodes`] rely on this to see the
    /// whole tree. A variant quietly omitted would let a caller hide unbounded
    /// nesting inside a list operation, which is the limit defeated.
    fn args<'a>(&'a self, visit: &mut impl FnMut(&'a WireExp)) {
        match self {
            WireExpListOp::Append { value, .. } => {
                visit(value);
            }
            WireExpListOp::AppendItems { list, .. } => {
                visit(list);
            }
            WireExpListOp::Insert { index, value, .. } => {
                visit(index);
                visit(value);
            }
            WireExpListOp::InsertItems { index, list, .. } => {
                visit(index);
                visit(list);
            }
            WireExpListOp::Increment { index, value, .. } => {
                visit(index);
                visit(value);
            }
            WireExpListOp::Set { index, value, .. } => {
                visit(index);
                visit(value);
            }
            WireExpListOp::Clear => {}
            WireExpListOp::Sort { .. } => {}
            WireExpListOp::RemoveByValue { value, .. } => {
                visit(value);
            }
            WireExpListOp::RemoveByValueList { values, .. } => {
                visit(values);
            }
            WireExpListOp::RemoveByValueRange { value_begin, value_end, .. } => {
                if let Some(value_begin) = value_begin {
                    visit(value_begin);
                }
                if let Some(value_end) = value_end {
                    visit(value_end);
                }
            }
            WireExpListOp::RemoveByValueRelRankRange { value, rank, .. } => {
                visit(value);
                visit(rank);
            }
            WireExpListOp::RemoveByValueRelRankRangeCount { value, rank, count, .. } => {
                visit(value);
                visit(rank);
                visit(count);
            }
            WireExpListOp::RemoveByIndex { index, .. } => {
                visit(index);
            }
            WireExpListOp::RemoveByIndexRange { index, .. } => {
                visit(index);
            }
            WireExpListOp::RemoveByIndexRangeCount { index, count, .. } => {
                visit(index);
                visit(count);
            }
            WireExpListOp::RemoveByRank { rank, .. } => {
                visit(rank);
            }
            WireExpListOp::RemoveByRankRange { rank, .. } => {
                visit(rank);
            }
            WireExpListOp::RemoveByRankRangeCount { rank, count, .. } => {
                visit(rank);
                visit(count);
            }
            WireExpListOp::Size => {}
            WireExpListOp::GetByValue { value, .. } => {
                visit(value);
            }
            WireExpListOp::GetByValueRange { value_begin, value_end, .. } => {
                if let Some(value_begin) = value_begin {
                    visit(value_begin);
                }
                if let Some(value_end) = value_end {
                    visit(value_end);
                }
            }
            WireExpListOp::GetByValueList { values, .. } => {
                visit(values);
            }
            WireExpListOp::GetByValueRelRankRange { value, rank, .. } => {
                visit(value);
                visit(rank);
            }
            WireExpListOp::GetByValueRelRankRangeCount { value, rank, count, .. } => {
                visit(value);
                visit(rank);
                visit(count);
            }
            WireExpListOp::GetByIndex { index, .. } => {
                visit(index);
            }
            WireExpListOp::GetByIndexRange { index, .. } => {
                visit(index);
            }
            WireExpListOp::GetByIndexRangeCount { index, count, .. } => {
                visit(index);
                visit(count);
            }
            WireExpListOp::GetByRank { rank, .. } => {
                visit(rank);
            }
            WireExpListOp::GetByRankRange { rank, .. } => {
                visit(rank);
            }
            WireExpListOp::GetByRankRangeCount { rank, count, .. } => {
                visit(rank);
                visit(count);
            }
        }
    }
}

impl WireExpMapOp {
    /// Visit every expression this operation carries as an argument.
    ///
    /// Written out per variant and matched exhaustively, for the reason
    /// [`WireExpListOp::args`] gives.
    fn args<'a>(&'a self, visit: &mut impl FnMut(&'a WireExp)) {
        match self {
            WireExpMapOp::Put { key, value, .. } => {
                visit(key);
                visit(value);
            }
            WireExpMapOp::PutItems { map, .. } => {
                visit(map);
            }
            WireExpMapOp::Increment { key, incr, .. } => {
                visit(key);
                visit(incr);
            }
            WireExpMapOp::Clear => {}
            WireExpMapOp::RemoveByKey { key, .. } => {
                visit(key);
            }
            WireExpMapOp::RemoveByKeyList { keys, .. } => {
                visit(keys);
            }
            WireExpMapOp::RemoveByKeyRange { key_begin, key_end, .. } => {
                if let Some(key_begin) = key_begin {
                    visit(key_begin);
                }
                if let Some(key_end) = key_end {
                    visit(key_end);
                }
            }
            WireExpMapOp::RemoveByKeyRelIndexRange { key, index, .. } => {
                visit(key);
                visit(index);
            }
            WireExpMapOp::RemoveByKeyRelIndexRangeCount { key, index, count, .. } => {
                visit(key);
                visit(index);
                visit(count);
            }
            WireExpMapOp::RemoveByValue { value, .. } => {
                visit(value);
            }
            WireExpMapOp::RemoveByValueList { values, .. } => {
                visit(values);
            }
            WireExpMapOp::RemoveByValueRange { value_begin, value_end, .. } => {
                if let Some(value_begin) = value_begin {
                    visit(value_begin);
                }
                if let Some(value_end) = value_end {
                    visit(value_end);
                }
            }
            WireExpMapOp::RemoveByValueRelRankRange { value, rank, .. } => {
                visit(value);
                visit(rank);
            }
            WireExpMapOp::RemoveByValueRelRankRangeCount { value, rank, count, .. } => {
                visit(value);
                visit(rank);
                visit(count);
            }
            WireExpMapOp::RemoveByIndex { index, .. } => {
                visit(index);
            }
            WireExpMapOp::RemoveByIndexRange { index, .. } => {
                visit(index);
            }
            WireExpMapOp::RemoveByIndexRangeCount { index, count, .. } => {
                visit(index);
                visit(count);
            }
            WireExpMapOp::RemoveByRank { rank, .. } => {
                visit(rank);
            }
            WireExpMapOp::RemoveByRankRange { rank, .. } => {
                visit(rank);
            }
            WireExpMapOp::RemoveByRankRangeCount { rank, count, .. } => {
                visit(rank);
                visit(count);
            }
            WireExpMapOp::Size => {}
            WireExpMapOp::GetByKey { key, .. } => {
                visit(key);
            }
            WireExpMapOp::GetByKeyRange { key_begin, key_end, .. } => {
                if let Some(key_begin) = key_begin {
                    visit(key_begin);
                }
                if let Some(key_end) = key_end {
                    visit(key_end);
                }
            }
            WireExpMapOp::GetByKeyList { keys, .. } => {
                visit(keys);
            }
            WireExpMapOp::GetByKeyRelIndexRange { key, index, .. } => {
                visit(key);
                visit(index);
            }
            WireExpMapOp::GetByKeyRelIndexRangeCount { key, index, count, .. } => {
                visit(key);
                visit(index);
                visit(count);
            }
            WireExpMapOp::GetByValue { value, .. } => {
                visit(value);
            }
            WireExpMapOp::GetByValueRange { value_begin, value_end, .. } => {
                if let Some(value_begin) = value_begin {
                    visit(value_begin);
                }
                if let Some(value_end) = value_end {
                    visit(value_end);
                }
            }
            WireExpMapOp::GetByValueList { values, .. } => {
                visit(values);
            }
            WireExpMapOp::GetByValueRelRankRange { value, rank, .. } => {
                visit(value);
                visit(rank);
            }
            WireExpMapOp::GetByValueRelRankRangeCount { value, rank, count, .. } => {
                visit(value);
                visit(rank);
                visit(count);
            }
            WireExpMapOp::GetByIndex { index, .. } => {
                visit(index);
            }
            WireExpMapOp::GetByIndexRange { index, .. } => {
                visit(index);
            }
            WireExpMapOp::GetByIndexRangeCount { index, count, .. } => {
                visit(index);
                visit(count);
            }
            WireExpMapOp::GetByRank { rank, .. } => {
                visit(rank);
            }
            WireExpMapOp::GetByRankRange { rank, .. } => {
                visit(rank);
            }
            WireExpMapOp::GetByRankRangeCount { rank, count, .. } => {
                visit(rank);
                visit(count);
            }
        }
    }
}

// ===== bitwise expressions ==================================================

/// A bitwise expression: an operation on a byte-string bin, evaluated by the
/// server.
///
/// No context path, unlike the collection families: the bitwise operations act on
/// a blob, and a blob has no nested structure to point into. `bin` is the blob
/// operated on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireExpBit {
    /// What to do.
    pub op: WireExpBitOp,
    /// The blob to do it to.
    pub bin: WireExp,
}

/// What a [`WireExpBit`] does.
///
/// Offsets and sizes are expressions, as everywhere else in this module. The
/// modify operations take a [`WireBitPolicy`] and return the whole modified blob;
/// the reads return the part they name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireExpBitOp {
    /// Grow or shrink to a byte size.
    Resize {
        /// Write flags.
        policy: WireBitPolicy,
        /// New size in bytes.
        byte_size: WireExp,
        /// Which end to change, and whether shrinking is allowed.
        resize_flags: WireBitResize,
    },
    /// Insert bytes at a byte offset.
    Insert {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bytes.
        byte_offset: WireExp,
        /// The bytes.
        value: WireExp,
    },
    /// Remove bytes at a byte offset.
    Remove {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bytes.
        byte_offset: WireExp,
        /// How many bytes.
        byte_size: WireExp,
    },
    /// Overwrite a bit range.
    Set {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// The bits.
        value: WireExp,
    },
    /// OR a bit range with a value.
    Or {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// The operand.
        value: WireExp,
    },
    /// XOR a bit range with a value.
    Xor {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// The operand.
        value: WireExp,
    },
    /// AND a bit range with a value.
    And {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// The operand.
        value: WireExp,
    },
    /// Invert a bit range.
    Not {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
    },
    /// Shift a bit range left.
    Lshift {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// How far.
        shift: WireExp,
    },
    /// Shift a bit range right.
    Rshift {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// How far.
        shift: WireExp,
    },
    /// Add to the integer in a bit range.
    Add {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// How much to add.
        value: WireExp,
        /// Whether the field is signed.
        signed: bool,
        /// What to do when the result does not fit.
        action: WireBitOverflow,
    },
    /// Subtract from the integer in a bit range.
    Subtract {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// How much to subtract.
        value: WireExp,
        /// Whether the field is signed.
        signed: bool,
        /// What to do when the result does not fit.
        action: WireBitOverflow,
    },
    /// Write an integer into a bit range.
    SetInt {
        /// Write flags.
        policy: WireBitPolicy,
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// The integer.
        value: WireExp,
    },

    // ----- read -----
    /// Read a bit range as a blob.
    Get {
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
    },
    /// Count the set bits in a bit range.
    Count {
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
    },
    /// Index of the left-most bit in a range matching `value`.
    Lscan {
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// The bit value to find: true or false.
        value: WireExp,
    },
    /// Index of the right-most bit in a range matching `value`.
    Rscan {
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// The bit value to find.
        value: WireExp,
    },
    /// Read a bit range as an integer.
    GetInt {
        /// Where, in bits.
        bit_offset: WireExp,
        /// How many bits.
        bit_size: WireExp,
        /// Whether to read it as signed.
        signed: bool,
    },
}

// ===== HyperLogLog expressions ==============================================

/// A HyperLogLog expression: an operation on an HLL bin, evaluated by the server.
///
/// No context path, for the same reason [`WireExpBit`] has none: a sketch is
/// opaque bytes with no addressable structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireExpHll {
    /// What to do.
    pub op: WireExpHllOp,
    /// The sketch to do it to.
    pub bin: WireExp,
}

/// What a [`WireExpHll`] does.
///
/// The `list` arguments are lists of values to add or of *other sketches* to
/// combine with — which one depends on the operation, and each says so.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireExpHllOp {
    /// Create or reset a sketch.
    Init {
        /// Write flags.
        policy: WireHllPolicy,
        /// Precision, as a bit count.
        index_bit_count: WireExp,
    },
    /// Create or reset a sketch that also supports similarity estimates.
    InitWithMinHash {
        /// Write flags.
        policy: WireHllPolicy,
        /// Precision, as a bit count.
        index_bit_count: WireExp,
        /// MinHash bit count.
        min_hash_count: WireExp,
    },
    /// Add values to a sketch.
    Add {
        /// Write flags.
        policy: WireHllPolicy,
        /// The values to add.
        list: WireExp,
    },
    /// Add values, creating the sketch with a precision if it does not exist.
    AddWithIndex {
        /// Write flags.
        policy: WireHllPolicy,
        /// The values to add.
        list: WireExp,
        /// Precision for a sketch that has to be created.
        index_bit_count: WireExp,
    },
    /// Add values, creating the sketch with a precision and MinHash count.
    AddWithIndexAndMinHash {
        /// Write flags.
        policy: WireHllPolicy,
        /// The values to add.
        list: WireExp,
        /// Precision for a sketch that has to be created.
        index_bit_count: WireExp,
        /// MinHash bit count.
        min_hash_count: WireExp,
    },

    // ----- read -----
    /// The estimated number of distinct values.
    GetCount,
    /// A sketch that is the union of this one and the others.
    GetUnion {
        /// The other sketches.
        list: WireExp,
    },
    /// The estimated size of that union, without building it.
    GetUnionCount {
        /// The other sketches.
        list: WireExp,
    },
    /// The estimated size of the intersection.
    GetIntersectCount {
        /// The other sketches.
        list: WireExp,
    },
    /// The estimated Jaccard similarity, from 0.0 to 1.0.
    GetSimilarity {
        /// The other sketches.
        list: WireExp,
    },
    /// The sketch's own parameters: its index bit count and MinHash count.
    Describe,
    /// Whether a value may be in the sketch.
    ///
    /// A HyperLogLog answers this with false positives and no false negatives, so
    /// `false` is certain and `true` is probable — which is what makes it cheap.
    MayContain {
        /// The values to test.
        list: WireExp,
    },
}

// ===== string expressions ===================================================

/// Write semantics for the string operations that change a string.
///
/// One boolean rather than a bitmask, for the reason
/// [`crate::op::WireListWriteFlags`] gives: a bitmask crossing a language boundary
/// is a number nobody can read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireStringPolicy {
    /// Do not fail the operation when it cannot be applied; leave the bin alone
    /// and return nil for that step.
    ///
    /// Note that a *missing* bin is not that case: the additive operations create
    /// the bin from an empty string and the others are a silent no-op, with or
    /// without this flag.
    pub no_fail: bool,
}

/// Which numbers [`WireExpStrOp::IsNumericTyped`] accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireStringNumericType {
    /// An integer or a floating-point number.
    Any,
    /// Only integers.
    Int,
    /// Only floating-point numbers.
    Float,
}

/// A string expression: an operation on a string, evaluated by the server.
///
/// Needs **server 8.1.3 or later**, which is the same requirement the string
/// *operations* carry — the server opcodes are the same ones. The daemon checks
/// and refuses rather than letting an older server reject the operation obscurely.
///
/// `src` is the string operated on: usually [`WireExp::Bin`] with
/// [`WireExpType::Str`], but any expression evaluating to a string, which is what
/// lets these chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireExpStr {
    /// What to do.
    pub op: WireExpStrOp,
    /// The string to do it to.
    pub src: WireExp,
}

/// What a [`WireExpStr`] does.
///
/// Offsets and lengths are in **characters**, not bytes, except where a name says
/// otherwise ([`ByteLength`](Self::ByteLength), [`ToBlob`](Self::ToBlob)) — the
/// server works in Unicode code points here, so a multi-byte character counts
/// once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireExpStrOp {
    // ----- measure and inspect -----
    /// Length in characters.
    Strlen,
    /// Length in bytes, which differs from [`Strlen`](Self::Strlen) for anything
    /// outside ASCII.
    ByteLength,
    /// The character at an index.
    CharAt {
        /// Which character.
        index: WireExp,
    },
    /// From an index to the end.
    Substr {
        /// Where to start.
        start: WireExp,
    },
    /// A bounded range of characters.
    SubstrRange {
        /// Where to start.
        start: WireExp,
        /// Where to stop.
        end: WireExp,
    },
    /// Index of the first occurrence of a needle, or -1.
    Find {
        /// What to look for.
        needle: WireExp,
    },
    /// Index of the nth occurrence of a needle, or -1.
    FindNth {
        /// What to look for.
        needle: WireExp,
        /// Which occurrence, counting from zero.
        occurrence: WireExp,
    },
    /// Whether a needle appears at all.
    Contains {
        /// What to look for.
        needle: WireExp,
    },
    /// Whether the string starts with a prefix.
    StartsWith {
        /// The prefix.
        prefix: WireExp,
    },
    /// Whether the string ends with a suffix.
    EndsWith {
        /// The suffix.
        suffix: WireExp,
    },
    /// Whether every character is upper case.
    IsUpper,
    /// Whether every character is lower case.
    IsLower,
    /// Whether the string reads as a number.
    IsNumeric,
    /// Whether the string reads as a number of a particular kind.
    IsNumericTyped {
        /// Which kinds count.
        numeric_type: WireStringNumericType,
    },

    // ----- convert -----
    /// Parse as an integer.
    ToInteger,
    /// Parse as a double.
    ToDouble,
    /// The string's bytes, as a blob.
    ToBlob,
    /// Anything as its string form.
    ToString,
    /// Decode base64 into a blob.
    B64Decode,
    /// Split on whitespace.
    Split,
    /// Split on a separator.
    SplitBySeparator {
        /// The separator.
        separator: WireExp,
    },

    // ----- match -----
    /// Whether a pattern matches.
    ///
    /// The pattern is an expression here, unlike [`WireExp::Regex`] where it is a
    /// literal the server compiles once — these are different server operations,
    /// and this one recompiles per record.
    RegexCompare {
        /// The pattern.
        pattern: WireExp,
    },
    /// Whether a pattern matches, with flags.
    RegexCompareWithFlags {
        /// The pattern.
        pattern: WireExp,
        /// ICU regex flags, OR-ed together.
        regex_flags: i64,
    },

    // ----- modify: these return the whole modified string -----
    /// Insert a value at an index.
    Insert {
        /// Write flags.
        policy: WireStringPolicy,
        /// Where.
        index: WireExp,
        /// What.
        value: WireExp,
    },
    /// Overwrite from an index.
    Overwrite {
        /// Write flags.
        policy: WireStringPolicy,
        /// Where.
        index: WireExp,
        /// What.
        value: WireExp,
    },
    /// Join a list of strings onto the end.
    Concat {
        /// Write flags.
        policy: WireStringPolicy,
        /// The strings to append, as a list.
        values: WireExp,
    },
    /// Append one string.
    Append {
        /// Write flags.
        policy: WireStringPolicy,
        /// What to append.
        value: WireExp,
    },
    /// Prepend one string.
    Prepend {
        /// Write flags.
        policy: WireStringPolicy,
        /// What to prepend.
        value: WireExp,
    },
    /// Cut out a range of characters.
    Snip {
        /// Write flags.
        policy: WireStringPolicy,
        /// Where to start.
        start: WireExp,
        /// Where to stop.
        end: WireExp,
    },
    /// Replace the first occurrence.
    Replace {
        /// Write flags.
        policy: WireStringPolicy,
        /// What to replace.
        needle: WireExp,
        /// What with.
        replacement: WireExp,
    },
    /// Replace every occurrence.
    ReplaceAll {
        /// Write flags.
        policy: WireStringPolicy,
        /// What to replace.
        needle: WireExp,
        /// What with.
        replacement: WireExp,
    },
    /// Replace by pattern.
    RegexReplace {
        /// Write flags.
        policy: WireStringPolicy,
        /// The pattern.
        pattern: WireExp,
        /// What to replace matches with.
        replacement: WireExp,
        /// ICU regex flags. Include the global flag to replace every match.
        regex_flags: i64,
    },
    /// Upper case.
    Upper {
        /// Write flags.
        policy: WireStringPolicy,
    },
    /// Lower case.
    Lower {
        /// Write flags.
        policy: WireStringPolicy,
    },
    /// Case-fold, for case-insensitive comparison.
    ///
    /// Not the same as lower-casing: folding is defined for scripts where case
    /// does not map one-to-one, which is what makes it the right basis for a
    /// comparison rather than for display.
    CaseFold {
        /// Write flags.
        policy: WireStringPolicy,
    },
    /// Normalise to Unicode NFC.
    NormalizeNfc {
        /// Write flags.
        policy: WireStringPolicy,
    },
    /// Trim leading whitespace.
    TrimStart {
        /// Write flags.
        policy: WireStringPolicy,
    },
    /// Trim trailing whitespace.
    TrimEnd {
        /// Write flags.
        policy: WireStringPolicy,
    },
    /// Trim both ends.
    Trim {
        /// Write flags.
        policy: WireStringPolicy,
    },
    /// Pad the start to a length.
    PadStart {
        /// Write flags.
        policy: WireStringPolicy,
        /// The length to reach.
        target_length: WireExp,
        /// What to pad with, repeated as needed.
        pad_string: WireExp,
    },
    /// Pad the end to a length.
    PadEnd {
        /// Write flags.
        policy: WireStringPolicy,
        /// The length to reach.
        target_length: WireExp,
        /// What to pad with.
        pad_string: WireExp,
    },
    /// Repeat the string.
    Repeat {
        /// Write flags.
        policy: WireStringPolicy,
        /// How many times.
        count: WireExp,
    },
}

impl WireExpBitOp {
    /// Visit every expression this operation carries as an argument.
    ///
    /// Written out per variant and matched exhaustively, for the reason
    /// [`WireExpListOp::args`] gives.
    fn args<'a>(&'a self, visit: &mut impl FnMut(&'a WireExp)) {
        match self {
            WireExpBitOp::Resize { byte_size, .. } => {
                visit(byte_size);
            }
            WireExpBitOp::Insert { byte_offset, value, .. } => {
                visit(byte_offset);
                visit(value);
            }
            WireExpBitOp::Remove { byte_offset, byte_size, .. } => {
                visit(byte_offset);
                visit(byte_size);
            }
            WireExpBitOp::Set { bit_offset, bit_size, value, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(value);
            }
            WireExpBitOp::Or { bit_offset, bit_size, value, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(value);
            }
            WireExpBitOp::Xor { bit_offset, bit_size, value, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(value);
            }
            WireExpBitOp::And { bit_offset, bit_size, value, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(value);
            }
            WireExpBitOp::Not { bit_offset, bit_size, .. } => {
                visit(bit_offset);
                visit(bit_size);
            }
            WireExpBitOp::Lshift { bit_offset, bit_size, shift, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(shift);
            }
            WireExpBitOp::Rshift { bit_offset, bit_size, shift, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(shift);
            }
            WireExpBitOp::Add { bit_offset, bit_size, value, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(value);
            }
            WireExpBitOp::Subtract { bit_offset, bit_size, value, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(value);
            }
            WireExpBitOp::SetInt { bit_offset, bit_size, value, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(value);
            }
            WireExpBitOp::Get { bit_offset, bit_size, .. } => {
                visit(bit_offset);
                visit(bit_size);
            }
            WireExpBitOp::Count { bit_offset, bit_size, .. } => {
                visit(bit_offset);
                visit(bit_size);
            }
            WireExpBitOp::Lscan { bit_offset, bit_size, value, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(value);
            }
            WireExpBitOp::Rscan { bit_offset, bit_size, value, .. } => {
                visit(bit_offset);
                visit(bit_size);
                visit(value);
            }
            WireExpBitOp::GetInt { bit_offset, bit_size, .. } => {
                visit(bit_offset);
                visit(bit_size);
            }
        }
    }
}

impl WireExpHllOp {
    /// Visit every expression this operation carries as an argument.
    ///
    /// As [`WireExpBitOp::args`].
    fn args<'a>(&'a self, visit: &mut impl FnMut(&'a WireExp)) {
        match self {
            WireExpHllOp::Init { index_bit_count, .. } => {
                visit(index_bit_count);
            }
            WireExpHllOp::InitWithMinHash { index_bit_count, min_hash_count, .. } => {
                visit(index_bit_count);
                visit(min_hash_count);
            }
            WireExpHllOp::Add { list, .. } => {
                visit(list);
            }
            WireExpHllOp::AddWithIndex { list, index_bit_count, .. } => {
                visit(list);
                visit(index_bit_count);
            }
            WireExpHllOp::AddWithIndexAndMinHash { list, index_bit_count, min_hash_count, .. } => {
                visit(list);
                visit(index_bit_count);
                visit(min_hash_count);
            }
            WireExpHllOp::GetCount => {}
            WireExpHllOp::GetUnion { list, .. } => {
                visit(list);
            }
            WireExpHllOp::GetUnionCount { list, .. } => {
                visit(list);
            }
            WireExpHllOp::GetIntersectCount { list, .. } => {
                visit(list);
            }
            WireExpHllOp::GetSimilarity { list, .. } => {
                visit(list);
            }
            WireExpHllOp::Describe => {}
            WireExpHllOp::MayContain { list, .. } => {
                visit(list);
            }
        }
    }
}

impl WireExpStrOp {
    /// Visit every expression this operation carries as an argument.
    ///
    /// As [`WireExpBitOp::args`]. The longest of these, and the one where a
    /// forgotten field would be least obvious.
    fn args<'a>(&'a self, visit: &mut impl FnMut(&'a WireExp)) {
        match self {
            WireExpStrOp::Strlen => {}
            WireExpStrOp::ByteLength => {}
            WireExpStrOp::CharAt { index, .. } => {
                visit(index);
            }
            WireExpStrOp::Substr { start, .. } => {
                visit(start);
            }
            WireExpStrOp::SubstrRange { start, end, .. } => {
                visit(start);
                visit(end);
            }
            WireExpStrOp::Find { needle, .. } => {
                visit(needle);
            }
            WireExpStrOp::FindNth { needle, occurrence, .. } => {
                visit(needle);
                visit(occurrence);
            }
            WireExpStrOp::Contains { needle, .. } => {
                visit(needle);
            }
            WireExpStrOp::StartsWith { prefix, .. } => {
                visit(prefix);
            }
            WireExpStrOp::EndsWith { suffix, .. } => {
                visit(suffix);
            }
            WireExpStrOp::IsUpper => {}
            WireExpStrOp::IsLower => {}
            WireExpStrOp::IsNumeric => {}
            WireExpStrOp::IsNumericTyped { .. } => {}
            WireExpStrOp::ToInteger => {}
            WireExpStrOp::ToDouble => {}
            WireExpStrOp::ToBlob => {}
            WireExpStrOp::ToString => {}
            WireExpStrOp::B64Decode => {}
            WireExpStrOp::Split => {}
            WireExpStrOp::SplitBySeparator { separator, .. } => {
                visit(separator);
            }
            WireExpStrOp::RegexCompare { pattern, .. } => {
                visit(pattern);
            }
            WireExpStrOp::RegexCompareWithFlags { pattern, .. } => {
                visit(pattern);
            }
            WireExpStrOp::Insert { index, value, .. } => {
                visit(index);
                visit(value);
            }
            WireExpStrOp::Overwrite { index, value, .. } => {
                visit(index);
                visit(value);
            }
            WireExpStrOp::Concat { values, .. } => {
                visit(values);
            }
            WireExpStrOp::Append { value, .. } => {
                visit(value);
            }
            WireExpStrOp::Prepend { value, .. } => {
                visit(value);
            }
            WireExpStrOp::Snip { start, end, .. } => {
                visit(start);
                visit(end);
            }
            WireExpStrOp::Replace { needle, replacement, .. } => {
                visit(needle);
                visit(replacement);
            }
            WireExpStrOp::ReplaceAll { needle, replacement, .. } => {
                visit(needle);
                visit(replacement);
            }
            WireExpStrOp::RegexReplace { pattern, replacement, .. } => {
                visit(pattern);
                visit(replacement);
            }
            WireExpStrOp::Upper { .. } => {}
            WireExpStrOp::Lower { .. } => {}
            WireExpStrOp::CaseFold { .. } => {}
            WireExpStrOp::NormalizeNfc { .. } => {}
            WireExpStrOp::TrimStart { .. } => {}
            WireExpStrOp::TrimEnd { .. } => {}
            WireExpStrOp::Trim { .. } => {}
            WireExpStrOp::PadStart { target_length, pad_string, .. } => {
                visit(target_length);
                visit(pad_string);
            }
            WireExpStrOp::PadEnd { target_length, pad_string, .. } => {
                visit(target_length);
                visit(pad_string);
            }
            WireExpStrOp::Repeat { count, .. } => {
                visit(count);
            }
        }
    }
}

// ===== CDT path expressions =================================================

/// A CDT path expression: one that fans out over a collection.
///
/// The rest of this module addresses **one** node — a list element, a map entry —
/// and a path expression addresses **many**, because its context contains a
/// fan-out step ([`crate::op::WireCtx::AllChildren`] or
/// [`AllChildrenWithFilter`](crate::op::WireCtx::AllChildrenWithFilter)). "The
/// price of every book" is a path expression; "the price of the first book" is not.
///
/// Needs server **8.1.1 or later**, which the daemon checks — the fan-out is a
/// server-side capability, not a client-side loop.
///
/// # Why `return_type` is an [`WireExpType`] and not a return-type bitmask
///
/// The collection families answer with "values" or "count" or "index", chosen from
/// [`crate::op::WireListReturn`]. A path expression answers with a *value of a
/// type*, and what shape that value has is decided by the [`WireExpPathOp`] —
/// `SelectMapKeys` gives keys, `SelectValues` gives values. So the only open
/// question is how to read the result, which is exactly what an `ExpType` says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireExpPath {
    /// What to do.
    pub op: WireExpPathOp,
    /// How to read the result.
    pub return_type: WireExpType,
    /// The collection to walk.
    pub bin: WireExp,
    /// The path, which must contain a fan-out step for this to be worth using.
    pub ctx: Vec<WireCtx>,
}

/// What a [`WireExpPath`] does with the nodes its path selected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireExpPathOp {
    /// Select, with an explicit flag saying what to take from each node.
    ///
    /// The general form: [`SelectValues`](Self::SelectValues) and its siblings are
    /// this with the flag filled in, and are what most code should use.
    SelectByPath {
        /// `SelectFlag` bits, OR-ed together.
        flag: i64,
    },
    /// The value of each selected node.
    SelectValues,
    /// The map key of each selected node.
    SelectMapKeys,
    /// The key and value of each selected node, as pairs.
    SelectMapEntries,
    /// The tree from the root down, keeping only the nodes the path matched.
    ///
    /// Where the others return a flat collection of what matched, this returns the
    /// original structure with everything else pruned — so the answer still tells
    /// you *where* each match was.
    SelectMatchingTree,

    /// Modify each selected node, with an explicit flag.
    ModifyByPath {
        /// `ModifyFlag` bits, OR-ed together.
        flag: i64,
        /// The expression producing each node's new value. It sees the node
        /// through the loop variable.
        modify: WireExp,
    },
    /// Modify each selected node, failing on a type mismatch.
    Modify {
        /// The expression producing each node's new value.
        modify: WireExp,
    },
    /// Modify each selected node, ignoring type mismatches.
    ModifyNoFail {
        /// The expression producing each node's new value.
        modify: WireExp,
    },
    /// Remove each selected node.
    Remove,
}

impl WireExpPathOp {
    /// Visit every expression this operation carries.
    ///
    /// Only the modify forms carry one: the flags are integers and the selects take
    /// nothing. Matched exhaustively for the reason
    /// [`WireExpListOp::args`] gives.
    fn args<'a>(&'a self, visit: &mut impl FnMut(&'a WireExp)) {
        match self {
            WireExpPathOp::SelectByPath { .. }
            | WireExpPathOp::SelectValues
            | WireExpPathOp::SelectMapKeys
            | WireExpPathOp::SelectMapEntries
            | WireExpPathOp::SelectMatchingTree
            | WireExpPathOp::Remove => {}
            WireExpPathOp::ModifyByPath { modify, .. }
            | WireExpPathOp::Modify { modify }
            | WireExpPathOp::ModifyNoFail { modify } => visit(modify),
        }
    }
}

/// Which part of the node a loop variable refers to.
///
/// Inside a context filter or a modify expression, the loop variable stands for
/// "the child currently being considered". A child of a map has a key, a value and
/// an index; a child of a list has a value and an index. This says which one is
/// meant.
///
/// A newtype over the server's integer rather than an enum, mirroring
/// `aerospike-core`'s `LoopVarPart` — the values are the server's, and a client
/// enum would be a second place they are written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WireLoopVarPart(pub i64);

impl WireLoopVarPart {
    /// The child's map key.
    pub const MAP_KEY: WireLoopVarPart = WireLoopVarPart(0);
    /// The child's value — a list element, or a map value.
    pub const VALUE: WireLoopVarPart = WireLoopVarPart(1);
    /// The child's list index.
    pub const INDEX: WireLoopVarPart = WireLoopVarPart(2);
}
