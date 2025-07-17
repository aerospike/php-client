/*
 * Copyright 2012-2023 Aerospike, Inc.
 *
 * Portions may be licensed to Aerospike, Inc. under one or more contributor
 * license agreements WHICH ARE COMPATIBLE WITH THE APACHE LICENSE, VERSION 2.0.
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not
 * use this file except in compliance with the License. You may obtain a copy of
 * the License at http:///www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations under
 * the License.
 */

#![cfg_attr(windows, feature(abi_vectorcall))]
#![allow(non_snake_case)]

mod grpc;

use grpc::proto::{self};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

use byteorder::{ByteOrder, NetworkEndian};
use ripemd160::digest::Digest;
use ripemd160::Ripemd160;
use version_compare::{Cmp, Version};

use ext_php_rs::binary::Binary;
use ext_php_rs::boxed::ZBox;
use ext_php_rs::convert::IntoZendObject;
use ext_php_rs::convert::{FromZval, IntoZval};
use ext_php_rs::error::Result;
use ext_php_rs::exception::throw_object;
use ext_php_rs::flags::DataType;
use ext_php_rs::info_table_end;
use ext_php_rs::info_table_row;
use ext_php_rs::info_table_start;
use ext_php_rs::php_class;
use ext_php_rs::prelude::*;
use ext_php_rs::types::ArrayKey;
use ext_php_rs::types::ZendHashTable;
use ext_php_rs::types::ZendObject;
use ext_php_rs::types::Zval;
use ext_php_rs::zend::ModuleEntry;

use byteorder::{LittleEndian, ReadBytesExt};
use rand::prelude::*;

use lazy_static::lazy_static;
use log::trace;

lazy_static! {
    static ref CLIENTS: Mutex<HashMap<String, Arc<Mutex<grpc::BlockingClient>>>> =
        Mutex::new(HashMap::new());
}

pub type AsResult<T = ()> = std::result::Result<T, AerospikeException>;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PARTITIONS: u16 = 4096;
const CITRUSLEAF_EPOCH: u64 = 1262304000;

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ExpressionType (ExpType)
//
////////////////////////////////////////////////////////////////////////////////////////////

/// ExpType defines the expression's data type.
#[php_class(name = "Aerospike\\ExpType")]
pub struct ExpType {
    _as: proto::ExpType,
}

impl FromZval<'_> for ExpType {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &ExpType = zval.extract()?;

        Some(ExpType { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl ExpType {
    /// ExpTypeNIL is NIL Expression Type
    pub fn Nil() -> Self {
        ExpType {
            _as: proto::ExpType::Nil,
        }
    }

    /// ExpTypeBOOL is BOOLEAN Expression Type
    pub fn Bool() -> Self {
        ExpType {
            _as: proto::ExpType::Bool,
        }
    }

    /// ExpTypeINT is INTEGER Expression Type
    pub fn Int() -> Self {
        ExpType {
            _as: proto::ExpType::Int,
        }
    }

    /// ExpTypeSTRING is STRING Expression Type
    pub fn String() -> Self {
        ExpType {
            _as: proto::ExpType::String,
        }
    }

    /// ExpTypeLIST is LIST Expression Type
    pub fn List() -> Self {
        ExpType {
            _as: proto::ExpType::List,
        }
    }

    /// ExpTypeMAP is MAP Expression Type
    pub fn Map() -> Self {
        ExpType {
            _as: proto::ExpType::Map,
        }
    }

    /// ExpTypeBLOB is BLOB Expression Type
    pub fn Blob() -> Self {
        ExpType {
            _as: proto::ExpType::Blob,
        }
    }

    /// ExpTypeFLOAT is FLOAT Expression Type
    pub fn Float() -> Self {
        ExpType {
            _as: proto::ExpType::Float,
        }
    }

    /// ExpTypeGEO is GEO String Expression Type
    pub fn Geo() -> Self {
        ExpType {
            _as: proto::ExpType::Geo,
        }
    }

    /// ExpTypeHLL is HLL Expression Type
    pub fn Hll() -> Self {
        ExpType {
            _as: proto::ExpType::Hll,
        }
    }
}

impl From<ExpType> for i32 {
    fn from(input: ExpType) -> Self {
        match &input._as {
            proto::ExpType::Nil => 0,
            proto::ExpType::Bool => 1,
            proto::ExpType::Int => 2,
            proto::ExpType::String => 3,
            proto::ExpType::List => 4,
            proto::ExpType::Map => 5,
            proto::ExpType::Blob => 6,
            proto::ExpType::Float => 7,
            proto::ExpType::Geo => 8,
            proto::ExpType::Hll => 9,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Filter Expression
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Filter expression, which can be applied to most commands, to control which records are
/// affected by the command.
#[php_class(name = "Aerospike\\Expression")]
pub struct Expression {
    _as: proto::Expression,
}

impl FromZval<'_> for Expression {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Expression = zval.extract()?;

        Some(Expression { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Expression {
    pub fn new(
        cmd: Option<i32>,
        val: Option<PHPValue>,
        bin: Option<&Expression>,
        flags: Option<i64>,
        module: Option<ExpType>,
        exps: Vec<&Expression>,
    ) -> Self {
        Expression {
            _as: proto::Expression {
                cmd: cmd.map(|v| v.into()),
                val: val.map(|v| v.into()),
                bin: bin.map(|v| Box::new(v._as.clone())),
                flags: flags,
                module: module.map(|v| v.into()),
                exps: exps.iter().map(|e| e._as.clone()).collect(),
            },
        }
    }

    /// Create a record key expression of specified type.
    pub fn key(exp_type: ExpType) -> Self {
        let exp_type: i32 = exp_type.into();
        Expression::new(
            Some(proto::ExpOp::Key.into()),
            Some(PHPValue::Int(exp_type as i64).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create function that returns if the primary key is stored in the record meta data
    /// as a boolean expression. This would occur when `send_key` is true on record write.
    pub fn key_exists() -> Self {
        Expression::new(
            Some(proto::ExpOp::KeyExists.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create 64 bit int bin expression.
    pub fn int_bin(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::Bin.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            Some(ExpType::Int()),
            vec![],
        )
    }

    /// Create string bin expression.
    pub fn string_bin(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::Bin.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            Some(ExpType::String()),
            vec![],
        )
    }

    /// Create blob bin expression.
    pub fn blob_bin(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::Bin.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            Some(ExpType::Blob()),
            vec![],
        )
    }

    /// Create 64 bit float bin expression.
    pub fn float_bin(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::Bin.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            Some(ExpType::Float()),
            vec![],
        )
    }

    /// Create geo bin expression.
    pub fn geo_bin(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::Bin.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            Some(ExpType::Geo()),
            vec![],
        )
    }

    /// Create list bin expression.
    pub fn list_bin(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::Bin.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            Some(ExpType::List()),
            vec![],
        )
    }

    /// Create map bin expression.
    pub fn map_bin(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::Bin.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            Some(ExpType::Map()),
            vec![],
        )
    }

    /// Create a HLL bin expression
    pub fn hll_bin(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::Bin.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            Some(ExpType::Hll()),
            vec![],
        )
    }

    /// Create function that returns if bin of specified name exists.
    pub fn bin_exists(name: String) -> Self {
        Expression::ne(
            &Expression::bin_type(name),
            &Expression::int_val(ParticleType::Null().into()),
        )
    }

    /// ExpBinType creates a function that returns bin's integer particle type. Valid values are:
    ///
    ///	NULL    = 0
    ///	INTEGER = 1
    ///	FLOAT   = 2
    ///	STRING  = 3
    ///	BLOB    = 4
    ///	DIGEST  = 6
    ///	BOOL    = 17
    ///	HLL     = 18
    ///	MAP     = 19
    ///	LIST    = 20
    ///	LDT     = 21
    ///	GEOJSON = 23
    pub fn bin_type(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::BinType.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create function that returns record set name string.
    pub fn set_name() -> Self {
        Expression::new(
            Some(proto::ExpOp::SetName.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create function that returns record size on disk.
    /// If server storage-engine is memory, then zero is returned.
    ///
    /// This expression should only be used for server versions less than 7.0. Use
    /// record_size for server version 7.0+.
    pub fn device_size() -> Self {
        Expression::new(
            Some(proto::ExpOp::DeviceSize.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create expression that returns record size in memory. If server storage-engine is
    /// not memory nor data-in-memory, then zero is returned. This expression usually evaluates
    /// quickly because record meta data is cached in memory.
    ///
    /// Requires server version between 5.3 inclusive and 7.0 exclusive.
    /// Use record_size for server version 7.0+.
    pub fn memory_size() -> Self {
        Expression::new(
            Some(proto::ExpOp::MemorySize.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create function that returns record last update time expressed as 64 bit integer
    /// nanoseconds since 1970-01-01 epoch.
    pub fn last_update() -> Self {
        Expression::new(
            Some(proto::ExpOp::LastUpdate.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create expression that returns milliseconds since the record was last updated.
    /// This expression usually evaluates quickly because record meta data is cached in memory.
    pub fn since_update() -> Self {
        Expression::new(
            Some(proto::ExpOp::SinceUpdate.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create function that returns record expiration time expressed as 64 bit integer
    /// nanoseconds since 1970-01-01 epoch.
    pub fn void_time() -> Self {
        Expression::new(
            Some(proto::ExpOp::VoidTime.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create function that returns record expiration time (time to live) in integer seconds.
    pub fn ttl() -> Self {
        Expression::new(
            Some(proto::ExpOp::Ttl.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create expression that returns if record has been deleted and is still in tombstone state.
    /// This expression usually evaluates quickly because record meta data is cached in memory.
    pub fn is_tombstone() -> Self {
        Expression::new(
            Some(proto::ExpOp::IsTombstone.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create function that returns record digest modulo as integer.
    pub fn digest_modulo(modulo: i64) -> Self {
        Expression::new(
            Some(proto::ExpOp::DigestModulo.into()),
            Some(PHPValue::Int(modulo).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create function like regular expression string operation.
    pub fn regex_compare(regex: String, flags: i64, bin: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Regex.into()),
            Some(PHPValue::String(regex).into()),
            Some(bin),
            Some(flags),
            None,
            vec![],
        )
    }

    /// Create compare geospatial operation.
    pub fn geo_compare(left: &Expression, right: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Geo.into()),
            None,
            None,
            None,
            None,
            vec![left, right],
        )
    }

    /// Creates 64 bit integer value
    pub fn int_val(val: i64) -> Self {
        Expression::new(
            None,
            Some(PHPValue::Int(val).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Creates a Boolean value
    pub fn bool_val(val: bool) -> Self {
        Expression::new(
            None,
            Some(PHPValue::Bool(val).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Creates String bin value
    pub fn string_val(val: String) -> Self {
        Expression::new(
            None,
            Some(PHPValue::String(val).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Creates 64 bit float bin value
    pub fn float_val(val: f64) -> Self {
        Expression::new(
            None,
            Some(PHPValue::Float(ordered_float::OrderedFloat(val)).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Creates Blob bin value
    pub fn blob_val(val: Vec<u8>) -> Self {
        Expression::new(
            None,
            Some(PHPValue::Blob(val).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create List bin PHPValue
    /// Not Supported in pre-alpha release
    pub fn list_val(val: Vec<PHPValue>) -> Self {
        Expression::new(
            None,
            Some(PHPValue::List(val).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create Map bin PHPValue
    /// Value must be a map
    pub fn map_val(val: PHPValue) -> Option<Self> {
        if !assert_map(&val) {
            return None;
        }

        Some(Expression::new(
            None,
            Some(val.clone()),
            None,
            None,
            None,
            vec![],
        ))
    }

    /// Create geospatial json string value.
    pub fn geo_val(val: String) -> Self {
        Expression::new(
            None,
            Some(PHPValue::GeoJSON(val).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create a Nil PHPValue
    pub fn nil() -> Self {
        Expression::new(None, Some(PHPValue::Nil.into()), None, None, None, vec![])
    }

    /// Create a Infinity PHPValue
    pub fn infinity() -> Self {
        Expression::new(
            None,
            Some(PHPValue::Infinity.into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create a WildCard PHPValue
    pub fn wildcard() -> Self {
        Expression::new(
            None,
            Some(PHPValue::Wildcard.into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create "not" operator expression.
    pub fn not(exp: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Not.into()),
            None,
            None,
            None,
            None,
            vec![exp],
        )
    }

    /// Create "and" (&&) operator that applies to a variable number of expressions.
    /// /// (a > 5 || a == 0) && b < 3
    pub fn and(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::And.into()), None, None, None, None, exps)
    }

    /// Create "or" (||) operator that applies to a variable number of expressions.
    pub fn or(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::Or.into()), None, None, None, None, exps)
    }

    /// Create "xor" (^) operator that applies to a variable number of expressions.
    pub fn xor(exps: Vec<&Expression>) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntXor.into()),
            None,
            None,
            None,
            None,
            exps,
        )
    }

    /// Create equal (==) expression.
    pub fn eq(left: &Expression, right: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Eq.into()),
            None,
            None,
            None,
            None,
            vec![left, right],
        )
    }

    /// Create not equal (!=) expression
    pub fn ne(left: &Expression, right: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Ne.into()),
            None,
            None,
            None,
            None,
            vec![left, right],
        )
    }

    /// Create greater than (>) operation.
    pub fn gt(left: &Expression, right: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Gt.into()),
            None,
            None,
            None,
            None,
            vec![left, right],
        )
    }

    /// Create greater than or equal (>=) operation.
    pub fn ge(left: &Expression, right: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Ge.into()),
            None,
            None,
            None,
            None,
            vec![left, right],
        )
    }

    /// Create less than (<) operation.
    pub fn lt(left: &Expression, right: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Lt.into()),
            None,
            None,
            None,
            None,
            vec![left, right],
        )
    }

    /// Create less than or equals (<=) operation.
    pub fn le(left: &Expression, right: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Le.into()),
            None,
            None,
            None,
            None,
            vec![left, right],
        )
    }

    /// Create "add" (+) operator that applies to a variable number of expressions.
    /// Return sum of all `FilterExpressions` given. All arguments must resolve to the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn num_add(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::Add.into()), None, None, None, None, exps)
    }

    /// Create "subtract" (-) operator that applies to a variable number of expressions.
    /// If only one `FilterExpressions` is provided, return the negation of that argument.
    /// Otherwise, return the sum of the 2nd to Nth `FilterExpressions` subtracted from the 1st
    /// `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn num_sub(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::Sub.into()), None, None, None, None, exps)
    }

    /// Create "multiply" (*) operator that applies to a variable number of expressions.
    /// Return the product of all `FilterExpressions`. If only one `FilterExpressions` is supplied, return
    /// that `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn num_mul(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::Mul.into()), None, None, None, None, exps)
    }

    /// Create "divide" (/) operator that applies to a variable number of expressions.
    /// If there is only one `FilterExpressions`, returns the reciprocal for that `FilterExpressions`.
    /// Otherwise, return the first `FilterExpressions` divided by the product of the rest.
    /// All `FilterExpressions` must resolve to the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn num_div(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::Div.into()), None, None, None, None, exps)
    }

    /// Create "power" operator that raises a "base" to the "exponent" power.
    /// All arguments must resolve to floats.
    /// Requires server version 5.6.0+.
    pub fn num_pow(base: &Expression, exponent: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Pow.into()),
            None,
            None,
            None,
            None,
            vec![base, exponent],
        )
    }

    /// Create "log" operator for logarithm of "num" with base "base".
    /// All arguments must resolve to floats.
    /// Requires server version 5.6.0+.
    pub fn num_log(num: &Expression, base: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Log.into()),
            None,
            None,
            None,
            None,
            vec![num, base],
        )
    }

    /// Create "modulo" (%) operator that determines the remainder of "numerator"
    /// divided by "denominator". All arguments must resolve to integers.
    /// Requires server version 5.6.0+.
    pub fn num_mod(numerator: &Expression, denominator: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Mod.into()),
            None,
            None,
            None,
            None,
            vec![numerator, denominator],
        )
    }

    /// Create operator that returns absolute value of a number.
    /// All arguments must resolve to integer or float.
    /// Requires server version 5.6.0+.
    pub fn num_abs(value: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Abs.into()),
            None,
            None,
            None,
            None,
            vec![value],
        )
    }

    /// Create expression that rounds a floating point number down to the closest integer value.
    /// The return type is float.
    /// Requires server version 5.6.0+.
    pub fn num_floor(num: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Floor.into()),
            None,
            None,
            None,
            None,
            vec![num],
        )
    }

    /// Create expression that rounds a floating point number up to the closest integer value.
    /// The return type is float.
    /// Requires server version 5.6.0+.
    pub fn num_ceil(num: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::Ceil.into()),
            None,
            None,
            None,
            None,
            vec![num],
        )
    }

    /// Create expression that converts an integer to a float.
    /// Requires server version 5.6.0+.
    pub fn to_int(num: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::ToInt.into()),
            None,
            None,
            None,
            None,
            vec![num],
        )
    }

    /// Create expression that converts a float to an integer.
    /// Requires server version 5.6.0+.
    pub fn to_float(num: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::ToFloat.into()),
            None,
            None,
            None,
            None,
            vec![num],
        )
    }

    /// Create integer "and" (&) operator that is applied to two or more integers.
    /// All arguments must resolve to integers.
    /// Requires server version 5.6.0+.
    pub fn int_and(exps: Vec<&Expression>) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntAnd.into()),
            None,
            None,
            None,
            None,
            exps,
        )
    }

    /// Create integer "or" (|) operator that is applied to two or more integers.
    /// All arguments must resolve to integers.
    /// Requires server version 5.6.0+.
    pub fn int_or(exps: Vec<&Expression>) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntOr.into()),
            None,
            None,
            None,
            None,
            exps,
        )
    }

    /// Create integer "xor" (^) operator that is applied to two or more integers.
    /// All arguments must resolve to integers.
    /// Requires server version 5.6.0+.
    pub fn int_xor(exps: Vec<&Expression>) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntXor.into()),
            None,
            None,
            None,
            None,
            exps,
        )
    }

    /// Create integer "not" (~) operator.
    /// Requires server version 5.6.0+.
    pub fn int_not(exp: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntNot.into()),
            None,
            None,
            None,
            None,
            vec![exp],
        )
    }

    /// Create integer "left shift" (<<) operator.
    /// Requires server version 5.6.0+.
    pub fn int_lshift(value: &Expression, shift: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntLShift.into()),
            None,
            None,
            None,
            None,
            vec![value, shift],
        )
    }

    /// Create integer "logical right shift" (>>>) operator.
    /// Requires server version 5.6.0+.
    pub fn int_rshift(value: &Expression, shift: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntRShift.into()),
            None,
            None,
            None,
            None,
            vec![value, shift],
        )
    }

    /// Create integer "arithmetic right shift" (>>) operator.
    /// The sign bit is preserved and not shifted.
    /// Requires server version 5.6.0+.
    pub fn int_arshift(value: &Expression, shift: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntArShift.into()),
            None,
            None,
            None,
            None,
            vec![value, shift],
        )
    }

    /// Create expression that returns count of integer bits that are set to 1.
    /// Requires server version 5.6.0+
    pub fn int_count(exp: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntCount.into()),
            None,
            None,
            None,
            None,
            vec![exp],
        )
    }

    /// Create expression that scans integer bits from left (most significant bit) to
    /// right (least significant bit), looking for a search bit value. When the
    /// search value is found, the index of that bit (where the most significant bit is
    /// index 0) is returned. If "search" is true, the scan will search for the bit
    /// value 1. If "search" is false it will search for bit value 0.
    /// Requires server version 5.6.0+.
    pub fn int_lscan(value: &Expression, search: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntLScan.into()),
            None,
            None,
            None,
            None,
            vec![value, search],
        )
    }

    /// Create expression that scans integer bits from right (least significant bit) to
    /// left (most significant bit), looking for a search bit value. When the
    /// search value is found, the index of that bit (where the most significant bit is
    /// index 0) is returned. If "search" is true, the scan will search for the bit
    /// value 1. If "search" is false it will search for bit value 0.
    /// Requires server version 5.6.0+.
    pub fn int_rscan(value: &Expression, search: &Expression) -> Self {
        Expression::new(
            Some(proto::ExpOp::IntRScan.into()),
            None,
            None,
            None,
            None,
            vec![value, search],
        )
    }

    /// Create expression that returns the minimum value in a variable number of expressions.
    /// All arguments must be the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn min(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::Min.into()), None, None, None, None, exps)
    }

    /// Create expression that returns the maximum value in a variable number of expressions.
    /// All arguments must be the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn max(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::Max.into()), None, None, None, None, exps)
    }

    ///--------------------------------------------------
    /// Variables
    ///--------------------------------------------------

    /// Conditionally select an expression from a variable number of expression pairs
    /// followed by default expression action.
    /// Requires server version 5.6.0+.
    /// ```
    /// /// Args Format: bool exp1, action exp1, bool exp2, action exp2, ..., action-default
    /// /// Apply operator based on type.
    pub fn cond(exps: Vec<&Expression>) -> Self {
        Expression::new(
            Some(proto::ExpOp::Cond.into()),
            None,
            None,
            None,
            None,
            exps,
        )
    }

    /// Define variables and expressions in scope.
    /// Requires server version 5.6.0+.
    /// ```
    /// /// 5 < a < 10
    pub fn exp_let(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::Let.into()), None, None, None, None, exps)
    }

    /// Assign variable to an expression that can be accessed later.
    /// Requires server version 5.6.0+.
    /// ```
    /// /// 5 < a < 10
    pub fn def(name: String, value: &Expression) -> Self {
        Expression::new(
            None,
            Some(PHPValue::String(name).into()),
            None,
            None,
            None,
            vec![value],
        )
    }

    /// Retrieve expression value from a variable.
    /// Requires server version 5.6.0+.
    pub fn var(name: String) -> Self {
        Expression::new(
            Some(proto::ExpOp::Var.into()),
            Some(PHPValue::String(name).into()),
            None,
            None,
            None,
            vec![],
        )
    }

    /// Create unknown value. Used to intentionally fail an expression.
    /// The failure can be ignored with `ExpWriteFlags` `EVAL_NO_FAIL`
    /// or `ExpReadFlags` `EVAL_NO_FAIL`.
    /// Requires server version 5.6.0+.
    pub fn unknown() -> Self {
        Expression::new(
            Some(proto::ExpOp::Unknown.into()),
            None,
            None,
            None,
            None,
            vec![],
        )
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ReadModeAP
//
////////////////////////////////////////////////////////////////////////////////////////////

/// ReadModeAP is the read policy in AP (availability) mode namespaces.
/// It indicates how duplicates should be consulted in a read operation.
/// Only makes a difference during migrations and only applicable in AP mode.
#[php_class(name = "Aerospike\\ReadModeAP")]
pub struct ReadModeAP {
    _as: proto::ReadModeAp,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ReadModeAP {
    /// ReadModeAPOne indicates that a single node should be involved in the read operation.
    pub fn One() -> Self {
        ReadModeAP {
            _as: proto::ReadModeAp::One,
        }
    }

    /// ReadModeAPAll indicates that all duplicates should be consulted in
    /// the read operation.
    pub fn All() -> Self {
        ReadModeAP {
            _as: proto::ReadModeAp::All,
        }
    }
}

impl From<&ReadModeAP> for i32 {
    fn from(v: &ReadModeAP) -> i32 {
        match v._as {
            proto::ReadModeAp::One => 0,
            proto::ReadModeAp::All => 1,
        }
    }
}

impl From<i32> for ReadModeAP {
    fn from(v: i32) -> ReadModeAP {
        match v {
            0 => ReadModeAP {
                _as: proto::ReadModeAp::One,
            },
            1 => ReadModeAP {
                _as: proto::ReadModeAp::All,
            },
            _ => unreachable!(),
        }
    }
}

impl FromZval<'_> for ReadModeAP {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &ReadModeAP = zval.extract()?;

        Some(ReadModeAP { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ReadModeSC
//
////////////////////////////////////////////////////////////////////////////////////////////

/// ReadModeSC is the read policy in SC (strong consistency) mode namespaces.
/// Determines SC read consistency options.
#[php_class(name = "Aerospike\\ReadModeSC")]
pub struct ReadModeSC {
    _as: proto::ReadModeSc,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ReadModeSC {
    /// ReadModeSCSession ensures this client will only see an increasing sequence of record versions.
    /// Server only reads from master.  This is the default.
    pub fn Session() -> Self {
        ReadModeSC {
            _as: proto::ReadModeSc::Session,
        }
    }

    /// ReadModeSCLinearize ensures ALL clients will only see an increasing sequence of record versions.
    /// Server only reads from master.
    pub fn Linearize() -> Self {
        ReadModeSC {
            _as: proto::ReadModeSc::Linearize,
        }
    }

    /// ReadModeSCAllowReplica indicates that the server may read from master or any full (non-migrating) replica.
    /// Increasing sequence of record versions is not guaranteed.
    pub fn AllowReplica() -> Self {
        ReadModeSC {
            _as: proto::ReadModeSc::AllowReplica,
        }
    }

    /// ReadModeSCAllowUnavailable indicates that the server may read from master or any full (non-migrating) replica or from unavailable
    /// partitions.  Increasing sequence of record versions is not guaranteed.
    pub fn AllowUnavailable() -> Self {
        ReadModeSC {
            _as: proto::ReadModeSc::AllowUnavailable,
        }
    }
}

impl From<&ReadModeSC> for i32 {
    fn from(v: &ReadModeSC) -> i32 {
        match &v._as {
            proto::ReadModeSc::Session => 0,
            proto::ReadModeSc::Linearize => 1,
            proto::ReadModeSc::AllowReplica => 2,
            proto::ReadModeSc::AllowUnavailable => 3,
        }
    }
}

impl From<i32> for ReadModeSC {
    fn from(v: i32) -> ReadModeSC {
        match v {
            0 => ReadModeSC {
                _as: proto::ReadModeSc::Session,
            },
            1 => ReadModeSC {
                _as: proto::ReadModeSc::Linearize,
            },
            2 => ReadModeSC {
                _as: proto::ReadModeSc::AllowReplica,
            },
            3 => ReadModeSC {
                _as: proto::ReadModeSc::AllowUnavailable,
            },
            _ => unreachable!(),
        }
    }
}

impl FromZval<'_> for ReadModeSC {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &ReadModeSC = zval.extract()?;

        Some(ReadModeSC { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  RecordExistsAction
//
////////////////////////////////////////////////////////////////////////////////////////////

/// RecordExistsAction determines how to handle writes when
/// the record already exists.
#[php_class(name = "Aerospike\\RecordExistsAction")]
pub struct RecordExistsAction {
    _as: proto::RecordExistsAction,
}

impl FromZval<'_> for RecordExistsAction {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &RecordExistsAction = zval.extract()?;

        Some(RecordExistsAction { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl RecordExistsAction {
    /// Update means: Create or update record.
    /// Merge write command bins with existing bins.
    pub fn Update() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::Update,
        }
    }

    /// UpdateOnly means: Update record only. Fail if record does not exist.
    /// Merge write command bins with existing bins.
    pub fn Update_Only() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::UpdateOnly,
        }
    }

    /// Replace means: Create or replace record.
    /// Delete existing bins not referenced by write command bins.
    /// Supported by Aerospike 2 server versions >= 2.7.5 and
    /// Aerospike 3 server versions >= 3.1.6.
    pub fn Replace() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::Replace,
        }
    }

    /// ReplaceOnly means: Replace record only. Fail if record does not exist.
    /// Delete existing bins not referenced by write command bins.
    /// Supported by Aerospike 2 server versions >= 2.7.5 and
    /// Aerospike 3 server versions >= 3.1.6.
    pub fn Replace_Only() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::ReplaceOnly,
        }
    }

    /// CreateOnly means: Create only. Fail if record exists.
    pub fn Create_Only() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::CreateOnly,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  QueryDuration
//
////////////////////////////////////////////////////////////////////////////////////////////

/// QueryDuration represents the expected duration for a query operation in the Aerospike database.
#[php_class(name = "Aerospike\\QueryDuration")]
pub struct QueryDuration {
    _as: proto::QueryDuration,
}

impl FromZval<'_> for QueryDuration {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &QueryDuration = zval.extract()?;

        Some(QueryDuration { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl QueryDuration {
    /// LONG specifies that the query is expected to return more than 100 records per node. The server optimizes for a large record set in
    /// the following ways:
    ///
    /// Allow query to be run in multiple threads using the server's query threading configuration.
    /// Do not relax read consistency for AP namespaces.
    /// Add the query to the server's query monitor.
    /// Do not add the overall latency to the server's latency histogram.
    /// Do not allow server timeouts.    
    pub fn Long() -> Self {
        QueryDuration {
            _as: proto::QueryDuration::Long,
        }
    }

    /// Short specifies that the query is expected to return less than 100 records per node. The server optimizes for a small record set in
    /// the following ways:
    /// Always run the query in one thread and ignore the server's query threading configuration.
    /// Allow query to be inlined directly on the server's service thread.
    /// Relax read consistency for AP namespaces.
    /// Do not add the query to the server's query monitor.
    /// Add the overall latency to the server's latency histogram.
    /// Allow server timeouts. The default server timeout for a short query is 1 second.
    pub fn Short() -> Self {
        QueryDuration {
            _as: proto::QueryDuration::Short,
        }
    }

    /// LongRelaxAP will treat query as a LONG query, but relax read consistency for AP namespaces.
    /// This value is treated exactly like LONG for server versions < 7.1.
    pub fn LongRelaxAP() -> Self {
        QueryDuration {
            _as: proto::QueryDuration::LongRelaxAp,
        }
    }
}

impl From<&proto::QueryDuration> for QueryDuration {
    fn from(input: &proto::QueryDuration) -> Self {
        QueryDuration { _as: input.clone() }
    }
}

impl From<i32> for QueryDuration {
    fn from(input: i32) -> Self {
        match input {
            0 => QueryDuration::Long(),
            1 => QueryDuration::Short(),
            2 => QueryDuration::LongRelaxAP(),
            _ => unreachable!(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CommitLevel
//
////////////////////////////////////////////////////////////////////////////////////////////

/// CommitLevel indicates the desired consistency guarantee when committing a transaction on the server.
#[php_class(name = "Aerospike\\CommitLevel")]
pub struct CommitLevel {
    _as: proto::CommitLevel,
}

impl FromZval<'_> for CommitLevel {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CommitLevel = zval.extract()?;

        Some(CommitLevel { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl CommitLevel {
    /// CommitAll indicates the server should wait until successfully committing master and all
    /// replicas.
    pub fn Commit_All() -> Self {
        CommitLevel {
            _as: proto::CommitLevel::CommitAll,
        }
    }

    /// CommitMaster indicates the server should wait until successfully committing master only.
    pub fn Commit_Master() -> Self {
        CommitLevel {
            _as: proto::CommitLevel::CommitMaster,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ConsistencyLevel
//
////////////////////////////////////////////////////////////////////////////////////////////

/// `ConsistencyLevel` indicates how replicas should be consulted in a read
/// operation to provide the desired consistency guarantee.
#[derive(Debug, Clone, Copy)]
pub enum _ConsistencyLevel {
    ConsistencyOne,
    ConsistencyAll,
}

#[php_class(name = "Aerospike\\ConsistencyLevel")]
pub struct ConsistencyLevel {
    v: _ConsistencyLevel,
}

impl FromZval<'_> for ConsistencyLevel {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &ConsistencyLevel = zval.extract()?;

        Some(ConsistencyLevel { v: f.v })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl ConsistencyLevel {
    /// ConsistencyOne indicates only a single replica should be consulted in
    /// the read operation.
    pub fn Consistency_One() -> Self {
        ConsistencyLevel {
            v: _ConsistencyLevel::ConsistencyOne,
        }
    }

    /// ConsistencyAll indicates that all replicas should be consulted in
    /// the read operation.
    pub fn Consistency_All() -> Self {
        ConsistencyLevel {
            v: _ConsistencyLevel::ConsistencyAll,
        }
    }
}

impl From<&ConsistencyLevel> for proto::ConsistencyLevel {
    fn from(input: &ConsistencyLevel) -> Self {
        match &input.v {
            _ConsistencyLevel::ConsistencyOne => proto::ConsistencyLevel::ConsistencyOne,
            _ConsistencyLevel::ConsistencyAll => proto::ConsistencyLevel::ConsistencyAll,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
///
///  GenerationPolicy
///
////////////////////////////////////////////////////////////////////////////////////////////

/// `GenerationPolicy` determines how to handle record writes based on record generation.
#[php_class(name = "Aerospike\\GenerationPolicy")]
pub struct GenerationPolicy {
    _as: proto::GenerationPolicy,
}

impl FromZval<'_> for GenerationPolicy {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &GenerationPolicy = zval.extract()?;

        Some(GenerationPolicy { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl GenerationPolicy {
    /// None means: Do not use record generation to restrict writes.
    pub fn None() -> Self {
        GenerationPolicy {
            _as: proto::GenerationPolicy::None,
        }
    }

    /// ExpectGenEqual means: Update/delete record if expected generation is equal to server
    /// generation. Otherwise, fail.
    pub fn Expect_Gen_Equal() -> Self {
        GenerationPolicy {
            _as: proto::GenerationPolicy::ExpectGenEqual,
        }
    }

    /// ExpectGenGreater means: Update/delete record if expected generation greater than the server
    /// generation. Otherwise, fail. This is useful for restore after backup.
    pub fn Expect_Gen_Greater() -> Self {
        GenerationPolicy {
            _as: proto::GenerationPolicy::ExpectGenGt,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Expiration
//
////////////////////////////////////////////////////////////////////////////////////////////

const NAMESPACE_DEFAULT: u32 = 0x0000_0000;
const NEVER_EXPIRE: u32 = 0xFFFF_FFFF; // -1 as i32
const DONT_UPDATE: u32 = 0xFFFF_FFFE;

/// Record expiration, also known as time-to-live (TTL).
#[derive(Debug, Clone, Copy)]
pub enum _Expiration {
    Seconds(u32),
    NamespaceDefault,
    Never,
    DontUpdate,
}

#[php_class(name = "Aerospike\\Expiration")]
pub struct Expiration {
    _as: _Expiration,
}

impl FromZval<'_> for Expiration {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Expiration = zval.extract()?;

        Some(Expiration { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Expiration {
    /// Set the record to expire X seconds from now
    pub fn Seconds(seconds: u32) -> Self {
        Expiration {
            _as: _Expiration::Seconds(seconds),
        }
    }

    /// Set the record's expiry time using the default time-to-live (TTL) value for the namespace
    pub fn Namespace_Default() -> Self {
        Expiration {
            _as: _Expiration::NamespaceDefault,
        }
    }

    /// Set the record to never expire. Requires Aerospike 2 server version 2.7.2 or later or
    /// Aerospike 3 server version 3.1.4 or later. Do not use with older servers.
    pub fn Never() -> Self {
        Expiration {
            _as: _Expiration::Never,
        }
    }

    /// Do not change the record's expiry time when updating the record; requires Aerospike server
    /// version 3.10.1 or later.
    pub fn Dont_Update() -> Self {
        Expiration {
            _as: _Expiration::DontUpdate,
        }
    }
}

impl From<&Expiration> for u32 {
    fn from(exp: &Expiration) -> u32 {
        match &exp._as {
            _Expiration::Seconds(secs) => *secs,
            _Expiration::NamespaceDefault => NAMESPACE_DEFAULT,
            _Expiration::Never => NEVER_EXPIRE,
            _Expiration::DontUpdate => DONT_UPDATE,
        }
    }
}

impl From<u32> for Expiration {
    fn from(exp: u32) -> Expiration {
        match exp {
            NAMESPACE_DEFAULT => Expiration::Namespace_Default(),
            NEVER_EXPIRE => Expiration::Never(),
            DONT_UPDATE => Expiration::Dont_Update(),
            secs => Expiration::Seconds(secs),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Concurrency
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Specifies whether a command, that needs to be executed on multiple cluster nodes, should be
/// executed sequentially, one node at a time, or in parallel on multiple nodes using the client's
/// thread pool.
#[derive(Debug, Clone, Copy)]
pub enum _Concurrency {
    Sequential,
    Parallel,
    MaxThreads(u32),
}

#[php_class(name = "Aerospike\\Concurrency")]
pub struct Concurrency {
    v: _Concurrency,
}

impl FromZval<'_> for Concurrency {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Concurrency = zval.extract()?;

        Some(Concurrency { v: f.v })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Concurrency {
    /// Issue commands sequentially. This mode has a performance advantage for small to
    /// medium sized batch sizes because requests can be issued in the main transaction thread.
    /// This is the default.
    pub fn Sequential() -> Self {
        Concurrency {
            v: _Concurrency::Sequential,
        }
    }

    /// Issue all commands in parallel threads. This mode has a performance advantage for
    /// extremely large batch sizes because each node can process the request immediately. The
    /// downside is extra threads will need to be created (or takedn from a thread pool).
    pub fn Parallel() -> Self {
        Concurrency {
            v: _Concurrency::Parallel,
        }
    }

    /// Issue up to N commands in parallel threads. When a request completes, a new request
    /// will be issued until all threads are complete. This mode prevents too many parallel threads
    /// being created for large cluster implementations. The downside is extra threads will still
    /// need to be created (or taken from a thread pool).
    ///
    /// E.g. if there are 16 nodes/namespace combinations requested and concurrency is set to
    /// `MaxThreads(8)`, then batch requests will be made for 8 node/namespace combinations in
    /// parallel threads. When a request completes, a new request will be issued until all 16
    /// requests are complete.
    pub fn Max_Threads(threads: u32) -> Self {
        Concurrency {
            v: _Concurrency::MaxThreads(threads),
        }
    }
}

impl From<&Concurrency> for u32 {
    fn from(input: &Concurrency) -> Self {
        match &input.v {
            _Concurrency::Sequential => 1,
            _Concurrency::Parallel => 0,
            _Concurrency::MaxThreads(threads) => *threads,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ListOrderType
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Specifies whether a command, that needs to be executed on multiple cluster nodes, should be
/// executed sequentially, one node at a time, or in parallel on multiple nodes using the client's
/// thread pool.
#[php_class(name = "Aerospike\\ListOrderType")]
#[derive(PartialEq)]
pub struct ListOrderType {
    _as: proto::ListOrderType,
}

impl FromZval<'_> for ListOrderType {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &ListOrderType = zval.extract()?;

        Some(ListOrderType { _as: f._as })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl ListOrderType {
    fn flag(&self) -> i32 {
        match self._as {
            proto::ListOrderType::Unordered => 0,
            proto::ListOrderType::Ordered => 1,
        }
    }

    /// ListOrderOrdered signifies that list is Ordered.
    pub fn Ordered() -> Self {
        ListOrderType {
            _as: proto::ListOrderType::Ordered,
        }
    }

    /// ListOrderUnordered signifies that list is not ordered. This is the default.
    pub fn Unordered() -> Self {
        ListOrderType {
            _as: proto::ListOrderType::Unordered,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  MapOrderType
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Specifies whether a command, that needs to be executed on multiple cluster nodes, should be
/// executed sequentially, one node at a time, or in parallel on multiple nodes using the client's
/// thread pool.
#[php_class(name = "Aerospike\\MapOrderType")]
pub struct MapOrderType {
    _as: proto::MapOrderType,
}

impl FromZval<'_> for MapOrderType {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &MapOrderType = zval.extract()?;

        Some(MapOrderType { _as: f._as })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl MapOrderType {
    fn attr(&self) -> i32 {
        match self._as {
            proto::MapOrderType::Unordered => 0,
            proto::MapOrderType::KeyOrdered => 1,
            proto::MapOrderType::KeyValueOrdered => 3,
        }
    }

    fn flag(&self) -> i32 {
        match self._as {
            proto::MapOrderType::Unordered => 0x40,
            proto::MapOrderType::KeyOrdered => 0x80,
            proto::MapOrderType::KeyValueOrdered => 0xc0,
        }
    }

    /// Map is not ordered. This is the default.
    pub fn Unordered() -> Self {
        MapOrderType {
            _as: proto::MapOrderType::Unordered,
        }
    }

    /// Order map by key.
    pub fn Key_Ordered() -> Self {
        MapOrderType {
            _as: proto::MapOrderType::KeyOrdered,
        }
    }

    /// Order map by key, then value.
    pub fn Key_Value_Ordered() -> Self {
        MapOrderType {
            _as: proto::MapOrderType::KeyValueOrdered,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CDTContext
//
////////////////////////////////////////////////////////////////////////////////////////////

enum CDTContextType {
    ListIndex = 0x10,
    ListRank = 0x11,
    ListValue = 0x13,
    MapIndex = 0x20,
    MapRank = 0x21,
    MapKey = 0x22,
    MapValue = 0x23,
}

/// CDTContext defines Nested CDT context. Identifies the location of nested list/map to apply the operation.
/// for the current level.
/// An array of CTX identifies location of the list/map on multiple
/// levels on nesting.
#[php_class(name = "Aerospike\\Context")]
pub struct CDTContext {
    _as: proto::CdtContext,
}

/// `CDTContext` excapsulates parameters for transaction policy attributes
/// used in all database operation calls.
#[php_impl]
#[derive(ZvalConvert)]
impl CDTContext {
    pub fn __construct() -> Self {
        CDTContext {
            _as: proto::CdtContext::default(),
        }
    }

    fn list_order_flag(order: ListOrderType, pad: bool) -> i32 {
        if order.flag() == 1 {
            return 0xc0;
        }

        if pad {
            return 0x80;
        }

        return 0x40;
    }

    /// CtxListIndex defines Lookup list by index offset.
    /// If the index is negative, the resolved index starts backwards from end of list.
    /// If an index is out of bounds, a parameter error will be returned.
    /// Examples:
    /// 0: First item.
    /// 4: Fifth item.
    /// -1: Last item.
    /// -3: Third to last item.
    pub fn ListIndex(index: i32) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::ListIndex as i32,
                value: Some(PHPValue::Int(index.into()).into()),
            },
        }
    }

    /// CtxListIndexCreate list with given type at index offset, given an order and pad.
    pub fn ListIndexCreate(index: i32, order: ListOrderType, pad: bool) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::ListIndex as i32 | Self::list_order_flag(order, pad),
                value: Some(PHPValue::Int(index.into()).into()),
            },
        }
    }

    /// CtxListRank defines Lookup list by rank.
    /// 0 = smallest value
    /// N = Nth smallest value
    /// -1 = largest value
    pub fn ListRank(rank: i32) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::ListRank as i32,
                value: Some(PHPValue::Int(rank.into()).into()),
            },
        }
    }

    /// CtxListValue defines Lookup list by value.
    pub fn ListValue(key: PHPValue) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::ListValue as i32,
                value: Some(key.into()),
            },
        }
    }

    /// CtxMapIndex defines Lookup map by index offset.
    /// If the index is negative, the resolved index starts backwards from end of list.
    /// If an index is out of bounds, a parameter error will be returned.
    /// Examples:
    /// 0: First item.
    /// 4: Fifth item.
    /// -1: Last item.
    /// -3: Third to last item.
    pub fn MapIndex(index: i32) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::MapIndex as i32,
                value: Some(PHPValue::Int(index.into()).into()),
            },
        }
    }

    /// CtxMapRank defines Lookup map by rank.
    /// 0 = smallest value
    /// N = Nth smallest value
    /// -1 = largest value
    pub fn MapRank(rank: i32) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::MapRank as i32,
                value: Some(PHPValue::Int(rank.into()).into()),
            },
        }
    }

    /// CtxMapKey defines Lookup map by key.
    pub fn MapKey(key: PHPValue) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::MapKey as i32,
                value: Some(key.into()),
            },
        }
    }

    /// CtxMapKeyCreate creates map with given type at map key.
    pub fn MapKeyCreate(key: PHPValue, order: MapOrderType) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::MapKey as i32 | order.flag(),
                value: Some(key.into()),
            },
        }
    }

    /// CtxMapValue defines Lookup map by value.
    pub fn MapValue(key: PHPValue) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::MapValue as i32,
                value: Some(key.into()),
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ReadPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// `ReadPolicy` encapsulates parameters for transaction policy attributes
/// used in all database operation calls.
#[php_class(name = "Aerospike\\ReadPolicy")]
pub struct ReadPolicy {
    _as: proto::ReadPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ReadPolicy {
    pub fn __construct() -> Self {
        ReadPolicy::default()
    }

    /// MaxRetries determines the maximum number of retries before aborting the current transaction.
    /// The initial attempt is not counted as a retry.
    ///
    /// If MaxRetries is exceeded, the transaction will abort with an error.
    ///
    /// WARNING: Database writes that are not idempotent (such as AddOp)
    /// should not be retried because the write operation may be performed
    /// multiple times if the client timed out previous transaction attempts.
    /// It's important to use a distinct WritePolicy for non-idempotent
    /// writes which sets maxRetries = 0;
    ///
    /// Default for read: 2 (initial attempt + 2 retries = 3 attempts)
    ///
    /// Default for write: 0 (no retries)
    ///
    /// Default for partition scan or query with nil filter: 5
    /// (6 attempts. See ScanPolicy comments.)
    #[getter]
    pub fn get_max_retries(&self) -> u32 {
        self._as.max_retries
    }

    #[setter]
    pub fn set_max_retries(&mut self, max_retries: u32) {
        self._as.max_retries = max_retries;
    }

    /// SleepMultiplier specifies the multiplying factor to be used for exponential backoff during retries.
    /// Default to (1.0); Only values greater than 1 are valid.
    #[getter]
    pub fn get_sleep_multiplier(&self) -> f64 {
        self._as.sleep_multiplier
    }

    #[setter]
    pub fn set_sleep_multiplier(&mut self, sleep_multiplier: f64) {
        self._as.sleep_multiplier = sleep_multiplier;
    }

    /// TotalTimeout specifies total transaction timeout.
    ///
    /// The TotalTimeout is tracked on the client and also sent to the server along
    /// with the transaction in the wire protocol. The client will most likely
    /// timeout first, but the server has the capability to Timeout the transaction.
    ///
    /// If TotalTimeout is not zero and TotalTimeout is reached before the transaction
    /// completes, the transaction will abort with TotalTimeout error.
    ///
    /// If TotalTimeout is zero, there will be no time limit and the transaction will retry
    /// on network timeouts/errors until MaxRetries is exceeded. If MaxRetries is exceeded, the
    /// transaction also aborts with Timeout error.
    ///
    /// Default for scan/query: 0 (no time limit and rely on MaxRetries)
    ///
    /// Default for all other commands: 1000ms
    #[getter]
    pub fn get_total_timeout(&self) -> u64 {
        self._as.total_timeout
    }

    #[setter]
    pub fn set_total_timeout(&mut self, timeout_millis: u64) {
        self._as.total_timeout = timeout_millis;
    }

    /// SocketTimeout determines network timeout for each attempt.
    ///
    /// If SocketTimeout is not zero and SocketTimeout is reached before an attempt completes,
    /// the Timeout above is checked. If Timeout is not exceeded, the transaction
    /// is retried. If both SocketTimeout and Timeout are non-zero, SocketTimeout must be less
    /// than or equal to Timeout, otherwise Timeout will also be used for SocketTimeout.
    ///
    /// Default: 30s
    #[getter]
    pub fn get_socket_timeout(&self) -> u64 {
        self._as.socket_timeout
    }

    #[setter]
    pub fn set_socket_timeout(&mut self, timeout_millis: u64) {
        self._as.socket_timeout = timeout_millis;
    }

    /// ReadTouchTTLPercent determines how record TTL (time to live) is affected on reads. When enabled, the server can
    /// efficiently operate as a read-based LRU cache where the least recently used records are expired.
    /// The value is expressed as a percentage of the TTL sent on the most recent write such that a read
    /// within this interval of the record’s end of life will generate a touch.
    ///
    /// For example, if the most recent write had a TTL of 10 hours and read_touch_ttl_percent is set to
    /// 80, the next read within 8 hours of the record's end of life (equivalent to 2 hours after the most
    /// recent write) will result in a touch, resetting the TTL to another 10 hours.
    ///
    /// Values:
    ///
    /// 0 : Use server config default-read-touch-ttl-pct for the record's namespace/set.
    /// -1 : Do not reset record TTL on reads.
    /// 1 - 100 : Reset record TTL on reads when within this percentage of the most recent write TTL.
    /// Default: 0
    #[getter]
    pub fn get_read_touch_ttl_percent(&self) -> i32 {
        self._as.read_touch_ttl_percent
    }

    #[setter]
    pub fn set_read_touch_ttl_percent(&mut self, percent: i32) {
        self._as.read_touch_ttl_percent = percent;
    }

    /// SendKey determines to whether send user defined key in addition to hash digest on both reads and writes.
    /// If the key is sent on a write, the key will be stored with the record on
    /// the server.
    /// The default is to not send the user defined key.
    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as.send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as.send_key = send_key;
    }

    /// UseCompression uses zlib compression on command buffers sent to the server and responses received
    /// from the server when the buffer size is greater than 128 bytes.
    ///
    /// This option will increase cpu and memory usage (for extra compressed buffers),but
    /// decrease the size of data sent over the network.
    ///
    /// Default: false
    #[getter]
    pub fn get_use_compression(&self) -> bool {
        self._as.use_compression
    }

    #[setter]
    pub fn set_use_compression(&mut self, use_compression: bool) {
        self._as.use_compression = use_compression;
    }

    /// ExitFastOnExhaustedConnectionPool determines if a command that tries to get a
    /// connection from the connection pool will wait and retry in case the pool is
    /// exhausted until a connection becomes available (or the TotalTimeout is reached).
    /// If set to true, an error will be return immediately.
    /// If set to false, getting a connection will be retried.
    /// This only applies if LimitConnectionsToQueueSize is set to true and the number of open connections to a node has reached ConnectionQueueSize.
    /// The default is false
    #[getter]
    pub fn get_exit_fast_on_exhausted_connection_pool(&self) -> bool {
        self._as.exit_fast_on_exhausted_connection_pool
    }

    #[setter]
    pub fn set_exit_fast_on_exhausted_connection_pool(
        &mut self,
        exit_fast_on_exhausted_connection_pool: bool,
    ) {
        self._as.exit_fast_on_exhausted_connection_pool = exit_fast_on_exhausted_connection_pool;
    }

    /// ReadModeAP indicates read policy for AP (availability) namespaces.
    #[getter]
    pub fn get_read_mode_ap(&self) -> ReadModeAP {
        ReadModeAP {
            _as: match self._as.read_mode_ap {
                0 => proto::ReadModeAp::One,
                1 => proto::ReadModeAp::All,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_ap(&mut self, read_mode_ap: ReadModeAP) {
        self._as.read_mode_ap = read_mode_ap._as.into();
    }

    /// ReadModeSC indicates read policy for SC (strong consistency) namespaces.
    #[getter]
    pub fn get_read_mode_sc(&self) -> ReadModeSC {
        ReadModeSC {
            _as: match self._as.read_mode_ap {
                0 => proto::ReadModeSc::Session,
                1 => proto::ReadModeSc::Linearize,
                2 => proto::ReadModeSc::AllowReplica,
                3 => proto::ReadModeSc::AllowUnavailable,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_sc(&mut self, read_mode_sc: ReadModeSC) {
        self._as.read_mode_sc = read_mode_sc._as.into();
    }

    /// FilterExpression is the optional Filter Expression. Supported on Server v5.2+
    #[getter]
    pub fn get_filter_expression(&self) -> Option<Expression> {
        self._as
            .filter_expression
            .clone()
            .map(|fe| Expression { _as: fe })
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }
}

impl Default for ReadPolicy {
    fn default() -> Self {
        ReadPolicy {
            _as: proto::ReadPolicy {
                max_retries: 3,
                sleep_multiplier: 1.0,
                total_timeout: 1000,
                socket_timeout: 500,
                send_key: false,
                use_compression: false,
                exit_fast_on_exhausted_connection_pool: false,
                read_mode_ap: proto::ReadModeAp::One.into(),
                read_mode_sc: proto::ReadModeSc::Session.into(),
                filter_expression: None,
                sleep_between_retries: 1,
                replica_policy: 1,
                read_touch_ttl_percent: 0,
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  AdminPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// `AdminPolicy` encapsulates parameters for all admin operations.
#[php_class(name = "Aerospike\\AdminPolicy")]
pub struct AdminPolicy {
    _as: proto::AdminPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl AdminPolicy {
    pub fn __construct() -> Self {
        AdminPolicy::default()
    }

    /// User administration command socket timeout.
    /// Default is 2 seconds.
    #[getter]
    pub fn get_timeout(&self) -> u32 {
        self._as.timeout
    }

    #[setter]
    pub fn set_timeout(&mut self, timeout_millis: u32) {
        self._as.timeout = timeout_millis;
    }
}

impl Default for AdminPolicy {
    fn default() -> Self {
        AdminPolicy {
            _as: proto::AdminPolicy { timeout: 3000 },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  InfoPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// `InfoPolicy` encapsulates parameters for all info operations.
#[php_class(name = "Aerospike\\InfoPolicy")]
pub struct InfoPolicy {
    _as: proto::InfoPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl InfoPolicy {
    pub fn __construct() -> Self {
        InfoPolicy::default()
    }
}

impl Default for InfoPolicy {
    fn default() -> Self {
        InfoPolicy {
            _as: proto::InfoPolicy { timeout: 3000 },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  WritePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_class(name = "Aerospike\\WritePolicy")]
pub struct WritePolicy {
    _as: proto::WritePolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl WritePolicy {
    pub fn __construct() -> Self {
        WritePolicy::default()
    }

    /// RecordExistsAction qualifies how to handle writes where the record already exists.
    #[getter]
    pub fn get_record_exists_action(&self) -> RecordExistsAction {
        RecordExistsAction {
            _as: match &self._as.record_exists_action {
                0 => proto::RecordExistsAction::Update,
                1 => proto::RecordExistsAction::UpdateOnly,
                2 => proto::RecordExistsAction::Replace,
                3 => proto::RecordExistsAction::ReplaceOnly,
                4 => proto::RecordExistsAction::CreateOnly,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_record_exists_action(&mut self, record_exists_action: RecordExistsAction) {
        self._as.record_exists_action = record_exists_action._as.into();
    }

    /// GenerationPolicy qualifies how to handle record writes based on record generation. The default (NONE)
    /// indicates that the generation is not used to restrict writes.
    #[getter]
    pub fn get_generation_policy(&self) -> GenerationPolicy {
        GenerationPolicy {
            _as: match &self._as.generation_policy {
                0 => proto::GenerationPolicy::None,
                1 => proto::GenerationPolicy::ExpectGenEqual,
                2 => proto::GenerationPolicy::ExpectGenGt,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_generation_policy(&mut self, generation_policy: GenerationPolicy) {
        self._as.generation_policy = generation_policy._as.into();
    }

    /// Desired consistency guarantee when committing a transaction on the server. The default
    /// (COMMIT_ALL) indicates that the server should wait for master and all replica commits to
    /// be successful before returning success to the client.
    #[getter]
    pub fn get_commit_level(&self) -> CommitLevel {
        CommitLevel {
            _as: match &self._as.commit_level {
                0 => proto::CommitLevel::CommitAll,
                1 => proto::CommitLevel::CommitMaster,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_commit_level(&mut self, commit_level: CommitLevel) {
        self._as.commit_level = commit_level._as.into();
    }

    /// Generation determines expected generation.
    /// Generation is the number of times a record has been
    /// modified (including creation) on the server.
    /// If a write operation is creating a record, the expected generation would be 0.
    #[getter]
    pub fn get_generation(&self) -> u32 {
        self._as.generation
    }

    #[setter]
    pub fn set_generation(&mut self, generation: u32) {
        self._as.generation = generation;
    }

    /// Expiration determines record expiration in seconds. Also known as TTL (Time-To-Live).
    /// Seconds record will live before being removed by the server.
    /// Expiration values:
    /// TTLServerDefault (0): Default to namespace configuration variable "default-ttl" on the server.
    /// TTLDontExpire (MaxUint32): Never expire for Aerospike 2 server versions >= 2.7.2 and Aerospike 3+ server
    /// TTLDontUpdate (MaxUint32 - 1): Do not change ttl when record is written. Supported by Aerospike server versions >= 3.10.1
    /// > 0: Actual expiration in seconds.
    #[getter]
    pub fn get_expiration(&self) -> Expiration {
        match self._as.expiration {
            NAMESPACE_DEFAULT => Expiration::Namespace_Default(),
            NEVER_EXPIRE => Expiration::Never(),
            DONT_UPDATE => Expiration::Dont_Update(),
            secs => Expiration::Seconds(secs),
        }
    }

    #[setter]
    pub fn set_expiration(&mut self, expiration: Expiration) {
        self._as.expiration = (&expiration).into();
    }

    /// RespondPerEachOp defines for client.Operate() method, return a result for every operation.
    /// Some list operations do not return results by default (ListClearOp() for example).
    /// This can sometimes make it difficult to determine the desired result offset in the returned
    /// bin's result list.
    ///
    /// Setting RespondPerEachOp to true makes it easier to identify the desired result offset
    /// (result offset equals bin's operate sequence). This only makes sense when multiple list
    /// operations are used in one operate call and some of those operations do not return results
    /// by default.
    #[getter]
    pub fn get_respond_per_each_op(&self) -> bool {
        self._as.respond_per_each_op
    }

    #[setter]
    pub fn set_respond_per_each_op(&mut self, respond_per_each_op: bool) {
        self._as.respond_per_each_op = respond_per_each_op;
    }

    /// DurableDelete leaves a tombstone for the record if the transaction results in a record deletion.
    /// This prevents deleted records from reappearing after node failures.
    /// Valid for Aerospike Server Enterprise Edition 3.10+ only.
    #[getter]
    pub fn get_durable_delete(&self) -> bool {
        self._as.respond_per_each_op
    }

    #[setter]
    pub fn set_durable_delete(&mut self, durable_delete: bool) {
        self._as.durable_delete = durable_delete;
    }

    /// ***************************************************************************
    /// ReadPolicy Attrs
    /// ***************************************************************************

    #[getter]
    pub fn get_max_retries(&self) -> u32 {
        self._as.policy.as_ref().unwrap().max_retries
    }

    #[setter]
    pub fn set_max_retries(&mut self, max_retries: u32) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.max_retries = max_retries);
    }

    #[getter]
    pub fn get_sleep_multiplier(&self) -> f64 {
        self._as.policy.as_ref().unwrap().sleep_multiplier
    }

    #[setter]
    pub fn set_sleep_multiplier(&mut self, sleep_multiplier: f64) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.sleep_multiplier = sleep_multiplier);
    }

    #[getter]
    pub fn get_total_timeout(&self) -> u64 {
        self._as.policy.as_ref().unwrap().total_timeout
    }

    #[setter]
    pub fn set_total_timeout(&mut self, timeout_millis: u64) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.total_timeout = timeout_millis);
    }

    #[getter]
    pub fn get_socket_timeout(&self) -> u64 {
        self._as.policy.as_ref().unwrap().socket_timeout
    }

    #[setter]
    pub fn set_socket_timeout(&mut self, timeout_millis: u64) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.socket_timeout = timeout_millis);
    }

    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as.policy.as_ref().unwrap().send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.send_key = send_key);
    }

    #[getter]
    pub fn get_use_compression(&self) -> bool {
        self._as.policy.as_ref().unwrap().use_compression
    }

    #[setter]
    pub fn set_use_compression(&mut self, use_compression: bool) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.use_compression = use_compression);
    }

    #[getter]
    pub fn get_exit_fast_on_exhausted_connection_pool(&self) -> bool {
        self._as
            .policy
            .as_ref()
            .unwrap()
            .exit_fast_on_exhausted_connection_pool
    }

    #[setter]
    pub fn set_exit_fast_on_exhausted_connection_pool(
        &mut self,
        exit_fast_on_exhausted_connection_pool: bool,
    ) {
        self._as.policy.as_mut().map(|ref mut p| {
            p.exit_fast_on_exhausted_connection_pool = exit_fast_on_exhausted_connection_pool
        });
    }

    #[getter]
    pub fn get_read_mode_ap(&self) -> ReadModeAP {
        ReadModeAP {
            _as: match self._as.policy.as_ref().unwrap().read_mode_ap {
                0 => proto::ReadModeAp::One,
                1 => proto::ReadModeAp::All,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_ap(&mut self, read_mode_ap: ReadModeAP) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.read_mode_ap = read_mode_ap._as.into());
    }

    #[getter]
    pub fn get_read_mode_sc(&self) -> ReadModeSC {
        ReadModeSC {
            _as: match self._as.policy.as_ref().unwrap().read_mode_ap {
                0 => proto::ReadModeSc::Session,
                1 => proto::ReadModeSc::Linearize,
                2 => proto::ReadModeSc::AllowReplica,
                3 => proto::ReadModeSc::AllowUnavailable,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_sc(&mut self, read_mode_sc: ReadModeSC) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.read_mode_sc = read_mode_sc._as.into());
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<Expression> {
        self._as
            .policy
            .as_ref()
            .unwrap()
            .filter_expression
            .clone()
            .map(|fe| Expression { _as: fe })
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
        match filter_expression {
            Some(fe) => self
                ._as
                .policy
                .as_mut()
                .map(|ref mut p| p.filter_expression = Some(fe._as)),
            None => self
                ._as
                .policy
                .as_mut()
                .map(|ref mut p| p.filter_expression = None),
        };
    }
}

impl Default for WritePolicy {
    fn default() -> Self {
        WritePolicy {
            _as: proto::WritePolicy {
                policy: Some(proto::ReadPolicy::default()),
                record_exists_action: proto::RecordExistsAction::Update.into(),
                generation_policy: proto::GenerationPolicy::None.into(),
                commit_level: proto::CommitLevel::CommitAll.into(),
                generation: 0,
                expiration: 0,
                respond_per_each_op: false,
                durable_delete: false,
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  MultiPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// MultiPolicy contains parameters for policy attributes used in
/// query and scan operations.
pub struct MultiPolicy {
    _as: proto::MultiPolicy,
}

impl Default for MultiPolicy {
    fn default() -> Self {
        let mut rp = ReadPolicy::default();
        rp.set_total_timeout(0);
        MultiPolicy {
            _as: proto::MultiPolicy {
                read_policy: Some(rp._as),
                max_concurrent_nodes: 0,
                records_per_second: 0,
                record_queue_size: 50,
                include_bin_data: true,
                max_records: 0,
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  QueryPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// QueryPolicy encapsulates parameters for policy attributes used in query operations.
#[php_class(name = "Aerospike\\QueryPolicy")]
pub struct QueryPolicy {
    _as: proto::QueryPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl QueryPolicy {
    pub fn __construct() -> Self {
        QueryPolicy::default()
    }

    /// QueryDuration represents the expected duration for a query operation in the Aerospike database.
    /// It provides options for specifying whether a query is expected to return a large number of records per node (Long),
    /// a small number of records per node (Short), or a long query with relaxed read consistency for AP namespaces (LongRelaxAP).
    /// These options influence how the server optimizes query execution to meet the expected duration requirements.
    #[getter]
    pub fn get_expected_duration(&self) -> QueryDuration {
        self._as.expected_duration.into()
    }

    #[setter]
    pub fn set_expected_duration(&mut self, expected_duration: QueryDuration) {
        self._as.expected_duration = expected_duration._as.into();
    }

    /// ***************************************************************************
    /// MultiPolicy Attrs
    /// ***************************************************************************

    /// Maximum number of concurrent requests to server nodes at any point in time.
    /// If there are 16 nodes in the cluster and maxConcurrentNodes is 8, then queries
    /// will be made to 8 nodes in parallel.  When a query completes, a new query will
    /// be issued until all 16 nodes have been queried.
    /// Default (0) is to issue requests to all server nodes in parallel.
    /// 1 will to issue requests to server nodes one by one avoiding parallel queries.
    #[getter]
    pub fn get_max_concurrent_nodes(&self) -> u32 {
        self._as.multi_policy.as_ref().unwrap().max_concurrent_nodes
    }

    #[setter]
    pub fn set_max_concurrent_nodes(&mut self, max_concurrent_nodes: u32) {
        self._as.multi_policy.as_mut().unwrap().max_concurrent_nodes = max_concurrent_nodes;
    }

    /// Number of records to place in queue before blocking.
    /// Records received from multiple server nodes will be placed in a queue.
    /// A separate goroutine consumes these records in parallel.
    /// If the queue is full, the producer goroutines will block until records are consumed.
    #[getter]
    pub fn get_record_queue_size(&self) -> u32 {
        self._as.multi_policy.as_ref().unwrap().record_queue_size
    }

    #[setter]
    pub fn set_record_queue_size(&mut self, record_queue_size: u32) {
        self._as.multi_policy.as_mut().unwrap().record_queue_size = record_queue_size;
    }

    /// ***************************************************************************
    /// ReadPolicy Attrs
    /// ***************************************************************************

    #[getter]
    pub fn get_max_retries(&self) -> u32 {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .max_retries
    }

    #[setter]
    pub fn set_max_retries(&mut self, max_retries: u32) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.max_retries = max_retries);
    }

    #[getter]
    pub fn get_sleep_multiplier(&self) -> f64 {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .sleep_multiplier
    }

    #[setter]
    pub fn set_sleep_multiplier(&mut self, sleep_multiplier: f64) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.sleep_multiplier = sleep_multiplier);
    }

    #[getter]
    pub fn get_total_timeout(&self) -> u64 {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .total_timeout
    }

    #[setter]
    pub fn set_total_timeout(&mut self, timeout_millis: u64) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.total_timeout = timeout_millis);
    }

    #[getter]
    pub fn get_socket_timeout(&self) -> u64 {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .socket_timeout
    }

    #[setter]
    pub fn set_socket_timeout(&mut self, timeout_millis: u64) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.socket_timeout = timeout_millis);
    }

    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.send_key = send_key);
    }

    #[getter]
    pub fn get_use_compression(&self) -> bool {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .use_compression
    }

    #[setter]
    pub fn set_use_compression(&mut self, use_compression: bool) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.use_compression = use_compression);
    }

    #[getter]
    pub fn get_exit_fast_on_exhausted_connection_pool(&self) -> bool {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .exit_fast_on_exhausted_connection_pool
    }

    #[setter]
    pub fn set_exit_fast_on_exhausted_connection_pool(
        &mut self,
        exit_fast_on_exhausted_connection_pool: bool,
    ) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| {
                p.exit_fast_on_exhausted_connection_pool = exit_fast_on_exhausted_connection_pool
            });
    }

    #[getter]
    pub fn get_read_mode_ap(&self) -> ReadModeAP {
        ReadModeAP {
            _as: match self
                ._as
                .multi_policy
                .as_ref()
                .unwrap()
                .read_policy
                .as_ref()
                .unwrap()
                .read_mode_ap
            {
                0 => proto::ReadModeAp::One,
                1 => proto::ReadModeAp::All,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_ap(&mut self, read_mode_ap: ReadModeAP) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.read_mode_ap = read_mode_ap._as.into());
    }

    #[getter]
    pub fn get_read_mode_sc(&self) -> ReadModeSC {
        ReadModeSC {
            _as: match self
                ._as
                .multi_policy
                .as_ref()
                .unwrap()
                .read_policy
                .as_ref()
                .unwrap()
                .read_mode_ap
            {
                0 => proto::ReadModeSc::Session,
                1 => proto::ReadModeSc::Linearize,
                2 => proto::ReadModeSc::AllowReplica,
                3 => proto::ReadModeSc::AllowUnavailable,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_sc(&mut self, read_mode_sc: ReadModeSC) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.read_mode_sc = read_mode_sc._as.into());
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<Expression> {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .filter_expression
            .clone()
            .map(|fe| Expression { _as: fe })
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
        match filter_expression {
            Some(fe) => self
                ._as
                .multi_policy
                .as_mut()
                .unwrap()
                .read_policy
                .as_mut()
                .map(|ref mut p| p.filter_expression = Some(fe._as)),
            None => self
                ._as
                .multi_policy
                .as_mut()
                .unwrap()
                .read_policy
                .as_mut()
                .map(|ref mut p| p.filter_expression = None),
        };
    }
}

impl Default for QueryPolicy {
    fn default() -> Self {
        QueryPolicy {
            _as: proto::QueryPolicy {
                multi_policy: Some(MultiPolicy::default()._as),
                expected_duration: QueryDuration::Long()._as.into(),
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ScanPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// `ScanPolicy` encapsulates optional parameters used in scan operations.
#[php_class(name = "Aerospike\\ScanPolicy")]
pub struct ScanPolicy {
    _as: proto::ScanPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ScanPolicy {
    pub fn __construct() -> Self {
        ScanPolicy::default()
    }

    /// ***************************************************************************
    /// MultiPolicy Attrs
    /// ***************************************************************************

    #[getter]
    pub fn get_max_records(&self) -> u64 {
        self._as.multi_policy.as_ref().unwrap().max_records
    }

    #[setter]
    pub fn set_max_records(&mut self, max_records: u64) {
        self._as.multi_policy.as_mut().unwrap().max_records = max_records;
    }

    #[getter]
    pub fn get_max_concurrent_nodes(&self) -> u32 {
        self._as.multi_policy.as_ref().unwrap().max_concurrent_nodes
    }

    #[setter]
    pub fn set_max_concurrent_nodes(&mut self, max_concurrent_nodes: u32) {
        self._as.multi_policy.as_mut().unwrap().max_concurrent_nodes = max_concurrent_nodes;
    }

    #[getter]
    pub fn get_record_queue_size(&self) -> u32 {
        self._as.multi_policy.as_ref().unwrap().record_queue_size
    }

    #[setter]
    pub fn set_record_queue_size(&mut self, record_queue_size: u32) {
        self._as.multi_policy.as_mut().unwrap().record_queue_size = record_queue_size;
    }

    /// ***************************************************************************
    /// ReadPolicy Attrs
    /// ***************************************************************************

    #[getter]
    pub fn get_max_retries(&self) -> u32 {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .max_retries
    }

    #[setter]
    pub fn set_max_retries(&mut self, max_retries: u32) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.max_retries = max_retries);
    }

    #[getter]
    pub fn get_sleep_multiplier(&self) -> f64 {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .sleep_multiplier
    }

    #[setter]
    pub fn set_sleep_multiplier(&mut self, sleep_multiplier: f64) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.sleep_multiplier = sleep_multiplier);
    }

    #[getter]
    pub fn get_total_timeout(&self) -> u64 {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .total_timeout
    }

    #[setter]
    pub fn set_total_timeout(&mut self, timeout_millis: u64) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.total_timeout = timeout_millis);
    }

    #[getter]
    pub fn get_socket_timeout(&self) -> u64 {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .socket_timeout
    }

    #[setter]
    pub fn set_socket_timeout(&mut self, timeout_millis: u64) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.socket_timeout = timeout_millis);
    }

    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.send_key = send_key);
    }

    #[getter]
    pub fn get_use_compression(&self) -> bool {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .use_compression
    }

    #[setter]
    pub fn set_use_compression(&mut self, use_compression: bool) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.use_compression = use_compression);
    }

    #[getter]
    pub fn get_exit_fast_on_exhausted_connection_pool(&self) -> bool {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .exit_fast_on_exhausted_connection_pool
    }

    #[setter]
    pub fn set_exit_fast_on_exhausted_connection_pool(
        &mut self,
        exit_fast_on_exhausted_connection_pool: bool,
    ) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| {
                p.exit_fast_on_exhausted_connection_pool = exit_fast_on_exhausted_connection_pool
            });
    }

    #[getter]
    pub fn get_read_mode_ap(&self) -> ReadModeAP {
        ReadModeAP {
            _as: match self
                ._as
                .multi_policy
                .as_ref()
                .unwrap()
                .read_policy
                .as_ref()
                .unwrap()
                .read_mode_ap
            {
                0 => proto::ReadModeAp::One,
                1 => proto::ReadModeAp::All,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_ap(&mut self, read_mode_ap: ReadModeAP) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.read_mode_ap = read_mode_ap._as.into());
    }

    #[getter]
    pub fn get_read_mode_sc(&self) -> ReadModeSC {
        ReadModeSC {
            _as: match self
                ._as
                .multi_policy
                .as_ref()
                .unwrap()
                .read_policy
                .as_ref()
                .unwrap()
                .read_mode_ap
            {
                0 => proto::ReadModeSc::Session,
                1 => proto::ReadModeSc::Linearize,
                2 => proto::ReadModeSc::AllowReplica,
                3 => proto::ReadModeSc::AllowUnavailable,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_sc(&mut self, read_mode_sc: ReadModeSC) {
        self._as
            .multi_policy
            .as_mut()
            .unwrap()
            .read_policy
            .as_mut()
            .map(|ref mut p| p.read_mode_sc = read_mode_sc._as.into());
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<Expression> {
        self._as
            .multi_policy
            .as_ref()
            .unwrap()
            .read_policy
            .as_ref()
            .unwrap()
            .filter_expression
            .clone()
            .map(|fe| Expression { _as: fe })
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
        match filter_expression {
            Some(fe) => self
                ._as
                .multi_policy
                .as_mut()
                .unwrap()
                .read_policy
                .as_mut()
                .map(|ref mut p| p.filter_expression = Some(fe._as)),
            None => self
                ._as
                .multi_policy
                .as_mut()
                .unwrap()
                .read_policy
                .as_mut()
                .map(|ref mut p| p.filter_expression = None),
        };
    }
}

impl Default for ScanPolicy {
    fn default() -> Self {
        ScanPolicy {
            _as: proto::ScanPolicy {
                multi_policy: Some(MultiPolicy::default()._as),
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  IndexCollectionType
//
////////////////////////////////////////////////////////////////////////////////////////////

/// IndexCollectionType is the secondary index collection type.
#[php_class(name = "Aerospike\\IndexCollectionType")]
pub struct IndexCollectionType {
    _as: proto::IndexCollectionType,
}

#[php_impl]
#[derive(ZvalConvert)]
impl IndexCollectionType {
    /// ICT_DEFAULT is the Normal scalar index.
    pub fn Default() -> Self {
        IndexCollectionType {
            _as: proto::IndexCollectionType::Default,
        }
    }

    /// ICT_LIST is Index list elements.
    pub fn List() -> Self {
        IndexCollectionType {
            _as: proto::IndexCollectionType::List,
        }
    }

    /// ICT_MAPKEYS is Index map keys.
    pub fn MapKeys() -> Self {
        IndexCollectionType {
            _as: proto::IndexCollectionType::MapKeys,
        }
    }

    /// ICT_MAPVALUES is Index map values.
    pub fn MapValues() -> Self {
        IndexCollectionType {
            _as: proto::IndexCollectionType::MapValues,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ParticleType
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Server particle types. Unsupported types are commented out.
#[php_class(name = "Aerospike\\ParticleType")]
pub struct ParticleType {
    _as: proto::ParticleType,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ParticleType {
    pub fn Null() -> Self {
        ParticleType {
            _as: proto::ParticleType::Null,
        }
    }

    pub fn Integer() -> Self {
        ParticleType {
            _as: proto::ParticleType::Integer,
        }
    }

    pub fn Float() -> Self {
        ParticleType {
            _as: proto::ParticleType::Float,
        }
    }

    pub fn String() -> Self {
        ParticleType {
            _as: proto::ParticleType::String,
        }
    }

    pub fn Blob() -> Self {
        ParticleType {
            _as: proto::ParticleType::Blob,
        }
    }

    pub fn Digest() -> Self {
        ParticleType {
            _as: proto::ParticleType::Digest,
        }
    }

    pub fn Bool() -> Self {
        ParticleType {
            _as: proto::ParticleType::Bool,
        }
    }

    pub fn Hll() -> Self {
        ParticleType {
            _as: proto::ParticleType::Hll,
        }
    }

    pub fn Map() -> Self {
        ParticleType {
            _as: proto::ParticleType::Map,
        }
    }

    pub fn List() -> Self {
        ParticleType {
            _as: proto::ParticleType::List,
        }
    }

    pub fn Geo_Json() -> Self {
        ParticleType {
            _as: proto::ParticleType::GeoJson,
        }
    }
}

impl From<ParticleType> for i64 {
    fn from(input: ParticleType) -> Self {
        match &input._as {
            proto::ParticleType::Null => 0,
            proto::ParticleType::Integer => 1,
            proto::ParticleType::Float => 2,
            proto::ParticleType::String => 3,
            proto::ParticleType::Blob => 4,
            proto::ParticleType::Digest => 6,
            proto::ParticleType::Bool => 17,
            proto::ParticleType::Hll => 18,
            proto::ParticleType::Map => 19,
            proto::ParticleType::List => 20,
            proto::ParticleType::Ldt => 21,
            proto::ParticleType::GeoJson => 23,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  IndexType
//
////////////////////////////////////////////////////////////////////////////////////////////

/// IndexType the type of the secondary index.
#[php_class(name = "Aerospike\\IndexType")]
pub struct IndexType {
    _as: proto::IndexType,
}

#[php_impl]
#[derive(ZvalConvert)]
impl IndexType {
    /// NUMERIC specifies an index on numeric values.
    pub fn Numeric() -> Self {
        IndexType {
            _as: proto::IndexType::Numeric,
        }
    }

    /// STRING specifies an index on string values.
    pub fn String() -> Self {
        IndexType {
            _as: proto::IndexType::String,
        }
    }

    /// BLOB specifies a []byte index. Requires server version 7.0+.
    pub fn Blob() -> Self {
        IndexType {
            _as: proto::IndexType::Blob,
        }
    }

    /// GEO2DSPHERE specifies 2-dimensional spherical geospatial index.
    pub fn Geo2DSphere() -> Self {
        IndexType {
            _as: proto::IndexType::Geo2DSphere,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Filter
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Query filter definition. Currently, only one filter is allowed in a Statement, and must be on a
/// bin which has a secondary index defined.
/// Filter instances should be instantiated using one of the provided macros.
#[php_class(name = "Aerospike\\Filter")]
pub struct Filter {
    _as: proto::QueryFilter,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Filter {
    /// NewEqualFilter creates a new equality filter instance for query.
    /// Value can be an integer, string or a blob (byte array). Byte arrays are only supported on server v7+.
    pub fn equal(bin_name: &str, value: PHPValue, ctx: Option<Vec<&CDTContext>>) -> Self {
        Filter {
            _as: proto::QueryFilter {
                name: bin_name.into(),
                idx_type: proto::IndexCollectionType::Default.into(),
                value_particle_type: value.particle_type() as u32,
                begin: Some(value.clone().into()),
                end: Some(value.clone().into()),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            },
        }
    }

    /// NewRangeFilter creates a range filter for query.
    /// Range arguments must be int64 values.
    /// String ranges are not supported.
    pub fn range(
        bin_name: &str,
        begin: PHPValue,
        end: PHPValue,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Self {
        Filter {
            _as: proto::QueryFilter {
                name: bin_name.into(),
                idx_type: proto::IndexCollectionType::Default.into(),
                value_particle_type: begin.particle_type() as u32,
                begin: Some(begin.clone().into()),
                end: Some(end.clone().into()),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            },
        }
    }

    /// NewContainsFilter creates a contains filter for query on collection index.
    /// Value can be an integer, string or a blob (byte array). Byte arrays are only supported on server v7+.
    pub fn contains(
        bin_name: &str,
        value: PHPValue,
        cit: Option<&IndexCollectionType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Self {
        let default = IndexCollectionType::Default();
        let cit = cit.unwrap_or(&default);
        Filter {
            _as: proto::QueryFilter {
                name: bin_name.into(),
                idx_type: cit._as.into(),
                value_particle_type: value.particle_type() as u32,
                begin: Some(value.clone().into()),
                end: Some(value.clone().into()),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            },
        }
    }

    /// NewContainsRangeFilter creates a contains filter for query on ranges of data in a collection index.
    pub fn contains_range(
        bin_name: &str,
        begin: PHPValue,
        end: PHPValue,
        cit: Option<&IndexCollectionType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Self {
        let default = IndexCollectionType::Default();
        let cit = cit.unwrap_or(&default);
        Filter {
            _as: proto::QueryFilter {
                name: bin_name.into(),
                idx_type: cit._as.into(),
                value_particle_type: begin.particle_type() as u32,
                begin: Some(begin.clone().into()),
                end: Some(end.clone().into()),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            },
        }
    }

    /// NewGeoWithinRegionFilter creates a geospatial "within region" filter for query.
    /// Argument must be a valid GeoJSON region.
    pub fn within_region(
        bin_name: &str,
        region: &str,
        cit: Option<&IndexCollectionType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Self {
        let default = IndexCollectionType::Default();
        let cit = cit.unwrap_or(&default);
        let region = Value::string(region.into());
        Filter {
            _as: proto::QueryFilter {
                name: bin_name.into(),
                idx_type: cit._as.into(),
                value_particle_type: PHPValue::GeoJSON("".into()).particle_type() as u32,
                begin: Some(region.clone().into()),
                end: Some(region.clone().into()),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            },
        }
    }

    /// NewGeoWithinRegionForCollectionFilter creates a geospatial "within region" filter for query on collection index.
    /// Argument must be a valid GeoJSON region.
    pub fn within_radius(
        bin_name: &str,
        lat: f64,
        lng: f64,
        radius: f64,
        cit: Option<&IndexCollectionType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Self {
        let default = IndexCollectionType::Default();
        let cit = cit.unwrap_or(&default);
        let rgnStr = format!(
            r#"{{ "type": "AeroCircle", "coordinates": [[{:.8}, {:.8}], {}] }}"#,
            lng, lat, radius
        );
        let value = Value::string(rgnStr);
        Filter {
            _as: proto::QueryFilter {
                name: bin_name.into(),
                idx_type: cit._as.into(),
                value_particle_type: PHPValue::GeoJSON("".into()).particle_type() as u32,
                begin: Some(value.clone().into()),
                end: Some(value.clone().into()),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            },
        }
    }

    /// NewGeoRegionsContainingPointFilter creates a geospatial "containing point" filter for query.
    /// Argument must be a valid GeoJSON point.
    pub fn regions_containing_point(
        bin_name: &str,
        lat: f64,
        lng: f64,
        cit: Option<&IndexCollectionType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Self {
        let default = IndexCollectionType::Default();
        let cit = cit.unwrap_or(&default);
        let point = format!(
            r#"{{"type":"Point","coordinates":[{:.8},{:.8}]}}"#,
            lng, lat
        );
        let value = Value::string(point);
        Filter {
            _as: proto::QueryFilter {
                name: bin_name.into(),
                idx_type: cit._as.into(),
                value_particle_type: PHPValue::GeoJSON("".into()).particle_type() as u32,
                begin: Some(value.clone().into()),
                end: Some(value.clone().into()),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            },
        }
    }
}

impl FromZval<'_> for Filter {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Filter = zval.extract()?;

        Some(Filter { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Statement
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Statement encapsulates query statement parameters.
#[php_class(name = "Aerospike\\Statement")]
pub struct Statement {
    _as: proto::Statement,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Statement {
    pub fn __construct(
        namespace: &str,
        set_name: &str,
        filter: Option<Filter>,
        bin_names: Option<Vec<String>>,
    ) -> Self {
        let mut rng = rand::thread_rng();
        let filter_proto = filter.map(|f| f._as.clone());
        Statement {
            _as: proto::Statement {
                namespace: namespace.into(),
                set_name: set_name.into(),
                bin_names: bin_names.unwrap_or_default(),
                return_data: true,
                task_id: rng.gen(),
                filter: filter_proto,
                index_name: None,
                udf_call: None,
            },
        }
    }

    /// Filter determines query index filter (Optional).
    /// This filter is applied to the secondary index on query.
    /// Query index filters must reference a bin which has a secondary index defined.
    #[getter]
    pub fn get_filter(&self) -> Option<Filter> {
        self._as.filter.as_ref().map(|f| Filter { _as: f.clone() })
    }

    #[setter]
    pub fn set_filter(&mut self, filter: Option<Filter>) {
        self._as.filter = filter.map(|f| f._as.clone());
    }

    /// IndexName determines query index name (Optional)
    /// If not set, the server will determine the index from the filter's bin name.
    #[getter]
    pub fn get_index_name(&self) -> Option<String> {
        self._as.index_name.clone()
    }

    #[setter]
    pub fn set_index_name(&mut self, index_name: Option<String>) {
        self._as.index_name = index_name;
    }

    /// BinNames detemines bin names (optional)
    #[getter]
    pub fn get_bin_names(&self) -> Vec<String> {
        self._as.bin_names.clone()
    }

    #[setter]
    pub fn set_bin_names(&mut self, bin_names: Vec<String>) {
        self._as.bin_names = bin_names;
    }

    /// Namespace determines query Namespace
    #[getter]
    pub fn get_namespace(&self) -> String {
        self._as.namespace.clone()
    }

    #[setter]
    pub fn set_namespace(&mut self, namespace: String) {
        self._as.namespace = namespace;
    }

    /// SetName determines query Set name (Optional)
    #[getter]
    pub fn get_setname(&self) -> String {
        self._as.set_name.clone()
    }

    #[setter]
    pub fn set_setname(&mut self, set_name: String) {
        self._as.set_name = set_name;
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  PartitionStatus
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Virtual collection of records retrieved through queries and scans. During a query/scan,
/// multiple threads will retrieve records from the server nodes and put these records on an
/// internal queue managed by the recordset. The single user thread consumes these records from the
/// queue.
#[php_class(name = "Aerospike\\PartitionStatus")]
pub struct PartitionStatus {
    _as: proto::PartitionStatus,
}

#[php_impl]
#[derive(ZvalConvert)]
impl PartitionStatus {
    pub fn __construct(id: u32) -> Self {
        PartitionStatus {
            _as: proto::PartitionStatus {
                bval: None,
                id: id,
                retry: false,
                digest: vec![],
            },
        }
    }

    /// get BVal
    #[getter]
    pub fn get_bval(&self) -> Option<i64> {
        self._as.bval
    }

    /// Id shows the partition Id.
    #[getter]
    pub fn get_partition_id(&self) -> u32 {
        self._as.id
    }

    /// Digest records the digest of the last key digest received from the server
    /// for this partition.
    #[getter]
    pub fn get_digest(&self) -> Vec<u8> {
        self._as.digest.clone()
    }

    /// Retry signifies if the partition requires a retry.
    #[getter]
    pub fn get_retry(&self) -> bool {
        self._as.retry
    }
}

impl From<&proto::PartitionStatus> for PartitionStatus {
    fn from(input: &proto::PartitionStatus) -> Self {
        PartitionStatus { _as: input.clone() }
    }
}

impl FromZval<'_> for PartitionStatus {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &PartitionStatus = zval.extract()?;

        Some(PartitionStatus { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  PartitionFilter
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Virtual collection of records retrieved through queries and scans. During a query/scan,
/// multiple threads will retrieve records from the server nodes and put these records on an
/// internal queue managed by the recordset. The single user thread consumes these records from the
/// queue.
#[php_class(name = "Aerospike\\PartitionFilter")]
pub struct PartitionFilter {
    _as: Arc<Mutex<proto::PartitionFilter>>,
}

#[php_impl]
#[derive(ZvalConvert)]
impl PartitionFilter {
    pub fn __construct() -> Self {
        Self::all()
    }

    #[getter]
    pub fn get_partition_status(&self) -> Vec<PartitionStatus> {
        let p = self._as.lock().unwrap();
        p.partitions.iter().map(|ps| ps.into()).collect()
    }

    /// NewPartitionFilterAll creates a partition filter that
    /// reads all the partitions.
    pub fn all() -> Self {
        PartitionFilter {
            _as: Arc::new(Mutex::new(proto::PartitionFilter {
                begin: 0,
                count: PARTITIONS as u32,
                digest: vec![],
                partitions: vec![],
                done: false,
                retry: false,
            })),
        }
    }

    /// NewPartitionFilterById creates a partition filter by partition id.
    /// Partition id is between 0 - 4095
    pub fn partition(id: u32) -> Self {
        PartitionFilter {
            _as: Arc::new(Mutex::new(proto::PartitionFilter {
                begin: id,
                count: 1,
                digest: vec![],
                partitions: vec![],
                done: false,
                retry: false,
            })),
        }
    }

    /// NewPartitionFilterByRange creates a partition filter by partition range.
    /// begin partition id is between 0 - 4095
    /// count is the number of partitions, in the range of 1 - 4096 inclusive.
    pub fn range(begin: u32, count: u32) -> Self {
        PartitionFilter {
            _as: Arc::new(Mutex::new(proto::PartitionFilter {
                begin: begin,
                count: count,
                digest: vec![],
                partitions: vec![],
                done: false,
                retry: false,
            })),
        }
    }

    fn init_partition_status(&mut self) {
        let mut p = self._as.lock().unwrap();
        if p.partitions.len() > 0 {
            return;
        }

        p.partitions = (0..PARTITIONS)
            .map(|id| PartitionStatus::__construct(id as u32)._as)
            .collect();
    }
}

impl FromZval<'_> for PartitionFilter {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &PartitionFilter = zval.extract()?;

        Some(PartitionFilter { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Recordset
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Virtual collection of records retrieved through queries and scans. During a query/scan,
/// multiple threads will retrieve records from the server nodes and put these records on an
/// internal queue managed by the recordset. The single user thread consumes these records from the
/// queue.
#[php_class(name = "Aerospike\\Recordset")]
pub struct Recordset {
    _as: Option<tonic::Streaming<proto::AerospikeStreamResponse>>,
    client: Arc<Mutex<grpc::BlockingClient>>,
    partition_filter: PartitionFilter,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Recordset {
    /// Drop the stream, which will signal the server and close the recordset
    pub fn close(&mut self) {
        self._as = None;
    }

    /// IsActive returns true if the operation hasn't been finished or cancelled.
    #[getter]
    pub fn get_active(&self) -> bool {
        self._as.is_some()
    }

    /// Records is a channel on which the resulting records will be sent back.
    pub fn next(&mut self) -> Option<Result<Record>> {
        let mut pid: Option<usize> = None;
        let mut digest: Option<Vec<u8>> = None;
        let mut bval: Option<i64> = None;
        let mut close: Option<bool> = None;

        let rec = self._as.as_mut().map(|mut stream| {
            let mut client = self.client.lock().unwrap();
            let res = client.next_record(&mut stream);
            match res {
                None => {
                    close = Some(true);
                    None
                }
                Some(Err(pe)) => {
                    let e = format!("{pe}");
                    let error = AerospikeException::new(&e);
                    let _ = throw_object(error.into_zval(true).unwrap());
                    None
                }
                Some(Ok(proto::AerospikeStreamResponse {
                    record: Some(ref rec),
                    bval: bv,
                    ..
                })) => {
                    pid = rec
                        .key
                        .as_ref()
                        .map(|k| Key { _as: k.clone() }.partition_id())?;
                    digest = rec.key.as_ref().map(|k| k.digest.clone())?;
                    bval = bv;

                    Some(Ok(rec.into()))
                }
                Some(Ok(proto::AerospikeStreamResponse {
                    error: Some(ref pe),
                    ..
                })) => {
                    let error: AerospikeException = pe.into();
                    let _ = throw_object(error.into_zval(true).unwrap());
                    None
                }
                _ => None,
            }
        })?;

        // update partition_filter
        pid.map(|pid| {
            let mut p = self.partition_filter._as.lock().unwrap();
            let begin = p.begin as usize;
            let ps = &mut p.partitions[pid - begin];
            ps.bval = bval;
            digest.map(|digest| ps.digest = digest);
        });

        // close the recordset
        close.map(|_| self.close());

        rec
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Bin
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Container object for a record bin, comprising a name and a value.
#[php_class(name = "Aerospike\\Bin")]
#[derive(Debug)]
pub struct Bin {
    _as: proto::Bin,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Bin {
    pub fn __construct(name: &str, value: &Zval) -> PhpResult<Self> {
        let v_op: Option<PHPValue> = from_zval(value);
        match v_op {
            Some(v) => {
                let _as = proto::Bin {
                    name: name.into(),
                    value: Some(v.into()),
                };
                Ok(Bin { _as: _as })
            }
            _ => Err(format!("Invalid input for argument `value`").into()),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
///
///  Record
///
////////////////////////////////////////////////////////////////////////////////////////////

/// Container object for a database record.
#[php_class(name = "Aerospike\\Record")]
pub struct Record {
    _as: proto::Record,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Record {
    /// Bins is the map of requested name/value bins.
    pub fn bin(&self, name: &str) -> Option<PHPValue> {
        let b = self._as.bins.get(name);
        b.map(|v| (*v).clone().into())
    }

    /// Bins is the map of requested name/value bins.
    #[getter]
    pub fn get_bins(&self) -> Option<PHPValue> {
        Some(self._as.bins.clone().into())
    }

    /// Generation shows record modification count.
    #[getter]
    pub fn get_generation(&self) -> Option<u32> {
        Some(self._as.generation)
    }

    /// Expiration is TTL (Time-To-Live).
    /// Number of seconds until record expires.
    #[getter]
    pub fn get_expiration(&self) -> Expiration {
        match self._as.expiration {
            0 => NEVER_EXPIRE.into(),
            secs => secs.into(),
        }
    }

    /// Expiration is TTL (Time-To-Live).
    /// Number of seconds until record expires.
    #[getter]
    pub fn get_ttl(&self) -> Option<u32> {
        match self._as.expiration {
            0 => NEVER_EXPIRE.into(),
            secs => {
                let expiration = CITRUSLEAF_EPOCH + (secs as u64);
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                // Record may not have expired on server, but delay or clock differences may
                // cause it to look expired on client. Floor at 1, not 0, to avoid old
                // "never expires" interpretation.
                if expiration > now {
                    return (((expiration as u64) - now) as u32).into();
                }
                return (1 as u32).into();
            }
        }
    }

    /// Key is the record's key.
    /// Might be empty, or may only consist of digest value.
    #[getter]
    pub fn get_key(&self) -> Option<Key> {
        Some(Key {
            _as: self._as.key.clone()?,
        })
    }
}

impl FromZval<'_> for Record {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Record = zval.extract()?;

        Some(Record { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchPolicy encapsulates parameters for policy attributes used in write operations.
/// This object is passed into methods where database writes can occur.
#[php_class(name = "Aerospike\\BatchPolicy")]
#[derive(Debug)]
pub struct BatchPolicy {
    _as: proto::BatchPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchPolicy {
    pub fn __construct() -> Self {
        BatchPolicy::default()
    }

    // ***************************************************************************
    // ReadPolicy Attrs
    // ***************************************************************************

    #[getter]
    pub fn get_max_retries(&self) -> u32 {
        self._as.policy.as_ref().unwrap().max_retries
    }

    #[setter]
    pub fn set_max_retries(&mut self, max_retries: u32) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.max_retries = max_retries);
    }

    #[getter]
    pub fn get_sleep_multiplier(&self) -> f64 {
        self._as.policy.as_ref().unwrap().sleep_multiplier
    }

    #[setter]
    pub fn set_sleep_multiplier(&mut self, sleep_multiplier: f64) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.sleep_multiplier = sleep_multiplier);
    }

    #[getter]
    pub fn get_total_timeout(&self) -> u64 {
        self._as.policy.as_ref().unwrap().total_timeout
    }

    #[setter]
    pub fn set_total_timeout(&mut self, timeout_millis: u64) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.total_timeout = timeout_millis);
    }

    #[getter]
    pub fn get_socket_timeout(&self) -> u64 {
        self._as.policy.as_ref().unwrap().socket_timeout
    }

    #[setter]
    pub fn set_socket_timeout(&mut self, timeout_millis: u64) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.socket_timeout = timeout_millis);
    }

    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as.policy.as_ref().unwrap().send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.send_key = send_key);
    }

    #[getter]
    pub fn get_use_compression(&self) -> bool {
        self._as.policy.as_ref().unwrap().use_compression
    }

    #[setter]
    pub fn set_use_compression(&mut self, use_compression: bool) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.use_compression = use_compression);
    }

    #[getter]
    pub fn get_exit_fast_on_exhausted_connection_pool(&self) -> bool {
        self._as
            .policy
            .as_ref()
            .unwrap()
            .exit_fast_on_exhausted_connection_pool
    }

    #[setter]
    pub fn set_exit_fast_on_exhausted_connection_pool(
        &mut self,
        exit_fast_on_exhausted_connection_pool: bool,
    ) {
        self._as.policy.as_mut().map(|ref mut p| {
            p.exit_fast_on_exhausted_connection_pool = exit_fast_on_exhausted_connection_pool
        });
    }

    #[getter]
    pub fn get_read_mode_ap(&self) -> ReadModeAP {
        ReadModeAP {
            _as: match self._as.policy.as_ref().unwrap().read_mode_ap {
                0 => proto::ReadModeAp::One,
                1 => proto::ReadModeAp::All,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_ap(&mut self, read_mode_ap: ReadModeAP) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.read_mode_ap = read_mode_ap._as.into());
    }

    #[getter]
    pub fn get_read_mode_sc(&self) -> ReadModeSC {
        ReadModeSC {
            _as: match self._as.policy.as_ref().unwrap().read_mode_sc {
                0 => proto::ReadModeSc::Session,
                1 => proto::ReadModeSc::Linearize,
                2 => proto::ReadModeSc::AllowReplica,
                3 => proto::ReadModeSc::AllowUnavailable,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_sc(&mut self, read_mode_sc: ReadModeSC) {
        self._as
            .policy
            .as_mut()
            .map(|ref mut p| p.read_mode_sc = read_mode_sc._as.into());
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<Expression> {
        self._as
            .policy
            .as_ref()
            .unwrap()
            .filter_expression
            .clone()
            .map(|fe| Expression { _as: fe })
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
        match filter_expression {
            Some(fe) => self
                ._as
                .policy
                .as_mut()
                .map(|ref mut p| p.filter_expression = Some(fe._as)),
            None => self
                ._as
                .policy
                .as_mut()
                .map(|ref mut p| p.filter_expression = None),
        };
    }

    #[getter]
    pub fn get_concurrent_nodes(&self) -> i32 {
        if let Some(nodes) = self._as.concurrent_nodes {
            nodes
        } else {
            1
        }
    }

    #[setter]
    pub fn set_concurrent_nodes(&mut self, concurrent_nodes: i32) {
        self._as.concurrent_nodes = Some(concurrent_nodes);
    }

    #[getter]
    pub fn get_allow_inline(&self) -> bool {
        self._as.allow_inline
    }

    #[setter]
    pub fn set_allow_inline(&mut self, allow_inline: bool) {
        self._as.allow_inline = allow_inline;
    }

    #[getter]
    pub fn get_allow_inline_ssd(&self) -> bool {
        self._as.allow_inline_ssd
    }

    #[setter]
    pub fn set_allow_inline_ssd(&mut self, allow_inline_ssd: bool) {
        self._as.allow_inline_ssd = allow_inline_ssd;
    }

    #[getter]
    pub fn get_respond_all_keys(&self) -> bool {
        self._as.respond_all_keys
    }

    #[setter]
    pub fn set_respond_all_keys(&mut self, respond_all_keys: bool) {
        self._as.respond_all_keys = respond_all_keys
    }

    #[getter]
    pub fn get_allow_partial_results(&self) -> bool {
        self._as.allow_partial_results
    }

    #[setter]
    pub fn set_allow_partial_results(&mut self, allow_partial_results: bool) {
        self._as.respond_all_keys = allow_partial_results
    }
}

impl Default for BatchPolicy {
    fn default() -> Self {
        BatchPolicy {
            _as: proto::BatchPolicy {
                policy: Some(ReadPolicy::default()._as),
                concurrent_nodes: Some(1), // Default concurrent nodes value
                allow_inline: true,        // Default allow inline value
                allow_inline_ssd: false,   // Default allow inline SSD value
                respond_all_keys: true,    // Default respond all keys value
                allow_partial_results: false, // Default allow partial results value
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchReadPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchReadPolicy attributes used in batch read commands.
#[php_class(name = "Aerospike\\BatchReadPolicy")]
pub struct BatchReadPolicy {
    _as: proto::BatchReadPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchReadPolicy {
    pub fn __construct() -> Self {
        BatchReadPolicy::default()
    }

    /// FilterExpression is the optional expression filter. If FilterExpression exists and evaluates to false, the specific batch key
    /// request is not performed and BatchRecord.ResultCode is set to types.FILTERED_OUT.
    ///
    /// Default: null
    #[getter]
    pub fn get_filter_expression(&self) -> Option<Expression> {
        self._as
            .filter_expression
            .clone()
            .map(|fe| Expression { _as: fe })
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }

    /// ReadModeAP indicates read policy for AP (availability) namespaces.
    #[getter]
    pub fn get_read_mode_ap(&self) -> ReadModeAP {
        ReadModeAP {
            _as: match self._as.read_mode_ap {
                0 => proto::ReadModeAp::One,
                1 => proto::ReadModeAp::All,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_ap(&mut self, read_mode_ap: ReadModeAP) {
        self._as.read_mode_ap = read_mode_ap._as.into();
    }

    /// ReadModeSC indicates read policy for SC (strong consistency) namespaces.
    #[getter]
    pub fn get_read_mode_sc(&self) -> ReadModeSC {
        ReadModeSC {
            _as: match self._as.read_mode_ap {
                0 => proto::ReadModeSc::Session,
                1 => proto::ReadModeSc::Linearize,
                2 => proto::ReadModeSc::AllowReplica,
                3 => proto::ReadModeSc::AllowUnavailable,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_read_mode_sc(&mut self, read_mode_sc: ReadModeSC) {
        self._as.read_mode_sc = read_mode_sc._as.into();
    }

    /// ReadTouchTTLPercent determines how record TTL (time to live) is affected on reads. When enabled, the server can
    /// efficiently operate as a read-based LRU cache where the least recently used records are expired.
    /// The value is expressed as a percentage of the TTL sent on the most recent write such that a read
    /// within this interval of the record’s end of life will generate a touch.
    ///
    /// For example, if the most recent write had a TTL of 10 hours and read_touch_ttl_percent is set to
    /// 80, the next read within 8 hours of the record's end of life (equivalent to 2 hours after the most
    /// recent write) will result in a touch, resetting the TTL to another 10 hours.
    ///
    /// Values:
    ///
    /// 0 : Use server config default-read-touch-ttl-pct for the record's namespace/set.
    /// -1 : Do not reset record TTL on reads.
    /// 1 - 100 : Reset record TTL on reads when within this percentage of the most recent write TTL.
    /// Default: 0
    #[getter]
    pub fn get_read_touch_ttl_percent(&self) -> i32 {
        self._as.read_touch_ttl_percent
    }

    #[setter]
    pub fn set_read_touch_ttl_percent(&mut self, percent: i32) {
        self._as.read_touch_ttl_percent = percent;
    }
}

impl Default for BatchReadPolicy {
    fn default() -> Self {
        BatchReadPolicy {
            _as: proto::BatchReadPolicy {
                filter_expression: None,
                read_mode_ap: proto::ReadModeAp::One.into(),
                read_mode_sc: proto::ReadModeSc::Session.into(),
                read_touch_ttl_percent: 0,
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchWritePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchWritePolicy attributes used in batch write commands.
#[php_class(name = "Aerospike\\BatchWritePolicy")]
pub struct BatchWritePolicy {
    _as: proto::BatchWritePolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchWritePolicy {
    pub fn __construct() -> Self {
        BatchWritePolicy::default()
    }

    /// FilterExpression is optional expression filter. If FilterExpression exists and evaluates to false, the specific batch key
    /// request is not performed and BatchRecord#resultCode is set to types.FILTERED_OUT.
    ///
    /// Default: nil
    #[getter]
    pub fn get_filter_expression(&self) -> Option<Expression> {
        self._as
            .filter_expression
            .clone()
            .map(|fe| Expression { _as: fe })
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }

    /// RecordExistsAction qualifies how to handle writes where the record already exists.
    #[getter]
    pub fn get_record_exists_action(&self) -> RecordExistsAction {
        RecordExistsAction {
            _as: match &self._as.record_exists_action {
                0 => proto::RecordExistsAction::Update,
                1 => proto::RecordExistsAction::UpdateOnly,
                2 => proto::RecordExistsAction::Replace,
                3 => proto::RecordExistsAction::ReplaceOnly,
                4 => proto::RecordExistsAction::CreateOnly,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_record_exists_action(&mut self, record_exists_action: RecordExistsAction) {
        self._as.record_exists_action = record_exists_action._as.into();
    }

    /// Desired consistency guarantee when committing a transaction on the server. The default
    /// (COMMIT_ALL) indicates that the server should wait for master and all replica commits to
    /// be successful before returning success to the client.
    ///
    /// Default: CommitLevel.COMMIT_ALL
    #[getter]
    pub fn get_generation_policy(&self) -> GenerationPolicy {
        GenerationPolicy {
            _as: match &self._as.generation_policy {
                0 => proto::GenerationPolicy::None,
                1 => proto::GenerationPolicy::ExpectGenEqual,
                2 => proto::GenerationPolicy::ExpectGenGt,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_generation_policy(&mut self, generation_policy: GenerationPolicy) {
        self._as.generation_policy = generation_policy._as.into();
    }

    /// GenerationPolicy qualifies how to handle record writes based on record generation. The default (NONE)
    /// indicates that the generation is not used to restrict writes.
    ///
    /// The server does not support this field for UDF execute() calls. The read-modify-write
    /// usage model can still be enforced inside the UDF code itself.
    ///
    /// Default: GenerationPolicy.NONE
    /// indicates that the generation is not used to restrict writes.
    #[getter]
    pub fn get_commit_level(&self) -> CommitLevel {
        CommitLevel {
            _as: match &self._as.commit_level {
                0 => proto::CommitLevel::CommitAll,
                1 => proto::CommitLevel::CommitMaster,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_commit_level(&mut self, commit_level: CommitLevel) {
        self._as.commit_level = commit_level._as.into();
    }

    /// Expected generation. Generation is the number of times a record has been modified
    /// (including creation) on the server. If a write operation is creating a record,
    /// the expected generation would be 0. This field is only relevant when
    /// generationPolicy is not NONE.
    ///
    /// The server does not support this field for UDF execute() calls. The read-modify-write
    /// usage model can still be enforced inside the UDF code itself.
    ///
    /// Default: 0
    #[getter]
    pub fn get_generation(&self) -> u32 {
        self._as.generation
    }

    #[setter]
    pub fn set_generation(&mut self, generation: u32) {
        self._as.generation = generation;
    }

    /// Expiration determines record expiration in seconds. Also known as TTL (Time-To-Live).
    /// Seconds record will live before being removed by the server.
    /// Expiration values:
    /// TTLServerDefault (0): Default to namespace configuration variable "default-ttl" on the server.
    /// TTLDontExpire (MaxUint32): Never expire for Aerospike 2 server versions >= 2.7.2 and Aerospike 3+ server
    /// TTLDontUpdate (MaxUint32 - 1): Do not change ttl when record is written. Supported by Aerospike server versions >= 3.10.1
    /// > 0: Actual expiration in seconds.
    #[getter]
    pub fn get_expiration(&self) -> Expiration {
        match self._as.expiration {
            NAMESPACE_DEFAULT => Expiration::Namespace_Default(),
            NEVER_EXPIRE => Expiration::Never(),
            DONT_UPDATE => Expiration::Dont_Update(),
            secs => Expiration::Seconds(secs),
        }
    }

    #[setter]
    pub fn set_expiration(&mut self, expiration: Expiration) {
        self._as.expiration = (&expiration).into();
    }

    /// DurableDelete leaves a tombstone for the record if the transaction results in a record deletion.
    /// This prevents deleted records from reappearing after node failures.
    /// Valid for Aerospike Server Enterprise Edition 3.10+ only.
    #[getter]
    pub fn get_durable_delete(&self) -> bool {
        self._as.durable_delete
    }

    #[setter]
    pub fn set_durable_delete(&mut self, durable_delete: bool) {
        self._as.durable_delete = durable_delete;
    }
}

impl Default for BatchWritePolicy {
    fn default() -> Self {
        BatchWritePolicy {
            _as: proto::BatchWritePolicy {
                filter_expression: None,
                record_exists_action: proto::RecordExistsAction::Update.into(),
                generation_policy: proto::GenerationPolicy::None.into(),
                commit_level: proto::CommitLevel::CommitAll.into(),
                generation: 0,
                expiration: 0,
                durable_delete: false,
                send_key: false,
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchDeletePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchDeletePolicy is used in batch delete commands.
#[php_class(name = "Aerospike\\BatchDeletePolicy")]
pub struct BatchDeletePolicy {
    _as: proto::BatchDeletePolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchDeletePolicy {
    pub fn __construct() -> Self {
        BatchDeletePolicy::default()
    }

    /// FilterExpression is optional expression filter. If FilterExpression exists and evaluates to false, the specific batch key
    /// request is not performed and BatchRecord.ResultCode is set to type.FILTERED_OUT.
    /// Default: nil
    #[getter]
    pub fn get_filter_expression(&self) -> Option<Expression> {
        self._as
            .filter_expression
            .clone()
            .map(|fe| Expression { _as: fe })
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }

    /// Desired consistency guarantee when committing a transaction on the server. The default
    /// (COMMIT_ALL) indicates that the server should wait for master and all replica commits to
    /// be successful before returning success to the client.
    /// Default: CommitLevel.COMMIT_ALL
    #[getter]
    pub fn get_commit_level(&self) -> CommitLevel {
        CommitLevel {
            _as: match &self._as.commit_level {
                0 => proto::CommitLevel::CommitAll,
                1 => proto::CommitLevel::CommitMaster,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_commit_level(&mut self, commit_level: CommitLevel) {
        self._as.commit_level = commit_level._as.into();
    }

    /// Expected generation. Generation is the number of times a record has been modified
    /// (including creation) on the server. This field is only relevant when generationPolicy
    /// is not NONE.
    /// Default: 0
    #[getter]
    pub fn get_generation(&self) -> u32 {
        self._as.generation
    }

    #[setter]
    pub fn set_generation(&mut self, generation: u32) {
        self._as.generation = generation;
    }

    /// If the transaction results in a record deletion, leave a tombstone for the record.
    /// This prevents deleted records from reappearing after node failures.
    /// Valid for Aerospike Server Enterprise Edition only.
    /// Default: false (do not tombstone deleted records).
    #[getter]
    pub fn get_durable_delete(&self) -> bool {
        self._as.durable_delete
    }

    #[setter]
    pub fn set_durable_delete(&mut self, durable_delete: bool) {
        self._as.durable_delete = durable_delete;
    }

    /// Send user defined key in addition to hash digest.
    /// If true, the key will be stored with the tombstone record on the server.
    /// Default: false (do not send the user defined key)
    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as.send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as.send_key = send_key;
    }
}

impl Default for BatchDeletePolicy {
    fn default() -> Self {
        BatchDeletePolicy {
            _as: proto::BatchDeletePolicy {
                filter_expression: None,
                generation_policy: proto::GenerationPolicy::None.into(),
                commit_level: proto::CommitLevel::CommitAll.into(),
                generation: 0,
                durable_delete: false,
                send_key: false,
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchUdfPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchUDFPolicy attributes used in batch UDF execute commands.
#[php_class(name = "Aerospike\\BatchUdfPolicy")]
pub struct BatchUdfPolicy {
    _as: proto::BatchUdfPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchUdfPolicy {
    pub fn __construct() -> Self {
        BatchUdfPolicy::default()
    }

    /// Optional expression filter. If FilterExpression exists and evaluates to false, the specific batch key
    /// request is not performed and BatchRecord.ResultCode is set to types.FILTERED_OUT.
    ///
    /// Default: nil
    #[getter]
    pub fn get_filter_expression(&self) -> Option<Expression> {
        self._as
            .filter_expression
            .clone()
            .map(|fe| Expression { _as: fe })
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }
    /// Desired consistency guarantee when committing a transaction on the server. The default
    /// (COMMIT_ALL) indicates that the server should wait for master and all replica commits to
    /// be successful before returning success to the client.
    ///
    /// Default: CommitLevel.COMMIT_ALL
    #[getter]
    pub fn get_commit_level(&self) -> CommitLevel {
        CommitLevel {
            _as: match &self._as.commit_level {
                0 => proto::CommitLevel::CommitAll,
                1 => proto::CommitLevel::CommitMaster,
                _ => unreachable!(),
            },
        }
    }

    #[setter]
    pub fn set_commit_level(&mut self, commit_level: CommitLevel) {
        self._as.commit_level = commit_level._as.into();
    }

    /// Expiration determines record expiration in seconds. Also known as TTL (Time-To-Live).
    /// Seconds record will live before being removed by the server.
    /// Expiration values:
    /// TTLServerDefault (0): Default to namespace configuration variable "default-ttl" on the server.
    /// TTLDontExpire (MaxUint32): Never expire for Aerospike 2 server versions >= 2.7.2 and Aerospike 3+ server
    /// TTLDontUpdate (MaxUint32 - 1): Do not change ttl when record is written. Supported by Aerospike server versions >= 3.10.1
    /// > 0: Actual expiration in seconds.
    #[getter]
    pub fn get_expiration(&self) -> Expiration {
        match self._as.expiration {
            NAMESPACE_DEFAULT => Expiration::Namespace_Default(),
            NEVER_EXPIRE => Expiration::Never(),
            DONT_UPDATE => Expiration::Dont_Update(),
            secs => Expiration::Seconds(secs),
        }
    }

    #[setter]
    pub fn set_expiration(&mut self, expiration: Expiration) {
        self._as.expiration = (&expiration).into();
    }

    /// DurableDelete leaves a tombstone for the record if the transaction results in a record deletion.
    /// This prevents deleted records from reappearing after node failures.
    /// Valid for Aerospike Server Enterprise Edition 3.10+ only.
    #[getter]
    pub fn get_durable_delete(&self) -> bool {
        self._as.durable_delete
    }

    #[setter]
    pub fn set_durable_delete(&mut self, durable_delete: bool) {
        self._as.durable_delete = durable_delete;
    }

    /// SendKey determines to whether send user defined key in addition to hash digest on both reads and writes.
    /// If the key is sent on a write, the key will be stored with the record on
    /// the server.
    /// The default is to not send the user defined key.
    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as.send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as.send_key = send_key;
    }
}

impl Default for BatchUdfPolicy {
    fn default() -> Self {
        BatchUdfPolicy {
            _as: proto::BatchUdfPolicy {
                filter_expression: None,
                commit_level: proto::CommitLevel::CommitAll.into(),
                expiration: 0,
                durable_delete: false,
                send_key: false,
            },
        }
    }
}

//////////////////////////////////////////////////////////////////////////////////////////
//
//  Operation
//
//////////////////////////////////////////////////////////////////////////////////////////

/// OperationType determines operation type
#[php_class(name = "Aerospike\\Operation")]
pub struct Operation {
    _as: proto::operation::Op,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Operation {
    /// read bin database operation.
    pub fn get(bin_name: Option<String>) -> Self {
        Operation {
            _as: proto::operation::Op::Std(proto::StdOperation {
                op_type: proto::OperationType::Get.into(),
                bin_name: bin_name,
                ..proto::StdOperation::default()
            }),
        }
    }

    /// read record header database operation.
    pub fn get_header() -> Self {
        Operation {
            _as: proto::operation::Op::Std(proto::StdOperation {
                op_type: proto::OperationType::GetHeader.into(),
                ..proto::StdOperation::default()
            }),
        }
    }

    /// set database operation.
    pub fn put(bin: &Bin) -> Self {
        Operation {
            _as: proto::operation::Op::Std(proto::StdOperation {
                op_type: proto::OperationType::Put.into(),
                bin_name: Some(bin._as.name.clone()),
                bin_value: bin._as.value.clone(),
                ..proto::StdOperation::default()
            }),
        }
    }

    /// string append database operation.
    pub fn append(bin: &Bin) -> Self {
        Operation {
            _as: proto::operation::Op::Std(proto::StdOperation {
                op_type: proto::OperationType::Append.into(),
                bin_name: Some(bin._as.name.clone()),
                bin_value: bin._as.value.clone(),
                ..proto::StdOperation::default()
            }),
        }
    }

    /// string prepend database operation.
    pub fn prepend(bin: &Bin) -> Self {
        Operation {
            _as: proto::operation::Op::Std(proto::StdOperation {
                op_type: proto::OperationType::Prepend.into(),
                bin_name: Some(bin._as.name.clone()),
                bin_value: bin._as.value.clone(),
                ..proto::StdOperation::default()
            }),
        }
    }

    /// integer add database operation.
    pub fn add(bin: &Bin) -> Self {
        Operation {
            _as: proto::operation::Op::Std(proto::StdOperation {
                op_type: proto::OperationType::Add.into(),
                bin_name: Some(bin._as.name.clone()),
                bin_value: bin._as.value.clone(),
                ..proto::StdOperation::default()
            }),
        }
    }

    /// touch record database operation.
    pub fn touch() -> Self {
        Operation {
            _as: proto::operation::Op::Std(proto::StdOperation {
                op_type: proto::OperationType::Touch.into(),
                ..proto::StdOperation::default()
            }),
        }
    }

    /// delete record database operation.
    pub fn delete() -> Self {
        Operation {
            _as: proto::operation::Op::Std(proto::StdOperation {
                op_type: proto::OperationType::Delete.into(),
                ..proto::StdOperation::default()
            }),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
///
///  BatchRecord
///
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchRecord encasulates the Batch key and record result.
#[php_class(name = "Aerospike\\BatchRecord")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchRecord {
    _as: proto::BatchRecord,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchRecord {
    /// Key.
    #[getter]
    pub fn get_key(&self) -> Option<Key> {
        Some(Key {
            _as: self._as.key.clone()?,
        })
    }

    /// Record result after batch command has completed.  Will be nil if record was not found
    /// or an error occurred. See ResultCode.
    #[getter]
    pub fn get_record(&self) -> Option<Record> {
        let r: proto::Record = self._as.record.clone()?;
        Some(Record { _as: r })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchRead
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchRead specifies the Key and bin names used in batch read commands
/// where variable bins are needed for each key.
#[php_class(name = "Aerospike\\BatchRead")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchRead {
    _as: proto::BatchRead,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchRead {
    pub fn __construct(policy: &BatchReadPolicy, key: &Key, bins: Option<Vec<String>>) -> Self {
        let read_all_bins = match bins {
            None => false,
            Some(ref l) => l.len() == 0,
        };

        BatchRead {
            _as: proto::BatchRead {
                batch_record: Some(proto::BatchRecord {
                    key: Some(key._as.clone()),
                    record: None,
                    error: None,
                }),
                policy: Some(policy._as.clone()),
                bin_names: bins.clone().unwrap_or(vec![]),
                read_all_bins: read_all_bins,
                ops: vec![],
            },
        }
    }

    /// Optional read policy.
    pub fn ops(policy: &BatchReadPolicy, key: &Key, ops: Vec<&Operation>) -> Self {
        BatchRead {
            _as: proto::BatchRead {
                batch_record: Some(proto::BatchRecord {
                    key: Some(key._as.clone()),
                    record: None,
                    error: None,
                }),
                policy: Some(policy._as.clone()),
                bin_names: vec![],
                read_all_bins: false,
                ops: ops
                    .into_iter()
                    .map(|v| proto::Operation {
                        op: Some(v._as.clone()),
                    })
                    .collect(),
            },
        }
    }

    /// Ops specifies the operations to perform for every key.
    /// Ops are mutually exclusive with BinNames.
    /// A binName can be emulated with `GetOp(binName)`
    /// Supported by server v5.6.0+.
    pub fn header(policy: &BatchReadPolicy, key: &Key) -> Self {
        BatchRead {
            _as: proto::BatchRead {
                batch_record: Some(proto::BatchRecord {
                    key: Some(key._as.clone()),
                    record: None,
                    error: None,
                }),
                policy: Some(policy._as.clone()),
                bin_names: vec![],
                read_all_bins: false,
                ops: vec![],
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchWrite
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchWrite encapsulates a batch key and read/write operations with write policy.
#[php_class(name = "Aerospike\\BatchWrite")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchWrite {
    _as: proto::BatchWrite,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchWrite {
    pub fn __construct(policy: &BatchWritePolicy, key: &Key, ops: Vec<&Operation>) -> Self {
        BatchWrite {
            _as: proto::BatchWrite {
                batch_record: Some(proto::BatchRecord {
                    key: Some(key._as.clone()),
                    record: None,
                    error: None,
                }),
                policy: Some(policy._as.clone()),
                ops: ops
                    .into_iter()
                    .map(|v| proto::Operation {
                        op: Some(v._as.clone()),
                    })
                    .collect(),
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchDelete
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchDelete encapsulates a batch delete operation.
#[php_class(name = "Aerospike\\BatchDelete")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchDelete {
    _as: proto::BatchDelete,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchDelete {
    pub fn __construct(policy: &BatchDeletePolicy, key: &Key) -> Self {
        BatchDelete {
            _as: proto::BatchDelete {
                batch_record: Some(proto::BatchRecord {
                    key: Some(key._as.clone()),
                    record: None,
                    error: None,
                }),
                policy: Some(policy._as.clone()),
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchUdf
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BatchUDF encapsulates a batch user defined function operation.
#[php_class(name = "Aerospike\\BatchUdf")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchUdf {
    _as: proto::BatchUdf,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchUdf {
    pub fn __construct(
        policy: &BatchUdfPolicy,
        key: &Key,
        package_name: String,
        function_name: String,
        function_args: Vec<PHPValue>,
    ) -> Self {
        BatchUdf {
            _as: proto::BatchUdf {
                batch_record: Some(proto::BatchRecord {
                    key: Some(key._as.clone()),
                    record: None,
                    error: None,
                }),
                policy: Some(policy._as.clone()),
                package_name: package_name,
                function_name: function_name,
                function_args: function_args.into_iter().map(|v| v.into()).collect(),
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  UdfLanguage
//
////////////////////////////////////////////////////////////////////////////////////////////

/// `UdfLanguage` determines how to handle record writes based on record generation.
#[php_class(name = "Aerospike\\UdfLanguage")]
pub struct UdfLanguage {
    _as: proto::UdfLanguage,
}

impl FromZval<'_> for UdfLanguage {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &UdfLanguage = zval.extract()?;

        Some(UdfLanguage { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl UdfLanguage {
    /// lua language.
    pub fn Lua() -> Self {
        UdfLanguage {
            _as: proto::UdfLanguage::Lua,
        }
    }
}

impl From<proto::UdfLanguage> for UdfLanguage {
    fn from(input: proto::UdfLanguage) -> Self {
        UdfLanguage { _as: input.clone() }
    }
}

impl From<i32> for UdfLanguage {
    fn from(input: i32) -> Self {
        match input {
            0 => Self::Lua(),
            _ => unreachable!(),
        }
    }
}

impl From<UdfLanguage> for i32 {
    fn from(input: UdfLanguage) -> Self {
        match input._as {
            proto::UdfLanguage::Lua => 0,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  UdfMeta
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Represents UDF (User-Defined Function) metadata for Aerospike.
#[php_class(name = "Aerospike\\UdfMeta")]
#[derive(Debug, PartialEq, Clone)]
pub struct UdfMeta {
    _as: proto::UdfMeta,
}

#[php_impl]
#[derive(ZvalConvert)]
impl UdfMeta {
    /// Getter method to retrieve the package name of the UDF.
    #[getter]
    pub fn get_package_name(&self) -> String {
        self._as.package_name.clone()
    }

    /// Getter method to retrieve the hash of the UDF.
    #[getter]
    pub fn get_hash(&self) -> String {
        self._as.hash.clone()
    }

    /// Getter method to retrieve the language of the UDF.
    #[getter]
    pub fn get_language(&self) -> UdfLanguage {
        self._as.language.into()
    }
}

impl From<&proto::UserRole> for UserRole {
    fn from(input: &proto::UserRole) -> Self {
        UserRole { _as: input.clone() }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  UserRole
//
////////////////////////////////////////////////////////////////////////////////////////////

/// UserRoles contains information about a user.
#[php_class(name = "Aerospike\\UserRole")]
#[derive(Debug, PartialEq, Clone)]
pub struct UserRole {
    _as: proto::UserRole,
}

#[php_impl]
#[derive(ZvalConvert)]
impl UserRole {
    /// User name.
    #[getter]
    pub fn get_user(&self) -> String {
        self._as.user.clone()
    }

    /// Roles is a list of assigned roles.
    #[getter]
    pub fn get_roles(&self) -> Vec<String> {
        self._as.roles.clone()
    }

    /// ReadInfo is the list of read statistics. List may be nil.
    /// Current statistics by offset are:
    ///
    /// 0: read quota in records per second
    /// 1: single record read transaction rate (TPS)
    /// 2: read scan/query record per second rate (RPS)
    /// 3: number of limitless read scans/queries
    ///
    /// Future server releases may add additional statistics.
    #[getter]
    pub fn get_read_info(&self) -> Vec<u64> {
        self._as.read_info.clone().into()
    }

    /// WriteInfo is the list of write statistics. List may be nil.
    /// Current statistics by offset are:
    ///
    /// 0: write quota in records per second
    /// 1: single record write transaction rate (TPS)
    /// 2: write scan/query record per second rate (RPS)
    /// 3: number of limitless write scans/queries
    ///
    /// Future server releases may add additional statistics.
    #[getter]
    pub fn get_write_info(&self) -> Vec<u64> {
        self._as.write_info.clone().into()
    }

    /// ConnsInUse is the number of currently open connections for the user
    #[getter]
    pub fn get_conns_in_use(&self) -> u64 {
        self._as.conns_in_use.into()
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Role
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Role allows granular access to database entities for users.
#[php_class(name = "Aerospike\\Role")]
#[derive(Debug, PartialEq, Clone)]
pub struct Role {
    _as: proto::Role,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Role {
    /// Name is role name
    #[getter]
    pub fn get_name(&self) -> String {
        self._as.name.clone()
    }

    /// Privilege is the list of assigned privileges
    #[getter]
    pub fn get_privileges(&self) -> Vec<Privilege> {
        self._as.privileges.iter().map(|v| v.into()).collect()
    }

    /// While is the list of allowable IP addresses
    #[getter]
    pub fn get_allowlist(&self) -> Vec<String> {
        self._as.allowlist.clone().into()
    }

    /// ReadQuota is the maximum reads per second limit for the role
    #[getter]
    pub fn get_read_quota(&self) -> u64 {
        self._as.read_quota.into()
    }

    /// WriteQuota is the maximum writes per second limit for the role
    #[getter]
    pub fn write_quota(&self) -> u64 {
        self._as.write_quota.into()
    }
}

impl From<&proto::Role> for Role {
    fn from(input: &proto::Role) -> Self {
        Role { _as: input.clone() }
    }
}

impl FromZval<'_> for Role {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Role = zval.extract()?;

        Some(Role { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Privilege
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Privilege determines user access granularity.
#[php_class(name = "Aerospike\\Privilege")]
#[derive(Debug, PartialEq, Clone)]
pub struct Privilege {
    _as: proto::Privilege,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Privilege {
    #[getter]
    pub fn get_name(&self) -> String {
        self._as.name.clone()
    }

    #[getter]
    pub fn get_namespace(&self) -> String {
        self._as.namespace.clone()
    }

    #[getter]
    pub fn get_setname(&self) -> String {
        self._as.set_name.clone()
    }

    /// UserAdmin allows to manages users and their roles.
    pub fn user_admin() -> String {
        "user-admin".into()
    }

    /// SysAdmin allows to manage indexes, user defined functions and server configuration.
    pub fn sys_admin() -> String {
        "sys-admin".into()
    }

    /// DataAdmin allows to manage indicies and user defined functions.
    pub fn data_admin() -> String {
        "data-admin".into()
    }

    /// UDFAdmin allows to manage user defined functions.
    pub fn udf_admin() -> String {
        "udf-admin".into()
    }

    /// SIndexAdmin allows to manage indicies.
    pub fn sindex_admin() -> String {
        "sindex-admin".into()
    }

    /// ReadWriteUDF allows read, write and UDF transactions with the database.
    pub fn read_write_udf() -> String {
        "read-write-udf".into()
    }

    /// ReadWrite allows read and write transactions with the database.
    pub fn read_write() -> String {
        "read-write".into()
    }

    /// Read allows read transactions with the database.
    pub fn read() -> String {
        "read".into()
    }

    /// Write allows write transactions with the database.
    pub fn write() -> String {
        "write".into()
    }

    /// Truncate allow issuing truncate commands.
    pub fn truncate() -> String {
        "truncate".into()
    }
}

impl From<&proto::Privilege> for Privilege {
    fn from(input: &proto::Privilege) -> Self {
        Privilege { _as: input.clone() }
    }
}

impl FromZval<'_> for Privilege {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Privilege = zval.extract()?;

        Some(Privilege { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtListReturnType
//
////////////////////////////////////////////////////////////////////////////////////////////

/// ListReturnType determines the returned values in CDT List operations.
#[php_class(name = "Aerospike\\ListReturnType")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtListReturnType {
    /// _as: proto::CdtListReturnType,
    _as: i32,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtListReturnType {
    /// ListReturnTypeNone will not return a result.
    pub fn None() -> Self {
        Self {
            _as: proto::CdtListReturnType::None.into(),
        }
    }

    /// ListReturnTypeIndex will return index offset order.
    /// 0 = first key
    /// N = Nth key
    /// -1 = last key
    pub fn Index() -> Self {
        Self {
            _as: proto::CdtListReturnType::Index.into(),
        }
    }

    /// ListReturnTypeReverseIndex will return reverse index offset order.
    /// 0 = last key
    /// -1 = first key
    pub fn Reverse_index() -> Self {
        Self {
            _as: proto::CdtListReturnType::ReverseIndex.into(),
        }
    }

    /// ListReturnTypeRank will return value order.
    /// 0 = smallest value
    /// N = Nth smallest value
    /// -1 = largest value
    pub fn Rank() -> Self {
        Self {
            _as: proto::CdtListReturnType::Rank.into(),
        }
    }

    /// ListReturnTypeReverseRank will return reverse value order.
    /// 0 = largest value
    /// N = Nth largest value
    /// -1 = smallest value
    pub fn Reverse_rank() -> Self {
        Self {
            _as: proto::CdtListReturnType::ReverseRank.into(),
        }
    }

    /// ListReturnTypeCount will return count of items selected.
    pub fn Count() -> Self {
        Self {
            _as: proto::CdtListReturnType::Count.into(),
        }
    }

    /// ListReturnTypeValue will return value for single key read and value list for range read.
    pub fn Value() -> Self {
        Self {
            _as: proto::CdtListReturnType::Value.into(),
        }
    }

    /// ListReturnTypeExists returns true if count > 0.
    pub fn Exists() -> Self {
        Self {
            _as: proto::CdtListReturnType::Exists.into(),
        }
    }

    /// ListReturnTypeInverted will invert meaning of list command and return values.  For example:
    /// ListOperation.getByIndexRange(binName, index, count, ListReturnType.INDEX | ListReturnType.INVERTED)
    /// With the INVERTED flag enabled, the items outside of the specified index range will be returned.
    /// The meaning of the list command can also be inverted.  For example:
    /// ListOperation.removeByIndexRange(binName, index, count, ListReturnType.INDEX | ListReturnType.INVERTED);
    /// With the INVERTED flag enabled, the items outside of the specified index range will be removed and returned.
    pub fn Inverted(&self) -> Self {
        Self {
            _as: self._as | 0x10000,
        }
    }
}

impl From<&proto::CdtListReturnType> for CdtListReturnType {
    fn from(input: &proto::CdtListReturnType) -> Self {
        CdtListReturnType {
            _as: (*input).into(),
        }
    }
}

impl FromZval<'_> for CdtListReturnType {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtListReturnType = zval.extract()?;

        Some(CdtListReturnType { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtListWriteFlags
//
////////////////////////////////////////////////////////////////////////////////////////////

/// ListWriteFlags detemines write flags for CDT lists
/// type ListWriteFlags int
#[php_class(name = "Aerospike\\ListWriteFlags")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtListWriteFlags {
    _as: proto::CdtListWriteFlags,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtListWriteFlags {
    /// ListWriteFlagsDefault is the default behavior. It means:  Allow duplicate values and insertions at any index.
    pub fn Default() -> Self {
        Self {
            _as: proto::CdtListWriteFlags::Default,
        }
    }

    /// ListWriteFlagsAddUnique means: Only add unique values.
    pub fn Add_Unique() -> Self {
        Self {
            _as: proto::CdtListWriteFlags::AddUnique,
        }
    }

    /// ListWriteFlagsInsertBounded means: Enforce list boundaries when inserting.  Do not allow values to be inserted
    /// at index outside current list boundaries.
    pub fn Insert_Bounded() -> Self {
        Self {
            _as: proto::CdtListWriteFlags::InsertBounded,
        }
    }

    /// ListWriteFlagsNoFail means: do not raise error if a list item fails due to write flag constraints.
    pub fn No_Fail() -> Self {
        Self {
            _as: proto::CdtListWriteFlags::NoFail,
        }
    }

    /// ListWriteFlagsPartial means: allow other valid list items to be committed if a list item fails due to
    /// write flag constraints.
    pub fn Partial() -> Self {
        Self {
            _as: proto::CdtListWriteFlags::Partial,
        }
    }
}

impl From<&proto::CdtListWriteFlags> for CdtListWriteFlags {
    fn from(input: &proto::CdtListWriteFlags) -> Self {
        CdtListWriteFlags { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtListWriteFlags {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtListWriteFlags = zval.extract()?;

        Some(CdtListWriteFlags { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtListSortFlags
//
////////////////////////////////////////////////////////////////////////////////////////////

// TODO: Check the PHP enum system and see if the current system work for us
// TODO: Add the additional expressions (HLL, BIT, etc.)
// TODO: Add method comments

/// ListOrderType determines the order of returned values in CDT list operations.
#[php_class(name = "Aerospike\\ListSortFlags")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtListSortFlags {
    _as: proto::CdtListSortFlags,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtListSortFlags {
    /// ListSortFlagsDefault is the default sort flag for CDT lists, and sort in Ascending order.
    pub fn Default() -> Self {
        Self {
            _as: proto::CdtListSortFlags::Default,
        }
    }

    /// ListSortFlagsDescending will sort the contents of the list in descending order.
    pub fn Descending() -> Self {
        Self {
            _as: proto::CdtListSortFlags::Descending,
        }
    }

    /// ListSortFlagsDropDuplicates will drop duplicate values in the results of the CDT list operation.
    pub fn Drop_Duplicates() -> Self {
        Self {
            _as: proto::CdtListSortFlags::DropDuplicates,
        }
    }
}

impl From<&proto::CdtListSortFlags> for CdtListSortFlags {
    fn from(input: &proto::CdtListSortFlags) -> Self {
        CdtListSortFlags { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtListSortFlags {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtListSortFlags = zval.extract()?;

        Some(CdtListSortFlags { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtListPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// ListPolicy directives when creating a list and writing list items.
#[php_class(name = "Aerospike\\ListPolicy")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtListPolicy {
    _as: proto::CdtListPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtListPolicy {
    /// NewListPolicy creates a policy with directives when creating a list and writing list items.
    /// Flags are ListWriteFlags. You can specify multiple by `or`ing them together.
    pub fn __construct(order: ListOrderType, flags: Option<Vec<CdtListWriteFlags>>) -> Self {
        let flags: i32 = flags
            .map(|flags| {
                flags.iter().fold(0 as i32, |acc, f| {
                    let f: i32 = f._as.into();
                    acc | f
                })
            })
            .unwrap_or(0);

        CdtListPolicy {
            _as: proto::CdtListPolicy {
                order: order._as.into(),
                flags: flags,
            },
        }
    }
}

impl From<&proto::CdtListPolicy> for CdtListPolicy {
    fn from(input: &proto::CdtListPolicy) -> Self {
        CdtListPolicy { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtListPolicy {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtListPolicy = zval.extract()?;

        Some(CdtListPolicy { _as: f._as.clone() })
    }
}

///////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtListOperation
//
////////////////////////////////////////////////////////////////////////////////////////////

/// List operations support negative indexing.  If the index is negative, the
/// resolved index starts backwards from end of list. If an index is out of bounds,
/// a parameter error will be returned. If a range is partially out of bounds, the
/// valid part of the range will be returned. Index/Range examples:
///
/// Index/Range examples:
///
///    Index 0: First item in list.
///    Index 4: Fifth item in list.
///    Index -1: Last item in list.
///    Index -3: Third to last item in list.
///    Index 1 Count 2: Second and third items in list.
///    Index -3 Count 3: Last three items in list.
///    Index -5 Count 4: Range between fifth to last item to second to last item inclusive.
///
#[php_class(name = "Aerospike\\ListOp")]
pub struct CdtListOperation {
    _as: proto::CdtListOperation,
}

impl FromZval<'_> for CdtListOperation {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtListOperation = zval.extract()?;

        Some(CdtListOperation { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtListOperation {
    /// ListCreateOp creates list create operation.
    /// Server creates list at given context level. The context is allowed to be beyond list
    /// boundaries only if pad is set to true.  In that case, nil list entries will be inserted to
    /// satisfy the context position.
    pub fn create(
        bin_name: String,
        order: ListOrderType,
        pad: bool,
        index: Option<bool>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        let order: i32 = order._as.into();
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Create.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(order as i64).into(),
                    PHPValue::Bool(pad).into(),
                    PHPValue::Bool(index.unwrap_or(false)).into(),
                ],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListSetOrderOp creates a set list order operation.
    /// Server sets list order.  Server returns nil.
    pub fn set_order(
        bin_name: String,
        order: ListOrderType,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        let order: i32 = order._as.into();
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::SetOrder.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(order as i64).into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListAppendOp creates a list append operation.
    /// Server appends values to end of list bin.
    /// Server returns list size on bin name.
    /// It will panic is no values have been passed.
    pub fn append(
        policy: &CdtListPolicy,
        bin_name: String,
        values: Vec<PHPValue>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Append.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::List(values.iter().map(|v| (*v).clone().into()).collect()).into(),
                ],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListInsertOp creates a list insert operation.
    /// Server inserts value to specified index of list bin.
    /// Server returns list size on bin name.
    /// It will panic is no values have been passed.
    pub fn insert(
        policy: &CdtListPolicy,
        bin_name: String,
        index: i64,
        values: Vec<PHPValue>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Insert.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(index).into(),
                    PHPValue::List(values.iter().map(|v| (*v).clone().into()).collect()).into(),
                ],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListPopOp creates list pop operation.
    /// Server returns item at specified index and removes item from list bin.
    pub fn pop(bin_name: String, index: i64, ctx: Option<Vec<&CDTContext>>) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Pop.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListPopRangeOp creates a list pop range operation.
    /// Server returns items starting at specified index and removes items from list bin.
    pub fn pop_range(
        bin_name: String,
        index: i64,
        count: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::PopRange.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into(), PHPValue::Int(count).into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListPopRangeFromOp creates a list pop range operation.
    /// Server returns items starting at specified index to the end of list and removes items from list bin.
    pub fn pop_range_from(
        bin_name: String,
        index: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::PopRangeFrom.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByValueOp creates list remove by value operation.
    /// Server removes the item identified by value and returns removed data specified by returnType.
    pub fn remove_values(
        bin_name: String,
        values: Vec<PHPValue>,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByValueList.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    PHPValue::List(values.iter().map(|v| (*v).clone().into()).collect()).into(),
                ],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByValueRangeOp creates a list remove operation.
    /// Server removes list items identified by value range (valueBegin inclusive, valueEnd exclusive).
    /// If valueBegin is nil, the range is less than valueEnd.
    /// If valueEnd is nil, the range is greater than equal to valueBegin.
    /// Server returns removed data specified by returnType
    pub fn remove_by_value_range(
        bin_name: String,
        begin: PHPValue,
        end: Option<PHPValue>,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        let args = if end.is_some() {
            vec![begin.into(), end.unwrap().into()]
        } else {
            vec![begin.into()]
        };

        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByValueList.into(),
                policy: None,
                bin_name: bin_name,
                args: args,
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByValueRelativeRankRangeOp creates a list remove by value relative to rank range operation.
    /// Server removes list items nearest to value and greater by relative rank.
    /// Server returns removed data specified by returnType.
    ///
    /// Examples for ordered list [0,4,5,9,11,15]:
    ///
    ///	(value,rank) = [removed items]
    ///	(5,0) = [5,9,11,15]
    ///	(5,1) = [9,11,15]
    ///	(5,-1) = [4,5,9,11,15]
    ///	(3,0) = [4,5,9,11,15]
    ///	(3,3) = [11,15]
    ///	(3,-3) = [0,4,5,9,11,15]
    pub fn remove_by_value_relative_rank_range(
        bin_name: String,
        value: PHPValue,
        rank: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByValueRelativeRankRange.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![value.into(), PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByValueRelativeRankRangeCountOp creates a list remove by value relative to rank range operation.
    /// Server removes list items nearest to value and greater by relative rank with a count limit.
    /// Server returns removed data specified by returnType.
    /// Examples for ordered list [0,4,5,9,11,15]:
    ///
    ///	(value,rank,count) = [removed items]
    ///	(5,0,2) = [5,9]
    ///	(5,1,1) = [9]
    ///	(5,-1,2) = [4,5]
    ///	(3,0,1) = [4]
    ///	(3,3,7) = [11,15]
    ///	(3,-3,2) = []
    pub fn remove_by_value_relative_rank_range_count(
        bin_name: String,
        value: PHPValue,
        rank: i64,
        count: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByValueRelativeRankRangeCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    value.into(),
                    PHPValue::Int(rank).into(),
                    PHPValue::Int(count).into(),
                ],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveRangeOp creates a list remove range operation.
    /// Server removes "count" items starting at specified index from list bin.
    /// Server returns number of items removed.
    pub fn remove_range(
        bin_name: String,
        index: i64,
        count: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveRange.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into(), PHPValue::Int(count).into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveRangeFromOp creates a list remove range operation.
    /// Server removes all items starting at specified index to the end of list.
    /// Server returns number of items removed.
    pub fn remove_range_from(
        bin_name: String,
        index: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        // TODO: compare return_type signatures with the java client
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveRangeFrom.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListSetOp creates a list set operation.
    /// Server sets item value at specified index in list bin.
    /// Server does not return a result by default.
    pub fn set(
        bin_name: String,
        index: i64,
        value: PHPValue,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Set.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into(), value.into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListTrimOp creates a list trim operation.
    /// Server removes items in list bin that do not fall into range specified by index
    /// and count range. If the range is out of bounds, then all items will be removed.
    /// Server returns number of elements that were removed.
    pub fn trim(
        bin_name: String,
        index: i64,
        count: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Trim.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into(), PHPValue::Int(count).into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListClearOp creates a list clear operation.
    /// Server removes all items in list bin.
    /// Server does not return a result by default.
    pub fn clear(bin_name: String, ctx: Option<Vec<&CDTContext>>) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Clear.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListIncrementOp creates a list increment operation.
    /// Server increments list[index] by value.
    /// Value should be integer(IntegerValue, LongValue) or float(FloatValue).
    /// Server returns list[index] after incrementing.
    pub fn increment(
        bin_name: String,
        index: i64,
        value: PHPValue,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Increment.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into(), value.into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListSizeOp creates a list size operation.
    /// Server returns size of list on bin name.
    pub fn size(bin_name: String, ctx: Option<Vec<&CDTContext>>) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Size.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListSortOp creates list sort operation.
    /// Server sorts list according to sortFlags.
    /// Server does not return a result by default.
    pub fn sort(
        bin_name: String,
        sort_flags: &CdtListSortFlags,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        let sort_flags: i32 = sort_flags._as.into();
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::Sort.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(sort_flags as i64).into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByIndexOp creates a list remove operation.
    /// Server removes list item identified by index and returns removed data specified by returnType.
    pub fn remove_by_index(
        bin_name: String,
        index: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByIndex.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByIndexRangeOp creates a list remove operation.
    /// Server removes list items starting at specified index to the end of list and returns removed
    /// data specified by returnType.
    pub fn remove_by_index_range(
        bin_name: String,
        index: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByIndexRange.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByIndexRangeCountOp creates a list remove operation.
    /// Server removes "count" list items starting at specified index and returns removed data specified by returnType.
    pub fn remove_by_index_range_count(
        bin_name: String,
        index: i64,
        count: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByIndexRangeCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into(), PHPValue::Int(count).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByRankOp creates a list remove operation.
    /// Server removes list item identified by rank and returns removed data specified by returnType.
    pub fn remove_by_rank(
        bin_name: String,
        rank: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByRank.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByRankRangeOp creates a list remove operation.
    /// Server removes list items starting at specified rank to the last ranked item and returns removed
    /// data specified by returnType.
    pub fn remove_by_rank_range(
        bin_name: String,
        rank: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByRankRange.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListRemoveByRankRangeCountOp creates a list remove operation.
    /// Server removes "count" list items starting at specified rank and returns removed data specified by returnType.
    pub fn remove_by_rank_range_count(
        bin_name: String,
        rank: i64,
        count: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::RemoveByRankRangeCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into(), PHPValue::Int(count).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByValueOp creates a list get by value operation.
    /// Server selects list items identified by value and returns selected data specified by returnType.
    pub fn get_by_values(
        bin_name: String,
        values: Vec<PHPValue>,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByValueList.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    PHPValue::List(values.iter().map(|v| (*v).clone().into()).collect()).into(),
                ],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByValueRangeOp creates a list get by value range operation.
    /// Server selects list items identified by value range (valueBegin inclusive, valueEnd exclusive)
    /// If valueBegin is nil, the range is less than valueEnd.
    /// If valueEnd is nil, the range is greater than equal to valueBegin.
    /// Server returns selected data specified by returnType.
    pub fn get_by_value_range(
        bin_name: String,
        begin: PHPValue,
        end: Option<PHPValue>,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        let args = if end.is_some() {
            vec![begin.into(), end.unwrap().into()]
        } else {
            vec![begin.into()]
        };

        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByValueRange.into(),
                policy: None,
                bin_name: bin_name,
                args: args,
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByIndexOp creates list get by index operation.
    /// Server selects list item identified by index and returns selected data specified by returnType
    pub fn get_by_index(
        bin_name: String,
        index: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByIndex.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByIndexRangeOp creates list get by index range operation.
    /// Server selects list items starting at specified index to the end of list and returns selected
    /// data specified by returnType.
    pub fn get_by_index_range(
        bin_name: String,
        index: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByIndexRange.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByIndexRangeCountOp creates list get by index range operation.
    /// Server selects "count" list items starting at specified index and returns selected data specified
    /// by returnType.
    pub fn get_by_index_range_count(
        bin_name: String,
        index: i64,
        count: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByIndexRangeCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into(), PHPValue::Int(count).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByRankOp creates a list get by rank operation.
    /// Server selects list item identified by rank and returns selected data specified by returnType.
    pub fn get_by_rank(
        bin_name: String,
        rank: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByRank.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByRankRangeOp creates a list get by rank range operation.
    /// Server selects list items starting at specified rank to the last ranked item and returns selected
    /// data specified by returnType
    pub fn get_by_rank_range(
        bin_name: String,
        rank: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByRankRange.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByRankRangeCountOp creates a list get by rank range operation.
    /// Server selects "count" list items starting at specified rank and returns selected data specified by returnType.
    pub fn get_by_rank_range_count(
        bin_name: String,
        rank: i64,
        count: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByRankRangeCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into(), PHPValue::Int(count).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByValueRelativeRankRangeOp creates a list get by value relative to rank range operation.
    /// Server selects list items nearest to value and greater by relative rank.
    /// Server returns selected data specified by returnType.
    ///
    /// Examples for ordered list [0,4,5,9,11,15]:
    ///
    ///	(value,rank) = [selected items]
    ///	(5,0) = [5,9,11,15]
    ///	(5,1) = [9,11,15]
    ///	(5,-1) = [4,5,9,11,15]
    ///	(3,0) = [4,5,9,11,15]
    ///	(3,3) = [11,15]
    ///	(3,-3) = [0,4,5,9,11,15]
    pub fn get_by_value_relative_rank_range(
        bin_name: String,
        value: PHPValue,
        rank: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByValueRelativeRankRange.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![value.into(), PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// ListGetByValueRelativeRankRangeCountOp creates a list get by value relative to rank range operation.
    /// Server selects list items nearest to value and greater by relative rank with a count limit.
    /// Server returns selected data specified by returnType.
    ///
    /// Examples for ordered list [0,4,5,9,11,15]:
    ///
    ///	(value,rank,count) = [selected items]
    ///	(5,0,2) = [5,9]
    ///	(5,1,1) = [9]
    ///	(5,-1,2) = [4,5]
    ///	(3,0,1) = [4]
    ///	(3,3,7) = [11,15]
    ///	(3,-3,2) = []
    pub fn get_by_value_relative_rank_range_count(
        bin_name: String,
        value: PHPValue,
        rank: i64,
        count: i64,
        return_type: Option<CdtListReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::List(proto::CdtListOperation {
                op: proto::CdtListCommandOp::GetByValueRelativeRankRangeCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    value.into(),
                    PHPValue::Int(rank).into(),
                    PHPValue::Int(count).into(),
                ],
                return_type: return_type
                    .map(|rt| rt._as.into())
                    .unwrap_or(Some(proto::CdtListReturnType::Value.into())),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtMapReturnType
//
////////////////////////////////////////////////////////////////////////////////////////////

/// MapReturnType defines the map return type.
/// Type of data to return when selecting or removing items from the map.
#[php_class(name = "Aerospike\\MapReturnType")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtMapReturnType {
    /// _as: proto::CdtMapReturnType,
    _as: i32,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtMapReturnType {
    /// NONE will will not return a result.
    pub fn None() -> Self {
        Self {
            _as: proto::CdtMapReturnType::None.into(),
        }
    }

    /// INDEX will return key index order.
    ///
    /// 0 = first key
    /// N = Nth key
    /// -1 = last key
    pub fn Index() -> Self {
        Self {
            _as: proto::CdtMapReturnType::Index.into(),
        }
    }

    /// REVERSE_INDEX will return reverse key order.
    ///
    /// 0 = last key
    /// -1 = first key
    pub fn Reverse_Index() -> Self {
        Self {
            _as: proto::CdtMapReturnType::ReverseIndex.into(),
        }
    }

    /// RANK will return value order.
    ///
    /// 0 = smallest value
    /// N = Nth smallest value
    /// -1 = largest value
    pub fn Rank() -> Self {
        Self {
            _as: proto::CdtMapReturnType::Rank.into(),
        }
    }

    /// REVERSE_RANK will return reverse value order.
    ///
    /// 0 = largest value
    /// N = Nth largest value
    /// -1 = smallest value
    pub fn Reverse_Rank() -> Self {
        Self {
            _as: proto::CdtMapReturnType::ReverseRank.into(),
        }
    }

    /// COUNT will return count of items selected.
    pub fn Count() -> Self {
        Self {
            _as: proto::CdtMapReturnType::Count.into(),
        }
    }

    /// KEY will return key for single key read and key list for range read.
    pub fn Key() -> Self {
        Self {
            _as: proto::CdtMapReturnType::Key.into(),
        }
    }

    /// VALUE will return value for single key read and value list for range read.
    pub fn Value() -> Self {
        Self {
            _as: proto::CdtMapReturnType::Value.into(),
        }
    }

    /// KEY_VALUE will return key/value items. The possible return types are:
    ///
    /// map[interface{}]interface{} : Returned for unordered maps
    /// []MapPair : Returned for range results where range order needs to be preserved.
    pub fn Key_Value() -> Self {
        Self {
            _as: proto::CdtMapReturnType::KeyValue.into(),
        }
    }

    /// EXISTS returns true if count > 0.
    pub fn Exists() -> Self {
        Self {
            _as: proto::CdtMapReturnType::Exists.into(),
        }
    }

    /// UNORDERED_MAP returns an unordered map.
    pub fn Unordered_Map() -> Self {
        Self {
            _as: proto::CdtMapReturnType::UnorderedMap.into(),
        }
    }

    /// ORDERED_MAP returns an ordered map.
    pub fn Ordered_Map() -> Self {
        Self {
            _as: proto::CdtMapReturnType::OrderedMap.into(),
        }
    }

    /// INVERTED will invert meaning of map command and return values.  For example:
    /// MapRemoveByKeyRange(binName, keyBegin, keyEnd, MapReturnType.KEY | MapReturnType.INVERTED)
    /// With the INVERTED flag enabled, the keys outside of the specified key range will be removed and returned.
    pub fn Inverted(&self) -> Self {
        Self {
            _as: self._as | 0x10000,
        }
    }
}

impl From<&proto::CdtMapReturnType> for CdtMapReturnType {
    fn from(input: &proto::CdtMapReturnType) -> Self {
        CdtMapReturnType {
            _as: (*input).into(),
        }
    }
}

impl FromZval<'_> for CdtMapReturnType {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtMapReturnType = zval.extract()?;

        Some(CdtMapReturnType { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtMapWriteMode
//
////////////////////////////////////////////////////////////////////////////////////////////

/// MapWriteMode should only be used for server versions < 4.3.
/// MapWriteFlags are recommended for server versions >= 4.3.
#[php_class(name = "Aerospike\\MapWriteMode")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtMapWriteMode {
    _as: proto::CdtMapWriteMode,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtMapWriteMode {
    /// If the key already exists, the item will be overwritten.
    /// If the key does not exist, a new item will be created.
    pub fn Update() -> Self {
        Self {
            _as: proto::CdtMapWriteMode::Update,
        }
    }

    /// If the key already exists, the item will be overwritten.
    /// If the key does not exist, the write will fail.
    pub fn Update_Only() -> Self {
        Self {
            _as: proto::CdtMapWriteMode::UpdateOnly,
        }
    }

    /// If the key already exists, the write will fail.
    /// If the key does not exist, a new item will be created.
    pub fn Create_Only() -> Self {
        Self {
            _as: proto::CdtMapWriteMode::CreateOnly,
        }
    }
}

impl From<&proto::CdtMapWriteMode> for CdtMapWriteMode {
    fn from(input: &proto::CdtMapWriteMode) -> Self {
        CdtMapWriteMode { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtMapWriteMode {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtMapWriteMode = zval.extract()?;

        Some(CdtMapWriteMode { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtMapWriteFlags
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Map write bit flags.
/// Requires server versions >= 4.3.
#[php_class(name = "Aerospike\\MapWriteFlags")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtMapWriteFlags {
    _as: proto::CdtMapWriteFlags,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtMapWriteFlags {
    /// MapWriteFlagsDefault is the Default. Allow create or update.
    pub fn Default() -> Self {
        Self {
            _as: proto::CdtMapWriteFlags::Default,
        }
    }

    /// MapWriteFlagsCreateOnly means: If the key already exists, the item will be denied.
    /// If the key does not exist, a new item will be created.
    pub fn Create_Only() -> Self {
        Self {
            _as: proto::CdtMapWriteFlags::CreateOnly,
        }
    }

    /// MapWriteFlagsUpdateOnly means: If the key already exists, the item will be overwritten.
    /// If the key does not exist, the item will be denied.
    pub fn Update_Only() -> Self {
        Self {
            _as: proto::CdtMapWriteFlags::UpdateOnly,
        }
    }

    /// MapWriteFlagsNoFail means: Do not raise error if a map item is denied due to write flag constraints.
    pub fn No_Fail() -> Self {
        Self {
            _as: proto::CdtMapWriteFlags::NoFail,
        }
    }

    /// MapWriteFlagsNoFail means: Allow other valid map items to be committed if a map item is denied due to
    /// write flag constraints.
    pub fn Partial() -> Self {
        Self {
            _as: proto::CdtMapWriteFlags::Partial,
        }
    }
}

impl From<&proto::CdtMapWriteFlags> for CdtMapWriteFlags {
    fn from(input: &proto::CdtMapWriteFlags) -> Self {
        CdtMapWriteFlags { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtMapWriteFlags {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtMapWriteFlags = zval.extract()?;

        Some(CdtMapWriteFlags { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtMapPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// MapPolicy directives when creating a map and writing map items.
#[php_class(name = "Aerospike\\MapPolicy")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtMapPolicy {
    _as: proto::CdtMapPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtMapPolicy {
    /// NewMapPolicy creates a MapPolicy with WriteMode. Use with servers before v4.3.
    pub fn __construct(
        order: &MapOrderType,
        flags: Option<Vec<&CdtMapWriteFlags>>,
        persisted_index: Option<bool>,
    ) -> Self {
        let flags: i32 = flags
            .unwrap_or(vec![])
            .into_iter()
            .fold(0 as i32, |acc, f| {
                let f: i32 = f._as.into();
                acc | f
            });

        Self {
            _as: proto::CdtMapPolicy {
                map_order: order._as.into(),
                flags: flags,
                persisted_index: persisted_index.unwrap_or(false),
            },
        }
    }
}

impl From<&proto::CdtMapPolicy> for CdtMapPolicy {
    fn from(input: &proto::CdtMapPolicy) -> Self {
        CdtMapPolicy { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtMapPolicy {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtMapPolicy = zval.extract()?;

        Some(CdtMapPolicy { _as: f._as.clone() })
    }
}

impl Default for CdtMapPolicy {
    fn default() -> Self {
        CdtMapPolicy {
            _as: proto::CdtMapPolicy {
                map_order: proto::MapOrderType::Unordered.into(),
                flags: proto::CdtMapWriteFlags::Default.into(),
                persisted_index: false,
            },
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtMapOperation
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Unique key map bin operations. Create map operations used by the client operate command.
/// The default unique key map is unordered.
///
/// All maps maintain an index and a rank.  The index is the item offset from the start of the map,
/// for both unordered and ordered maps.  The rank is the sorted index of the value component.
/// Map supports negative indexing for index and rank.
///
/// Index examples:
///
///  Index 0: First item in map.
///  Index 4: Fifth item in map.
///  Index -1: Last item in map.
///  Index -3: Third to last item in map.
///  Index 1 Count 2: Second and third items in map.
///  Index -3 Count 3: Last three items in map.
///  Index -5 Count 4: Range between fifth to last item to second to last item inclusive.
///
///
/// Rank examples:
///
///  Rank 0: Item with lowest value rank in map.
///  Rank 4: Fifth lowest ranked item in map.
///  Rank -1: Item with highest ranked value in map.
///  Rank -3: Item with third highest ranked value in map.
///  Rank 1 Count 2: Second and third lowest ranked items in map.
///  Rank -3 Count 3: Top three ranked items in map.
///
///
/// Nested CDT operations are supported by optional CTX context arguments.  Examples:
///
///  bin = {key1:{key11:9,key12:4}, key2:{key21:3,key22:5}}
///  Set map value to 11 for map key "key21" inside of map key "key2".
///  MapOperation.put(MapPolicy.Default, "bin", StringValue("key21"), IntegerValue(11), CtxMapKey(StringValue("key2")))
///  bin result = {key1:{key11:9,key12:4},key2:{key21:11,key22:5}}
///
///  bin : {key1:{key11:{key111:1},key12:{key121:5}}, key2:{key21:{"key211":7}}}
///  Set map value to 11 in map key "key121" for highest ranked map ("key12") inside of map key "key1".
///  MapPutOp(DefaultMapPolicy(), "bin", StringValue("key121"), IntegerValue(11), CtxMapKey(StringValue("key1")), CtxMapRank(-1))
///  bin result = {key1:{key11:{key111:1},key12:{key121:11}}, key2:{key21:{"key211":7}}}

#[php_class(name = "Aerospike\\MapOp")]
pub struct CdtMapOperation {
    _as: proto::CdtMapOperation,
}

impl FromZval<'_> for CdtMapOperation {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtMapOperation = zval.extract()?;

        Some(CdtMapOperation { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtMapOperation {
    /// MapCreateOp creates a map create operation.
    /// Server creates map at given context level.
    pub fn create(
        bin_name: String,
        order: &MapOrderType,
        with_index: Option<bool>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::Create.into(),
                policy: Some(CdtMapPolicy::__construct(order, None, with_index)._as),
                bin_name: bin_name,
                args: vec![],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapSetPolicyOp creates set map policy operation.
    /// Server sets map policy attributes.  Server returns nil.
    ///
    /// The required map policy attributes can be changed after the map is created.
    pub fn set_policy(
        policy: &CdtMapPolicy,
        bin_name: String,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::SetPolicy.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapSizeOp creates map size operation.
    /// Server returns size of map.
    pub fn size(bin_name: String, ctx: Option<Vec<&CDTContext>>) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::SetPolicy.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapPutOp creates map put operation.
    /// Server writes key/value item to map bin and returns map size.
    ///
    /// The required map policy dictates the type of map to create when it does not exist.
    /// The map policy also specifies the mode used when writing items to the map.
    pub fn put(
        policy: &CdtMapPolicy,
        bin_name: String,
        map: PHPValue,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Option<Operation> {
        if !assert_map(&map) {
            return None;
        }

        Some(Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::PutItems.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![map.into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        })
    }

    /// MapIncrementOp creates map increment operation.
    /// Server increments values by incr for all items identified by key and returns final result.
    /// Valid only for numbers.
    ///
    /// The required map policy dictates the type of map to create when it does not exist.
    /// The map policy also specifies the mode used when writing items to the map.
    pub fn increment(
        policy: &CdtMapPolicy,
        bin_name: String,
        key: PHPValue,
        incr: PHPValue,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::Increment.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![key.into(), incr.into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapDecrementOp creates map decrement operation.
    /// Server decrements values by decr for all items identified by key and returns final result.
    /// Valid only for numbers.
    ///
    /// The required map policy dictates the type of map to create when it does not exist.
    /// The map policy also specifies the mode used when writing items to the map.
    pub fn decrement(
        policy: &CdtMapPolicy,
        bin_name: String,
        key: PHPValue,
        decr: PHPValue,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::Decrement.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![key.into(), decr.into()],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapClearOp creates map clear operation.
    /// Server removes all items in map.  Server returns nil.
    pub fn clear(bin_name: String, ctx: Option<Vec<&CDTContext>>) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::Clear.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![],
                return_type: None,
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByKeyOp creates map remove operation.
    /// Server removes map item identified by key and returns removed data specified by returnType.
    pub fn remove_by_keys(
        bin_name: String,
        keys: Vec<PHPValue>,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByKeyList.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::List(keys.iter().map(|k| k.clone().into()).collect()).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByKeyRangeOp creates map remove operation.
    /// Server removes map items identified by key range (keyBegin inclusive, keyEnd exclusive).
    /// If keyBegin is nil, the range is less than keyEnd.
    /// If keyEnd is nil, the range is greater than equal to keyBegin.
    ///
    /// Server returns removed data specified by returnType.
    pub fn remove_by_key_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        begin: PHPValue,
        end: PHPValue,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByKeyRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![begin.into(), end.into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByValueOp creates map remove operation.
    /// Server removes map items identified by value and returns removed data specified by returnType.
    pub fn remove_by_values(
        policy: &CdtMapPolicy,
        bin_name: String,
        values: Vec<PHPValue>,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByValueList.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::List(values.iter().map(|v| (*v).clone().into()).collect()).into(),
                ],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByValueListOp creates map remove operation.
    /// Server removes map items identified by values and returns removed data specified by returnType.
    pub fn remove_by_value_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        begin: PHPValue,
        end: PHPValue,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByValueRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![begin.into(), end.into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByValueRelativeRankRangeOp creates a map remove by value relative to rank range operation.
    /// Server removes map items nearest to value and greater by relative rank.
    /// Server returns removed data specified by returnType.
    ///
    /// Examples for map [{4=2},{9=10},{5=15},{0=17}]:
    ///
    ///	(value,rank) = [removed items]
    ///	(11,1) = [{0=17}]
    ///	(11,-1) = [{9=10},{5=15},{0=17}]
    pub fn remove_by_value_relative_rank_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        value: PHPValue,
        rank: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByValueRelativeRankRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![value.into(), PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByValueRelativeRankRangeCountOp creates a map remove by value relative to rank range operation.
    /// Server removes map items nearest to value and greater by relative rank with a count limit.
    /// Server returns removed data specified by returnType (See MapReturnType).
    ///
    /// Examples for map [{4=2},{9=10},{5=15},{0=17}]:
    ///
    ///	(value,rank,count) = [removed items]
    ///	(11,1,1) = [{0=17}]
    ///	(11,-1,1) = [{9=10}]
    pub fn remove_by_value_relative_rank_range_count(
        policy: &CdtMapPolicy,
        bin_name: String,
        value: PHPValue,
        rank: i64,
        count: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByValueRelativeRankRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    value.into(),
                    PHPValue::Int(rank).into(),
                    PHPValue::Int(count).into(),
                ],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByIndexOp creates map remove operation.
    /// Server removes map item identified by index and returns removed data specified by returnType.
    pub fn remove_by_index(
        policy: &CdtMapPolicy,
        bin_name: String,
        index: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByIndex.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByIndexRangeOp creates map remove operation.
    /// Server removes map items starting at specified index to the end of map and returns removed
    /// data specified by returnTyp
    pub fn remove_by_index_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        index: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByIndexRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByIndexRangeCountOp creates map remove operation.
    /// Server removes "count" map items starting at specified index and returns removed data specified by returnType.
    pub fn remove_by_index_range_count(
        policy: &CdtMapPolicy,
        bin_name: String,
        index: i64,
        count: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByIndexRangeCount.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into(), PHPValue::Int(count).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByRankOp creates map remove operation.
    /// Server removes map item identified by rank and returns removed data specified by returnType.
    pub fn remove_by_rank(
        policy: &CdtMapPolicy,
        bin_name: String,
        rank: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByRank.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByRankRangeOp creates map remove operation.
    /// Server removes map items starting at specified rank to the last ranked item and returns removed
    /// data specified by returnType.
    pub fn remove_by_rank_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        rank: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByRankRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByRankRangeCountOp creates map remove operation.
    /// Server removes "count" map items starting at specified rank and returns removed data specified by returnType.
    pub fn remove_by_rank_range_count(
        policy: &CdtMapPolicy,
        bin_name: String,
        rank: i64,
        count: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByRankRangeCount.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into(), PHPValue::Int(count).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByKeyRelativeIndexRangeOp creates a map remove by key relative to index range operation.
    /// Server removes map items nearest to key and greater by index.
    /// Server returns removed data specified by returnType.
    ///
    /// Examples for map [{0=17},{4=2},{5=15},{9=10}]:
    ///
    ///	(value,index) = [removed items]
    ///	(5,0) = [{5=15},{9=10}]
    ///	(5,1) = [{9=10}]
    ///	(5,-1) = [{4=2},{5=15},{9=10}]
    ///	(3,2) = [{9=10}]
    ///	(3,-2) = [{0=17},{4=2},{5=15},{9=10}]
    pub fn remove_by_key_relative_index_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        key: PHPValue,
        index: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByKeyRelativeIndexRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![key.into(), PHPValue::Int(index).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapRemoveByKeyRelativeIndexRangeCountOp creates map remove by key relative to index range operation.
    /// Server removes map items nearest to key and greater by index with a count limit.
    /// Server returns removed data specified by returnType.
    ///
    /// Examples for map [{0=17},{4=2},{5=15},{9=10}]:
    ///
    ///	(value,index,count) = [removed items]
    ///	(5,0,1) = [{5=15}]
    ///	(5,1,2) = [{9=10}]
    ///	(5,-1,1) = [{4=2}]
    ///	(3,2,1) = [{9=10}]
    ///	(3,-2,2) = [{0=17}]
    pub fn remove_by_key_relative_index_range_count(
        policy: &CdtMapPolicy,
        bin_name: String,
        key: PHPValue,
        index: i64,
        count: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::RemoveByKeyRelativeIndexRangeCount.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    key.into(),
                    PHPValue::Int(index).into(),
                    PHPValue::Int(count).into(),
                ],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByKeyOp creates map get by key operation.
    /// Server selects map item identified by key and returns selected data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_keys(
        policy: &CdtMapPolicy,
        bin_name: String,
        keys: Vec<PHPValue>,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByKeyList.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::List(keys.iter().map(|key| key.clone().into()).collect()).into(),
                ],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByKeyRangeOp creates map get by key range operation.
    /// Server selects map items identified by key range (keyBegin inclusive, keyEnd exclusive).
    /// If keyBegin is nil, the range is less than keyEnd.
    /// If keyEnd is nil, the range is greater than equal to keyBegin.
    ///
    /// Server returns selected data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_key_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        begin: PHPValue,
        end: PHPValue,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByKeyRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![begin.into(), end.into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByKeyRelativeIndexRangeOp creates a map get by key relative to index range operation.
    /// Server selects map items nearest to key and greater by index.
    /// Server returns selected data specified by returnType.
    ///
    /// Examples for ordered map [{0=17},{4=2},{5=15},{9=10}]:
    ///
    ///	(value,index) = [selected items]
    ///	(5,0) = [{5=15},{9=10}]
    ///	(5,1) = [{9=10}]
    ///	(5,-1) = [{4=2},{5=15},{9=10}]
    ///	(3,2) = [{9=10}]
    ///	(3,-2) = [{0=17},{4=2},{5=15},{9=10}]
    /// Should be used with BatchRead.
    pub fn get_by_key_relative_index_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        key: PHPValue,
        index: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByKeyRelativeIndexRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![key.into(), PHPValue::Int(index).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByKeyRelativeIndexRangeCountOp creates a map get by key relative to index range operation.
    /// Server selects map items nearest to key and greater by index with a count limit.
    /// Server returns selected data specified by returnType (See MapReturnType).
    ///
    /// Examples for ordered map [{0=17},{4=2},{5=15},{9=10}]:
    ///
    ///	(value,index,count) = [selected items]
    ///	(5,0,1) = [{5=15}]
    ///	(5,1,2) = [{9=10}]
    ///	(5,-1,1) = [{4=2}]
    ///	(3,2,1) = [{9=10}]
    ///	(3,-2,2) = [{0=17}]
    /// Should be used with BatchRead.
    pub fn get_by_key_relative_index_range_count(
        policy: &CdtMapPolicy,
        bin_name: String,
        key: PHPValue,
        index: i64,
        count: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByKeyRelativeIndexRangeCount.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    key.into(),
                    PHPValue::Int(index).into(),
                    PHPValue::Int(count).into(),
                ],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByKeyListOp creates a map get by key list operation.
    /// Server selects map items identified by keys and returns selected data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_values(
        policy: &CdtMapPolicy,
        bin_name: String,
        values: Vec<PHPValue>,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByValueList.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::List(values.iter().map(|v| (*v).clone().into()).collect()).into(),
                ],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByValueRangeOp creates map get by value range operation.
    /// Server selects map items identified by value range (valueBegin inclusive, valueEnd exclusive)
    /// If valueBegin is nil, the range is less than valueEnd.
    /// If valueEnd is nil, the range is greater than equal to valueBegin.
    ///
    /// Server returns selected data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_value_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        begin: PHPValue,
        end: PHPValue,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByValueRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![begin.into(), end.into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByValueRelativeRankRangeOp creates a map get by value relative to rank range operation.
    /// Server selects map items nearest to value and greater by relative rank.
    /// Server returns selected data specified by returnType.
    ///
    /// Examples for map [{4=2},{9=10},{5=15},{0=17}]:
    ///
    ///	(value,rank) = [selected items]
    ///	(11,1) = [{0=17}]
    ///	(11,-1) = [{9=10},{5=15},{0=17}]
    /// Should be used with BatchRead.
    pub fn get_by_value_relative_rank_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        value: PHPValue,
        rank: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByValueRelativeRankRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![value.into(), PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByValueRelativeRankRangeCountOp creates a map get by value relative to rank range operation.
    /// Server selects map items nearest to value and greater by relative rank with a count limit.
    /// Server returns selected data specified by returnType.
    ///
    /// Examples for map [{4=2},{9=10},{5=15},{0=17}]:
    ///
    ///	(value,rank,count) = [selected items]
    ///	(11,1,1) = [{0=17}]
    ///	(11,-1,1) = [{9=10}]
    /// Should be used with BatchRead.
    pub fn get_by_value_relative_rank_range_count(
        policy: &CdtMapPolicy,
        bin_name: String,
        value: PHPValue,
        rank: i64,
        count: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByValueRelativeRankRangeCount.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    value.into(),
                    PHPValue::Int(rank).into(),
                    PHPValue::Int(count).into(),
                ],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByIndexOp creates map get by index operation.
    /// Server selects map item identified by index and returns selected data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_index(
        policy: &CdtMapPolicy,
        bin_name: String,
        index: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByIndex.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![PHPValue::Int(index).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByIndexRangeOp creates map get by index range operation.
    /// Server selects map items starting at specified index to the end of map and returns selected
    /// data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_index_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        begin: PHPValue,
        end: PHPValue,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByIndexRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![begin.into(), end.into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByIndexRangeCountOp creates map get by index range operation.
    /// Server selects "count" map items starting at specified index and returns selected data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_index_range_count(
        policy: &CdtMapPolicy,
        bin_name: String,
        index: i64,
        rank: i64,
        count: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByIndexRangeCount.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(index).into(),
                    PHPValue::Int(rank).into(),
                    PHPValue::Int(count).into(),
                ],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByRankOp creates map get by rank operation.
    /// Server selects map item identified by rank and returns selected data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_rank(
        policy: &CdtMapPolicy,
        bin_name: String,
        rank: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByRank.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![PHPValue::Int(rank).into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByRankRangeOp creates map get by rank range operation.
    /// Server selects map items starting at specified rank to the last ranked item and returns selected
    /// data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_rank_range(
        policy: &CdtMapPolicy,
        bin_name: String,
        begin: PHPValue,
        end: PHPValue,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByRankRange.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![begin.into(), end.into()],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// MapGetByRankRangeCountOp creates map get by rank range operation.
    /// Server selects "count" map items starting at specified rank and returns selected data specified by returnType.
    /// Should be used with BatchRead.
    pub fn get_by_rank_range_count(
        policy: &CdtMapPolicy,
        bin_name: String,
        rank: i64,
        range: i64,
        count: i64,
        return_type: Option<CdtMapReturnType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Map(proto::CdtMapOperation {
                op: proto::CdtMapCommandOp::GetByRankRangeCount.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(rank).into(),
                    PHPValue::Int(range).into(),
                    PHPValue::Int(count).into(),
                ],
                return_type: return_type
                    .map(|v| v._as)
                    .or(Some(CdtMapReturnType::Key_Value()._as)),
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtHllWriteFlags
//
////////////////////////////////////////////////////////////////////////////////////////////

/// HLLWriteFlags specifies the HLL write operation flags.
#[php_class(name = "Aerospike\\HllWriteFlags")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtHllWriteFlags {
    _as: proto::CdtHllWriteFlags,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtHllWriteFlags {
    /// HLLWriteFlagsDefault is Default. Allow create or update.
    pub fn Default() -> Self {
        Self {
            _as: proto::CdtHllWriteFlags::Default,
        }
    }

    /// HLLWriteFlagsCreateOnly behaves like the following:
    /// If the bin already exists, the operation will be denied.
    /// If the bin does not exist, a new bin will be created.
    pub fn Create_Only() -> Self {
        Self {
            _as: proto::CdtHllWriteFlags::CreateOnly,
        }
    }

    /// HLLWriteFlagsUpdateOnly behaves like the following:
    /// If the bin already exists, the bin will be overwritten.
    /// If the bin does not exist, the operation will be denied.
    pub fn Update_Only() -> Self {
        Self {
            _as: proto::CdtHllWriteFlags::UpdateOnly,
        }
    }

    /// HLLWriteFlagsNoFail does not raise error if operation is denied.
    pub fn No_Fail() -> Self {
        Self {
            _as: proto::CdtHllWriteFlags::NoFail,
        }
    }

    /// HLLWriteFlagsAllowFold allows the resulting set to be the minimum of provided index bits.
    /// Also, allow the usage of less precise HLL algorithms when minHash bits
    /// of all participating sets do not match.
    pub fn Allow_Fold() -> Self {
        Self {
            _as: proto::CdtHllWriteFlags::AllowFold,
        }
    }
}

impl From<&proto::CdtHllWriteFlags> for CdtHllWriteFlags {
    fn from(input: &proto::CdtHllWriteFlags) -> Self {
        CdtHllWriteFlags { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtHllWriteFlags {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtHllWriteFlags = zval.extract()?;

        Some(CdtHllWriteFlags { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtHllPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// HLLPolicy determines the HyperLogLog operation policy.
#[php_class(name = "Aerospike\\HllPolicy")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtHllPolicy {
    _as: proto::CdtHllPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtHllPolicy {
    /// new HLLPolicy uses specified optional HLLWriteFlags when performing HLL operations.
    pub fn __construct(flags: Option<CdtListWriteFlags>) -> Self {
        let flags: i32 = flags.map(|f| f._as.into()).unwrap_or(0);

        // DefaultHLLPolicy uses the default policy when performing HLL operations.
        CdtHllPolicy {
            _as: proto::CdtHllPolicy { flags: flags },
        }
    }
}

impl From<&proto::CdtHllPolicy> for CdtHllPolicy {
    fn from(input: &proto::CdtHllPolicy) -> Self {
        CdtHllPolicy { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtHllPolicy {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtHllPolicy = zval.extract()?;

        Some(CdtHllPolicy { _as: f._as.clone() })
    }
}

///////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtHllOperation
//
////////////////////////////////////////////////////////////////////////////////////////////

/// HyperLogLog (HLL) operations.
/// Requires server versions >= 4.9.
///
/// HyperLogLog operations on HLL items nested in lists/maps are not currently
/// supported by the server.
#[php_class(name = "Aerospike\\HllOp")]
pub struct CdtHllOperation {
    _as: proto::CdtHllOperation,
}

impl FromZval<'_> for CdtHllOperation {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtHllOperation = zval.extract()?;

        Some(CdtHllOperation { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtHllOperation {
    /// HLLInitOp creates HLL init operation with minhash bits.
    /// Server creates a new HLL or resets an existing HLL.
    /// Server does not return a value.
    ///
    /// policy			write policy, use DefaultHLLPolicy for default
    /// binName			name of bin
    /// indexBitCount	number of index bits. Must be between 4 and 16 inclusive. Pass -1 for default.
    /// minHashBitCount  number of min hash bits. Must be between 4 and 58 inclusive. Pass -1 for default.
    /// indexBitCount + minHashBitCount must be <= 64.
    pub fn init(
        policy: &CdtHllPolicy,
        bin_name: String,
        index_bit_count: i64,
        min_hash_bit_count: i64,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::Init.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(index_bit_count).into(),
                    PHPValue::Int(min_hash_bit_count).into(),
                ],
            }),
        }
    }

    /// HLLAddOp creates HLL add operation with minhash bits.
    /// Server adds values to HLL set. If HLL bin does not exist, use indexBitCount and minHashBitCount
    /// to create HLL bin. Server returns number of entries that caused HLL to update a register.
    ///
    /// policy			write policy, use DefaultHLLPolicy for default
    /// binName			name of bin
    /// list				list of values to be added
    /// indexBitCount	number of index bits. Must be between 4 and 16 inclusive. Pass -1 for default.
    /// minHashBitCount  number of min hash bits. Must be between 4 and 58 inclusive. Pass -1 for default.
    /// indexBitCount + minHashBitCount must be <= 64.
    pub fn add(
        policy: &CdtHllPolicy,
        bin_name: String,
        list: Vec<PHPValue>,
        index_bit_count: i64,
        min_hash_bit_count: i64,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::Add.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::List(list).into(),
                    PHPValue::Int(index_bit_count).into(),
                    PHPValue::Int(min_hash_bit_count).into(),
                ],
            }),
        }
    }

    /// HLLSetUnionOp creates HLL set union operation.
    /// Server sets union of specified HLL objects with HLL bin.
    /// Server does not return a value.
    ///
    /// policy			write policy, use DefaultHLLPolicy for default
    /// binName			name of bin
    /// list				list of HLL objects
    pub fn set_union(
        policy: &CdtHllPolicy,
        bin_name: String,
        list: Vec<PHPValue>,
    ) -> Option<Operation> {
        if !assert_hll_list(&list) {
            return None;
        };

        Some(Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::SetUnion.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![PHPValue::List(list).into()],
            }),
        })
    }

    /// HLLRefreshCountOp creates HLL refresh operation.
    /// Server updates the cached count (if stale) and returns the count.
    ///
    /// binName			name of bin
    pub fn refresh_count(bin_name: String) -> Option<Operation> {
        Some(Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::RefreshCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![],
            }),
        })
    }

    /// HLLFoldOp creates HLL fold operation.
    /// Servers folds indexBitCount to the specified value.
    /// This can only be applied when minHashBitCount on the HLL bin is 0.
    /// Server does not return a value.
    ///
    /// binName			name of bin
    /// indexBitCount		number of index bits. Must be between 4 and 16 inclusive.
    pub fn fold(bin_name: String, index_bit_count: i64) -> Option<Operation> {
        Some(Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::Fold.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::Int(index_bit_count).into()],
            }),
        })
    }

    /// HLLGetCountOp creates HLL getCount operation.
    /// Server returns estimated number of elements in the HLL bin.
    ///
    /// binName			name of bin
    pub fn get_count(bin_name: String) -> Option<Operation> {
        Some(Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::GetCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![],
            }),
        })
    }

    /// HLLGetUnionOp creates HLL getUnion operation.
    /// Server returns an HLL object that is the union of all specified HLL objects in the list
    /// with the HLL bin.
    ///
    /// binName			name of bin
    /// list				list of HLL objects
    pub fn get_union(bin_name: String, list: Vec<PHPValue>) -> Option<Operation> {
        if !assert_hll_list(&list) {
            return None;
        };

        Some(Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::GetUnion.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::List(list).into()],
            }),
        })
    }

    /// HLLGetUnionCountOp creates HLL getUnionCount operation.
    /// Server returns estimated number of elements that would be contained by the union of these
    /// HLL objects.
    ///
    /// binName			name of bin
    /// list				list of HLL objects
    pub fn get_union_count(bin_name: String, list: Vec<PHPValue>) -> Option<Operation> {
        if !assert_hll_list(&list) {
            return None;
        };

        Some(Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::GetUnionCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::List(list).into()],
            }),
        })
    }

    /// HLLGetIntersectCountOp creates HLL getIntersectCount operation.
    /// Server returns estimated number of elements that would be contained by the intersection of
    /// these HLL objects.
    ///
    /// binName			name of bin
    /// list				list of HLL objects
    pub fn get_intersect_count(bin_name: String, list: Vec<PHPValue>) -> Option<Operation> {
        if !assert_hll_list(&list) {
            return None;
        };

        Some(Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::GetIntersectCount.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::List(list).into()],
            }),
        })
    }

    /// HLLGetSimilarityOp creates HLL getSimilarity operation.
    /// Server returns estimated similarity of these HLL objects. Return type is a double.
    ///
    /// binName			name of bin
    /// list				list of HLL objects
    pub fn get_similarity(bin_name: String, list: Vec<PHPValue>) -> Option<Operation> {
        if !assert_hll_list(&list) {
            return None;
        };

        Some(Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::GetSimilarity.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![PHPValue::List(list).into()],
            }),
        })
    }

    /// HLLDescribeOp creates HLL describe operation.
    /// Server returns indexBitCount and minHashBitCount used to create HLL bin in a list of longs.
    /// The list size is 2.
    ///
    /// binName			name of bin
    pub fn describe(bin_name: String) -> Operation {
        Operation {
            _as: proto::operation::Op::Hll(proto::CdtHllOperation {
                op: proto::CdtHllCommandOp::Describe.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![],
            }),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtBitwiseWriteFlags
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BitWriteFlags specify bitwise operation policy write flags.
#[php_class(name = "Aerospike\\BitwiseWriteFlags")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtBitwiseWriteFlags {
    _as: proto::CdtBitwiseWriteFlags,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtBitwiseWriteFlags {
    /// BitWriteFlagsDefault allows create or update.
    pub fn Default() -> Self {
        Self {
            _as: proto::CdtBitwiseWriteFlags::Default,
        }
    }

    /// BitWriteFlagsCreateOnly specifies that:
    /// If the bin already exists, the operation will be denied.
    /// If the bin does not exist, a new bin will be created.
    pub fn Create_Only() -> Self {
        Self {
            _as: proto::CdtBitwiseWriteFlags::CreateOnly,
        }
    }

    /// BitWriteFlagsUpdateOnly specifies that:
    /// If the bin already exists, the bin will be overwritten.
    /// If the bin does not exist, the operation will be denied.
    pub fn Update_Only() -> Self {
        Self {
            _as: proto::CdtBitwiseWriteFlags::UpdateOnly,
        }
    }

    /// BitWriteFlagsNoFail specifies not to raise error if operation is denied.
    pub fn No_Fail() -> Self {
        Self {
            _as: proto::CdtBitwiseWriteFlags::NoFail,
        }
    }

    /// BitWriteFlagsPartial allows other valid operations to be committed if this operations is
    /// denied due to flag constraints.
    pub fn Partial() -> Self {
        Self {
            _as: proto::CdtBitwiseWriteFlags::Partial,
        }
    }
}

impl From<&proto::CdtBitwiseWriteFlags> for CdtBitwiseWriteFlags {
    fn from(input: &proto::CdtBitwiseWriteFlags) -> Self {
        CdtBitwiseWriteFlags { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtBitwiseWriteFlags {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtBitwiseWriteFlags = zval.extract()?;

        Some(CdtBitwiseWriteFlags { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtBitwiseResizeFlags
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BitResizeFlags specifies the bitwise operation flags for resize.
#[php_class(name = "Aerospike\\BitwiseResizeFlags")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtBitwiseResizeFlags {
    _as: proto::CdtBitwiseResizeFlags,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtBitwiseResizeFlags {
    /// BitResizeFlagsDefault specifies the defalt flag.
    pub fn Default() -> Self {
        Self {
            _as: proto::CdtBitwiseResizeFlags::Default,
        }
    }

    /// BitResizeFlagsFromFront Adds/removes bytes from the beginning instead of the end.
    pub fn From_Front() -> Self {
        Self {
            _as: proto::CdtBitwiseResizeFlags::FromFront,
        }
    }

    /// BitResizeFlagsGrowOnly will only allow the []byte size to increase.
    pub fn Grow_Only() -> Self {
        Self {
            _as: proto::CdtBitwiseResizeFlags::GrowOnly,
        }
    }

    /// BitResizeFlagsShrinkOnly will only allow the []byte size to decrease.
    pub fn Shrink_Only() -> Self {
        Self {
            _as: proto::CdtBitwiseResizeFlags::ShrinkOnly,
        }
    }
}

impl From<&proto::CdtBitwiseResizeFlags> for CdtBitwiseResizeFlags {
    fn from(input: &proto::CdtBitwiseResizeFlags) -> Self {
        CdtBitwiseResizeFlags { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtBitwiseResizeFlags {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtBitwiseResizeFlags = zval.extract()?;

        Some(CdtBitwiseResizeFlags { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtBitwiseOverflowAction
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BitOverflowAction specifies the action to take when bitwise add/subtract results in overflow/underflow.
#[php_class(name = "Aerospike\\BitwiseOverflowAction")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtBitwiseOverflowAction {
    _as: proto::CdtBitwiseOverflowAction,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtBitwiseOverflowAction {
    /// BitOverflowActionFail specifies to fail operation with error.
    pub fn Fail() -> Self {
        Self {
            _as: proto::CdtBitwiseOverflowAction::Fail,
        }
    }

    /// BitOverflowActionSaturate specifies that in add/subtract overflows/underflows, set to max/min value.
    /// Example: MAXINT + 1 = MAXINT
    pub fn Saturate() -> Self {
        Self {
            _as: proto::CdtBitwiseOverflowAction::Saturate,
        }
    }

    /// BitOverflowActionWrap specifies that in add/subtract overflows/underflows, wrap the value.
    /// Example: MAXINT + 1 = -1
    pub fn Wrap() -> Self {
        Self {
            _as: proto::CdtBitwiseOverflowAction::Wrap,
        }
    }
}

impl From<&proto::CdtBitwiseOverflowAction> for CdtBitwiseOverflowAction {
    fn from(input: &proto::CdtBitwiseOverflowAction) -> Self {
        CdtBitwiseOverflowAction { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtBitwiseOverflowAction {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtBitwiseOverflowAction = zval.extract()?;

        Some(CdtBitwiseOverflowAction { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtBitwisePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// BitPolicy determines the Bit operation policy.
#[php_class(name = "Aerospike\\BitwisePolicy")]
#[derive(Debug, PartialEq, Clone)]
pub struct CdtBitwisePolicy {
    _as: proto::CdtBitwisePolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtBitwisePolicy {
    /// new BitwisePolicy(int) will return a BitPolicy will provided flags.
    pub fn __construct(flags: Option<CdtBitwiseWriteFlags>) -> Self {
        let flags: i32 = flags.map(|f| f._as.into()).unwrap_or(0);

        CdtBitwisePolicy {
            _as: proto::CdtBitwisePolicy { flags: flags },
        }
    }
}

impl From<&proto::CdtBitwisePolicy> for CdtBitwisePolicy {
    fn from(input: &proto::CdtBitwisePolicy) -> Self {
        CdtBitwisePolicy { _as: input.clone() }
    }
}

impl FromZval<'_> for CdtBitwisePolicy {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtBitwisePolicy = zval.extract()?;

        Some(CdtBitwisePolicy { _as: f._as.clone() })
    }
}

///////////////////////////////////////////////////////////////////////////////////////////
//
//  CdtBitwiseOperation
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Bit operations. Create bit operations used by client operate command.
/// Offset orientation is left-to-right.  Negative offsets are supported.
/// If the offset is negative, the offset starts backwards from end of the bitmap.
/// If an offset is out of bounds, a parameter error will be returned.
///
///	Nested CDT operations are supported by optional CTX context arguments.  Example:
///	bin = [[0b00000001, 0b01000010],[0b01011010]]
///	Resize first bitmap (in a list of bitmaps) to 3 bytes.
///	BitOperation.resize("bin", 3, BitResizeFlags.DEFAULT, CTX.listIndex(0))
///	bin result = [[0b00000001, 0b01000010, 0b00000000],[0b01011010]]
#[php_class(name = "Aerospike\\BitwiseOp")]
pub struct CdtBitwiseOperation {
    _as: proto::CdtBitwiseOperation,
}

impl FromZval<'_> for CdtBitwiseOperation {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CdtBitwiseOperation = zval.extract()?;

        Some(CdtBitwiseOperation { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl CdtBitwiseOperation {
    /// BitResizeOp creates byte "resize" operation.
    /// Server resizes []byte to byteSize according to resizeFlags (See BitResizeFlags).
    /// Server does not return a value.
    /// Example:
    ///
    ///	$bin = [0b00000001, 0b01000010]
    ///	$byteSize = 4
    ///	$resizeFlags = 0
    ///	$bin result = [0b00000001, 0b01000010, 0b00000000, 0b00000000]
    pub fn resize(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        byte_size: i64,
        resize_flags: Option<CdtBitwiseResizeFlags>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        let resize_flags: i32 = resize_flags.map(|rf| rf._as.into()).unwrap_or(0);
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Resize.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(byte_size).into(),
                    PHPValue::Int(resize_flags as i64).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitInsertOp creates byte "insert" operation.
    /// Server inserts value bytes into []byte bin at byteOffset.
    /// Server does not return a value.
    /// Example:
    ///
    ///	$bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	$byteOffset = 1
    ///	$value = [0b11111111, 0b11000111]
    ///	$bin result = [0b00000001, 0b11111111, 0b11000111, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    pub fn insert(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        byte_offset: i64,
        value: Vec<u8>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Insert.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(byte_offset).into(),
                    PHPValue::Blob(value).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitRemoveOp creates byte "remove" operation.
    /// Server removes bytes from []byte bin at byteOffset for byteSize.
    /// Server does not return a value.
    /// Example:
    ///
    ///	$bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	$byteOffset = 2
    ///	$byteSize = 3
    ///	$bin result = [0b00000001, 0b01000010]
    pub fn remove(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        byte_offset: i64,
        byte_size: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Remove.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(byte_offset).into(),
                    PHPValue::Int(byte_size).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitSetOp creates bit "set" operation.
    /// Server sets value on []byte bin at bitOffset for bitSize.
    /// Server does not return a value.
    /// Example:
    ///
    ///	$bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	$bitOffset = 13
    ///	$bitSize = 3
    ///	$value = [0b11100000]
    ///	$bin result = [0b00000001, 0b01000111, 0b00000011, 0b00000100, 0b00000101]
    pub fn set(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        value: Vec<u8>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Set.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Blob(value).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitOrOp creates bit "or" operation.
    /// Server performs bitwise "or" on value and []byte bin at bitOffset for bitSize.
    /// Server does not return a value.
    /// Example:
    ///
    ///	$bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	$bitOffset = 17
    ///	$bitSize = 6
    ///	$value = [0b10101000]
    ///	bin result = [0b00000001, 0b01000010, 0b01010111, 0b00000100, 0b00000101]
    pub fn or(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        value: Vec<u8>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Or.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Blob(value).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitXorOp creates bit "exclusive or" operation.
    /// Server performs bitwise "xor" on value and []byte bin at bitOffset for bitSize.
    /// Server does not return a value.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 17
    ///	bitSize = 6
    ///	value = [0b10101100]
    ///	bin result = [0b00000001, 0b01000010, 0b01010101, 0b00000100, 0b00000101]
    pub fn xor(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        value: Vec<u8>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Xor.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Blob(value).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitAndOp creates bit "and" operation.
    /// Server performs bitwise "and" on value and []byte bin at bitOffset for bitSize.
    /// Server does not return a value.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 23
    ///	bitSize = 9
    ///	value = [0b00111100, 0b10000000]
    ///	bin result = [0b00000001, 0b01000010, 0b00000010, 0b00000000, 0b00000101]
    pub fn and(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        value: Vec<u8>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::And.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Blob(value).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitNotOp creates bit "not" operation.
    /// Server negates []byte bin starting at bitOffset for bitSize.
    /// Server does not return a value.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 25
    ///	bitSize = 6
    ///	bin result = [0b00000001, 0b01000010, 0b00000011, 0b01111010, 0b00000101]
    pub fn not(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Not.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitLShiftOp creates bit "left shift" operation.
    /// Server shifts left []byte bin starting at bitOffset for bitSize.
    /// Server does not return a value.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 32
    ///	bitSize = 8
    ///	shift = 3
    ///	bin result = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00101000]
    pub fn lshift(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        shift: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::LShift.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Int(shift).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitRShiftOp creates bit "right shift" operation.
    /// Server shifts right []byte bin starting at bitOffset for bitSize.
    /// Server does not return a value.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 0
    ///	bitSize = 9
    ///	shift = 1
    ///	bin result = [0b00000000, 0b11000010, 0b00000011, 0b00000100, 0b00000101]
    pub fn rshift(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        shift: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::RShift.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Int(shift).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitAddOp creates bit "add" operation.
    /// Server adds value to []byte bin starting at bitOffset for bitSize. BitSize must be <= 64.
    /// Signed indicates if bits should be treated as a signed number.
    /// If add overflows/underflows, BitOverflowAction is used.
    /// Server does not return a value.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 24
    ///	bitSize = 16
    ///	value = 128
    ///	signed = false
    ///	bin result = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b10000101]
    pub fn add(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        value: i64,
        signed: bool,
        action: CdtBitwiseOverflowAction,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        let action: i32 = action._as.into();
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Add.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Int(value).into(),
                    PHPValue::Bool(signed).into(),
                    PHPValue::Int(action as i64).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitSubtractOp creates bit "subtract" operation.
    /// Server subtracts value from []byte bin starting at bitOffset for bitSize. BitSize must be <= 64.
    /// Signed indicates if bits should be treated as a signed number.
    /// If add overflows/underflows, BitOverflowAction is used.
    /// Server does not return a value.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 24
    ///	bitSize = 16
    ///	value = 128
    ///	signed = false
    ///	bin result = [0b00000001, 0b01000010, 0b00000011, 0b0000011, 0b10000101]
    pub fn subtract(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        value: i64,
        signed: bool,
        action: CdtBitwiseOverflowAction,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        let action: i32 = action._as.into();
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Subtract.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Int(value).into(),
                    PHPValue::Bool(signed).into(),
                    PHPValue::Int(action as i64).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitSetIntOp creates bit "setInt" operation.
    /// Server sets value to []byte bin starting at bitOffset for bitSize. Size must be <= 64.
    /// Server does not return a value.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 1
    ///	bitSize = 8
    ///	value = 127
    ///	bin result = [0b00111111, 0b11000010, 0b00000011, 0b0000100, 0b00000101]
    pub fn set_int(
        policy: &CdtBitwisePolicy,
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        value: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::SetInt.into(),
                policy: Some(policy._as.clone()),
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Int(value).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitGetOp creates bit "get" operation.
    /// Server returns bits from []byte bin starting at bitOffset for bitSize.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 9
    ///	bitSize = 5
    ///	returns [0b1000000]
    pub fn get(
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Get.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitCountOp creates bit "count" operation.
    /// Server returns integer count of set bits from []byte bin starting at bitOffset for bitSize.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 20
    ///	bitSize = 4
    ///	returns 2
    pub fn count(
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::Count.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitLScanOp creates bit "left scan" operation.
    /// Server returns integer bit offset of the first specified value bit in []byte bin
    /// starting at bitOffset for bitSize.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 24
    ///	bitSize = 8
    ///	value = true
    ///	returns 5
    pub fn lscan(
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        value: bool,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::LScan.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Bool(value).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitRScanOp creates bit "right scan" operation.
    /// Server returns integer bit offset of the last specified value bit in []byte bin
    /// starting at bitOffset for bitSize.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 32
    ///	bitSize = 8
    ///	value = true
    ///	returns 7
    pub fn rscan(
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        value: bool,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::RScan.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Bool(value).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }

    /// BitGetIntOp creates bit "get integer" operation.
    /// Server returns integer from []byte bin starting at bitOffset for bitSize.
    /// Signed indicates if bits should be treated as a signed number.
    /// Example:
    ///
    ///	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
    ///	bitOffset = 8
    ///	bitSize = 16
    ///	signed = false
    ///	returns 16899
    pub fn get_int(
        bin_name: String,
        bit_offset: i64,
        bit_size: i64,
        signed: bool,
        ctx: Option<Vec<&CDTContext>>,
    ) -> Operation {
        Operation {
            _as: proto::operation::Op::Bitwise(proto::CdtBitwiseOperation {
                op: proto::CdtBitwiseCommandOp::GetInt.into(),
                policy: None,
                bin_name: bin_name,
                args: vec![
                    PHPValue::Int(bit_offset).into(),
                    PHPValue::Int(bit_size).into(),
                    PHPValue::Bool(signed).into(),
                ],
                ctx: ctx
                    .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                    .unwrap_or(vec![]),
            }),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Client
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Instantiate a Client instance to access an Aerospike database cluster and perform database
/// operations.
///
/// The client is thread-safe. Only one client instance should be used per cluster. Multiple
/// threads should share this cluster instance.
///
/// Your application uses this class' API to perform database operations such as writing and
/// reading records, and selecting sets of records. Write operations include specialized
/// functionality such as append/prepend and arithmetic addition.
///
/// Each record may have multiple bins, unless the Aerospike server nodes are configured as
/// "single-bin". In "multi-bin" mode, partial records may be written or read by specifying the
/// relevant subset of bins.
fn new_aerospike_client(socket: &str) -> PhpResult<grpc::BlockingClient> {
    let client = grpc::BlockingClient::connect(socket.into()).map_err(|e| e.to_string())?;
    Ok(client)
}

#[php_class(name = "Aerospike\\Client")]
pub struct Client {
    client: Arc<Mutex<grpc::BlockingClient>>,
    socket: String,
}

/// This trivial implementation of `drop` adds a print to console.
impl Drop for Client {
    fn drop(&mut self) {
        trace!("Dropping client: {}, ptr: {:p}", self.socket, &self);
    }
}

/// Client encapsulates an Aerospike cluster.
/// All database operations are available against this object.
#[php_impl]
#[derive(ZvalConvert)]
impl Client {
    /// Connects to the Aerospike database using the provided socket address.
    ///
    /// If a persisted client object is found for the given socket address, it is returned.
    /// Otherwise, a new client object is created, persisted, and returned.
    ///
    /// # Arguments
    ///
    /// * `socket` - A string representing the socket address of the Aerospike database.
    ///
    /// # Returns
    ///
    /// * `Err("Error connecting to the database".into())` - If an error occurs during connection.
    pub fn connect(socket: &str) -> PhpResult<Zval> {
        match get_persisted_client(socket) {
            Some(c) => {
                trace!("Found Aerospike Client object for {}", socket);
                return Ok(c);
            }
            None => (),
        }

        trace!("Creating a new Aerospike Client object for {}", socket);

        let c = Arc::new(Mutex::new(new_aerospike_client(&socket)?));

        // check if version numbers match
        let request = tonic::Request::new(proto::AerospikeVersionRequest {});
        let grpcClient = c.clone();
        let mut client = grpcClient.lock().unwrap();
        let res = client.version(request).map_err(|e| e.to_string())?;
        // Or match the comparison operators
        let vClient = Version::from(VERSION).unwrap();
        let vServer = Version::from(&res.get_ref().version).unwrap();
        if vServer.compare(&vClient) != Cmp::Eq {
            return Err(format!(
                "Rust Client version `{}` does not match the connection manager version `{}`",
                vClient, vServer,
            )
            .into());
        };

        persist_client(socket, c)?;

        match get_persisted_client(socket) {
            Some(c) => {
                return Ok(c);
            }
            None => Err("Error connecting to the database".into()),
        }
    }

    /// Retrieves the socket address associated with this client.
    ///
    /// # Returns
    ///
    /// A string representing the socket address.
    #[getter]
    pub fn socket(&self) -> String {
        self.socket.clone()
    }

    /// Write record bin(s). The policy specifies the transaction timeout, record expiration and
    /// how the transaction is handled when the record already exists.
    pub fn put(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<proto::Bin> = bins.into_iter().map(|b| b.into()).collect();

        let request = tonic::Request::new(proto::AerospikePutRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
            bins: bins.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.put(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::Error {
                result_code: 0,
                in_doubt: _,
            } => Ok(()),
            pe => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// Read record for the specified key. Depending on the bins value provided, all record bins,
    /// only selected record bins or only the record headers will be returned. The policy can be
    /// used to specify timeouts.
    pub fn get(
        &mut self,
        policy: &ReadPolicy,
        key: &Key,
        bins: Option<Vec<String>>,
    ) -> PhpResult<Option<Record>> {
        let request = tonic::Request::new(proto::AerospikeGetRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
            bin_names: bins.unwrap_or(vec![]),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.get(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeSingleResponse {
                error: None,
                record: Some(rec),
            } => Ok(Some(Record {
                _as: (*rec).clone(),
            })),
            // Not found: Do not throw an exception
            proto::AerospikeSingleResponse {
                error:
                    Some(proto::Error {
                        result_code: ResultCode::KEY_NOT_FOUND_ERROR,
                        in_doubt: false,
                    }),
                record: None,
            } => Ok(None),
            proto::AerospikeSingleResponse {
                error: Some(pe),
                record: None,
            } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(None)
            }
            _ => unreachable!(),
        }
    }

    /// Read record for the specified key. Depending on the bins value provided, all record bins,
    /// only selected record bins or only the record headers will be returned. The policy can be
    /// used to specify timeouts.
    pub fn get_header(&mut self, policy: &ReadPolicy, key: &Key) -> PhpResult<Option<Record>> {
        let request = tonic::Request::new(proto::AerospikeGetHeaderRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.get_header(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeSingleResponse {
                error: None,
                record: Some(rec),
            } => Ok(Some(Record {
                _as: (*rec).clone(),
            })),
            // Not found: Do not throw an exception
            proto::AerospikeSingleResponse {
                error:
                    Some(proto::Error {
                        result_code: ResultCode::KEY_NOT_FOUND_ERROR,
                        in_doubt: false,
                    }),
                record: None,
            } => Ok(None),
            proto::AerospikeSingleResponse {
                error: Some(pe),
                record: None,
            } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(None)
            }
            _ => unreachable!(),
        }
    }

    /// Add integer bin values to existing record bin values. The policy specifies the transaction
    /// timeout, record expiration and how the transaction is handled when the record already
    /// exists. This call only works for integer values.
    pub fn add(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<proto::Bin> = bins.into_iter().map(|b| b.into()).collect();

        let request = tonic::Request::new(proto::AerospikePutRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
            bins: bins.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.add(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::Error {
                result_code: 0,
                in_doubt: _,
            } => Ok(()),
            pe => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// Append bin string values to existing record bin values. The policy specifies the
    /// transaction timeout, record expiration and how the transaction is handled when the record
    /// already exists. This call only works for string values.
    pub fn append(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<proto::Bin> = bins.into_iter().map(|b| b.into()).collect();

        let request = tonic::Request::new(proto::AerospikePutRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
            bins: bins.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.append(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::Error {
                result_code: 0,
                in_doubt: _,
            } => Ok(()),
            pe => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// Prepend bin string values to existing record bin values. The policy specifies the
    /// transaction timeout, record expiration and how the transaction is handled when the record
    /// already exists. This call only works for string values.
    pub fn prepend(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<proto::Bin> = bins.into_iter().map(|b| b.into()).collect();

        let request = tonic::Request::new(proto::AerospikePutRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
            bins: bins.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.prepend(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::Error { result_code: 0, .. } => Ok(()),
            pe => {
                let error: AerospikeException = pe.into();
                Ok(throw_object(error.into_zval(false)?)?)
            }
        }
    }

    /// Delete record for specified key. The policy specifies the transaction timeout.
    /// The call returns `true` if the record existed on the server before deletion.
    pub fn delete(&self, policy: &WritePolicy, key: &Key) -> PhpResult<bool> {
        let request = tonic::Request::new(proto::AerospikeDeleteRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.delete(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeDeleteResponse {
                error: None,
                existed,
            } => Ok(existed.is_some()),
            proto::AerospikeDeleteResponse {
                error: Some(pe), ..
            } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(false)
            }
        }
    }

    /// Reset record's time to expiration using the policy's expiration. Fail if the record does
    /// not exist.
    pub fn touch(&self, policy: &WritePolicy, key: &Key) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeTouchRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.touch(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::Error { result_code: 0, .. } => Ok(()),
            pe => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// Determine if a record key exists. The policy can be used to specify timeouts.
    pub fn exists(&self, policy: &ReadPolicy, key: &Key) -> PhpResult<bool> {
        let request = tonic::Request::new(proto::AerospikeExistsRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.exists(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeExistsResponse {
                error: None,
                exists,
            } => Ok(exists.unwrap()),
            proto::AerospikeExistsResponse {
                error: Some(pe), ..
            } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(false)
            }
        }
    }

    /// BatchExecute will read/write multiple records for specified batch keys in one batch call.
    /// This method allows different namespaces/bins for each key in the batch.
    /// The returned records are located in the same list.
    ///
    /// BatchRecord can be *BatchRead, *BatchWrite, *BatchDelete or *BatchUDF.
    ///
    /// Requires server version 6.0+
    pub fn batch(&self, policy: &BatchPolicy, cmds: Vec<&Zval>) -> PhpResult<Vec<BatchRecord>> {
        let mut res = Vec::<proto::BatchOperate>::with_capacity(cmds.len());
        cmds.into_iter().for_each(|v| {
            if let Some(&BatchRead { ref _as }) = v.extract() {
                res.push(proto::BatchOperate {
                    br: Some((*_as).clone()),
                    ..proto::BatchOperate::default()
                });
            } else if let Some(&BatchWrite { ref _as }) = v.extract() {
                res.push(proto::BatchOperate {
                    bw: Some((*_as).clone()),
                    ..proto::BatchOperate::default()
                });
            } else if let Some(&BatchDelete { ref _as }) = v.extract() {
                res.push(proto::BatchOperate {
                    bd: Some((*_as).clone()),
                    ..proto::BatchOperate::default()
                });
            } else if let Some(&BatchUdf { ref _as }) = v.extract() {
                res.push(proto::BatchOperate {
                    bu: Some((*_as).clone()),
                    ..proto::BatchOperate::default()
                });
            } else {
                let error = AerospikeException::new("Invalid Batch command".into());
                let _ = throw_object(error.into_zval(true).unwrap());
            }
        });

        let request = tonic::Request::new(proto::AerospikeBatchOperateRequest {
            policy: Some(policy._as.clone()),
            records: res,
        });

        let mut client = self.client.lock().unwrap();
        let res = client.batch_operate(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeBatchOperateResponse {
                error: None,
                records,
            } => Ok(records
                .into_iter()
                .map(|v| BatchRecord { _as: (*v).clone() })
                .collect()),
            proto::AerospikeBatchOperateResponse {
                error: Some(pe), ..
            } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(vec![])
            }
        }
    }

    /// Removes all records in the specified namespace/set efficiently.
    pub fn truncate(
        &self,
        policy: &InfoPolicy,
        namespace: &str,
        set_name: &str,
        before_nanos: Option<i64>,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeTruncateRequest {
            policy: Some(policy._as.clone()),
            namespace: namespace.into(),
            set_name: set_name.into(),
            before_nanos: before_nanos,
        });

        let mut client = self.client.lock().unwrap();
        let res = client.truncate(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeTruncateResponse { error: None } => Ok(()),
            proto::AerospikeTruncateResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// Read all records in the specified namespace and set and return a record iterator. The scan
    /// executor puts records on a queue in separate threads. The calling thread concurrently pops
    /// records off the queue through the record iterator. Up to `policy.max_concurrent_nodes`
    /// nodes are scanned in parallel. If concurrent nodes is set to zero, the server nodes are
    /// read in series.
    pub fn scan(
        &self,
        policy: &ScanPolicy,
        mut partition_filter: PartitionFilter,
        namespace: &str,
        set_name: &str,
        bins: Option<Vec<String>>,
    ) -> PhpResult<Recordset> {
        let res = {
            let pf = partition_filter._as.lock().unwrap();
            let request = tonic::Request::new(proto::AerospikeScanRequest {
                policy: Some(policy._as.clone()),
                namespace: namespace.into(),
                set_name: set_name.into(),
                bin_names: bins.unwrap_or(vec![]),
                partition_filter: Some(pf.clone()),
            });

            let mut client = self.client.lock().unwrap();
            client.scan(request).map_err(|e| e.to_string())?
        };

        // init the partition_status status
        // we late init it to avoid sending it to the server when the value is default
        // since it will be initialized there anyway
        partition_filter.init_partition_status();

        Ok(Recordset {
            _as: Some(res.into_inner()),
            client: self.client.clone(),
            partition_filter: partition_filter,
        })
    }

    /// Execute a query on all server nodes and return a record iterator. The query executor puts
    /// records on a queue in separate threads. The calling thread concurrently pops records off
    /// the queue through the record iterator.
    pub fn query(
        &self,
        policy: &QueryPolicy,
        mut partition_filter: PartitionFilter,
        statement: &mut Statement,
    ) -> PhpResult<Recordset> {
        let res = {
            let pf = partition_filter._as.lock().unwrap();
            let request = tonic::Request::new(proto::AerospikeQueryRequest {
                policy: Some(policy._as.clone()),
                partition_filter: Some(pf.clone()),
                statement: statement._as.clone().into(),
            });

            let mut client = self.client.lock().unwrap();
            client.query(request).map_err(|e| e.to_string())?
        };

        // init the partition_status status
        // we late init it to avoid sending it to the server when the value is default
        // since it will be initialized there anyway
        partition_filter.init_partition_status();

        Ok(Recordset {
            _as: Some(res.into_inner()),
            client: self.client.clone(),
            partition_filter: partition_filter,
        })
    }

    /// CreateIndex creates a secondary index.
    /// This asynchronous server call will return before the command is complete.
    /// The user can optionally wait for command completion by using the returned
    /// IndexTask instance.
    /// This method is only supported by Aerospike 3+ servers.
    /// If the policy is nil, the default relevant policy will be used.
    pub fn create_index(
        &self,
        policy: &WritePolicy,
        namespace: &str,
        set_name: &str,
        bin_name: &str,
        index_name: &str,
        index_type: &IndexType,
        cit: Option<&IndexCollectionType>,
        ctx: Option<Vec<&CDTContext>>,
    ) -> PhpResult<()> {
        let ictDefault = &IndexCollectionType::Default();
        let cit = cit.unwrap_or(ictDefault);
        let request = tonic::Request::new(proto::AerospikeCreateIndexRequest {
            policy: Some(policy._as.clone()),
            namespace: namespace.into(),
            set_name: set_name.into(),
            index_name: index_name.into(),
            bin_name: bin_name.into(),
            index_type: index_type._as.into(),
            index_collection_type: cit._as.into(),
            ctx: ctx
                .map(|ctx| ctx.iter().map(|ctx| ctx._as.clone()).collect())
                .unwrap_or(vec![]),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.create_index(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeCreateIndexResponse { error: None } => Ok(()),
            proto::AerospikeCreateIndexResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// DropIndex deletes a secondary index. It will block until index is dropped on all nodes.
    /// This method is only supported by Aerospike 3+ servers.
    /// If the policy is nil, the default relevant policy will be used.
    pub fn drop_index(
        &self,
        policy: &WritePolicy,
        namespace: &str,
        set_name: &str,
        index_name: &str,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeDropIndexRequest {
            policy: Some(policy._as.clone()),
            namespace: namespace.into(),
            set_name: set_name.into(),
            index_name: index_name.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.drop_index(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeDropIndexResponse { error: None } => Ok(()),
            proto::AerospikeDropIndexResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// RegisterUDF registers a package containing user defined functions with server.
    /// This asynchronous server call will return before command is complete.
    /// The user can optionally wait for command completion by using the returned
    /// RegisterTask instance.
    ///
    /// This method is only supported by Aerospike 3+ servers.
    /// If the policy is nil, the default relevant policy will be used.
    pub fn register_udf(
        &self,
        policy: &WritePolicy,
        udf_body: &str,
        package_name: &str,
        language: Option<UdfLanguage>,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeRegisterUdfRequest {
            policy: Some(policy._as.clone()),
            udf_body: udf_body.into(),
            package_name: package_name.into(),
            language: language.unwrap_or(UdfLanguage::Lua()).into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.register_udf(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeRegisterUdfResponse { error: None } => Ok(()),
            proto::AerospikeRegisterUdfResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// DropUDF removes a package containing user defined functions in the server.
    /// This asynchronous server call will return before command is complete.
    /// The user can optionally wait for command completion by using the returned
    /// RemoveTask instance.
    ///
    /// This method is only supported by Aerospike 3+ servers.
    /// If the policy is nil, the default relevant policy will be used.
    pub fn drop_udf(&self, policy: &WritePolicy, package_name: &str) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeDropUdfRequest {
            policy: Some(policy._as.clone()),
            package_name: package_name.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.drop_udf(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeDropUdfResponse { error: None } => Ok(()),
            proto::AerospikeDropUdfResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// ListUDF lists all packages containing user defined functions in the server.
    /// This method is only supported by Aerospike 3+ servers.
    /// If the policy is nil, the default relevant policy will be used.
    pub fn list_udf(&self, policy: &ReadPolicy) -> PhpResult<Vec<UdfMeta>> {
        let request = tonic::Request::new(proto::AerospikeListUdfRequest {
            policy: Some(policy._as.clone()),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.list_udf(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeListUdfResponse {
                error: None,
                udf_list,
            } => Ok(udf_list
                .into_iter()
                .map(|v| UdfMeta { _as: (*v).clone() })
                .collect()),
            proto::AerospikeListUdfResponse {
                error: Some(pe), ..
            } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(vec![])
            }
        }
    }

    /// Execute executes a user defined function on server and return results.
    /// The function operates on a single record.
    /// The package name is used to locate the udf file location:
    ///
    /// udf file = <server udf dir>/<package name>.lua
    ///
    /// This method is only supported by Aerospike 3+ servers.
    /// If the policy is nil, the default relevant policy will be used.
    pub fn udf_execute(
        &self,
        policy: &WritePolicy,
        key: &Key,
        package_name: String,
        function_name: String,
        args: Vec<PHPValue>,
    ) -> PhpResult<PHPValue> {
        let args: Vec<proto::Value> = args.into_iter().map(|v| v.into()).collect();

        let request = tonic::Request::new(proto::AerospikeUdfExecuteRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
            package_name: package_name,
            function_name: function_name,
            args: args.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.udf_execute(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeUdfExecuteResponse {
                error: None,
                result,
            } => Ok(match result {
                Some(v) => v.clone().into(),
                None => PHPValue::Nil,
            }),
            proto::AerospikeUdfExecuteResponse {
                error: Some(pe), ..
            } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(Value::nil())
            }
        }
    }

    //-------------------------------------------------------
    // User administration
    //-------------------------------------------------------

    /// CreateUser creates a new user with password and roles. Clear-text password will be hashed using bcrypt
    /// before sending to server.
    pub fn create_user(
        &self,
        policy: &AdminPolicy,
        user: String,
        password: String,
        roles: Vec<String>,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeCreateUserRequest {
            policy: Some(policy._as.clone()),
            user: user.into(),
            password: password.into(),
            roles: roles.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.create_user(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeCreateUserResponse { error: None } => Ok(()),
            proto::AerospikeCreateUserResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// DropUser removes a user from the cluster.
    pub fn drop_user(&self, policy: &AdminPolicy, user: String) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeDropUserRequest {
            policy: Some(policy._as.clone()),
            user: user.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.drop_user(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeDropUserResponse { error: None } => Ok(()),
            proto::AerospikeDropUserResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// ChangePassword changes a user's password. Clear-text password will be hashed using bcrypt before sending to server.
    pub fn change_password(
        &self,
        policy: &AdminPolicy,
        user: String,
        password: String,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeChangePasswordRequest {
            policy: Some(policy._as.clone()),
            user: user.into(),
            password: password.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.change_password(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeChangePasswordResponse { error: None } => Ok(()),
            proto::AerospikeChangePasswordResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// GrantRoles adds roles to user's list of roles.
    pub fn grant_roles(
        &self,
        policy: &AdminPolicy,
        user: String,
        roles: Vec<String>,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeGrantRolesRequest {
            policy: Some(policy._as.clone()),
            user: user.into(),
            roles: roles.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.grant_roles(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeGrantRolesResponse { error: None } => Ok(()),
            proto::AerospikeGrantRolesResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// RevokeRoles removes roles from user's list of roles.
    pub fn revoke_roles(
        &self,
        policy: &AdminPolicy,
        user: String,
        roles: Vec<String>,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeRevokeRolesRequest {
            policy: Some(policy._as.clone()),
            user: user.into(),
            roles: roles.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.revoke_roles(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeRevokeRolesResponse { error: None } => Ok(()),
            proto::AerospikeRevokeRolesResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// QueryUser retrieves roles for a given user.
    pub fn query_users(
        &self,
        policy: &AdminPolicy,
        user: Option<String>,
    ) -> PhpResult<Vec<UserRole>> {
        let request = tonic::Request::new(proto::AerospikeQueryUsersRequest {
            policy: Some(policy._as.clone()),
            user: user,
        });

        let mut client = self.client.lock().unwrap();
        let res = client.query_users(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeQueryUsersResponse {
                error: None,
                user_roles,
            } => Ok(user_roles.iter().map(|v| v.into()).collect()),
            proto::AerospikeQueryUsersResponse {
                error: Some(pe), ..
            } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(vec![])
            }
        }
    }

    /// QueryRole retrieves privileges for a given role.
    pub fn query_roles(
        &self,
        policy: &AdminPolicy,
        role_name: Option<String>,
    ) -> PhpResult<Vec<Role>> {
        let request = tonic::Request::new(proto::AerospikeQueryRolesRequest {
            policy: Some(policy._as.clone()),
            role_name: role_name,
        });

        let mut client = self.client.lock().unwrap();
        let res = client.query_roles(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeQueryRolesResponse { error: None, roles } => {
                Ok(roles.iter().map(|v| v.into()).collect())
            }
            proto::AerospikeQueryRolesResponse {
                error: Some(pe), ..
            } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(vec![])
            }
        }
    }

    /// CreateRole creates a user-defined role.
    /// Quotas require server security configuration "enable-quotas" to be set to true.
    /// Pass 0 for quota values for no limit.
    pub fn create_role(
        &self,
        policy: &AdminPolicy,
        role_name: String,
        privileges: Vec<Privilege>,
        allowlist: Vec<String>,
        read_quota: u32,
        write_quota: u32,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeCreateRoleRequest {
            policy: Some(policy._as.clone()),
            role_name: role_name,
            privileges: privileges.iter().map(|v| v._as.clone()).collect(),
            allowlist: allowlist,
            read_quota: read_quota,
            write_quota: write_quota,
        });

        let mut client = self.client.lock().unwrap();
        let res = client.create_role(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeCreateRoleResponse { error: None } => Ok(()),
            proto::AerospikeCreateRoleResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// DropRole removes a user-defined role.
    pub fn drop_role(&self, policy: &AdminPolicy, role_name: String) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeDropRoleRequest {
            policy: Some(policy._as.clone()),
            role_name: role_name,
        });

        let mut client = self.client.lock().unwrap();
        let res = client.drop_role(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeDropRoleResponse { error: None } => Ok(()),
            proto::AerospikeDropRoleResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// GrantPrivileges grant privileges to a user-defined role.
    pub fn grant_privileges(
        &self,
        policy: &AdminPolicy,
        role_name: String,
        privileges: Vec<Privilege>,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeGrantPrivilegesRequest {
            policy: Some(policy._as.clone()),
            role_name: role_name,
            privileges: privileges.iter().map(|v| v._as.clone()).collect(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client
            .grant_privileges(request)
            .map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeGrantPrivilegesResponse { error: None } => Ok(()),
            proto::AerospikeGrantPrivilegesResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// RevokePrivileges revokes privileges from a user-defined role.
    pub fn revoke_privileges(
        &self,
        policy: &AdminPolicy,
        role_name: String,
        privileges: Vec<Privilege>,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeRevokePrivilegesRequest {
            policy: Some(policy._as.clone()),
            role_name: role_name,
            privileges: privileges.iter().map(|v| v._as.clone()).collect(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client
            .revoke_privileges(request)
            .map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeRevokePrivilegesResponse { error: None } => Ok(()),
            proto::AerospikeRevokePrivilegesResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// SetAllowlist sets IP address whitelist for a role. If whitelist is nil or empty, it removes existing whitelist from role.
    pub fn set_allowlist(
        &self,
        policy: &AdminPolicy,
        role_name: String,
        allowlist: Vec<String>,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeSetAllowlistRequest {
            policy: Some(policy._as.clone()),
            role_name: role_name,
            allowlist: allowlist,
        });

        let mut client = self.client.lock().unwrap();
        let res = client.set_allowlist(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeSetAllowlistResponse { error: None } => Ok(()),
            proto::AerospikeSetAllowlistResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }

    /// SetQuotas sets maximum reads/writes per second limits for a role.  If a quota is zero, the limit is removed.
    /// Quotas require server security configuration "enable-quotas" to be set to true.
    /// Pass 0 for quota values for no limit.
    pub fn set_quotas(
        &self,
        policy: &AdminPolicy,
        role_name: String,
        read_quota: u32,
        write_quota: u32,
    ) -> PhpResult<()> {
        let request = tonic::Request::new(proto::AerospikeSetQuotasRequest {
            policy: Some(policy._as.clone()),
            role_name: role_name,
            read_quota: read_quota,
            write_quota: write_quota,
        });

        let mut client = self.client.lock().unwrap();
        let res = client.set_quotas(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeSetQuotasResponse { error: None } => Ok(()),
            proto::AerospikeSetQuotasResponse { error: Some(pe) } => {
                let error: AerospikeException = pe.into();
                throw_object(error.into_zval(true)?)?;
                Ok(())
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  AerospikeException
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Represents an exception specific to the Aerospike database operations.
#[php_class(name = "Aerospike\\AerospikeException")]
#[extends(ext_php_rs::zend::ce::exception())]
#[derive(Debug, Clone)]
pub struct AerospikeException {
    #[prop(flags = ext_php_rs::flags::PropertyFlags::Public)]
    message: String,
    #[prop(flags = ext_php_rs::flags::PropertyFlags::Public)]
    code: i32,
    #[prop(flags = ext_php_rs::flags::PropertyFlags::Public)]
    in_doubt: bool,
}

/// Constructs a new `AerospikeException` with the specified error message.
///
/// # Arguments
///
/// * `message` - The error message describing the exception.
///
/// # Returns
///
/// A new `AerospikeException` instance initialized with the provided error message.
impl AerospikeException {
    pub fn new(message: &str) -> Self {
        AerospikeException {
            message: message.to_string(),
            code: ResultCode::COMMON_ERROR,
            in_doubt: false,
        }
    }
}

impl From<&proto::Error> for AerospikeException {
    fn from(error: &proto::Error) -> AerospikeException {
        let msg: String = ResultCode::to_string(error.result_code);
        AerospikeException {
            message: msg,
            code: error.result_code,
            in_doubt: error.in_doubt,
        }
    }
}

impl From<AerospikeException> for PhpException {
    fn from(error: AerospikeException) -> PhpException {
        PhpException::default(error.message)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Key
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Key is the unique record identifier. Records can be identified using a specified namespace,
/// an optional set name, and a user defined key which must be unique within a set.
/// Records can also be identified by namespace/digest which is the combination used
/// on the server.
#[php_class(name = "Aerospike\\Key")]
pub struct Key {
    _as: proto::Key,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Key {
    pub fn __construct(namespace: &str, set: &str, key: PHPValue) -> Self {
        let _as = proto::Key {
            digest: None, //Some(Self::compute_digest(set, key.clone())),
            namespace: Some(namespace.into()),
            set: Some(set.into()),
            value: Some(key.into()),
        };
        Key { _as: _as }
    }

    /// namespace. Equivalent to database name.
    #[getter]
    pub fn get_namespace(&self) -> String {
        self._as.namespace.clone().unwrap_or("".into())
    }

    /// Optional set name. Equivalent to database table.
    #[getter]
    pub fn get_setname(&self) -> String {
        self._as.set.clone().unwrap_or("".into())
    }

    /// getValue() returns key's value.
    #[getter]
    pub fn get_value(&self) -> Option<PHPValue> {
        self._as.value.clone().map(|v| v.into())
    }

    /// Generate unique server hash value from set name, key type and user defined key.
    /// The hash function is RIPEMD-160 (a 160 bit hash).
    fn compute_digest(&self) -> Vec<u8> {
        let mut hash = Ripemd160::new();
        match (self._as.set.as_ref(), self._as.value.as_ref()) {
            (Some(set), Some(value)) => {
                hash.input(set.as_bytes());
                let value: PHPValue = value.clone().into();
                hash.input(&[value.particle_type() as u8]);
                match value.write_key_bytes(&mut hash) {
                    Ok(()) => (),
                    Err(_) => {
                        return vec![];
                    }
                };
                let h: [u8; 20] = hash.result().into();
                h.into()
            }
            _ => vec![],
        }
    }

    /// get_digest_bytes returns key digest as byte array.
    pub fn get_digest_bytes(&self) -> Vec<u8> {
        self._as
            .digest
            .as_ref()
            .map(|digest| digest.clone())
            .unwrap_or(self.compute_digest())
    }

    /// get_digest returns key digest as string.
    #[getter]
    pub fn get_digest(&self) -> String {
        hex::encode(self.get_digest_bytes())
    }

    /// PartitionId returns the partition that the key belongs to.
    #[getter]
    fn partition_id(&self) -> Option<usize> {
        let digest = self.get_digest_bytes();
        let mut rdr = Cursor::new(&digest[0..4]);
        Some(rdr.read_u32::<LittleEndian>().unwrap() as usize & (PARTITIONS as usize - 1))
    }
}

impl FromZval<'_> for Key {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Key = zval.extract()?;

        Some(Key { _as: f._as.clone() })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  GeoJSON
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of the GeoJson Value for Aerospike.
#[php_class(name = "Aerospike\\GeoJSON")]
pub struct GeoJSON {
    v: String,
}

impl FromZval<'_> for GeoJSON {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &GeoJSON = zval.extract()?;

        Some(GeoJSON { v: f.v.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl GeoJSON {
    #[getter]
    pub fn get_value(&self) -> String {
        self.v.clone()
    }

    #[setter]
    pub fn set_value(&mut self, geo: String) {
        self.v = geo
    }

    /// Returns a string representation of the value.
    pub fn as_string(&self) -> String {
        PHPValue::GeoJSON(self.v.clone()).as_string()
    }
}

impl fmt::Display for GeoJSON {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(f, "{}", self.as_string())
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Json
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of the Json (Map<String, Value>) data structure for Aerospike.
#[php_class(name = "Aerospike\\Json")]
pub struct Json {
    v: HashMap<String, PHPValue>,
}

impl FromZval<'_> for Json {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Json = zval.extract()?;

        Some(Json { v: f.v.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Json {
    /// getter method to get the json value
    #[getter]
    pub fn get_value(&self) -> HashMap<String, PHPValue> {
        self.v.clone()
    }

    /// setter method to set the json value
    #[setter]
    pub fn set_value(&mut self, v: HashMap<String, PHPValue>) {
        self.v = v
    }

    /// Returns a string representation of the value.
    pub fn as_string(&self) -> String {
        PHPValue::Json(self.v.clone()).as_string()
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(f, "{}", self.as_string())
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Infinity
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Represents a infinity value for Aerospike.
#[php_class(name = "Aerospike\\Infinity")]
pub struct Infinity {}

impl FromZval<'_> for Infinity {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let _f: &Infinity = zval.extract()?;

        Some(Infinity {})
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Wildcard
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Represents a wildcard value for Aerospike.
#[php_class(name = "Aerospike\\Wildcard")]
pub struct Wildcard {}

impl FromZval<'_> for Wildcard {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let _f: &Wildcard = zval.extract()?;

        Some(Wildcard {})
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BLOB
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of the BLOB data structure for Aerospike.
#[php_class(name = "Aerospike\\BLOB")]
pub struct BLOB {
    v: Vec<u8>,
}

impl FromZval<'_> for BLOB {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &BLOB = zval.extract()?;

        Some(BLOB { v: f.v.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl BLOB {
    #[getter]
    pub fn get_binary(&self) -> Binary<u8> {
        self.v.clone().into_iter().collect::<Binary<_>>()
    }

    #[getter]
    pub fn get_value(&self) -> Vec<u8> {
        self.v.clone()
    }

    #[setter]
    pub fn set_value(&mut self, blob: Vec<u8>) {
        self.v = blob
    }

    /// Returns a string representation of the value.
    pub fn as_string(&self) -> String {
        PHPValue::Blob(self.v.clone()).as_string()
    }

    /// Returns a string representation of the value.
    pub fn equals(&self, other: &Self) -> bool {
        self.v == other.v
    }
}

impl fmt::Display for BLOB {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(f, "{}", self.as_string())
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  HLL
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of the HyperLogLog (HLL) data structure for Aerospike.
#[php_class(name = "Aerospike\\HLL")]
pub struct HLL {
    v: Vec<u8>,
}

impl FromZval<'_> for HLL {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &HLL = zval.extract()?;

        Some(HLL { v: f.v.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl HLL {
    #[getter]
    pub fn get_value(&self) -> Vec<u8> {
        self.v.clone()
    }

    #[setter]
    pub fn set_value(&mut self, hll: Vec<u8>) {
        self.v = hll
    }

    /// Returns a string representation of the value.
    pub fn as_string(&self) -> String {
        PHPValue::HLL(self.v.clone()).as_string()
    }
}

impl fmt::Display for HLL {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(f, "{}", self.as_string())
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  PHPValue
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Container for bin values stored in the Aerospike database.
#[derive(Debug, Clone, PartialEq, Eq)]
// TODO: underlying_value; convert to proto::Value to avoid conversions
pub enum PHPValue {
    /// Empty value.
    Nil,
    /// Boolean value.
    Bool(bool),
    /// Integer value. All integers are represented as 64-bit numerics in Aerospike.
    Int(i64),
    /// Unsigned integer value. The largest integer value that can be stored in a record bin is
    /// `i64::max_value()`; however the list and map data types can store integer values (and keys)
    /// up to `u64::max_value()`.
    ///
    /// # Panics
    ///
    /// Attempting to store an `u64` value as a record bin value will cause a panic. Use casting to
    /// store and retrieve `u64` values.
    UInt(u64),
    /// Floating point value. All floating point values are stored in 64-bit IEEE-754 format in
    /// Aerospike. Aerospike server v3.6.0 and later support double data type.
    Float(ordered_float::OrderedFloat<f64>),
    /// String value.
    String(String),
    /// Byte array value.
    Blob(Vec<u8>),
    /// List data type is an ordered collection of values. Lists can contain values of any
    /// supported data type. List data order is maintained on writes and reads.
    List(Vec<PHPValue>),
    /// Map data type is a collection of key-value pairs. Each key can only appear once in a
    /// collection and is associated with a value. Map keys and values can be any supported data
    /// type.
    /// TODO: Implement the ordered map and remove hashmap completely
    HashMap(HashMap<PHPValue, PHPValue>),
    /// Map data type is a collection of key-value pairs. Each key can only appear once in a
    /// collection and is associated with a value. Map keys and values can be any supported data
    /// type.
    Json(HashMap<String, PHPValue>),
    /// GeoJSON data type are JSON formatted strings to encode geospatial information.
    GeoJSON(String),

    /// HLL value
    HLL(Vec<u8>),

    /// Wildcard value.
    Wildcard,

    /// Infinity value.
    Infinity,
}

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for PHPValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match *self {
            PHPValue::Nil => {
                let v: Option<u8> = None;
                v.hash(state);
            }
            PHPValue::Bool(ref val) => val.hash(state),
            PHPValue::Int(ref val) => val.hash(state),
            PHPValue::UInt(ref val) => val.hash(state),
            PHPValue::Float(ref val) => val.hash(state),
            PHPValue::String(ref val) | PHPValue::GeoJSON(ref val) => val.hash(state),
            PHPValue::Blob(ref val) | PHPValue::HLL(ref val) => val.hash(state),
            PHPValue::List(ref val) => val.hash(state),
            PHPValue::HashMap(_) => {
                let error = AerospikeException::new("HashMaps cannot be used as map keys.");
                let _ = throw_object(error.into_zval(true).unwrap());
            }
            PHPValue::Json(_) => {
                let error = AerospikeException::new("Jsons cannot be used as map keys.");
                let _ = throw_object(error.into_zval(true).unwrap());
            }
            PHPValue::Infinity => {
                let error = AerospikeException::new("Infinity cannot be used as map keys.");
                let _ = throw_object(error.into_zval(true).unwrap());
            }
            PHPValue::Wildcard => {
                let error = AerospikeException::new("Infinity cannot be used as map keys.");
                let _ = throw_object(error.into_zval(true).unwrap());
            } // PHPValue::OrderedMap(_) => panic!("OrderedMaps cannot be used as map keys."),
        }
    }
}

impl PHPValue {
    /// Returns a string representation of the value.
    pub fn as_string(&self) -> String {
        match *self {
            PHPValue::Nil => "<null>".to_string(),
            PHPValue::Int(ref val) => val.to_string(),
            PHPValue::UInt(ref val) => val.to_string(),
            PHPValue::Bool(ref val) => val.to_string(),
            PHPValue::Float(ref val) => val.to_string(),
            PHPValue::String(ref val) => val.to_string(),
            PHPValue::GeoJSON(ref val) => format!("GeoJSON('{}')", val.to_string()),
            PHPValue::Blob(ref val) => format!("Blob({:?})", val),
            PHPValue::HLL(ref val) => format!("HLL('{:?}')", val),
            PHPValue::List(ref val) => format!("{:?}", val),
            PHPValue::HashMap(ref val) => format!("{:?}", val),
            PHPValue::Json(ref val) => format!("{:?}", val),
            PHPValue::Infinity => "<infinity>".to_string(),
            PHPValue::Wildcard => "<wildcard>".to_string(),
            // PHPValue::OrderedMap(ref val) => format!("{:?}", val),
        }
    }

    fn particle_type(&self) -> u32 {
        match *self {
            PHPValue::Nil => 0,
            PHPValue::Int(_) => 1,
            PHPValue::UInt(_) => 1,
            PHPValue::Float(_) => 2,
            PHPValue::String(_) => 3,
            PHPValue::Blob(_) => 4,
            PHPValue::Bool(_) => 17,
            PHPValue::HLL(_) => 18,
            PHPValue::HashMap(_) => 19,
            PHPValue::Json(_) => 19,
            PHPValue::List(_) => 20,
            PHPValue::GeoJSON(_) => 23,
            PHPValue::Infinity => unreachable!(),
            PHPValue::Wildcard => unreachable!(),
            // PHPValue::OrderedMap(_) => format!("{:?}", val),
        }
    }

    /// Serialize the value as a record key.
    /// For internal use only.
    fn write_key_bytes(self, h: &mut Ripemd160) -> Result<(), String> {
        match self {
            PHPValue::Int(ref val) => {
                let mut buf = [0; 8];
                NetworkEndian::write_i64(&mut buf, *val);
                h.input(&buf);
                Ok(())
            }
            PHPValue::String(ref val) => {
                h.input(val.as_bytes());
                Ok(())
            }
            PHPValue::Blob(ref val) => {
                h.input(val);
                Ok(())
            }
            _ => Err(format!("Data type is not supported as Key value: {}", self)),
        }
    }
}

impl fmt::Display for PHPValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(f, "{}", self.as_string())
    }
}

impl IntoZval for PHPValue {
    const TYPE: DataType = DataType::Mixed;

    fn set_zval(self, zv: &mut Zval, persistent: bool) -> Result<()> {
        match self {
            PHPValue::Nil => zv.set_null(),
            PHPValue::Bool(b) => zv.set_bool(b),
            PHPValue::Int(i) => zv.set_long(i),
            PHPValue::UInt(ui) => zv.set_long(ui as i64),
            PHPValue::Float(f) => zv.set_double(f),
            PHPValue::String(s) => zv.set_string(&s, persistent)?,
            // PHPValue::Blob(b) => zv.set_binary(b),
            PHPValue::List(l) => zv.set_array(l)?,
            PHPValue::Json(h) => {
                let mut arr = ZendHashTable::with_capacity(h.len() as u32);
                h.iter().for_each(|(k, v)| {
                    arr.insert::<PHPValue>(&k.to_string(), v.clone().into())
                        .expect("error converting hash");
                });

                zv.set_hashtable(arr)
            }
            PHPValue::HashMap(h) => {
                let mut arr = ZendHashTable::with_capacity(h.len() as u32);
                h.iter().for_each(|(k, v)| {
                    arr.insert::<PHPValue>(&k.to_string(), v.clone().into())
                        .expect("error converting hash");
                });

                zv.set_hashtable(arr)
            }
            PHPValue::GeoJSON(s) => {
                let geo = GeoJSON { v: s };
                let zo: ZBox<ZendObject> = geo.into_zend_object()?;
                zv.set_object(zo.into_raw());
            }
            PHPValue::Blob(b) => {
                let blob = BLOB { v: b };
                let zo: ZBox<ZendObject> = blob.into_zend_object()?;
                zv.set_object(zo.into_raw());
            }
            PHPValue::HLL(b) => {
                let hll = HLL { v: b };
                let zo: ZBox<ZendObject> = hll.into_zend_object()?;
                zv.set_object(zo.into_raw());
            }
            PHPValue::Infinity => {
                let inf = Infinity {};
                let zo: ZBox<ZendObject> = inf.into_zend_object()?;
                zv.set_object(zo.into_raw());
            }
            PHPValue::Wildcard => {
                let inf = Wildcard {};
                let zo: ZBox<ZendObject> = inf.into_zend_object()?;
                zv.set_object(zo.into_raw());
            }
        }

        Ok(())
    }
}

fn from_zval(zval: &Zval) -> Option<PHPValue> {
    match zval.get_type() {
        DataType::Object(_) => {
            if let Some(o) = zval.extract::<BLOB>() {
                return Some(PHPValue::Blob(o.v));
            } else if let Some(o) = zval.extract::<HLL>() {
                return Some(PHPValue::HLL(o.v));
            } else if let Some(o) = zval.extract::<GeoJSON>() {
                return Some(PHPValue::GeoJSON(o.v));
            } else if let Some(_) = zval.extract::<Infinity>() {
                return Some(PHPValue::Infinity);
            } else if let Some(_) = zval.extract::<Wildcard>() {
                return Some(PHPValue::Wildcard);
            }
            let error = AerospikeException::new("Invalid Object");
            let _ = throw_object(error.into_zval(true).unwrap());
            None
        }
        // DataType::Undef => Some(PHPValue::Nil),
        DataType::Null => Some(PHPValue::Nil),
        DataType::False => Some(PHPValue::Bool(false)),
        DataType::True => Some(PHPValue::Bool(true)),
        DataType::Bool => zval.bool().map(|v| PHPValue::Bool(v)),
        DataType::Long => zval.long().map(|v| PHPValue::Int(v)),
        DataType::Double => zval
            .double()
            .map(|v| PHPValue::Float(ordered_float::OrderedFloat(v))),
        DataType::String => zval.string().map(|v| PHPValue::String(v)),
        DataType::Array => {
            zval.array().map(|arr| {
                if arr.has_sequential_keys() {
                    // it's an array
                    let val_arr: Vec<PHPValue> =
                        arr.iter().map(|(_, v)| from_zval(v).unwrap()).collect();
                    PHPValue::List(val_arr)
                } else if arr.has_numerical_keys() {
                    // it's a hashmap with numerical keys
                    let mut h = HashMap::<PHPValue, PHPValue>::with_capacity(arr.len());
                    arr.iter().for_each(|(i, v)| match i {
                        ArrayKey::Long(index) => {
                            h.insert(PHPValue::UInt(index as u64), from_zval(v).unwrap());
                        }
                        ArrayKey::String(_) => {}
                    });
                    PHPValue::HashMap(h)
                } else {
                    // it's a hashmap with string keys
                    let mut h = HashMap::with_capacity(arr.len());
                    arr.iter().for_each(|(k, v)| match k {
                        ArrayKey::Long(_) => {}
                        ArrayKey::String(index) => {
                            h.insert(
                                PHPValue::String(index),
                                from_zval(v).expect("Invalid value in hashmap".into()),
                            );
                        }
                    });
                    PHPValue::HashMap(h)
                }
            })
        }
        _ => unreachable!(),
    }
}

impl FromZval<'_> for PHPValue {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        from_zval(zval)
    }
}

impl From<HashMap<String, proto::Value>> for PHPValue {
    fn from(h: HashMap<String, proto::Value>) -> Self {
        let mut hash = HashMap::<PHPValue, PHPValue>::with_capacity(h.len());
        h.iter().for_each(|(k, v)| {
            hash.insert(PHPValue::String(k.into()), (*v).clone().into());
        });
        PHPValue::HashMap(hash)
    }
}

impl From<HashMap<PHPValue, PHPValue>> for PHPValue {
    fn from(h: HashMap<PHPValue, PHPValue>) -> Self {
        PHPValue::HashMap(h)
    }
}

impl From<PHPValue> for proto::Value {
    fn from(other: PHPValue) -> Self {
        match other {
            PHPValue::Nil => proto::Value {
                v: Some(proto::value::V::Nil(true)),
            },
            PHPValue::Bool(b) => proto::Value {
                v: Some(proto::value::V::B(b)),
            },
            PHPValue::Int(i) => proto::Value {
                v: Some(proto::value::V::I(i)),
            },
            PHPValue::UInt(ui) => proto::Value {
                v: Some(proto::value::V::I(ui as i64)),
            },
            PHPValue::Float(f) => proto::Value {
                v: Some(proto::value::V::F(f64::from(f).into())),
            },
            PHPValue::String(s) => proto::Value {
                v: Some(proto::value::V::S(s)),
            },
            PHPValue::Blob(b) => proto::Value {
                v: Some(proto::value::V::Blob(b)),
            },
            PHPValue::List(l) => {
                let mut nl = Vec::<proto::Value>::with_capacity(l.len());
                l.iter().for_each(|v| nl.push(v.clone().into()));
                proto::Value {
                    v: Some(proto::value::V::L(proto::List { l: nl })),
                }
            }
            PHPValue::HashMap(h) => {
                let mut arr = Vec::with_capacity(h.len());
                h.iter().for_each(|(k, v)| {
                    arr.push(proto::MapEntry {
                        k: Some((*k).clone().into()),
                        v: Some((*v).clone().into()),
                    });
                });
                proto::Value {
                    v: Some(proto::value::V::M(proto::Map { m: arr })),
                }
            }
            PHPValue::Json(h) => {
                let mut arr = Vec::with_capacity(h.len());
                h.iter().for_each(|(k, v)| {
                    arr.push(proto::JsonEntry {
                        k: k.clone(),
                        v: Some((*v).clone().into()),
                    });
                });
                proto::Value {
                    v: Some(proto::value::V::Json(proto::Json { j: arr })),
                }
            }
            PHPValue::GeoJSON(gj) => proto::Value {
                v: Some(proto::value::V::Geo(gj)),
            },
            PHPValue::HLL(b) => proto::Value {
                v: Some(proto::value::V::Hll(b)),
            },
            PHPValue::Infinity => proto::Value {
                v: Some(proto::value::V::Infinity(true)),
            },
            PHPValue::Wildcard => proto::Value {
                v: Some(proto::value::V::Wildcard(true)),
            },
        }
    }
}

impl From<proto::Value> for PHPValue {
    fn from(other: proto::Value) -> Self {
        match other.v.unwrap() {
            proto::value::V::Nil(_) => PHPValue::Nil,
            proto::value::V::B(b) => PHPValue::Bool(b),
            proto::value::V::I(i) => PHPValue::Int(i),
            proto::value::V::F(f) => PHPValue::Float(ordered_float::OrderedFloat(f.into())),
            proto::value::V::S(s) => PHPValue::String(s.into()),
            proto::value::V::Blob(blob) => PHPValue::Blob(blob.to_vec()),
            proto::value::V::L(l) => {
                let mut nl = Vec::<PHPValue>::with_capacity(l.l.len());
                l.l.iter().for_each(|v| nl.push((*v).clone().into()));
                PHPValue::List(nl)
            }
            proto::value::V::Json(json) => {
                let mut arr = HashMap::<String, PHPValue>::with_capacity(json.j.len());
                json.j.iter().for_each(|me| {
                    arr.insert(me.k.clone(), (me.v.clone().unwrap()).into());
                });
                PHPValue::Json(arr)
            }
            proto::value::V::M(h) => {
                let mut arr = HashMap::<PHPValue, PHPValue>::with_capacity(h.m.len());
                h.m.iter().for_each(|me| {
                    if me.k.is_some() {
                        arr.insert(
                            (me.k.clone().unwrap()).into(),
                            (me.v.clone().unwrap()).into(),
                        );
                    };
                });
                PHPValue::HashMap(arr)
            }
            proto::value::V::Geo(gj) => PHPValue::GeoJSON(gj.into()),
            proto::value::V::Hll(b) => PHPValue::HLL(b.to_vec()),
            proto::value::V::Infinity(_) => PHPValue::Infinity,
            proto::value::V::Wildcard(_) => PHPValue::Wildcard,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Value
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\Value")]
pub struct Value;

/// Value interface is used to efficiently serialize objects into the wire protocol.
#[php_impl]
#[derive(ZvalConvert)]
impl Value {
    pub fn nil() -> PHPValue {
        PHPValue::Nil
    }

    pub fn int(val: i64) -> PHPValue {
        PHPValue::Int(val)
    }

    pub fn uint(val: u64) -> PHPValue {
        PHPValue::UInt(val)
    }

    pub fn float(val: f64) -> PHPValue {
        PHPValue::Float(ordered_float::OrderedFloat(val.into()))
    }

    pub fn bool(val: bool) -> PHPValue {
        PHPValue::Bool(val)
    }

    pub fn string(val: String) -> PHPValue {
        PHPValue::String(val)
    }

    pub fn list(val: Vec<PHPValue>) -> PHPValue {
        PHPValue::List(val)
    }

    pub fn map(val: &Zval) -> PHPValue {
        match from_zval(val) {
            Some(PHPValue::HashMap(hm)) => PHPValue::HashMap(hm),
            _ => {
                let error = AerospikeException::new("Invalid value".into());
                let _ = throw_object(error.into_zval(true).unwrap());
                PHPValue::Nil
            }
        }
    }

    pub fn blob(zval: &Zval) -> PhpResult<PHPValue> {
        match zval.get_type() {
            DataType::String => {
                if zval.binary::<u8>().is_some() {
                    Ok(zval.binary().map(|v| PHPValue::Blob(v.into())).unwrap())
                } else {
                    Ok(zval.string().map(|v| PHPValue::Blob(v.into())).unwrap())
                }
            }
            DataType::Array => {
                if let Some(arr) = zval.array() {
                    if arr.has_sequential_keys() {
                        // it's an array
                        let val_arr: Vec<u8> = arr
                            .iter()
                            .map(|(_, v)| match from_zval(v) {
                                Some(PHPValue::Int(b)) if b >= 0 && b <= 255 => b as u8,
                                Some(PHPValue::Int(b)) if b < 0 && b > 255 => {
                                    let msg = format!("Invalid value {} in array for Value::blob. Must be an array of integers [0, 255]", b);
                                    let error = AerospikeException::new(&msg);
                                    throw_object(error.into_zval(true).unwrap()).unwrap();
                                    0
                                },
                                _ => {
                                    let error = AerospikeException::new("Invalid array for Value::blob. Must be an array of integers [0, 255]");
                                    throw_object(error.into_zval(true).unwrap()).unwrap();
                                    0
                                },
                            })
                            .collect();
                        return Ok(PHPValue::Blob(val_arr));
                    }
                };
                return Err(format!(
                    "Invalid Array type for Value::blob. Must be an array of integers [0, 255]"
                )
                .into());
            }
            _ => return Err(format!("Nah").into()),
        }
    }

    pub fn geo_json(val: String) -> PHPValue {
        PHPValue::GeoJSON(val)
    }

    pub fn hll(val: Vec<u8>) -> PHPValue {
        PHPValue::HLL(val)
    }

    pub fn json(val: HashMap<String, PHPValue>) -> PHPValue {
        PHPValue::Json(val)
    }

    pub fn infinity() -> PHPValue {
        PHPValue::Infinity
    }

    pub fn wildcard() -> PHPValue {
        PHPValue::Wildcard
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Converters
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of conversion traits for interoperability with the `proto` module.
impl From<&proto::Key> for Key {
    fn from(other: &proto::Key) -> Self {
        Key { _as: other.clone() }
    }
}

impl From<&proto::Record> for Record {
    fn from(other: &proto::Record) -> Self {
        Record { _as: other.clone() }
    }
}

impl From<&Bin> for proto::Bin {
    fn from(other: &Bin) -> Self {
        other._as.clone()
    }
}

#[derive(Debug)]
pub struct AeroPHPError(String);

impl std::fmt::Display for AeroPHPError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[allow(non_camel_case_types)]
////////////////////////////////////////////////////////////////////////////////////////////
//
// ResultCode
//
////////////////////////////////////////////////////////////////////////////////////////////

/// ResultCode signifies the database operation error codes.
/// The positive numbers align with the server side file kvs.h.
#[php_class(name = "Aerospike\\ResultCode")]
struct ResultCode {}

#[php_impl]
#[derive(ZvalConvert)]
impl ResultCode {
    /// GRPC_ERROR is wrapped and directly returned from the grpc library
    const GRPC_ERROR: i32 = -21;

    /// BATCH_FAILED means one or more keys failed in a batch.
    const BATCH_FAILED: i32 = -20;

    /// NO_RESPONSE means no response was received from the server.
    const NO_RESPONSE: i32 = -19;

    /// NETWORK_ERROR defines a network error. Checked the wrapped error for detail.
    const NETWORK_ERROR: i32 = -18;

    /// COMMON_ERROR defines a common, none-aerospike error. Checked the wrapped error for detail.
    const COMMON_ERROR: i32 = -17;

    /// MAX_RETRIES_EXCEEDED defines max retries limit reached.
    const MAX_RETRIES_EXCEEDED: i32 = -16;

    /// MAX_ERROR_RATE defines max errors limit reached.
    const MAX_ERROR_RATE: i32 = -15;

    /// RACK_NOT_DEFINED defines requested Rack for node/namespace was not defined in the cluster.
    const RACK_NOT_DEFINED: i32 = -13;

    /// INVALID_CLUSTER_PARTITION_MAP defines cluster has an invalid partition map, usually due to bad configuration.
    const INVALID_CLUSTER_PARTITION_MAP: i32 = -12;

    /// SERVER_NOT_AVAILABLE defines server is not accepting requests.
    const SERVER_NOT_AVAILABLE: i32 = -11;

    /// CLUSTER_NAME_MISMATCH_ERROR defines cluster Name does not match the ClientPolicy.ClusterName value.
    const CLUSTER_NAME_MISMATCH_ERROR: i32 = -10;

    /// RECORDSET_CLOSED defines recordset has already been closed or cancelled
    const RECORDSET_CLOSED: i32 = -9;

    /// NO_AVAILABLE_CONNECTIONS_TO_NODE defines there were no connections available to the node in the pool, and the pool was limited
    const NO_AVAILABLE_CONNECTIONS_TO_NODE: i32 = -8;

    /// TYPE_NOT_SUPPORTED defines data type is not supported by aerospike server.
    const TYPE_NOT_SUPPORTED: i32 = -7;

    /// COMMAND_REJECTED defines info Command was rejected by the server.
    const COMMAND_REJECTED: i32 = -6;

    /// QUERY_TERMINATED defines query was terminated by user.
    const QUERY_TERMINATED: i32 = -5;

    /// SCAN_TERMINATED defines scan was terminated by user.
    const SCAN_TERMINATED: i32 = -4;

    /// INVALID_NODE_ERROR defines chosen node is not currently active.
    const INVALID_NODE_ERROR: i32 = -3;

    /// PARSE_ERROR defines client parse error.
    const PARSE_ERROR: i32 = -2;

    /// SERIALIZE_ERROR defines client serialization error.
    const SERIALIZE_ERROR: i32 = -1;

    /// OK defines operation was successful.
    const OK: i32 = 0;

    /// SERVER_ERROR defines unknown server failure.
    const SERVER_ERROR: i32 = 1;

    /// KEY_NOT_FOUND_ERROR defines on retrieving, touching or replacing a record that doesn't exist.
    const KEY_NOT_FOUND_ERROR: i32 = 2;

    /// GENERATION_ERROR defines on modifying a record with unexpected generation.
    const GENERATION_ERROR: i32 = 3;

    /// PARAMETER_ERROR defines bad parameter(s) were passed in database operation call.
    const PARAMETER_ERROR: i32 = 4;

    /// KEY_EXISTS_ERROR defines on create-only (write unique) operations on a record that already exists.
    const KEY_EXISTS_ERROR: i32 = 5;

    /// BIN_EXISTS_ERROR defines bin already exists on a create-only operation.
    const BIN_EXISTS_ERROR: i32 = 6;

    /// CLUSTER_KEY_MISMATCH defines expected cluster ID was not received.
    const CLUSTER_KEY_MISMATCH: i32 = 7;

    /// SERVER_MEM_ERROR defines server has run out of memory.
    const SERVER_MEM_ERROR: i32 = 8;

    /// TIMEOUT defines client or server has timed out.
    const TIMEOUT: i32 = 9;

    /// ALWAYS_FORBIDDEN defines operation not allowed in current configuration.
    const ALWAYS_FORBIDDEN: i32 = 10;

    /// PARTITION_UNAVAILABLE defines partition is unavailable.
    const PARTITION_UNAVAILABLE: i32 = 11;

    /// BIN_TYPE_ERROR defines operation is not supported with configured bin type (single-bin or multi-bin);
    const BIN_TYPE_ERROR: i32 = 12;

    /// RECORD_TOO_BIG defines record size exceeds limit.
    const RECORD_TOO_BIG: i32 = 13;

    /// KEY_BUSY defines too many concurrent operations on the same record.
    const KEY_BUSY: i32 = 14;

    /// SCAN_ABORT defines scan aborted by server.
    const SCAN_ABORT: i32 = 15;

    /// UNSUPPORTED_FEATURE defines unsupported Server Feature (e.g. Scan + UDF)
    const UNSUPPORTED_FEATURE: i32 = 16;

    /// BIN_NOT_FOUND defines bin not found on update-only operation.
    const BIN_NOT_FOUND: i32 = 17;

    /// DEVICE_OVERLOAD defines device not keeping up with writes.
    const DEVICE_OVERLOAD: i32 = 18;

    /// KEY_MISMATCH defines key type mismatch.
    const KEY_MISMATCH: i32 = 19;

    /// INVALID_NAMESPACE defines invalid namespace.
    const INVALID_NAMESPACE: i32 = 20;

    /// BIN_NAME_TOO_LONG defines bin name length greater than 14 characters, or maximum number of unique bin names are exceeded;
    const BIN_NAME_TOO_LONG: i32 = 21;

    /// FAIL_FORBIDDEN defines operation not allowed at this time.
    const FAIL_FORBIDDEN: i32 = 22;

    /// FAIL_ELEMENT_NOT_FOUND defines element Not Found in CDT
    const FAIL_ELEMENT_NOT_FOUND: i32 = 23;

    /// FAIL_ELEMENT_EXISTS defines element Already Exists in CDT
    const FAIL_ELEMENT_EXISTS: i32 = 24;

    /// ENTERPRISE_ONLY defines attempt to use an Enterprise feature on a Community server or a server without the applicable feature key;
    const ENTERPRISE_ONLY: i32 = 25;

    /// OP_NOT_APPLICABLE defines the operation cannot be applied to the current bin value on the server.
    const OP_NOT_APPLICABLE: i32 = 26;

    /// FILTERED_OUT defines the transaction was not performed because the filter was false.
    const FILTERED_OUT: i32 = 27;

    /// LOST_CONFLICT defines write command loses conflict to XDR.
    const LOST_CONFLICT: i32 = 28;

    /// QUERY_END defines there are no more records left for query.
    const QUERY_END: i32 = 50;

    /// SECURITY_NOT_SUPPORTED defines security type not supported by connected server.
    const SECURITY_NOT_SUPPORTED: i32 = 51;

    /// SECURITY_NOT_ENABLED defines administration command is invalid.
    const SECURITY_NOT_ENABLED: i32 = 52;

    /// SECURITY_SCHEME_NOT_SUPPORTED defines administration field is invalid.
    const SECURITY_SCHEME_NOT_SUPPORTED: i32 = 53;

    /// INVALID_COMMAND defines administration command is invalid.
    const INVALID_COMMAND: i32 = 54;

    /// INVALID_FIELD defines administration field is invalid.
    const INVALID_FIELD: i32 = 55;

    /// ILLEGAL_STATE defines security protocol not followed.
    const ILLEGAL_STATE: i32 = 56;

    /// INVALID_USER defines user name is invalid.
    const INVALID_USER: i32 = 60;

    /// USER_ALREADY_EXISTS defines user was previously created.
    const USER_ALREADY_EXISTS: i32 = 61;

    /// INVALID_PASSWORD defines password is invalid.
    const INVALID_PASSWORD: i32 = 62;

    /// EXPIRED_PASSWORD defines security credential is invalid.
    const EXPIRED_PASSWORD: i32 = 63;

    /// FORBIDDEN_PASSWORD defines forbidden password (e.g. recently used)
    const FORBIDDEN_PASSWORD: i32 = 64;

    /// INVALID_CREDENTIAL defines security credential is invalid.
    const INVALID_CREDENTIAL: i32 = 65;

    /// EXPIRED_SESSION defines login session expired.
    const EXPIRED_SESSION: i32 = 66;

    /// INVALID_ROLE defines role name is invalid.
    const INVALID_ROLE: i32 = 70;

    /// ROLE_ALREADY_EXISTS defines role already exists.
    const ROLE_ALREADY_EXISTS: i32 = 71;

    /// INVALID_PRIVILEGE defines privilege is invalid.
    const INVALID_PRIVILEGE: i32 = 72;

    /// INVALID_WHITELIST defines invalid IP address whiltelist
    const INVALID_WHITELIST: i32 = 73;

    /// QUOTAS_NOT_ENABLED defines Quotas not enabled on server.
    const QUOTAS_NOT_ENABLED: i32 = 74;

    /// INVALID_QUOTA defines invalid quota value.
    const INVALID_QUOTA: i32 = 75;

    /// NOT_AUTHENTICATED defines user must be authentication before performing database operations.
    const NOT_AUTHENTICATED: i32 = 80;

    /// ROLE_VIOLATION defines user does not posses the required role to perform the database operation.
    const ROLE_VIOLATION: i32 = 81;

    /// NOT_WHITELISTED defines command not allowed because sender IP address not whitelisted.
    const NOT_WHITELISTED: i32 = 82;

    /// QUOTA_EXCEEDED defines Quota exceeded.
    const QUOTA_EXCEEDED: i32 = 83;

    /// UDF_BAD_RESPONSE defines a user defined function returned an error code.
    const UDF_BAD_RESPONSE: i32 = 100;

    /// BATCH_DISABLED defines batch functionality has been disabled.
    const BATCH_DISABLED: i32 = 150;

    /// BATCH_MAX_REQUESTS_EXCEEDED defines batch max requests have been exceeded.
    const BATCH_MAX_REQUESTS_EXCEEDED: i32 = 151;

    /// BATCH_QUEUES_FULL defines all batch queues are full.
    const BATCH_QUEUES_FULL: i32 = 152;

    /// GEO_INVALID_GEOJSON defines invalid GeoJSON on insert/update
    const GEO_INVALID_GEOJSON: i32 = 160;

    /// INDEX_FOUND defines secondary index already exists.
    const INDEX_FOUND: i32 = 200;

    /// INDEX_NOTFOUND defines requested secondary index does not exist.
    const INDEX_NOT_FOUND: i32 = 201;

    /// INDEX_OOM defines secondary index memory space exceeded.
    const INDEX_OOM: i32 = 202;

    /// INDEX_NOTREADABLE defines secondary index not available.
    const INDEX_NOT_READABLE: i32 = 203;

    /// INDEX_GENERIC defines generic secondary index error.
    const INDEX_GENERIC: i32 = 204;

    /// INDEX_NAME_MAXLEN defines index name maximum length exceeded.
    const INDEX_NAME_MAX_LEN: i32 = 205;

    /// INDEX_MAXCOUNT defines maximum number of indexes exceeded.
    const INDEX_MAX_COUNT: i32 = 206;

    /// QUERY_ABORTED defines secondary index query aborted.
    const QUERY_ABORTED: i32 = 210;

    /// QUERY_QUEUEFULL defines secondary index queue full.
    const QUERY_QUEUE_FULL: i32 = 211;

    /// QUERY_TIMEOUT defines secondary index query timed out on server.
    const QUERY_TIMEOUT: i32 = 212;

    /// QUERY_GENERIC defines generic query error.
    const QUERY_GENERIC: i32 = 213;

    /// QUERY_NETIO_ERR defines query NetIO error on server
    const QUERY_NET_IO_ERR: i32 = 214;

    /// QUERY_DUPLICATE defines duplicate TaskId sent for the statement
    const QUERY_DUPLICATE: i32 = 215;

    /// AEROSPIKE_ERR_UDF_NOT_FOUND defines UDF does not exist.
    const AEROSPIKE_ERR_UDF_NOT_FOUND: i32 = 1301;

    /// AEROSPIKE_ERR_LUA_FILE_NOT_FOUND defines LUA file does not exist.
    const AEROSPIKE_ERR_LUA_FILE_NOT_FOUND: i32 = 1302;

    pub fn to_string(code: i32) -> String {
        match code {
             ResultCode::GRPC_ERROR => "wrapped and directly returned from the grpc library".into(),
             ResultCode::BATCH_FAILED => "one or more keys failed in a batch".into(),
             ResultCode::NO_RESPONSE => "no response was received from the server".into(),
             ResultCode::NETWORK_ERROR => "a network error. Checked the wrapped error for detail".into(),
             ResultCode::COMMON_ERROR => "a common, none-aerospike error. Checked the wrapped error for detail".into(),
             ResultCode::MAX_RETRIES_EXCEEDED => "max retries limit reached".into(),
             ResultCode::MAX_ERROR_RATE => "max errors limit reached".into(),
             ResultCode::RACK_NOT_DEFINED => "requested Rack for node/namespace was not defined in the cluster".into(),
             ResultCode::INVALID_CLUSTER_PARTITION_MAP => "cluster has an invalid partition map, usually due to bad configuration".into(),
             ResultCode::SERVER_NOT_AVAILABLE => "server is not accepting requests".into(),
             ResultCode::CLUSTER_NAME_MISMATCH_ERROR => "cluster Name does not match the ClientPolicy.ClusterName value".into(),
             ResultCode::RECORDSET_CLOSED=> "recordset has already been closed or cancelled".into(),
             ResultCode::NO_AVAILABLE_CONNECTIONS_TO_NODE=> "there were no connections available to the node in the pool, and the pool was limited".into(),
             ResultCode::TYPE_NOT_SUPPORTED=> "data type is not supported by aerospike server".into(),
             ResultCode::COMMAND_REJECTED=> "info Command was rejected by the server".into(),
             ResultCode::QUERY_TERMINATED=> "query was terminated by user".into(),
             ResultCode::SCAN_TERMINATED=> "scan was terminated by user".into(),
             ResultCode::INVALID_NODE_ERROR=> "chosen node is not currently active".into(),
             ResultCode::PARSE_ERROR=> "client parse error".into(),
             ResultCode::SERIALIZE_ERROR=> "client serialization error".into(),
             ResultCode::OK=> "operation was successful".into(),
             ResultCode::SERVER_ERROR=> "unknown server failure".into(),
             ResultCode::KEY_NOT_FOUND_ERROR=> "on retrieving, touching or replacing a record that doesn't exist".into(),
             ResultCode::GENERATION_ERROR=> "on modifying a record with unexpected generation".into(),
             ResultCode::PARAMETER_ERROR=> "bad parameter(s) were passed in database operation call".into(),
             ResultCode::KEY_EXISTS_ERROR=> "on create-only (write unique) operations on a record that already exists".into(),
             ResultCode::BIN_EXISTS_ERROR=> "bin already exists on a create-only operation".into(),
             ResultCode::CLUSTER_KEY_MISMATCH=> "expected cluster ID was not received".into(),
             ResultCode::SERVER_MEM_ERROR=> "server has run out of memory".into(),
             ResultCode::TIMEOUT=> "client or server has timed out".into(),
             ResultCode::ALWAYS_FORBIDDEN=> "operation not allowed in current configuration".into(),
             ResultCode::PARTITION_UNAVAILABLE=> "partition is unavailable".into(),
             ResultCode::BIN_TYPE_ERROR=> "operation is not supported with configured bin type (single-bin or multi-bin)".into(),
             ResultCode::RECORD_TOO_BIG=> "record size exceeds limit".into(),
             ResultCode::KEY_BUSY=> "too many concurrent operations on the same record".into(),
             ResultCode::SCAN_ABORT=> "scan aborted by server".into(),
             ResultCode::UNSUPPORTED_FEATURE=> "unsupported Server Feature (e.g. Scan + UDF)".into(),
             ResultCode::BIN_NOT_FOUND=> "bin not found on update-only operation".into(),
             ResultCode::DEVICE_OVERLOAD=> "device not keeping up with writes".into(),
             ResultCode::KEY_MISMATCH=> "key type mismatch".into(),
             ResultCode::INVALID_NAMESPACE=> "invalid namespace".into(),
             ResultCode::BIN_NAME_TOO_LONG=> "bin name length greater than 14 characters, or maximum number of unique bin names are exceeded".into(),
             ResultCode::FAIL_FORBIDDEN=> "operation not allowed at this time".into(),
             ResultCode::FAIL_ELEMENT_NOT_FOUND=> "element Not Found in CDT".into(),
             ResultCode::FAIL_ELEMENT_EXISTS=> "element Already Exists in CDT".into(),
             ResultCode::ENTERPRISE_ONLY=> "attempt to use an Enterprise feature on a Community server or a server without the applicable feature key".into(),
             ResultCode::OP_NOT_APPLICABLE=> "the operation cannot be applied to the current bin value on the server".into(),
             ResultCode::FILTERED_OUT=> "the transaction was not performed because the filter was false".into(),
             ResultCode::LOST_CONFLICT=> "write command loses conflict to XDR".into(),
             ResultCode::QUERY_END=> "there are no more records left for query".into(),
             ResultCode::SECURITY_NOT_SUPPORTED=> "security type not supported by connected server".into(),
             ResultCode::SECURITY_NOT_ENABLED=> "administration command is invalid".into(),
             ResultCode::SECURITY_SCHEME_NOT_SUPPORTED=> "administration field is invalid".into(),
             ResultCode::INVALID_COMMAND=> "administration command is invalid".into(),
             ResultCode::INVALID_FIELD=> "administration field is invalid".into(),
             ResultCode::ILLEGAL_STATE=> "security protocol not followed".into(),
             ResultCode::INVALID_USER=> "user name is invalid".into(),
             ResultCode::USER_ALREADY_EXISTS=> "user was previously created".into(),
             ResultCode::INVALID_PASSWORD=> "password is invalid".into(),
             ResultCode::EXPIRED_PASSWORD=> "security credential is invalid".into(),
             ResultCode::FORBIDDEN_PASSWORD=> "forbidden password (e.g. recently used)".into(),
             ResultCode::INVALID_CREDENTIAL=> "security credential is invalid".into(),
             ResultCode::EXPIRED_SESSION=> "login session expired".into(),
             ResultCode::INVALID_ROLE=> "role name is invalid".into(),
             ResultCode::ROLE_ALREADY_EXISTS=> "role already exists".into(),
             ResultCode::INVALID_PRIVILEGE=> "privilege is invalid".into(),
             ResultCode::INVALID_WHITELIST=> "invalid IP address whiltelist".into(),
             ResultCode::QUOTAS_NOT_ENABLED=> "Quotas not enabled on server".into(),
             ResultCode::INVALID_QUOTA=> "invalid quota value".into(),
             ResultCode::NOT_AUTHENTICATED=> "user must be authentication before performing database operations".into(),
             ResultCode::ROLE_VIOLATION=> "user does not posses the required role to perform the database operation".into(),
             ResultCode::NOT_WHITELISTED=> "command not allowed because sender IP address not whitelisted".into(),
             ResultCode::QUOTA_EXCEEDED=> "Quota exceeded".into(),
             ResultCode::UDF_BAD_RESPONSE => "a user defined function returned an error code".into(),
             ResultCode::BATCH_DISABLED => "batch functionality has been disabled".into(),
             ResultCode::BATCH_MAX_REQUESTS_EXCEEDED => "batch max requests have been exceeded".into(),
             ResultCode::BATCH_QUEUES_FULL => "all batch queues are full".into(),
             ResultCode::GEO_INVALID_GEOJSON => "invalid GeoJSON on insert/update".into(),
             ResultCode::INDEX_FOUND => "secondary index already exists".into(),
             ResultCode::INDEX_NOT_FOUND => "requested secondary index does not exist".into(),
             ResultCode::INDEX_OOM => "secondary index memory space exceeded".into(),
             ResultCode::INDEX_NOT_READABLE => "secondary index not available".into(),
             ResultCode::INDEX_GENERIC => "generic secondary index error".into(),
             ResultCode::INDEX_NAME_MAX_LEN => "index name maximum length exceeded".into(),
             ResultCode::INDEX_MAX_COUNT => "maximum number of indexes exceeded".into(),
             ResultCode::QUERY_ABORTED => "secondary index query aborted".into(),
             ResultCode::QUERY_QUEUE_FULL => "secondary index queue full".into(),
             ResultCode::QUERY_TIMEOUT => "secondary index query timed out on server".into(),
             ResultCode::QUERY_GENERIC => "generic query error".into(),
             ResultCode::QUERY_NET_IO_ERR => "query NetIO error on server".into(),
             ResultCode::QUERY_DUPLICATE => "duplicate TaskId sent for the statement".into(),
             ResultCode::AEROSPIKE_ERR_UDF_NOT_FOUND => "UDF does not exist".into(),
             ResultCode::AEROSPIKE_ERR_LUA_FILE_NOT_FOUND => "LUA file does not exist".into(),
             _ => "Unknown Error".into()
             }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  utility methods
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Utility methods
fn assert_map(val: &PHPValue) -> bool {
    match val {
        PHPValue::HashMap(_) => true,
        _ => {
            let error = AerospikeException::new("Invalid type");
            throw_object(error.into_zval(true).unwrap()).unwrap();
            false
        }
    }
}

fn assert_hll_list(val: &Vec<PHPValue>) -> bool {
    // try to find a non-hll value
    val.iter()
        .find(|val| match val {
            PHPValue::HLL(_) => false,
            _ => {
                let error = AerospikeException::new("Invalid type");
                throw_object(error.into_zval(true).unwrap()).unwrap();
                return true;
            }
        })
        .is_none()
}

fn persist_client(key: &str, c: Arc<Mutex<grpc::BlockingClient>>) -> Result<()> {
    trace!("Persisting Client pointer: {:p}", &c);
    let mut clients = CLIENTS.lock().unwrap();
    clients.insert(key.into(), c);
    Ok(())
}

fn get_persisted_client(key: &str) -> Option<Zval> {
    let clients = CLIENTS.lock().unwrap();
    let grpc_client = clients.get(key.into())?;
    let client = Client {
        client: grpc_client.clone(),
        socket: key.into(),
    };

    let mut zval = Zval::new();
    let zo: ZBox<ZendObject> = client.into_zend_object().ok()?;
    zval.set_object(zo.into_raw());
    Some(zval)
}

/// Used by the `phpinfo()` function and when you run `php -i`.
/// This will probably be simplified with another macro eventually!
pub extern "C" fn php_module_info(_module: *mut ModuleEntry) {
    info_table_start!();
    info_table_row!("Aerospike Client PHP (IPC)", "enabled");
    info_table_end!();
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
}
