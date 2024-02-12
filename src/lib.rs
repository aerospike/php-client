/*
 * Copyright 2012-2023 Aerospike, Inc.
 *
 * Portions may be licensed to Aerospike, Inc. under one or more contributor
 * license agreements WHICH ARE COMPATIBLE WITH THE APACHE LICENSE, VERSION 2.0.
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not
 * use this file except in compliance with the License. You may obtain a copy of
 * the License at http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations under
 * the License.
 */

#![cfg_attr(windows, feature(abi_vectorcall))]

mod grpc;

use grpc::proto;

use ext_php_rs::prelude::*;

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use ext_php_rs::boxed::ZBox;
use ext_php_rs::convert::IntoZendObject;
use ext_php_rs::convert::{FromZval, FromZvalMut, IntoZval};
use ext_php_rs::error::Result;
use ext_php_rs::flags::DataType;
use ext_php_rs::php_class;
use ext_php_rs::types::ZendHashTable;
use ext_php_rs::types::ZendObject;
use ext_php_rs::types::Zval;
use ext_php_rs::exception::throw_object;
use ext_php_rs::exception::throw_with_code;
use ext_php_rs::zend::{ce, ClassEntry};
use ext_php_rs::types::{ArrayKey};

use chrono::Local;
use colored::*;
use lazy_static::lazy_static;
use log::LevelFilter;
use log::{debug, info, trace, warn};

lazy_static! {
    static ref CLIENTS: Mutex<HashMap<String, Arc<Mutex<grpc::BlockingClient>>>> =
        Mutex::new(HashMap::new());
}

pub type AspResult<T = ()> = std::result::Result<T, AspException>;


////////////////////////////////////////////////////////////////////////////////////////////
//
//  ExpressionType (ExpType)
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Expression Data Types for usage in some `FilterExpressions`
// #[derive(Debug, Clone, Copy)]
// pub enum _ExpType {
//     NIL,
//     BOOL,
//     INT,
//     STRING,
//     LIST,
//     MAP,
//     BLOB,
//     FLOAT,
//     GEO,
//     HLL,
// }

// #[php_class(name = "Aerospike\\ExpType")]
// pub struct ExpType {
//     v: _ExpType,
// }

// impl FromZval<'_> for ExpType {
//     const TYPE: DataType = DataType::Mixed;

//     fn from_zval(zval: &Zval) -> Option<Self> {
//         let f: &ExpType = zval.extract()?;

//         Some(ExpType {
//             _as: f._as.clone(),
//             v: f.v.clone(),
//         })
//     }
// }

// #[php_impl]
// #[derive(ZvalConvert)]
// impl ExpType {
//     pub fn nil() -> Self {
//         ExpType { v: _ExpType::NIL }
//     }

//     pub fn bool() -> Self {
//         ExpType { v: _ExpType::BOOL }
//     }

//     pub fn int() -> Self {
//         ExpType { v: _ExpType::INT }
//     }

//     pub fn string() -> Self {
//         ExpType {
//             v: _ExpType::STRING,
//         }
//     }

//     pub fn list() -> Self {
//         ExpType { v: _ExpType::LIST }
//     }

//     pub fn map() -> Self {
//         ExpType { v: _ExpType::MAP }
//     }

//     pub fn blob() -> Self {
//         ExpType { v: _ExpType::BLOB }
//     }

//     pub fn float() -> Self {
//         ExpType { v: _ExpType::FLOAT }
//     }

//     pub fn geo() -> Self {
//         ExpType { v: _ExpType::GEO }
//     }

//     pub fn hll() -> Self {
//         ExpType { v: _ExpType::HLL }
//     }
// }

// impl From<&ExpType> for aerospike_core::expressions::ExpType {
//     fn from(input: &ExpType) -> Self {
//         match &input.v {
//             _ExpType::NIL => aerospike_core::expressions::ExpType::NIL,
//             _ExpType::BOOL => aerospike_core::expressions::ExpType::BOOL,
//             _ExpType::INT => aerospike_core::expressions::ExpType::INT,
//             _ExpType::STRING => aerospike_core::expressions::ExpType::STRING,
//             _ExpType::LIST => aerospike_core::expressions::ExpType::LIST,
//             _ExpType::MAP => aerospike_core::expressions::ExpType::MAP,
//             _ExpType::BLOB => aerospike_core::expressions::ExpType::BLOB,
//             _ExpType::FLOAT => aerospike_core::expressions::ExpType::FLOAT,
//             _ExpType::GEO => aerospike_core::expressions::ExpType::GEO,
//             _ExpType::HLL => aerospike_core::expressions::ExpType::HLL,
//         }
//     }
// }

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Filter Expression
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Filter expression, which can be applied to most commands, to control which records are
/// affected by the command.
// #[php_class(name = "Aerospike\\FilterExpression")]
// pub struct FilterExpression {
//     _as: aerospike_core::expressions::FilterExpression,
// }

// impl FromZval<'_> for FilterExpression {
//     const TYPE: DataType = DataType::Mixed;

//     fn from_zval(zval: &Zval) -> Option<Self> {
//         let f: &FilterExpression = zval.extract()?;

//         Some(FilterExpression { _as: f._as.clone() })
//     }
// }

// #[php_impl]
// #[derive(ZvalConvert)]
// impl FilterExpression {
//     /// Create a record key expression of specified type.
//     pub fn key(exp_type: ExpType) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::key(exp_type._as),
//         }
//     }

//     /// Create function that returns if the primary key is stored in the record meta data
//     /// as a boolean expression. This would occur when `send_key` is true on record write.
//     pub fn key_exists() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::key_exists(),
//         }
//     }

//     /// Create 64 bit int bin expression.
//     pub fn int_bin(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_bin(name),
//         }
//     }

//     /// Create string bin expression.
//     pub fn string_bin(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::string_bin(name),
//         }
//     }

//     /// Create blob bin expression.
//     pub fn blob_bin(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::blob_bin(name),
//         }
//     }

//     /// Create 64 bit float bin expression.
//     pub fn float_bin(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::float_bin(name),
//         }
//     }

//     /// Create geo bin expression.
//     pub fn geo_bin(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::geo_bin(name),
//         }
//     }

//     /// Create list bin expression.
//     pub fn list_bin(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::list_bin(name),
//         }
//     }

//     /// Create map bin expression.
//     pub fn map_bin(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::map_bin(name),
//         }
//     }

//     /// Create a HLL bin expression
//     pub fn hll_bin(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::hll_bin(name),
//         }
//     }

//     /// Create function that returns if bin of specified name exists.
//     pub fn bin_exists(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::ne(
//                 aerospike_core::expressions::bin_type(name),
//                 aerospike_core::expressions::int_val(0 as i64),
//             ),
//         }
//     }

//     /// Create function that returns bin's integer particle type.
//     pub fn bin_type(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::bin_type(name),
//         }
//     }

//     /// Create function that returns record set name string.
//     pub fn set_name() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::set_name(),
//         }
//     }

//     /// Create function that returns record size on disk.
//     /// If server storage-engine is memory, then zero is returned.
//     pub fn device_size() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::device_size(),
//         }
//     }

//     /// Create function that returns record last update time expressed as 64 bit integer
//     /// nanoseconds since 1970-01-01 epoch.
//     pub fn last_update() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::last_update(),
//         }
//     }

//     /// Create expression that returns milliseconds since the record was last updated.
//     /// This expression usually evaluates quickly because record meta data is cached in memory.
//     pub fn since_update() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::since_update(),
//         }
//     }

//     /// Create function that returns record expiration time expressed as 64 bit integer
//     /// nanoseconds since 1970-01-01 epoch.
//     pub fn void_time() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::void_time(),
//         }
//     }

//     /// Create function that returns record expiration time (time to live) in integer seconds.
//     pub fn ttl() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::ttl(),
//         }
//     }

//     /// Create expression that returns if record has been deleted and is still in tombstone state.
//     /// This expression usually evaluates quickly because record meta data is cached in memory.
//     pub fn is_tombstone() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::is_tombstone(),
//         }
//     }

//     /// Create function that returns record digest modulo as integer.
//     pub fn digest_modulo(modulo: i64) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::digest_modulo(modulo),
//         }
//     }

//     /// Create function like regular expression string operation.
//     pub fn regex_compare(regex: String, flags: i64, bin: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::regex_compare(regex, flags, bin._as),
//         }
//     }

//     /// Create compare geospatial operation.
//     pub fn geo_compare(left: FilterExpression, right: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::geo_compare(left._as, right._as),
//         }
//     }

//     /// Creates 64 bit integer value
//     pub fn int_val(val: i64) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_val(val),
//         }
//     }

//     /// Creates a Boolean value
//     pub fn bool_val(val: bool) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::bool_val(val),
//         }
//     }

//     /// Creates String bin value
//     pub fn string_val(val: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::string_val(val),
//         }
//     }

//     /// Creates 64 bit float bin value
//     pub fn float_val(val: f64) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::float_val(val),
//         }
//     }

//     /// Creates Blob bin value
//     pub fn blob_val(val: Vec<u8>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::blob_val(val),
//         }
//     }

//     /// Create List bin PHPValue
//     /// Not Supported in pre-alpha release
//     // pub fn list_val(val: Vec<PHPValue>) -> Self {
//     //     FilterExpression {
//     //         _as: aerospike_core::expressions::list_val(val)
//     //     }
//     // }

//     /// Create Map bin PHPValue
//     /// Not Supported in pre-alpha release
//     // pub fn map_val(val: HashMap<PHPValue, PHPValue>) -> Self {
//     //     FilterExpression {
//     //         _as: aerospike_core::expressions::map_val(val)
//     //     }
//     // }

//     /// Create geospatial json string value.
//     pub fn geo_val(val: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::geo_val(val),
//         }
//     }

//     /// Create a Nil PHPValue
//     pub fn nil() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::nil(),
//         }
//     }

//     /// Create "not" operator expression.
//     pub fn not(exp: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::not(exp._as),
//         }
//     }

//     /// Create "and" (&&) operator that applies to a variable number of expressions.
//     /// // (a > 5 || a == 0) && b < 3
//     pub fn and(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::and(exps.into_iter().map(|exp| exp._as).collect()),
//         }
//     }

//     /// Create "or" (||) operator that applies to a variable number of expressions.
//     pub fn or(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::or(exps.into_iter().map(|exp| exp._as).collect()),
//         }
//     }

//     /// Create "xor" (^) operator that applies to a variable number of expressions.
//     pub fn xor(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::xor(exps.into_iter().map(|exp| exp._as).collect()),
//         }
//     }

//     /// Create equal (==) expression.
//     pub fn eq(left: FilterExpression, right: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::eq(left._as, right._as),
//         }
//     }

//     /// Create not equal (!=) expression
//     pub fn ne(left: FilterExpression, right: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::ne(left._as, right._as),
//         }
//     }

//     /// Create greater than (>) operation.
//     pub fn gt(left: FilterExpression, right: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::gt(left._as, right._as),
//         }
//     }

//     /// Create greater than or equal (>=) operation.
//     pub fn ge(left: FilterExpression, right: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::ge(left._as, right._as),
//         }
//     }

//     /// Create less than (<) operation.
//     pub fn lt(left: FilterExpression, right: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::lt(left._as, right._as),
//         }
//     }

//     /// Create less than or equals (<=) operation.
//     pub fn le(left: FilterExpression, right: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::le(left._as, right._as),
//         }
//     }

//     /// Create "add" (+) operator that applies to a variable number of expressions.
//     /// Return sum of all `FilterExpressions` given. All arguments must resolve to the same type (integer or float).
//     /// Requires server version 5.6.0+.
//     pub fn num_add(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_add(
//                 exps.into_iter().map(|exp| exp._as).collect(),
//             ),
//         }
//     }

//     /// Create "subtract" (-) operator that applies to a variable number of expressions.
//     /// If only one `FilterExpressions` is provided, return the negation of that argument.
//     /// Otherwise, return the sum of the 2nd to Nth `FilterExpressions` subtracted from the 1st
//     /// `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
//     /// Requires server version 5.6.0+.
//     pub fn num_sub(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_sub(
//                 exps.into_iter().map(|exp| exp._as).collect(),
//             ),
//         }
//     }

//     /// Create "multiply" (*) operator that applies to a variable number of expressions.
//     /// Return the product of all `FilterExpressions`. If only one `FilterExpressions` is supplied, return
//     /// that `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
//     /// Requires server version 5.6.0+.
//     pub fn num_mul(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_mul(
//                 exps.into_iter().map(|exp| exp._as).collect(),
//             ),
//         }
//     }

//     /// Create "divide" (/) operator that applies to a variable number of expressions.
//     /// If there is only one `FilterExpressions`, returns the reciprocal for that `FilterExpressions`.
//     /// Otherwise, return the first `FilterExpressions` divided by the product of the rest.
//     /// All `FilterExpressions` must resolve to the same type (integer or float).
//     /// Requires server version 5.6.0+.
//     pub fn num_div(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_div(
//                 exps.into_iter().map(|exp| exp._as).collect(),
//             ),
//         }
//     }

//     /// Create "power" operator that raises a "base" to the "exponent" power.
//     /// All arguments must resolve to floats.
//     /// Requires server version 5.6.0+.
//     pub fn num_pow(base: FilterExpression, exponent: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_pow(base._as, exponent._as),
//         }
//     }

//     /// Create "log" operator for logarithm of "num" with base "base".
//     /// All arguments must resolve to floats.
//     /// Requires server version 5.6.0+.
//     pub fn num_log(num: FilterExpression, base: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_log(num._as, base._as),
//         }
//     }

//     /// Create "modulo" (%) operator that determines the remainder of "numerator"
//     /// divided by "denominator". All arguments must resolve to integers.
//     /// Requires server version 5.6.0+.
//     pub fn num_mod(numerator: FilterExpression, denominator: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_mod(numerator._as, denominator._as),
//         }
//     }

//     /// Create operator that returns absolute value of a number.
//     /// All arguments must resolve to integer or float.
//     /// Requires server version 5.6.0+.
//     pub fn num_abs(value: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_abs(value._as),
//         }
//     }

//     /// Create expression that rounds a floating point number down to the closest integer value.
//     /// The return type is float.
//     // Requires server version 5.6.0+.
//     pub fn num_floor(num: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_floor(num._as),
//         }
//     }

//     /// Create expression that rounds a floating point number up to the closest integer value.
//     /// The return type is float.
//     /// Requires server version 5.6.0+.
//     pub fn num_ceil(num: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::num_ceil(num._as),
//         }
//     }

//     /// Create expression that converts an integer to a float.
//     /// Requires server version 5.6.0+.
//     pub fn to_int(num: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::to_int(num._as),
//         }
//     }

//     /// Create expression that converts a float to an integer.
//     /// Requires server version 5.6.0+.
//     pub fn to_float(num: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::to_float(num._as),
//         }
//     }

//     /// Create integer "and" (&) operator that is applied to two or more integers.
//     /// All arguments must resolve to integers.
//     /// Requires server version 5.6.0+.
//     pub fn int_and(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_and(
//                 exps.into_iter().map(|exp| exp._as).collect(),
//             ),
//         }
//     }

//     /// Create integer "or" (|) operator that is applied to two or more integers.
//     /// All arguments must resolve to integers.
//     /// Requires server version 5.6.0+.
//     pub fn int_or(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_or(exps.into_iter().map(|exp| exp._as).collect()),
//         }
//     }

//     /// Create integer "xor" (^) operator that is applied to two or more integers.
//     /// All arguments must resolve to integers.
//     /// Requires server version 5.6.0+.
//     pub fn int_xor(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_xor(
//                 exps.into_iter().map(|exp| exp._as).collect(),
//             ),
//         }
//     }

//     /// Create integer "not" (~) operator.
//     /// Requires server version 5.6.0+.
//     pub fn int_not(exp: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_not(exp._as),
//         }
//     }

//     /// Create integer "left shift" (<<) operator.
//     /// Requires server version 5.6.0+.
//     pub fn int_lshift(value: FilterExpression, shift: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_lshift(value._as, shift._as),
//         }
//     }

//     /// Create integer "logical right shift" (>>>) operator.
//     /// Requires server version 5.6.0+.
//     pub fn int_rshift(value: FilterExpression, shift: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_rshift(value._as, shift._as),
//         }
//     }

//     /// Create integer "arithmetic right shift" (>>) operator.
//     /// The sign bit is preserved and not shifted.
//     /// Requires server version 5.6.0+.
//     pub fn int_arshift(value: FilterExpression, shift: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_arshift(value._as, shift._as),
//         }
//     }

//     /// Create expression that returns count of integer bits that are set to 1.
//     /// Requires server version 5.6.0+
//     pub fn int_count(exp: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_count(exp._as),
//         }
//     }

//     /// Create expression that scans integer bits from left (most significant bit) to
//     /// right (least significant bit), looking for a search bit value. When the
//     /// search value is found, the index of that bit (where the most significant bit is
//     /// index 0) is returned. If "search" is true, the scan will search for the bit
//     /// value 1. If "search" is false it will search for bit value 0.
//     /// Requires server version 5.6.0+.
//     pub fn int_lscan(value: FilterExpression, search: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_lscan(value._as, search._as),
//         }
//     }

//     /// Create expression that scans integer bits from right (least significant bit) to
//     /// left (most significant bit), looking for a search bit value. When the
//     /// search value is found, the index of that bit (where the most significant bit is
//     /// index 0) is returned. If "search" is true, the scan will search for the bit
//     /// value 1. If "search" is false it will search for bit value 0.
//     /// Requires server version 5.6.0+.
//     pub fn int_rscan(value: FilterExpression, search: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::int_rscan(value._as, search._as),
//         }
//     }

//     /// Create expression that returns the minimum value in a variable number of expressions.
//     /// All arguments must be the same type (integer or float).
//     /// Requires server version 5.6.0+.
//     pub fn min(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::min(exps.into_iter().map(|exp| exp._as).collect()),
//         }
//     }

//     /// Create expression that returns the maximum value in a variable number of expressions.
//     /// All arguments must be the same type (integer or float).
//     /// Requires server version 5.6.0+.
//     pub fn max(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::max(exps.into_iter().map(|exp| exp._as).collect()),
//         }
//     }

//     //--------------------------------------------------
//     // Variables
//     //--------------------------------------------------

//     /// Conditionally select an expression from a variable number of expression pairs
//     /// followed by default expression action.
//     /// Requires server version 5.6.0+.
//     /// ```
//     /// // Args Format: bool exp1, action exp1, bool exp2, action exp2, ..., action-default
//     /// // Apply operator based on type.
//     pub fn cond(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::cond(exps.into_iter().map(|exp| exp._as).collect()),
//         }
//     }

//     /// Define variables and expressions in scope.
//     /// Requires server version 5.6.0+.
//     /// ```
//     /// // 5 < a < 10
//     pub fn exp_let(exps: Vec<FilterExpression>) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::exp_let(
//                 exps.into_iter().map(|exp| exp._as).collect(),
//             ),
//         }
//     }

//     /// Assign variable to an expression that can be accessed later.
//     /// Requires server version 5.6.0+.
//     /// ```
//     /// // 5 < a < 10
//     pub fn def(name: String, value: FilterExpression) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::def(name, value._as),
//         }
//     }

//     /// Retrieve expression value from a variable.
//     /// Requires server version 5.6.0+.
//     pub fn var(name: String) -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::var(name),
//         }
//     }

//     /// Create unknown value. Used to intentionally fail an expression.
//     /// The failure can be ignored with `ExpWriteFlags` `EVAL_NO_FAIL`
//     /// or `ExpReadFlags` `EVAL_NO_FAIL`.
//     /// Requires server version 5.6.0+.
//     pub fn unknown() -> Self {
//         FilterExpression {
//             _as: aerospike_core::expressions::unknown(),
//         }
//     }
// }

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Priority
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Priority of operations on database server.
#[derive(Debug, Clone, Copy)]
pub enum _Priority {
    Default,
    Low,
    Medium,
    High,
}

#[php_class(name = "Aerospike\\Priority")]
pub struct Priority {
    v: _Priority,
}

impl FromZval<'_> for Priority {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Priority = zval.extract()?;

        Some(Priority { v: f.v.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Priority {
    /// Default determines that the server defines the priority.
    pub fn default() -> Self {
        Priority {
            v: _Priority::Default,
        }
    }

    /// Low determines that the server should run the operation in a background thread.
    pub fn low() -> Self {
        Priority { v: _Priority::Low }
    }

    /// Medium determines that the server should run the operation at medium priority.
    pub fn medium() -> Self {
        Priority {
            v: _Priority::Medium,
        }
    }

    /// High determines that the server should run the operation at the highest priority.
    pub fn high() -> Self {
        Priority { v: _Priority::High }
    }
}

impl From<&Priority> for proto::Priority {
    fn from(input: &Priority) -> Self {
        match &input.v {
            _Priority::Default => proto::Priority::Default,
            _Priority::Low => proto::Priority::Low,
            _Priority::Medium => proto::Priority::Medium,
            _Priority::High => proto::Priority::High,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  RecordExistsAction
//
////////////////////////////////////////////////////////////////////////////////////////////

/// `RecordExistsAction` determines how to handle record writes based on record generation.
#[derive(Debug, PartialEq, Clone)]
pub enum _RecordExistsAction {
    Update,
    UpdateOnly,
    Replace,
    ReplaceOnly,
    CreateOnly,
}

#[php_class(name = "Aerospike\\RecordExistsAction")]
pub struct RecordExistsAction {
    v: _RecordExistsAction,
}

impl FromZval<'_> for RecordExistsAction {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &RecordExistsAction = zval.extract()?;

        Some(RecordExistsAction { v: f.v.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl RecordExistsAction {
    /// Update means: Create or update record.
    /// Merge write command bins with existing bins.
    pub fn update() -> Self {
        RecordExistsAction {
            v: _RecordExistsAction::Update,
        }
    }

    /// UpdateOnly means: Update record only. Fail if record does not exist.
    /// Merge write command bins with existing bins.
    pub fn update_only() -> Self {
        RecordExistsAction {
            v: _RecordExistsAction::UpdateOnly,
        }
    }

    /// Replace means: Create or replace record.
    /// Delete existing bins not referenced by write command bins.
    /// Supported by Aerospike 2 server versions >= 2.7.5 and
    /// Aerospike 3 server versions >= 3.1.6.
    pub fn replace() -> Self {
        RecordExistsAction {
            v: _RecordExistsAction::Replace,
        }
    }

    /// ReplaceOnly means: Replace record only. Fail if record does not exist.
    /// Delete existing bins not referenced by write command bins.
    /// Supported by Aerospike 2 server versions >= 2.7.5 and
    /// Aerospike 3 server versions >= 3.1.6.
    pub fn replace_only() -> Self {
        RecordExistsAction {
            v: _RecordExistsAction::ReplaceOnly,
        }
    }

    /// CreateOnly means: Create only. Fail if record exists.
    pub fn create_only() -> Self {
        RecordExistsAction {
            v: _RecordExistsAction::CreateOnly,
        }
    }
}

impl From<&RecordExistsAction> for proto::RecordExistsAction {
    fn from(input: &RecordExistsAction) -> Self {
        match &input.v {
            _RecordExistsAction::Update => proto::RecordExistsAction::Update,
            _RecordExistsAction::UpdateOnly => proto::RecordExistsAction::UpdateOnly,
            _RecordExistsAction::Replace => proto::RecordExistsAction::Replace,
            _RecordExistsAction::ReplaceOnly => proto::RecordExistsAction::ReplaceOnly,
            _RecordExistsAction::CreateOnly => proto::RecordExistsAction::CreateOnly,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CommitLevel
//
////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub enum _CommitLevel {
    CommitAll,
    CommitMaster,
}

#[php_class(name = "Aerospike\\CommitLevel")]
pub struct CommitLevel {
    v: _CommitLevel,
}

impl FromZval<'_> for CommitLevel {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CommitLevel = zval.extract()?;

        Some(CommitLevel { v: f.v.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl CommitLevel {
    /// CommitAll indicates the server should wait until successfully committing master and all
    /// replicas.
    pub fn commit_all() -> Self {
        CommitLevel {
            v: _CommitLevel::CommitAll,
        }
    }

    /// CommitMaster indicates the server should wait until successfully committing master only.
    pub fn commit_master() -> Self {
        CommitLevel {
            v: _CommitLevel::CommitMaster,
        }
    }
}

impl From<&CommitLevel> for proto::CommitLevel {
    fn from(input: &CommitLevel) -> Self {
        match &input.v {
            _CommitLevel::CommitAll => proto::CommitLevel::CommitAll,
            _CommitLevel::CommitMaster => proto::CommitLevel::CommitMaster,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ConsistencyLevel
//
////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Debug, Clone, Copy)]
pub enum _ConsistencyLevel {
    ConsistencyOne,
    ConsistencyAll,
}

/// `ConsistencyLevel` indicates how replicas should be consulted in a read
/// operation to provide the desired consistency guarantee.
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
    pub fn consistency_one() -> Self {
        ConsistencyLevel {
            v: _ConsistencyLevel::ConsistencyOne,
        }
    }

    /// ConsistencyAll indicates that all replicas should be consulted in
    /// the read operation.
    pub fn consistency_all() -> Self {
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
//
//  GenerationPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, PartialEq, Clone)]
pub enum _GenerationPolicy {
    None,
    ExpectGenEqual,
    ExpectGenGreater,
}

/// `GenerationPolicy` determines how to handle record writes based on record generation.
#[php_class(name = "Aerospike\\GenerationPolicy")]
pub struct GenerationPolicy {
    v: _GenerationPolicy,
}

impl FromZval<'_> for GenerationPolicy {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &GenerationPolicy = zval.extract()?;

        Some(GenerationPolicy { v: f.v.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl GenerationPolicy {
    /// None means: Do not use record generation to restrict writes.
    pub fn none() -> Self {
        GenerationPolicy {
            v: _GenerationPolicy::None,
        }
    }

    /// ExpectGenEqual means: Update/delete record if expected generation is equal to server
    /// generation. Otherwise, fail.
    pub fn expect_gen_equal() -> Self {
        GenerationPolicy {
            v: _GenerationPolicy::ExpectGenEqual,
        }
    }

    /// ExpectGenGreater means: Update/delete record if expected generation greater than the server
    /// generation. Otherwise, fail. This is useful for restore after backup.
    pub fn expect_gen_greater() -> Self {
        GenerationPolicy {
            v: _GenerationPolicy::ExpectGenGreater,
        }
    }
}

impl From<&GenerationPolicy> for proto::GenerationPolicy {
    fn from(input: &GenerationPolicy) -> Self {
        match &input.v {
            _GenerationPolicy::None => proto::GenerationPolicy::None,
            _GenerationPolicy::ExpectGenEqual => proto::GenerationPolicy::ExpectGenEqual,
            _GenerationPolicy::ExpectGenGreater => proto::GenerationPolicy::ExpectGenGt,
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

#[derive(Debug, Clone, Copy)]
pub enum _Expiration {
    Seconds(u32),
    NamespaceDefault,
    Never,
    DontUpdate,
}

/// Record expiration, also known as time-to-live (TTL).
#[php_class(name = "Aerospike\\Expiration")]
pub struct Expiration {
    v: _Expiration,
}

impl FromZval<'_> for Expiration {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Expiration = zval.extract()?;

        Some(Expiration { v: f.v.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Expiration {
    /// Set the record to expire X seconds from now
    pub fn seconds(seconds: u32) -> Self {
        Expiration {
            v: _Expiration::Seconds(seconds),
        }
    }

    /// Set the record's expiry time using the default time-to-live (TTL) value for the namespace
    pub fn namespace_default() -> Self {
        Expiration {
            v: _Expiration::NamespaceDefault,
        }
    }

    /// Set the record to never expire. Requires Aerospike 2 server version 2.7.2 or later or
    /// Aerospike 3 server version 3.1.4 or later. Do not use with older servers.
    pub fn never() -> Self {
        Expiration {
            v: _Expiration::Never,
        }
    }

    /// Do not change the record's expiry time when updating the record; requires Aerospike server
    /// version 3.10.1 or later.
    pub fn dont_update() -> Self {
        Expiration {
            v: _Expiration::DontUpdate,
        }
    }
}

impl From<&Expiration> for u32 {
    fn from(exp: &Expiration) -> u32 {
        match &exp.v {
            _Expiration::Seconds(secs) => *secs,
            _Expiration::NamespaceDefault => NAMESPACE_DEFAULT,
            _Expiration::Never => NEVER_EXPIRE,
            _Expiration::DontUpdate => DONT_UPDATE,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Concurrency
//
////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy)]
pub enum _Concurrency {
    Sequential,
    Parallel,
    MaxThreads(u32),
}

/// Specifies whether a command, that needs to be executed on multiple cluster nodes, should be
/// executed sequentially, one node at a time, or in parallel on multiple nodes using the client's
/// thread pool.
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
    pub fn sequential() -> Self {
        Concurrency {
            v: _Concurrency::Sequential,
        }
    }

    /// Issue all commands in parallel threads. This mode has a performance advantage for
    /// extremely large batch sizes because each node can process the request immediately. The
    /// downside is extra threads will need to be created (or takedn from a thread pool).
    pub fn parallel() -> Self {
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
    pub fn max_threads(threads: u32) -> Self {
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

    pub fn ordered() -> Self {
        ListOrderType {
            _as: proto::ListOrderType::Unordered,
        }
    }

    pub fn unoredered() -> Self {
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

    pub fn unoredered() -> Self {
        MapOrderType {
            _as: proto::MapOrderType::Unordered,
        }
    }

    pub fn key_ordered() -> Self {
        MapOrderType {
            _as: proto::MapOrderType::KeyOrdered,
        }
    }

    pub fn key_value_ordered() -> Self {
        MapOrderType {
            _as: proto::MapOrderType::KeyValueOrdered,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BasePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

// #[php_class(name = "Aerospike\\BasePolicy")]
// pub struct BasePolicy {
//     _as: proto::BasePolicy,
// }

// impl FromZval<'_> for BasePolicy {
//     const TYPE: DataType = DataType::Mixed;

//     fn from_zval(zval: &Zval) -> Option<Self> {
//         let f: &BasePolicy = zval.extract()?;

//         Some(BasePolicy { _as: f._as.clone() })
//     }
// }

// /// Trait implemented by most policy types; policies that implement this trait typically encompass
// /// an instance of `BasePolicy`.
// #[php_impl]
// #[derive(ZvalConvert)]
// impl BasePolicy {
//     #[getter]
//     pub fn get_priority(&self) -> Priority {
//         Priority {
//             v: match &self._as.priority {
//                 aerospike_core::Priority::Default => _Priority::Default,
//                 aerospike_core::Priority::Low => _Priority::Low,
//                 aerospike_core::Priority::Medium => _Priority::Medium,
//                 aerospike_core::Priority::High => _Priority::High,
//             },
//         }
//     }

//     #[setter]
//     pub fn set_priority(&mut self, priority: Priority) {
//         self._as.priority = priority._as;
//     }

//     #[getter]
//     pub fn get_consistency_level(&self) -> ConsistencyLevel {
//         ConsistencyLevel {
//             _as: self._as.consistency_level.clone(),
//             v: match &self._as.consistency_level {
//                 aerospike_core::ConsistencyLevel::ConsistencyOne => {
//                     _ConsistencyLevel::ConsistencyOne
//                 }
//                 aerospike_core::ConsistencyLevel::ConsistencyAll => {
//                     _ConsistencyLevel::ConsistencyAll
//                 }
//             },
//         }
//     }

//     #[setter]
//     pub fn set_consistency_level(&mut self, consistency_level: ConsistencyLevel) {
//         self._as.consistency_level = consistency_level._as;
//     }

//     #[getter]
//     pub fn get_timeout(&self) -> u64 {
//         self._as
//             .timeout
//             .map(|duration| duration.as_millis() as u64)
//             .unwrap_or_default()
//     }

//     #[setter]
//     pub fn set_timeout(&mut self, timeout_millis: u64) {
//         let timeout = Duration::from_millis(timeout_millis);
//         self._as.timeout = Some(timeout);
//     }

//     #[getter]
//     pub fn get_max_retries(&self) -> Option<usize> {
//         self._as.max_retries
//     }

//     #[setter]
//     pub fn set_max_retries(&mut self, max_retries: Option<usize>) {
//         self._as.max_retries = max_retries;
//     }

//     #[getter]
//     pub fn get_sleep_between_retries(&self) -> u64 {
//         self._as
//             .sleep_between_retries
//             .map(|duration| duration.as_millis() as u64)
//             .unwrap_or_default()
//     }

//     #[setter]
//     pub fn set_sleep_between_retries(&mut self, sleep_between_retries_millis: u64) {
//         let sleep_between_retries = Duration::from_millis(sleep_between_retries_millis);
//         self._as.timeout = Some(sleep_between_retries);
//     }

//     #[getter]
//     pub fn get_filter_expression(&self) -> Option<FilterExpression> {
//         match &self._as.filter_expression {
//             Some(fe) => Some(FilterExpression { _as: fe.clone() }),
//             None => None,
//         }
//     }

//     #[setter]
//     pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
//         match filter_expression {
//             Some(fe) => self._as.filter_expression = Some(fe._as),
//             None => self._as.filter_expression = None,
//         }
//     }
// }

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

#[php_class(name = "Aerospike\\CDTContext")]
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

    // CtxListIndex defines Lookup list by index offset.
    // If the index is negative, the resolved index starts backwards from end of list.
    // If an index is out of bounds, a parameter error will be returned.
    // Examples:
    // 0: First item.
    // 4: Fifth item.
    // -1: Last item.
    // -3: Third to last item.
    pub fn ListIndex(index: i32) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::ListIndex as i32,
                value: Some(PHPValue::Int(index.into()).into()),
            },
        }
    }

    // CtxListIndexCreate list with given type at index offset, given an order and pad.
    pub fn ListIndexCreate(index: i32, order: ListOrderType, pad: bool) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::ListIndex as i32 | Self::list_order_flag(order, pad),
                value: Some(PHPValue::Int(index.into()).into()),
            },
        }
    }

    // CtxListRank defines Lookup list by rank.
    // 0 = smallest value
    // N = Nth smallest value
    // -1 = largest value
    pub fn ListRank(rank: i32) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::ListRank as i32,
                value: Some(PHPValue::Int(rank.into()).into()),
            },
        }
    }

    // CtxListValue defines Lookup list by value.
    pub fn ListValue(key: PHPValue) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::ListValue as i32,
                value: Some(key.into()),
            },
        }
    }

    // CtxMapIndex defines Lookup map by index offset.
    // If the index is negative, the resolved index starts backwards from end of list.
    // If an index is out of bounds, a parameter error will be returned.
    // Examples:
    // 0: First item.
    // 4: Fifth item.
    // -1: Last item.
    // -3: Third to last item.
    pub fn MapIndex(index: i32) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::MapIndex as i32,
                value: Some(PHPValue::Int(index.into()).into()),
            },
        }
    }

    // CtxMapRank defines Lookup map by rank.
    // 0 = smallest value
    // N = Nth smallest value
    // -1 = largest value
    pub fn MapRank(rank: i32) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::MapRank as i32,
                value: Some(PHPValue::Int(rank.into()).into()),
            },
        }
    }

    // CtxMapKey defines Lookup map by key.
    pub fn MapKey(key: PHPValue) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::MapKey as i32,
                value: Some(key.into()),
            },
        }
    }

    // CtxMapKeyCreate creates map with given type at map key.
    pub fn MapKeyCreate(key: PHPValue, order: MapOrderType) -> Self {
        CDTContext {
            _as: proto::CdtContext {
                id: CDTContextType::MapKey as i32 | order.flag(),
                value: Some(key.into()),
            },
        }
    }

    // CtxMapValue defines Lookup map by value.
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

#[php_class(name = "Aerospike\\ReadPolicy")]
pub struct ReadPolicy {
    _as: proto::ReadPolicy,
}

/// `ReadPolicy` excapsulates parameters for transaction policy attributes
/// used in all database operation calls.
#[php_impl]
#[derive(ZvalConvert)]
impl ReadPolicy {
    pub fn __construct() -> Self {
        ReadPolicy {
            _as: proto::ReadPolicy::default(),
        }
    }

    // #[getter]
    // pub fn get_priority(&self) -> Priority {
    //     Priority {
    //         _as: self._as.priority.clone(),
    //         v: match &self._as.priority {
    //             aerospike_sync::Priority::Default => _Priority::Default,
    //             aerospike_sync::Priority::Low => _Priority::Low,
    //             aerospike_sync::Priority::Medium => _Priority::Medium,
    //             aerospike_sync::Priority::High => _Priority::High,
    //         },
    //     }
    // }

    // #[setter]
    // pub fn set_priority(&mut self, priority: Priority) {
    //     self._as.priority = priority._as;
    // }

    // #[getter]
    // pub fn get_max_retries(&self) -> Option<usize> {
    //     self._as.max_retries
    // }

    // #[setter]
    // pub fn set_max_retries(&mut self, max_retries: Option<usize>) {
    //     self._as.max_retries = max_retries;
    // }

    // #[getter]
    // pub fn get_timeout(&self) -> u64 {
    //     self._as
    //         .timeout
    //         .map(|duration| duration.as_millis() as u64)
    //         .unwrap_or_default()
    // }

    // #[setter]
    // pub fn set_timeout(&mut self, timeout_millis: u64) {
    //     let timeout = Duration::from_millis(timeout_millis);
    //     self._as.timeout = Some(timeout);
    // }

    // #[getter]
    // pub fn get_filter_expression(&self) -> Option<FilterExpression> {
    //     match &self._as.filter_expression {
    //         Some(fe) => Some(FilterExpression { _as: fe.clone() }),
    //         None => None,
    //     }
    // }

    // #[setter]
    // pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
    //     match filter_expression {
    //         Some(fe) => self._as.filter_expression = Some(fe._as),
    //         None => self._as.filter_expression = None,
    //     }
    // }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  WritePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\WritePolicy")]
pub struct WritePolicy {
    _as: proto::WritePolicy,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl WritePolicy {
    pub fn __construct() -> Self {
        WritePolicy {
            _as: proto::WritePolicy::default(),
        }
    }

    // #[getter]
    // pub fn get_base_policy(&self) -> BasePolicy {
    //     BasePolicy {
    //         _as: self._as.base_policy.clone(),
    //     }
    // }

    // #[setter]
    // pub fn set_base_policy(&mut self, base_policy: BasePolicy) {
    //     self._as.base_policy = base_policy._as;
    // }

    // #[getter]
    // pub fn get_record_exists_action(&self) -> RecordExistsAction {
    //     RecordExistsAction {
    //         _as: self._as.record_exists_action.clone(),
    //         v: match &self._as.record_exists_action {
    //             aerospike_core::RecordExistsAction::Update => _RecordExistsAction::Update,
    //             aerospike_core::RecordExistsAction::UpdateOnly => _RecordExistsAction::UpdateOnly,
    //             aerospike_core::RecordExistsAction::Replace => _RecordExistsAction::Replace,
    //             aerospike_core::RecordExistsAction::ReplaceOnly => _RecordExistsAction::ReplaceOnly,
    //             aerospike_core::RecordExistsAction::CreateOnly => _RecordExistsAction::CreateOnly,
    //         },
    //     }
    // }

    // #[setter]
    // pub fn set_record_exists_action(&mut self, record_exists_action: RecordExistsAction) {
    //     self._as.record_exists_action = record_exists_action._as;
    // }

    // #[getter]
    // pub fn get_generation_policy(&self) -> GenerationPolicy {
    //     GenerationPolicy {
    //         _as: self._as.generation_policy.clone(),
    //         v: match &self._as.generation_policy {
    //             aerospike_core::GenerationPolicy::None => _GenerationPolicy::None,
    //             aerospike_core::GenerationPolicy::ExpectGenEqual => {
    //                 _GenerationPolicy::ExpectGenEqual
    //             }
    //             aerospike_core::GenerationPolicy::ExpectGenGreater => {
    //                 _GenerationPolicy::ExpectGenGreater
    //             }
    //         },
    //     }
    // }

    // #[setter]
    // pub fn set_generation_policy(&mut self, generation_policy: GenerationPolicy) {
    //     self._as.generation_policy = generation_policy._as;
    // }

    // #[getter]
    // pub fn get_commit_level(&self) -> CommitLevel {
    //     CommitLevel {
    //         _as: self._as.commit_level.clone(),
    //         v: match &self._as.commit_level {
    //             aerospike_core::CommitLevel::CommitAll => _CommitLevel::CommitAll,
    //             aerospike_core::CommitLevel::CommitMaster => _CommitLevel::CommitMaster,
    //         },
    //     }
    // }

    // #[setter]
    // pub fn set_commit_level(&mut self, commit_level: CommitLevel) {
    //     self._as.commit_level = commit_level._as;
    // }

    // #[getter]
    // pub fn get_generation(&self) -> u32 {
    //     self._as.generation
    // }

    // #[setter]
    // pub fn set_generation(&mut self, generation: u32) {
    //     self._as.generation = generation;
    // }

    // #[getter]
    // pub fn get_expiration(&self) -> Expiration {
    //     Expiration {
    //         _as: self._as.expiration,
    //         v: match &self._as.expiration {
    //             aerospike_core::Expiration::Seconds(secs) => _Expiration::Seconds(*secs),
    //             aerospike_core::Expiration::NamespaceDefault => _Expiration::NamespaceDefault,
    //             aerospike_core::Expiration::Never => _Expiration::Never,
    //             aerospike_core::Expiration::DontUpdate => _Expiration::DontUpdate,
    //         },
    //     }
    // }

    // #[setter]
    // pub fn set_expiration(&mut self, expiration: Expiration) {
    //     self._as.expiration = expiration._as;
    // }

    // #[getter]
    // pub fn get_send_key(&self) -> bool {
    //     self._as.send_key
    // }

    // #[setter]
    // pub fn set_send_key(&mut self, send_key: bool) {
    //     self._as.send_key = send_key;
    // }

    // #[getter]
    // pub fn get_respond_per_each_op(&self) -> bool {
    //     self._as.respond_per_each_op
    // }

    // #[setter]
    // pub fn set_respond_per_each_op(&mut self, respond_per_each_op: bool) {
    //     self._as.respond_per_each_op = respond_per_each_op;
    // }

    // #[getter]
    // pub fn get_durable_delete(&self) -> bool {
    //     self._as.respond_per_each_op
    // }

    // #[setter]
    // pub fn set_durable_delete(&mut self, durable_delete: bool) {
    //     self._as.durable_delete = durable_delete;
    // }

    // #[getter]
    // pub fn get_filter_expression(&self) -> Option<FilterExpression> {
    //     match &self._as.filter_expression {
    //         Some(fe) => Some(FilterExpression { _as: fe.clone() }),
    //         None => None,
    //     }
    // }

    // #[setter]
    // pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
    //     match filter_expression {
    //         Some(fe) => self._as.filter_expression = Some(fe._as),
    //         None => self._as.filter_expression = None,
    //     }
    // }
}

// ////////////////////////////////////////////////////////////////////////////////////////////
// //
// //  QueryPolicy
// //
// ////////////////////////////////////////////////////////////////////////////////////////////

// #[php_class(name = "Aerospike\\QueryPolicy")]
// pub struct QueryPolicy {
//     _as: aerospike_core::QueryPolicy,
// }

// /// `QueryPolicy` encapsulates parameters for query operations.
// #[php_impl]
// #[derive(ZvalConvert)]
// impl QueryPolicy {
//     pub fn __construct() -> Self {
//         QueryPolicy {
//             _as: aerospike_core::QueryPolicy::default(),
//         }
//     }

//     #[getter]
//     pub fn get_base_policy(&self) -> BasePolicy {
//         BasePolicy {
//             _as: self._as.base_policy.clone(),
//         }
//     }

//     #[setter]
//     pub fn set_base_policy(&mut self, base_policy: BasePolicy) {
//         self._as.base_policy = base_policy._as;
//     }
//     #[getter]
//     pub fn get_max_concurrent_nodes(&self) -> usize {
//         self._as.max_concurrent_nodes
//     }

//     #[setter]
//     pub fn set_max_concurrent_nodes(&mut self, max_concurrent_nodes: usize) {
//         self._as.max_concurrent_nodes = max_concurrent_nodes;
//     }

//     #[getter]
//     pub fn get_record_queue_size(&self) -> usize {
//         self._as.record_queue_size
//     }

//     #[setter]
//     pub fn set_record_queue_size(&mut self, record_queue_size: usize) {
//         self._as.record_queue_size = record_queue_size;
//     }

//     #[getter]
//     pub fn get_fail_on_cluster_change(&self) -> bool {
//         self._as.fail_on_cluster_change
//     }

//     #[setter]
//     pub fn set_fail_on_cluster_change(&mut self, fail_on_cluster_change: bool) {
//         self._as.fail_on_cluster_change = fail_on_cluster_change;
//     }

//     #[getter]
//     pub fn get_filter_expression(&self) -> Option<FilterExpression> {
//         match &self._as.filter_expression {
//             Some(fe) => Some(FilterExpression { _as: fe.clone() }),
//             None => None,
//         }
//     }

//     #[setter]
//     pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
//         match filter_expression {
//             Some(fe) => self._as.filter_expression = Some(fe._as),
//             None => self._as.filter_expression = None,
//         }
//     }
// }

// ////////////////////////////////////////////////////////////////////////////////////////////
// //
// //  ScanPolicy
// //
// ////////////////////////////////////////////////////////////////////////////////////////////

// #[php_class(name = "Aerospike\\ScanPolicy")]
// pub struct ScanPolicy {
//     _as: aerospike_core::ScanPolicy,
// }

// /// `ScanPolicy` encapsulates optional parameters used in scan operations.
// #[php_impl]
// #[derive(ZvalConvert)]
// impl ScanPolicy {
//     pub fn __construct() -> Self {
//         ScanPolicy {
//             _as: aerospike_core::ScanPolicy::default(),
//         }
//     }

//     #[getter]
//     pub fn get_base_policy(&self) -> BasePolicy {
//         BasePolicy {
//             _as: self._as.base_policy.clone(),
//         }
//     }

//     #[setter]
//     pub fn set_base_policy(&mut self, base_policy: BasePolicy) {
//         self._as.base_policy = base_policy._as;
//     }

//     #[getter]
//     pub fn get_scan_percent(&self) -> u8 {
//         self._as.scan_percent
//     }

//     #[setter]
//     pub fn set_scan_percent(&mut self, scan_percent: u8) {
//         self._as.scan_percent = scan_percent;
//     }

//     #[getter]
//     pub fn get_max_concurrent_nodes(&self) -> usize {
//         self._as.max_concurrent_nodes
//     }

//     #[setter]
//     pub fn set_max_concurrent_nodes(&mut self, max_concurrent_nodes: usize) {
//         self._as.max_concurrent_nodes = max_concurrent_nodes;
//     }

//     #[getter]
//     pub fn get_record_queue_size(&self) -> usize {
//         self._as.record_queue_size
//     }

//     #[setter]
//     pub fn set_record_queue_size(&mut self, record_queue_size: usize) {
//         self._as.record_queue_size = record_queue_size;
//     }

//     #[getter]
//     pub fn get_fail_on_cluster_change(&self) -> bool {
//         self._as.fail_on_cluster_change
//     }

//     #[setter]
//     pub fn set_fail_on_cluster_change(&mut self, fail_on_cluster_change: bool) {
//         self._as.fail_on_cluster_change = fail_on_cluster_change;
//     }

//     #[getter]
//     pub fn get_socket_timeout(&self) -> u32 {
//         self._as.socket_timeout
//     }

//     #[setter]
//     pub fn set_socket_timeout(&mut self, socket_timeout: u32) {
//         self._as.socket_timeout = socket_timeout;
//     }

//     #[getter]
//     pub fn get_filter_expression(&self) -> Option<FilterExpression> {
//         match &self._as.filter_expression {
//             Some(fe) => Some(FilterExpression { _as: fe.clone() }),
//             None => None,
//         }
//     }

//     #[setter]
//     pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
//         match filter_expression {
//             Some(fe) => self._as.filter_expression = Some(fe._as),
//             None => self._as.filter_expression = None,
//         }
//     }
// }

////////////////////////////////////////////////////////////////////////////////////////////
//
//  IndexCollectionType
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\IndexCollectionType")]
pub struct IndexCollectionType {
    _as: proto::IndexCollectionType,
}

#[php_impl]
#[derive(ZvalConvert)]
impl IndexCollectionType {
    pub fn Default() -> Self {
        IndexCollectionType {
            _as: proto::IndexCollectionType::Default,
        }
    }
    pub fn List() -> Self {
        IndexCollectionType {
            _as: proto::IndexCollectionType::List,
        }
    }
    pub fn MapKeys() -> Self {
        IndexCollectionType {
            _as: proto::IndexCollectionType::MapKeys,
        }
    }
    pub fn MapValues() -> Self {
        IndexCollectionType {
            _as: proto::IndexCollectionType::MapValues,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  IndexType
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\IndexType")]
pub struct IndexType {
    _as: proto::IndexType,
}

#[php_impl]
#[derive(ZvalConvert)]
impl IndexType {
    pub fn Numeric() -> Self {
        IndexType {
            _as: proto::IndexType::Numeric,
        }
    }

    pub fn String() -> Self {
        IndexType {
            _as: proto::IndexType::String,
        }
    }

    pub fn Blob() -> Self {
        IndexType {
            _as: proto::IndexType::Blob,
        }
    }

    pub fn Geo2DSphere() -> Self {
        IndexType {
            _as: proto::IndexType::Geo2DSphere,
        }
    }
}

// impl From<&IndexType> for String {
//     fn from(input: &IndexType) -> String {
//         match &input._as {
//             proto::IndexType::Numeric => "NUMERIC".into(),
//             proto::IndexType::String => "STRING".into(),
//             proto::IndexType::Blob => "BLOB".into(),
//             proto::IndexType::Geo2DSphere => "GEO2DSPHERE".into(),
//         }
//     }
// }

// ////////////////////////////////////////////////////////////////////////////////////////////
// //
// //  Filter
// //
// ////////////////////////////////////////////////////////////////////////////////////////////

// /// Query filter definition. Currently, only one filter is allowed in a Statement, and must be on a
// /// bin which has a secondary index defined.
// ///
// /// Filter instances should be instantiated using one of the provided macros:
// ///
// /// - `as_eq`
// /// - `as_range`
// /// - `as_contains`
// /// - `as_contains_range`
// /// - `as_within_region`
// /// - `as_within_radius`
// /// - `as_regions_containing_point`

// #[php_class(name = "Aerospike\\Filter")]
// pub struct Filter {
//     _as: aerospike_core::query::Filter,
// }

// impl FromZval<'_> for Filter {
//     const TYPE: DataType = DataType::Mixed;

//     fn from_zval(zval: &Zval) -> Option<Self> {
//         let f: &Filter = zval.extract()?;

//         Some(Filter { _as: f._as.clone() })
//     }
// }

// #[php_impl]
// #[derive(ZvalConvert)]
// impl Filter {
//     pub fn range(bin_name: &str, begin: PHPValue, end: PHPValue) -> Self {
//         Filter {
//             _as: aerospike_core::as_range!(
//                 bin_name,
//                 aerospike_core::Value::from(begin),
//                 aerospike_core::Value::from(end)
//             ),
//         }
//     }

//     pub fn contains(bin_name: &str, value: PHPValue, cit: Option<&IndexCollectionType>) -> Self {
//         let default = IndexCollectionType::Default();
//         let cit = cit.unwrap_or(&default);
//         Filter {
//             _as: aerospike_core::as_contains!(
//                 bin_name,
//                 aerospike_core::Value::from(value),
//                 aerospike_core::query::IndexCollectionType::from(cit)
//             ),
//         }
//     }

//     pub fn contains_range(
//         bin_name: &str,
//         begin: PHPValue,
//         end: PHPValue,
//         cit: Option<&IndexCollectionType>,
//     ) -> Self {
//         let default = IndexCollectionType::Default();
//         let cit = cit.unwrap_or(&default);
//         Filter {
//             _as: aerospike_core::as_contains_range!(
//                 bin_name,
//                 aerospike_core::Value::from(begin),
//                 aerospike_core::Value::from(end),
//                 aerospike_core::query::IndexCollectionType::from(cit)
//             ),
//         }
//     }

//     // Example code :
//     // $pointString = '{"type":"AeroCircle","coordinates":[[-89.0000,23.0000], 1000]}'
//     // Filter::regionsContainingPoint("bin_name", $pointString)
//     pub fn within_region(bin_name: &str, region: &str, cit: Option<&IndexCollectionType>) -> Self {
//         let default = IndexCollectionType::Default();
//         let cit = cit.unwrap_or(&default);
//         Filter {
//             _as: aerospike_core::as_within_region!(
//                 bin_name,
//                 region,
//                 aerospike_core::query::IndexCollectionType::from(cit)
//             ),
//         }
//     }

//     // Example code :
//     // $lat = 43.0004;
//     // $lng = -89.0005;
//     // $radius = 1000;
//     // $filter = Filter::regionsContainingPoint("bin_name", $lat, $lng, $radius);
//     pub fn within_radius(
//         bin_name: &str,
//         lat: f64,
//         lng: f64,
//         radius: f64,
//         cit: Option<&IndexCollectionType>,
//     ) -> Self {
//         let default = IndexCollectionType::Default();
//         let cit = cit.unwrap_or(&default);
//         Filter {
//             _as: aerospike_core::as_within_radius!(
//                 bin_name,
//                 lat,
//                 lng,
//                 radius,
//                 aerospike_core::query::IndexCollectionType::from(cit)
//             ),
//         }
//     }

//     // Example code :
//     // $pointString = '{"type":"Point","coordinates":[-89.0000,23.0000]}'
//     // Filter::regionsContainingPoint("bin_name", $pointString)
//     pub fn regions_containing_point(
//         bin_name: &str,
//         point: &str,
//         cit: Option<&IndexCollectionType>,
//     ) -> Self {
//         let default = IndexCollectionType::Default();
//         let cit = cit.unwrap_or(&default);
//         Filter {
//             _as: aerospike_core::as_regions_containing_point!(
//                 bin_name,
//                 point,
//                 aerospike_core::query::IndexCollectionType::from(cit)
//             ),
//         }
//     }
// }

// ////////////////////////////////////////////////////////////////////////////////////////////
// //
// //  Statement
// //
// ////////////////////////////////////////////////////////////////////////////////////////////

// /// Query statement parameters.
// #[php_class(name = "Aerospike\\Statement")]
// pub struct Statement {
//     _as: aerospike_core::Statement,
// }

// #[php_impl]
// #[derive(ZvalConvert)]
// impl Statement {
//     pub fn __construct(namespace: &str, set_name: &str, bins: Option<Vec<String>>) -> Self {
//         Statement {
//             _as: aerospike_core::Statement::new(namespace, set_name, bins_flag(bins)),
//         }
//     }

//     #[getter]
//     pub fn get_filters(&self) -> Option<Vec<Filter>> {
//         self._as
//             .filters
//             .as_ref()
//             .map(|filters| filters.iter().map(|f| Filter { _as: f.clone() }).collect())
//     }

//     #[setter]
//     pub fn set_filters(&mut self, filters: Option<Vec<Filter>>) {
//         match filters {
//             None => self._as.filters = None,
//             Some(filters) => {
//                 self._as.filters = Some(filters.iter().map(|qf| qf._as.clone()).collect());
//             }
//         };
//         // Ok(())
//     }
// }

// ////////////////////////////////////////////////////////////////////////////////////////////
// //
// //  Recordset
// //
// ////////////////////////////////////////////////////////////////////////////////////////////

// /// Virtual collection of records retrieved through queries and scans. During a query/scan,
// /// multiple threads will retrieve records from the server nodes and put these records on an
// /// internal queue managed by the recordset. The single user thread consumes these records from the
// /// queue.
// #[php_class(name = "Aerospike\\Recordset")]
// pub struct Recordset {
//     _as: Arc<aerospike_core::Recordset>,
// }

// #[php_impl]
// #[derive(ZvalConvert)]
// impl Recordset {
//     pub fn close(&self) {
//         self._as.close();
//     }

//     #[getter]
//     pub fn get_active(&self) -> bool {
//         self._as.is_active()
//     }

//     pub fn next(&self) -> Option<Result<Record>> {
//         match self._as.next_record() {
//             None => None,
//             Some(Err(e)) => panic!("{}", e),
//             Some(Ok(rec)) => Some(Ok(rec.into())),
//         }
//     }
// }

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Bin
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Container object for a record bin, comprising a name and a value.
#[php_class(name = "Aerospike\\Bin")]
pub struct Bin {
    _as: proto::Bin,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Bin {
    pub fn __construct(name: &str, value: PHPValue) -> Self {
        let _as = proto::Bin {
            name: name.into(),
            value: Some(value.into()),
        };
        Bin { _as: _as }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Record
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Container object for a database record.
#[php_class(name = "Aerospike\\Record")]
pub struct Record {
    _as: proto::Record,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Record {
    pub fn bin(&self, name: &str) -> Option<PHPValue> {
        let b = self._as.bins.get(name);
        b.map(|v| v.into())
    }

    #[getter]
    pub fn get_bins(&self) -> Option<PHPValue> {
        Some(self._as.bins.clone().into())
    }

    #[getter]
    pub fn get_generation(&self) -> Option<u32> {
        Some(self._as.generation)
    }

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

#[php_class(name = "Aerospike\\BatchPolicy")]
pub struct BatchPolicy {
    _as: proto::BatchPolicy,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl BatchPolicy {
    pub fn __construct() -> Self {
        BatchPolicy {
            _as: proto::BatchPolicy::default(),
        }
    }

    //     #[getter]
    //     pub fn get_base_policy(&self) -> BasePolicy {
    //         BasePolicy {
    //             _as: self._as.base_policy.clone(),
    //         }
    //     }

    //     #[setter]
    //     pub fn set_base_policy(&mut self, base_policy: BasePolicy) {
    //         self._as.base_policy = base_policy._as;
    //     }

    //     #[getter]
    //     pub fn get_concurrency(&self) -> Concurrency {
    //         Concurrency {
    //             _as: self._as.concurrency, // Assuming _as.concurrency is the corresponding field in aerospike_core
    //             v: match &self._as.concurrency {
    //                 aerospike_core::Concurrency::Sequential => _Concurrency::Sequential,
    //                 aerospike_core::Concurrency::Parallel => _Concurrency::Parallel,
    //                 aerospike_core::Concurrency::MaxThreads(threads) => {
    //                     _Concurrency::MaxThreads(*threads)
    //                 }
    //             },
    //         }
    //     }

    //     #[setter]
    //     pub fn set_concurrency(&mut self, concurrency: Concurrency) {
    //         self._as.concurrency = concurrency._as;
    //     }

    //     #[getter]
    //     pub fn get_allow_inline(&self) -> bool {
    //         self._as.allow_inline
    //     }

    //     #[setter]
    //     pub fn set_send_set_name(&mut self, send_set_name: bool) {
    //         self._as.send_set_name = send_set_name;
    //     }

    //     #[getter]
    //     pub fn get_send_set_name(&self) -> bool {
    //         self._as.send_set_name
    //     }

    //     #[setter]
    //     pub fn set_allow_inline(&mut self, allow_inline: bool) {
    //         self._as.allow_inline = allow_inline;
    //     }

    //     #[getter]
    //     pub fn get_filter_expression(&self) -> Option<FilterExpression> {
    //         match &self._as.filter_expression {
    //             Some(fe) => Some(FilterExpression { _as: fe.clone() }),
    //             None => None,
    //         }
    //     }

    //     #[setter]
    //     pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
    //         match filter_expression {
    //             Some(fe) => self._as.filter_expression = Some(fe._as),
    //             None => self._as.filter_expression = None,
    //         }
    //     }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchReadPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\BatchReadPolicy")]
pub struct BatchReadPolicy {
    _as: proto::BatchReadPolicy,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl BatchReadPolicy {
    pub fn __construct() -> Self {
        BatchReadPolicy {
            _as: proto::BatchReadPolicy::default(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchWritePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\BatchWritePolicy")]
pub struct BatchWritePolicy {
    _as: proto::BatchWritePolicy,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl BatchWritePolicy {
    pub fn __construct() -> Self {
        BatchWritePolicy {
            _as: proto::BatchWritePolicy::default(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchDeletePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\BatchDeletePolicy")]
pub struct BatchDeletePolicy {
    _as: proto::BatchDeletePolicy,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl BatchDeletePolicy {
    pub fn __construct() -> Self {
        BatchDeletePolicy {
            _as: proto::BatchDeletePolicy::default(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchUdfPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\BatchUdfPolicy")]
pub struct BatchUdfPolicy {
    _as: proto::BatchUdfPolicy,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl BatchUdfPolicy {
    pub fn __construct() -> Self {
        BatchUdfPolicy {
            _as: proto::BatchUdfPolicy::default(),
        }
    }
}

//////////////////////////////////////////////////////////////////////////////////////////

//  Operation

//////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\Operation")]
pub struct Operation {
    _as: proto::Operation,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl Operation {
    // read bin database operation.
    pub fn get(bin_name: Option<String>) -> Self {
        Operation {
            _as: proto::Operation {
                op_type: proto::OperationType::Read.into(),
                bin_name: bin_name,
                ..proto::Operation::default()
            },
        }
    }

    // read record header database operation.
    pub fn get_header() -> Self {
        Operation {
            _as: proto::Operation {
                op_type: proto::OperationType::ReadHeader.into(),
                ..proto::Operation::default()
            },
        }
    }

    // set database operation.
    pub fn put(bin: &Bin) -> Self {
        Operation {
            _as: proto::Operation {
                op_type: proto::OperationType::Write.into(),
                bin_name: Some(bin._as.name.clone()),
                bin_value: bin._as.value.clone(),
                ..proto::Operation::default()
            },
        }
    }

    // string append database operation.
    pub fn append(bin: &Bin) -> Self {
        Operation {
            _as: proto::Operation {
                op_type: proto::OperationType::Append.into(),
                bin_name: Some(bin._as.name.clone()),
                bin_value: bin._as.value.clone(),
                ..proto::Operation::default()
            },
        }
    }

    // string prepend database operation.
    pub fn prepend(bin: &Bin) -> Self {
        Operation {
            _as: proto::Operation {
                op_type: proto::OperationType::Prepend.into(),
                bin_name: Some(bin._as.name.clone()),
                bin_value: bin._as.value.clone(),
                ..proto::Operation::default()
            },
        }
    }

    // integer add database operation.
    pub fn add(bin: &Bin) -> Self {
        Operation {
            _as: proto::Operation {
                op_type: proto::OperationType::Add.into(),
                bin_name: Some(bin._as.name.clone()),
                bin_value: bin._as.value.clone(),
                ..proto::Operation::default()
            },
        }
    }

    // touch record database operation.
    pub fn touch() -> Self {
        Operation {
            _as: proto::Operation {
                op_type: proto::OperationType::Touch.into(),
                ..proto::Operation::default()
            },
        }
    }

    // delete record database operation.
    pub fn delete() -> Self {
        Operation {
            _as: proto::Operation {
                op_type: proto::OperationType::Delete.into(),
                ..proto::Operation::default()
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchRecord
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\BatchRecord")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchRecord {
    _as: proto::BatchRecord,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl BatchRecord {
    #[getter]
    pub fn get_key(&self) -> Option<Key> {
        Some(Key {
            _as: self._as.key.clone()?,
        })
    }

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

#[php_class(name = "Aerospike\\BatchRead")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchRead {
    _as: proto::BatchRead,
}

/// `WritePolicy` encapsulates parameters for all write operations.
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

    pub fn Ops(policy: &BatchReadPolicy, key: &Key, ops: Vec<&Operation>) -> Self {
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
                ops: ops.into_iter().map(|v| v._as.clone()).collect(),
            },
        }
    }

    pub fn Header(policy: &BatchReadPolicy, key: &Key) -> Self {
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

#[php_class(name = "Aerospike\\BatchWrite")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchWrite {
    _as: proto::BatchWrite,
}

/// `WritePolicy` encapsulates parameters for all write operations.
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
                ops: ops.into_iter().map(|v| v._as.clone()).collect(),
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchDelete
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\BatchDelete")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchDelete {
    _as: proto::BatchDelete,
}

/// `WritePolicy` encapsulates parameters for all write operations.
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

#[php_class(name = "Aerospike\\BatchUdf")]
#[derive(Debug, PartialEq, Clone)]
pub struct BatchUdf {
    _as: proto::BatchUdf,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl BatchUdf {
    pub fn __construct(
        policy: &BatchUdfPolicy,
        key: &Key,
        packageName: String,
        functionName: String,
        functionArgs: Vec<PHPValue>,
    ) -> Self {
        BatchUdf {
            _as: proto::BatchUdf {
                batch_record: Some(proto::BatchRecord {
                    key: Some(key._as.clone()),
                    record: None,
                    error: None,
                }),
                policy: Some(policy._as.clone()),
                package_name: packageName,
                function_name: functionName,
                function_args: functionArgs.into_iter().map(|v| v.into()).collect(),
            },
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
pub fn new_aerospike_client(socket: &str) -> PhpResult<grpc::BlockingClient> {
    let client = grpc::BlockingClient::connect(socket.into()).map_err(|e| e.to_string())?;
    Ok(client)
}

#[php_function]
pub fn Aerospike(hosts: &str) -> PhpResult<Zval> {
    match get_persisted_client(hosts) {
        Some(c) => {
            trace!("Found Aerospike Client object for {}", hosts);
            return Ok(c);
        }
        None => (),
    }

    trace!("Creating a new Aerospike Client object for {}", hosts);

    let c = Arc::new(Mutex::new(new_aerospike_client(&hosts)?));
    persist_client(hosts, c)?;

    match get_persisted_client(hosts) {
        Some(c) => {
            return Ok(c);
        }
        None => Err("Error connecting to the database".into()),
    }
}

#[php_class(name = "Aerospike\\Client")]
pub struct Client {
    client: Arc<Mutex<grpc::BlockingClient>>,
    hosts: String,
}

// This trivial implementation of `drop` adds a print to console.
impl Drop for Client {
    fn drop(&mut self) {
        trace!("Dropping client: {}, ptr: {:p}", self.hosts, &self);
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Client {
    pub fn hosts(&self) -> &str {
        &self.hosts
    }

    /// Write record bin(s). The policy specifies the transaction timeout, record expiration and
    /// how the transaction is handled when the record already exists.
    pub fn put(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<proto::Bin> = bins.into_iter().map(|b| b.into()).collect();

        let mut request = tonic::Request::new(proto::AerospikePutRequest {
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
            proto::Error {
                result_code,
                in_doubt,
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
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
    ) -> PhpResult<Record> {
        let mut request = tonic::Request::new(proto::AerospikeGetRequest {
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
            } => Ok(Record {
                _as: (*rec).clone(),
            }),
            proto::AerospikeSingleResponse {
                error:
                    Some(proto::Error {
                        result_code,
                        in_doubt,
                    }),
                record: None,
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
            _ => unreachable!(),
        }
    }

    /// Add integer bin values to existing record bin values. The policy specifies the transaction
    /// timeout, record expiration and how the transaction is handled when the record already
    /// exists. This call only works for integer values.
    pub fn add(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<proto::Bin> = bins.into_iter().map(|b| b.into()).collect();

        let mut request = tonic::Request::new(proto::AerospikePutRequest {
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
            proto::Error {
                result_code,
                in_doubt,
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
        }
    }

    /// Append bin string values to existing record bin values. The policy specifies the
    /// transaction timeout, record expiration and how the transaction is handled when the record
    /// already exists. This call only works for string values.
    pub fn append(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<proto::Bin> = bins.into_iter().map(|b| b.into()).collect();

        let mut request = tonic::Request::new(proto::AerospikePutRequest {
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
            proto::Error {
                result_code,
                in_doubt,
            } =>{
                let error = AspException { message: "Exception in append".to_string() , code: *result_code };
                let _ = throw_object(error.into_zval(true).unwrap());
                Err(AerospikeException::new("TODO(Sachin): Implement Exception").into())
            } 
        }
    }

    /// Prepend bin string values to existing record bin values. The policy specifies the
    /// transaction timeout, record expiration and how the transaction is handled when the record
    /// already exists. This call only works for string values.
    pub fn prepend(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<proto::Bin> = bins.into_iter().map(|b| b.into()).collect();

        let mut request = tonic::Request::new(proto::AerospikePutRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
            bins: bins.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.prepend(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::Error {
                result_code: 0,
                in_doubt: _,
            } => Ok(()),
            proto::Error {
                result_code,
                in_doubt,
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
        }
    }

    /// Delete record for specified key. The policy specifies the transaction timeout.
    /// The call returns `true` if the record existed on the server before deletion.
    pub fn delete(&self, policy: &WritePolicy, key: &Key) -> PhpResult<bool> {
        let mut request = tonic::Request::new(proto::AerospikeDeleteRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.delete(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeDeleteResponse {
                error: None,
                existed: Some(existed),
            } => Ok(*existed),
            proto::AerospikeDeleteResponse {
                error:
                    Some(proto::Error {
                        result_code,
                        in_doubt,
                    }),
                ..
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
            _ => unreachable!(),
        }
    }

    /// Reset record's time to expiration using the policy's expiration. Fail if the record does
    /// not exist.
    pub fn touch(&self, policy: &WritePolicy, key: &Key) -> PhpResult<()> {
        let mut request = tonic::Request::new(proto::AerospikeTouchRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.touch(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::Error {
                result_code: 0,
                in_doubt: _,
            } => Ok(()),
            proto::Error {
                result_code,
                in_doubt,
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
        }
    }

    /// Determine if a record key exists. The policy can be used to specify timeouts.
    pub fn exists(&self, policy: &ReadPolicy, key: &Key) -> PhpResult<bool> {
        let mut request = tonic::Request::new(proto::AerospikeExistsRequest {
            policy: Some(policy._as.clone()),
            key: Some(key._as.clone()),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.exists(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeExistsResponse {
                error: None,
                exists: Some(exists),
            } => Ok(*exists),
            proto::AerospikeExistsResponse {
                error:
                    Some(proto::Error {
                        result_code,
                        in_doubt,
                    }),
                ..
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
            _ => unreachable!(),
        }
    }

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
                panic!("TODO(Sachin): Implement Exception");
            }
        });

        let mut request = tonic::Request::new(proto::AerospikeBatchOperateRequest {
            policy: Some(policy._as.clone()),
            records: res,
        });

        let mut client = self.client.lock().unwrap();
        let res = client.batch_operate(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeBatchOperateResponse {
                error: None,
                records: records,
            } => Ok(records
                .into_iter()
                .map(|v| BatchRecord { _as: (*v).clone() })
                .collect()),
            proto::AerospikeBatchOperateResponse {
                error:
                    Some(proto::Error {
                        result_code,
                        in_doubt,
                    }),
                ..
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
            _ => unreachable!(),
        }
    }

    // /// Removes all records in the specified namespace/set efficiently.
    // pub fn truncate(
    //     &self,
    //     namespace: &str,
    //     set_name: &str,
    //     before_nanos: Option<i64>,
    // ) -> PhpResult<()> {
    //     let before_nanos = before_nanos.unwrap_or_default();
    //     self._as
    //         .truncate(namespace, set_name, before_nanos)
    //         .map_err(|e| e.to_string())?;
    //     Ok(())
    // }

    // /// Read all records in the specified namespace and set and return a record iterator. The scan
    // /// executor puts records on a queue in separate threads. The calling thread concurrently pops
    // /// records off the queue through the record iterator. Up to `policy.max_concurrent_nodes`
    // /// nodes are scanned in parallel. If concurrent nodes is set to zero, the server nodes are
    // /// read in series.
    // pub fn scan(
    //     &self,
    //     policy: &ScanPolicy,
    //     namespace: &str,
    //     set_name: &str,
    //     bins: Option<Vec<String>>,
    // ) -> PhpResult<Recordset> {
    //     let res = self
    //         ._as
    //         .scan(&policy._as, namespace, set_name, bins_flag(bins))
    //         .map_err(|e| e.to_string())?;
    //     Ok(res.into())
    // }

    // /// Execute a query on all server nodes and return a record iterator. The query executor puts
    // /// records on a queue in separate threads. The calling thread concurrently pops records off
    // /// the queue through the record iterator.
    // pub fn query(&self, policy: &QueryPolicy, statement: &Statement) -> PhpResult<Recordset> {
    //     let stmt = statement._as.clone();
    //     let res = self
    //         ._as
    //         .query(&policy._as, stmt)
    //         .map_err(|e| e.to_string())
    //         .map_err(|e| e.to_string())?;
    //     Ok(res.into())
    // }

    /// Create a secondary index on a bin containing scalar values. This asynchronous server call
    /// returns before the command is complete.
    pub fn create_index(
        &self,
        policy: &WritePolicy,
        namespace: &str,
        set_name: &str,
        bin_name: &str,
        index_name: &str,
        index_type: &IndexType,
        cit: &IndexCollectionType,
        ctx: Vec<&CDTContext>,
    ) -> PhpResult<()> {
        let mut request = tonic::Request::new(proto::AerospikeCreateIndexRequest {
            policy: Some(policy._as.clone()),
            namespace: namespace.into(),
            set_name: set_name.into(),
            index_name: index_name.into(),
            bin_name: bin_name.into(),
            index_type: index_type._as.into(),
            index_collection_type: cit._as.into(),
            ctx: ctx.into_iter().map(|ctx| ctx._as.clone()).collect(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.create_index(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeCreateIndexResponse { error: None } => Ok(()),
            proto::AerospikeCreateIndexResponse {
                error:
                    Some(proto::Error {
                        result_code,
                        in_doubt,
                    }),
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
            _ => unreachable!(),
        }
    }

    pub fn drop_index(
        &self,
        policy: &WritePolicy,
        namespace: &str,
        set_name: &str,
        index_name: &str,
    ) -> PhpResult<()> {
        let mut request = tonic::Request::new(proto::AerospikeDropIndexRequest {
            policy: Some(policy._as.clone()),
            namespace: namespace.into(),
            set_name: set_name.into(),
            index_name: index_name.into(),
        });

        let mut client = self.client.lock().unwrap();
        let res = client.drop_index(request).map_err(|e| e.to_string())?;
        match res.get_ref() {
            proto::AerospikeDropIndexResponse { error: None } => Ok(()),
            proto::AerospikeDropIndexResponse {
                error:
                    Some(proto::Error {
                        result_code,
                        in_doubt,
                    }),
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
            _ => unreachable!(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  AspException
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\AspException")]
#[extends(ext_php_rs::zend::ce::exception())]
pub struct AspException {
    #[prop(flags = ext_php_rs::flags::PropertyFlags::Public)]
    message: String,
    #[prop(flags = ext_php_rs::flags::PropertyFlags::Public)]
    code: i32,
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Aerospike Excetpion
//
////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
#[php_class(name = "Aerospike\\AerospikeException")]
pub struct AerospikeException {
    pub message: String,
}

impl AerospikeException {
    pub fn new(message: &str) -> Self {
        AerospikeException {
            message: message.to_string(),
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

#[php_class(name = "Aerospike\\Key")]
pub struct Key {
    _as: proto::Key,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Key {
    pub fn __construct(namespace: &str, set: &str, key: PHPValue) -> Self {
        let _as = proto::Key {
            digest: vec![].into(), // TODO(Khosrow): Implement
            namespace: Some(namespace.into()),
            set: Some(set.into()),
            value: Some(key.into()),
        };
        Key { _as: _as }
    }

    #[getter]
    pub fn get_namespace(&self) -> String {
        self._as.namespace.clone().unwrap_or("".into())
    }

    #[getter]
    pub fn get_setname(&self) -> String {
        self._as.set.clone().unwrap_or("".into())
    }

    #[getter]
    pub fn get_value(&self) -> Option<PHPValue> {
        self._as.value.clone().map(|v| (&v.clone()).into())
    }

    // #[getter]
    // pub fn get_digest(&self) -> Option<String> {
    //     Some(hex::encode(self._as.digest))
    // }
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
    #[getter]
    pub fn get_value(&self) -> HashMap<String, PHPValue> {
        self.v.clone()
    }

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
//  HLL
//
////////////////////////////////////////////////////////////////////////////////////////////

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

// Container for bin values stored in the Aerospike database.
#[derive(Debug, Clone, PartialEq, Eq)]
//TODO: underlying_value; convert to proto::Value to avoid conversions
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
    HashMap(HashMap<PHPValue, PHPValue>),
    /// Map data type is a collection of key-value pairs. Each key can only appear once in a
    /// collection and is associated with a value. Map keys and values can be any supported data
    /// type.
    Json(HashMap<String, PHPValue>),
    /// GeoJSON data type are JSON formatted strings to encode geospatial information.
    GeoJSON(String),

    /// HLL value
    HLL(Vec<u8>),
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
            PHPValue::HashMap(_) => panic!("HashMaps cannot be used as map keys."),
            PHPValue::Json(_) => panic!("Jsons cannot be used as map keys."),
            // PHPValue::OrderedMap(_) => panic!("OrderedMaps cannot be used as map keys."),
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
            PHPValue::Blob(ref val) => format!("{:?}", val),
            PHPValue::HLL(ref val) => format!("HLL('{:?}')", val),
            PHPValue::List(ref val) => format!("{:?}", val),
            PHPValue::HashMap(ref val) => format!("{:?}", val),
            PHPValue::Json(ref val) => format!("{:?}", val),
            // PHPValue::OrderedMap(ref val) => format!("{:?}", val),
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
            PHPValue::Blob(b) => zv.set_binary(b),
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
            PHPValue::HLL(b) => {
                let hll = HLL { v: b };
                let zo: ZBox<ZendObject> = hll.into_zend_object()?;
                zv.set_object(zo.into_raw());
            }
        }

        Ok(())
    }
}

fn from_zval(zval: &Zval) -> Option<PHPValue> {
    match zval.get_type() {
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
        DataType::Object(_) => {
            if let Some(o) = zval.extract::<GeoJSON>() {
                return Some(PHPValue::GeoJSON(o.v));
            } else if let Some(o) = zval.extract::<HLL>() {
                return Some(PHPValue::HLL(o.v));
            }
            panic!("Invalid value");
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
            hash.insert(PHPValue::String(k.into()), v.into());
        });
        PHPValue::HashMap(hash)
    }
}

impl From<PHPValue> for proto::Value {
    fn from(other: PHPValue) -> Self {
        match other {
            PHPValue::Nil => proto::Value {
                nil: Some(true),
                ..Default::default()
            }, //aerospike_core::Value::Nil,
            PHPValue::Bool(b) => proto::Value {
                b: Some(b),
                ..Default::default()
            }, //aerospike_core::Value::Bool(b),
            PHPValue::Int(i) => proto::Value {
                i: Some(i),
                ..Default::default()
            }, //aerospike_core::Value::Int(i),
            PHPValue::UInt(ui) => proto::Value {
                i: Some(ui as i64),
                ..Default::default()
            }, //aerospike_core::Value::UInt(ui),
            PHPValue::Float(f) => proto::Value {
                f: Some(f64::from(f).into()),
                ..Default::default()
            }, //aerospike_core::Value::Float(f64::from(f).into()),
            PHPValue::String(s) => proto::Value {
                s: Some(s),
                ..Default::default()
            }, //aerospike_core::Value::String(s),
            PHPValue::Blob(b) => proto::Value {
                blob: Some(b),
                ..Default::default()
            }, //aerospike_core::Value::Blob(b),
            PHPValue::List(l) => {
                let mut nl = Vec::<proto::Value>::with_capacity(l.len());
                l.iter().for_each(|v| nl.push(v.clone().into()));
                proto::Value {
                    l: nl,
                    ..Default::default()
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
                    m: arr,
                    ..Default::default()
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
                    json: arr,
                    ..Default::default()
                }
            }
            PHPValue::GeoJSON(gj) => proto::Value {
                geo: Some(gj),
                ..Default::default()
            }, //aerospike_core::Value::GeoJSON(gj),
            PHPValue::HLL(b) => proto::Value {
                hll: Some(b),
                ..Default::default()
            }, //aerospike_core::Value::HLL(b),
        }
    }
}

impl From<&proto::Value> for PHPValue {
    fn from(other: &proto::Value) -> Self {
        match other {
            &proto::Value {
                nil: Some(true), ..
            } => PHPValue::Nil,
            &proto::Value { b: Some(b), .. } => PHPValue::Bool(b),
            &proto::Value { i: Some(i), .. } => PHPValue::Int(i),
            &proto::Value { f: Some(f), .. } => {
                PHPValue::Float(ordered_float::OrderedFloat(f.into()))
            }
            proto::Value { s: Some(s), .. } => PHPValue::String(s.into()),
            proto::Value {
                blob: Some(blob), ..
            } => PHPValue::Blob(blob.to_vec()),
            proto::Value { l: l, .. } => {
                let mut nl = Vec::<PHPValue>::with_capacity(l.len());
                l.iter().for_each(|v| nl.push(v.into()));
                PHPValue::List(nl)
            }
            proto::Value { json: j, .. } => {
                let mut arr = HashMap::<String, PHPValue>::with_capacity(j.len());
                j.iter().for_each(|me| {
                    arr.insert(me.k.clone(), (&me.v.clone().unwrap()).into());
                });
                PHPValue::Json(arr)
            }
            proto::Value { m: h, .. } => {
                let mut arr = HashMap::<PHPValue, PHPValue>::with_capacity(h.len());
                h.iter().for_each(|me| {
                    arr.insert(
                        (&me.k.clone().unwrap()).into(),
                        (&me.v.clone().unwrap()).into(),
                    );
                });
                PHPValue::HashMap(arr)
            }
            proto::Value { geo: Some(gj), .. } => PHPValue::GeoJSON(gj.into()),
            proto::Value { hll: Some(b), .. } => PHPValue::HLL(b.to_vec()),
            _ => unreachable!(),
        }
    }
}

// ////////////////////////////////////////////////////////////////////////////////////////////
// //
// //  Value
// //
// ////////////////////////////////////////////////////////////////////////////////////////////

// #[php_class(name = "Aerospike\\Value")]
// pub struct Value;

// #[php_impl]
// #[derive(ZvalConvert)]
// impl Value {
//     pub fn nil() -> PHPValue {
//         PHPValue::Nil
//     }

//     pub fn int(val: i64) -> PHPValue {
//         PHPValue::Int(val)
//     }

//     pub fn uint(val: u64) -> PHPValue {
//         PHPValue::UInt(val)
//     }

//     pub fn string(val: String) -> PHPValue {
//         PHPValue::String(val)
//     }

//     pub fn blob(val: Vec<u8>) -> PHPValue {
//         PHPValue::Blob(val)
//     }

//     pub fn geo_json(val: String) -> GeoJSON {
//         GeoJSON { v: val }
//     }

//     pub fn hll(val: Vec<u8>) -> HLL {
//         HLL { v: val }
//     }
// }

// ////////////////////////////////////////////////////////////////////////////////////////////
// //
// //  Converters
// //
// ////////////////////////////////////////////////////////////////////////////////////////////

// impl From<aerospike_core::Bin> for Bin {
//     fn from(other: aerospike_core::Bin) -> Self {
//         Bin { _as: other }
//     }
// }

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

// impl From<Arc<aerospike_core::Recordset>> for Recordset {
//     fn from(other: Arc<aerospike_core::Recordset>) -> Self {
//         Recordset { _as: other }
//     }
// }

#[derive(Debug)]
pub struct AeroPHPError(String);

impl std::fmt::Display for AeroPHPError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// impl std::error::Error for AeroPHPError {}

// impl std::convert::From<String> for AeroPHPError {
//     fn from(msg: String) -> Self {
//         Self(msg)
//     }
// }

// TODO: Implement the Aerospike::Exception class
// impl From<aerospike_core::Error> for AeroPHPError {
//     fn from(e: aerospike_core::Error) -> Self {
//         Self(e.to_string())
//     }
// }

// impl From<AeroPHPError> for ext_php_rs::error::Error {
//     fn from(e: AeroPHPError) -> Self {
//         Self(e.to_string())
//     }
// }

////////////////////////////////////////////////////////////////////////////////////////////
//
//  utility methods
//
////////////////////////////////////////////////////////////////////////////////////////////

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
        hosts: key.into(),
    };

    let mut zval = Zval::new();
    let zo: ZBox<ZendObject> = client.into_zend_object().ok()?;
    zval.set_object(zo.into_raw());
    Some(zval)
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
}

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         let cp = crate::ClientPolicy::__construct();
//         let client = crate::Aerospike(&mut cp, "localhost:3000");

//         let policy = crate::ReadPolicy::__construct();
//         let key = crate::Key::__construct("test".into(), "test".into(), crate::PHPValue::Int(1));
//         let res = client.get(&policy, &key, None).unwrap();

//         println!("{}", res._as.bins.ro_s());
//     }
// }
