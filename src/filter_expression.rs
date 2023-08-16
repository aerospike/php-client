use ext_php_rs::prelude::*;
use ext_php_rs::error::Result;
use ext_php_rs::flags::DataType;
use ext_php_rs::php_class;
use ext_php_rs::rc::PhpRc;
use ext_php_rs::types::ZendHashTable;
use ext_php_rs::types::ZendObject;
use ext_php_rs::types::Zval;

////////////////////////////////////////////////////////////////////////////////////////////
//
// Exp Type (Enum)
//
////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
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

#[php_impl]
#[derive(ZvalConvert)]
impl ExpressionType {
    pub fn NIL() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::NIL,
            v: _ExpType::NIL,
        }
    }
    pub fn BOOL() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::BOOL,
            v: _ExpType::BOOL,
        }
    }
    pub fn INT() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::INT,
            v: _ExpType::INT,
        }
    }
    pub fn STRING() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::STRING,
            v: _ExpType::STRING,
        }
    }
    pub fn LIST() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::LIST,
            v: _ExpType::LIST,
        }
    }
    pub fn MAP() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::MAP,
            v: _ExpType::MAP,
        }
    }
    pub fn BLOB() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::BLOB,
            v: _ExpType::BLOB,
        }
    }
    pub fn FLOAT() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::FLOAT,
            v: _ExpType::FLOAT,
        }
    }
    pub fn GEO() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::GEO,
            v: _ExpType::GEO,
        }
    }
    pub fn HLL() -> Self {
        ExpressionType {
            _as: aerospike_core::expressions::ExpType::HLL,
            v: _ExpType::HLL,
        }
    }
}