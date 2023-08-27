#![cfg_attr(windows, feature(abi_vectorcall))]
use ext_php_rs::prelude::*;

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;

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
use log::*;

lazy_static! {
    static ref CLIENTS: Mutex<HashMap<String, Arc<aerospike_sync::Client>>> =
        Mutex::new(HashMap::new());
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

#[php_impl]
#[derive(ZvalConvert)]
impl BatchPolicy {
    pub fn __construct() -> Self {
        BatchPolicy {
            _as: aerospike_core::BatchPolicy::default(),
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

#[php_impl]
#[derive(ZvalConvert)]
impl ReadPolicy {
    pub fn __construct() -> Self {
        ReadPolicy {
            _as: aerospike_core::ReadPolicy::default(),
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

#[php_impl]
#[derive(ZvalConvert)]
impl WritePolicy {
    pub fn __construct() -> Self {
        WritePolicy {
            _as: aerospike_core::WritePolicy::default(),
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

#[php_impl]
#[derive(ZvalConvert)]
impl ScanPolicy {
    pub fn __construct() -> Self {
        ScanPolicy {
            _as: aerospike_core::ScanPolicy::default(),
        }
    }
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
    pub fn hello_world(&self) -> String {
        "Hello world!".into()
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
                    arr.insert::<Value>(&k.to_string(), v.clone().into())
                        .expect("error converting hash");
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
                    arr.iter().for_each(|(_, k, v)| {
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
    let target = Box::new(File::create("/var/log/client_php.log").expect("Can't create file"));

    env_logger::Builder::new()
        .target(env_logger::Target::Pipe(target))
        .filter(None, LevelFilter::Info)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {} {}:{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();

    info!("Module Aerospike loaded");
    module
}
