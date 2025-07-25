<?php

namespace Aerospike {
    /**
     * BatchUDF encapsulates a batch user defined function operation.
     */
    class BatchUdf {
        public function __construct(\Aerospike\BatchUdfPolicy $policy, \Aerospike\Key $key, string $package_name, string $function_name, array $function_args) {}
    }

    /**
     * Map write bit flags.
     * Requires server versions >= 4.3.
     */
    class MapWriteFlags {
        /**
         * MapWriteFlagsDefault is the Default. Allow create or update.
         */
        public static function Default(): \Aerospike\MapWriteFlags {}

        /**
         * MapWriteFlagsCreateOnly means: If the key already exists, the item will be denied.
         * If the key does not exist, a new item will be created.
         */
        public static function CreateOnly(): \Aerospike\MapWriteFlags {}

        /**
         * MapWriteFlagsUpdateOnly means: If the key already exists, the item will be overwritten.
         * If the key does not exist, the item will be denied.
         */
        public static function UpdateOnly(): \Aerospike\MapWriteFlags {}

        /**
         * MapWriteFlagsNoFail means: Do not raise error if a map item is denied due to write flag constraints.
         */
        public static function NoFail(): \Aerospike\MapWriteFlags {}

        /**
         * MapWriteFlagsNoFail means: Allow other valid map items to be committed if a map item is denied due to
         * write flag constraints.
         */
        public static function Partial(): \Aerospike\MapWriteFlags {}
    }

    /**
     * QueryDuration represents the expected duration for a query operation in the Aerospike database. 
     */
    class QueryDuration {
        /**
         * LONG specifies that the query is expected to return more than 100 records per node. The server optimizes for a large record set in
         * the following ways:
         *
         * Allow query to be run in multiple threads using the server's query threading configuration.
         * Do not relax read consistency for AP namespaces.
         * Add the query to the server's query monitor.
         * Do not add the overall latency to the server's latency histogram.
         * Do not allow server timeouts.    
         */
        public static function long(): \Aerospike\QueryDuration {}

        /**
         * Short specifies that the query is expected to return less than 100 records per node. The server optimizes for a small record set in
         * the following ways:
         * Always run the query in one thread and ignore the server's query threading configuration.
         * Allow query to be inlined directly on the server's service thread.
         * Relax read consistency for AP namespaces.
         * Do not add the query to the server's query monitor.
         * Add the overall latency to the server's latency histogram.
         * Allow server timeouts. The default server timeout for a short query is 1 second.
         */
        public static function short(): \Aerospike\QueryDuration {}

        /**
         * LongRelaxAP will treat query as a LONG query, but relax read consistency for AP namespaces.
         * This value is treated exactly like LONG for server versions < 7.1.
         */
        public static function longRelaxAP(): \Aerospike\QueryDuration {}
    }

    /** Instantiate a Client instance to access an Aerospike database cluster and perform database operations.
     * The client is thread-safe. Only one client instance should be used per cluster. Multiple threads should share this cluster instance.
     * Your application uses this class' API to perform database operations such as writing and reading records, and selecting sets of records. Write operations include specialized functionality such as append/prepend and arithmetic addition.
     * Each record may have multiple bins, unless the Aerospike server nodes are configured as "single-bin". In "multi-bin" mode, partial records may be written or read by specifying the relevant subset of bins.
    */
    class Client {
        public $socket;

        /**
         * Connects to the Aerospike database using the provided socket address.
         *
         * If a persisted client object is found for the given socket address, it is returned.
         * Otherwise, a new client object is created, persisted, and returned.
         *
         * # Arguments
         *
         * * `socket` - A string representing the socket address of the Aerospike database.
         *
         * # Returns
         *
         * * `Err("Error connecting to the database".into())` - If an error occurs during connection.
         */
        public static function connect(string $socket): mixed {}

        /**
         * Retrieves the socket address associated with this client.
         *
         * # Returns
         *
         * A string representing the socket address.
         */
        public function socket(): string {}

        /**
         * Write record bin(s). The policy specifies the transaction timeout, record expiration and
         * how the transaction is handled when the record already exists.
         */
        public function put(\Aerospike\WritePolicy $policy, \Aerospike\Key $key, array $bins): mixed {}

        /**
         * Read record for the specified key. Depending on the bins value provided, all record bins,
         * only selected record bins or only the record headers will be returned. The policy can be
         * used to specify timeouts.
         */
        public function get(\Aerospike\ReadPolicy $policy, \Aerospike\Key $key, ?array $bins): \Aerospike\Record {}

        /**
         * Read record for the specified key. Depending on the bins value provided, all record bins,
         * only selected record bins or only the record headers will be returned. The policy can be
         * used to specify timeouts.
         */
        public function getHeader(\Aerospike\ReadPolicy $policy, \Aerospike\Key $key): \Aerospike\Record {}

        /**
         * Add integer bin values to existing record bin values. The policy specifies the transaction
         * timeout, record expiration and how the transaction is handled when the record already
         * exists. This call only works for integer values.
         */
        public function add(\Aerospike\WritePolicy $policy, \Aerospike\Key $key, array $bins): mixed {}

        /**
         * Append bin string values to existing record bin values. The policy specifies the
         * transaction timeout, record expiration and how the transaction is handled when the record
         * already exists. This call only works for string values.
         */
        public function append(\Aerospike\WritePolicy $policy, \Aerospike\Key $key, array $bins): mixed {}

        /**
         * Prepend bin string values to existing record bin values. The policy specifies the
         * transaction timeout, record expiration and how the transaction is handled when the record
         * already exists. This call only works for string values.
         */
        public function prepend(\Aerospike\WritePolicy $policy, \Aerospike\Key $key, array $bins): mixed {}

        /**
         * Delete record for specified key. The policy specifies the transaction timeout.
         * The call returns `true` if the record existed on the server before deletion.
         */
        public function delete(\Aerospike\WritePolicy $policy, \Aerospike\Key $key): bool {}

        /**
         * Reset record's time to expiration using the policy's expiration. Fail if the record does
         * not exist.
         */
        public function touch(\Aerospike\WritePolicy $policy, \Aerospike\Key $key): mixed {}

        /**
         * Determine if a record key exists. The policy can be used to specify timeouts.
         */
        public function exists(\Aerospike\ReadPolicy $policy, \Aerospike\Key $key): bool {}

        /**
         * BatchExecute will read/write multiple records for specified batch keys in one batch call.
         * This method allows different namespaces/bins for each key in the batch.
         * The returned records are located in the same list.
         *
         * BatchRecord can be *BatchRead, *BatchWrite, *BatchDelete or *BatchUDF.
         *
         * Requires server version 6.0+
         */
        public function batch(\Aerospike\BatchPolicy $policy, array $cmds): array {}

        /**
         * Removes all records in the specified namespace/set efficiently.
         */
        public function truncate(\Aerospike\InfoPolicy $policy, string $namespace, string $set_name, ?int $before_nanos): mixed {}

        /**
         * Read all records in the specified namespace and set and return a record iterator. The scan
         * executor puts records on a queue in separate threads. The calling thread concurrently pops
         * records off the queue through the record iterator. Up to `policy.max_concurrent_nodes`
         * nodes are scanned in parallel. If concurrent nodes is set to zero, the server nodes are
         * read in series.
         */
        public function scan(\Aerospike\ScanPolicy $policy, mixed $partition_filter, string $namespace, string $set_name, ?array $bins): \Aerospike\Recordset {}

        /**
         * Execute a query on all server nodes and return a record iterator. The query executor puts
         * records on a queue in separate threads. The calling thread concurrently pops records off
         * the queue through the record iterator.
         */
        public function query(\Aerospike\QueryPolicy $policy, mixed $partition_filter, \Aerospike\Statement $statement): \Aerospike\Recordset {}

        /**
         * CreateIndex creates a secondary index.
         * This asynchronous server call will return before the command is complete.
         * The user can optionally wait for command completion by using the returned
         * IndexTask instance.
         * This method is only supported by Aerospike 3+ servers.
         * If the policy is nil, the default relevant policy will be used.
         */
        public function createIndex(\Aerospike\WritePolicy $policy, string $namespace, string $set_name, string $bin_name, string $index_name, \Aerospike\IndexType $index_type, ?\Aerospike\IndexCollectionType $cit, ?array $ctx): mixed {}

        /**
         * DropIndex deletes a secondary index. It will block until index is dropped on all nodes.
         * This method is only supported by Aerospike 3+ servers.
         * If the policy is nil, the default relevant policy will be used.
         */
        public function dropIndex(\Aerospike\WritePolicy $policy, string $namespace, string $set_name, string $index_name): mixed {}

        /**
         * RegisterUDF registers a package containing user defined functions with server.
         * This asynchronous server call will return before command is complete.
         * The user can optionally wait for command completion by using the returned
         * RegisterTask instance.
         *
         * This method is only supported by Aerospike 3+ servers.
         * If the policy is nil, the default relevant policy will be used.
         */
        public function registerUdf(\Aerospike\WritePolicy $policy, string $udf_body, string $package_name, mixed $language): mixed {}

        /**
         * DropUDF removes a package containing user defined functions in the server.
         * This asynchronous server call will return before command is complete.
         * The user can optionally wait for command completion by using the returned
         * RemoveTask instance.
         *
         * This method is only supported by Aerospike 3+ servers.
         * If the policy is nil, the default relevant policy will be used.
         */
        public function dropUdf(\Aerospike\WritePolicy $policy, string $package_name): mixed {}

        /**
         * ListUDF lists all packages containing user defined functions in the server.
         * This method is only supported by Aerospike 3+ servers.
         * If the policy is nil, the default relevant policy will be used.
         */
        public function listUdf(\Aerospike\ReadPolicy $policy): array {}

        /**
         * Execute executes a user defined function on server and return results.
         * The function operates on a single record.
         * The package name is used to locate the udf file location:
         *
         * udf file = <server udf dir>/<package name>.lua
         *
         * This method is only supported by Aerospike 3+ servers.
         * If the policy is nil, the default relevant policy will be used.
         */
        public function udfExecute(\Aerospike\WritePolicy $policy, \Aerospike\Key $key, string $package_name, string $function_name, array $args): mixed {}

        /**
         * CreateUser creates a new user with password and roles. Clear-text password will be hashed using bcrypt
         * before sending to server.
         */
        public function createUser(\Aerospike\AdminPolicy $policy, string $user, string $password, array $roles): mixed {}

        /**
         * DropUser removes a user from the cluster.
         */
        public function dropUser(\Aerospike\AdminPolicy $policy, string $user): mixed {}

        /**
         * ChangePassword changes a user's password. Clear-text password will be hashed using bcrypt before sending to server.
         */
        public function changePassword(\Aerospike\AdminPolicy $policy, string $user, string $password): mixed {}

        /**
         * GrantRoles adds roles to user's list of roles.
         */
        public function grantRoles(\Aerospike\AdminPolicy $policy, string $user, array $roles): mixed {}

        /**
         * RevokeRoles removes roles from user's list of roles.
         */
        public function revokeRoles(\Aerospike\AdminPolicy $policy, string $user, array $roles): mixed {}

        /**
         * QueryUser retrieves roles for a given user.
         */
        public function queryUsers(\Aerospike\AdminPolicy $policy, ?string $user): array {}

        /**
         * QueryRole retrieves privileges for a given role.
         */
        public function queryRoles(\Aerospike\AdminPolicy $policy, ?string $role_name): array {}

        /**
         * CreateRole creates a user-defined role.
         * Quotas require server security configuration "enable-quotas" to be set to true.
         * Pass 0 for quota values for no limit.
         */
        public function createRole(\Aerospike\AdminPolicy $policy, string $role_name, array $privileges, array $allowlist, int $read_quota, int $write_quota): mixed {}

        /**
         * DropRole removes a user-defined role.
         */
        public function dropRole(\Aerospike\AdminPolicy $policy, string $role_name): mixed {}

        /**
         * GrantPrivileges grant privileges to a user-defined role.
         */
        public function grantPrivileges(\Aerospike\AdminPolicy $policy, string $role_name, array $privileges): mixed {}

        /**
         * RevokePrivileges revokes privileges from a user-defined role.
         */
        public function revokePrivileges(\Aerospike\AdminPolicy $policy, string $role_name, array $privileges): mixed {}

        /**
         * SetAllowlist sets IP address whitelist for a role. If whitelist is nil or empty, it removes existing whitelist from role.
         */
        public function setAllowlist(\Aerospike\AdminPolicy $policy, string $role_name, array $allowlist): mixed {}

        /**
         * SetQuotas sets maximum reads/writes per second limits for a role.  If a quota is zero, the limit is removed.
         * Quotas require server security configuration "enable-quotas" to be set to true.
         * Pass 0 for quota values for no limit.
         */
        public function setQuotas(\Aerospike\AdminPolicy $policy, string $role_name, int $read_quota, int $write_quota): mixed {}
    }

    /**
     * CommitLevel indicates the desired consistency guarantee when committing a transaction on the server.
     */
    class CommitLevel {
        /**
         * CommitAll indicates the server should wait until successfully committing master and all
         * replicas.
         */
        public static function CommitAll(): \Aerospike\CommitLevel {}

        /**
         * CommitMaster indicates the server should wait until successfully committing master only.
         */
        public static function CommitMaster(): \Aerospike\CommitLevel {}
    }

    /**
     * `WritePolicy` encapsulates parameters for all write operations.
     */
    class WritePolicy {
        public $socket_timeout;

        public $respond_per_each_op;

        /**
         * Generation determines expected generation.
         * Generation is the number of times a record has been
         * modified (including creation) on the server.
         * If a write operation is creating a record, the expected generation would be 0.
         */
        public $generation;

        public $use_compression;

        public $filter_expression;

        /**
         * Desired consistency guarantee when committing a transaction on the server. The default
         * (COMMIT_ALL) indicates that the server should wait for master and all replica commits to
         * be successful before returning success to the client.
         */
        public $commit_level;

        public $exit_fast_on_exhausted_connection_pool;


        /**
         * GenerationPolicy qualifies how to handle record writes based on record generation. The default (NONE)
         * indicates that the generation is not used to restrict writes.
         */
        public $generation_policy;

        public $max_retries;

        public $sleep_multiplier;

        public $read_mode_sc;

        public $total_timeout;

        public $read_mode_ap;

        public $send_key;

        /**
         * RecordExistsAction qualifies how to handle writes where the record already exists.
         */
        public $record_exists_action;

        public $durable_delete;

        public $expiration;

        public function __construct() {}

        public function getRecordExistsAction(): \Aerospike\RecordExistsAction {}

        public function setRecordExistsAction(mixed $record_exists_action) {}

        public function getGenerationPolicy(): \Aerospike\GenerationPolicy {}

        public function setGenerationPolicy(mixed $generation_policy) {}

        public function getCommitLevel(): \Aerospike\CommitLevel {}

        public function setCommitLevel(mixed $commit_level) {}

        public function getGeneration(): int {}

        public function setGeneration(int $generation) {}

        /**
         * Expiration determines record expiration in seconds. Also known as TTL (Time-To-Live).
         * Seconds record will live before being removed by the server.
         * Expiration values:
         * TTLServerDefault (0): Default to namespace configuration variable "default-ttl" on the server.
         * TTLDontExpire (MaxUint32): Never expire for Aerospike 2 server versions >= 2.7.2 and Aerospike 3+ server
         * TTLDontUpdate (MaxUint32 - 1): Do not change ttl when record is written. Supported by Aerospike server versions >= 3.10.1
         * > 0: Actual expiration in seconds.
         */
        public function getExpiration(): \Aerospike\Expiration {}

        public function setExpiration(mixed $expiration) {}

        /**
         * RespondPerEachOp defines for client.Operate() method, return a result for every operation.
         * Some list operations do not return results by default (ListClearOp() for example).
         * This can sometimes make it difficult to determine the desired result offset in the returned
         * bin's result list.
         *
         * Setting RespondPerEachOp to true makes it easier to identify the desired result offset
         * (result offset equals bin's operate sequence). This only makes sense when multiple list
         * operations are used in one operate call and some of those operations do not return results
         * by default.
         */
        public function getRespondPerEachOp(): bool {}

        public function setRespondPerEachOp(bool $respond_per_each_op) {}

        /**
         * DurableDelete leaves a tombstone for the record if the transaction results in a record deletion.
         * This prevents deleted records from reappearing after node failures.
         * Valid for Aerospike Server Enterprise Edition 3.10+ only.
         */
        public function getDurableDelete(): bool {}

        public function setDurableDelete(bool $durable_delete) {}

        /**
         * ***************************************************************************
         * ReadPolicy Attrs
         * ***************************************************************************
         */
        public function getMaxRetries(): int {}

        public function setMaxRetries(int $max_retries) {}

        public function getSleepMultiplier(): float {}

        public function setSleepMultiplier(float $sleep_multiplier) {}

        public function getTotalTimeout(): int {}

        public function setTotalTimeout(int $timeout_millis) {}

        public function getSocketTimeout(): int {}

        public function setSocketTimeout(int $timeout_millis) {}

        public function getSendKey(): bool {}

        public function setSendKey(bool $send_key) {}

        public function getUseCompression(): bool {}

        public function setUseCompression(bool $use_compression) {}

        public function getExitFastOnExhaustedConnectionPool(): bool {}

        public function setExitFastOnExhaustedConnectionPool(bool $exit_fast_on_exhausted_connection_pool) {}

        public function getReadModeAp(): \Aerospike\ReadModeAP {}

        public function setReadModeAp(mixed $read_mode_ap) {}

        public function getReadModeSc(): \Aerospike\ReadModeSC {}

        public function setReadModeSc(mixed $read_mode_sc) {}

        public function getFilterExpression(): ?\Aerospike\Expression {}

        public function setFilterExpression(mixed $filter_expression) {}
    }

    /**
     * `ReadPolicy` excapsulates parameters for transaction policy attributes
     * used in all database operation calls.
     */
    class ReadPolicy {
        public $send_key;

        public $filter_expression;

        public $max_retries;

        public $read_mode_sc;

        public $exit_fast_on_exhausted_connection_pool;

        public $read_mode_ap;

        public $sleep_multiplier;

        public $total_timeout;

        public $socket_timeout;

        public $use_compression;

        public function __construct() {}

        /**
         * MaxRetries determines the maximum number of retries before aborting the current transaction.
         * The initial attempt is not counted as a retry.
         *
         * If MaxRetries is exceeded, the transaction will abort with an error.
         *
         * WARNING: Database writes that are not idempotent (such as AddOp)
         * should not be retried because the write operation may be performed
         * multiple times if the client timed out previous transaction attempts.
         * It's important to use a distinct WritePolicy for non-idempotent
         * writes which sets maxRetries = 0;
         *
         * Default for read: 2 (initial attempt + 2 retries = 3 attempts)
         *
         * Default for write: 0 (no retries)
         *
         * Default for partition scan or query with nil filter: 5
         * (6 attempts. See ScanPolicy comments.)
         */
        public function getMaxRetries(): int {}

        public function setMaxRetries(int $max_retries) {}

        /**
         * SleepMultiplier specifies the multiplying factor to be used for exponential backoff during retries.
         * Default to (1.0); Only values greater than 1 are valid.
         */
        public function getSleepMultiplier(): float {}

        public function setSleepMultiplier(float $sleep_multiplier) {}

        /**
         * TotalTimeout specifies total transaction timeout.
         *
         * The TotalTimeout is tracked on the client and also sent to the server along
         * with the transaction in the wire protocol. The client will most likely
         * timeout first, but the server has the capability to Timeout the transaction.
         *
         * If TotalTimeout is not zero and TotalTimeout is reached before the transaction
         * completes, the transaction will abort with TotalTimeout error.
         *
         * If TotalTimeout is zero, there will be no time limit and the transaction will retry
         * on network timeouts/errors until MaxRetries is exceeded. If MaxRetries is exceeded, the
         * transaction also aborts with Timeout error.
         *
         * Default for scan/query: 0 (no time limit and rely on MaxRetries)
         *
         * Default for all other commands: 1000ms
         */
        public function getTotalTimeout(): int {}

        public function setTotalTimeout(int $timeout_millis) {}

        /**
         * SocketTimeout determines network timeout for each attempt.
         *
         * If SocketTimeout is not zero and SocketTimeout is reached before an attempt completes,
         * the Timeout above is checked. If Timeout is not exceeded, the transaction
         * is retried. If both SocketTimeout and Timeout are non-zero, SocketTimeout must be less
         * than or equal to Timeout, otherwise Timeout will also be used for SocketTimeout.
         *
         * Default: 30s
         */
        public function getSocketTimeout(): int {}

        public function setSocketTimeout(int $timeout_millis) {}

        /**
         * SendKey determines to whether send user defined key in addition to hash digest on both reads and writes.
         * If the key is sent on a write, the key will be stored with the record on
         * the server.
         * The default is to not send the user defined key.
         */
        public function getSendKey(): bool {}

        public function setSendKey(bool $send_key) {}

        /**
         * UseCompression uses zlib compression on command buffers sent to the server and responses received
         * from the server when the buffer size is greater than 128 bytes.
         *
         * This option will increase cpu and memory usage (for extra compressed buffers),but
         * decrease the size of data sent over the network.
         *
         * Default: false
         */
        public function getUseCompression(): bool {}

        public function setUseCompression(bool $use_compression) {}

        /**
         * ExitFastOnExhaustedConnectionPool determines if a command that tries to get a
         * connection from the connection pool will wait and retry in case the pool is
         * exhausted until a connection becomes available (or the TotalTimeout is reached).
         * If set to true, an error will be return immediately.
         * If set to false, getting a connection will be retried.
         * This only applies if LimitConnectionsToQueueSize is set to true and the number of open connections to a node has reached ConnectionQueueSize.
         * The default is false
         */
        public function getExitFastOnExhaustedConnectionPool(): bool {}

        public function setExitFastOnExhaustedConnectionPool(bool $exit_fast_on_exhausted_connection_pool) {}

        /**
         * ReadModeAP indicates read policy for AP (availability) namespaces.
         */
        public function getReadModeAp(): \Aerospike\ReadModeAP {}

        public function setReadModeAp(mixed $read_mode_ap) {}

        /**
         * ReadModeSC indicates read policy for SC (strong consistency) namespaces.
         */
        public function getReadModeSc(): \Aerospike\ReadModeSC {}

        public function setReadModeSc(mixed $read_mode_sc) {}

        /**
         * FilterExpression is the optional Filter Expression. Supported on Server v5.2+
         */
        public function getFilterExpression(): ?\Aerospike\Expression {}

        public function setFilterExpression(mixed $filter_expression) {}
    }

    /**
     * BatchPolicy encapsulates parameters for policy attributes used in write operations.
     * This object is passed into methods where database writes can occur.
     */
    class BatchPolicy {
        public $send_key;

        public $read_mode_ap;

        public $allow_inline;

        public $use_compression;

        public $exit_fast_on_exhausted_connection_pool;

        public $sleep_multiplier;

        public $allow_inline_ssd;

        public $total_timeout;

        public $socket_timeout;

        public $read_mode_sc;

        public $concurrent_nodes;

        public $filter_expression;

        public $respond_all_keys;

        public $max_retries;

        public $allow_partial_results;

        public function __construct() {}

        public function getMaxRetries(): int {}

        public function setMaxRetries(int $max_retries) {}

        public function getSleepMultiplier(): float {}

        public function setSleepMultiplier(float $sleep_multiplier) {}

        public function getTotalTimeout(): int {}

        public function setTotalTimeout(int $timeout_millis) {}

        public function getSocketTimeout(): int {}

        public function setSocketTimeout(int $timeout_millis) {}

        public function getSendKey(): bool {}

        public function setSendKey(bool $send_key) {}

        public function getUseCompression(): bool {}

        public function setUseCompression(bool $use_compression) {}

        public function getExitFastOnExhaustedConnectionPool(): bool {}

        public function setExitFastOnExhaustedConnectionPool(bool $exit_fast_on_exhausted_connection_pool) {}

        public function getReadModeAp(): \Aerospike\ReadModeAP {}

        public function setReadModeAp(mixed $read_mode_ap) {}

        public function getReadModeSc(): \Aerospike\ReadModeSC {}

        public function setReadModeSc(mixed $read_mode_sc) {}

        public function getFilterExpression(): ?\Aerospike\Expression {}

        public function setFilterExpression(mixed $filter_expression) {}

        public function getConcurrentNodes(): int {}

        public function setConcurrentNodes(int $concurrent_nodes) {}

        public function getAllowInline(): bool {}

        public function setAllowInline(bool $allow_inline) {}

        public function getAllowInlineSsd(): bool {}

        public function setAllowInlineSsd(bool $allow_inline_ssd) {}

        public function getRespondAllKeys(): bool {}

        public function setRespondAllKeys(bool $respond_all_keys) {}

        public function getAllowPartialResults(): bool {}

        public function setAllowPartialResults(bool $allow_partial_results) {}
    }
    
    /**
     * Specifies whether a command, that needs to be executed on multiple cluster nodes, should be executed sequentially, one node at a time, or in parallel on multiple nodes using the client's
     * thread pool.
     */
    class Concurrency {
        /**
         * Issue commands sequentially. This mode has a performance advantage for small to
         * medium sized batch sizes because requests can be issued in the main transaction thread.
         * This is the default.
         */
        public static function Sequential(): \Aerospike\Concurrency {}

        /**
         * Issue all commands in parallel threads. This mode has a performance advantage for
         * extremely large batch sizes because each node can process the request immediately. The
         * downside is extra threads will need to be created (or takedn from a thread pool).
         */
        public static function Parallel(): \Aerospike\Concurrency {}

        /**
         * Issue up to N commands in parallel threads. When a request completes, a new request
         * will be issued until all threads are complete. This mode prevents too many parallel threads
         * being created for large cluster implementations. The downside is extra threads will still
         * need to be created (or taken from a thread pool).
         *
         * E.g. if there are 16 nodes/namespace combinations requested and concurrency is set to
         * `MaxThreads(8)`, then batch requests will be made for 8 node/namespace combinations in
         * parallel threads. When a request completes, a new request will be issued until all 16
         * requests are complete.
         */
        public static function MaxThreads(int $threads): \Aerospike\Concurrency {}
    }

    /**
     * BatchDeletePolicy is used in batch delete commands.
     */
    class BatchDeletePolicy {
        public $commit_level;

        public $filter_expression;

        public $generation;

        public $durable_delete;

        public $send_key;

        public function __construct() {}

        /**
         * FilterExpression is optional expression filter. If FilterExpression exists and evaluates to false, the specific batch key
         * request is not performed and BatchRecord.ResultCode is set to type.FILTERED_OUT.
         * Default: nil
         */
        public function getFilterExpression(): ?\Aerospike\Expression {}

        public function setFilterExpression(mixed $filter_expression) {}

        /**
         * Desired consistency guarantee when committing a transaction on the server. The default
         * (COMMIT_ALL) indicates that the server should wait for master and all replica commits to
         * be successful before returning success to the client.
         * Default: CommitLevel.COMMIT_ALL
         */
        public function getCommitLevel(): \Aerospike\CommitLevel {}

        public function setCommitLevel(mixed $commit_level) {}

        /**
         * Expected generation. Generation is the number of times a record has been modified
         * (including creation) on the server. This field is only relevant when generationPolicy
         * is not NONE.
         * Default: 0
         */
        public function getGeneration(): int {}

        public function setGeneration(int $generation) {}

        /**
         * If the transaction results in a record deletion, leave a tombstone for the record.
         * This prevents deleted records from reappearing after node failures.
         * Valid for Aerospike Server Enterprise Edition only.
         * Default: false (do not tombstone deleted records).
         */
        public function getDurableDelete(): bool {}

        public function setDurableDelete(bool $durable_delete) {}

        /**
         * Send user defined key in addition to hash digest.
         * If true, the key will be stored with the tombstone record on the server.
         * Default: false (do not send the user defined key)
         */
        public function getSendKey(): bool {}

        public function setSendKey(bool $send_key) {}
    }

    /**
     * HLLPolicy determines the HyperLogLog operation policy.
     */
    class HllPolicy {
        /**
         * new HLLPolicy uses specified optional HLLWriteFlags when performing HLL operations.
         */
        public function __construct(mixed $flags) {}
    }

    /**
     *
     *  GenerationPolicy
     *
     * `GenerationPolicy` determines how to handle record writes based on record generation.
     */
    class GenerationPolicy {
        /**
         * None means: Do not use record generation to restrict writes.
         */
        public static function None(): \Aerospike\GenerationPolicy {}

        /**
         * ExpectGenEqual means: Update/delete record if expected generation is equal to server
         * generation. Otherwise, fail.
         */
        public static function ExpectGenEqual(): \Aerospike\GenerationPolicy {}

        /**
         * ExpectGenGreater means: Update/delete record if expected generation greater than the server
         * generation. Otherwise, fail. This is useful for restore after backup.
         */
        public static function ExpectGenGreater(): \Aerospike\GenerationPolicy {}
    }

    /**
     * Virtual collection of records retrieved through queries and scans. During a query/scan,
     * multiple threads will retrieve records from the server nodes and put these records on an
     * internal queue managed by the recordset. The single user thread consumes these records from the
     * queue.
     */
    class PartitionStatus {
        public $partition_id;

        public $bval;

        public $digest;

        public $retry;

        public function __construct(int $id) {}

        /**
         * get BVal
         */
        public function getBval(): ?int {}

        /**
         * Id shows the partition Id.
         */
        public function getPartitionId(): int {}

        /**
         * Digest records the digest of the last key digest received from the server
         * for this partition.
         */
        public function getDigest(): array {}

        /**
         * Retry signifies if the partition requires a retry.
         */
        public function getRetry(): bool {}
    }

    /**
     * Filter expression, which can be applied to most commands, to control which records are
     * affected by the command.
     */
    class Expression {
        public static function new(?int $cmd, mixed $val, ?\Aerospike\Expression $bin, ?int $flags, mixed $module, array $exps): \Aerospike\Expression {}

        /**
         * Create a record key expression of specified type.
         */
        public static function key(mixed $exp_type): \Aerospike\Expression {}

        /**
         * Create function that returns if the primary key is stored in the record meta data
         * as a boolean expression. This would occur when `send_key` is true on record write.
         */
        public static function keyExists(): \Aerospike\Expression {}

        /**
         * Create 64 bit int bin expression.
         */
        public static function intBin(string $name): \Aerospike\Expression {}

        /**
         * Create string bin expression.
         */
        public static function stringBin(string $name): \Aerospike\Expression {}

        /**
         * Create blob bin expression.
         */
        public static function blobBin(string $name): \Aerospike\Expression {}

        /**
         * Create 64 bit float bin expression.
         */
        public static function floatBin(string $name): \Aerospike\Expression {}

        /**
         * Create geo bin expression.
         */
        public static function geoBin(string $name): \Aerospike\Expression {}

        /**
         * Create list bin expression.
         */
        public static function listBin(string $name): \Aerospike\Expression {}

        /**
         * Create map bin expression.
         */
        public static function mapBin(string $name): \Aerospike\Expression {}

        /**
         * Create a HLL bin expression
         */
        public static function hllBin(string $name): \Aerospike\Expression {}

        /**
         * Create function that returns if bin of specified name exists.
         */
        public static function binExists(string $name): \Aerospike\Expression {}

        /**
         * ExpBinType creates a function that returns bin's integer particle type. Valid values are:
         *
         *	NULL    = 0
         *	INTEGER = 1
         *	FLOAT   = 2
         *	STRING  = 3
         *	BLOB    = 4
         *	DIGEST  = 6
         *	BOOL    = 17
         *	HLL     = 18
         *	MAP     = 19
         *	LIST    = 20
         *	LDT     = 21
         *	GEOJSON = 23
         */
        public static function binType(string $name): \Aerospike\Expression {}

        /**
         * Create function that returns record set name string.
         */
        public static function setName(): \Aerospike\Expression {}

        /**
         * Create function that returns record size on disk.
         * If server storage-engine is memory, then zero is returned.
         *
         * This expression should only be used for server versions less than 7.0. Use
         * record_size for server version 7.0+.
         */
        public static function deviceSize(): \Aerospike\Expression {}

        /**
         * Create expression that returns record size in memory. If server storage-engine is
         * not memory nor data-in-memory, then zero is returned. This expression usually evaluates
         * quickly because record meta data is cached in memory.
         *
         * Requires server version between 5.3 inclusive and 7.0 exclusive.
         * Use record_size for server version 7.0+.
         */
        public static function memorySize(): \Aerospike\Expression {}

        /**
         * Create function that returns record last update time expressed as 64 bit integer
         * nanoseconds since 1970-01-01 epoch.
         */
        public static function lastUpdate(): \Aerospike\Expression {}

        /**
         * Create expression that returns milliseconds since the record was last updated.
         * This expression usually evaluates quickly because record meta data is cached in memory.
         */
        public static function sinceUpdate(): \Aerospike\Expression {}

        /**
         * Create function that returns record expiration time expressed as 64 bit integer
         * nanoseconds since 1970-01-01 epoch.
         */
        public static function voidTime(): \Aerospike\Expression {}

        /**
         * Create function that returns record expiration time (time to live) in integer seconds.
         */
        public static function ttl(): \Aerospike\Expression {}

        /**
         * Create expression that returns if record has been deleted and is still in tombstone state.
         * This expression usually evaluates quickly because record meta data is cached in memory.
         */
        public static function isTombstone(): \Aerospike\Expression {}

        /**
         * Create function that returns record digest modulo as integer.
         */
        public static function digestModulo(int $modulo): \Aerospike\Expression {}

        /**
         * Create function like regular expression string operation.
         */
        public static function regexCompare(string $regex, int $flags, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Create compare geospatial operation.
         */
        public static function geoCompare(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Creates 64 bit integer value
         */
        public static function intVal(int $val): \Aerospike\Expression {}

        /**
         * Creates a Boolean value
         */
        public static function boolVal(bool $val): \Aerospike\Expression {}

        /**
         * Creates String bin value
         */
        public static function stringVal(string $val): \Aerospike\Expression {}

        /**
         * Creates 64 bit float bin value
         */
        public static function floatVal(float $val): \Aerospike\Expression {}

        /**
         * Creates Blob bin value
         */
        public static function blobVal(array $val): \Aerospike\Expression {}

        /**
         * Create List bin PHPValue
         * Not Supported in pre-alpha release
         */
        public static function listVal(array $val): \Aerospike\Expression {}

        /**
         * Create Map bin PHPValue
         * Value must be a map
         */
        public static function mapVal(mixed $val): ?\Aerospike\Expression {}

        /**
         * Create geospatial json string value.
         */
        public static function geoVal(string $val): \Aerospike\Expression {}

        /**
         * Create a Nil PHPValue
         */
        public static function nil(): \Aerospike\Expression {}

        /**
         * Create a Infinity PHPValue
         */
        public static function infinity(): \Aerospike\Expression {}

        /**
         * Create a WildCard PHPValue
         */
        public static function wildcard(): \Aerospike\Expression {}

        /**
         * Create "not" operator expression.
         */
        public static function not(\Aerospike\Expression $exp): \Aerospike\Expression {}

        /**
         * Create "and" (&&) operator that applies to a variable number of expressions.
         * /// (a > 5 || a == 0) && b < 3
         */
        public static function and(array $exps): \Aerospike\Expression {}

        /**
         * Create "or" (||) operator that applies to a variable number of expressions.
         */
        public static function or(array $exps): \Aerospike\Expression {}

        /**
         * Create "xor" (^) operator that applies to a variable number of expressions.
         */
        public static function xor(array $exps): \Aerospike\Expression {}

        /**
         * Create equal (==) expression.
         */
        public static function eq(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Create not equal (!=) expression
         */
        public static function ne(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Create greater than (>) operation.
         */
        public static function gt(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Create greater than or equal (>=) operation.
         */
        public static function ge(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Create less than (<) operation.
         */
        public static function lt(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Create less than or equals (<=) operation.
         */
        public static function le(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Create "add" (+) operator that applies to a variable number of expressions.
         * Return sum of all `FilterExpressions` given. All arguments must resolve to the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function numAdd(array $exps): \Aerospike\Expression {}

        /**
         * Create "subtract" (-) operator that applies to a variable number of expressions.
         * If only one `FilterExpressions` is provided, return the negation of that argument.
         * Otherwise, return the sum of the 2nd to Nth `FilterExpressions` subtracted from the 1st
         * `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function numSub(array $exps): \Aerospike\Expression {}

        /**
         * Create "multiply" (*) operator that applies to a variable number of expressions.
         * Return the product of all `FilterExpressions`. If only one `FilterExpressions` is supplied, return
         * that `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function numMul(array $exps): \Aerospike\Expression {}

        /**
         * Create "divide" (/) operator that applies to a variable number of expressions.
         * If there is only one `FilterExpressions`, returns the reciprocal for that `FilterExpressions`.
         * Otherwise, return the first `FilterExpressions` divided by the product of the rest.
         * All `FilterExpressions` must resolve to the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function numDiv(array $exps): \Aerospike\Expression {}

        /**
         * Create "power" operator that raises a "base" to the "exponent" power.
         * All arguments must resolve to floats.
         * Requires server version 5.6.0+.
         */
        public static function numPow(\Aerospike\Expression $base, \Aerospike\Expression $exponent): \Aerospike\Expression {}

        /**
         * Create "log" operator for logarithm of "num" with base "base".
         * All arguments must resolve to floats.
         * Requires server version 5.6.0+.
         */
        public static function numLog(\Aerospike\Expression $num, \Aerospike\Expression $base): \Aerospike\Expression {}

        /**
         * Create "modulo" (%) operator that determines the remainder of "numerator"
         * divided by "denominator". All arguments must resolve to integers.
         * Requires server version 5.6.0+.
         */
        public static function numMod(\Aerospike\Expression $numerator, \Aerospike\Expression $denominator): \Aerospike\Expression {}

        /**
         * Create operator that returns absolute value of a number.
         * All arguments must resolve to integer or float.
         * Requires server version 5.6.0+.
         */
        public static function numAbs(\Aerospike\Expression $value): \Aerospike\Expression {}

        /**
         * Create expression that rounds a floating point number down to the closest integer value.
         * The return type is float.
         * Requires server version 5.6.0+.
         */
        public static function numFloor(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * Create expression that rounds a floating point number up to the closest integer value.
         * The return type is float.
         * Requires server version 5.6.0+.
         */
        public static function numCeil(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * Create expression that converts an integer to a float.
         * Requires server version 5.6.0+.
         */
        public static function toInt(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * Create expression that converts a float to an integer.
         * Requires server version 5.6.0+.
         */
        public static function toFloat(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * Create integer "and" (&) operator that is applied to two or more integers.
         * All arguments must resolve to integers.
         * Requires server version 5.6.0+.
         */
        public static function intAnd(array $exps): \Aerospike\Expression {}

        /**
         * Create integer "or" (|) operator that is applied to two or more integers.
         * All arguments must resolve to integers.
         * Requires server version 5.6.0+.
         */
        public static function intOr(array $exps): \Aerospike\Expression {}

        /**
         * Create integer "xor" (^) operator that is applied to two or more integers.
         * All arguments must resolve to integers.
         * Requires server version 5.6.0+.
         */
        public static function intXor(array $exps): \Aerospike\Expression {}

        /**
         * Create integer "not" (~) operator.
         * Requires server version 5.6.0+.
         */
        public static function intNot(\Aerospike\Expression $exp): \Aerospike\Expression {}

        /**
         * Create integer "left shift" (<<) operator.
         * Requires server version 5.6.0+.
         */
        public static function intLshift(\Aerospike\Expression $value, \Aerospike\Expression $shift): \Aerospike\Expression {}

        /**
         * Create integer "logical right shift" (>>>) operator.
         * Requires server version 5.6.0+.
         */
        public static function intRshift(\Aerospike\Expression $value, \Aerospike\Expression $shift): \Aerospike\Expression {}

        /**
         * Create integer "arithmetic right shift" (>>) operator.
         * The sign bit is preserved and not shifted.
         * Requires server version 5.6.0+.
         */
        public static function intArshift(\Aerospike\Expression $value, \Aerospike\Expression $shift): \Aerospike\Expression {}

        /**
         * Create expression that returns count of integer bits that are set to 1.
         * Requires server version 5.6.0+
         */
        public static function intCount(\Aerospike\Expression $exp): \Aerospike\Expression {}

        /**
         * Create expression that scans integer bits from left (most significant bit) to
         * right (least significant bit), looking for a search bit value. When the
         * search value is found, the index of that bit (where the most significant bit is
         * index 0) is returned. If "search" is true, the scan will search for the bit
         * value 1. If "search" is false it will search for bit value 0.
         * Requires server version 5.6.0+.
         */
        public static function intLscan(\Aerospike\Expression $value, \Aerospike\Expression $search): \Aerospike\Expression {}

        /**
         * Create expression that scans integer bits from right (least significant bit) to
         * left (most significant bit), looking for a search bit value. When the
         * search value is found, the index of that bit (where the most significant bit is
         * index 0) is returned. If "search" is true, the scan will search for the bit
         * value 1. If "search" is false it will search for bit value 0.
         * Requires server version 5.6.0+.
         */
        public static function intRscan(\Aerospike\Expression $value, \Aerospike\Expression $search): \Aerospike\Expression {}

        /**
         * Create expression that returns the minimum value in a variable number of expressions.
         * All arguments must be the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function min(array $exps): \Aerospike\Expression {}

        /**
         * Create expression that returns the maximum value in a variable number of expressions.
         * All arguments must be the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function max(array $exps): \Aerospike\Expression {}

        /**
         *--------------------------------------------------
         * Variables
         *--------------------------------------------------
         * Conditionally select an expression from a variable number of expression pairs
         * followed by default expression action.
         * Requires server version 5.6.0+.
         * ```
         * /// Args Format: bool exp1, action exp1, bool exp2, action exp2, ..., action-default
         * /// Apply operator based on type.
         */
        public static function cond(array $exps): \Aerospike\Expression {}

        /**
         * Define variables and expressions in scope.
         * Requires server version 5.6.0+.
         * ```
         * /// 5 < a < 10
         */
        public static function expLet(array $exps): \Aerospike\Expression {}

        /**
         * Assign variable to an expression that can be accessed later.
         * Requires server version 5.6.0+.
         * ```
         * /// 5 < a < 10
         */
        public static function def(string $name, \Aerospike\Expression $value): \Aerospike\Expression {}

        /**
         * Retrieve expression value from a variable.
         * Requires server version 5.6.0+.
         */
        public static function var(string $name): \Aerospike\Expression {}

        /**
         * Create unknown value. Used to intentionally fail an expression.
         * The failure can be ignored with `ExpWriteFlags` `EVAL_NO_FAIL`
         * or `ExpReadFlags` `EVAL_NO_FAIL`.
         * Requires server version 5.6.0+.
         */
        public static function unknown(): \Aerospike\Expression {}
    }

    /**
     * Container object for a record bin, comprising a name and a value.
     */
    class Bin {
        public function __construct(string $name, mixed $value) {}
    }

    /**
     * CDTContext defines Nested CDT context. Identifies the location of nested list/map to apply the operation for the current level.
     * An array of CTX identifies location of the list/map on multiple levels on nesting.
     */
    class Context {
        public function __construct() {}

        public static function listOrderFlag(mixed $order, bool $pad): int {}

        /**
         * CtxListIndex defines Lookup list by index offset.
         * If the index is negative, the resolved index starts backwards from end of list.
         * If an index is out of bounds, a parameter error will be returned.
         * Examples:
         * 0: First item.
         * 4: Fifth item.
         * -1: Last item.
         * -3: Third to last item.
         */
        public static function listIndex(int $index): \Aerospike\Context {}

        /**
         * CtxListIndexCreate list with given type at index offset, given an order and pad.
         */
        public static function listIndexCreate(int $index, mixed $order, bool $pad): \Aerospike\Context {}

        /**
         * CtxListRank defines Lookup list by rank.
         * 0 = smallest value
         * N = Nth smallest value
         * -1 = largest value
         */
        public static function listRank(int $rank): \Aerospike\Context {}

        /**
         * CtxListValue defines Lookup list by value.
         */
        public static function listValue(mixed $key): \Aerospike\Context {}

        /**
         * CtxMapIndex defines Lookup map by index offset.
         * If the index is negative, the resolved index starts backwards from end of list.
         * If an index is out of bounds, a parameter error will be returned.
         * Examples:
         * 0: First item.
         * 4: Fifth item.
         * -1: Last item.
         * -3: Third to last item.
         */
        public static function mapIndex(int $index): \Aerospike\Context {}

        /**
         * CtxMapRank defines Lookup map by rank.
         * 0 = smallest value
         * N = Nth smallest value
         * -1 = largest value
         */
        public static function mapRank(int $rank): \Aerospike\Context {}

        /**
         * CtxMapKey defines Lookup map by key.
         */
        public static function mapKey(mixed $key): \Aerospike\Context {}

        /**
         * CtxMapKeyCreate creates map with given type at map key.
         */
        public static function mapKeyCreate(mixed $key, mixed $order): \Aerospike\Context {}

        /**
         * CtxMapValue defines Lookup map by value.
         */
        public static function mapValue(mixed $key): \Aerospike\Context {}
    }

    /**
     * ListReturnType determines the returned values in CDT List operations.
     */
    class ListReturnType {
        /**
         * ListReturnTypeNone will not return a result.
         */
        public static function None(): \Aerospike\ListReturnType {}

        /**
         * ListReturnTypeIndex will return index offset order.
         * 0 = first key
         * N = Nth key
         * -1 = last key
         */
        public static function Index(): \Aerospike\ListReturnType {}

        /**
         * ListReturnTypeReverseIndex will return reverse index offset order.
         * 0 = last key
         * -1 = first key
         */
        public static function ReverseIndex(): \Aerospike\ListReturnType {}

        /**
         * ListReturnTypeRank will return value order.
         * 0 = smallest value
         * N = Nth smallest value
         * -1 = largest value
         */
        public static function Rank(): \Aerospike\ListReturnType {}

        /**
         * ListReturnTypeReverseRank will return reverse value order.
         * 0 = largest value
         * N = Nth largest value
         * -1 = smallest value
         */
        public static function ReverseRank(): \Aerospike\ListReturnType {}

        /**
         * ListReturnTypeCount will return count of items selected.
         */
        public static function count(): \Aerospike\ListReturnType {}

        /**
         * ListReturnTypeValue will return value for single key read and value list for range read.
         */
        public static function Value(): \Aerospike\ListReturnType {}

        /**
         * ListReturnTypeExists returns true if count > 0.
         */
        public static function Exists(): \Aerospike\ListReturnType {}

        /**
         * ListReturnTypeInverted will invert meaning of list command and return values.  For example:
         * ListOperation.getByIndexRange(binName, index, count, ListReturnType.INDEX | ListReturnType.INVERTED)
         * With the INVERTED flag enabled, the items outside of the specified index range will be returned.
         * The meaning of the list command can also be inverted.  For example:
         * ListOperation.removeByIndexRange(binName, index, count, ListReturnType.INDEX | ListReturnType.INVERTED);
         * With the INVERTED flag enabled, the items outside of the specified index range will be removed and returned.
         */
        public function inverted(): \Aerospike\ListReturnType {}
    }

    /**
     * Implementation of the GeoJson Value for Aerospike.
     */
    class GeoJSON {
        public $value;

        public function getValue(): string {}

        public function setValue(string $geo) {}

        /**
         * Returns a string representation of the value.
         */
        public function asString(): string {}
    }

    /**
     * Implementation of the HyperLogLog (HLL) data structure for Aerospike.
     */
    class HLL {
        public $value;

        public function getValue(): array {}

        public function setValue(array $hll) {}

        /**
         * Returns a string representation of the value.
         */
        public function asString(): string {}
    }

    /**
     * ReadModeAP is the read policy in AP (availability) mode namespaces.
     * It indicates how duplicates should be consulted in a read operation.
     * Only makes a difference during migrations and only applicable in AP mode.
     */
    class ReadModeAP {
        /**
         * ReadModeAPOne indicates that a single node should be involved in the read operation.
         */
        public static function one(): \Aerospike\ReadModeAP {}

        /**
         * ReadModeAPAll indicates that all duplicates should be consulted in
         * the read operation.
         */
        public static function all(): \Aerospike\ReadModeAP {}
    }

    /**
     * ListPolicy directives when creating a list and writing list items.
     */
    class ListPolicy {
        /**
         * NewListPolicy creates a policy with directives when creating a list and writing list items.
         * Flags are ListWriteFlags. You can specify multiple by `or`ing them together.
         */
        public function __construct(mixed $order, ?array $flags) {}
    }

    /**
     * IndexType the type of the secondary index.
     */
    class IndexType {
        /**
         * NUMERIC specifies an index on numeric values.
         */
        public static function Numeric(): \Aerospike\IndexType {}

        /**
         * STRING specifies an index on string values.
         */
        public static function String(): \Aerospike\IndexType {}

        /**
         * BLOB specifies a []byte index. Requires server version 7.0+.
         */
        public static function Blob(): \Aerospike\IndexType {}

        /**
         * GEO2DSPHERE specifies 2-dimensional spherical geospatial index.
         */
        public static function Geo2DSphere(): \Aerospike\IndexType {}
    }

    /**
     * MapPolicy directives when creating a map and writing map items.
     */
    class MapPolicy {
        /**
         * NewMapPolicy creates a MapPolicy with WriteMode. Use with servers before v4.3.
         */
        public function __construct(\Aerospike\MapOrderType $order, ?array $flags, ?bool $persisted_index) {}
    }

    /**
     * RecordExistsAction determines how to handle writes when
     * the record already exists.
     */
    class RecordExistsAction {
        /**
         * Update means: Create or update record.
         * Merge write command bins with existing bins.
         */
        public static function Update(): \Aerospike\RecordExistsAction {}

        /**
         * UpdateOnly means: Update record only. Fail if record does not exist.
         * Merge write command bins with existing bins.
         */
        public static function UpdateOnly(): \Aerospike\RecordExistsAction {}

        /**
         * Replace means: Create or replace record.
         * Delete existing bins not referenced by write command bins.
         * Supported by Aerospike 2 server versions >= 2.7.5 and
         * Aerospike 3 server versions >= 3.1.6.
         */
        public static function Replace(): \Aerospike\RecordExistsAction {}

        /**
         * ReplaceOnly means: Replace record only. Fail if record does not exist.
         * Delete existing bins not referenced by write command bins.
         * Supported by Aerospike 2 server versions >= 2.7.5 and
         * Aerospike 3 server versions >= 3.1.6.
         */
        public static function ReplaceOnly(): \Aerospike\RecordExistsAction {}

        /**
         * CreateOnly means: Create only. Fail if record exists.
         */
        public static function CreateOnly(): \Aerospike\RecordExistsAction {}
    }

    /**
     * Server particle types. Unsupported types are commented out.
     */
    class ParticleType {
        public static function null(): \Aerospike\ParticleType {}

        public static function integer(): \Aerospike\ParticleType {}

        public static function float(): \Aerospike\ParticleType {}

        public static function string(): \Aerospike\ParticleType {}

        public static function blob(): \Aerospike\ParticleType {}

        public static function digest(): \Aerospike\ParticleType {}

        public static function bool(): \Aerospike\ParticleType {}

        public static function hll(): \Aerospike\ParticleType {}

        public static function map(): \Aerospike\ParticleType {}

        public static function list(): \Aerospike\ParticleType {}

        public static function geoJson(): \Aerospike\ParticleType {}
    }

    /**
     * `ScanPolicy` encapsulates optional parameters used in scan operations.
     */
    class ScanPolicy {
        public $use_compression;

        public $exit_fast_on_exhausted_connection_pool;

        public $read_mode_sc;

        public $filter_expression;

        public $read_mode_ap;

        public $sleep_multiplier;

        public $max_concurrent_nodes;

        public $record_queue_size;

        public $max_retries;

        public $total_timeout;

        public $max_records;

        public $socket_timeout;

        public $send_key;

        public function __construct() {}

        /**
         * ***************************************************************************
         * MultiPolicy Attrs
         * ***************************************************************************
         */
        public function getMaxRecords(): int {}

        public function setMaxRecords(int $max_records) {}

        public function getMaxConcurrentNodes(): int {}

        public function setMaxConcurrentNodes(int $max_concurrent_nodes) {}

        public function getRecordQueueSize(): int {}

        public function setRecordQueueSize(int $record_queue_size) {}

        /**
         * ***************************************************************************
         * ReadPolicy Attrs
         * ***************************************************************************
         */
        public function getMaxRetries(): int {}

        public function setMaxRetries(int $max_retries) {}

        public function getSleepMultiplier(): float {}

        public function setSleepMultiplier(float $sleep_multiplier) {}

        public function getTotalTimeout(): int {}

        public function setTotalTimeout(int $timeout_millis) {}

        public function getSocketTimeout(): int {}

        public function setSocketTimeout(int $timeout_millis) {}

        public function getSendKey(): bool {}

        public function setSendKey(bool $send_key) {}

        public function getUseCompression(): bool {}

        public function setUseCompression(bool $use_compression) {}

        public function getExitFastOnExhaustedConnectionPool(): bool {}

        public function setExitFastOnExhaustedConnectionPool(bool $exit_fast_on_exhausted_connection_pool) {}

        public function getReadModeAp(): \Aerospike\ReadModeAP {}

        public function setReadModeAp(mixed $read_mode_ap) {}

        public function getReadModeSc(): \Aerospike\ReadModeSC {}

        public function setReadModeSc(mixed $read_mode_sc) {}

        public function getFilterExpression(): ?\Aerospike\Expression {}

        public function setFilterExpression(mixed $filter_expression) {}
    }

    /** 
     * `ConsistencyLevel` indicates how replicas should be consulted in a read
     *   operation to provide the desired consistency guarantee.
    */ 
    class ConsistencyLevel {
        /**
         * ConsistencyOne indicates only a single replica should be consulted in
         * the read operation.
         */
        public static function ConsistencyOne(): \Aerospike\ConsistencyLevel {}

        /**
         * ConsistencyAll indicates that all replicas should be consulted in
         * the read operation.
         */
        public static function ConsistencyAll(): \Aerospike\ConsistencyLevel {}
    }

    /**
     * Specifies whether a command, that needs to be executed on multiple cluster nodes, should be
     * executed sequentially, one node at a time, or in parallel on multiple nodes using the client's
     * thread pool.
     */
    class ListOrderType {
        public function flag(): int {}

        /**
         * ListOrderOrdered signifies that list is Ordered.
         */
        public static function Ordered(): \Aerospike\ListOrderType {}

        /**
         * ListOrderUnordered signifies that list is not ordered. This is the default.
         */
        public static function Unordered(): \Aerospike\ListOrderType {}
    }

    /**
     * QueryPolicy encapsulates parameters for policy attributes used in query operations.
     */
    class QueryPolicy {
        public $expected_duration;

        public $max_retries;

        public $use_compression;

        public $read_mode_sc;

        public $filter_expression;

        public $max_concurrent_nodes;

        public $exit_fast_on_exhausted_connection_pool;

        public $send_key;

        public $sleep_multiplier;

        public $total_timeout;

        public $read_mode_ap;

        public $record_queue_size;

        public $socket_timeout;

        public function __construct() {}

        /**
         * QueryDuration represents the expected duration for a query operation in the Aerospike database. 
         * It provides options for specifying whether a query is expected to return a large number of records per node (Long), 
         * a small number of records per node (Short), or a long query with relaxed read consistency for AP namespaces (LongRelaxAP). 
         * These options influence how the server optimizes query execution to meet the expected duration requirements.
         */
        public function getExpectedDuration(): \Aerospike\QueryDuration {}

        public function setExpectedDuration(mixed $expected_duration) {}

        /**
         * ***************************************************************************
         * MultiPolicy Attrs
         * ***************************************************************************
         * Maximum number of concurrent requests to server nodes at any point in time.
         * If there are 16 nodes in the cluster and maxConcurrentNodes is 8, then queries
         * will be made to 8 nodes in parallel.  When a query completes, a new query will
         * be issued until all 16 nodes have been queried.
         * Default (0) is to issue requests to all server nodes in parallel.
         * 1 will to issue requests to server nodes one by one avoiding parallel queries.
         */
        public function getMaxConcurrentNodes(): int {}

        public function setMaxConcurrentNodes(int $max_concurrent_nodes) {}

        /**
         * Number of records to place in queue before blocking.
         * Records received from multiple server nodes will be placed in a queue.
         * A separate goroutine consumes these records in parallel.
         * If the queue is full, the producer goroutines will block until records are consumed.
         */
        public function getRecordQueueSize(): int {}

        public function setRecordQueueSize(int $record_queue_size) {}

        /**
         * ***************************************************************************
         * ReadPolicy Attrs
         * ***************************************************************************
         */
        public function getMaxRetries(): int {}

        public function setMaxRetries(int $max_retries) {}

        public function getSleepMultiplier(): float {}

        public function setSleepMultiplier(float $sleep_multiplier) {}

        public function getTotalTimeout(): int {}

        public function setTotalTimeout(int $timeout_millis) {}

        public function getSocketTimeout(): int {}

        public function setSocketTimeout(int $timeout_millis) {}

        public function getSendKey(): bool {}

        public function setSendKey(bool $send_key) {}

        public function getUseCompression(): bool {}

        public function setUseCompression(bool $use_compression) {}

        public function getExitFastOnExhaustedConnectionPool(): bool {}

        public function setExitFastOnExhaustedConnectionPool(bool $exit_fast_on_exhausted_connection_pool) {}

        public function getReadModeAp(): \Aerospike\ReadModeAP {}

        public function setReadModeAp(mixed $read_mode_ap) {}

        public function getReadModeSc(): \Aerospike\ReadModeSC {}

        public function setReadModeSc(mixed $read_mode_sc) {}

        public function getFilterExpression(): ?\Aerospike\Expression {}

        public function setFilterExpression(mixed $filter_expression) {}
    }

    /**
     * `AdminPolicy` encapsulates parameters for all admin operations.
     */
    class AdminPolicy {
        public $timeout;

        public function __construct() {}

        /**
         * User administration command socket timeout.
         * Default is 2 seconds.
         */
        public function getTimeout(): int {}

        public function setTimeout(int $timeout_millis) {}
    }

    /**
     * BatchDelete encapsulates a batch delete operation.
     */
    class BatchDelete {
        public function __construct(\Aerospike\BatchDeletePolicy $policy, \Aerospike\Key $key) {}
    }

    /**
     * Represents UDF (User-Defined Function) metadata for Aerospike.
     */
    class UdfMeta {
        public $package_name;

        public $language;

        public $hash;

        /**
         * Getter method to retrieve the package name of the UDF.
         */
        public function getPackageName(): string {}

        /**
         * Getter method to retrieve the hash of the UDF.
         */
        public function getHash(): string {}

        /**
         * Getter method to retrieve the language of the UDF.
         */
        public function getLanguage(): \Aerospike\UdfLanguage {}
    }

    /**
     * BitResizeFlags specifies the bitwise operation flags for resize.
     */
    class BitwiseResizeFlags {
        /**
         * BitResizeFlagsDefault specifies the defalt flag.
         */
        public static function Default(): \Aerospike\BitwiseResizeFlags {}

        /**
         * BitResizeFlagsFromFront Adds/removes bytes from the beginning instead of the end.
         */
        public static function FromFront(): \Aerospike\BitwiseResizeFlags {}

        /**
         * BitResizeFlagsGrowOnly will only allow the []byte size to increase.
         */
        public static function GrowOnly(): \Aerospike\BitwiseResizeFlags {}

        /**
         * BitResizeFlagsShrinkOnly will only allow the []byte size to decrease.
         */
        public static function ShrinkOnly(): \Aerospike\BitwiseResizeFlags {}
    }

    /**
     * MapReturnType defines the map return type.
     * Type of data to return when selecting or removing items from the map.
     */
    class MapReturnType {
        /**
         * NONE will will not return a result.
         */
        public static function None(): \Aerospike\MapReturnType {}

        /**
         * INDEX will return key index order.
         *
         * 0 = first key
         * N = Nth key
         * -1 = last key
         */
        public static function Index(): \Aerospike\MapReturnType {}

        /**
         * REVERSE_INDEX will return reverse key order.
         *
         * 0 = last key
         * -1 = first key
         */
        public static function ReverseIndex(): \Aerospike\MapReturnType {}

        /**
         * RANK will return value order.
         *
         * 0 = smallest value
         * N = Nth smallest value
         * -1 = largest value
         */
        public static function Rank(): \Aerospike\MapReturnType {}

        /**
         * REVERSE_RANK will return reverse value order.
         *
         * 0 = largest value
         * N = Nth largest value
         * -1 = smallest value
         */
        public static function ReverseRank(): \Aerospike\MapReturnType {}

        /**
         * COUNT will return count of items selected.
         */
        public static function Count(): \Aerospike\MapReturnType {}

        /**
         * KEY will return key for single key read and key list for range read.
         */
        public static function Key(): \Aerospike\MapReturnType {}

        /**
         * VALUE will return value for single key read and value list for range read.
         */
        public static function Value(): \Aerospike\MapReturnType {}

        /**
         * KEY_VALUE will return key/value items. The possible return types are:
         *
         * map[interface{}]interface{} : Returned for unordered maps
         * []MapPair : Returned for range results where range order needs to be preserved.
         */
        public static function KeyValue(): \Aerospike\MapReturnType {}

        /**
         * EXISTS returns true if count > 0.
         */
        public static function Exists(): \Aerospike\MapReturnType {}

        /**
         * UNORDERED_MAP returns an unordered map.
         */
        public static function UnorderedMap(): \Aerospike\MapReturnType {}

        /**
         * ORDERED_MAP returns an ordered map.
         */
        public static function OrderedMap(): \Aerospike\MapReturnType {}

        /**
         * INVERTED will invert meaning of map command and return values.  For example:
         * MapRemoveByKeyRange(binName, keyBegin, keyEnd, MapReturnType.KEY | MapReturnType.INVERTED)
         * With the INVERTED flag enabled, the keys outside of the specified key range will be removed and returned.
         */
        public function Inverted(): \Aerospike\MapReturnType {}
    }

    /**
     * ReadModeSC is the read policy in SC (strong consistency) mode namespaces.
     * Determines SC read consistency options.
     */
    class ReadModeSC {
        /**
         * ReadModeSCSession ensures this client will only see an increasing sequence of record versions.
         * Server only reads from master.  This is the default.
         */
        public static function Session(): \Aerospike\ReadModeSC {}

        /**
         * ReadModeSCLinearize ensures ALL clients will only see an increasing sequence of record versions.
         * Server only reads from master.
         */
        public static function Linearize(): \Aerospike\ReadModeSC {}

        /**
         * ReadModeSCAllowReplica indicates that the server may read from master or any full (non-migrating) replica.
         * Increasing sequence of record versions is not guaranteed.
         */
        public static function AllowReplica(): \Aerospike\ReadModeSC {}

        /**
         * ReadModeSCAllowUnavailable indicates that the server may read from master or any full (non-migrating) replica or from unavailable
         * partitions.  Increasing sequence of record versions is not guaranteed.
         */
        public static function AllowUnavailable(): \Aerospike\ReadModeSC {}
    }

    /**
     * Query filter definition. Currently, only one filter is allowed in a Statement, and must be on a
     * bin which has a secondary index defined.
     * Filter instances should be instantiated using one of the provided macros.
     */
    class Filter {
        /**
         * NewEqualFilter creates a new equality filter instance for query.
         * Value can be an integer, string or a blob (byte array). Byte arrays are only supported on server v7+.
         */
        public static function equal(string $bin_name, mixed $value, ?array $ctx): \Aerospike\Filter {}

        /**
         * NewRangeFilter creates a range filter for query.
         * Range arguments must be int64 values.
         * String ranges are not supported.
         */
        public static function range(string $bin_name, mixed $begin, mixed $end, ?array $ctx): \Aerospike\Filter {}

        /**
         * NewContainsFilter creates a contains filter for query on collection index.
         * Value can be an integer, string or a blob (byte array). Byte arrays are only supported on server v7+.
         */
        public static function contains(string $bin_name, mixed $value, ?\Aerospike\IndexCollectionType $cit, ?array $ctx): \Aerospike\Filter {}

        /**
         * NewContainsRangeFilter creates a contains filter for query on ranges of data in a collection index.
         */
        public static function containsRange(string $bin_name, mixed $begin, mixed $end, ?\Aerospike\IndexCollectionType $cit, ?array $ctx): \Aerospike\Filter {}

        /**
         * NewGeoWithinRegionFilter creates a geospatial "within region" filter for query.
         * Argument must be a valid GeoJSON region.
         */
        public static function withinRegion(string $bin_name, string $region, ?\Aerospike\IndexCollectionType $cit, ?array $ctx): \Aerospike\Filter {}

        /**
         * NewGeoWithinRegionForCollectionFilter creates a geospatial "within region" filter for query on collection index.
         * Argument must be a valid GeoJSON region.
         */
        public static function withinRadius(string $bin_name, float $lat, float $lng, float $radius, ?\Aerospike\IndexCollectionType $cit, ?array $ctx): \Aerospike\Filter {}

        /**
         * NewGeoRegionsContainingPointFilter creates a geospatial "containing point" filter for query.
         * Argument must be a valid GeoJSON point.
         */
        public static function regionsContainingPoint(string $bin_name, float $lat, float $lng, ?\Aerospike\IndexCollectionType $cit, ?array $ctx): \Aerospike\Filter {}
    }

    /**
     * BatchRead specifies the Key and bin names used in batch read commands
     * where variable bins are needed for each key.
     */
    class BatchRead {
        public function __construct(\Aerospike\BatchReadPolicy $policy, \Aerospike\Key $key, ?array $bins) {}

        /**
         * Optional read policy.
         */
        public static function ops(\Aerospike\BatchReadPolicy $policy, \Aerospike\Key $key, array $ops): \Aerospike\BatchRead {}

        /**
         * Ops specifies the operations to perform for every key.
         * Ops are mutually exclusive with BinNames.
         * A binName can be emulated with `GetOp(binName)`
         * Supported by server v5.6.0+.
         */
        public static function header(\Aerospike\BatchReadPolicy $policy, \Aerospike\Key $key): \Aerospike\BatchRead {}
    }

    /**
     * BitPolicy determines the Bit operation policy.
     */
    class BitwisePolicy {
        /**
         * new BitwisePolicy(int) will return a BitPolicy will provided flags.
         */
        public function __construct(mixed $flags) {}
    }

    /**
     * Bit operations. Create bit operations used by client operate command.
     * Offset orientation is left-to-right.  Negative offsets are supported.
     * If the offset is negative, the offset starts backwards from end of the bitmap.
     * If an offset is out of bounds, a parameter error will be returned.
     *
     *	Nested CDT operations are supported by optional CTX context arguments.  Example:
     *	bin = [[0b00000001, 0b01000010],[0b01011010]]
     *	Resize first bitmap (in a list of bitmaps) to 3 bytes.
     *	BitOperation.resize("bin", 3, BitResizeFlags.DEFAULT, CTX.listIndex(0))
     *	bin result = [[0b00000001, 0b01000010, 0b00000000],[0b01011010]]
     */
    class BitwiseOp {
        /**
         * BitResizeOp creates byte "resize" operation.
         * Server resizes []byte to byteSize according to resizeFlags (See BitResizeFlags).
         * Server does not return a value.
         * Example:
         *
         *	$bin = [0b00000001, 0b01000010]
         *	$byteSize = 4
         *	$resizeFlags = 0
         *	$bin result = [0b00000001, 0b01000010, 0b00000000, 0b00000000]
         */
        public static function resize(\Aerospike\BitwisePolicy $policy, string $bin_name, int $byte_size, mixed $resize_flags, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitInsertOp creates byte "insert" operation.
         * Server inserts value bytes into []byte bin at byteOffset.
         * Server does not return a value.
         * Example:
         *
         *	$bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	$byteOffset = 1
         *	$value = [0b11111111, 0b11000111]
         *	$bin result = [0b00000001, 0b11111111, 0b11000111, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         */
        public static function insert(\Aerospike\BitwisePolicy $policy, string $bin_name, int $byte_offset, array $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitRemoveOp creates byte "remove" operation.
         * Server removes bytes from []byte bin at byteOffset for byteSize.
         * Server does not return a value.
         * Example:
         *
         *	$bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	$byteOffset = 2
         *	$byteSize = 3
         *	$bin result = [0b00000001, 0b01000010]
         */
        public static function remove(\Aerospike\BitwisePolicy $policy, string $bin_name, int $byte_offset, int $byte_size, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitSetOp creates bit "set" operation.
         * Server sets value on []byte bin at bitOffset for bitSize.
         * Server does not return a value.
         * Example:
         *
         *	$bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	$bitOffset = 13
         *	$bitSize = 3
         *	$value = [0b11100000]
         *	$bin result = [0b00000001, 0b01000111, 0b00000011, 0b00000100, 0b00000101]
         */
        public static function set(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, array $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitOrOp creates bit "or" operation.
         * Server performs bitwise "or" on value and []byte bin at bitOffset for bitSize.
         * Server does not return a value.
         * Example:
         *
         *	$bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	$bitOffset = 17
         *	$bitSize = 6
         *	$value = [0b10101000]
         *	bin result = [0b00000001, 0b01000010, 0b01010111, 0b00000100, 0b00000101]
         */
        public static function or(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, array $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitXorOp creates bit "exclusive or" operation.
         * Server performs bitwise "xor" on value and []byte bin at bitOffset for bitSize.
         * Server does not return a value.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 17
         *	bitSize = 6
         *	value = [0b10101100]
         *	bin result = [0b00000001, 0b01000010, 0b01010101, 0b00000100, 0b00000101]
         */
        public static function xor(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, array $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitAndOp creates bit "and" operation.
         * Server performs bitwise "and" on value and []byte bin at bitOffset for bitSize.
         * Server does not return a value.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 23
         *	bitSize = 9
         *	value = [0b00111100, 0b10000000]
         *	bin result = [0b00000001, 0b01000010, 0b00000010, 0b00000000, 0b00000101]
         */
        public static function and(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, array $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitNotOp creates bit "not" operation.
         * Server negates []byte bin starting at bitOffset for bitSize.
         * Server does not return a value.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 25
         *	bitSize = 6
         *	bin result = [0b00000001, 0b01000010, 0b00000011, 0b01111010, 0b00000101]
         */
        public static function not(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitLShiftOp creates bit "left shift" operation.
         * Server shifts left []byte bin starting at bitOffset for bitSize.
         * Server does not return a value.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 32
         *	bitSize = 8
         *	shift = 3
         *	bin result = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00101000]
         */
        public static function lshift(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, int $shift, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitRShiftOp creates bit "right shift" operation.
         * Server shifts right []byte bin starting at bitOffset for bitSize.
         * Server does not return a value.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 0
         *	bitSize = 9
         *	shift = 1
         *	bin result = [0b00000000, 0b11000010, 0b00000011, 0b00000100, 0b00000101]
         */
        public static function rshift(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, int $shift, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitAddOp creates bit "add" operation.
         * Server adds value to []byte bin starting at bitOffset for bitSize. BitSize must be <= 64.
         * Signed indicates if bits should be treated as a signed number.
         * If add overflows/underflows, BitOverflowAction is used.
         * Server does not return a value.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 24
         *	bitSize = 16
         *	value = 128
         *	signed = false
         *	bin result = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b10000101]
         */
        public static function add(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, int $value, bool $signed, mixed $action, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitSubtractOp creates bit "subtract" operation.
         * Server subtracts value from []byte bin starting at bitOffset for bitSize. BitSize must be <= 64.
         * Signed indicates if bits should be treated as a signed number.
         * If add overflows/underflows, BitOverflowAction is used.
         * Server does not return a value.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 24
         *	bitSize = 16
         *	value = 128
         *	signed = false
         *	bin result = [0b00000001, 0b01000010, 0b00000011, 0b0000011, 0b10000101]
         */
        public static function subtract(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, int $value, bool $signed, mixed $action, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitSetIntOp creates bit "setInt" operation.
         * Server sets value to []byte bin starting at bitOffset for bitSize. Size must be <= 64.
         * Server does not return a value.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 1
         *	bitSize = 8
         *	value = 127
         *	bin result = [0b00111111, 0b11000010, 0b00000011, 0b0000100, 0b00000101]
         */
        public static function setInt(\Aerospike\BitwisePolicy $policy, string $bin_name, int $bit_offset, int $bit_size, int $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitGetOp creates bit "get" operation.
         * Server returns bits from []byte bin starting at bitOffset for bitSize.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 9
         *	bitSize = 5
         *	returns [0b1000000]
         */
        public static function get(string $bin_name, int $bit_offset, int $bit_size, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitCountOp creates bit "count" operation.
         * Server returns integer count of set bits from []byte bin starting at bitOffset for bitSize.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 20
         *	bitSize = 4
         *	returns 2
         */
        public static function count(string $bin_name, int $bit_offset, int $bit_size, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitLScanOp creates bit "left scan" operation.
         * Server returns integer bit offset of the first specified value bit in []byte bin
         * starting at bitOffset for bitSize.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 24
         *	bitSize = 8
         *	value = true
         *	returns 5
         */
        public static function lscan(string $bin_name, int $bit_offset, int $bit_size, bool $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitRScanOp creates bit "right scan" operation.
         * Server returns integer bit offset of the last specified value bit in []byte bin
         * starting at bitOffset for bitSize.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 32
         *	bitSize = 8
         *	value = true
         *	returns 7
         */
        public static function rscan(string $bin_name, int $bit_offset, int $bit_size, bool $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * BitGetIntOp creates bit "get integer" operation.
         * Server returns integer from []byte bin starting at bitOffset for bitSize.
         * Signed indicates if bits should be treated as a signed number.
         * Example:
         *
         *	bin = [0b00000001, 0b01000010, 0b00000011, 0b00000100, 0b00000101]
         *	bitOffset = 8
         *	bitSize = 16
         *	signed = false
         *	returns 16899
         */
        public static function getInt(string $bin_name, int $bit_offset, int $bit_size, bool $signed, ?array $ctx): \Aerospike\Operation {}
    }

    /**
     * IndexCollectionType is the secondary index collection type.
     */
    class IndexCollectionType {
        /**
         * ICT_DEFAULT is the Normal scalar index.
         */
        public static function Default(): \Aerospike\IndexCollectionType {}

        /**
         * ICT_LIST is Index list elements.
         */
        public static function List(): \Aerospike\IndexCollectionType {}

        /**
         * ICT_MAPKEYS is Index map keys.
         */
        public static function MapKeys(): \Aerospike\IndexCollectionType {}

        /**
         * ICT_MAPVALUES is Index map values.
         */
        public static function MapValues(): \Aerospike\IndexCollectionType {}
    }

    /**
     * HyperLogLog (HLL) operations.
     * Requires server versions >= 4.9.
     *
     * HyperLogLog operations on HLL items nested in lists/maps are not currently
     * supported by the server.
     */
    class HllOp {
        /**
         * HLLInitOp creates HLL init operation with minhash bits.
         * Server creates a new HLL or resets an existing HLL.
         * Server does not return a value.
         *
         * policy			write policy, use DefaultHLLPolicy for default
         * binName			name of bin
         * indexBitCount	number of index bits. Must be between 4 and 16 inclusive. Pass -1 for default.
         * minHashBitCount  number of min hash bits. Must be between 4 and 58 inclusive. Pass -1 for default.
         * indexBitCount + minHashBitCount must be <= 64.
         */
        public static function init(\Aerospike\HllPolicy $policy, string $bin_name, int $index_bit_count, int $min_hash_bit_count): \Aerospike\Operation {}

        /**
         * HLLAddOp creates HLL add operation with minhash bits.
         * Server adds values to HLL set. If HLL bin does not exist, use indexBitCount and minHashBitCount
         * to create HLL bin. Server returns number of entries that caused HLL to update a register.
         *
         * policy			write policy, use DefaultHLLPolicy for default
         * binName			name of bin
         * list				list of values to be added
         * indexBitCount	number of index bits. Must be between 4 and 16 inclusive. Pass -1 for default.
         * minHashBitCount  number of min hash bits. Must be between 4 and 58 inclusive. Pass -1 for default.
         * indexBitCount + minHashBitCount must be <= 64.
         */
        public static function add(\Aerospike\HllPolicy $policy, string $bin_name, array $list, int $index_bit_count, int $min_hash_bit_count): \Aerospike\Operation {}

        /**
         * HLLSetUnionOp creates HLL set union operation.
         * Server sets union of specified HLL objects with HLL bin.
         * Server does not return a value.
         *
         * policy			write policy, use DefaultHLLPolicy for default
         * binName			name of bin
         * list				list of HLL objects
         */
        public static function setUnion(\Aerospike\HllPolicy $policy, string $bin_name, array $list): ?\Aerospike\Operation {}

        /**
         * HLLRefreshCountOp creates HLL refresh operation.
         * Server updates the cached count (if stale) and returns the count.
         *
         * binName			name of bin
         */
        public static function refreshCount(string $bin_name): ?\Aerospike\Operation {}

        /**
         * HLLFoldOp creates HLL fold operation.
         * Servers folds indexBitCount to the specified value.
         * This can only be applied when minHashBitCount on the HLL bin is 0.
         * Server does not return a value.
         *
         * binName			name of bin
         * indexBitCount		number of index bits. Must be between 4 and 16 inclusive.
         */
        public static function fold(string $bin_name, int $index_bit_count): ?\Aerospike\Operation {}

        /**
         * HLLGetCountOp creates HLL getCount operation.
         * Server returns estimated number of elements in the HLL bin.
         *
         * binName			name of bin
         */
        public static function getCount(string $bin_name): ?\Aerospike\Operation {}

        /**
         * HLLGetUnionOp creates HLL getUnion operation.
         * Server returns an HLL object that is the union of all specified HLL objects in the list
         * with the HLL bin.
         *
         * binName			name of bin
         * list				list of HLL objects
         */
        public static function getUnion(string $bin_name, array $list): ?\Aerospike\Operation {}

        /**
         * HLLGetUnionCountOp creates HLL getUnionCount operation.
         * Server returns estimated number of elements that would be contained by the union of these
         * HLL objects.
         *
         * binName			name of bin
         * list				list of HLL objects
         */
        public static function getUnionCount(string $bin_name, array $list): ?\Aerospike\Operation {}

        /**
         * HLLGetIntersectCountOp creates HLL getIntersectCount operation.
         * Server returns estimated number of elements that would be contained by the intersection of
         * these HLL objects.
         *
         * binName			name of bin
         * list				list of HLL objects
         */
        public static function getIntersectCount(string $bin_name, array $list): ?\Aerospike\Operation {}

        /**
         * HLLGetSimilarityOp creates HLL getSimilarity operation.
         * Server returns estimated similarity of these HLL objects. Return type is a double.
         *
         * binName			name of bin
         * list				list of HLL objects
         */
        public static function getSimilarity(string $bin_name, array $list): ?\Aerospike\Operation {}

        /**
         * HLLDescribeOp creates HLL describe operation.
         * Server returns indexBitCount and minHashBitCount used to create HLL bin in a list of longs.
         * The list size is 2.
         *
         * binName			name of bin
         */
        public static function describe(string $bin_name): \Aerospike\Operation {}
    }

    /**
     * Represents an exception specific to the Aerospike database operations.
     */
    class AerospikeException{
        public $message;

        public $in_doubt;

        public $code;
    }

    /**
     * Virtual collection of records retrieved through queries and scans. During a query/scan,
     * multiple threads will retrieve records from the server nodes and put these records on an
     * internal queue managed by the recordset. The single user thread consumes these records from the
     * queue.
     */
    class PartitionFilter {
        public $partition_status;

        public function __construct() {}

        public function getPartitionStatus(): array {}

        /**
         * NewPartitionFilterAll creates a partition filter that
         * reads all the partitions.
         */
        public static function all(): \Aerospike\PartitionFilter {}

        /**
         * NewPartitionFilterById creates a partition filter by partition id.
         * Partition id is between 0 - 4095
         */
        public static function partition(int $id): \Aerospike\PartitionFilter {}

        /**
         * NewPartitionFilterByRange creates a partition filter by partition range.
         * begin partition id is between 0 - 4095
         * count is the number of partitions, in the range of 1 - 4096 inclusive.
         */
        public static function range(int $begin, int $count): \Aerospike\PartitionFilter {}

        public function initPartitionStatus() {}
    }

    /**
     * OperationType determines operation type
     */
    class Operation {
        /**
         * read bin database operation.
         */
        public static function get(?string $bin_name): \Aerospike\Operation {}

        /**
         * read record header database operation.
         */
        public static function getHeader(): \Aerospike\Operation {}

        /**
         * set database operation.
         */
        public static function put(\Aerospike\Bin $bin): \Aerospike\Operation {}

        /**
         * string append database operation.
         */
        public static function append(\Aerospike\Bin $bin): \Aerospike\Operation {}

        /**
         * string prepend database operation.
         */
        public static function prepend(\Aerospike\Bin $bin): \Aerospike\Operation {}

        /**
         * integer add database operation.
         */
        public static function add(\Aerospike\Bin $bin): \Aerospike\Operation {}

        /**
         * touch record database operation.
         */
        public static function touch(): \Aerospike\Operation {}

        /**
         * delete record database operation.
         */
        public static function delete(): \Aerospike\Operation {}
    }

    /**
     * ExpType defines the expression's data type.
     */
    class ExpType {
        /**
         * ExpTypeNIL is NIL Expression Type
         */
        public static function Nil(): \Aerospike\ExpType {}

        /**
         * ExpTypeBOOL is BOOLEAN Expression Type
         */
        public static function Bool(): \Aerospike\ExpType {}

        /**
         * ExpTypeINT is INTEGER Expression Type
         */
        public static function Int(): \Aerospike\ExpType {}

        /**
         * ExpTypeSTRING is STRING Expression Type
         */
        public static function String(): \Aerospike\ExpType {}

        /**
         * ExpTypeLIST is LIST Expression Type
         */
        public static function List(): \Aerospike\ExpType {}

        /**
         * ExpTypeMAP is MAP Expression Type
         */
        public static function Map(): \Aerospike\ExpType {}

        /**
         * ExpTypeBLOB is BLOB Expression Type
         */
        public static function Blob(): \Aerospike\ExpType {}

        /**
         * ExpTypeFLOAT is FLOAT Expression Type
         */
        public static function Float(): \Aerospike\ExpType {}

        /**
         * ExpTypeGEO is GEO String Expression Type
         */
        public static function Geo(): \Aerospike\ExpType {}

        /**
         * ExpTypeHLL is HLL Expression Type
         */
        public static function Hll(): \Aerospike\ExpType {}
    }

    /**
     * Key is the unique record identifier. Records can be identified using a specified namespace,
     * an optional set name, and a user defined key which must be unique within a set.
     * Records can also be identified by namespace/digest which is the combination used
     * on the server.
     */
    class Key {
        public $namespace;

        public $setname;

        public $digest;

        public $partition_id;

        public $value;

        public function __construct(string $namespace, string $set, mixed $key) {}

        /**
         * namespace. Equivalent to database name.
         */
        public function getNamespace(): string {}

        /**
         * Optional set name. Equivalent to database table.
         */
        public function getSetname(): string {}

        /**
         * getValue() returns key's value.
         */
        public function getValue(): mixed {}

        /**
         * Generate unique server hash value from set name, key type and user defined key.
         * The hash function is RIPEMD-160 (a 160 bit hash).
         */
        public function computeDigest(): array {}

        /**
         * get_digest_bytes returns key digest as byte array.
         */
        public function getDigestBytes(): array {}

        /**
         * get_digest returns key digest as string.
         */
        public function getDigest(): string {}

        /**
         * PartitionId returns the partition that the key belongs to.
         */
        public function partitionId(): ?int {}
    }

    /**
     * Implementation of the Json (Map<String, Value>) data structure for Aerospike.
     */
    class Json {
        public $value;

        /**
         * getter method to get the json value
         */
        public function getValue(): array {}

        /**
         * setter method to set the json value
         */
        public function setValue(array $v) {}

        /**
         * Returns a string representation of the value.
         */
        public function asString(): string {}
    }

    /**
     * BatchReadPolicy attributes used in batch read commands.
     */
    class BatchReadPolicy {
        public $read_mode_ap;

        public $read_mode_sc;

        public $filter_expression;

        public function __construct() {}

        /**
         * FilterExpression is the optional expression filter. If FilterExpression exists and evaluates to false, the specific batch key
         * request is not performed and BatchRecord.ResultCode is set to types.FILTERED_OUT.
         *
         * Default: null
         */
        public function getFilterExpression(): ?\Aerospike\Expression {}

        public function setFilterExpression(mixed $filter_expression) {}

        /**
         * ReadModeAP indicates read policy for AP (availability) namespaces.
         */
        public function getReadModeAp(): \Aerospike\ReadModeAP {}

        public function setReadModeAp(mixed $read_mode_ap) {}

        /**
         * ReadModeSC indicates read policy for SC (strong consistency) namespaces.
         */
        public function getReadModeSc(): \Aerospike\ReadModeSC {}

        public function setReadModeSc(mixed $read_mode_sc) {}
    }

    /**
     * List operations support negative indexing.  If the index is negative, the
     * resolved index starts backwards from end of list. If an index is out of bounds,
     * a parameter error will be returned. If a range is partially out of bounds, the
     * valid part of the range will be returned. Index/Range examples:
     *
     * Index/Range examples:
     *
     *    Index 0: First item in list.
     *    Index 4: Fifth item in list.
     *    Index -1: Last item in list.
     *    Index -3: Third to last item in list.
     *    Index 1 Count 2: Second and third items in list.
     *    Index -3 Count 3: Last three items in list.
     *    Index -5 Count 4: Range between fifth to last item to second to last item inclusive.
     *
     */
    class ListOp {
        /**
         * ListCreateOp creates list create operation.
         * Server creates list at given context level. The context is allowed to be beyond list
         * boundaries only if pad is set to true.  In that case, nil list entries will be inserted to
         * satisfy the context position.
         */
        public static function create(string $bin_name, mixed $order, bool $pad, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListSetOrderOp creates a set list order operation.
         * Server sets list order.  Server returns nil.
         */
        public static function setOrder(string $bin_name, mixed $order, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListAppendOp creates a list append operation.
         * Server appends values to end of list bin.
         * Server returns list size on bin name.
         * It will panic is no values have been passed.
         */
        public static function append(\Aerospike\ListPolicy $policy, string $bin_name, array $values, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListInsertOp creates a list insert operation.
         * Server inserts value to specified index of list bin.
         * Server returns list size on bin name.
         * It will panic is no values have been passed.
         */
        public static function insert(\Aerospike\ListPolicy $policy, string $bin_name, int $index, array $values, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListPopOp creates list pop operation.
         * Server returns item at specified index and removes item from list bin.
         */
        public static function pop(string $bin_name, int $index, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListPopRangeOp creates a list pop range operation.
         * Server returns items starting at specified index and removes items from list bin.
         */
        public static function popRange(string $bin_name, int $index, int $count, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListPopRangeFromOp creates a list pop range operation.
         * Server returns items starting at specified index to the end of list and removes items from list bin.
         */
        public static function popRangeFrom(string $bin_name, int $index, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByValueOp creates list remove by value operation.
         * Server removes the item identified by value and returns removed data specified by returnType.
         */
        public static function removeValues(string $bin_name, array $values, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByValueRangeOp creates a list remove operation.
         * Server removes list items identified by value range (valueBegin inclusive, valueEnd exclusive).
         * If valueBegin is nil, the range is less than valueEnd.
         * If valueEnd is nil, the range is greater than equal to valueBegin.
         * Server returns removed data specified by returnType
         */
        public static function removeByValueRange(string $bin_name, mixed $begin, mixed $end, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByValueRelativeRankRangeOp creates a list remove by value relative to rank range operation.
         * Server removes list items nearest to value and greater by relative rank.
         * Server returns removed data specified by returnType.
         *
         * Examples for ordered list [0,4,5,9,11,15]:
         *
         *	(value,rank) = [removed items]
         *	(5,0) = [5,9,11,15]
         *	(5,1) = [9,11,15]
         *	(5,-1) = [4,5,9,11,15]
         *	(3,0) = [4,5,9,11,15]
         *	(3,3) = [11,15]
         *	(3,-3) = [0,4,5,9,11,15]
         */
        public static function removeByValueRelativeRankRange(string $bin_name, mixed $value, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByValueRelativeRankRangeCountOp creates a list remove by value relative to rank range operation.
         * Server removes list items nearest to value and greater by relative rank with a count limit.
         * Server returns removed data specified by returnType.
         * Examples for ordered list [0,4,5,9,11,15]:
         *
         *	(value,rank,count) = [removed items]
         *	(5,0,2) = [5,9]
         *	(5,1,1) = [9]
         *	(5,-1,2) = [4,5]
         *	(3,0,1) = [4]
         *	(3,3,7) = [11,15]
         *	(3,-3,2) = []
         */
        public static function removeByValueRelativeRankRangeCount(string $bin_name, mixed $value, int $rank, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveRangeOp creates a list remove range operation.
         * Server removes "count" items starting at specified index from list bin.
         * Server returns number of items removed.
         */
        public static function removeRange(string $bin_name, int $index, int $count, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveRangeFromOp creates a list remove range operation.
         * Server removes all items starting at specified index to the end of list.
         * Server returns number of items removed.
         */
        public static function removeRangeFrom(string $bin_name, int $index, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListSetOp creates a list set operation.
         * Server sets item value at specified index in list bin.
         * Server does not return a result by default.
         */
        public static function set(string $bin_name, int $index, mixed $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListTrimOp creates a list trim operation.
         * Server removes items in list bin that do not fall into range specified by index
         * and count range. If the range is out of bounds, then all items will be removed.
         * Server returns number of elements that were removed.
         */
        public static function trim(string $bin_name, int $index, int $count, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListClearOp creates a list clear operation.
         * Server removes all items in list bin.
         * Server does not return a result by default.
         */
        public static function clear(string $bin_name, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListIncrementOp creates a list increment operation.
         * Server increments list[index] by value.
         * Value should be integer(IntegerValue, LongValue) or float(FloatValue).
         * Server returns list[index] after incrementing.
         */
        public static function increment(string $bin_name, int $index, mixed $value, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListSizeOp creates a list size operation.
         * Server returns size of list on bin name.
         */
        public static function size(string $bin_name, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListSortOp creates list sort operation.
         * Server sorts list according to sortFlags.
         * Server does not return a result by default.
         */
        public static function sort(string $bin_name, \Aerospike\ListSortFlags $sort_flags, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByIndexOp creates a list remove operation.
         * Server removes list item identified by index and returns removed data specified by returnType.
         */
        public static function removeByIndex(string $bin_name, int $index, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByIndexRangeOp creates a list remove operation.
         * Server removes list items starting at specified index to the end of list and returns removed
         * data specified by returnType.
         */
        public static function removeByIndexRange(string $bin_name, int $index, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByIndexRangeCountOp creates a list remove operation.
         * Server removes "count" list items starting at specified index and returns removed data specified by returnType.
         */
        public static function removeByIndexRangeCount(string $bin_name, int $index, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByRankOp creates a list remove operation.
         * Server removes list item identified by rank and returns removed data specified by returnType.
         */
        public static function removeByRank(string $bin_name, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByRankRangeOp creates a list remove operation.
         * Server removes list items starting at specified rank to the last ranked item and returns removed
         * data specified by returnType.
         */
        public static function removeByRankRange(string $bin_name, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListRemoveByRankRangeCountOp creates a list remove operation.
         * Server removes "count" list items starting at specified rank and returns removed data specified by returnType.
         */
        public static function removeByRankRangeCount(string $bin_name, int $rank, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByValueOp creates a list get by value operation.
         * Server selects list items identified by value and returns selected data specified by returnType.
         */
        public static function getByValues(string $bin_name, array $values, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByValueRangeOp creates a list get by value range operation.
         * Server selects list items identified by value range (valueBegin inclusive, valueEnd exclusive)
         * If valueBegin is nil, the range is less than valueEnd.
         * If valueEnd is nil, the range is greater than equal to valueBegin.
         * Server returns selected data specified by returnType.
         */
        public static function getByValueRange(string $bin_name, mixed $begin, mixed $end, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByIndexOp creates list get by index operation.
         * Server selects list item identified by index and returns selected data specified by returnType
         */
        public static function getByIndex(string $bin_name, int $index, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByIndexRangeOp creates list get by index range operation.
         * Server selects list items starting at specified index to the end of list and returns selected
         * data specified by returnType.
         */
        public static function getByIndexRange(string $bin_name, int $index, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByIndexRangeCountOp creates list get by index range operation.
         * Server selects "count" list items starting at specified index and returns selected data specified
         * by returnType.
         */
        public static function getByIndexRangeCount(string $bin_name, int $index, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByRankOp creates a list get by rank operation.
         * Server selects list item identified by rank and returns selected data specified by returnType.
         */
        public static function getByRank(string $bin_name, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByRankRangeOp creates a list get by rank range operation.
         * Server selects list items starting at specified rank to the last ranked item and returns selected
         * data specified by returnType
         */
        public static function getByRankRange(string $bin_name, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByRankRangeCountOp creates a list get by rank range operation.
         * Server selects "count" list items starting at specified rank and returns selected data specified by returnType.
         */
        public static function getByRankRangeCount(string $bin_name, int $rank, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByValueRelativeRankRangeOp creates a list get by value relative to rank range operation.
         * Server selects list items nearest to value and greater by relative rank.
         * Server returns selected data specified by returnType.
         *
         * Examples for ordered list [0,4,5,9,11,15]:
         *
         *	(value,rank) = [selected items]
         *	(5,0) = [5,9,11,15]
         *	(5,1) = [9,11,15]
         *	(5,-1) = [4,5,9,11,15]
         *	(3,0) = [4,5,9,11,15]
         *	(3,3) = [11,15]
         *	(3,-3) = [0,4,5,9,11,15]
         */
        public static function getByValueRelativeRankRange(string $bin_name, mixed $value, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * ListGetByValueRelativeRankRangeCountOp creates a list get by value relative to rank range operation.
         * Server selects list items nearest to value and greater by relative rank with a count limit.
         * Server returns selected data specified by returnType.
         *
         * Examples for ordered list [0,4,5,9,11,15]:
         *
         *	(value,rank,count) = [selected items]
         *	(5,0,2) = [5,9]
         *	(5,1,1) = [9]
         *	(5,-1,2) = [4,5]
         *	(3,0,1) = [4]
         *	(3,3,7) = [11,15]
         *	(3,-3,2) = []
         */
        public static function getByValueRelativeRankRangeCount(string $bin_name, mixed $value, int $rank, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}
    }

    /**
     * Record expiration, also known as time-to-live (TTL).
     */
    class Expiration {
        /**
         * Set the record to expire X seconds from now
         */
        public static function Seconds(int $seconds): \Aerospike\Expiration {}

        /**
         * Set the record's expiry time using the default time-to-live (TTL) value for the namespace
         */
        public static function NamespaceDefault(): \Aerospike\Expiration {}

        /**
         * Set the record to never expire. Requires Aerospike 2 server version 2.7.2 or later or
         * Aerospike 3 server version 3.1.4 or later. Do not use with older servers.
         */
        public static function Never(): \Aerospike\Expiration {}

        /**
         * Do not change the record's expiry time when updating the record; requires Aerospike server
         * version 3.10.1 or later.
         */
        public static function DontUpdate(): \Aerospike\Expiration {}
    }

    /**
     * Unique key map bin operations. Create map operations used by the client operate command.
     * The default unique key map is unordered.
     *
     * All maps maintain an index and a rank.  The index is the item offset from the start of the map,
     * for both unordered and ordered maps.  The rank is the sorted index of the value component.
     * Map supports negative indexing for index and rank.
     *
     * Index examples:
     *
     *  Index 0: First item in map.
     *  Index 4: Fifth item in map.
     *  Index -1: Last item in map.
     *  Index -3: Third to last item in map.
     *  Index 1 Count 2: Second and third items in map.
     *  Index -3 Count 3: Last three items in map.
     *  Index -5 Count 4: Range between fifth to last item to second to last item inclusive.
     *
     *
     * Rank examples:
     *
     *  Rank 0: Item with lowest value rank in map.
     *  Rank 4: Fifth lowest ranked item in map.
     *  Rank -1: Item with highest ranked value in map.
     *  Rank -3: Item with third highest ranked value in map.
     *  Rank 1 Count 2: Second and third lowest ranked items in map.
     *  Rank -3 Count 3: Top three ranked items in map.
     *
     *
     * Nested CDT operations are supported by optional CTX context arguments.  Examples:
     *
     *  bin = {key1:{key11:9,key12:4}, key2:{key21:3,key22:5}}
     *  Set map value to 11 for map key "key21" inside of map key "key2".
     *  MapOperation.put(MapPolicy.Default, "bin", StringValue("key21"), IntegerValue(11), CtxMapKey(StringValue("key2")))
     *  bin result = {key1:{key11:9,key12:4},key2:{key21:11,key22:5}}
     *
     *  bin : {key1:{key11:{key111:1},key12:{key121:5}}, key2:{key21:{"key211":7}}}
     *  Set map value to 11 in map key "key121" for highest ranked map ("key12") inside of map key "key1".
     *  MapPutOp(DefaultMapPolicy(), "bin", StringValue("key121"), IntegerValue(11), CtxMapKey(StringValue("key1")), CtxMapRank(-1))
     *  bin result = {key1:{key11:{key111:1},key12:{key121:11}}, key2:{key21:{"key211":7}}}
     */
    class MapOp {
        /**
         * MapCreateOp creates a map create operation.
         * Server creates map at given context level.
         */
        public static function create(string $bin_name, \Aerospike\MapOrderType $order, ?bool $with_index, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapSetPolicyOp creates set map policy operation.
         * Server sets map policy attributes.  Server returns nil.
         *
         * The required map policy attributes can be changed after the map is created.
         */
        public static function setPolicy(\Aerospike\MapPolicy $policy, string $bin_name, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapSizeOp creates map size operation.
         * Server returns size of map.
         */
        public static function size(string $bin_name, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapPutOp creates map put operation.
         * Server writes key/value item to map bin and returns map size.
         *
         * The required map policy dictates the type of map to create when it does not exist.
         * The map policy also specifies the mode used when writing items to the map.
         */
        public static function put(\Aerospike\MapPolicy $policy, string $bin_name, mixed $map, ?array $ctx): ?\Aerospike\Operation {}

        /**
         * MapIncrementOp creates map increment operation.
         * Server increments values by incr for all items identified by key and returns final result.
         * Valid only for numbers.
         *
         * The required map policy dictates the type of map to create when it does not exist.
         * The map policy also specifies the mode used when writing items to the map.
         */
        public static function increment(\Aerospike\MapPolicy $policy, string $bin_name, mixed $key, mixed $incr, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapDecrementOp creates map decrement operation.
         * Server decrements values by decr for all items identified by key and returns final result.
         * Valid only for numbers.
         *
         * The required map policy dictates the type of map to create when it does not exist.
         * The map policy also specifies the mode used when writing items to the map.
         */
        public static function decrement(\Aerospike\MapPolicy $policy, string $bin_name, mixed $key, mixed $decr, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapClearOp creates map clear operation.
         * Server removes all items in map.  Server returns nil.
         */
        public static function clear(string $bin_name, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByKeyOp creates map remove operation.
         * Server removes map item identified by key and returns removed data specified by returnType.
         */
        public static function removeByKeys(string $bin_name, array $keys, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByKeyRangeOp creates map remove operation.
         * Server removes map items identified by key range (keyBegin inclusive, keyEnd exclusive).
         * If keyBegin is nil, the range is less than keyEnd.
         * If keyEnd is nil, the range is greater than equal to keyBegin.
         *
         * Server returns removed data specified by returnType.
         */
        public static function removeByKeyRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $begin, mixed $end, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByValueOp creates map remove operation.
         * Server removes map items identified by value and returns removed data specified by returnType.
         */
        public static function removeByValues(\Aerospike\MapPolicy $policy, string $bin_name, array $values, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByValueListOp creates map remove operation.
         * Server removes map items identified by values and returns removed data specified by returnType.
         */
        public static function removeByValueRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $begin, mixed $end, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByValueRelativeRankRangeOp creates a map remove by value relative to rank range operation.
         * Server removes map items nearest to value and greater by relative rank.
         * Server returns removed data specified by returnType.
         *
         * Examples for map [{4=2},{9=10},{5=15},{0=17}]:
         *
         *	(value,rank) = [removed items]
         *	(11,1) = [{0=17}]
         *	(11,-1) = [{9=10},{5=15},{0=17}]
         */
        public static function removeByValueRelativeRankRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $value, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByValueRelativeRankRangeCountOp creates a map remove by value relative to rank range operation.
         * Server removes map items nearest to value and greater by relative rank with a count limit.
         * Server returns removed data specified by returnType (See MapReturnType).
         *
         * Examples for map [{4=2},{9=10},{5=15},{0=17}]:
         *
         *	(value,rank,count) = [removed items]
         *	(11,1,1) = [{0=17}]
         *	(11,-1,1) = [{9=10}]
         */
        public static function removeByValueRelativeRankRangeCount(\Aerospike\MapPolicy $policy, string $bin_name, mixed $value, int $rank, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByIndexOp creates map remove operation.
         * Server removes map item identified by index and returns removed data specified by returnType.
         */
        public static function removeByIndex(\Aerospike\MapPolicy $policy, string $bin_name, int $index, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByIndexRangeOp creates map remove operation.
         * Server removes map items starting at specified index to the end of map and returns removed
         * data specified by returnTyp
         */
        public static function removeByIndexRange(\Aerospike\MapPolicy $policy, string $bin_name, int $index, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByIndexRangeCountOp creates map remove operation.
         * Server removes "count" map items starting at specified index and returns removed data specified by returnType.
         */
        public static function removeByIndexRangeCount(\Aerospike\MapPolicy $policy, string $bin_name, int $index, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByRankOp creates map remove operation.
         * Server removes map item identified by rank and returns removed data specified by returnType.
         */
        public static function removeByRank(\Aerospike\MapPolicy $policy, string $bin_name, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByRankRangeOp creates map remove operation.
         * Server removes map items starting at specified rank to the last ranked item and returns removed
         * data specified by returnType.
         */
        public static function removeByRankRange(\Aerospike\MapPolicy $policy, string $bin_name, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByRankRangeCountOp creates map remove operation.
         * Server removes "count" map items starting at specified rank and returns removed data specified by returnType.
         */
        public static function removeByRankRangeCount(\Aerospike\MapPolicy $policy, string $bin_name, int $rank, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByKeyRelativeIndexRangeOp creates a map remove by key relative to index range operation.
         * Server removes map items nearest to key and greater by index.
         * Server returns removed data specified by returnType.
         *
         * Examples for map [{0=17},{4=2},{5=15},{9=10}]:
         *
         *	(value,index) = [removed items]
         *	(5,0) = [{5=15},{9=10}]
         *	(5,1) = [{9=10}]
         *	(5,-1) = [{4=2},{5=15},{9=10}]
         *	(3,2) = [{9=10}]
         *	(3,-2) = [{0=17},{4=2},{5=15},{9=10}]
         */
        public static function removeByKeyRelativeIndexRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $key, int $index, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapRemoveByKeyRelativeIndexRangeCountOp creates map remove by key relative to index range operation.
         * Server removes map items nearest to key and greater by index with a count limit.
         * Server returns removed data specified by returnType.
         *
         * Examples for map [{0=17},{4=2},{5=15},{9=10}]:
         *
         *	(value,index,count) = [removed items]
         *	(5,0,1) = [{5=15}]
         *	(5,1,2) = [{9=10}]
         *	(5,-1,1) = [{4=2}]
         *	(3,2,1) = [{9=10}]
         *	(3,-2,2) = [{0=17}]
         */
        public static function removeByKeyRelativeIndexRangeCount(\Aerospike\MapPolicy $policy, string $bin_name, mixed $key, int $index, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByKeyOp creates map get by key operation.
         * Server selects map item identified by key and returns selected data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByKeys(\Aerospike\MapPolicy $policy, string $bin_name, array $keys, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByKeyRangeOp creates map get by key range operation.
         * Server selects map items identified by key range (keyBegin inclusive, keyEnd exclusive).
         * If keyBegin is nil, the range is less than keyEnd.
         * If keyEnd is nil, the range is greater than equal to keyBegin.
         *
         * Server returns selected data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByKeyRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $begin, mixed $end, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByKeyRelativeIndexRangeOp creates a map get by key relative to index range operation.
         * Server selects map items nearest to key and greater by index.
         * Server returns selected data specified by returnType.
         *
         * Examples for ordered map [{0=17},{4=2},{5=15},{9=10}]:
         *
         *	(value,index) = [selected items]
         *	(5,0) = [{5=15},{9=10}]
         *	(5,1) = [{9=10}]
         *	(5,-1) = [{4=2},{5=15},{9=10}]
         *	(3,2) = [{9=10}]
         *	(3,-2) = [{0=17},{4=2},{5=15},{9=10}]
         * Should be used with BatchRead.
         */
        public static function getByKeyRelativeIndexRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $key, int $index, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByKeyRelativeIndexRangeCountOp creates a map get by key relative to index range operation.
         * Server selects map items nearest to key and greater by index with a count limit.
         * Server returns selected data specified by returnType (See MapReturnType).
         *
         * Examples for ordered map [{0=17},{4=2},{5=15},{9=10}]:
         *
         *	(value,index,count) = [selected items]
         *	(5,0,1) = [{5=15}]
         *	(5,1,2) = [{9=10}]
         *	(5,-1,1) = [{4=2}]
         *	(3,2,1) = [{9=10}]
         *	(3,-2,2) = [{0=17}]
         * Should be used with BatchRead.
         */
        public static function getByKeyRelativeIndexRangeCount(\Aerospike\MapPolicy $policy, string $bin_name, mixed $key, int $index, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByKeyListOp creates a map get by key list operation.
         * Server selects map items identified by keys and returns selected data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByValues(\Aerospike\MapPolicy $policy, string $bin_name, array $values, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByValueRangeOp creates map get by value range operation.
         * Server selects map items identified by value range (valueBegin inclusive, valueEnd exclusive)
         * If valueBegin is nil, the range is less than valueEnd.
         * If valueEnd is nil, the range is greater than equal to valueBegin.
         *
         * Server returns selected data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByValueRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $begin, mixed $end, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByValueRelativeRankRangeOp creates a map get by value relative to rank range operation.
         * Server selects map items nearest to value and greater by relative rank.
         * Server returns selected data specified by returnType.
         *
         * Examples for map [{4=2},{9=10},{5=15},{0=17}]:
         *
         *	(value,rank) = [selected items]
         *	(11,1) = [{0=17}]
         *	(11,-1) = [{9=10},{5=15},{0=17}]
         * Should be used with BatchRead.
         */
        public static function getByValueRelativeRankRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $value, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByValueRelativeRankRangeCountOp creates a map get by value relative to rank range operation.
         * Server selects map items nearest to value and greater by relative rank with a count limit.
         * Server returns selected data specified by returnType.
         *
         * Examples for map [{4=2},{9=10},{5=15},{0=17}]:
         *
         *	(value,rank,count) = [selected items]
         *	(11,1,1) = [{0=17}]
         *	(11,-1,1) = [{9=10}]
         * Should be used with BatchRead.
         */
        public static function getByValueRelativeRankRangeCount(\Aerospike\MapPolicy $policy, string $bin_name, mixed $value, int $rank, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByIndexOp creates map get by index operation.
         * Server selects map item identified by index and returns selected data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByIndex(\Aerospike\MapPolicy $policy, string $bin_name, int $index, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByIndexRangeOp creates map get by index range operation.
         * Server selects map items starting at specified index to the end of map and returns selected
         * data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByIndexRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $begin, mixed $end, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByIndexRangeCountOp creates map get by index range operation.
         * Server selects "count" map items starting at specified index and returns selected data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByIndexRangeCount(\Aerospike\MapPolicy $policy, string $bin_name, int $index, int $rank, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByRankOp creates map get by rank operation.
         * Server selects map item identified by rank and returns selected data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByRank(\Aerospike\MapPolicy $policy, string $bin_name, int $rank, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByRankRangeOp creates map get by rank range operation.
         * Server selects map items starting at specified rank to the last ranked item and returns selected
         * data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByRankRange(\Aerospike\MapPolicy $policy, string $bin_name, mixed $begin, mixed $end, mixed $return_type, ?array $ctx): \Aerospike\Operation {}

        /**
         * MapGetByRankRangeCountOp creates map get by rank range operation.
         * Server selects "count" map items starting at specified rank and returns selected data specified by returnType.
         * Should be used with BatchRead.
         */
        public static function getByRankRangeCount(\Aerospike\MapPolicy $policy, string $bin_name, int $rank, int $range, int $count, mixed $return_type, ?array $ctx): \Aerospike\Operation {}
    }

    /**
     * Represents a infinity value for Aerospike.
     */
    class Infinity {
    }

    /**
     *
     *  BatchRecord
     *
     * BatchRecord encasulates the Batch key and record result.
     */
    class BatchRecord {
        public $key;

        public $record;

        /**
         * Key.
         */
        public function getKey(): ?\Aerospike\Key {}

        /**
         * Record result after batch command has completed.  Will be nil if record was not found
         * or an error occurred. See ResultCode.
         */
        public function getRecord(): ?\Aerospike\Record {}
    }

    /**
     * Virtual collection of records retrieved through queries and scans. During a query/scan,
     * multiple threads will retrieve records from the server nodes and put these records on an
     * internal queue managed by the recordset. The single user thread consumes these records from the
     * queue.
     */
    class Recordset {
        public $active;

        /**
         * Drop the stream, which will signal the server and close the recordset
         */
        public function close() {}

        /**
         * IsActive returns true if the operation hasn't been finished or cancelled.
         */
        public function getActive(): bool {}

        /**
         * Records is a channel on which the resulting records will be sent back.
         */
        public function next(): ?\Aerospike\Record {}
    }

    /**
     * `UdfLanguage` determines how to handle record writes based on record generation.
     */
    class UdfLanguage {
        /**
         * lua language.
         */
        public static function Lua(): \Aerospike\UdfLanguage {}
    }

    /**
     * BitWriteFlags specify bitwise operation policy write flags.
     */
    class BitwiseWriteFlags {
        /**
         * BitWriteFlagsDefault allows create or update.
         */
        public static function Default(): \Aerospike\BitwiseWriteFlags {}

        /**
         * BitWriteFlagsCreateOnly specifies that:
         * If the bin already exists, the operation will be denied.
         * If the bin does not exist, a new bin will be created.
         */
        public static function CreateOnly(): \Aerospike\BitwiseWriteFlags {}

        /**
         * BitWriteFlagsUpdateOnly specifies that:
         * If the bin already exists, the bin will be overwritten.
         * If the bin does not exist, the operation will be denied.
         */
        public static function UpdateOnly(): \Aerospike\BitwiseWriteFlags {}

        /**
         * BitWriteFlagsNoFail specifies not to raise error if operation is denied.
         */
        public static function NoFail(): \Aerospike\BitwiseWriteFlags {}

        /**
         * BitWriteFlagsPartial allows other valid operations to be committed if this operations is
         * denied due to flag constraints.
         */
        public static function Partial(): \Aerospike\BitwiseWriteFlags {}
    }

    /**
     * `InfoPolicy` encapsulates parameters for all info operations.
     */
    class InfoPolicy {
        public function __construct() {}
    }

    /**
     * Represents a wildcard value for Aerospike.
     */
    class Wildcard {
    }

    /**
     * UserRoles contains information about a user.
     */
    class UserRole {
        public $conns_in_use;

        public $read_info;

        public $write_info;

        public $user;

        public $roles;

        /**
         * User name.
         */
        public function getUser(): string {}

        /**
         * Roles is a list of assigned roles.
         */
        public function getRoles(): array {}

        /**
         * ReadInfo is the list of read statistics. List may be nil.
         * Current statistics by offset are:
         *
         * 0: read quota in records per second
         * 1: single record read transaction rate (TPS)
         * 2: read scan/query record per second rate (RPS)
         * 3: number of limitless read scans/queries
         *
         * Future server releases may add additional statistics.
         */
        public function getReadInfo(): array {}

        /**
         * WriteInfo is the list of write statistics. List may be nil.
         * Current statistics by offset are:
         *
         * 0: write quota in records per second
         * 1: single record write transaction rate (TPS)
         * 2: write scan/query record per second rate (RPS)
         * 3: number of limitless write scans/queries
         *
         * Future server releases may add additional statistics.
         */
        public function getWriteInfo(): array {}

        /**
         * ConnsInUse is the number of currently open connections for the user
         */
        public function getConnsInUse(): int {}
    }

    /**
     * BatchWritePolicy attributes used in batch write commands.
     */
    class BatchWritePolicy {
        public $generation_policy;

        public $filter_expression;

        public $generation;

        public $expiration;

        public $durable_delete;

        public $commit_level;

        public $record_exists_action;

        public $send_key;

        public function __construct() {}

        /**
         * FilterExpression is optional expression filter. If FilterExpression exists and evaluates to false, the specific batch key
         * request is not performed and BatchRecord#resultCode is set to types.FILTERED_OUT.
         *
         * Default: nil
         */
        public function getFilterExpression(): ?\Aerospike\Expression {}

        public function setFilterExpression(mixed $filter_expression) {}

        /**
         * RecordExistsAction qualifies how to handle writes where the record already exists.
         */
        public function getRecordExistsAction(): \Aerospike\RecordExistsAction {}

        public function setRecordExistsAction(mixed $record_exists_action) {}

        /**
         * Desired consistency guarantee when committing a transaction on the server. The default
         * (COMMIT_ALL) indicates that the server should wait for master and all replica commits to
         * be successful before returning success to the client.
         *
         * Default: CommitLevel.COMMIT_ALL
         */
        public function getGenerationPolicy(): \Aerospike\GenerationPolicy {}

        public function setGenerationPolicy(mixed $generation_policy) {}

        /**
         * GenerationPolicy qualifies how to handle record writes based on record generation. The default (NONE)
         * indicates that the generation is not used to restrict writes.
         *
         * The server does not support this field for UDF execute() calls. The read-modify-write
         * usage model can still be enforced inside the UDF code itself.
         *
         * Default: GenerationPolicy.NONE
         * indicates that the generation is not used to restrict writes.
         */
        public function getCommitLevel(): \Aerospike\CommitLevel {}

        public function setCommitLevel(mixed $commit_level) {}

        /**
         * Expected generation. Generation is the number of times a record has been modified
         * (including creation) on the server. If a write operation is creating a record,
         * the expected generation would be 0. This field is only relevant when
         * generationPolicy is not NONE.
         *
         * The server does not support this field for UDF execute() calls. The read-modify-write
         * usage model can still be enforced inside the UDF code itself.
         *
         * Default: 0
         */
        public function getGeneration(): int {}

        public function setGeneration(int $generation) {}

        /**
         * Expiration determines record expiration in seconds. Also known as TTL (Time-To-Live).
         * Seconds record will live before being removed by the server.
         * Expiration values:
         * TTLServerDefault (0): Default to namespace configuration variable "default-ttl" on the server.
         * TTLDontExpire (MaxUint32): Never expire for Aerospike 2 server versions >= 2.7.2 and Aerospike 3+ server
         * TTLDontUpdate (MaxUint32 - 1): Do not change ttl when record is written. Supported by Aerospike server versions >= 3.10.1
         * > 0: Actual expiration in seconds.
         */
        public function getExpiration(): \Aerospike\Expiration {}

        public function setExpiration(mixed $expiration) {}

        /**
         * DurableDelete leaves a tombstone for the record if the transaction results in a record deletion.
         * This prevents deleted records from reappearing after node failures.
         * Valid for Aerospike Server Enterprise Edition 3.10+ only.
         */
        public function getDurableDelete(): bool {}

        public function setDurableDelete(bool $durable_delete) {}

        /**
         * SendKey determines to whether send user defined key in addition to hash digest on both reads and writes.
         * If the key is sent on a write, the key will be stored with the record on
         * the server.
         * The default is to not send the user defined key.
         */
        public function getSendKey(): bool {}

        public function setSendKey(bool $send_key) {}
    }

    /**
     *  Value interface is used to efficiently serialize objects into the wire protocol.
     */
    class Value {
        public static function nil(): mixed {}

        public static function int(int $val): mixed {}

        public static function uint(int $val): mixed {}

        public static function float(float $val): mixed {}

        public static function bool(bool $val): mixed {}

        public static function string(string $val): mixed {}

        public static function list(array $val): mixed {}

        public static function map(mixed $val): mixed {}

        public static function blob(array $val): mixed {}

        public static function geoJson(string $val): mixed {}

        public static function hll(array $val): mixed {}

        public static function json(array $val): mixed {}

        public static function infinity(): mixed {}

        public static function wildcard(): mixed {}
    }

    /**
     * Privilege determines user access granularity.
     */
    class Privilege {
        public $namespace;

        public $name;

        public $setname;

        public function getName(): string {}

        public function getNamespace(): string {}

        public function getSetname(): string {}

        /**
         * UserAdmin allows to manages users and their roles.
         */
        public static function userAdmin(): string {}

        /**
         * SysAdmin allows to manage indexes, user defined functions and server configuration.
         */
        public static function sysAdmin(): string {}

        /**
         * DataAdmin allows to manage indicies and user defined functions.
         */
        public static function dataAdmin(): string {}

        /**
         * UDFAdmin allows to manage user defined functions.
         */
        public static function udfAdmin(): string {}

        /**
         * SIndexAdmin allows to manage indicies.
         */
        public static function sindexAdmin(): string {}

        /**
         * ReadWriteUDF allows read, write and UDF transactions with the database.
         */
        public static function readWriteUdf(): string {}

        /**
         * ReadWrite allows read and write transactions with the database.
         */
        public static function readWrite(): string {}

        /**
         * Read allows read transactions with the database.
         */
        public static function read(): string {}

        /**
         * Write allows write transactions with the database.
         */
        public static function write(): string {}

        /**
         * Truncate allow issuing truncate commands.
         */
        public static function truncate(): string {}
    }

    /**
     * BatchWrite encapsulates a batch key and read/write operations with write policy.
     */
    class BatchWrite {
        public function __construct(\Aerospike\BatchWritePolicy $policy, \Aerospike\Key $key, array $ops) {}
    }

    /**
     * HLLWriteFlags specifies the HLL write operation flags.
     */
    class HllWriteFlags {
        /**
         * HLLWriteFlagsDefault is Default. Allow create or update.
         */
        public static function Default(): \Aerospike\HllWriteFlags {}

        /**
         * HLLWriteFlagsCreateOnly behaves like the following:
         * If the bin already exists, the operation will be denied.
         * If the bin does not exist, a new bin will be created.
         */
        public static function CreateOnly(): \Aerospike\HllWriteFlags {}

        /**
         * HLLWriteFlagsUpdateOnly behaves like the following:
         * If the bin already exists, the bin will be overwritten.
         * If the bin does not exist, the operation will be denied.
         */
        public static function UpdateOnly(): \Aerospike\HllWriteFlags {}

        /**
         * HLLWriteFlagsNoFail does not raise error if operation is denied.
         */
        public static function NoFail(): \Aerospike\HllWriteFlags {}

        /**
         * HLLWriteFlagsAllowFold allows the resulting set to be the minimum of provided index bits.
         * Also, allow the usage of less precise HLL algorithms when minHash bits
         * of all participating sets do not match.
         */
        public static function AllowFold(): \Aerospike\HllWriteFlags {}
    }

    /**
     * BatchUDFPolicy attributes used in batch UDF execute commands.
     */
    class BatchUdfPolicy {
        public $expiration;

        public $send_key;

        public $filter_expression;

        public $commit_level;

        public $durable_delete;

        public function __construct() {}

        /**
         * Optional expression filter. If FilterExpression exists and evaluates to false, the specific batch key
         * request is not performed and BatchRecord.ResultCode is set to types.FILTERED_OUT.
         *
         * Default: nil
         */
        public function getFilterExpression(): ?\Aerospike\Expression {}

        public function setFilterExpression(mixed $filter_expression) {}

        /**
         * Desired consistency guarantee when committing a transaction on the server. The default
         * (COMMIT_ALL) indicates that the server should wait for master and all replica commits to
         * be successful before returning success to the client.
         *
         * Default: CommitLevel.COMMIT_ALL
         */
        public function getCommitLevel(): \Aerospike\CommitLevel {}

        public function setCommitLevel(mixed $commit_level) {}

        /**
         * Expiration determines record expiration in seconds. Also known as TTL (Time-To-Live).
         * Seconds record will live before being removed by the server.
         * Expiration values:
         * TTLServerDefault (0): Default to namespace configuration variable "default-ttl" on the server.
         * TTLDontExpire (MaxUint32): Never expire for Aerospike 2 server versions >= 2.7.2 and Aerospike 3+ server
         * TTLDontUpdate (MaxUint32 - 1): Do not change ttl when record is written. Supported by Aerospike server versions >= 3.10.1
         * > 0: Actual expiration in seconds.
         */
        public function getExpiration(): \Aerospike\Expiration {}

        public function setExpiration(mixed $expiration) {}

        /**
         * DurableDelete leaves a tombstone for the record if the transaction results in a record deletion.
         * This prevents deleted records from reappearing after node failures.
         * Valid for Aerospike Server Enterprise Edition 3.10+ only.
         */
        public function getDurableDelete(): bool {}

        public function setDurableDelete(bool $durable_delete) {}

        /**
         * SendKey determines to whether send user defined key in addition to hash digest on both reads and writes.
         * If the key is sent on a write, the key will be stored with the record on
         * the server.
         * The default is to not send the user defined key.
         */
        public function getSendKey(): bool {}

        public function setSendKey(bool $send_key) {}
    }

    /**
     * Role allows granular access to database entities for users.
     */
    class Role {
        public $write_quota;

        public $privileges;

        public $name;

        public $read_quota;

        public $allowlist;

        /**
         * Name is role name
         */
        public function getName(): string {}

        /**
         * Privilege is the list of assigned privileges
         */
        public function getPrivileges(): array {}

        /**
         * While is the list of allowable IP addresses
         */
        public function getAllowlist(): array {}

        /**
         * ReadQuota is the maximum reads per second limit for the role
         */
        public function getReadQuota(): int {}

        /**
         * WriteQuota is the maximum writes per second limit for the role
         */
        public function writeQuota(): int {}
    }

    /**
     * ListOrderType determines the order of returned values in CDT list operations.
     */
    class ListSortFlags {
        /**
         * ListSortFlagsDefault is the default sort flag for CDT lists, and sort in Ascending order.
         */
        public static function Default(): \Aerospike\ListSortFlags {}

        /**
         * ListSortFlagsDescending will sort the contents of the list in descending order.
         */
        public static function Descending(): \Aerospike\ListSortFlags {}

        /**
         * ListSortFlagsDropDuplicates will drop duplicate values in the results of the CDT list operation.
         */
        public static function DropDuplicates(): \Aerospike\ListSortFlags {}
    }

    /**
     * Specifies whether a command, that needs to be executed on multiple cluster nodes, should be
     * executed sequentially, one node at a time, or in parallel on multiple nodes using the client's
     * thread pool.
     */
    class MapOrderType {
        public function attr(): int {}

        public function flag(): int {}

        /**
         * Map is not ordered. This is the default.
         */
        public static function Unordered(): \Aerospike\MapOrderType {}

        /**
         * Order map by key.
         */
        public static function KeyOrdered(): \Aerospike\MapOrderType {}

        /**
         * Order map by key, then value.
         */
        public static function KeyValueOrdered(): \Aerospike\MapOrderType {}
    }

    /**
     * MapWriteMode should only be used for server versions < 4.3.
     * MapWriteFlags are recommended for server versions >= 4.3.
     */
    class MapWriteMode {
        /**
         * If the key already exists, the item will be overwritten.
         * If the key does not exist, a new item will be created.
         */
        public static function Update(): \Aerospike\MapWriteMode {}

        /**
         * If the key already exists, the item will be overwritten.
         * If the key does not exist, the write will fail.
         */
        public static function UpdateOnly(): \Aerospike\MapWriteMode {}

        /**
         * If the key already exists, the write will fail.
         * If the key does not exist, a new item will be created.
         */
        public static function CreateOnly(): \Aerospike\MapWriteMode {}
    }

    /**
     * Statement encapsulates query statement parameters.
     */
    class Statement {
        public $bin_names;

        public $index_name;

        public $filter;

        public $namespace;

        public $setname;

        public function __construct(string $namespace, string $set_name, mixed $filter, ?array $bin_names) {}

        /**
         * Filter determines query index filter (Optional).
         * This filter is applied to the secondary index on query.
         * Query index filters must reference a bin which has a secondary index defined.
         */
        public function getFilter(): ?\Aerospike\Filter {}

        public function setFilter(mixed $filter) {}

        /**
         * IndexName determines query index name (Optional)
         * If not set, the server will determine the index from the filter's bin name.
         */
        public function getIndexName(): ?string {}

        public function setIndexName(?string $index_name) {}

        /**
         * BinNames detemines bin names (optional)
         */
        public function getBinNames(): array {}

        public function setBinNames(array $bin_names) {}

        /**
         * Namespace determines query Namespace
         */
        public function getNamespace(): string {}

        public function setNamespace(string $namespace) {}

        /**
         * SetName determines query Set name (Optional)
         */
        public function getSetname(): string {}

        public function setSetname(string $set_name) {}
    }

    /**
     *
     *  Record
     *
     * Container object for a database record.
     */
    class Record {
        public $generation;

        public $expiration;

        public $bins;

        public $ttl;

        public $key;

        /**
         * Bins is the map of requested name/value bins.
         */
        public function bin(string $name): mixed {}

        /**
         * Bins is the map of requested name/value bins.
         */
        public function getBins(): mixed {}

        /**
         * Generation shows record modification count.
         */
        public function getGeneration(): ?int {}

        /**
         * Expiration is TTL (Time-To-Live).
         * Number of seconds until record expires.
         */
        public function getExpiration(): \Aerospike\Expiration {}

        /**
         * Expiration is TTL (Time-To-Live).
         * Number of seconds until record expires.
         */
        public function getTtl(): ?int {}

        /**
         * Key is the record's key.
         * Might be empty, or may only consist of digest value.
         */
        public function getKey(): ?\Aerospike\Key {}
    }

    /**
     * BitOverflowAction specifies the action to take when bitwise add/subtract results in overflow/underflow.
     */
    class BitwiseOverflowAction {
        /**
         * BitOverflowActionFail specifies to fail operation with error.
         */
        public static function Fail(): \Aerospike\BitwiseOverflowAction {}

        /**
         * BitOverflowActionSaturate specifies that in add/subtract overflows/underflows, set to max/min value.
         * Example: MAXINT + 1 = MAXINT
         */
        public static function Saturate(): \Aerospike\BitwiseOverflowAction {}

        /**
         * BitOverflowActionWrap specifies that in add/subtract overflows/underflows, wrap the value.
         * Example: MAXINT + 1 = -1
         */
        public static function Wrap(): \Aerospike\BitwiseOverflowAction {}
    }

    /**
     * ResultCode signifies the database operation error codes.
     * The positive numbers align with the server side file kvs.h.
     */
    class ResultCode {
        /**
         * GRPC_ERROR is wrapped and directly returned from the grpc library
         */
        const GRPC_ERROR = null;

        /**
         * BATCH_FAILED means one or more keys failed in a batch.
         */
        const BATCH_FAILED = null;

        /**
         * NO_RESPONSE means no response was received from the server.
         */
        const NO_RESPONSE = null;

        /**
         * NETWORK_ERROR defines a network error. Checked the wrapped error for detail.
         */
        const NETWORK_ERROR = null;

        /**
         * COMMON_ERROR defines a common, none-aerospike error. Checked the wrapped error for detail.
         */
        const COMMON_ERROR = null;

        /**
         * MAX_RETRIES_EXCEEDED defines max retries limit reached.
         */
        const MAX_RETRIES_EXCEEDED = null;

        /**
         * MAX_ERROR_RATE defines max errors limit reached.
         */
        const MAX_ERROR_RATE = null;

        /**
         * RACK_NOT_DEFINED defines requested Rack for node/namespace was not defined in the cluster.
         */
        const RACK_NOT_DEFINED = null;

        /**
         * INVALID_CLUSTER_PARTITION_MAP defines cluster has an invalid partition map, usually due to bad configuration.
         */
        const INVALID_CLUSTER_PARTITION_MAP = null;

        /**
         * SERVER_NOT_AVAILABLE defines server is not accepting requests.
         */
        const SERVER_NOT_AVAILABLE = null;

        /**
         * CLUSTER_NAME_MISMATCH_ERROR defines cluster Name does not match the ClientPolicy.ClusterName value.
         */
        const CLUSTER_NAME_MISMATCH_ERROR = null;

        /**
         * RECORDSET_CLOSED defines recordset has already been closed or cancelled
         */
        const RECORDSET_CLOSED = null;

        /**
         * NO_AVAILABLE_CONNECTIONS_TO_NODE defines there were no connections available to the node in the pool, and the pool was limited
         */
        const NO_AVAILABLE_CONNECTIONS_TO_NODE = null;

        /**
         * TYPE_NOT_SUPPORTED defines data type is not supported by aerospike server.
         */
        const TYPE_NOT_SUPPORTED = null;

        /**
         * COMMAND_REJECTED defines info Command was rejected by the server.
         */
        const COMMAND_REJECTED = null;

        /**
         * QUERY_TERMINATED defines query was terminated by user.
         */
        const QUERY_TERMINATED = null;

        /**
         * SCAN_TERMINATED defines scan was terminated by user.
         */
        const SCAN_TERMINATED = null;

        /**
         * INVALID_NODE_ERROR defines chosen node is not currently active.
         */
        const INVALID_NODE_ERROR = null;

        /**
         * PARSE_ERROR defines client parse error.
         */
        const PARSE_ERROR = null;

        /**
         * SERIALIZE_ERROR defines client serialization error.
         */
        const SERIALIZE_ERROR = null;

        /**
         * OK defines operation was successful.
         */
        const OK = null;

        /**
         * SERVER_ERROR defines unknown server failure.
         */
        const SERVER_ERROR = null;

        /**
         * KEY_NOT_FOUND_ERROR defines on retrieving, touching or replacing a record that doesn't exist.
         */
        const KEY_NOT_FOUND_ERROR = null;

        /**
         * GENERATION_ERROR defines on modifying a record with unexpected generation.
         */
        const GENERATION_ERROR = null;

        /**
         * PARAMETER_ERROR defines bad parameter(s) were passed in database operation call.
         */
        const PARAMETER_ERROR = null;

        /**
         * KEY_EXISTS_ERROR defines on create-only (write unique) operations on a record that already exists.
         */
        const KEY_EXISTS_ERROR = null;

        /**
         * BIN_EXISTS_ERROR defines bin already exists on a create-only operation.
         */
        const BIN_EXISTS_ERROR = null;

        /**
         * CLUSTER_KEY_MISMATCH defines expected cluster ID was not received.
         */
        const CLUSTER_KEY_MISMATCH = null;

        /**
         * SERVER_MEM_ERROR defines server has run out of memory.
         */
        const SERVER_MEM_ERROR = null;

        /**
         * TIMEOUT defines client or server has timed out.
         */
        const TIMEOUT = null;

        /**
         * ALWAYS_FORBIDDEN defines operation not allowed in current configuration.
         */
        const ALWAYS_FORBIDDEN = null;

        /**
         * PARTITION_UNAVAILABLE defines partition is unavailable.
         */
        const PARTITION_UNAVAILABLE = null;

        /**
         * BIN_TYPE_ERROR defines operation is not supported with configured bin type (single-bin or multi-bin);
         */
        const BIN_TYPE_ERROR = null;

        /**
         * RECORD_TOO_BIG defines record size exceeds limit.
         */
        const RECORD_TOO_BIG = null;

        /**
         * KEY_BUSY defines too many concurrent operations on the same record.
         */
        const KEY_BUSY = null;

        /**
         * SCAN_ABORT defines scan aborted by server.
         */
        const SCAN_ABORT = null;

        /**
         * UNSUPPORTED_FEATURE defines unsupported Server Feature (e.g. Scan + UDF)
         */
        const UNSUPPORTED_FEATURE = null;

        /**
         * BIN_NOT_FOUND defines bin not found on update-only operation.
         */
        const BIN_NOT_FOUND = null;

        /**
         * DEVICE_OVERLOAD defines device not keeping up with writes.
         */
        const DEVICE_OVERLOAD = null;

        /**
         * KEY_MISMATCH defines key type mismatch.
         */
        const KEY_MISMATCH = null;

        /**
         * INVALID_NAMESPACE defines invalid namespace.
         */
        const INVALID_NAMESPACE = null;

        /**
         * BIN_NAME_TOO_LONG defines bin name length greater than 14 characters, or maximum number of unique bin names are exceeded;
         */
        const BIN_NAME_TOO_LONG = null;

        /**
         * FAIL_FORBIDDEN defines operation not allowed at this time.
         */
        const FAIL_FORBIDDEN = null;

        /**
         * FAIL_ELEMENT_NOT_FOUND defines element Not Found in CDT
         */
        const FAIL_ELEMENT_NOT_FOUND = null;

        /**
         * FAIL_ELEMENT_EXISTS defines element Already Exists in CDT
         */
        const FAIL_ELEMENT_EXISTS = null;

        /**
         * ENTERPRISE_ONLY defines attempt to use an Enterprise feature on a Community server or a server without the applicable feature key;
         */
        const ENTERPRISE_ONLY = null;

        /**
         * OP_NOT_APPLICABLE defines the operation cannot be applied to the current bin value on the server.
         */
        const OP_NOT_APPLICABLE = null;

        /**
         * FILTERED_OUT defines the transaction was not performed because the filter was false.
         */
        const FILTERED_OUT = null;

        /**
         * LOST_CONFLICT defines write command loses conflict to XDR.
         */
        const LOST_CONFLICT = null;

        /**
         * QUERY_END defines there are no more records left for query.
         */
        const QUERY_END = null;

        /**
         * SECURITY_NOT_SUPPORTED defines security type not supported by connected server.
         */
        const SECURITY_NOT_SUPPORTED = null;

        /**
         * SECURITY_NOT_ENABLED defines administration command is invalid.
         */
        const SECURITY_NOT_ENABLED = null;

        /**
         * SECURITY_SCHEME_NOT_SUPPORTED defines administration field is invalid.
         */
        const SECURITY_SCHEME_NOT_SUPPORTED = null;

        /**
         * INVALID_COMMAND defines administration command is invalid.
         */
        const INVALID_COMMAND = null;

        /**
         * INVALID_FIELD defines administration field is invalid.
         */
        const INVALID_FIELD = null;

        /**
         * ILLEGAL_STATE defines security protocol not followed.
         */
        const ILLEGAL_STATE = null;

        /**
         * INVALID_USER defines user name is invalid.
         */
        const INVALID_USER = null;

        /**
         * USER_ALREADY_EXISTS defines user was previously created.
         */
        const USER_ALREADY_EXISTS = null;

        /**
         * INVALID_PASSWORD defines password is invalid.
         */
        const INVALID_PASSWORD = null;

        /**
         * EXPIRED_PASSWORD defines security credential is invalid.
         */
        const EXPIRED_PASSWORD = null;

        /**
         * FORBIDDEN_PASSWORD defines forbidden password (e.g. recently used)
         */
        const FORBIDDEN_PASSWORD = null;

        /**
         * INVALID_CREDENTIAL defines security credential is invalid.
         */
        const INVALID_CREDENTIAL = null;

        /**
         * EXPIRED_SESSION defines login session expired.
         */
        const EXPIRED_SESSION = null;

        /**
         * INVALID_ROLE defines role name is invalid.
         */
        const INVALID_ROLE = null;

        /**
         * ROLE_ALREADY_EXISTS defines role already exists.
         */
        const ROLE_ALREADY_EXISTS = null;

        /**
         * INVALID_PRIVILEGE defines privilege is invalid.
         */
        const INVALID_PRIVILEGE = null;

        /**
         * INVALID_WHITELIST defines invalid IP address whiltelist
         */
        const INVALID_WHITELIST = null;

        /**
         * QUOTAS_NOT_ENABLED defines Quotas not enabled on server.
         */
        const QUOTAS_NOT_ENABLED = null;

        /**
         * INVALID_QUOTA defines invalid quota value.
         */
        const INVALID_QUOTA = null;

        /**
         * NOT_AUTHENTICATED defines user must be authentication before performing database operations.
         */
        const NOT_AUTHENTICATED = null;

        /**
         * ROLE_VIOLATION defines user does not posses the required role to perform the database operation.
         */
        const ROLE_VIOLATION = null;

        /**
         * NOT_WHITELISTED defines command not allowed because sender IP address not whitelisted.
         */
        const NOT_WHITELISTED = null;

        /**
         * QUOTA_EXCEEDED defines Quota exceeded.
         */
        const QUOTA_EXCEEDED = null;

        /**
         * UDF_BAD_RESPONSE defines a user defined function returned an error code.
         */
        const UDF_BAD_RESPONSE = null;

        /**
         * BATCH_DISABLED defines batch functionality has been disabled.
         */
        const BATCH_DISABLED = null;

        /**
         * BATCH_MAX_REQUESTS_EXCEEDED defines batch max requests have been exceeded.
         */
        const BATCH_MAX_REQUESTS_EXCEEDED = null;

        /**
         * BATCH_QUEUES_FULL defines all batch queues are full.
         */
        const BATCH_QUEUES_FULL = null;

        /**
         * GEO_INVALID_GEOJSON defines invalid GeoJSON on insert/update
         */
        const GEO_INVALID_GEOJSON = null;

        /**
         * INDEX_FOUND defines secondary index already exists.
         */
        const INDEX_FOUND = null;

        /**
         * INDEX_NOTFOUND defines requested secondary index does not exist.
         */
        const INDEX_NOT_FOUND = null;

        /**
         * INDEX_OOM defines secondary index memory space exceeded.
         */
        const INDEX_OOM = null;

        /**
         * INDEX_NOTREADABLE defines secondary index not available.
         */
        const INDEX_NOT_READABLE = null;

        /**
         * INDEX_GENERIC defines generic secondary index error.
         */
        const INDEX_GENERIC = null;

        /**
         * INDEX_NAME_MAXLEN defines index name maximum length exceeded.
         */
        const INDEX_NAME_MAX_LEN = null;

        /**
         * INDEX_MAXCOUNT defines maximum number of indexes exceeded.
         */
        const INDEX_MAX_COUNT = null;

        /**
         * QUERY_ABORTED defines secondary index query aborted.
         */
        const QUERY_ABORTED = null;

        /**
         * QUERY_QUEUEFULL defines secondary index queue full.
         */
        const QUERY_QUEUE_FULL = null;

        /**
         * QUERY_TIMEOUT defines secondary index query timed out on server.
         */
        const QUERY_TIMEOUT = null;

        /**
         * QUERY_GENERIC defines generic query error.
         */
        const QUERY_GENERIC = null;

        /**
         * QUERY_NETIO_ERR defines query NetIO error on server
         */
        const QUERY_NET_IO_ERR = null;

        /**
         * QUERY_DUPLICATE defines duplicate TaskId sent for the statement
         */
        const QUERY_DUPLICATE = null;

        /**
         * AEROSPIKE_ERR_UDF_NOT_FOUND defines UDF does not exist.
         */
        const AEROSPIKE_ERR_UDF_NOT_FOUND = null;

        /**
         * AEROSPIKE_ERR_LUA_FILE_NOT_FOUND defines LUA file does not exist.
         */
        const AEROSPIKE_ERR_LUA_FILE_NOT_FOUND = null;

        public static function toString(int $code): string {}
    }

    /**
     * ListWriteFlags detemines write flags for CDT lists
     * type ListWriteFlags int
     */
    class ListWriteFlags {
        /**
         * ListWriteFlagsDefault is the default behavior. It means:  Allow duplicate values and insertions at any index.
         */
        public static function Default(): \Aerospike\ListWriteFlags {}

        /**
         * ListWriteFlagsAddUnique means: Only add unique values.
         */
        public static function AddUnique(): \Aerospike\ListWriteFlags {}

        /**
         * ListWriteFlagsInsertBounded means: Enforce list boundaries when inserting.  Do not allow values to be inserted
         * at index outside current list boundaries.
         */
        public static function InsertBounded(): \Aerospike\ListWriteFlags {}

        /**
         * ListWriteFlagsNoFail means: do not raise error if a list item fails due to write flag constraints.
         */
        public static function NoFail(): \Aerospike\ListWriteFlags {}

        /**
         * ListWriteFlagsPartial means: allow other valid list items to be committed if a list item fails due to
         * write flag constraints.
         */
        public static function Partial(): \Aerospike\ListWriteFlags {}
    }
}
