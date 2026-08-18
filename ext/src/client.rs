// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

// The management commands' parameters are camelCase because PHP named arguments
// use the parameter name exactly as written; see `crate::policy` for why the lint
// cannot be scoped more tightly than the module.
#![allow(non_snake_case)]

//! `Aerospike\Client`.

use aerospike_php_ipc::admin::{
    WireIndexCreateBody, WireIndexDropBody, WireIndexOn, WireInfoBody, WireInfoReply, WireNodes,
    WireNodesBody, WireQueryUdfBody, WireTaskHandle, WireTruncateBody, WireUdfExecuteBody,
    WireUdfList, WireUdfListBody, WireUdfRegisterBody, WireUdfRemoveBody, WireUdfResult,
};
use aerospike_php_ipc::batch::{WireBatchBody, WireBatchReply};
use aerospike_php_ipc::query::WireQueryPage;
use aerospike_php_ipc::security::{
    WireAdminTarget, WireRoleAllowlistBody, WireRoleCreateBody, WireRoleNameBody,
    WireRolePrivilegesBody, WireRoleQuotasBody, WireRoleQueryBody, WireRoles, WireUserCreateBody,
    WireUserNameBody, WireUserPasswordBody, WireUserQueryBody, WireUserRolesBody, WireUsers,
};
use aerospike_php_ipc::txn::{WireTxnBeginBody, WireTxnHandle};
use aerospike_php_ipc::{
    decode_body, encode_body, opcode, GetBody, ModifyBody, OperateBody, PongBody, PutBody,
    RecordBody, ReplyHeader, StatusCode, Target, TargetBody, WirePolicy, WireValue,
};
use ext_php_rs::binary::Binary;
use ext_php_rs::boxed::ZBox;
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendHashTable, Zval};

use crate::admin::{Node as AerospikeNode, Task, UdfModule};
use crate::arg::Given;
use crate::batch::{self, BatchResult};
use crate::enums::{CollectionIndex, IndexType, UdfLanguage};
use crate::error::{AeroError, AeroResult};
use crate::ops::{self, Expression};
use crate::policy::{AdminPolicy, QueryPolicy, ReadPolicy, WritePolicy};
use crate::query::{self, PartitionFilter, RecordSet, Statement};
use crate::security::{self, Role as AerospikeRole, User as AerospikeUser};
use crate::record::{self, Bin, Bins, DaemonInfo, Key, Record};
use crate::settings::{self, Settings};
use crate::transport;
use crate::txn::Transaction;
use crate::value;

/// A handle onto one cluster instance owned by the local Aerospike daemon.
///
/// The extension itself speaks to no database. Every call crosses shared
/// memory to `aerospike-php-daemon`, which owns the real client and the
/// connections; the daemon must be running, and must be **the same version as
/// this extension**, for any method here to succeed.
///
/// ```php
/// $client = new Aerospike\Client();            // the "default" instance
/// $client = new Aerospike\Client("analytics"); // a named instance
///
/// $key = new Aerospike\Key("test", "users", "alice");
/// $client->put(null, $key, [
///     new Aerospike\Bin("name", "Alice"),
///     new Aerospike\Bin("age", 30),
/// ]);
///
/// $record = $client->get(null, $key);
/// $record->bin("name");        // "Alice"
/// $record->generation();       // 1
/// ```
///
/// # The argument order is `aerospike-core`'s
///
/// Every command takes **the policy first, then the key, then whatever the
/// verb operates on** — `put($policy, $key, $bins)`, exactly as the Rust
/// client's `put(&policy, &key, &bins)`. Code and examples translate between
/// the two clients without reordering anything, which is the point.
///
/// The policy is nullable, and `null` means "use the daemon's configured
/// settings" — the same thing `&WritePolicy::default()` means in Rust, and the
/// same convention the Java client uses. It is not optional, though: it
/// occupies the first position whether or not you have one to give, because an
/// argument that moves depending on whether it is present is worse than one
/// extra `null`.
///
/// # How PHP values map to Aerospike values
///
/// PHP has a single array type — an ordered hash map — where Aerospike has
/// both lists and maps, so writing an array applies the conventional rule,
/// the same one `json_encode` uses:
///
/// - keys exactly `0, 1, .., n-1` **in that order** become an Aerospike
///   **list**; the empty array counts as a list
/// - anything else — string keys, gaps, a different order — becomes an
///   Aerospike **map**, whose keys may be ints or strings
/// - nested arrays recurse
///
/// `null`, `bool`, `int`, `float` and `string` map to the matching Aerospike
/// types. A PHP string is a byte string with no way to say whether it was meant
/// as text or as binary, so **every bare string is written as a text value**;
/// wrap bytes in `Aerospike\Blob` to write them as binary, and a GeoJSON
/// document in `Aerospike\GeoJson` so the server indexes it as geometry. A
/// string that is not valid UTF-8 is refused rather than written as text.
///
/// Reading back, those shapes come back **as their wrapper class**, so a value
/// round trips as itself:
///
/// | Aerospike value | PHP |
/// | --- | --- |
/// | nil | `null` |
/// | bool, int, double, string | `bool`, `int`, `float`, `string` |
/// | blob | `Aerospike\Blob` |
/// | GeoJSON | `Aerospike\GeoJson` |
/// | HyperLogLog | `Aerospike\Hll` |
/// | list | packed array |
/// | map, sorted map | associative array |
/// | several results for one bin | packed array, in operation order |
/// | a map read that returned keys and values | list of `[key, value]` pairs |
/// | a particle type this build cannot decode | `["particle_type" => int, "data" => Aerospike\Blob]` |
///
/// A **list round trips in order**, because its order is data. A **map comes
/// back in the server's key order**, not in PHP insertion order: Aerospike has
/// no insertion-ordered map type, so an associative array is stored key-ordered
/// and read back that way. This reports that faithfully rather than pretending
/// otherwise — do not rely on the insertion order of a map you wrote, in either
/// direction.
///
/// Anything else — an object of some other class, a resource — is rejected with
/// an `Aerospike\AerospikeException` naming the bin and the position inside it.
///
/// # Filters make a call throw
///
/// `filter` on either policy is an expression in the Aerospike Expression
/// Language, compiled by the server (which needs server 8.1.3 or later). **A
/// record the filter rejects makes the call fail**, on a read as much as on a
/// write: the result code is 27, `FILTERED_OUT`. So "no match" is control flow
/// a caller has to catch:
///
/// ```php
/// try {
///     $record = $client->get(new Aerospike\ReadPolicy(filter: '$.age > 21'), $key);
/// } catch (Aerospike\AerospikeException $e) {
///     if ($e->getResultCode() !== 27) { throw $e; }
///     $record = null;   // the record exists, but the filter rejected it
/// }
/// ```
///
/// # Configuration
///
/// `aerospike.instance` in `php.ini` sets the instance used when the
/// constructor is given none; `aerospike.timeout_ms` sets how long a call
/// waits for the daemon, and `aerospike.spin_iters`,
/// `aerospike.yield_iters`, `aerospike.initial_sleep_us` and
/// `aerospike.max_sleep_us` tune how it waits. All are read when a client is
/// constructed.
#[php_class]
#[php(name = "Aerospike\\Client")]
#[derive(Debug, Default)]
pub struct Client {
    instance: String,
    settings: Settings,
}

#[php_impl]
impl Client {
    /// Open a client for `instance`, defaulting to `aerospike.instance`.
    ///
    /// Cheap and non-blocking: nothing is attached until the first call, so
    /// a failure to reach the daemon surfaces on `ping()`, `put()` or
    /// `get()` rather than here.
    #[php(defaults(instance = None))]
    pub fn __construct(instance: Option<String>) -> Client {
        let resolved = settings::resolve(instance.as_deref());
        Client {
            instance: resolved.instance,
            settings: resolved.settings,
        }
    }

    /// Which cluster instance this client talks to.
    pub fn instance(&self) -> String {
        self.instance.clone()
    }

    /// Check that the daemon is alive, and learn what it serves.
    ///
    /// The version it reports is necessarily this extension's own — a daemon of
    /// any other version could not have answered, because the shared-memory
    /// service they meet on has the version in its name.
    pub fn ping(&self) -> PhpResult<DaemonInfo> {
        // PING carries no body: the contract defines no request payload for
        // it, and which instance is being pinged is already implied by the
        // service the request went to.
        let (header, payload) = self.call(opcode::PING, "ping", &[])?;
        expect_ok(&header, &payload, "ping")?;
        let pong: PongBody = decode_body(&payload).map_err(AeroError::codec)?;
        Ok(DaemonInfo::new(pong.daemon_version, pong.instances))
    }

    /// Write bins to one record.
    ///
    /// `$bins` is a list of `Aerospike\Bin`. A bin whose value is `null`
    /// **deletes** that bin; bin order is preserved.
    ///
    /// ```php
    /// $client->put(null, $key, [new Aerospike\Bin("age", 31)]);
    /// ```
    pub fn put(
        &self,
        policy: Option<Given<&WritePolicy>>,
        key: Given<&Key>,
        bins: Vec<&Zval>,
    ) -> PhpResult<()> {
        let body = PutBody {
            target: self.write_target(policy, key)?,
            bins: record::wire_bins(&record::bin_list(&bins, "put")?),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::PUT, "put", &payload)?;
        expect_ok(&header, &reply, "put")?;
        Ok(())
    }

    /// Read one record, or `null` if it does not exist.
    ///
    /// `$bins` selects what comes back; omitting it reads every bin. See
    /// `Aerospike\Bins` for reading a few bins, or none at all.
    ///
    /// ```php
    /// $record = $client->get(null, $key, Aerospike\Bins::some(["name", "age"]));
    /// ```
    ///
    /// The explicit `= null` default is what makes `$bins` *optional* rather
    /// than merely nullable, for anything reading the signature — reflection, a
    /// generated stub, a static analyser.
    #[php(defaults(bins = None))]
    pub fn get(
        &self,
        policy: Option<Given<&ReadPolicy>>,
        key: Given<&Key>,
        bins: Option<Given<&Bins>>,
    ) -> PhpResult<Option<Record>> {
        let body = GetBody {
            target: self.read_target(policy, key)?,
            bins: Given::or_none(bins, "bins")?.map_or_else(Bins::all_wire, Bins::to_wire),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::GET, "get", &payload)?;

        // A record that is simply absent is an ordinary outcome, not an
        // error, so it comes back as `null` rather than as an exception.
        if header.status() == StatusCode::RECORD_NOT_FOUND {
            return Ok(None);
        }
        expect_ok(&header, &reply, "get")?;

        let record: RecordBody = decode_body(&reply).map_err(AeroError::codec)?;
        Ok(Some(Record::from_reply(&header, record)))
    }

    /// Delete one record, returning whether it existed.
    ///
    /// `false` means the record was already gone, which is not an error — the
    /// end state is the one that was asked for. Set `durableDelete: true` on
    /// the policy to leave a tombstone (Enterprise only), which is what stops a
    /// deleted record from reappearing after a cold restart.
    pub fn delete(&self, policy: Option<Given<&WritePolicy>>, key: Given<&Key>) -> PhpResult<bool> {
        let body = TargetBody {
            target: self.write_target(policy, key)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::DELETE, "delete", &payload)?;
        Ok(found_or_not(&header, &reply, "delete")?)
    }

    /// Reset one record's time-to-live, and bump its generation.
    ///
    /// The new TTL comes from the policy's `expiration`, defaulting to the
    /// namespace's. Unlike `delete()` and `exists()` this **throws** when the
    /// record is absent: there is nothing to touch, and a silent no-op would
    /// leave a caller believing a record's life had been extended.
    pub fn touch(&self, policy: Option<Given<&WritePolicy>>, key: Given<&Key>) -> PhpResult<()> {
        let body = TargetBody {
            target: self.write_target(policy, key)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::TOUCH, "touch", &payload)?;
        expect_ok(&header, &reply, "touch")?;
        Ok(())
    }

    /// Whether one record exists.
    ///
    /// Reads no bins, so this is a metadata-only round trip rather than a
    /// `get()` whose result is thrown away.
    pub fn exists(&self, policy: Option<Given<&ReadPolicy>>, key: Given<&Key>) -> PhpResult<bool> {
        let body = TargetBody {
            target: self.read_target(policy, key)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::EXISTS, "exists", &payload)?;
        Ok(found_or_not(&header, &reply, "exists")?)
    }

    /// Add numeric deltas to bins, creating the record or the bins if needed.
    ///
    /// Every value must be an int or a float — an int adds to an integer bin, a
    /// float to a double bin — and mixing the two on one bin is a server error.
    /// Deltas may be negative, which is how a counter is decremented.
    ///
    /// ```php
    /// $client->add(null, $key, [
    ///     new Aerospike\Bin("views", 1),
    ///     new Aerospike\Bin("seconds", 0.5),
    /// ]);
    /// ```
    pub fn add(
        &self,
        policy: Option<Given<&WritePolicy>>,
        key: Given<&Key>,
        bins: Vec<&Zval>,
    ) -> PhpResult<()> {
        self.modify(Modify::Add, policy, key, bins)
    }

    /// Append to string or blob bins, creating them if needed.
    ///
    /// Every value must be a string or an `Aerospike\Blob`, and must match the
    /// bin's existing type.
    pub fn append(
        &self,
        policy: Option<Given<&WritePolicy>>,
        key: Given<&Key>,
        bins: Vec<&Zval>,
    ) -> PhpResult<()> {
        self.modify(Modify::Append, policy, key, bins)
    }

    /// Prepend to string or blob bins, creating them if needed.
    ///
    /// Every value must be a string or an `Aerospike\Blob`, and must match the
    /// bin's existing type.
    pub fn prepend(
        &self,
        policy: Option<Given<&WritePolicy>>,
        key: Given<&Key>,
        bins: Vec<&Zval>,
    ) -> PhpResult<()> {
        self.modify(Modify::Prepend, policy, key, bins)
    }

    /// Run many rows — reads, writes, deletes and UDF calls — in one round trip
    /// per node.
    ///
    /// `$rows` is a list of `Aerospike\BatchRow`, built by the static methods of
    /// `Aerospike\BatchRead`, `BatchWrite`, `BatchDelete` and `BatchUdf`. The
    /// rows may name any key in any namespace, and each may be a different kind
    /// of work — that is what a batch is for.
    ///
    /// ```php
    /// $results = $client->batch(null, [
    ///     Aerospike\BatchRead::all($alice),
    ///     Aerospike\BatchWrite::ops($bob, [Aerospike\Op::add(new Aerospike\Bin("hits", 1))]),
    ///     Aerospike\BatchDelete::key($carol),
    /// ]);
    /// ```
    ///
    /// Returns one `Aerospike\BatchResult` per row, **in the order the rows were
    /// given**. A row that failed carries its own result code and no record —
    /// and the batch as a whole still succeeded, so **check `isOk()` on each row
    /// rather than relying on an exception.** Only a failure that stops the
    /// batch being sent at all throws.
    ///
    /// The policy is a `ReadPolicy`: it carries what every row shares —
    /// timeouts, retries, replica choice, a filter applied to all of them.
    /// Anything that can differ per row belongs on the row.
    pub fn batch(
        &self,
        policy: Option<Given<&ReadPolicy>>,
        rows: Vec<&Zval>,
    ) -> PhpResult<Vec<BatchResult>> {
        let policy = Given::or_none(policy, "policy")?;
        let body = WireBatchBody {
            instance: self.instance.clone(),
            policy: policy.map(ReadPolicy::to_wire).unwrap_or_default(),
            rows: batch::row_list(&rows)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::BATCH, "batch", &payload)?;
        expect_ok(&header, &reply, "batch")?;

        let answers: WireBatchReply = decode_body(&reply).map_err(AeroError::codec)?;
        if answers.rows.len() != rows.len() {
            return Err(AeroError::client(format!(
                "the daemon answered a {}-row batch with {} results; the contract says one per \
                 row, in order",
                rows.len(),
                answers.rows.len()
            ))
            .into());
        }
        Ok(answers.rows.into_iter().map(BatchResult::from_wire).collect())
    }

    /// Run several operations against one record, in order, atomically.
    ///
    /// `$ops` is a list of `Aerospike\Operation`, built by the static methods of
    /// `Aerospike\Op` and `Aerospike\ListOp`. The operations run in the order
    /// given and nobody else's write can interleave with them, which is the
    /// whole reason this exists rather than a sequence of separate calls:
    ///
    /// ```php
    /// $record = $client->operate(null, $key, [
    ///     Aerospike\Op::add(new Aerospike\Bin("views", 1)),
    ///     Aerospike\ListOp::append("history", $event),
    ///     Aerospike\ListOp::size("history"),
    /// ]);
    /// $record->bin("history");   // the new size, from the last operation
    /// ```
    ///
    /// Returns the results of the operations that produced one, as a `Record`,
    /// or `null` when the record does not exist and nothing created it. A call
    /// whose operations all write answers with a `Record` holding no bins.
    ///
    /// **Two reads of the same bin come back as a list**, in operation order,
    /// because one bin name cannot hold two answers. That is the server's own
    /// shape for it.
    ///
    /// The policy is a `WritePolicy` whatever the operations are — that is the
    /// signature `aerospike-core` has, since `operate` may write. A call that
    /// only reads is still checked against the read rules, so a write-only
    /// setting on it is refused rather than ignored.
    pub fn operate(
        &self,
        policy: Option<Given<&WritePolicy>>,
        key: Given<&Key>,
        ops: Vec<&Zval>,
    ) -> PhpResult<Option<Record>> {
        let body = OperateBody {
            target: self.write_target(policy, key)?,
            ops: ops::operation_list(&ops)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::OPERATE, "operate", &payload)?;

        // As on a read: a record that is simply not there is an outcome, not a
        // failure. It can only happen when no operation would have created it.
        if header.status() == StatusCode::RECORD_NOT_FOUND {
            return Ok(None);
        }
        expect_ok(&header, &reply, "operate")?;

        let record: RecordBody = decode_body(&reply).map_err(AeroError::codec)?;
        Ok(Some(Record::from_reply(&header, record)))
    }

    /// Scan a set, or query a secondary index, reading the results a page at a
    /// time.
    ///
    /// One method for both, exactly as `aerospike-core` has one: a
    /// `Aerospike\Statement` with no filter visits every record — a scan — and one
    /// with a filter uses the index the filter names.
    ///
    /// ```php
    /// $statement = new Aerospike\Statement("test", "users");
    /// foreach ($client->query(null, null, $statement) as $record) {
    ///     echo $record->bin("name"), "\n";
    /// }
    /// ```
    ///
    /// `$partitions` divides the ring, for splitting one traversal between
    /// workers; `null` means all of it. The policy carries the page size, the
    /// record ceiling and the rate limit — see `Aerospike\QueryPolicy`.
    ///
    /// Returns an `Aerospike\RecordSet`, which is an `Iterator` and is **readable
    /// once**: it is a position in a traversal, not a collection. Abandoning it
    /// mid-way is fine — `break` out of the `foreach` and the daemon's cursor is
    /// released when the object goes out of scope.
    ///
    /// The first page is fetched here, so a query whose whole result fits in one
    /// page costs a single round trip and leaves nothing open. A failure of that
    /// first page throws; a failure of a later one throws from the `foreach`.
    ///
    /// The first two arguments are **nullable, not optional** — as on every other
    /// verb here. PHP cannot give a leading parameter a default when a later one
    /// has none, so `query(statement: $s)` is not available however the defaults
    /// are declared; pass the two `null`s.
    /// A record's metadata, with no bins.
    ///
    /// The same call as `get($policy, $key, Bins::none())`, under the name the 1.x
    /// client used. Worth having as its own verb: reading a generation to guard a
    /// write is common, and it should not look like a read that forgot its bins.
    ///
    /// `null` when the record is absent, as `get()` is.
    pub fn get_header(
        &self,
        policy: Option<Given<&ReadPolicy>>,
        key: Given<&Key>,
    ) -> PhpResult<Option<Record>> {
        self.get(policy, key, Some(Given::Value(&Bins::none())))
    }

    /// Every record in a set, or in a whole namespace.
    ///
    /// A scan **is** a query with no filter — `query()` with a filter-less
    /// `Statement` does exactly this — and the 1.x client spelled it as its own
    /// method taking the namespace and set directly. Both spellings work; this one
    /// saves building a `Statement` for the common case.
    ///
    /// `$binNames` selects bins by name; `null` reads them all. Records arrive a
    /// page at a time, as they do from `query()`.
    ///
    /// ```php
    /// foreach ($client->scan(null, null, 'test', 'users') as $record) { … }
    /// ```
    #[php(defaults(binNames = None))]
    pub fn scan(
        &self,
        policy: Option<Given<&QueryPolicy>>,
        partitions: Option<Given<&PartitionFilter>>,
        namespace: String,
        set: String,
        binNames: Option<Given<Vec<String>>>,
    ) -> PhpResult<RecordSet> {
        let statement = Statement::of_scan(
            namespace,
            set,
            Given::or_none(binNames, "binNames")?,
        )?;
        self.query(policy, partitions, Given::Value(&statement))
    }

    pub fn query(
        &self,
        policy: Option<Given<&QueryPolicy>>,
        partitions: Option<Given<&PartitionFilter>>,
        statement: Given<&Statement>,
    ) -> PhpResult<RecordSet> {
        let policy = Given::or_none(policy, "policy")?;
        let partitions = Given::or_none(partitions, "partitions")?;
        let statement = statement.required("statement")?;

        let body = query::request(self.instance.clone(), policy, partitions, statement);
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::QUERY, "query", &payload)?;
        expect_ok(&header, &reply, "query")?;

        let page: WireQueryPage = decode_body(&reply).map_err(AeroError::codec)?;
        Ok(RecordSet::open(
            self.instance.clone(),
            self.settings.clone(),
            page,
        )?)
    }

    // ===== Cluster management ===============================================

    /// Register a UDF module on every node.
    ///
    /// `$source` is the module's **text**, not a path: the daemon may not share a
    /// filesystem with this worker, so a path resolved on its side could register
    /// a different file. Read the file here.
    ///
    /// ```php
    /// $task = $client->registerUdf(null, file_get_contents('example.lua'), 'example.lua');
    /// $task->waitTillComplete(10_000);
    /// ```
    ///
    /// Returns an `Aerospike\Task`: the call succeeds once one node has taken the
    /// module, and it then propagates. **Registering does not overwrite
    /// atomically** — during propagation different nodes may briefly run different
    /// versions of the module, which matters for a UDF a query is using at the
    /// time.
    #[php(defaults(language = None))]
    pub fn register_udf(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        source: Given<Binary<u8>>,
        serverPath: Given<String>,
        language: Option<Given<UdfLanguage>>,
    ) -> PhpResult<Task> {
        let body = WireUdfRegisterBody {
            instance: self.instance.clone(),
            timeout_ms: self.admin_timeout(policy)?,
            server_path: serverPath.required("serverPath")?,
            source: source.required("source")?.to_vec(),
            language: Given::or_none(language, "language")?
                .unwrap_or_default()
                .to_wire(),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.task(opcode::UDF_REGISTER, "registerUdf", &payload)
    }

    /// Remove a UDF module from every node.
    ///
    /// Returns an `Aerospike\Task`, as registering does. A module still referenced
    /// by a running background query is removed anyway; the query keeps the copy
    /// it started with.
    pub fn remove_udf(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        serverPath: Given<String>,
    ) -> PhpResult<Task> {
        let body = WireUdfRemoveBody {
            instance: self.instance.clone(),
            timeout_ms: self.admin_timeout(policy)?,
            server_path: serverPath.required("serverPath")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.task(opcode::UDF_REMOVE, "removeUdf", &payload)
    }

    /// List the UDF modules the cluster holds.
    ///
    /// The one method here with no counterpart in `aerospike-core` — it has no
    /// `list_udf`, so the daemon issues the `udf-list` info command and parses the
    /// server's record format, once, rather than leaving every caller to.
    pub fn list_udf(&self, policy: Option<Given<&AdminPolicy>>) -> PhpResult<Vec<UdfModule>> {
        let body = WireUdfListBody {
            instance: self.instance.clone(),
            timeout_ms: self.admin_timeout(policy)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::UDF_LIST, "listUdf", &payload)?;
        expect_ok(&header, &reply, "listUdf")?;

        let list: WireUdfList = decode_body(&reply).map_err(AeroError::codec)?;
        Ok(list.modules.into_iter().map(UdfModule::from_wire).collect())
    }

    /// Run a UDF against one record, and return what it returned.
    ///
    /// ```php
    /// $result = $client->executeUdf(null, $key, 'example', 'bump', ['views', 1]);
    /// ```
    ///
    /// `$package` is the module name **without** its extension — `example` for a
    /// module registered as `example.lua`. That asymmetry is the server's.
    ///
    /// **This is a write, whatever the function does.** The server takes a write
    /// lock on the record because it cannot know in advance whether the Lua will
    /// change it, which is why the policy is a `WritePolicy` and why a UDF is not a
    /// way to do a cheap read.
    ///
    /// Returns `null` when the function returned nothing. A function that returned
    /// Lua `nil` also arrives as `null` — the two are indistinguishable through
    /// PHP, as they are through every other client.
    #[php(defaults(args = None))]
    pub fn execute_udf(
        &self,
        policy: Option<Given<&WritePolicy>>,
        key: Given<&Key>,
        package: Given<String>,
        function: Given<String>,
        args: Option<Given<Vec<&Zval>>>,
    ) -> PhpResult<Option<Zval>> {
        let body = WireUdfExecuteBody {
            target: self.write_target(policy, key)?,
            package: package.required("package")?,
            function: function.required("function")?,
            args: udf_args(Given::or_none(args, "args")?)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::UDF_EXECUTE, "executeUdf", &payload)?;
        expect_ok(&header, &reply, "executeUdf")?;

        let result: WireUdfResult = decode_body(&reply).map_err(AeroError::codec)?;
        match result.value {
            Some(value) => Ok(Some(value::wire_to_zval(&value)?)),
            None => Ok(None),
        }
    }

    /// Apply a UDF to every record a statement matches, in the background.
    ///
    /// The server runs it without the client waiting, so this returns an
    /// `Aerospike\Task` as soon as the job is accepted. A statement with no filter
    /// applies the function to the whole set — which is how a bulk update is done
    /// without moving any records to the client.
    ///
    /// ```php
    /// $task = $client->queryExecuteUdf(
    ///     null, new Aerospike\Statement('test', 'users'), 'example', 'bump', ['views']
    /// );
    /// ```
    ///
    /// **Nothing reports which records failed.** A background job's per-record
    /// errors are the server's; the task says only whether the job finished. For
    /// work that has to account for each record, use `batch()`.
    #[php(defaults(args = None))]
    pub fn query_execute_udf(
        &self,
        policy: Option<Given<&WritePolicy>>,
        statement: Given<&Statement>,
        package: Given<String>,
        function: Given<String>,
        args: Option<Given<Vec<&Zval>>>,
    ) -> PhpResult<Task> {
        let policy = Given::or_none(policy, "policy")?;
        let body = WireQueryUdfBody {
            instance: self.instance.clone(),
            policy: policy.map(WritePolicy::to_wire).unwrap_or_default(),
            statement: statement.required("statement")?.to_wire(),
            package: package.required("package")?,
            function: function.required("function")?,
            args: udf_args(Given::or_none(args, "args")?)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.task(opcode::QUERY_UDF, "queryExecuteUdf", &payload)
    }

    /// Create a secondary index over a bin.
    ///
    /// ```php
    /// $client->createIndexOnBin(null, 'test', 'users', 'age', 'age_idx', Aerospike\IndexType::Numeric)
    ///        ->waitTillComplete(30_000);
    /// ```
    ///
    /// `$collection` says whether the index covers the bin itself or, for a bin
    /// holding a collection, its list elements or map keys or values. `$ctx`
    /// reaches an index into a *nested* collection, and is a list of
    /// `Aerospike\Ctx` steps.
    ///
    /// Returns an `Aerospike\Task`, because the index then has to be built over
    /// every existing record — which on a large set takes minutes, and until it
    /// finishes **a query using the index returns incomplete results rather than
    /// an error**. So wait, or check the task, before relying on it.
    ///
    /// The index type must match the bin's contents. A numeric index over a string
    /// bin is not an error; it simply indexes nothing, and the query finds no
    /// records.
    #[php(defaults(collection = None, ctx = None))]
    #[allow(clippy::too_many_arguments)]
    pub fn create_index_on_bin(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        namespace: Given<String>,
        set: Given<String>,
        binName: Given<String>,
        indexName: Given<String>,
        indexType: Given<IndexType>,
        collection: Option<Given<CollectionIndex>>,
        ctx: Option<Given<Vec<&Zval>>>,
    ) -> PhpResult<Task> {
        let path = match Given::or_none(ctx, "ctx")? {
            Some(steps) => ops::context_list(&steps)?,
            None => Vec::new(),
        };
        let body = WireIndexCreateBody {
            instance: self.instance.clone(),
            timeout_ms: self.admin_timeout(policy)?,
            namespace: namespace.required("namespace")?,
            set: set.required("set")?,
            index_name: indexName.required("indexName")?,
            on: WireIndexOn::Bin {
                name: binName.required("binName")?,
                ctx: path,
            },
            index_type: indexType.required("indexType")?.to_wire(),
            collection: Given::or_none(collection, "collection")?
                .unwrap_or_default()
                .to_wire(),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.task(opcode::INDEX_CREATE, "createIndexOnBin", &payload)
    }

    /// Create a secondary index over an expression.
    ///
    /// The index covers whatever the expression produces, so there is no bin to
    /// look it up by — **a query must name this index by name**, with
    /// `Aerospike\Filter::equalByIndex()` or `rangeByIndex()`.
    ///
    /// ```php
    /// $client->createIndexUsingExpression(
    ///     null, 'test', 'users', 'total_idx', Aerospike\IndexType::Numeric, null,
    ///     Aerospike\Expression::ael('$.price:INT * $.quantity:INT')
    /// );
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn create_index_using_expression(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        namespace: Given<String>,
        set: Given<String>,
        indexName: Given<String>,
        indexType: Given<IndexType>,
        collection: Option<Given<CollectionIndex>>,
        expression: Given<&Expression>,
    ) -> PhpResult<Task> {
        let body = WireIndexCreateBody {
            instance: self.instance.clone(),
            timeout_ms: self.admin_timeout(policy)?,
            namespace: namespace.required("namespace")?,
            set: set.required("set")?,
            index_name: indexName.required("indexName")?,
            on: WireIndexOn::Expression(expression.required("expression")?.to_wire()),
            index_type: indexType.required("indexType")?.to_wire(),
            collection: Given::or_none(collection, "collection")?
                .unwrap_or_default()
                .to_wire(),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.task(opcode::INDEX_CREATE, "createIndexUsingExpression", &payload)
    }

    /// Drop a secondary index.
    ///
    /// Returns an `Aerospike\Task` whose completion is the index being *gone*.
    /// Dropping one a query is using does not fail the query; it falls back to
    /// whatever the server can still do.
    pub fn drop_index(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        namespace: Given<String>,
        set: Given<String>,
        indexName: Given<String>,
    ) -> PhpResult<Task> {
        let body = WireIndexDropBody {
            instance: self.instance.clone(),
            timeout_ms: self.admin_timeout(policy)?,
            namespace: namespace.required("namespace")?,
            set: set.required("set")?,
            index_name: indexName.required("indexName")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.task(opcode::INDEX_DROP, "dropIndex", &payload)
    }

    /// Delete every record of a set, or of a whole namespace.
    ///
    /// ```php
    /// $client->truncate(null, 'test', 'users', null);       // the whole set
    /// $client->truncate(null, 'test', '', null);            // the whole namespace
    /// $client->truncate(null, 'test', 'users', $nanos);     // only older records
    /// ```
    ///
    /// An empty `$set` truncates the namespace. `$beforeNanos` deletes only records
    /// last updated before that time, in **nanoseconds since the Unix epoch**
    /// (`hrtime()` is monotonic and not this; `time() * 1_000_000_000` is).
    ///
    /// Returns nothing, and returns quickly: the server marks the set truncated
    /// and reclaims the space in the background. Reads stop seeing the records
    /// immediately, which is what "deleted" means here.
    ///
    /// **Not undoable, and not filtered.** There is no per-record condition — that
    /// is what a query with a UDF is for.
    pub fn truncate(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        namespace: Given<String>,
        set: Given<String>,
        beforeNanos: Option<Given<i64>>,
    ) -> PhpResult<()> {
        let before = Given::or_none(beforeNanos, "beforeNanos")?;
        if let Some(nanos) = before {
            if nanos < 0 {
                return Err(AeroError::client(format!(
                    "beforeNanos must not be negative, but is {nanos}; pass null to truncate every \
                     record"
                ))
                .into());
            }
        }
        let body = WireTruncateBody {
            instance: self.instance.clone(),
            timeout_ms: self.admin_timeout(policy)?,
            namespace: namespace.required("namespace")?,
            set: set.required("set")?,
            // Zero and null both mean "every record" to the server, so a caller
            // that computed zero gets the same answer as one that passed nothing.
            before_nanos: before.filter(|nanos| *nanos > 0),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::TRUNCATE, "truncate", &payload)?;
        expect_ok(&header, &reply, "truncate")?;
        Ok(())
    }

    /// Send info commands to a node, and return its answers.
    ///
    /// ```php
    /// $info = $client->info(null, ['build', 'namespaces']);
    /// $info['build'];        // "8.1.3.0"
    /// ```
    ///
    /// The answers come back as an array keyed by command, **in the order the
    /// commands were asked** — which matters for the commands whose answer is a
    /// long record, since a caller often wants to walk them in order.
    ///
    /// `$node` names which node to ask, from `nodes()`. Most info commands answer
    /// for the whole cluster whichever node is asked; the ones that do not —
    /// `statistics`, `latencies` — are exactly the ones worth naming a node for.
    #[php(defaults(node = None))]
    pub fn info(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        commands: Given<Vec<String>>,
        node: Option<Given<String>>,
    ) -> PhpResult<ZBox<ZendHashTable>> {
        let commands = commands.required("commands")?;
        if commands.is_empty() {
            return Err(AeroError::client(
                "info() needs at least one command; the server has no answer for a request that \
                 asks nothing",
            )
            .into());
        }
        let body = WireInfoBody {
            instance: self.instance.clone(),
            timeout_ms: self.admin_timeout(policy)?,
            node: Given::or_none(node, "node")?,
            commands,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::INFO, "info", &payload)?;
        expect_ok(&header, &reply, "info")?;

        let answers: WireInfoReply = decode_body(&reply).map_err(AeroError::codec)?;
        let mut table = ZendHashTable::with_capacity(
            u32::try_from(answers.values.len()).unwrap_or(u32::MAX),
        );
        for (command, value) in answers.values {
            table
                .insert(command.as_str(), value)
                .map_err(|error| crate::error::php_internal(error, "an info answer"))?;
        }
        Ok(table)
    }

    // ===== Users, roles and privileges ======================================

    /// Create a user with a password, and optionally some roles.
    ///
    /// ```php
    /// $client->createUser(null, 'alice', 'secret', ['read-write']);
    /// ```
    ///
    /// **Every command in this family needs `security { enable-security true }` on
    /// the cluster.** Without it they fail with result code 52,
    /// `SecurityNotEnabled` — the server names it exactly, so this client does not
    /// probe for it.
    ///
    /// The password crosses shared memory in the clear, and reaches the server in
    /// the clear under `INTERNAL` auth, where the server hashes it. That is no
    /// weaker than this process holding the password to begin with, but it is worth
    /// knowing: hashing on this side is not an option, because the server decides
    /// the hash.
    #[php(defaults(roles = None))]
    pub fn create_user(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        user: Given<String>,
        password: Given<String>,
        roles: Option<Given<Vec<String>>>,
    ) -> PhpResult<()> {
        let body = WireUserCreateBody {
            target: self.admin_target(policy)?,
            user: user.required("user")?,
            password: password.required("password")?,
            roles: Given::or_none(roles, "roles")?.unwrap_or_default(),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::USER_CREATE, "createUser", &payload)
    }

    /// Create a user the client certificate identifies, with no password.
    ///
    /// For `PKI` authentication, where the certificate carries the identity. **The
    /// daemon has no TLS configuration surface yet**, so it refuses `PKI` auth at
    /// startup — a PKI user can be created from here, but this client cannot yet
    /// connect *as* one.
    #[php(defaults(roles = None))]
    pub fn create_pki_user(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        user: Given<String>,
        roles: Option<Given<Vec<String>>>,
    ) -> PhpResult<()> {
        let body = WireUserRolesBody {
            target: self.admin_target(policy)?,
            user: user.required("user")?,
            roles: Given::or_none(roles, "roles")?.unwrap_or_default(),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::USER_CREATE_PKI, "createPkiUser", &payload)
    }

    /// Remove a user.
    ///
    /// Their open connections are not closed, and anything they are running
    /// continues — the user simply cannot authenticate again.
    pub fn drop_user(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        user: Given<String>,
    ) -> PhpResult<()> {
        let body = WireUserNameBody {
            target: self.admin_target(policy)?,
            user: user.required("user")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::USER_DROP, "dropUser", &payload)
    }

    /// Change a user's password.
    ///
    /// Changing the password of the user the **daemon** authenticates as will break
    /// its connection at the next reconnect, since its own credentials come from its
    /// configuration file and are not updated by this.
    pub fn change_password(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        user: Given<String>,
        password: Given<String>,
    ) -> PhpResult<()> {
        let body = WireUserPasswordBody {
            target: self.admin_target(policy)?,
            user: user.required("user")?,
            password: password.required("password")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::USER_PASSWORD, "changePassword", &payload)
    }

    /// Describe one user, or every user.
    ///
    /// ```php
    /// foreach ($client->queryUsers(null) as $user) {
    ///     echo $user->name(), ': ', implode(', ', $user->roles()), "\n";
    /// }
    /// $alice = $client->queryUsers(null, 'alice')[0] ?? null;
    /// ```
    ///
    /// Naming a user that does not exist is a server error, not an empty list.
    #[php(defaults(user = None))]
    pub fn query_users(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        user: Option<Given<String>>,
    ) -> PhpResult<Vec<AerospikeUser>> {
        let body = WireUserQueryBody {
            target: self.admin_target(policy)?,
            user: Given::or_none(user, "user")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::USER_QUERY, "queryUsers", &payload)?;
        expect_ok(&header, &reply, "queryUsers")?;

        let users: WireUsers = decode_body(&reply).map_err(AeroError::codec)?;
        Ok(users.users.into_iter().map(AerospikeUser::from_wire).collect())
    }

    /// Assign roles to a user.
    ///
    /// Additive: roles the user already holds stay. Use `revokeRoles()` to take one
    /// away.
    pub fn grant_roles(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        user: Given<String>,
        roles: Given<Vec<String>>,
    ) -> PhpResult<()> {
        let body = WireUserRolesBody {
            target: self.admin_target(policy)?,
            user: user.required("user")?,
            roles: roles.required("roles")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::USER_GRANT_ROLES, "grantRoles", &payload)
    }

    /// Take roles away from a user.
    pub fn revoke_roles(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        user: Given<String>,
        roles: Given<Vec<String>>,
    ) -> PhpResult<()> {
        let body = WireUserRolesBody {
            target: self.admin_target(policy)?,
            user: user.required("user")?,
            roles: roles.required("roles")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::USER_REVOKE_ROLES, "revokeRoles", &payload)
    }

    /// Create a role.
    ///
    /// ```php
    /// use Aerospike\{Privilege, PrivilegeCode};
    ///
    /// $client->createRole(null, 'auditor', [
    ///     new Privilege(PrivilegeCode::Read, 'test'),
    /// ], ['10.0.0.0/8'], 1000, 0);
    /// ```
    ///
    /// `$allowlist` restricts where a holder may connect from; empty means anywhere.
    /// `$readQuota` and `$writeQuota` are records per second, and **`0` means
    /// unlimited** — so zero is a value, not an omission.
    #[php(defaults(allowlist = None, readQuota = None, writeQuota = None))]
    pub fn create_role(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        role: Given<String>,
        privileges: Vec<&Zval>,
        allowlist: Option<Given<Vec<String>>>,
        readQuota: Option<Given<i64>>,
        writeQuota: Option<Given<i64>>,
    ) -> PhpResult<()> {
        let body = WireRoleCreateBody {
            target: self.admin_target(policy)?,
            role: role.required("role")?,
            privileges: security::privilege_list(&privileges)?,
            allowlist: Given::or_none(allowlist, "allowlist")?.unwrap_or_default(),
            read_quota: quota("readQuota", Given::or_none(readQuota, "readQuota")?)?,
            write_quota: quota("writeQuota", Given::or_none(writeQuota, "writeQuota")?)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::ROLE_CREATE, "createRole", &payload)
    }

    /// Remove a role.
    ///
    /// Users who held it lose it. Dropping a role that is still granted is allowed:
    /// the grant simply disappears with it.
    pub fn drop_role(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        role: Given<String>,
    ) -> PhpResult<()> {
        let body = WireRoleNameBody {
            target: self.admin_target(policy)?,
            role: role.required("role")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::ROLE_DROP, "dropRole", &payload)
    }

    /// Describe one role, or every role.
    ///
    /// The list includes the server's own built-in roles — `read`, `read-write`,
    /// `sys-admin` and the rest — not only the ones created here.
    #[php(defaults(role = None))]
    pub fn query_roles(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        role: Option<Given<String>>,
    ) -> PhpResult<Vec<AerospikeRole>> {
        let body = WireRoleQueryBody {
            target: self.admin_target(policy)?,
            role: Given::or_none(role, "role")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::ROLE_QUERY, "queryRoles", &payload)?;
        expect_ok(&header, &reply, "queryRoles")?;

        let roles: WireRoles = decode_body(&reply).map_err(AeroError::codec)?;
        Ok(roles.roles.into_iter().map(AerospikeRole::from_wire).collect())
    }

    /// Add privileges to a role.
    pub fn grant_privileges(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        role: Given<String>,
        privileges: Vec<&Zval>,
    ) -> PhpResult<()> {
        let body = WireRolePrivilegesBody {
            target: self.admin_target(policy)?,
            role: role.required("role")?,
            privileges: security::privilege_list(&privileges)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::ROLE_GRANT_PRIVILEGES, "grantPrivileges", &payload)
    }

    /// Take privileges away from a role.
    pub fn revoke_privileges(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        role: Given<String>,
        privileges: Vec<&Zval>,
    ) -> PhpResult<()> {
        let body = WireRolePrivilegesBody {
            target: self.admin_target(policy)?,
            role: role.required("role")?,
            privileges: security::privilege_list(&privileges)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::ROLE_REVOKE_PRIVILEGES, "revokePrivileges", &payload)
    }

    /// Replace a role's address allowlist.
    ///
    /// **Replaces, not adds** — and an empty array *clears* it, which is how a
    /// restriction is removed. There is no other way to say it, so an empty list is
    /// accepted here where most of this API refuses one.
    pub fn set_allowlist(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        role: Given<String>,
        allowlist: Given<Vec<String>>,
    ) -> PhpResult<()> {
        let body = WireRoleAllowlistBody {
            target: self.admin_target(policy)?,
            role: role.required("role")?,
            allowlist: allowlist.required("allowlist")?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::ROLE_ALLOWLIST, "setAllowlist", &payload)
    }

    /// Replace a role's rate quotas, in records per second.
    ///
    /// **`0` lifts a quota**, so zero is a meaningful value rather than an omission —
    /// which is why both arguments are required here.
    pub fn set_quotas(
        &self,
        policy: Option<Given<&AdminPolicy>>,
        role: Given<String>,
        readQuota: Given<i64>,
        writeQuota: Given<i64>,
    ) -> PhpResult<()> {
        let body = WireRoleQuotasBody {
            target: self.admin_target(policy)?,
            role: role.required("role")?,
            read_quota: quota("readQuota", Some(readQuota.required("readQuota")?))?,
            write_quota: quota("writeQuota", Some(writeQuota.required("writeQuota")?))?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        self.admin_command(opcode::ROLE_QUOTAS, "setQuotas", &payload)
    }

    /// Open a multi-record transaction.
    ///
    /// ```php
    /// $txn = $client->beginTransaction();
    /// try {
    ///     $client->put(new Aerospike\WritePolicy(txn: $txn), $from, [new Aerospike\Bin('balance', 70)]);
    ///     $client->put(new Aerospike\WritePolicy(txn: $txn), $to,   [new Aerospike\Bin('balance', 30)]);
    ///     $txn->commit();
    /// } catch (Throwable $e) {
    ///     $txn->abort();
    ///     throw $e;
    /// }
    /// ```
    ///
    /// Commands join the transaction by carrying it on their policy, and either all
    /// of its writes land or none do. **An unfinished transaction rolls back** —
    /// `Aerospike\Transaction` aborts itself when it is destroyed, so a request that
    /// throws does not leave record locks behind.
    ///
    /// `$timeoutMs` is how long the **server** holds the transaction open before
    /// expiring it itself; `null` uses the server's default. The daemon also aborts
    /// one that has been idle too long, which is what bounds the damage a killed
    /// worker can do.
    ///
    /// Needs server **8.0 or later** — refused here, naming the node and its
    /// version, if not — and a namespace configured for **strong consistency**,
    /// which is the server's to enforce and shows up as a failure on the first
    /// write.
    ///
    /// There is no `Txn` constructor to mirror, because a transaction here has to
    /// be opened on a cluster: the daemon holds its read and write sets until the
    /// commit, and one built locally would have nowhere to keep them.
    #[php(defaults(timeoutMs = None))]
    pub fn begin_transaction(&self, timeoutMs: Option<Given<i64>>) -> PhpResult<Transaction> {
        let body = WireTxnBeginBody {
            instance: self.instance.clone(),
            timeout_ms: timeout_millis(Given::or_none(timeoutMs, "timeoutMs")?)?,
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::TXN_BEGIN, "beginTransaction", &payload)?;
        expect_ok(&header, &reply, "beginTransaction")?;

        let handle: WireTxnHandle = decode_body(&reply).map_err(AeroError::codec)?;
        Ok(Transaction::new(
            self.instance.clone(),
            self.settings.clone(),
            handle.id,
        ))
    }

    /// The cluster's nodes, as the daemon's tend loop last saw them.
    ///
    /// The daemon's view rather than a fresh query, which is the useful answer:
    /// this is where a command would be routed right now. A node the daemon has
    /// stopped believing in is still listed, with `isActive()` false.
    pub fn nodes(&self) -> PhpResult<Vec<AerospikeNode>> {
        let body = WireNodesBody {
            instance: self.instance.clone(),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(opcode::NODES, "nodes", &payload)?;
        expect_ok(&header, &reply, "nodes")?;

        let nodes: WireNodes = decode_body(&reply).map_err(AeroError::codec)?;
        Ok(nodes.nodes.into_iter().map(AerospikeNode::from_wire).collect())
    }
}

impl Client {
    fn call(&self, opcode: u16, operation: &str, body: &[u8]) -> AeroResult<(ReplyHeader, Vec<u8>)> {
        transport::call(&self.instance, &self.settings, opcode, operation, body)
    }

    /// The info timeout a management command should use.
    ///
    /// One place, because every one of them takes an `AdminPolicy` whose only
    /// field this is — and `None` has to keep meaning "the daemon's default"
    /// rather than becoming a number invented here.
    fn admin_timeout(&self, policy: Option<Given<&AdminPolicy>>) -> PhpResult<Option<u32>> {
        Ok(Given::or_none(policy, "policy")?.and_then(AdminPolicy::wire_timeout_ms))
    }

    /// The contract's shared target for a security command.
    fn admin_target(
        &self,
        policy: Option<Given<&AdminPolicy>>,
    ) -> PhpResult<WireAdminTarget> {
        Ok(WireAdminTarget {
            instance: self.instance.clone(),
            timeout_ms: self.admin_timeout(policy)?,
        })
    }

    /// Send a security command whose whole answer is "it worked".
    ///
    /// Ten of the fourteen answer with nothing, so this is where their one line of
    /// send-and-check lives rather than ten copies. Takes the encoded body rather
    /// than being generic over `serde::Serialize`, because this crate deliberately
    /// does not depend on serde — the contract owns the encoding.
    fn admin_command(
        &self,
        opcode: u16,
        operation: &str,
        payload: &[u8],
    ) -> PhpResult<()> {
        let (header, reply) = self.call(opcode, operation, payload)?;
        expect_ok(&header, &reply, operation)?;
        Ok(())
    }

    /// Send a command that answers with a task handle, and wrap it.
    ///
    /// Shared by the six long-running commands: they differ only in their body
    /// and their opcode, and every one of them answers the same way.
    ///
    /// Takes the already-encoded body rather than being generic over
    /// `serde::Serialize`, because this crate deliberately does not depend on
    /// serde — the contract crate owns the encoding, and every caller here has
    /// already been through it.
    fn task(&self, opcode: u16, operation: &str, payload: &[u8]) -> PhpResult<Task> {
        let (header, reply) = self.call(opcode, operation, payload)?;
        expect_ok(&header, &reply, operation)?;

        let handle: WireTaskHandle = decode_body(&reply).map_err(AeroError::codec)?;
        Ok(Task::new(
            self.instance.clone(),
            self.settings.clone(),
            handle,
        ))
    }

    /// Build the contract's [`Target`] for a write.
    ///
    /// Resolving the policy argument here rather than in each verb is what keeps
    /// the type check in one place: [`Given::or_none`] is what turns "something
    /// that is not a WritePolicy" into a `TypeError` instead of silently
    /// dropping it, and every write verb needs it.
    fn write_target(
        &self,
        policy: Option<Given<&WritePolicy>>,
        key: Given<&Key>,
    ) -> PhpResult<Target> {
        let policy = Given::or_none(policy, "policy")?;
        Ok(self.target(key.required("key")?, policy.map(WritePolicy::to_wire)))
    }

    /// Build the contract's [`Target`] for a read.
    fn read_target(
        &self,
        policy: Option<Given<&ReadPolicy>>,
        key: Given<&Key>,
    ) -> PhpResult<Target> {
        let policy = Given::or_none(policy, "policy")?;
        Ok(self.target(key.required("key")?, policy.map(ReadPolicy::to_wire)))
    }

    /// Which record, on which instance, under which policy. Shared by every
    /// single-key verb, exactly as the contract intends.
    ///
    /// An absent policy becomes [`WirePolicy::default`], which overrides
    /// nothing — so `null` really is "whatever the daemon is configured with"
    /// rather than a set of defaults invented here.
    fn target(&self, key: &Key, policy: Option<WirePolicy>) -> Target {
        let (namespace, set, user_key) = key.parts();
        Target {
            instance: self.instance.clone(),
            namespace,
            set,
            key: user_key,
            policy: policy.unwrap_or_default(),
        }
    }

    /// The shared body of `add`, `append` and `prepend`, which differ only in
    /// their opcode and in what a bin value is allowed to be.
    fn modify(
        &self,
        kind: Modify,
        policy: Option<Given<&WritePolicy>>,
        key: Given<&Key>,
        bins: Vec<&Zval>,
    ) -> PhpResult<()> {
        let bins = record::bin_list(&bins, kind.name())?;
        for &bin in &bins {
            kind.check(bin)?;
        }

        let body = ModifyBody {
            target: self.write_target(policy, key)?,
            bins: record::wire_bins(&bins),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = self.call(kind.opcode(), kind.name(), &payload)?;
        expect_ok(&header, &reply, kind.name())?;
        Ok(())
    }
}

/// Which of the three bin-wise modifications is being run.
///
/// One type rather than an opcode and a name threaded separately, mirroring the
/// daemon's own `Modify`: the three verbs differ in exactly two ways, and this
/// is where both of them live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modify {
    Add,
    Append,
    Prepend,
}

impl Modify {
    const fn opcode(self) -> u16 {
        match self {
            Modify::Add => opcode::ADD,
            Modify::Append => opcode::APPEND,
            Modify::Prepend => opcode::PREPEND,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Modify::Add => "add",
            Modify::Append => "append",
            Modify::Prepend => "prepend",
        }
    }

    /// Refuse a value the verb cannot possibly apply.
    ///
    /// The server would refuse it too, as a `PARAMETER_ERROR` that names neither
    /// the bin nor what was wrong with it. Checking here costs one match and
    /// turns that into a message a caller can act on.
    fn check(self, bin: &Bin) -> AeroResult<()> {
        let (acceptable, expected) = match self {
            Modify::Add => (
                matches!(bin.wire_value(), WireValue::Int(_) | WireValue::Float(_)),
                "an int or a float",
            ),
            Modify::Append | Modify::Prepend => (
                matches!(bin.wire_value(), WireValue::Str(_) | WireValue::Blob(_)),
                "a string or an Aerospike\\Blob",
            ),
        };
        if acceptable {
            return Ok(());
        }
        Err(AeroError::client(format!(
            "{} needs {expected} for bin \"{}\"",
            self.name(),
            bin.name_str()
        )))
    }
}

/// Narrow a PHP quota to the unsigned field the contract carries.
///
/// An omitted quota is zero, which is what the server reads as "unlimited" — so the
/// default and the explicit lift are the same value, deliberately. A *negative*
/// quota is refused rather than wrapped, because four billion records per second is
/// indistinguishable from no limit and would look like it worked.
fn quota(name: &str, value: Option<i64>) -> AeroResult<u32> {
    match value {
        None => Ok(0),
        Some(records) => u32::try_from(records).map_err(|_| {
            AeroError::client(format!(
                "{name} must be between 0 and {}, but is {records}; 0 means unlimited",
                u32::MAX
            ))
        }),
    }
}

/// Narrow a PHP timeout to the unsigned field the contract carries.
///
/// PHP has one integer type and it is signed, so this is the only place a negative
/// transaction timeout can be caught — and it must be, because wrapping it into
/// four billion milliseconds would open a transaction that effectively never
/// expires on the server.
fn timeout_millis(value: Option<i64>) -> AeroResult<Option<u32>> {
    value
        .map(|millis| {
            u32::try_from(millis).map_err(|_| {
                AeroError::client(format!(
                    "timeoutMs must be between 0 and {}, but is {millis}; pass null for the \
                     server's own default",
                    u32::MAX
                ))
            })
        })
        .transpose()
}

/// Convert a UDF's positional arguments.
///
/// A UDF argument is an ordinary Aerospike value, so this is the bin-value
/// conversion with the position in the message instead of a bin name — which is
/// what a caller needs, since the arguments have no names.
///
/// # Errors
/// An `AeroError` naming the position of a value PHP cannot express.
fn udf_args(args: Option<Vec<&Zval>>) -> AeroResult<Vec<WireValue>> {
    let Some(args) = args else {
        return Ok(Vec::new());
    };
    args.iter()
        .enumerate()
        .map(|(index, zval)| {
            value::zval_to_wire(zval, &value::Path::bin(&format!("argument {index}")))
        })
        .collect()
}

/// Turn any non-OK reply into an error.
fn expect_ok(header: &ReplyHeader, payload: &[u8], operation: &str) -> AeroResult<()> {
    if header.status().is_ok() {
        return Ok(());
    }
    Err(AeroError::from_reply(header, payload, operation))
}

/// A reply whose whole answer is "the record was there" or "it was not".
///
/// `delete` and `exists` are the two verbs for which an absent record is an
/// outcome rather than a failure, and the contract says so twice over: the
/// daemon answers `OK` with a [`WireValue::Bool`] payload, and a daemon that
/// instead reports [`StatusCode::RECORD_NOT_FOUND`] means the same thing. Both
/// are read as `false` — reading only the status would turn "it was already
/// gone" into "it was deleted", which is the one thing these two verbs exist to
/// tell apart.
fn found_or_not(header: &ReplyHeader, payload: &[u8], operation: &str) -> AeroResult<bool> {
    if header.status() == StatusCode::RECORD_NOT_FOUND {
        return Ok(false);
    }
    expect_ok(header, payload, operation)?;

    match decode_body::<WireValue>(payload).map_err(AeroError::codec)? {
        WireValue::Bool(found) => Ok(found),
        other => Err(AeroError::client(format!(
            "the daemon answered {operation} with {other:?} instead of a boolean; the contract \
             says this verb replies with whether the record was there"
        ))),
    }
}
