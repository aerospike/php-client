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
#![allow(non_snake_case)]

mod grpc;

use grpc::proto;

use ext_php_rs::prelude::*;

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::Mutex;

use ext_php_rs::boxed::ZBox;
use ext_php_rs::convert::IntoZendObject;
use ext_php_rs::convert::{FromZval, IntoZval};
use ext_php_rs::error::Result;
use ext_php_rs::exception::throw_object;
use ext_php_rs::flags::DataType;
use ext_php_rs::php_class;
use ext_php_rs::types::ArrayKey;
use ext_php_rs::types::ZendHashTable;
use ext_php_rs::types::ZendObject;
use ext_php_rs::types::Zval;

use lazy_static::lazy_static;
use log::LevelFilter;
use log::{debug, info, trace, warn};

lazy_static! {
    static ref CLIENTS: Mutex<HashMap<String, Arc<Mutex<grpc::BlockingClient>>>> =
        Mutex::new(HashMap::new());
}

pub type AspResult<T = ()> = std::result::Result<T, AspException>;

#[allow(non_camel_case_types)]

////////////////////////////////////////////////////////////////////////////////////////////
//
// ResultCode
//
////////////////////////////////////////////////////////////////////////////////////////////

enum ResultCode {
    // GRPC_ERROR is wrapped and directly returned from the grpc library
    GrpcError = -21,

    // BATCH_FAILED means one or more keys failed in a batch.
    BatchFailed = -20,

    // NO_RESPONSE means no response was received from the server.
    NoResponse = -19,

    // NETWORK_ERROR defines a network error. Checked the wrapped error for detail.
    NetworkError = -18,

    // COMMON_ERROR defines a common, none-aerospike error. Checked the wrapped error for detail.
    CommonError = -17,

    // MAX_RETRIES_EXCEEDED defines max retries limit reached.
    MaxRetriesExceeded = -16,

    // MAX_ERROR_RATE defines max errors limit reached.
    MaxErrorRate = -15,

    // RACK_NOT_DEFINED defines requested Rack for node/namespace was not defined in the cluster.
    RackNotDefined = -13,

    // INVALID_CLUSTER_PARTITION_MAP defines cluster has an invalid partition map, usually due to bad configuration.
    InvalidClusterPartitionMap = -12,

    // SERVER_NOT_AVAILABLE defines server is not accepting requests.
    ServerNotAvailable = -11,

    // CLUSTER_NAME_MISMATCH_ERROR defines cluster Name does not match the ClientPolicy.ClusterName value.
    ClusterNameMismatchError = -10,

    // RECORDSET_CLOSED defines recordset has already been closed or cancelled
    RecordsetClosed = -9,

    // NO_AVAILABLE_CONNECTIONS_TO_NODE defines there were no connections available to the node in the pool, and the pool was limited
    NoAvailableConnectionsToNode = -8,

    // TYPE_NOT_SUPPORTED defines data type is not supported by aerospike server.
    TypeNotSupported = -7,

    // COMMAND_REJECTED defines info Command was rejected by the server.
    CommandRejected = -6,

    // QUERY_TERMINATED defines query was terminated by user.
    QueryTerminated = -5,

    // SCAN_TERMINATED defines scan was terminated by user.
    ScanTerminated = -4,

    // INVALID_NODE_ERROR defines chosen node is not currently active.
    InvalidNodeError = -3,

    // PARSE_ERROR defines client parse error.
    ParseError = -2,

    // SERIALIZE_ERROR defines client serialization error.
    SerializeError = -1,

    // OK defines operation was successful.
    OK = 0,

    // SERVER_ERROR defines unknown server failure.
    ServerError = 1,

    // KEY_NOT_FOUND_ERROR defines on retrieving, touching or replacing a record that doesn't exist.
    KeyNotFoundError = 2,

    // GENERATION_ERROR defines on modifying a record with unexpected generation.
    GenerationError = 3,

    // PARAMETER_ERROR defines bad parameter(s) were passed in database operation call.
    ParameterError = 4,

    // KEY_EXISTS_ERROR defines on create-only (write unique) operations on a record that already
    // exists.
    KeyExistsError = 5,

    // BIN_EXISTS_ERROR defines bin already exists on a create-only operation.
    BinExistsError = 6,

    // CLUSTER_KEY_MISMATCH defines expected cluster ID was not received.
    ClusterKeyMismatch = 7,

    // SERVER_MEM_ERROR defines server has run out of memory.
    ServerMemError = 8,

    // TIMEOUT defines client or server has timed out.
    TIMEOUT = 9,

    // ALWAYS_FORBIDDEN defines operation not allowed in current configuration.
    AlwaysForbidden = 10,

    // PARTITION_UNAVAILABLE defines partition is unavailable.
    PartitionUnavailable = 11,

    // BIN_TYPE_ERROR defines operation is not supported with configured bin type (single-bin or
    // multi-bin).
    BinTypeError = 12,

    // RECORD_TOO_BIG defines record size exceeds limit.
    RecordTooBig = 13,

    // KEY_BUSY defines too many concurrent operations on the same record.
    KeyBusy = 14,

    // SCAN_ABORT defines scan aborted by server.
    ScanAbort = 15,

    // UNSUPPORTED_FEATURE defines unsupported Server Feature (e.g. Scan + UDF)
    UnsupportedFeature = 16,

    // BIN_NOT_FOUND defines bin not found on update-only operation.
    BinNotFound = 17,

    // DEVICE_OVERLOAD defines device not keeping up with writes.
    DeviceOverload = 18,

    // KEY_MISMATCH defines key type mismatch.
    KeyMismatch = 19,

    // INVALID_NAMESPACE defines invalid namespace.
    InvalidNamespace = 20,

    // BIN_NAME_TOO_LONG defines bin name length greater than 14 characters,
    // or maximum number of unique bin names are exceeded.
    BinNameTooLong = 21,

    // FAIL_FORBIDDEN defines operation not allowed at this time.
    FailForbidden = 22,

    // FAIL_ELEMENT_NOT_FOUND defines element Not Found in CDT
    FailElementNotFound = 23,

    // FAIL_ELEMENT_EXISTS defines element Already Exists in CDT
    FailElementExists = 24,

    // ENTERPRISE_ONLY defines attempt to use an Enterprise feature on a Community server or a server
    // without the applicable feature key.
    EnterpriseOnly = 25,

    // OP_NOT_APPLICABLE defines the operation cannot be applied to the current bin value on the server.
    OpNotApplicable = 26,

    // FILTERED_OUT defines the transaction was not performed because the filter was false.
    FilteredOut = 27,

    // LOST_CONFLICT defines write command loses conflict to XDR.
    LostConflict = 28,

    // QUERY_END defines there are no more records left for query.
    QueryEnd = 50,

    // SECURITY_NOT_SUPPORTED defines security type not supported by connected server.
    SecurityNotSupported = 51,

    // SECURITY_NOT_ENABLED defines administration command is invalid.
    SecurityNotEnabled = 52,

    // SECURITY_SCHEME_NOT_SUPPORTED defines administration field is invalid.
    SecuritySchemeNotSupported = 53,

    // INVALID_COMMAND defines administration command is invalid.
    InvalidCommand = 54,

    // INVALID_FIELD defines administration field is invalid.
    InvalidField = 55,

    // ILLEGAL_STATE defines security protocol not followed.
    IllegalState = 56,

    // INVALID_USER defines user name is invalid.
    InvalidUser = 60,

    // USER_ALREADY_EXISTS defines user was previously created.
    UserAlreadyExists = 61,

    // INVALID_PASSWORD defines password is invalid.
    InvalidPassword = 62,

    // EXPIRED_PASSWORD defines security credential is invalid.
    ExpiredPassword = 63,

    // FORBIDDEN_PASSWORD defines forbidden password (e.g. recently used)
    ForbiddenPassword = 64,

    // INVALID_CREDENTIAL defines security credential is invalid.
    InvalidCredential = 65,

    // EXPIRED_SESSION defines login session expired.
    ExpiredSession = 66,

    // INVALID_ROLE defines role name is invalid.
    InvalidRole = 70,

    // ROLE_ALREADY_EXISTS defines role already exists.
    RoleAlreadyExists = 71,

    // INVALID_PRIVILEGE defines privilege is invalid.
    InvalidPrivilege = 72,

    // INVALID_WHITELIST defines invalid IP address whiltelist
    InvalidWhitelist = 73,

    // QUOTAS_NOT_ENABLED defines Quotas not enabled on server.
    QuotasNotEnabled = 74,

    // INVALID_QUOTA defines invalid quota value.
    InvalidQuota = 75,

    // NOT_AUTHENTICATED defines user must be authentication before performing database operations.
    NotAuthenticated = 80,

    // ROLE_VIOLATION defines user does not posses the required role to perform the database operation.
    RoleViolation = 81,

    // NOT_WHITELISTED defines command not allowed because sender IP address not whitelisted.
    NotWhitelisted = 82,

    // QUOTA_EXCEEDED defines Quota exceeded.
    QuotaExceeded = 83,

    // UDF_BAD_RESPONSE defines a user defined function returned an error code.
    UdfBadResponse = 100,

    // BATCH_DISABLED defines batch functionality has been disabled.
    BatchDisabled = 150,

    // BATCH_MAX_REQUESTS_EXCEEDED defines batch max requests have been exceeded.
    BatchMaxRequestsExceeded = 151,

    // BATCH_QUEUES_FULL defines all batch queues are full.
    BatchQueuesFull = 152,

    // GEO_INVALID_GEOJSON defines invalid GeoJSON on insert/update
    GeoInvalidGeojson = 160,

    // INDEX_FOUND defines secondary index already exists.
    IndexFound = 200,

    // INDEX_NOTFOUND defines requested secondary index does not exist.
    IndexNotfound = 201,

    // INDEX_OOM defines secondary index memory space exceeded.
    IndexOom = 202,

    // INDEX_NOTREADABLE defines secondary index not available.
    IndexNotreadable = 203,

    // INDEX_GENERIC defines generic secondary index error.
    IndexGeneric = 204,

    // INDEX_NAME_MAXLEN defines index name maximum length exceeded.
    IndexNameMaxlen = 205,

    // INDEX_MAXCOUNT defines maximum number of indexes exceeded.
    IndexMaxcount = 206,

    // QUERY_ABORTED defines secondary index query aborted.
    QueryAborted = 210,

    // QUERY_QUEUEFULL defines secondary index queue full.
    QueryQueuefull = 211,

    // QUERY_TIMEOUT defines secondary index query timed out on server.
    QueryTimeout = 212,

    // QUERY_GENERIC defines generic query error.
    QueryGeneric = 213,

    // QUERY_NETIO_ERR defines query NetIO error on server
    QueryNetioErr = 214,

    // QUERY_DUPLICATE defines duplicate TaskId sent for the statement
    QueryDuplicate = 215,

    // AEROSPIKE_ERR_UDF_NOT_FOUND defines UDF does not exist.
    AerospikeErrUdfNotFound = 1301,

    // AEROSPIKE_ERR_LUA_FILE_NOT_FOUND defines LUA file does not exist.
    AerospikeErrLuaFileNotFound = 1302,
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  ExpressionType (ExpType)
//
////////////////////////////////////////////////////////////////////////////////////////////

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
    pub fn nil() -> Self {
        ExpType {
            _as: proto::ExpType::Nil,
        }
    }

    pub fn bool() -> Self {
        ExpType {
            _as: proto::ExpType::Bool,
        }
    }

    pub fn int() -> Self {
        ExpType {
            _as: proto::ExpType::Int,
        }
    }

    pub fn string() -> Self {
        ExpType {
            _as: proto::ExpType::String,
        }
    }

    pub fn list() -> Self {
        ExpType {
            _as: proto::ExpType::List,
        }
    }

    pub fn map() -> Self {
        ExpType {
            _as: proto::ExpType::Map,
        }
    }

    pub fn blob() -> Self {
        ExpType {
            _as: proto::ExpType::Blob,
        }
    }

    pub fn float() -> Self {
        ExpType {
            _as: proto::ExpType::Float,
        }
    }

    pub fn geo() -> Self {
        ExpType {
            _as: proto::ExpType::Geo,
        }
    }

    pub fn hll() -> Self {
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
            Some(ExpType::int()),
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
            Some(ExpType::string()),
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
            Some(ExpType::blob()),
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
            Some(ExpType::float()),
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
            Some(ExpType::geo()),
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
            Some(ExpType::list()),
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
            Some(ExpType::map()),
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
            Some(ExpType::hll()),
            vec![],
        )
    }

    /// Create function that returns if bin of specified name exists.
    pub fn bin_exists(name: String) -> Self {
        Expression::ne(
            &Expression::bin_type(name),
            &Expression::int_val(ParticleType::null().into()),
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
    /// Not Supported in pre-alpha release
    // pub fn map_val(val: HashMap<PHPValue, PHPValue>) -> Self {
    //  TODO(khosrow): Implement
    //     Expression::new(
    //         None,
    //         Some(PHPValue::HashMap(val).into()),
    //         None,
    //         None,
    //         None,
    //         vec![],
    //     )
    // }

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
    /// // (a > 5 || a == 0) && b < 3
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
    // Requires server version 5.6.0+.
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

    //--------------------------------------------------
    // Variables
    //--------------------------------------------------

    /// Conditionally select an expression from a variable number of expression pairs
    /// followed by default expression action.
    /// Requires server version 5.6.0+.
    /// ```
    /// // Args Format: bool exp1, action exp1, bool exp2, action exp2, ..., action-default
    /// // Apply operator based on type.
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
    /// // 5 < a < 10
    pub fn exp_let(exps: Vec<&Expression>) -> Self {
        Expression::new(Some(proto::ExpOp::Let.into()), None, None, None, None, exps)
    }

    /// Assign variable to an expression that can be accessed later.
    /// Requires server version 5.6.0+.
    /// ```
    /// // 5 < a < 10
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

#[php_class(name = "Aerospike\\ReadModeAP")]
pub struct ReadModeAP {
    _as: proto::ReadModeAp,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ReadModeAP {
    pub fn One() -> Self {
        ReadModeAP {
            _as: proto::ReadModeAp::One,
        }
    }

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

#[php_class(name = "Aerospike\\ReadModeSC")]
pub struct ReadModeSC {
    _as: proto::ReadModeSc,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ReadModeSC {
    pub fn Session() -> Self {
        ReadModeSC {
            _as: proto::ReadModeSc::Session,
        }
    }

    pub fn Linearize() -> Self {
        ReadModeSC {
            _as: proto::ReadModeSc::Linearize,
        }
    }

    pub fn AllowReplica() -> Self {
        ReadModeSC {
            _as: proto::ReadModeSc::AllowReplica,
        }
    }

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
    pub fn update() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::Update,
        }
    }

    /// UpdateOnly means: Update record only. Fail if record does not exist.
    /// Merge write command bins with existing bins.
    pub fn update_only() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::UpdateOnly,
        }
    }

    /// Replace means: Create or replace record.
    /// Delete existing bins not referenced by write command bins.
    /// Supported by Aerospike 2 server versions >= 2.7.5 and
    /// Aerospike 3 server versions >= 3.1.6.
    pub fn replace() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::Replace,
        }
    }

    /// ReplaceOnly means: Replace record only. Fail if record does not exist.
    /// Delete existing bins not referenced by write command bins.
    /// Supported by Aerospike 2 server versions >= 2.7.5 and
    /// Aerospike 3 server versions >= 3.1.6.
    pub fn replace_only() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::ReplaceOnly,
        }
    }

    /// CreateOnly means: Create only. Fail if record exists.
    pub fn create_only() -> Self {
        RecordExistsAction {
            _as: proto::RecordExistsAction::CreateOnly,
        }
    }
}

// impl From<&RecordExistsAction> for proto::RecordExistsAction {
//     fn from(input: &RecordExistsAction) -> Self {
//         match &input._as {
//             proto::RecordExistsAction::Update => proto::RecordExistsAction::Update,
//             proto::RecordExistsAction::UpdateOnly => proto::RecordExistsAction::UpdateOnly,
//             proto::RecordExistsAction::Replace => proto::RecordExistsAction::Replace,
//             proto::RecordExistsAction::ReplaceOnly => proto::RecordExistsAction::ReplaceOnly,
//             proto::RecordExistsAction::CreateOnly => proto::RecordExistsAction::CreateOnly,
//         }
//     }
// }

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CommitLevel
//
////////////////////////////////////////////////////////////////////////////////////////////

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
    pub fn commit_all() -> Self {
        CommitLevel {
            _as: proto::CommitLevel::CommitAll,
        }
    }

    /// CommitMaster indicates the server should wait until successfully committing master only.
    pub fn commit_master() -> Self {
        CommitLevel {
            _as: proto::CommitLevel::CommitMaster,
        }
    }
}

// impl From<&CommitLevel> for proto::CommitLevel {
//     fn from(input: &CommitLevel) -> Self {
//         match &input.v {
//             _CommitLevel::CommitAll => proto::CommitLevel::CommitAll,
//             _CommitLevel::CommitMaster => proto::CommitLevel::CommitMaster,
//         }
//     }
// }

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
    pub fn none() -> Self {
        GenerationPolicy {
            _as: proto::GenerationPolicy::None,
        }
    }

    /// ExpectGenEqual means: Update/delete record if expected generation is equal to server
    /// generation. Otherwise, fail.
    pub fn expect_gen_equal() -> Self {
        GenerationPolicy {
            _as: proto::GenerationPolicy::ExpectGenEqual,
        }
    }

    /// ExpectGenGreater means: Update/delete record if expected generation greater than the server
    /// generation. Otherwise, fail. This is useful for restore after backup.
    pub fn expect_gen_greater() -> Self {
        GenerationPolicy {
            _as: proto::GenerationPolicy::ExpectGenGt,
        }
    }
}

// impl From<&GenerationPolicy> for proto::GenerationPolicy {
//     fn from(input: &GenerationPolicy) -> Self {
//         match &input.v {
//             _GenerationPolicy::None => proto::GenerationPolicy::None,
//             _GenerationPolicy::ExpectGenEqual => proto::GenerationPolicy::ExpectGenEqual,
//             _GenerationPolicy::ExpectGenGreater => proto::GenerationPolicy::ExpectGenGt,
//         }
//     }
// }

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
    pub fn seconds(seconds: u32) -> Self {
        Expiration {
            _as: _Expiration::Seconds(seconds),
        }
    }

    /// Set the record's expiry time using the default time-to-live (TTL) value for the namespace
    pub fn namespace_default() -> Self {
        Expiration {
            _as: _Expiration::NamespaceDefault,
        }
    }

    /// Set the record to never expire. Requires Aerospike 2 server version 2.7.2 or later or
    /// Aerospike 3 server version 3.1.4 or later. Do not use with older servers.
    pub fn never() -> Self {
        Expiration {
            _as: _Expiration::Never,
        }
    }

    /// Do not change the record's expiry time when updating the record; requires Aerospike server
    /// version 3.10.1 or later.
    pub fn dont_update() -> Self {
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
            NAMESPACE_DEFAULT => Expiration::namespace_default(),
            NEVER_EXPIRE => Expiration::never(),
            DONT_UPDATE => Expiration::dont_update(),
            secs => Expiration::seconds(secs),
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
//     pub fn get_filter_expression(&self) -> Option<Expression> {
//         match &self._as.filter_expression {
//             Some(fe) => Some(Expression { _as: fe.clone() }),
//             None => None,
//         }
//     }

//     #[setter]
//     pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
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

    #[getter]
    pub fn get_max_retries(&self) -> u32 {
        self._as.max_retries
    }

    #[setter]
    pub fn set_max_retries(&mut self, max_retries: u32) {
        self._as.max_retries = max_retries;
    }

    #[getter]
    pub fn get_sleep_multiplier(&self) -> f64 {
        self._as.sleep_multiplier
    }

    #[setter]
    pub fn set_sleep_multiplier(&mut self, sleep_multiplier: f64) {
        self._as.sleep_multiplier = sleep_multiplier;
    }

    #[getter]
    pub fn get_total_timeout(&self) -> u64 {
        self._as.total_timeout
    }

    #[setter]
    pub fn set_total_timeout(&mut self, timeout_millis: u64) {
        self._as.total_timeout = timeout_millis;
    }

    #[getter]
    pub fn get_socket_timeout(&self) -> u64 {
        self._as.socket_timeout
    }

    #[setter]
    pub fn set_socket_timeout(&mut self, timeout_millis: u64) {
        self._as.socket_timeout = timeout_millis;
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
    pub fn get_use_compression(&self) -> bool {
        self._as.use_compression
    }

    #[setter]
    pub fn set_use_compression(&mut self, use_compression: bool) {
        self._as.use_compression = use_compression;
    }

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

////////////////////////////////////////////////////////////////////////////////////////////
//
//  InfoPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\InfoPolicy")]
pub struct InfoPolicy {
    _as: proto::InfoPolicy,
}

/// `InfoPolicy` encapsulates parameters for all info operations.
#[php_impl]
#[derive(ZvalConvert)]
impl InfoPolicy {
    pub fn __construct() -> Self {
        InfoPolicy {
            _as: proto::InfoPolicy::default(),
        }
    }
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
            _as: proto::WritePolicy {
                policy: Some(proto::ReadPolicy::default()),
                ..proto::WritePolicy::default()
            },
        }
    }

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
        match self._as.expiration {
            NAMESPACE_DEFAULT => Expiration::namespace_default(),
            NEVER => Expiration::never(),
            DONT_UPDATE => Expiration::dont_update(),
            secs => Expiration::seconds(secs),
        }
    }

    #[setter]
    pub fn set_expiration(&mut self, expiration: Expiration) {
        self._as.expiration = (&expiration).into();
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
//     pub fn get_filter_expression(&self) -> Option<Expression> {
//         match &self._as.filter_expression {
//             Some(fe) => Some(Expression { _as: fe.clone() }),
//             None => None,
//         }
//     }

//     #[setter]
//     pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
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
//     pub fn get_filter_expression(&self) -> Option<Expression> {
//         match &self._as.filter_expression {
//             Some(fe) => Some(Expression { _as: fe.clone() }),
//             None => None,
//         }
//     }

//     #[setter]
//     pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
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
//  ParticleType
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\ParticleType")]
pub struct ParticleType {
    _as: proto::ParticleType,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ParticleType {
    pub fn null() -> Self {
        ParticleType {
            _as: proto::ParticleType::Null,
        }
    }

    pub fn integer() -> Self {
        ParticleType {
            _as: proto::ParticleType::Integer,
        }
    }

    pub fn float() -> Self {
        ParticleType {
            _as: proto::ParticleType::Float,
        }
    }

    pub fn string() -> Self {
        ParticleType {
            _as: proto::ParticleType::String,
        }
    }

    pub fn blob() -> Self {
        ParticleType {
            _as: proto::ParticleType::Blob,
        }
    }

    pub fn digest() -> Self {
        ParticleType {
            _as: proto::ParticleType::Digest,
        }
    }

    pub fn bool() -> Self {
        ParticleType {
            _as: proto::ParticleType::Bool,
        }
    }

    pub fn hll() -> Self {
        ParticleType {
            _as: proto::ParticleType::Hll,
        }
    }

    pub fn map() -> Self {
        ParticleType {
            _as: proto::ParticleType::Map,
        }
    }

    pub fn list() -> Self {
        ParticleType {
            _as: proto::ParticleType::List,
        }
    }

    pub fn geo_json() -> Self {
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
        b.map(|v| (*v).clone().into())
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
    //     pub fn get_filter_expression(&self) -> Option<Expression> {
    //         match &self._as.filter_expression {
    //             Some(fe) => Some(Expression { _as: fe.clone() }),
    //             None => None,
    //         }
    //     }

    //     #[setter]
    //     pub fn set_filter_expression(&mut self, filter_expression: Option<Expression>) {
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
        match self._as.expiration {
            NAMESPACE_DEFAULT => Expiration::namespace_default(),
            NEVER => Expiration::never(),
            DONT_UPDATE => Expiration::dont_update(),
            secs => Expiration::seconds(secs),
        }
    }

    #[setter]
    pub fn set_expiration(&mut self, expiration: Expiration) {
        self._as.expiration = (&expiration).into();
    }

    #[getter]
    pub fn get_durable_delete(&self) -> bool {
        self._as.durable_delete
    }

    #[setter]
    pub fn set_durable_delete(&mut self, durable_delete: bool) {
        self._as.durable_delete = durable_delete;
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

    #[getter]
    pub fn get_generation(&self) -> u32 {
        self._as.generation
    }

    #[setter]
    pub fn set_generation(&mut self, generation: u32) {
        self._as.generation = generation;
    }

    #[getter]
    pub fn get_durable_delete(&self) -> bool {
        self._as.durable_delete
    }

    #[setter]
    pub fn set_durable_delete(&mut self, durable_delete: bool) {
        self._as.durable_delete = durable_delete;
    }

    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as.send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as.send_key = send_key;
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

    #[getter]
    pub fn get_expiration(&self) -> Expiration {
        match self._as.expiration {
            NAMESPACE_DEFAULT => Expiration::namespace_default(),
            NEVER => Expiration::never(),
            DONT_UPDATE => Expiration::dont_update(),
            secs => Expiration::seconds(secs),
        }
    }

    #[setter]
    pub fn set_expiration(&mut self, expiration: Expiration) {
        self._as.expiration = (&expiration).into();
    }

    #[getter]
    pub fn get_durable_delete(&self) -> bool {
        self._as.durable_delete
    }

    #[setter]
    pub fn set_durable_delete(&mut self, durable_delete: bool) {
        self._as.durable_delete = durable_delete;
    }

    #[getter]
    pub fn get_send_key(&self) -> bool {
        self._as.send_key
    }

    #[setter]
    pub fn set_send_key(&mut self, send_key: bool) {
        self._as.send_key = send_key;
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
                ops: ops.into_iter().map(|v| v._as.clone()).collect(),
            },
        }
    }

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

// #[php_function]
// pub fn Aerospike(hosts: &str) -> PhpResult<Zval> {
//     match get_persisted_client(hosts) {
//         Some(c) => {
//             trace!("Found Aerospike Client object for {}", hosts);
//             return Ok(c);
//         }
//         None => (),
//     }

//     trace!("Creating a new Aerospike Client object for {}", hosts);

//     let c = Arc::new(Mutex::new(new_aerospike_client(&hosts)?));
//     persist_client(hosts, c)?;

//     match get_persisted_client(hosts) {
//         Some(c) => {
//             return Ok(c);
//         }
//         None => Err("Error connecting to the database".into()),
//     }
// }

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
    pub fn connect(hosts: &str) -> PhpResult<Zval> {
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

    #[getter]
    pub fn hosts(&self) -> String {
        self.hosts.clone()
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
                        result_code: 0, //ResultCode::KeyNotFoundError,
                        in_doubt: false,
                    }),
                record: None,
            } => Ok(None),
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
                        result_code: 0, //ResultCode::KeyNotFoundError,
                        in_doubt: false,
                    }),
                record: None,
            } => Ok(None),
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
            proto::Error {
                result_code,
                in_doubt,
            } => {
                let error = AspException {
                    message: "Exception in append".to_string(),
                    code: *result_code,
                };
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

        let request = tonic::Request::new(proto::AerospikePutRequest {
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
        let request = tonic::Request::new(proto::AerospikeDeleteRequest {
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
        let request = tonic::Request::new(proto::AerospikeTouchRequest {
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
        let request = tonic::Request::new(proto::AerospikeExistsRequest {
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

    /// Removes all records in the specified namespace/set efficiently.
    pub fn truncate(
        &self,
        policy: &InfoPolicy,
        namespace: &str,
        set_name: &str,
        before_nanos: Option<i64>,
    ) -> PhpResult<()> {
        // let before_nanos = before_nanos.unwrap_or_default();

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
            proto::AerospikeTruncateResponse {
                error:
                    Some(proto::Error {
                        result_code,
                        in_doubt,
                    }),
            } => Err(AerospikeException::new("TODO(Sachin): Implement Exception").into()), // TODO:
            _ => unreachable!(),
        }
    }

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
        let request = tonic::Request::new(proto::AerospikeCreateIndexRequest {
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
        self._as.value.clone().map(|v| v.into())
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
//  Infinity
//
////////////////////////////////////////////////////////////////////////////////////////////

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

#[php_class(name = "Aerospike\\Wildcard")]
pub struct Wildcard {}

impl FromZval<'_> for Wildcard {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &Wildcard = zval.extract()?;

        Some(Wildcard {})
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
            PHPValue::HashMap(_) => panic!("HashMaps cannot be used as map keys."),
            PHPValue::Json(_) => panic!("Jsons cannot be used as map keys."),
            PHPValue::Infinity => panic!("Infinity cannot be used as map keys."),
            PHPValue::Wildcard => panic!("Infinity cannot be used as map keys."),
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
            PHPValue::Infinity => "<infinity>".to_string(),
            PHPValue::Wildcard => "<wildcard>".to_string(),
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
            } else if let Some(_) = zval.extract::<Infinity>() {
                return Some(PHPValue::Infinity);
            } else if let Some(_) = zval.extract::<Wildcard>() {
                return Some(PHPValue::Wildcard);
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

// impl FromZval<'_> for HashMap<PHPValue, PHPValue> {
//     const TYPE: DataType = DataType::Mixed;

//     fn from_zval(zval: &Zval) -> Option<Self> {
//         from_zval(zval)
//     }
// }

// impl FromZval<'_> for HashMap<PHPValue, PHPValue> {
//     fn from(h: HashMap<String, proto::Value>) -> Self {
//         let mut hash = HashMap::<PHPValue, PHPValue>::with_capacity(h.len());
//         h.iter().for_each(|(k, v)| {
//             hash.insert(PHPValue::String(k.into()), v.into());
//         });
//         PHPValue::HashMap(hash)
//     }
// }

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
                    arr.insert(
                        (me.k.clone().unwrap()).into(),
                        (me.v.clone().unwrap()).into(),
                    );
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

// ////////////////////////////////////////////////////////////////////////////////////////////
// //
// //  Value
// //
// ////////////////////////////////////////////////////////////////////////////////////////////

#[php_class(name = "Aerospike\\Value")]
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
