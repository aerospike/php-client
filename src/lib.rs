#![cfg_attr(windows, feature(abi_vectorcall))]
use ext_php_rs::prelude::*;

use std::collections::HashMap;

use std::hash::{Hash, Hasher};
use std::{fmt, usize};

use std::str;
use std::sync::Arc;
use std::time::Duration;

use ext_php_rs::boxed::ZBox;

use ext_php_rs::convert::IntoZendObject;
use ext_php_rs::convert::{FromZval, IntoZval};

use ext_php_rs::error::Result;
use ext_php_rs::flags::DataType;
use ext_php_rs::php_class;
use ext_php_rs::rc::PhpRc;
use ext_php_rs::types::ZendHashTable;
use ext_php_rs::types::ZendObject;
use ext_php_rs::types::Zval;

use aerospike_core::as_geo;
use aerospike_core::as_val;

use colored::*;

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Priority
//
////////////////////////////////////////////////////////////////////////////////////////////

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
    pub fn Default() -> Self {
        Priority {
            _as: aerospike_core::Priority::Default,
            v: _Priority::Default,
        }
    }
    pub fn Low() -> Self {
        Priority {
            _as: aerospike_core::Priority::Low,
            v: _Priority::Low,
        }
    }
    pub fn Medium() -> Self {
        Priority {
            _as: aerospike_core::Priority::Medium,
            v: _Priority::Medium,
        }
    }
    pub fn High() -> Self {
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
    pub fn Update() -> Self {
        RecordExistsAction {
            _as: aerospike_core::RecordExistsAction::Update,
            v: _RecordExistsAction::Update,
        }
    }

    pub fn UpdateOnly() -> Self {
        RecordExistsAction {
            _as: aerospike_core::RecordExistsAction::UpdateOnly,
            v: _RecordExistsAction::UpdateOnly,
        }
    }

    pub fn Replace() -> Self {
        RecordExistsAction {
            _as: aerospike_core::RecordExistsAction::Replace,
            v: _RecordExistsAction::Replace,
        }
    }

    pub fn ReplaceOnly() -> Self {
        RecordExistsAction {
            _as: aerospike_core::RecordExistsAction::ReplaceOnly,
            v: _RecordExistsAction::ReplaceOnly,
        }
    }

    pub fn CreateOnly() -> Self {
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
    pub fn CommitAll() -> Self {
        CommitLevel {
            _as: aerospike_core::CommitLevel::CommitAll,
            v: _CommitLevel::CommitAll,
        }
    }

    pub fn CommitMaster() -> Self {
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
    pub fn ConsistencyOne() -> Self {
        ConsistencyLevel {
            _as: aerospike_core::ConsistencyLevel::ConsistencyOne,
            v: _ConsistencyLevel::ConsistencyOne,
        }
    }

    pub fn ConsistencyAll() -> Self {
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
    pub fn None() -> Self {
        GenerationPolicy {
            _as: aerospike_core::GenerationPolicy::None,
            v: _GenerationPolicy::None,
        }
    }

    pub fn ExpectGenEqual() -> Self {
        GenerationPolicy {
            _as: aerospike_core::GenerationPolicy::ExpectGenEqual,
            v: _GenerationPolicy::ExpectGenEqual,
        }
    }

    pub fn ExpectGenGreater() -> Self {
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
    pub fn Seconds(seconds: u32) -> Self {
        Expiration {
            _as: aerospike_core::Expiration::Seconds(seconds),
            v: _Expiration::Seconds(seconds),
        }
    }

    pub fn NamespaceDefault() -> Self {
        Expiration {
            _as: aerospike_core::Expiration::NamespaceDefault,
            v: _Expiration::NamespaceDefault,
        }
    }

    pub fn Never() -> Self {
        Expiration {
            _as: aerospike_core::Expiration::Never,
            v: _Expiration::Never,
        }
    }

    pub fn DontUpdate() -> Self {
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
    pub fn Sequential() -> Self {
        Concurrency {
            _as: aerospike_core::Concurrency::Sequential,
            v: _Concurrency::Sequential,
        }
    }
    pub fn Parallel() -> Self {
        Concurrency {
            _as: aerospike_core::Concurrency::Parallel,
            v: _Concurrency::Parallel,
        }
    }
    pub fn MaxThreads(threads: usize) -> Self {
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
pub struct BasePolicyWrapper {
    _as: aerospike_core::policy::BasePolicy,
}

impl FromZval<'_> for BasePolicyWrapper {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        let f: &BasePolicyWrapper = zval.extract()?;

        Some(BasePolicyWrapper { _as: f._as.clone() })
    }
}

#[php_impl]
#[derive(ZvalConvert)]
impl BasePolicyWrapper {
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
    pub fn get_timeout(&self) -> Option<DurationWrapper> {
        self._as.timeout.map(DurationWrapper)
    }

    #[setter]
    pub fn set_timeout(&mut self, timeout: Option<DurationWrapper>) {
        self._as.timeout = timeout.map(|duration_wrapper| duration_wrapper.0);
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
    pub fn get_sleep_between_retries(&self) -> Option<DurationWrapper> {
        self._as.sleep_between_retries.map(DurationWrapper)
    }

    #[setter]
    pub fn set_sleep_between_retries(&mut self, sleep_between_retries: Option<DurationWrapper>) {
        self._as.sleep_between_retries =
            sleep_between_retries.map(|duration_wrapper| duration_wrapper.0);
    }

    // #[getter]
    // pub fn get_filter_expression(&self) -> Option<FilterExpression> {
    //     self._as.filter_expression
    // }

    // #[setter]
    // pub fn set_durable_delete(&mut self, filter_expression: FilterExpression) {
    //     self._as.filter_expression = Some(filter_expression);
    // }
}

// impl IntoZval for BasePolicy {
//     const TYPE: DataType = DataType::Mixed;

//     fn set_zval(self, zv: &mut Zval, persistent: bool) -> Result<()> {

//     }
// }

////////////////////////////////////////////////////////////////////////////////////////////
//
//  BatchPolicy
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct BatchPolicy {
    _as: aerospike_core::BatchPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl BatchPolicy {
    pub fn __construct() -> Self {
        BatchPolicy {
            _as: aerospike_core::BatchPolicy::default(),
        }
    }

    // #[getter]
    // pub fn get_base_policy(&self) -> BasePolicyWrapper {
    //     BasePolicyWrapper {
    //         _as: self._as.base_policy,
    //     }
    // }

    // #[setter]
    // pub fn set_base_policy(&mut self, base_policy: &BasePolicyWrapper) {
    //     self._as.base_policy = base_policy._as;
    // }

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
    pub fn set_allow_inline(&mut self, allow_inline: bool) {
        self._as.allow_inline = allow_inline;
    }

    #[getter]
    pub fn get_send_set_name(&self) -> bool {
        self._as.send_set_name
    }

    #[setter]
    pub fn set_send_key(&mut self, send_set_name: bool) {
        self._as.send_set_name = send_set_name;
    }

    // #[getter]
    // pub fn get_filter_expression(&self) -> Option<FilterExpression> {
    //     self._as.filter_expression
    // }

    // #[setter]
    // pub fn set_filter_expression(&mut self, filter_expression: FilterExpression) {
    //     self._as.filter_expression = Some(filter_expression);
    // }
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
    pub fn get_timeout(&self) -> Option<DurationWrapper> {
        self._as.timeout.map(DurationWrapper)
    }

    #[setter]
    pub fn set_timeout(&mut self, timeout: Option<DurationWrapper>) {
        self._as.timeout = timeout.map(|duration_wrapper| duration_wrapper.0);
    }

    // #[getter]
    // pub fn get_filter_expression(&self) -> Option<FilterExpression> {
    //     self._as.filter_expression
    // }

    // #[setter]
    // pub fn set_filter_expression(&mut self, filter_expression: FilterExpression) {
    //     self._as.filter_expression = Some(filter_expression);
    // }
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

#[php_impl]
#[derive(ZvalConvert)]
impl WritePolicy {
    pub fn __construct() -> Self {
        WritePolicy {
            _as: aerospike_core::WritePolicy::default(),
        }
    }

    // #[getter]
    // pub fn get_base_policy(&self) -> BasePolicyWrapper {
    //     BasePolicyWrapper {
    //         _as: self._as.base_policy,
    //     }
    // }

    // #[setter]
    // pub fn set_base_policy(&mut self, base_policy: &BasePolicyWrapper) {
    //     self._as.base_policy = base_policy._as;
    // }

    #[getter]
    pub fn get_record_exists_action(&self) -> RecordExistsAction {
        RecordExistsAction {
            _as: self._as.record_exists_action.clone(), // Assuming _as.record_exists_action is the corresponding field in aerospike_core
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
            _as: self._as.generation_policy.clone(), // Assuming _as.generation_policy is the corresponding field in aerospike_core
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
            _as: self._as.commit_level.clone(), // Assuming _as.commit_level is the corresponding field in aerospike_core
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
            _as: self._as.expiration, // Assuming _as.expiration is the corresponding field in aerospike_core
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

    // #[getter]
    // pub fn get_filter_expression(&self) -> Option<FilterExpression> {
    //     self._as.filter_expression
    // }

    // #[setter]
    // pub fn set_filter_expression(&mut self, filter_expression: FilterExpression) {
    //     self._as.filter_expression = Some(filter_expression);
    // }
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

#[php_impl]
#[derive(ZvalConvert)]
impl QueryPolicy {
    pub fn __construct() -> Self {
        QueryPolicy {
            _as: aerospike_core::QueryPolicy::default(),
        }
    }

    // #[getter]
    // pub fn get_base_policy(&self) -> BasePolicyWrapper {
    //     BasePolicyWrapper {
    //         _as: self._as.base_policy,
    //     }
    // }

    // #[setter]
    // pub fn set_base_policy(&mut self, base_policy: &BasePolicyWrapper) {
    //     self._as.base_policy = base_policy._as;
    // }

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

    // #[getter]
    // pub fn get_filter_expression(&self) -> Option<FilterExpression> {
    //     self._as.filter_expression
    // }

    // #[setter]
    // pub fn set_filter_expression(&mut self, filter_expression: FilterExpression) {
    //     self._as.filter_expression = Some(filter_expression);
    // }
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

#[php_impl]
#[derive(ZvalConvert)]
impl ScanPolicy {
    pub fn __construct() -> Self {
        ScanPolicy {
            _as: aerospike_core::ScanPolicy::default(),
        }
    }

    // #[getter]
    // pub fn get_base_policy(&self) -> BasePolicyWrapper {
    //     BasePolicyWrapper {
    //         _as: self._as.base_policy,
    //     }
    // }

    // #[setter]
    // pub fn set_base_policy(&mut self, base_policy: &BasePolicyWrapper) {
    //     self._as.base_policy = base_policy._as;
    // }

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

    // #[getter]
    // pub fn get_filter_expression(&self) -> Option<FilterExpression> {
    //     self._as.filter_expression
    // }

    // #[setter]
    // pub fn set_filter_expression(&mut self, filter_expression: FilterExpression) {
    //     self._as.filter_expression = Some(filter_expression);
    // }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  CollectionIndexType
//
////////////////////////////////////////////////////////////////////////////////////////////

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
    pub fn range(bin_name: &str, begin: Value, end: Value) -> Self {
        Filter {
            _as: aerospike_core::as_range!(
                bin_name,
                aerospike_core::Value::from(begin),
                aerospike_core::Value::from(end)
            ),
        }
    }

    pub fn contains(bin_name: &str, value: Value, cit: Option<&CollectionIndexType>) -> Self {
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
        begin: Value,
        end: Value,
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

#[php_class]
pub struct Statement {
    _as: aerospike_core::Statement,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Statement {
    pub fn __construct(namespace: &str, setname: &str, bins: Option<Vec<String>>) -> Self {
        Statement {
            _as: aerospike_core::Statement::new(namespace, setname, bins_flag(bins)),
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

#[php_class]
pub struct ClientPolicy {
    _as: aerospike_core::ClientPolicy,
}

#[php_impl]
#[derive(ZvalConvert)]
impl ClientPolicy {
    pub fn __construct() -> Self {
        ClientPolicy {
            _as: aerospike_core::ClientPolicy::default(),
        }
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

    // #[getter]
    // pub fn get_timeout(&self) -> Option<Duration> {
    //     self._as.timeout
    // }

    // #[setter]
    // pub fn set_timeout(&mut self, timeout: &Duration) {
    //     self._as.timeout = timeout;
    // }

    // /// Connection idle timeout. Every time a connection is used, its idle
    // /// deadline will be extended by this duration. When this deadline is reached,
    // /// the connection will be closed and discarded from the connection pool.
    // pub idle_timeout: Option<Duration>,

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
    // pub conn_pools_per_node: usize,

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
//  Host
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct Host {
    _as: aerospike_core::Host,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Host {
    pub fn __construct(name: &str, port: u16) -> Self {
        Host {
            _as: aerospike_core::Host::new(name, port),
        }
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self._as.name.clone()
    }

    #[setter]
    pub fn set_name(&mut self, name: String) {
        self._as.name = name;
    }

    #[getter]
    pub fn get_port(&self) -> u16 {
        self._as.port
    }

    #[setter]
    pub fn set_port(&mut self, port: u16) {
        self._as.port = port;
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Bin
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct Bin {
    _as: aerospike_core::Bin,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Bin {
    pub fn __construct(name: &str, value: Value) -> Self {
        let _as = aerospike_core::Bin::new(name.into(), value.into());
        Bin { _as: _as }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  Record
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_class]
pub struct Record {
    _as: aerospike_core::Record,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Record {
    pub fn bin(&self, name: &str) -> Option<Value> {
        let b = self._as.bins.get(name);
        b.map(|v| v.to_owned().into())
    }

    #[getter]
    pub fn get_bins(&self) -> Option<Value> {
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

#[php_function]
pub fn Aerospike(policy: &ClientPolicy, hosts: &str) -> PhpResult<Zval> {
    match get_persisted_client(hosts) {
        Some(c) => return Ok(c.shallow_clone()),
        None => (),
    }

    let hr = format!("Creating a new Aerospike Client object for {}", hosts);
    print_header(&hr, 1);
    let client = Client::new(&policy, &hosts)?;
    persist_client(hosts, client)?;

    let c = get_persisted_client(hosts).expect("Client could not be connected or retrieved");
    Ok(c.shallow_clone())
}

#[php_class]
pub struct Client {
    _as: aerospike_sync::Client,
}

#[php_impl]
#[derive(ZvalConvert)]
impl Client {
    pub fn new(policy: &ClientPolicy, hosts: &str) -> PhpResult<Self> {
        let _as = aerospike_sync::Client::new(&policy._as, &hosts).map_err(|e| e.to_string())?;
        Ok(Client { _as: _as })
    }

    pub fn put(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<aerospike_core::Bin> = bins.into_iter().map(|bin| bin._as.clone()).collect();
        self._as
            .put(&policy._as, &key._as, &bins)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

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

    pub fn add(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<aerospike_core::Bin> = bins.into_iter().map(|bin| bin._as.clone()).collect();
        self._as
            .add(&policy._as, &key._as, &bins)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn append(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<aerospike_core::Bin> = bins.into_iter().map(|bin| bin._as.clone()).collect();
        self._as
            .append(&policy._as, &key._as, &bins)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn prepend(&self, policy: &WritePolicy, key: &Key, bins: Vec<&Bin>) -> PhpResult<()> {
        let bins: Vec<aerospike_core::Bin> = bins.into_iter().map(|bin| bin._as.clone()).collect();
        self._as
            .prepend(&policy._as, &key._as, &bins)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete(&self, policy: &WritePolicy, key: &Key) -> PhpResult<bool> {
        let res = self
            ._as
            .delete(&policy._as, &key._as)
            .map_err(|e| e.to_string())?;
        Ok(res)
    }

    pub fn touch(&self, policy: &WritePolicy, key: &Key) -> PhpResult<()> {
        self._as
            .touch(&policy._as, &key._as)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn exists(&self, policy: &WritePolicy, key: &Key) -> PhpResult<bool> {
        let res = self
            ._as
            .exists(&policy._as, &key._as)
            .map_err(|e| e.to_string())?;
        Ok(res)
    }

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

    pub fn scan(
        &self,
        policy: &ScanPolicy,
        namespace: &str,
        setname: &str,
        bins: Option<Vec<String>>,
    ) -> PhpResult<Recordset> {
        let res = self
            ._as
            .scan(&policy._as, namespace, setname, bins_flag(bins))
            .map_err(|e| e.to_string())?;
        Ok(res.into())
    }

    pub fn query(&self, policy: &QueryPolicy, statement: &Statement) -> PhpResult<Recordset> {
        let stmt = statement._as.clone();
        let res = self
            ._as
            .query(&policy._as, stmt)
            .map_err(|e| e.to_string())
            .map_err(|e| e.to_string())?;
        Ok(res.into())
    }

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
    pub fn __construct(namespace: &str, set: &str, key: Value) -> Self {
        let _as = aerospike_core::Key::new(namespace, set, key.into()).unwrap();
        Key { _as: _as }
    }

    #[getter]
    pub fn get_namespace(&self) -> String {
        self._as.namespace.clone()
    }

    #[getter]
    pub fn get_setname(&self) -> String {
        self._as.set_name.clone()
    }

    #[getter]
    pub fn get_value(&self) -> Option<Value> {
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
//  Value
//
////////////////////////////////////////////////////////////////////////////////////////////

// Container for bin values stored in the Aerospike database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
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
    List(Vec<Value>),
    /// Map data type is a collection of key-value pairs. Each key can only appear once in a
    /// collection and is associated with a value. Map keys and values can be any supported data
    /// type.
    HashMap(HashMap<Value, Value>),
    /// GeoJSON data type are JSON formatted strings to encode geospatial information.
    GeoJSON(String),

    /// HLL value
    HLL(Vec<u8>),
}

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match *self {
            Value::Nil => {
                let v: Option<u8> = None;
                v.hash(state);
            }
            Value::Bool(ref val) => val.hash(state),
            Value::Int(ref val) => val.hash(state),
            Value::UInt(ref val) => val.hash(state),
            Value::Float(ref val) => val.hash(state),
            Value::String(ref val) | Value::GeoJSON(ref val) => val.hash(state),
            Value::Blob(ref val) | Value::HLL(ref val) => val.hash(state),
            Value::List(ref val) => val.hash(state),
            Value::HashMap(_) => panic!("HashMaps cannot be used as map keys."),
            // Value::OrderedMap(_) => panic!("OrderedMaps cannot be used as map keys."),
        }
    }
}

impl Value {
    /// Returns a string representation of the value.
    pub fn as_string(&self) -> String {
        match *self {
            Value::Nil => "<null>".to_string(),
            Value::Int(ref val) => val.to_string(),
            Value::UInt(ref val) => val.to_string(),
            Value::Bool(ref val) => val.to_string(),
            Value::Float(ref val) => val.to_string(),
            Value::String(ref val) | Value::GeoJSON(ref val) => val.to_string(),
            Value::Blob(ref val) | Value::HLL(ref val) => format!("{:?}", val),
            Value::List(ref val) => format!("{:?}", val),
            Value::HashMap(ref val) => format!("{:?}", val),
            // Value::OrderedMap(ref val) => format!("{:?}", val),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(f, "{}", self.as_string())
    }
}

impl IntoZval for Value {
    const TYPE: DataType = DataType::Mixed;

    fn set_zval(self, zv: &mut Zval, persistent: bool) -> Result<()> {
        match self {
            Value::Nil => zv.set_null(),
            Value::Bool(b) => zv.set_bool(b),
            Value::Int(i) => zv.set_long(i),
            Value::UInt(ui) => zv.set_long(ui as i64),
            Value::Float(f) => zv.set_double(f),
            Value::String(s) => zv.set_string(&s, persistent)?,
            Value::Blob(b) => zv.set_binary(b),
            Value::List(l) => zv.set_array(l)?,
            Value::HashMap(h) => {
                let mut arr = ZendHashTable::with_capacity(h.len() as u32);
                h.iter().for_each(|(k, v)| {
                    arr.insert::<Value>(&k.to_string(), v.clone().into());
                });

                zv.set_hashtable(arr)
            }
            Value::GeoJSON(gj) => zv.set_string(&gj, persistent)?,
            Value::HLL(b) => zv.set_binary(b),
        }

        Ok(())
    }
}

fn from_zval(zval: &Zval) -> Option<Value> {
    match zval.get_type() {
        // DataType::Undef => Some(Value::Nil),
        DataType::Null => Some(Value::Nil),
        DataType::False => Some(Value::Bool(false)),
        DataType::True => Some(Value::Bool(true)),
        DataType::Bool => zval.bool().map(|v| Value::Bool(v)),
        DataType::Long => zval.long().map(|v| Value::Int(v)),
        DataType::Double => zval
            .double()
            .map(|v| Value::Float(ordered_float::OrderedFloat(v))),
        DataType::String => zval.string().map(|v| Value::String(v)),
        DataType::Array => {
            zval.array().map(|arr| {
                if arr.has_sequential_keys() {
                    // it's an array
                    let val_arr: Vec<Value> =
                        arr.iter().map(|(_, _, v)| from_zval(v).unwrap()).collect();
                    Value::List(val_arr)
                } else if arr.has_numerical_keys() {
                    // it's a hashmap with numerical keys
                    let mut h = HashMap::<Value, Value>::with_capacity(arr.len());
                    arr.iter().for_each(|(i, _, v)| {
                        h.insert(Value::UInt(i), from_zval(v).unwrap());
                    });
                    Value::HashMap(h)
                } else {
                    // it's a hashmap with string keys
                    let mut h = HashMap::with_capacity(arr.len());
                    arr.iter().for_each(|(_idx, k, v)| {
                        h.insert(
                            Value::String(k.expect("Invalid key in hashmap".into())),
                            from_zval(v).expect("Invalid value in hashmap".into()),
                        );
                    });
                    Value::HashMap(h)
                }
            })
        }
        // DataType::Object(_) => panic!("OBJECT?!"),
        _ => unreachable!(),
    }
}

impl FromZval<'_> for Value {
    const TYPE: DataType = DataType::Mixed;

    fn from_zval(zval: &Zval) -> Option<Self> {
        from_zval(zval)
    }
}

impl From<HashMap<String, aerospike_core::Value>> for Value {
    fn from(h: HashMap<String, aerospike_core::Value>) -> Self {
        let mut hash = HashMap::<Value, Value>::with_capacity(h.len());
        h.iter().for_each(|(k, v)| {
            hash.insert(Value::String(k.into()), v.clone().into());
        });
        Value::HashMap(hash)
    }
}

impl From<Value> for aerospike_core::Value {
    fn from(other: Value) -> Self {
        match other {
            Value::Nil => aerospike_core::Value::Nil,
            Value::Bool(b) => aerospike_core::Value::Bool(b),
            Value::Int(i) => aerospike_core::Value::Int(i),
            Value::UInt(ui) => aerospike_core::Value::UInt(ui),
            Value::Float(f) => aerospike_core::Value::Float(f64::from(f).into()),
            Value::String(s) => aerospike_core::Value::String(s),
            Value::Blob(b) => aerospike_core::Value::Blob(b),
            Value::List(l) => {
                let mut nl = Vec::<aerospike_core::Value>::with_capacity(l.len());
                l.iter().for_each(|v| nl.push(v.clone().into()));
                aerospike_core::Value::List(nl)
            }
            Value::HashMap(h) => {
                let mut arr = HashMap::with_capacity(h.len());
                h.iter().for_each(|(k, v)| {
                    arr.insert(k.clone().into(), v.clone().into());
                });
                aerospike_core::Value::HashMap(arr)
            }
            Value::GeoJSON(gj) => aerospike_core::Value::GeoJSON(gj),
            Value::HLL(b) => aerospike_core::Value::HLL(b),
        }
    }
}

impl From<aerospike_core::Value> for Value {
    fn from(other: aerospike_core::Value) -> Self {
        match other {
            aerospike_core::Value::Nil => Value::Nil,
            aerospike_core::Value::Bool(b) => Value::Bool(b),
            aerospike_core::Value::Int(i) => Value::Int(i),
            aerospike_core::Value::UInt(ui) => Value::UInt(ui),
            aerospike_core::Value::Float(fv) => {
                Value::Float(ordered_float::OrderedFloat(fv.into()))
            }
            aerospike_core::Value::String(s) => Value::String(s),
            aerospike_core::Value::Blob(b) => Value::Blob(b),
            aerospike_core::Value::List(l) => {
                let mut nl = Vec::<Value>::with_capacity(l.len());
                l.iter().for_each(|v| nl.push(v.clone().into()));
                Value::List(nl)
            }
            aerospike_core::Value::HashMap(h) => {
                let mut arr = HashMap::with_capacity(h.len());
                h.iter().for_each(|(k, v)| {
                    arr.insert(k.clone().into(), v.clone().into());
                });
                Value::HashMap(arr)
            }
            aerospike_core::Value::GeoJSON(gj) => Value::GeoJSON(gj),
            aerospike_core::Value::HLL(b) => Value::HLL(b),
            _ => unreachable!(),
        }
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
//  DurationWrapper
//
////////////////////////////////////////////////////////////////////////////////////////////

pub struct DurationWrapper(Duration);

impl DurationWrapper {
    pub fn new(duration: Duration) -> Self {
        DurationWrapper(duration)
    }
}

impl IntoZval for DurationWrapper {
    const TYPE: DataType = DataType::Long;
    fn set_zval(self, zv: &mut Zval, persistent: bool) -> Result<()> {
        let php_duration = self.0.as_millis() as i64;
        zv.set_long(php_duration);
        Ok(())
    }
}

impl FromZval<'_> for DurationWrapper {
    const TYPE: DataType = DataType::Long;
    fn from_zval(zval: &'_ Zval) -> Option<Self> {
        if let Some(r_duation) = zval.long() {
            let duration = Duration::from_millis(r_duation as u64);
            Some(DurationWrapper(duration))
        } else {
            None
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////
//
//  utility methods
//
////////////////////////////////////////////////////////////////////////////////////////////

#[php_function(defaults(emph = 0))]
pub fn print_header(desc: &str, emph: u8) {
    let desc = if emph == 1 {
        desc.bold().red()
    } else {
        desc.normal()
    };

    println!("\n");
    println!("******************************************************************************");
    println!("*{:^76}*", " ");
    println!("*{:^76}*", " ");
    println!("*{:^76}*", desc);
    println!("*{:^76}*", " ");
    println!("*{:^76}*", " ");
    println!("******************************************************************************");
}

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

fn persist_client(key: &str, c: Client) -> Result<()> {
    let mut zval = Zval::new();
    let mut zo: ZBox<ZendObject> = c.into_zend_object()?;
    zo.dec_count();
    zval.set_object(zo.into_raw());

    // persist_value(key, Box::new(zval)).expect("Could not persist_client the value");
    zval.persist(key)
        .expect("Could not persist_client the value");
    Ok(())
}

fn get_persisted_client(key: &str) -> Option<&Zval> {
    // get_persisted_value(key)
    Zval::from_persistence(key)
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
}
