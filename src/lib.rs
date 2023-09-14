#![cfg_attr(windows, feature(abi_vectorcall))]

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
use ext_php_rs::convert::{FromZval, IntoZval};
use ext_php_rs::error::Result;
use ext_php_rs::flags::DataType;
use ext_php_rs::php_class;
use ext_php_rs::types::ZendHashTable;
use ext_php_rs::types::ZendObject;
use ext_php_rs::types::Zval;

use aerospike_core::as_geo;
use aerospike_core::as_val;

use chrono::Local;
use colored::*;
use lazy_static::lazy_static;
use log::LevelFilter;
use log::{debug, info, trace, warn};

lazy_static! {
    static ref CLIENTS: Mutex<HashMap<String, Arc<aerospike_sync::Client>>> =
        Mutex::new(HashMap::new());
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ExpressionType (ExpType)
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Expression Data Types for usage in some `FilterExpressions`
#[derive(Debug, Clone, Copy)]
pub enum _ExpType {
    NIL,
    BOOL,
    INT,
    STRING,
    LIST,
    MAP,
    BLOB,
    FLOAT,
    GEO,
    HLL,
}

#[php_class]
pub struct ExpType {
    _as: aerospike_core::expressions::ExpType,
    v: _ExpType,
}

impl FromZval<'_> for ExpType {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &ExpType = zval.extract()?;

        Some(ExpType {
            _as: f._as.clone(),
            v: f.v.clone(),
        })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl ExpType {
    pub fn nil() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::NIL,
            v: _ExpType::NIL,
        }
    }

    pub fn bool() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::BOOL,
            v: _ExpType::BOOL,
        }
    }

    pub fn int() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::INT,
            v: _ExpType::INT,
        }
    }

    pub fn string() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::STRING,
            v: _ExpType::STRING,
        }
    }

    pub fn list() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::LIST,
            v: _ExpType::LIST,
        }
    }

    pub fn map() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::MAP,
            v: _ExpType::MAP,
        }
    }

    pub fn blob() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::BLOB,
            v: _ExpType::BLOB,
        }
    }

    pub fn float() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::FLOAT,
            v: _ExpType::FLOAT,
        }
    }

    pub fn geo() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::GEO,
            v: _ExpType::GEO,
        }
    }

    pub fn hll() -> Self {
        ExpType {
            _as: aerospike_core::expressions::ExpType::HLL,
            v: _ExpType::HLL,
        }
    }
}

impl From<&ExpType> for aerospike_core::expressions::ExpType {
    fn from(input: &ExpType) -> Self {
        match &input.v {
            _ExpType::NIL => aerospike_core::expressions::ExpType::NIL,
            _ExpType::BOOL => aerospike_core::expressions::ExpType::BOOL,
            _ExpType::INT => aerospike_core::expressions::ExpType::INT,
            _ExpType::STRING => aerospike_core::expressions::ExpType::STRING,
            _ExpType::LIST => aerospike_core::expressions::ExpType::LIST,
            _ExpType::MAP => aerospike_core::expressions::ExpType::MAP,
            _ExpType::BLOB => aerospike_core::expressions::ExpType::BLOB,
            _ExpType::FLOAT => aerospike_core::expressions::ExpType::FLOAT,
            _ExpType::GEO => aerospike_core::expressions::ExpType::GEO,
            _ExpType::HLL => aerospike_core::expressions::ExpType::HLL,
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
#[php_class]
pub struct FilterExpression {
    _as: aerospike_core::expressions::FilterExpression,
}

impl FromZval<'_> for FilterExpression {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &FilterExpression = zval.extract()?;

        Some(FilterExpression { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl FilterExpression {
    /// Create a record key expression of specified type.
    pub fn key(exp_type: ExpType) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::key(exp_type._as),
        }
    }

    /// Create function that returns if the primary key is stored in the record meta data
    /// as a boolean expression. This would occur when `send_key` is true on record write.
    pub fn key_exists() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::key_exists(),
        }
    }

    /// Create 64 bit int bin expression.
    pub fn int_bin(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_bin(name),
        }
    }

    /// Create string bin expression.
    pub fn string_bin(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::string_bin(name),
        }
    }

    /// Create blob bin expression.
    pub fn blob_bin(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::blob_bin(name),
        }
    }

    /// Create 64 bit float bin expression.
    pub fn float_bin(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::float_bin(name),
        }
    }

    /// Create geo bin expression.
    pub fn geo_bin(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::geo_bin(name),
        }
    }

    /// Create list bin expression.
    pub fn list_bin(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::list_bin(name),
        }
    }

    /// Create map bin expression.
    pub fn map_bin(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::map_bin(name),
        }
    }

    /// Create a HLL bin expression
    pub fn hll_bin(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::hll_bin(name),
        }
    }

    /// Create function that returns if bin of specified name exists.
    pub fn bin_exists(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::ne(
                aerospike_core::expressions::bin_type(name),
                aerospike_core::expressions::int_val(0 as i64),
            ),
        }
    }

    /// Create function that returns bin's integer particle type.
    pub fn bin_type(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::bin_type(name),
        }
    }

    /// Create function that returns record set name string.
    pub fn set_name() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::set_name(),
        }
    }

    /// Create function that returns record size on disk.
    /// If server storage-engine is memory, then zero is returned.
    pub fn device_size() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::device_size(),
        }
    }

    /// Create function that returns record last update time expressed as 64 bit integer
    /// nanoseconds since 1970-01-01 epoch.
    pub fn last_update() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::last_update(),
        }
    }

    /// Create expression that returns milliseconds since the record was last updated.
    /// This expression usually evaluates quickly because record meta data is cached in memory.
    pub fn since_update() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::since_update(),
        }
    }

    /// Create function that returns record expiration time expressed as 64 bit integer
    /// nanoseconds since 1970-01-01 epoch.
    pub fn void_time() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::void_time(),
        }
    }

    /// Create function that returns record expiration time (time to live) in integer seconds.
    pub fn ttl() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::ttl(),
        }
    }

    /// Create expression that returns if record has been deleted and is still in tombstone state.
    /// This expression usually evaluates quickly because record meta data is cached in memory.
    pub fn is_tombstone() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::is_tombstone(),
        }
    }

    /// Create function that returns record digest modulo as integer.
    pub fn digest_modulo(modulo: i64) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::digest_modulo(modulo),
        }
    }

    /// Create function like regular expression string operation.
    pub fn regex_compare(regex: String, flags: i64, bin: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::regex_compare(regex, flags, bin._as),
        }
    }

    /// Create compare geospatial operation.
    pub fn geo_compare(left: FilterExpression, right: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::geo_compare(left._as, right._as),
        }
    }

    /// Creates 64 bit integer value
    pub fn int_val(val: i64) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_val(val),
        }
    }

    /// Creates a Boolean value
    pub fn bool_val(val: bool) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::bool_val(val),
        }
    }

    /// Creates String bin value
    pub fn string_val(val: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::string_val(val),
        }
    }

    /// Creates 64 bit float bin value
    pub fn float_val(val: f64) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::float_val(val),
        }
    }

    /// Creates Blob bin value
    pub fn blob_val(val: Vec<u8>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::blob_val(val),
        }
    }

    /// Create List bin PHPValue
    /// Not Supported in pre-alpha release
    // pub fn list_val(val: Vec<PHPValue>) -> Self {
    //     FilterExpression {
    //         _as: aerospike_core::expressions::list_val(val)
    //     }
    // }

    /// Create Map bin PHPValue
    /// Not Supported in pre-alpha release
    // pub fn map_val(val: HashMap<PHPValue, PHPValue>) -> Self {
    //     FilterExpression {
    //         _as: aerospike_core::expressions::map_val(val)
    //     }
    // }

    /// Create geospatial json string value.
    pub fn geo_val(val: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::geo_val(val),
        }
    }

    /// Create a Nil PHPValue
    pub fn nil() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::nil(),
        }
    }

    /// Create "not" operator expression.
    pub fn not(exp: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::not(exp._as),
        }
    }

    /// Create "and" (&&) operator that applies to a variable number of expressions.
    /// // (a > 5 || a == 0) && b < 3
    pub fn and(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::and(exps.into_iter().map(|exp| exp._as).collect()),
        }
    }

    /// Create "or" (||) operator that applies to a variable number of expressions.
    pub fn or(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::or(exps.into_iter().map(|exp| exp._as).collect()),
        }
    }

    /// Create "xor" (^) operator that applies to a variable number of expressions.
    pub fn xor(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::xor(exps.into_iter().map(|exp| exp._as).collect()),
        }
    }

    /// Create equal (==) expression.
    pub fn eq(left: FilterExpression, right: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::eq(left._as, right._as),
        }
    }

    /// Create not equal (!=) expression
    pub fn ne(left: FilterExpression, right: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::ne(left._as, right._as),
        }
    }

    /// Create greater than (>) operation.
    pub fn gt(left: FilterExpression, right: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::gt(left._as, right._as),
        }
    }

    /// Create greater than or equal (>=) operation.
    pub fn ge(left: FilterExpression, right: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::ge(left._as, right._as),
        }
    }

    /// Create less than (<) operation.
    pub fn lt(left: FilterExpression, right: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::lt(left._as, right._as),
        }
    }

    /// Create less than or equals (<=) operation.
    pub fn le(left: FilterExpression, right: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::le(left._as, right._as),
        }
    }

    /// Create "add" (+) operator that applies to a variable number of expressions.
    /// Return sum of all `FilterExpressions` given. All arguments must resolve to the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn num_add(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_add(
                exps.into_iter().map(|exp| exp._as).collect(),
            ),
        }
    }

    /// Create "subtract" (-) operator that applies to a variable number of expressions.
    /// If only one `FilterExpressions` is provided, return the negation of that argument.
    /// Otherwise, return the sum of the 2nd to Nth `FilterExpressions` subtracted from the 1st
    /// `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn num_sub(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_sub(
                exps.into_iter().map(|exp| exp._as).collect(),
            ),
        }
    }

    /// Create "multiply" (*) operator that applies to a variable number of expressions.
    /// Return the product of all `FilterExpressions`. If only one `FilterExpressions` is supplied, return
    /// that `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn num_mul(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_mul(
                exps.into_iter().map(|exp| exp._as).collect(),
            ),
        }
    }

    /// Create "divide" (/) operator that applies to a variable number of expressions.
    /// If there is only one `FilterExpressions`, returns the reciprocal for that `FilterExpressions`.
    /// Otherwise, return the first `FilterExpressions` divided by the product of the rest.
    /// All `FilterExpressions` must resolve to the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn num_div(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_div(
                exps.into_iter().map(|exp| exp._as).collect(),
            ),
        }
    }

    /// Create "power" operator that raises a "base" to the "exponent" power.
    /// All arguments must resolve to floats.
    /// Requires server version 5.6.0+.
    pub fn num_pow(base: FilterExpression, exponent: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_pow(base._as, exponent._as),
        }
    }

    /// Create "log" operator for logarithm of "num" with base "base".
    /// All arguments must resolve to floats.
    /// Requires server version 5.6.0+.
    pub fn num_log(num: FilterExpression, base: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_log(num._as, base._as),
        }
    }

    /// Create "modulo" (%) operator that determines the remainder of "numerator"
    /// divided by "denominator". All arguments must resolve to integers.
    /// Requires server version 5.6.0+.
    pub fn num_mod(numerator: FilterExpression, denominator: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_mod(numerator._as, denominator._as),
        }
    }

    /// Create operator that returns absolute value of a number.
    /// All arguments must resolve to integer or float.
    /// Requires server version 5.6.0+.
    pub fn num_abs(value: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_abs(value._as),
        }
    }

    /// Create expression that rounds a floating point number down to the closest integer value.
    /// The return type is float.
    // Requires server version 5.6.0+.
    pub fn num_floor(num: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_floor(num._as),
        }
    }

    /// Create expression that rounds a floating point number up to the closest integer value.
    /// The return type is float.
    /// Requires server version 5.6.0+.
    pub fn num_ceil(num: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::num_ceil(num._as),
        }
    }

    /// Create expression that converts an integer to a float.
    /// Requires server version 5.6.0+.
    pub fn to_int(num: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::to_int(num._as),
        }
    }

    /// Create expression that converts a float to an integer.
    /// Requires server version 5.6.0+.
    pub fn to_float(num: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::to_float(num._as),
        }
    }

    /// Create integer "and" (&) operator that is applied to two or more integers.
    /// All arguments must resolve to integers.
    /// Requires server version 5.6.0+.
    pub fn int_and(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_and(
                exps.into_iter().map(|exp| exp._as).collect(),
            ),
        }
    }

    /// Create integer "or" (|) operator that is applied to two or more integers.
    /// All arguments must resolve to integers.
    /// Requires server version 5.6.0+.
    pub fn int_or(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_or(exps.into_iter().map(|exp| exp._as).collect()),
        }
    }

    /// Create integer "xor" (^) operator that is applied to two or more integers.
    /// All arguments must resolve to integers.
    /// Requires server version 5.6.0+.
    pub fn int_xor(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_xor(
                exps.into_iter().map(|exp| exp._as).collect(),
            ),
        }
    }

    /// Create integer "not" (~) operator.
    /// Requires server version 5.6.0+.
    pub fn int_not(exp: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_not(exp._as),
        }
    }

    /// Create integer "left shift" (<<) operator.
    /// Requires server version 5.6.0+.
    pub fn int_lshift(value: FilterExpression, shift: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_lshift(value._as, shift._as),
        }
    }

    /// Create integer "logical right shift" (>>>) operator.
    /// Requires server version 5.6.0+.
    pub fn int_rshift(value: FilterExpression, shift: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_rshift(value._as, shift._as),
        }
    }

    /// Create integer "arithmetic right shift" (>>) operator.
    /// The sign bit is preserved and not shifted.
    /// Requires server version 5.6.0+.
    pub fn int_arshift(value: FilterExpression, shift: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_arshift(value._as, shift._as),
        }
    }

    /// Create expression that returns count of integer bits that are set to 1.
    /// Requires server version 5.6.0+
    pub fn int_count(exp: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_count(exp._as),
        }
    }

    /// Create expression that scans integer bits from left (most significant bit) to
    /// right (least significant bit), looking for a search bit value. When the
    /// search value is found, the index of that bit (where the most significant bit is
    /// index 0) is returned. If "search" is true, the scan will search for the bit
    /// value 1. If "search" is false it will search for bit value 0.
    /// Requires server version 5.6.0+.
    pub fn int_lscan(value: FilterExpression, search: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_lscan(value._as, search._as),
        }
    }

    /// Create expression that scans integer bits from right (least significant bit) to
    /// left (most significant bit), looking for a search bit value. When the
    /// search value is found, the index of that bit (where the most significant bit is
    /// index 0) is returned. If "search" is true, the scan will search for the bit
    /// value 1. If "search" is false it will search for bit value 0.
    /// Requires server version 5.6.0+.
    pub fn int_rscan(value: FilterExpression, search: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::int_rscan(value._as, search._as),
        }
    }

    /// Create expression that returns the minimum value in a variable number of expressions.
    /// All arguments must be the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn min(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::min(exps.into_iter().map(|exp| exp._as).collect()),
        }
    }

    /// Create expression that returns the maximum value in a variable number of expressions.
    /// All arguments must be the same type (integer or float).
    /// Requires server version 5.6.0+.
    pub fn max(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::max(exps.into_iter().map(|exp| exp._as).collect()),
        }
    }

    //--------------------------------------------------
    // Variables
    //--------------------------------------------------

    /// Conditionally select an expression from a variable number of expression pairs
    /// followed by default expression action.
    /// Requires server version 5.6.0+.
    /// ```
    /// // Args Format: bool exp1, action exp1, bool exp2, action exp2, ..., action-default
    /// // Apply operator based on type.
    pub fn cond(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::cond(exps.into_iter().map(|exp| exp._as).collect()),
        }
    }

    /// Define variables and expressions in scope.
    /// Requires server version 5.6.0+.
    /// ```
    /// // 5 < a < 10
    pub fn exp_let(exps: Vec<FilterExpression>) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::exp_let(
                exps.into_iter().map(|exp| exp._as).collect(),
            ),
        }
    }

    /// Assign variable to an expression that can be accessed later.
    /// Requires server version 5.6.0+.
    /// ```
    /// // 5 < a < 10
    pub fn def(name: String, value: FilterExpression) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::def(name, value._as),
        }
    }

    /// Retrieve expression value from a variable.
    /// Requires server version 5.6.0+.
    pub fn var(name: String) -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::var(name),
        }
    }

    /// Create unknown value. Used to intentionally fail an expression.
    /// The failure can be ignored with `ExpWriteFlags` `EVAL_NO_FAIL`
    /// or `ExpReadFlags` `EVAL_NO_FAIL`.
    /// Requires server version 5.6.0+.
    pub fn unknown() -> Self {
        FilterExpression {
            _as: aerospike_core::expressions::unknown(),
        }
    }
}

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

#[php_class]
pub struct Priority {
    _as: aerospike_core::Priority,
    v: _Priority,
}

impl FromZval<'_> for Priority {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Priority = zval.extract()?;

        Some(Priority {
            _as: f._as.clone(),
            v: f.v.clone(),
        })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Priority {
    /// Default determines that the server defines the priority.
    pub fn default() -> Self {
        Priority {
            _as: aerospike_core::Priority::Default,
            v: _Priority::Default,
        }
    }

    /// Low determines that the server should run the operation in a background thread.
    pub fn low() -> Self {
        Priority {
            _as: aerospike_core::Priority::Low,
            v: _Priority::Low,
        }
    }

    /// Medium determines that the server should run the operation at medium priority.
    pub fn medium() -> Self {
        Priority {
            _as: aerospike_core::Priority::Medium,
            v: _Priority::Medium,
        }
    }

    /// High determines that the server should run the operation at the highest priority.
    pub fn high() -> Self {
        Priority {
            _as: aerospike_core::Priority::High,
            v: _Priority::High,
        }
    }
}

impl From<&Priority> for aerospike_core::Priority {
    fn from(input: &Priority) -> Self {
        match &input.v {
            _Priority::Default => aerospike_core::Priority::Default,
            _Priority::Low => aerospike_core::Priority::Low,
            _Priority::Medium => aerospike_core::Priority::Medium,
            _Priority::High => aerospike_core::Priority::High,
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

#[php_class]
pub struct RecordExistsAction {
    _as: aerospike_core::RecordExistsAction,
    v: _RecordExistsAction,
}

impl FromZval<'_> for RecordExistsAction {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &RecordExistsAction = zval.extract()?;

        Some(RecordExistsAction {
            _as: f._as.clone(),
            v: f.v.clone(),
        })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl RecordExistsAction {
    /// Update means: Create or update record.
    /// Merge write command bins with existing bins.
    pub fn update() -> Self {
        RecordExistsAction {
            _as: aerospike_core::RecordExistsAction::Update,
            v: _RecordExistsAction::Update,
        }
    }

    /// UpdateOnly means: Update record only. Fail if record does not exist.
    /// Merge write command bins with existing bins.
    pub fn update_only() -> Self {
        RecordExistsAction {
            _as: aerospike_core::RecordExistsAction::UpdateOnly,
            v: _RecordExistsAction::UpdateOnly,
        }
    }

    /// Replace means: Create or replace record.
    /// Delete existing bins not referenced by write command bins.
    /// Supported by Aerospike 2 server versions >= 2.7.5 and
    /// Aerospike 3 server versions >= 3.1.6.
    pub fn replace() -> Self {
        RecordExistsAction {
            _as: aerospike_core::RecordExistsAction::Replace,
            v: _RecordExistsAction::Replace,
        }
    }

    /// ReplaceOnly means: Replace record only. Fail if record does not exist.
    /// Delete existing bins not referenced by write command bins.
    /// Supported by Aerospike 2 server versions >= 2.7.5 and
    /// Aerospike 3 server versions >= 3.1.6.
    pub fn replace_only() -> Self {
        RecordExistsAction {
            _as: aerospike_core::RecordExistsAction::ReplaceOnly,
            v: _RecordExistsAction::ReplaceOnly,
        }
    }

    /// CreateOnly means: Create only. Fail if record exists.
    pub fn create_only() -> Self {
        RecordExistsAction {
            _as: aerospike_core::RecordExistsAction::CreateOnly,
            v: _RecordExistsAction::CreateOnly,
        }
    }
}

impl From<&RecordExistsAction> for aerospike_core::RecordExistsAction {
    fn from(input: &RecordExistsAction) -> Self {
        match &input.v {
            _RecordExistsAction::Update => aerospike_core::RecordExistsAction::Update,
            _RecordExistsAction::UpdateOnly => aerospike_core::RecordExistsAction::UpdateOnly,
            _RecordExistsAction::Replace => aerospike_core::RecordExistsAction::Replace,
            _RecordExistsAction::ReplaceOnly => aerospike_core::RecordExistsAction::ReplaceOnly,
            _RecordExistsAction::CreateOnly => aerospike_core::RecordExistsAction::CreateOnly,
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

#[php_class]
pub struct CommitLevel {
    _as: aerospike_core::CommitLevel,
    v: _CommitLevel,
}

impl FromZval<'_> for CommitLevel {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &CommitLevel = zval.extract()?;

        Some(CommitLevel {
            _as: f._as.clone(),
            v: f.v.clone(),
        })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl CommitLevel {
    /// CommitAll indicates the server should wait until successfully committing master and all
    /// replicas.
    pub fn commit_all() -> Self {
        CommitLevel {
            _as: aerospike_core::CommitLevel::CommitAll,
            v: _CommitLevel::CommitAll,
        }
    }

    /// CommitMaster indicates the server should wait until successfully committing master only.
    pub fn commit_master() -> Self {
        CommitLevel {
            _as: aerospike_core::CommitLevel::CommitMaster,
            v: _CommitLevel::CommitMaster,
        }
    }
}

impl From<&CommitLevel> for aerospike_core::CommitLevel {
    fn from(input: &CommitLevel) -> Self {
        match &input.v {
            _CommitLevel::CommitAll => aerospike_core::CommitLevel::CommitAll,
            _CommitLevel::CommitMaster => aerospike_core::CommitLevel::CommitMaster,
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
#[php_class]
pub struct ConsistencyLevel {
    _as: aerospike_core::ConsistencyLevel,
    v: _ConsistencyLevel,
}

impl FromZval<'_> for ConsistencyLevel {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &ConsistencyLevel = zval.extract()?;

        Some(ConsistencyLevel {
            _as: f._as.clone(),
            v: f.v,
        })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl ConsistencyLevel {
    /// ConsistencyOne indicates only a single replica should be consulted in
    /// the read operation.
    pub fn consistency_one() -> Self {
        ConsistencyLevel {
            _as: aerospike_core::ConsistencyLevel::ConsistencyOne,
            v: _ConsistencyLevel::ConsistencyOne,
        }
    }

    /// ConsistencyAll indicates that all replicas should be consulted in
    /// the read operation.
    pub fn consistency_all() -> Self {
        ConsistencyLevel {
            _as: aerospike_core::ConsistencyLevel::ConsistencyAll,
            v: _ConsistencyLevel::ConsistencyAll,
        }
    }
}

impl From<&ConsistencyLevel> for aerospike_core::ConsistencyLevel {
    fn from(input: &ConsistencyLevel) -> Self {
        match &input.v {
            _ConsistencyLevel::ConsistencyOne => aerospike_core::ConsistencyLevel::ConsistencyOne,
            _ConsistencyLevel::ConsistencyAll => aerospike_core::ConsistencyLevel::ConsistencyAll,
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
#[php_class]
pub struct GenerationPolicy {
    _as: aerospike_core::GenerationPolicy,
    v: _GenerationPolicy,
}

impl FromZval<'_> for GenerationPolicy {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &GenerationPolicy = zval.extract()?;

        Some(GenerationPolicy {
            _as: f._as.clone(),
            v: f.v.clone(),
        })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl GenerationPolicy {
    /// None means: Do not use record generation to restrict writes.
    pub fn none() -> Self {
        GenerationPolicy {
            _as: aerospike_core::GenerationPolicy::None,
            v: _GenerationPolicy::None,
        }
    }

    /// ExpectGenEqual means: Update/delete record if expected generation is equal to server
    /// generation. Otherwise, fail.
    pub fn expect_gen_equal() -> Self {
        GenerationPolicy {
            _as: aerospike_core::GenerationPolicy::ExpectGenEqual,
            v: _GenerationPolicy::ExpectGenEqual,
        }
    }

    /// ExpectGenGreater means: Update/delete record if expected generation greater than the server
    /// generation. Otherwise, fail. This is useful for restore after backup.
    pub fn expect_gen_greater() -> Self {
        GenerationPolicy {
            _as: aerospike_core::GenerationPolicy::ExpectGenGreater,
            v: _GenerationPolicy::ExpectGenGreater,
        }
    }
}

impl From<&GenerationPolicy> for aerospike_core::GenerationPolicy {
    fn from(input: &GenerationPolicy) -> Self {
        match &input.v {
            _GenerationPolicy::None => aerospike_core::GenerationPolicy::None,
            _GenerationPolicy::ExpectGenEqual => aerospike_core::GenerationPolicy::ExpectGenEqual,
            _GenerationPolicy::ExpectGenGreater => {
                aerospike_core::GenerationPolicy::ExpectGenGreater
            }
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
#[php_class]
pub struct Expiration {
    _as: aerospike_core::Expiration,
    v: _Expiration,
}

impl FromZval<'_> for Expiration {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Expiration = zval.extract()?;

        Some(Expiration {
            _as: f._as.clone(),
            v: f.v.clone(),
        })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Expiration {
    /// Set the record to expire X seconds from now
    pub fn seconds(seconds: u32) -> Self {
        Expiration {
            _as: aerospike_core::Expiration::Seconds(seconds),
            v: _Expiration::Seconds(seconds),
        }
    }

    /// Set the record's expiry time using the default time-to-live (TTL) value for the namespace
    pub fn namespace_default() -> Self {
        Expiration {
            _as: aerospike_core::Expiration::NamespaceDefault,
            v: _Expiration::NamespaceDefault,
        }
    }

    /// Set the record to never expire. Requires Aerospike 2 server version 2.7.2 or later or
    /// Aerospike 3 server version 3.1.4 or later. Do not use with older servers.
    pub fn never() -> Self {
        Expiration {
            _as: aerospike_core::Expiration::Never,
            v: _Expiration::Never,
        }
    }

    /// Do not change the record's expiry time when updating the record; requires Aerospike server
    /// version 3.10.1 or later.
    pub fn dont_update() -> Self {
        Expiration {
            _as: aerospike_core::Expiration::DontUpdate,
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

impl From<&Expiration> for aerospike_core::Expiration {
    fn from(exp: &Expiration) -> Self {
        match &exp.v {
            _Expiration::Seconds(secs) => aerospike_core::Expiration::Seconds(*secs),
            _Expiration::NamespaceDefault => aerospike_core::Expiration::NamespaceDefault,
            _Expiration::Never => aerospike_core::Expiration::Never,
            _Expiration::DontUpdate => aerospike_core::Expiration::DontUpdate,
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
    MaxThreads(usize),
}

/// Specifies whether a command, that needs to be executed on multiple cluster nodes, should be
/// executed sequentially, one node at a time, or in parallel on multiple nodes using the client's
/// thread pool.
#[php_class]
pub struct Concurrency {
    _as: aerospike_core::Concurrency,
    v: _Concurrency,
}

impl FromZval<'_> for Concurrency {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Concurrency = zval.extract()?;

        Some(Concurrency {
            _as: f._as.clone(),
            v: f.v,
        })
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
            _as: aerospike_core::Concurrency::Sequential,
            v: _Concurrency::Sequential,
        }
    }

    /// Issue all commands in parallel threads. This mode has a performance advantage for
    /// extremely large batch sizes because each node can process the request immediately. The
    /// downside is extra threads will need to be created (or takedn from a thread pool).
    pub fn parallel() -> Self {
        Concurrency {
            _as: aerospike_core::Concurrency::Parallel,
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
    pub fn max_threads(threads: usize) -> Self {
        Concurrency {
            _as: aerospike_core::Concurrency::MaxThreads(threads),
            v: _Concurrency::MaxThreads(threads),
        }
    }
}

impl From<&Concurrency> for aerospike_core::Concurrency {
    fn from(input: &Concurrency) -> Self {
        match &input.v {
            _Concurrency::Sequential => aerospike_core::Concurrency::Sequential,
            _Concurrency::Parallel => aerospike_core::Concurrency::Parallel,
            _Concurrency::MaxThreads(threads) => aerospike_core::Concurrency::MaxThreads(*threads),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BasePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct BasePolicy {
    _as: aerospike_core::policy::BasePolicy,
}

impl FromZval<'_> for BasePolicy {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &BasePolicy = zval.extract()?;

        Some(BasePolicy { _as: f._as.clone() })
    }
}

/// Trait implemented by most policy types; policies that implement this trait typically encompass
/// an instance of `BasePolicy`.
#[php_impl]
#[derive(ZvalConvert)]
impl BasePolicy {
    #[getter]
    pub fn get_priority(&self) -> Priority {
        Priority {
            _as: self._as.priority.clone(),
            v: match &self._as.priority {
                aerospike_core::Priority::Default => _Priority::Default,
                aerospike_core::Priority::Low => _Priority::Low,
                aerospike_core::Priority::Medium => _Priority::Medium,
                aerospike_core::Priority::High => _Priority::High,
            },
        }
    }

    #[setter]
    pub fn set_priority(&mut self, priority: Priority) {
        self._as.priority = priority._as;
    }

    #[getter]
    pub fn get_consistency_level(&self) -> ConsistencyLevel {
        ConsistencyLevel {
            _as: self._as.consistency_level.clone(),
            v: match &self._as.consistency_level {
                aerospike_core::ConsistencyLevel::ConsistencyOne => {
                    _ConsistencyLevel::ConsistencyOne
                }
                aerospike_core::ConsistencyLevel::ConsistencyAll => {
                    _ConsistencyLevel::ConsistencyAll
                }
            },
        }
    }

    #[setter]
    pub fn set_consistency_level(&mut self, consistency_level: ConsistencyLevel) {
        self._as.consistency_level = consistency_level._as;
    }

    #[getter]
    pub fn get_timeout(&self) -> u64 {
        self._as
            .timeout
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default()
    }

    #[setter]
    pub fn set_timeout(&mut self, timeout_millis: u64) {
        let timeout = Duration::from_millis(timeout_millis);
        self._as.timeout = Some(timeout);
    }

    #[getter]
    pub fn get_max_retries(&self) -> Option<usize> {
        self._as.max_retries
    }

    #[setter]
    pub fn set_max_retries(&mut self, max_retries: Option<usize>) {
        self._as.max_retries = max_retries;
    }

    #[getter]
    pub fn get_sleep_between_retries(&self) -> u64 {
        self._as
            .sleep_between_retries
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default()
    }

    #[setter]
    pub fn set_sleep_between_retries(&mut self, sleep_between_retries_millis: u64) {
        let sleep_between_retries = Duration::from_millis(sleep_between_retries_millis);
        self._as.timeout = Some(sleep_between_retries);
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<FilterExpression> {
        match &self._as.filter_expression {
            Some(fe) => Some(FilterExpression { _as: fe.clone() }),
            None => None,
        }
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct BatchPolicy {
    _as: aerospike_core::BatchPolicy,
}

/// `BatchPolicy` encapsulates parameters for all batch operations.
#[php_impl]
#[derive(ZvalConvert)]
impl BatchPolicy {
    pub fn __construct() -> Self {
        BatchPolicy {
            _as: aerospike_core::BatchPolicy::default(),
        }
    }

    #[getter]
    pub fn get_base_policy(&self) -> BasePolicy {
        BasePolicy {
            _as: self._as.base_policy.clone(),
        }
    }

    #[setter]
    pub fn set_base_policy(&mut self, base_policy: BasePolicy) {
        self._as.base_policy = base_policy._as;
    }

    #[getter]
    pub fn get_concurrency(&self) -> Concurrency {
        Concurrency {
            _as: self._as.concurrency, // Assuming _as.concurrency is the corresponding field in aerospike_core
            v: match &self._as.concurrency {
                aerospike_core::Concurrency::Sequential => _Concurrency::Sequential,
                aerospike_core::Concurrency::Parallel => _Concurrency::Parallel,
                aerospike_core::Concurrency::MaxThreads(threads) => {
                    _Concurrency::MaxThreads(*threads)
                }
            },
        }
    }

    #[setter]
    pub fn set_concurrency(&mut self, concurrency: Concurrency) {
        self._as.concurrency = concurrency._as;
    }

    #[getter]
    pub fn get_allow_inline(&self) -> bool {
        self._as.allow_inline
    }

    #[setter]
    pub fn set_send_set_name(&mut self, send_set_name: bool) {
        self._as.send_set_name = send_set_name;
    }

    #[getter]
    pub fn get_send_set_name(&self) -> bool {
        self._as.send_set_name
    }

    #[setter]
    pub fn set_allow_inline(&mut self, allow_inline: bool) {
        self._as.allow_inline = allow_inline;
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<FilterExpression> {
        match &self._as.filter_expression {
            Some(fe) => Some(FilterExpression { _as: fe.clone() }),
            None => None,
        }
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchRead
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct BatchRead {
    _as: aerospike_core::BatchRead,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchRead {
    pub fn __construct(key: Key, bins: Option<Vec<String>>) -> Self {
        BatchRead {
            _as: aerospike_core::BatchRead::new(key._as, bins_flag(bins)),
        }
    }

    pub fn record(&self) -> Option<Record> {
        self._as.clone().record.map(|r| r.into())
    }
}

impl From<aerospike_core::BatchRead> for BatchRead {
    fn from(other: aerospike_core::BatchRead) -> Self {
        BatchRead { _as: other }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ReadPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct ReadPolicy {
    _as: aerospike_core::ReadPolicy,
}

/// `ReadPolicy` excapsulates parameters for transaction policy attributes
/// used in all database operation calls.
#[php_impl]
#[derive(ZvalConvert)]
impl ReadPolicy {
    pub fn __construct() -> Self {
        ReadPolicy {
            _as: aerospike_core::ReadPolicy::default(),
        }
    }

    #[getter]
    pub fn get_priority(&self) -> Priority {
        Priority {
            _as: self._as.priority.clone(),
            v: match &self._as.priority {
                aerospike_sync::Priority::Default => _Priority::Default,
                aerospike_sync::Priority::Low => _Priority::Low,
                aerospike_sync::Priority::Medium => _Priority::Medium,
                aerospike_sync::Priority::High => _Priority::High,
            },
        }
    }

    #[setter]
    pub fn set_priority(&mut self, priority: Priority) {
        self._as.priority = priority._as;
    }

    #[getter]
    pub fn get_max_retries(&self) -> Option<usize> {
        self._as.max_retries
    }

    #[setter]
    pub fn set_max_retries(&mut self, max_retries: Option<usize>) {
        self._as.max_retries = max_retries;
    }

    #[getter]
    pub fn get_timeout(&self) -> u64 {
        self._as
            .timeout
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default()
    }

    #[setter]
    pub fn set_timeout(&mut self, timeout_millis: u64) {
        let timeout = Duration::from_millis(timeout_millis);
        self._as.timeout = Some(timeout);
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<FilterExpression> {
        match &self._as.filter_expression {
            Some(fe) => Some(FilterExpression { _as: fe.clone() }),
            None => None,
        }
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  WritePolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct WritePolicy {
    _as: aerospike_core::WritePolicy,
}

/// `WritePolicy` encapsulates parameters for all write operations.
#[php_impl]
#[derive(ZvalConvert)]
impl WritePolicy {
    pub fn __construct() -> Self {
        WritePolicy {
            _as: aerospike_core::WritePolicy::default(),
        }
    }

    #[getter]
    pub fn get_base_policy(&self) -> BasePolicy {
        BasePolicy {
            _as: self._as.base_policy.clone(),
        }
    }

    #[setter]
    pub fn set_base_policy(&mut self, base_policy: BasePolicy) {
        self._as.base_policy = base_policy._as;
    }

    #[getter]
    pub fn get_record_exists_action(&self) -> RecordExistsAction {
        RecordExistsAction {
            _as: self._as.record_exists_action.clone(),
            v: match &self._as.record_exists_action {
                aerospike_core::RecordExistsAction::Update => _RecordExistsAction::Update,
                aerospike_core::RecordExistsAction::UpdateOnly => _RecordExistsAction::UpdateOnly,
                aerospike_core::RecordExistsAction::Replace => _RecordExistsAction::Replace,
                aerospike_core::RecordExistsAction::ReplaceOnly => _RecordExistsAction::ReplaceOnly,
                aerospike_core::RecordExistsAction::CreateOnly => _RecordExistsAction::CreateOnly,
            },
        }
    }

    #[setter]
    pub fn set_record_exists_action(&mut self, record_exists_action: RecordExistsAction) {
        self._as.record_exists_action = record_exists_action._as;
    }

    #[getter]
    pub fn get_generation_policy(&self) -> GenerationPolicy {
        GenerationPolicy {
            _as: self._as.generation_policy.clone(),
            v: match &self._as.generation_policy {
                aerospike_core::GenerationPolicy::None => _GenerationPolicy::None,
                aerospike_core::GenerationPolicy::ExpectGenEqual => {
                    _GenerationPolicy::ExpectGenEqual
                }
                aerospike_core::GenerationPolicy::ExpectGenGreater => {
                    _GenerationPolicy::ExpectGenGreater
                }
            },
        }
    }

    #[setter]
    pub fn set_generation_policy(&mut self, generation_policy: GenerationPolicy) {
        self._as.generation_policy = generation_policy._as;
    }

    #[getter]
    pub fn get_commit_level(&self) -> CommitLevel {
        CommitLevel {
            _as: self._as.commit_level.clone(),
            v: match &self._as.commit_level {
                aerospike_core::CommitLevel::CommitAll => _CommitLevel::CommitAll,
                aerospike_core::CommitLevel::CommitMaster => _CommitLevel::CommitMaster,
            },
        }
    }

    #[setter]
    pub fn set_commit_level(&mut self, commit_level: CommitLevel) {
        self._as.commit_level = commit_level._as;
    }

    #[getter]
    pub fn get_generation(&self) -> u32 {
        self._as.generation
    }

    #[setter]
    pub fn set_generation(&mut self, generation: u32) {
        self._as.generation = generation;
    }

    #[getter]
    pub fn get_expiration(&self) -> Expiration {
        Expiration {
            _as: self._as.expiration,
            v: match &self._as.expiration {
                aerospike_core::Expiration::Seconds(secs) => _Expiration::Seconds(*secs),
                aerospike_core::Expiration::NamespaceDefault => _Expiration::NamespaceDefault,
                aerospike_core::Expiration::Never => _Expiration::Never,
                aerospike_core::Expiration::DontUpdate => _Expiration::DontUpdate,
            },
        }
    }

    #[setter]
    pub fn set_expiration(&mut self, expiration: Expiration) {
        self._as.expiration = expiration._as;
    }

    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as.send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as.send_key = send_key;
    }

    #[getter]
    pub fn get_respond_per_each_op(&self) -> bool {
        self._as.respond_per_each_op
    }

    #[setter]
    pub fn set_respond_per_each_op(&mut self, respond_per_each_op: bool) {
        self._as.respond_per_each_op = respond_per_each_op;
    }

    #[getter]
    pub fn get_durable_delete(&self) -> bool {
        self._as.respond_per_each_op
    }

    #[setter]
    pub fn set_durable_delete(&mut self, durable_delete: bool) {
        self._as.durable_delete = durable_delete;
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<FilterExpression> {
        match &self._as.filter_expression {
            Some(fe) => Some(FilterExpression { _as: fe.clone() }),
            None => None,
        }
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  QueryPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct QueryPolicy {
    _as: aerospike_core::QueryPolicy,
}

/// `QueryPolicy` encapsulates parameters for query operations.
#[php_impl]
#[derive(ZvalConvert)]
impl QueryPolicy {
    pub fn __construct() -> Self {
        QueryPolicy {
            _as: aerospike_core::QueryPolicy::default(),
        }
    }

    #[getter]
    pub fn get_base_policy(&self) -> BasePolicy {
        BasePolicy {
            _as: self._as.base_policy.clone(),
        }
    }

    #[setter]
    pub fn set_base_policy(&mut self, base_policy: BasePolicy) {
        self._as.base_policy = base_policy._as;
    }
    #[getter]
    pub fn get_max_concurrent_nodes(&self) -> usize {
        self._as.max_concurrent_nodes
    }

    #[setter]
    pub fn set_max_concurrent_nodes(&mut self, max_concurrent_nodes: usize) {
        self._as.max_concurrent_nodes = max_concurrent_nodes;
    }

    #[getter]
    pub fn get_record_queue_size(&self) -> usize {
        self._as.record_queue_size
    }

    #[setter]
    pub fn set_record_queue_size(&mut self, record_queue_size: usize) {
        self._as.record_queue_size = record_queue_size;
    }

    #[getter]
    pub fn get_fail_on_cluster_change(&self) -> bool {
        self._as.fail_on_cluster_change
    }

    #[setter]
    pub fn set_fail_on_cluster_change(&mut self, fail_on_cluster_change: bool) {
        self._as.fail_on_cluster_change = fail_on_cluster_change;
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<FilterExpression> {
        match &self._as.filter_expression {
            Some(fe) => Some(FilterExpression { _as: fe.clone() }),
            None => None,
        }
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ScanPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct ScanPolicy {
    _as: aerospike_core::ScanPolicy,
}

/// `ScanPolicy` encapsulates optional parameters used in scan operations.
#[php_impl]
#[derive(ZvalConvert)]
impl ScanPolicy {
    pub fn __construct() -> Self {
        ScanPolicy {
            _as: aerospike_core::ScanPolicy::default(),
        }
    }

    #[getter]
    pub fn get_base_policy(&self) -> BasePolicy {
        BasePolicy {
            _as: self._as.base_policy.clone(),
        }
    }

    #[setter]
    pub fn set_base_policy(&mut self, base_policy: BasePolicy) {
        self._as.base_policy = base_policy._as;
    }

    #[getter]
    pub fn get_scan_percent(&self) -> u8 {
        self._as.scan_percent
    }

    #[setter]
    pub fn set_scan_percent(&mut self, scan_percent: u8) {
        self._as.scan_percent = scan_percent;
    }

    #[getter]
    pub fn get_max_concurrent_nodes(&self) -> usize {
        self._as.max_concurrent_nodes
    }

    #[setter]
    pub fn set_max_concurrent_nodes(&mut self, max_concurrent_nodes: usize) {
        self._as.max_concurrent_nodes = max_concurrent_nodes;
    }

    #[getter]
    pub fn get_record_queue_size(&self) -> usize {
        self._as.record_queue_size
    }

    #[setter]
    pub fn set_record_queue_size(&mut self, record_queue_size: usize) {
        self._as.record_queue_size = record_queue_size;
    }

    #[getter]
    pub fn get_fail_on_cluster_change(&self) -> bool {
        self._as.fail_on_cluster_change
    }

    #[setter]
    pub fn set_fail_on_cluster_change(&mut self, fail_on_cluster_change: bool) {
        self._as.fail_on_cluster_change = fail_on_cluster_change;
    }

    #[getter]
    pub fn get_socket_timeout(&self) -> u32 {
        self._as.socket_timeout
    }

    #[setter]
    pub fn set_socket_timeout(&mut self, socket_timeout: u32) {
        self._as.socket_timeout = socket_timeout;
    }

    #[getter]
    pub fn get_filter_expression(&self) -> Option<FilterExpression> {
        match &self._as.filter_expression {
            Some(fe) => Some(FilterExpression { _as: fe.clone() }),
            None => None,
        }
    }

    #[setter]
    pub fn set_filter_expression(&mut self, filter_expression: Option<FilterExpression>) {
        match filter_expression {
            Some(fe) => self._as.filter_expression = Some(fe._as),
            None => self._as.filter_expression = None,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CollectionIndexType
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Secondary index collection type.
enum _CollectionIndexType {
    Default,
    List,
    MapKeys,
    MapValues,
}

#[php_class]
pub struct CollectionIndexType {
    _as: aerospike_core::query::CollectionIndexType,
    v: _CollectionIndexType,
}

#[php_impl]
#[derive(ZvalConvert)]
impl CollectionIndexType {
    pub fn Default() -> Self {
        CollectionIndexType {
            _as: aerospike_core::query::CollectionIndexType::Default,
            v: _CollectionIndexType::Default,
        }
    }
    pub fn List() -> Self {
        CollectionIndexType {
            _as: aerospike_core::query::CollectionIndexType::List,
            v: _CollectionIndexType::List,
        }
    }
    pub fn MapKeys() -> Self {
        CollectionIndexType {
            _as: aerospike_core::query::CollectionIndexType::MapKeys,
            v: _CollectionIndexType::MapKeys,
        }
    }
    pub fn MapValues() -> Self {
        CollectionIndexType {
            _as: aerospike_core::query::CollectionIndexType::MapValues,
            v: _CollectionIndexType::MapValues,
        }
    }
}

impl From<&CollectionIndexType> for aerospike_core::query::CollectionIndexType {
    fn from(input: &CollectionIndexType) -> Self {
        match &input.v {
            _CollectionIndexType::Default => aerospike_core::query::CollectionIndexType::Default,
            _CollectionIndexType::List => aerospike_core::query::CollectionIndexType::List,
            _CollectionIndexType::MapKeys => aerospike_core::query::CollectionIndexType::MapKeys,
            _CollectionIndexType::MapValues => {
                aerospike_core::query::CollectionIndexType::MapValues
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  IndexType
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Underlying data type of secondary index.
enum _IndexType {
    Numeric,
    String,
    Geo2DSphere,
}

#[php_class]
pub struct IndexType {
    _as: aerospike_core::query::IndexType,
    v: _IndexType,
}

#[php_impl]
#[derive(ZvalConvert)]
impl IndexType {
    pub fn Numeric() -> Self {
        IndexType {
            _as: aerospike_core::query::IndexType::Numeric,
            v: _IndexType::Numeric,
        }
    }

    pub fn String() -> Self {
        IndexType {
            _as: aerospike_core::query::IndexType::String,
            v: _IndexType::String,
        }
    }

    pub fn Geo2DSphere() -> Self {
        IndexType {
            _as: aerospike_core::query::IndexType::Geo2DSphere,
            v: _IndexType::Geo2DSphere,
        }
    }
}

impl From<&IndexType> for aerospike_core::query::IndexType {
    fn from(input: &IndexType) -> Self {
        match &input.v {
            _IndexType::Numeric => aerospike_core::query::IndexType::Numeric,
            _IndexType::String => aerospike_core::query::IndexType::String,
            _IndexType::Geo2DSphere => aerospike_core::query::IndexType::Geo2DSphere,
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
///
/// Filter instances should be instantiated using one of the provided macros:
///
/// - `as_eq`
/// - `as_range`
/// - `as_contains`
/// - `as_contains_range`
/// - `as_within_region`
/// - `as_within_radius`
/// - `as_regions_containing_point`
#[php_class]
pub struct Filter {
    _as: aerospike_core::query::Filter,
}

impl FromZval<'_> for Filter {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Filter = zval.extract()?;

        Some(Filter { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl Filter {
    pub fn range(bin_name: &str, begin: PHPValue, end: PHPValue) -> Self {
        Filter {
            _as: aerospike_core::as_range!(
                bin_name,
                aerospike_core::Value::from(begin),
                aerospike_core::Value::from(end)
            ),
        }
    }

    pub fn contains(bin_name: &str, value: PHPValue, cit: Option<&CollectionIndexType>) -> Self {
        let default = CollectionIndexType::Default();
        let cit = cit.unwrap_or(&default);
        Filter {
            _as: aerospike_core::as_contains!(
                bin_name,
                aerospike_core::Value::from(value),
                aerospike_core::query::CollectionIndexType::from(cit)
            ),
        }
    }

    pub fn contains_range(
        bin_name: &str,
        begin: PHPValue,
        end: PHPValue,
        cit: Option<&CollectionIndexType>,
    ) -> Self {
        let default = CollectionIndexType::Default();
        let cit = cit.unwrap_or(&default);
        Filter {
            _as: aerospike_core::as_contains_range!(
                bin_name,
                aerospike_core::Value::from(begin),
                aerospike_core::Value::from(end),
                aerospike_core::query::CollectionIndexType::from(cit)
            ),
        }
    }

    // Example code :
    // $pointString = '{"type":"AeroCircle","coordinates":[[-89.0000,23.0000], 1000]}'
    // Filter::regionsContainingPoint("binName", $pointString)
    pub fn within_region(bin_name: &str, region: &str, cit: Option<&CollectionIndexType>) -> Self {
        let default = CollectionIndexType::Default();
        let cit = cit.unwrap_or(&default);
        Filter {
            _as: aerospike_core::as_within_region!(
                bin_name,
                region,
                aerospike_core::query::CollectionIndexType::from(cit)
            ),
        }
    }

    // Example code :
    // $lat = 43.0004;
    // $lng = -89.0005;
    // $radius = 1000;
    // $filter = Filter::regionsContainingPoint("binName", $lat, $lng, $radius);
    pub fn within_radius(
        bin_name: &str,
        lat: f64,
        lng: f64,
        radius: f64,
        cit: Option<&CollectionIndexType>,
    ) -> Self {
        let default = CollectionIndexType::Default();
        let cit = cit.unwrap_or(&default);
        Filter {
            _as: aerospike_core::as_within_radius!(
                bin_name,
                lat,
                lng,
                radius,
                aerospike_core::query::CollectionIndexType::from(cit)
            ),
        }
    }

    // Example code :
    // $pointString = '{"type":"Point","coordinates":[-89.0000,23.0000]}'
    // Filter::regionsContainingPoint("binName", $pointString)
    pub fn regions_containing_point(
        bin_name: &str,
        point: &str,
        cit: Option<&CollectionIndexType>,
    ) -> Self {
        let default = CollectionIndexType::Default();
        let cit = cit.unwrap_or(&default);
        Filter {
            _as: aerospike_core::as_regions_containing_point!(
                bin_name,
                point,
                aerospike_core::query::CollectionIndexType::from(cit)
            ),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Statement
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Query statement parameters.
#[php_class]
pub struct Statement {
    _as: aerospike_core::Statement,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Statement {
    pub fn __construct(namespace: &str, set_name: &str, bins: Option<Vec<String>>) -> Self {
        Statement {
            _as: aerospike_core::Statement::new(namespace, set_name, bins_flag(bins)),
        }
    }

    #[getter]
    pub fn get_filters(&self) -> Option<Vec<Filter>> {
        self._as
            .filters
            .as_ref()
            .map(|filters| filters.iter().map(|f| Filter { _as: f.clone() }).collect())
    }

    #[setter]
    pub fn set_filters(&mut self, filters: Option<Vec<Filter>>) {
        match filters {
            None => self._as.filters = None,
            Some(filters) => {
                self._as.filters = Some(filters.iter().map(|qf| qf._as.clone()).collect());
            }
        };
        // Ok(())
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
#[php_class]
pub struct Recordset {
    _as: Arc<aerospike_core::Recordset>,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Recordset {
    pub fn close(&self) {
        self._as.close();
    }

    #[getter]
    pub fn get_active(&self) -> bool {
        self._as.is_active()
    }

    pub fn next(&self) -> Option<Result<Record>> {
        match self._as.next_record() {
            None => None,
            Some(Err(e)) => panic!("{}", e),
            Some(Ok(rec)) => Some(Ok(rec.into())),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ClientPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

/// `ClientPolicy` encapsulates parameters for client policy command.
#[php_class]
pub struct ClientPolicy {
    _as: aerospike_core::ClientPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ClientPolicy {
    pub fn __construct() -> Self {
        let mut res = ClientPolicy {
            _as: aerospike_core::ClientPolicy::default(),
        };

        // By default, only allow two connections per node.
        // PHP is single threaded, so usually one connection should be enough per node.
        // TRhe second connection may come handy sometimes for tending.
        res._as.max_conns_per_node = 2;

        // timeout should be shorter than the 30s default due to the nature of internet.
        res._as.timeout = Some(Duration::new(5, 0));

        // idle_timeout should be longer than the 5s default due to the nature of internet.
        res._as.idle_timeout = Some(Duration::new(30, 0));
        res
    }

    #[getter]
    pub fn get_user(&self) -> Option<String> {
        self._as.user_password.clone().map(|(user, _)| user)
    }

    #[setter]
    pub fn set_user(&mut self, user: Option<String>) {
        match (user, &self._as.user_password) {
            (Some(user), Some((_, password))) => {
                self._as.user_password = Some((user, password.into()))
            }
            (Some(user), None) => self._as.user_password = Some((user, "".into())),
            (None, Some((_, password))) => {
                self._as.user_password = Some(("".into(), password.into()))
            }
            (None, None) => {}
        }
    }

    #[getter]
    pub fn get_password(&self) -> Option<String> {
        self._as.user_password.clone().map(|(_, password)| password)
    }

    #[setter]
    pub fn set_password(&mut self, password: Option<String>) {
        match (password, &self._as.user_password) {
            (Some(password), Some((user, _))) => {
                self._as.user_password = Some((user.into(), password))
            }
            (Some(password), None) => self._as.user_password = Some(("".into(), password)),
            (None, Some((user, _))) => self._as.user_password = Some((user.into(), "".into())),
            (None, None) => {}
        }
    }

    #[getter]
    pub fn get_timeout(&self) -> u64 {
        self._as
            .timeout
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default()
    }

    #[setter]
    pub fn set_timeout(&mut self, timeout_millis: u64) {
        let timeout = Duration::from_millis(timeout_millis);
        self._as.timeout = Some(timeout);
    }

    // /// Connection idle timeout. Every time a connection is used, its idle
    // /// deadline will be extended by this duration. When this deadline is reached,
    // /// the connection will be closed and discarded from the connection pool.
    #[getter]
    pub fn get_idle_timeout(&self) -> u64 {
        self._as
            .idle_timeout
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default()
    }

    #[setter]
    pub fn set_idle_timeout(&mut self, timeout_millis: u64) {
        let timeout = Duration::from_millis(timeout_millis);
        self._as.idle_timeout = Some(timeout);
    }

    #[getter]
    pub fn get_max_conns_per_node(&self) -> usize {
        self._as.max_conns_per_node
    }

    #[setter]
    pub fn set_max_conns_per_node(&mut self, sz: usize) {
        self._as.max_conns_per_node = sz;
    }

    // /// Number of connection pools used for each node. Machines with 8 CPU cores or less usually
    // /// need only one connection pool per node. Machines with larger number of CPU cores may have
    // /// their performance limited by contention for pooled connections. Contention for pooled
    // /// connections can be reduced by creating multiple mini connection pools per node.

    #[getter]
    pub fn get_conn_pools_per_node(&self) -> usize {
        self._as.conn_pools_per_node
    }

    #[setter]
    pub fn set_conn_pools_per_node(&mut self, sz: usize) {
        self._as.conn_pools_per_node = sz;
    }

    // /// Throw exception if host connection fails during addHost().
    // pub fail_if_not_connected: bool,

    // /// Threshold at which the buffer attached to the connection will be shrunk by deallocating
    // /// memory instead of just resetting the size of the underlying vec.
    // /// Should be set to a value that covers as large a percentile of payload sizes as possible,
    // /// while also being small enough not to occupy a significant amount of memory for the life
    // /// of the connection pool.
    // pub buffer_reclaim_threshold: usize,

    // /// TendInterval determines interval for checking for cluster state changes.
    // /// Minimum possible interval is 10 Milliseconds.
    // pub tend_interval: Duration,

    // /// A IP translation table is used in cases where different clients
    // /// use different server IP addresses.  This may be necessary when
    // /// using clients from both inside and outside a local area
    // /// network. Default is no translation.
    // /// The key is the IP address returned from friend info requests to other servers.
    // /// The value is the real IP address used to connect to the server.
    // pub ip_map: Option<HashMap<String, String>>,

    // /// UseServicesAlternate determines if the client should use "services-alternate"
    // /// instead of "services" in info request during cluster tending.
    // /// "services-alternate" returns server configured external IP addresses that client
    // /// uses to talk to nodes.  "services-alternate" can be used in place of
    // /// providing a client "ipMap".
    // /// This feature is recommended instead of using the client-side IpMap above.
    // ///
    // /// "services-alternate" is available with Aerospike Server versions >= 3.7.1.
    // pub use_services_alternate: bool,

    // /// Size of the thread pool used in scan and query commands. These commands are often sent to
    // /// multiple server nodes in parallel threads. A thread pool improves performance because
    // /// threads do not have to be created/destroyed for each command.
    // pub thread_pool_size: usize,

    // /// Expected cluster name. It not `None`, server nodes must return this cluster name in order
    // /// to join the client's view of the cluster. Should only be set when connecting to servers
    // /// that support the "cluster-name" info command.
    // pub cluster_name: Option<String>,
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Bin
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Container object for a record bin, comprising a name and a value.
#[php_class]
pub struct Bin {
    _as: aerospike_core::Bin,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Bin {
    pub fn __construct(name: &str, value: PHPValue) -> Self {
        let _as = aerospike_core::Bin::new(name.into(), value.into());
        Bin { _as: _as }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Record
//
////////////////////////////////////////////////////////////////////////////////////////////

/// Container object for a database record.
#[php_class]
pub struct Record {
    _as: aerospike_core::Record,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Record {
    pub fn bin(&self, name: &str) -> Option<PHPValue> {
        let b = self._as.bins.get(name);
        b.map(|v| v.to_owned().into())
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
        self._as.key.as_ref().map(|k| k.clone().into())
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
pub fn new_aerospike_client(
    policy: &ClientPolicy,
    hosts: &str,
) -> PhpResult<aerospike_sync::Client> {
    let res = aerospike_sync::Client::new(&policy._as, &hosts).map_err(|e| e.to_string())?;
    Ok(res)
}

#[php_function]
pub fn Aerospike(policy: &ClientPolicy, hosts: &str) -> PhpResult<Zval> {
    match get_persisted_client(hosts) {
        Some(c) => {
            trace!("Found Aerospike Client object for {}", hosts);
            return Ok(c);
        }
        None => (),
    }

    trace!("Creating a new Aerospike Client object for {}", hosts);

    let c = Arc::new(new_aerospike_client(&policy, &hosts)?);
    persist_client(hosts, c)?;

    match get_persisted_client(hosts) {
        Some(c) => {
            return Ok(c);
        }
        None => Err("Error connecting to the database".into()),
    }
}

#[php_class]
pub struct Client {
    _as: Arc<aerospike_sync::Client>,
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

    pub fn close(&self) -> PhpResult<()> {
        trace!("Closing the client pointer: {:p}", &self);
        self._as.close().map_err(|e| e.to_string())?;
        let mut clients = CLIENTS.lock().unwrap();
        clients.remove(&self.hosts);
        Ok(())
    }

    /// Write record bin(s). The policy specifies the transaction timeout, record expiration and
    /// how the transaction is handled when the record already exists.
    pub fn put(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<aerospike_core::Bin> = bins.into_iter().map(|bin| bin._as.clone()).collect();
        self._as
            .put(&policy._as, &key._as, &bins)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Read record for the specified key. Depending on the bins value provided, all record bins,
    /// only selected record bins or only the record headers will be returned. The policy can be
    /// used to specify timeouts.
    pub fn get(
        &self,
        policy: &ReadPolicy,
        key: &Key,
        bins: Option<Vec<String>>,
    ) -> PhpResult<Record> {
        let res = self
            ._as
            .get(&policy._as, &key._as, bins_flag(bins))
            .map_err(|e| e.to_string())?;
        Ok(res.into())
    }

    /// Add integer bin values to existing record bin values. The policy specifies the transaction
    /// timeout, record expiration and how the transaction is handled when the record already
    /// exists. This call only works for integer values.
    pub fn add(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<aerospike_core::Bin> = bins.into_iter().map(|bin| bin._as.clone()).collect();
        self._as
            .add(&policy._as, &key._as, &bins)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Append bin string values to existing record bin values. The policy specifies the
    /// transaction timeout, record expiration and how the transaction is handled when the record
    /// already exists. This call only works for string values.
    pub fn append(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<aerospike_core::Bin> = bins.into_iter().map(|bin| bin._as.clone()).collect();
        self._as
            .append(&policy._as, &key._as, &bins)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Prepend bin string values to existing record bin values. The policy specifies the
    /// transaction timeout, record expiration and how the transaction is handled when the record
    /// already exists. This call only works for string values.
    pub fn prepend(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<aerospike_core::Bin> = bins.into_iter().map(|bin| bin._as.clone()).collect();
        self._as
            .prepend(&policy._as, &key._as, &bins)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Delete record for specified key. The policy specifies the transaction timeout.
    /// The call returns `true` if the record existed on the server before deletion.
    pub fn delete(&self, policy: &WritePolicy, key: &Key) -> PhpResult<bool> {
        let res = self
            ._as
            .delete(&policy._as, &key._as)
            .map_err(|e| e.to_string())?;
        Ok(res)
    }

    /// Reset record's time to expiration using the policy's expiration. Fail if the record does
    /// not exist.
    pub fn touch(&self, policy: &WritePolicy, key: &Key) -> PhpResult<()> {
        self._as
            .touch(&policy._as, &key._as)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Determine if a record key exists. The policy can be used to specify timeouts.
    pub fn exists(&self, policy: &WritePolicy, key: &Key) -> PhpResult<bool> {
        let res = self
            ._as
            .exists(&policy._as, &key._as)
            .map_err(|e| e.to_string())?;
        Ok(res)
    }

    /// Removes all records in the specified namespace/set efficiently.
    pub fn truncate(
        &self,
        namespace: &str,
        set_name: &str,
        before_nanos: Option<i64>,
    ) -> PhpResult<()> {
        let before_nanos = before_nanos.unwrap_or_default();
        self._as
            .truncate(namespace, set_name, before_nanos)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Read all records in the specified namespace and set and return a record iterator. The scan
    /// executor puts records on a queue in separate threads. The calling thread concurrently pops
    /// records off the queue through the record iterator. Up to `policy.max_concurrent_nodes`
    /// nodes are scanned in parallel. If concurrent nodes is set to zero, the server nodes are
    /// read in series.
    pub fn scan(
        &self,
        policy: &ScanPolicy,
        namespace: &str,
        set_name: &str,
        bins: Option<Vec<String>>,
    ) -> PhpResult<Recordset> {
        let res = self
            ._as
            .scan(&policy._as, namespace, set_name, bins_flag(bins))
            .map_err(|e| e.to_string())?;
        Ok(res.into())
    }

    /// Execute a query on all server nodes and return a record iterator. The query executor puts
    /// records on a queue in separate threads. The calling thread concurrently pops records off
    /// the queue through the record iterator.
    pub fn query(&self, policy: &QueryPolicy, statement: &Statement) -> PhpResult<Recordset> {
        let stmt = statement._as.clone();
        let res = self
            ._as
            .query(&policy._as, stmt)
            .map_err(|e| e.to_string())
            .map_err(|e| e.to_string())?;
        Ok(res.into())
    }

    /// Create a secondary index on a bin containing scalar values. This asynchronous server call
    /// returns before the command is complete.
    pub fn create_index(
        &self,
        namespace: &str,
        set_name: &str,
        bin_name: &str,
        index_name: &str,
        index_type: &IndexType,
        cit: Option<&CollectionIndexType>,
    ) -> PhpResult<()> {
        let default = CollectionIndexType::Default();
        let cit = cit.unwrap_or(&default);
        self._as
            .create_complex_index(
                namespace,
                set_name,
                bin_name,
                index_name,
                index_type.into(),
                cit.into(),
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn drop_index(&self, namespace: &str, set_name: &str, index_name: &str) -> PhpResult<()> {
        self._as
            .drop_index(namespace, set_name, index_name)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn batch_get(
        &self,
        policy: &BatchPolicy,
        batch_reads: Vec<&BatchRead>,
    ) -> PhpResult<Vec<BatchRead>> {
        let batch_reads: Vec<aerospike_core::BatchRead> =
            batch_reads.into_iter().map(|br| br._as.clone()).collect();
        let res = self
            ._as
            .batch_get(&policy._as, batch_reads)
            .map_err(|e| e.to_string())?;

        let res: Vec<BatchRead> = res.into_iter().map(|br| br.into()).collect();
        Ok(res)
        // Ok(vec![])
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Key
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct Key {
    _as: aerospike_core::Key,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Key {
    pub fn __construct(namespace: &str, set: &str, key: PHPValue) -> Self {
        let _as = aerospike_core::Key::new(namespace, set, key.into()).unwrap();
        Key { _as: _as }
    }

    #[getter]
    pub fn get_namespace(&self) -> String {
        self._as.namespace.clone()
    }

    #[getter]
    pub fn get_set_name(&self) -> String {
        self._as.set_name.clone()
    }

    #[getter]
    pub fn get_value(&self) -> Option<PHPValue> {
        self._as.user_key.clone().map(|v| v.into())
    }

    #[getter]
    pub fn get_digest(&self) -> Option<String> {
        Some(hex::encode(self._as.digest))
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

#[php_class]
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
//  HLL
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
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
//TODO: underlying_value
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
                        arr.iter().map(|(_, _, v)| from_zval(v).unwrap()).collect();
                    PHPValue::List(val_arr)
                } else if arr.has_numerical_keys() {
                    // it's a hashmap with numerical keys
                    let mut h = HashMap::<PHPValue, PHPValue>::with_capacity(arr.len());
                    arr.iter().for_each(|(i, _, v)| {
                        h.insert(PHPValue::UInt(i), from_zval(v).unwrap());
                    });
                    PHPValue::HashMap(h)
                } else {
                    // it's a hashmap with string keys
                    let mut h = HashMap::with_capacity(arr.len());
                    arr.iter().for_each(|(_, k, v)| {
                        h.insert(
                            PHPValue::String(k.expect("Invalid key in hashmap".into())),
                            from_zval(v).expect("Invalid value in hashmap".into()),
                        );
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

impl From<HashMap<String, aerospike_core::Value>> for PHPValue {
    fn from(h: HashMap<String, aerospike_core::Value>) -> Self {
        let mut hash = HashMap::<PHPValue, PHPValue>::with_capacity(h.len());
        h.iter().for_each(|(k, v)| {
            hash.insert(PHPValue::String(k.into()), v.clone().into());
        });
        PHPValue::HashMap(hash)
    }
}

impl From<PHPValue> for aerospike_core::Value {
    fn from(other: PHPValue) -> Self {
        match other {
            PHPValue::Nil => aerospike_core::Value::Nil,
            PHPValue::Bool(b) => aerospike_core::Value::Bool(b),
            PHPValue::Int(i) => aerospike_core::Value::Int(i),
            PHPValue::UInt(ui) => aerospike_core::Value::UInt(ui),
            PHPValue::Float(f) => aerospike_core::Value::Float(f64::from(f).into()),
            PHPValue::String(s) => aerospike_core::Value::String(s),
            PHPValue::Blob(b) => aerospike_core::Value::Blob(b),
            PHPValue::List(l) => {
                let mut nl = Vec::<aerospike_core::Value>::with_capacity(l.len());
                l.iter().for_each(|v| nl.push(v.clone().into()));
                aerospike_core::Value::List(nl)
            }
            PHPValue::HashMap(h) => {
                let mut arr = HashMap::with_capacity(h.len());
                h.iter().for_each(|(k, v)| {
                    arr.insert(k.clone().into(), v.clone().into());
                });
                aerospike_core::Value::HashMap(arr)
            }
            PHPValue::GeoJSON(gj) => aerospike_core::Value::GeoJSON(gj),
            PHPValue::HLL(b) => aerospike_core::Value::HLL(b),
        }
    }
}

impl From<aerospike_core::Value> for PHPValue {
    fn from(other: aerospike_core::Value) -> Self {
        match other {
            aerospike_core::Value::Nil => PHPValue::Nil,
            aerospike_core::Value::Bool(b) => PHPValue::Bool(b),
            aerospike_core::Value::Int(i) => PHPValue::Int(i),
            aerospike_core::Value::UInt(ui) => PHPValue::UInt(ui),
            aerospike_core::Value::Float(fv) => {
                PHPValue::Float(ordered_float::OrderedFloat(fv.into()))
            }
            aerospike_core::Value::String(s) => PHPValue::String(s),
            aerospike_core::Value::Blob(b) => PHPValue::Blob(b),
            aerospike_core::Value::List(l) => {
                let mut nl = Vec::<PHPValue>::with_capacity(l.len());
                l.iter().for_each(|v| nl.push(v.clone().into()));
                PHPValue::List(nl)
            }
            aerospike_core::Value::HashMap(h) => {
                let mut arr = HashMap::with_capacity(h.len());
                h.iter().for_each(|(k, v)| {
                    arr.insert(k.clone().into(), v.clone().into());
                });
                PHPValue::HashMap(arr)
            }
            aerospike_core::Value::GeoJSON(gj) => PHPValue::GeoJSON(gj),
            aerospike_core::Value::HLL(b) => PHPValue::HLL(b),
            _ => unreachable!(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Value
//
////////////////////////////////////////////////////////////////////////////////////////////
#[php_class]
pub struct Value;

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

    pub fn string(val: String) -> PHPValue {
        PHPValue::String(val)
    }

    pub fn blob(val: Vec<u8>) -> PHPValue {
        PHPValue::Blob(val)
    }

    pub fn geo_json(val: String) -> GeoJSON {
        GeoJSON { v: val }
    }

    pub fn hll(val: Vec<u8>) -> HLL {
        HLL { v: val }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Converters
//
////////////////////////////////////////////////////////////////////////////////////////////

impl From<aerospike_core::Bin> for Bin {
    fn from(other: aerospike_core::Bin) -> Self {
        Bin { _as: other }
    }
}

impl From<aerospike_core::Key> for Key {
    fn from(other: aerospike_core::Key) -> Self {
        Key { _as: other }
    }
}

impl From<aerospike_core::Record> for Record {
    fn from(other: aerospike_core::Record) -> Self {
        Record { _as: other }
    }
}

impl From<Bin> for aerospike_core::Bin {
    fn from(other: Bin) -> Self {
        other._as
    }
}

impl From<Arc<aerospike_core::Recordset>> for Recordset {
    fn from(other: Arc<aerospike_core::Recordset>) -> Self {
        Recordset { _as: other }
    }
}

// #[derive(Debug)]
// pub struct AeroPHPError(String);

// impl std::fmt::Display for AeroPHPError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.0)
//     }
// }

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


fn bins_flag(bins: Option<Vec<String>>) -> aerospike_core::Bins {
    match bins {
        None => aerospike_core::Bins::All,
        Some(bins) => {
            if bins.len() > 0 {
                aerospike_core::Bins::Some(bins)
            } else {
                aerospike_core::Bins::None
            }
        }
    }
}

fn persist_client(key: &str, c: Arc<aerospike_sync::Client>) -> Result<()> {
    trace!("Persisting Client pointer: {:p}", &c);
    let mut clients = CLIENTS.lock().unwrap();
    clients.insert(key.into(), c);
    Ok(())
}

fn get_persisted_client(key: &str) -> Option<Zval> {
    let clients = CLIENTS.lock().unwrap();
    let as_client = clients.get(key.into())?;
    let client = Client {
        _as: as_client.to_owned(),
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
