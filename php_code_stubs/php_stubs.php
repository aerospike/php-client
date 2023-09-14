<?php

// Stubs for aerospike

namespace {
    function Aerospike(\ClientPolicy $policy, string $hosts): mixed {}

    function print_header(string $desc, ?int $emph) {}

    /**
     * Record expiration, also known as time-to-live (TTL).
     */
    class Expiration {
        /**
         * Set the record to expire X seconds from now
         */
        public static function seconds(int $seconds): \Expiration {}

        /**
         * Set the record's expiry time using the default time-to-live (TTL) value for the namespace
         */
        public static function namespaceDefault(): \Expiration {}

        /**
         * Set the record to never expire. Requires Aerospike 2 server version 2.7.2 or later or
         * Aerospike 3 server version 3.1.4 or later. Do not use with older servers.
         */
        public static function never(): \Expiration {}

        /**
         * Do not change the record's expiry time when updating the record; requires Aerospike server
         * version 3.10.1 or later.
         */
        public static function dontUpdate(): \Expiration {}
    }

    /**
     * Specifies whether a command, that needs to be executed on multiple cluster nodes, should be
     * executed sequentially, one node at a time, or in parallel on multiple nodes using the client's
     * thread pool.
     */
    class Concurrency {
        /**
         * Issue commands sequentially. This mode has a performance advantage for small to
         * medium sized batch sizes because requests can be issued in the main transaction thread.
         * This is the default.
         */
        public static function sequential(): \Concurrency {}

        /**
         * Issue all commands in parallel threads. This mode has a performance advantage for
         * extremely large batch sizes because each node can process the request immediately. The
         * downside is extra threads will need to be created (or takedn from a thread pool).
         */
        public static function parallel(): \Concurrency {}

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
        public static function maxThreads(int $threads): \Concurrency {}
    }

    /**
     * Query filter definition. Currently, only one filter is allowed in a Statement, and must be on a
     * bin which has a secondary index defined.
     *
     * Filter instances should be instantiated using one of the provided macros:
     *
     * - `as_eq`
     * - `as_range`
     * - `as_contains`
     * - `as_contains_range`
     * - `as_within_region`
     * - `as_within_radius`
     * - `as_regions_containing_point`
     */
    class Filter {

        /* Create range filter for queries; supports integer values.       
         *
         * @param  string $bin_name
         * @param  PhpValue $begin
         * @param  PhpValue $end
         * @return Filter
         */
        public static function range(string $bin_name, mixed $begin, mixed $end): \Filter {}

        /* Create contains number filter for queries on a collection index. */
        public static function contains(string $bin_name, mixed $value, ?\CollectionIndexType $cit): \Filter {}

        /* Create contains range filter for queries on a collection index. */
        public static function containsRange(string $bin_name, mixed $begin, mixed $end, ?\CollectionIndexType $cit): \Filter {}

        /*  Create geospatial "points within region" filter for queries. For queries on a collection index
        the collection index type must be specified.
        Usage :
        $pointString = '{"type":"AeroCircle","coordinates":[[-89.0000,23.0000], 1000]}'
        Filter::withinRegion("binName", $pointString)
        */
        public static function withinRegion(string $bin_name, string $region, ?\CollectionIndexType $cit): \Filter {}

        /*  Create geospatial "points within radius" filter for queries. For queries on a collection index
        the collection index type must be specified.
        Usage :

        Usage :
        $lat = 43.0004;
        $lng = -89.0005;
        $radius = 1000;
        $filter = Filter::withinRadius("binName", $lat, $lng, $radius);
        */
        public static function withinRadius(string $bin_name, float $lat, float $lng, float $radius, ?\CollectionIndexType $cit): \Filter {}
        
        /*  Create geospatial "regions containing point" filter for queries. For queries on a collection
        index the collection index type must be specified.
        Usage :
        $pointString = '{"type":"AeroCircle","coordinates":[[-89.0000,23.0000], 1000]}'
        Filter::regionsContainingPoint("binName", $pointString)
        */
        public static function regionsContainingPoint(string $bin_name, string $point, ?\CollectionIndexType $cit): \Filter {}
    }

    /* 
    `QueryPolicy` encapsulates parameters for query operations.
    */
    class QueryPolicy {
        public $fail_on_cluster_change;

        public $record_queue_size;

        public $base_policy;

        public $max_concurrent_nodes;

        public $filter_expression;

        public function __construct() {}

        public function getBasePolicy(): \BasePolicyWrapper {}

        public function setBasePolicy(mixed $base_policy) {}

        public function getMaxConcurrentNodes(): int {}

        public function setMaxConcurrentNodes(int $max_concurrent_nodes) {}

        public function getRecordQueueSize(): int {}

        public function setRecordQueueSize(int $record_queue_size) {}

        public function getFailOnClusterChange(): bool {}

        public function setFailOnClusterChange(bool $fail_on_cluster_change) {}

        public function getFilterExpression(): ?\FilterExpression {}

        public function setFilterExpression(?mixed $filter_expression) {}
    }

    class CommitLevel {
        /**
         * CommitAll indicates the server should wait until successfully committing master and all
         * replicas.
         */
        public static function commitAll(): \CommitLevel {}

        /**
         * CommitMaster indicates the server should wait until successfully committing master only.
         */
        public static function commitMaster(): \CommitLevel {}
    }

    class Priority {
        /**
         * Default determines that the server defines the priority.
         */
        public static function default(): \Priority {}

        /**
         * Low determines that the server should run the operation in a background thread.
         */
        public static function low(): \Priority {}

        /**
         * Medium determines that the server should run the operation at medium priority.
         */
        public static function medium(): \Priority {}

        /**
         * High determines that the server should run the operation at the highest priority.
         */
        public static function high(): \Priority {}
    }

    class ExpType {
        public static function nil(): \ExpType {}

        public static function bool(): \ExpType {}

        public static function int(): \ExpType {}

        public static function string(): \ExpType {}

        public static function list(): \ExpType {}

        public static function map(): \ExpType {}

        public static function blob(): \ExpType {}

        public static function float(): \ExpType {}

        public static function geo(): \ExpType {}

        public static function hll(): \ExpType {}
    }

    /**
     * Container object for a record bin, comprising a name and a value.
     */
    class Bin {
        public function __construct(string $name, mixed $value) {}
    }

    class BasePolicyWrapper {
        public $priority;

        public $consistency_level;

        public $max_retries;

        public $sleep_between_retries;

        public $timeout;

        public $filter_expression;

        public function getPriority(): \Priority {}

        public function setPriority(mixed $priority) {}

        public function getConsistencyLevel(): \ConsistencyLevel {}

        public function setConsistencyLevel(mixed $consistency_level) {}

        public function getTimeout(): int {}

        public function setTimeout(int $timeout_in_millis) {}

        public function getMaxRetries(): ?int {}

        public function setMaxRetries(?int $max_retries) {}

        public function getSleepBetweenRetries(): int {}

        public function setSleepBetweenRetries(int $sleep_between_retries_millis) {}

        public function getFilterExpression(): ?\FilterExpression {}

        public function setFilterExpression(?mixed $filter_expression) {}
    }

    class Value {
        public static function nil(): mixed {}

        public static function int(int $val): mixed {}

        public static function uint(int $val): mixed {}

        public static function string(string $val): mixed {}

        public static function blob(array $val): mixed {}

        public static function geoJson(string $val): \GeoJSON {}

        public static function hll(array $val): \HLL {}
    }

    /* 
    `ScanPolicy` encapsulates optional parameters used in scan operations.
    */
    class ScanPolicy {
        public $max_concurrent_nodes;

        public $fail_on_cluster_change;

        public $scan_percent;

        public $base_policy;

        public $socket_timeout;

        public $filter_expression;

        public $record_queue_size;

        public function __construct() {}

        public function getBasePolicy(): \BasePolicyWrapper {}

        public function setBasePolicy(mixed $base_policy) {}

        public function getScanPercent(): int {}

        public function setScanPercent(int $scan_percent) {}

        public function getMaxConcurrentNodes(): int {}

        public function setMaxConcurrentNodes(int $max_concurrent_nodes) {}

        public function getRecordQueueSize(): int {}

        public function setRecordQueueSize(int $record_queue_size) {}

        public function getFailOnClusterChange(): bool {}

        public function setFailOnClusterChange(bool $fail_on_cluster_change) {}

        public function getSocketTimeout(): int {}

        public function setSocketTimeout(int $socket_timeout) {}

        public function getFilterExpression(): ?\FilterExpression {}

        public function setFilterExpression(?mixed $filter_expression) {}
    }

    /* Underlying data type of secondary index. */
    class IndexType {
        public static function numeric(): \IndexType {}

        public static function string(): \IndexType {}

        public static function geo2DSphere(): \IndexType {}
    }

    /* creates a value of type GeoJson */
    class GeoJSON {
        public $value;

        public function getValue(): string {}

        public function setValue(string $geo) {}

        /**
         * Returns a string representation of the value.
         */
        public function asString(): string {}
    }

    /* creates a value of type HLL */
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
     * `ClientPolicy` encapsulates parameters for client policy command.
     */
    class ClientPolicy {
        public $conn_pools_per_node;

        public $user;

        public $timeout;

        public $idle_timeout;

        public $max_conns_per_node;

        public $password;

        public function __construct() {}

        public function getUser(): ?string {}

        public function setUser(?string $user) {}

        public function getPassword(): ?string {}

        public function setPassword(?string $password) {}

        public function getTimeout(): int {}

        public function setTimeout(int $timeout_in_millis) {}

        public function getIdleTimeout(): int {}

        public function setIdleTimeout(int $timeout_in_millis) {}

        public function getMaxConnsPerNode(): int {}

        public function setMaxConnsPerNode(int $sz) {}

        public function getConnPoolsPerNode(): int {}

        public function setConnPoolsPerNode(int $sz) {}
    }

    /**
     * `GenerationPolicy` determines how to handle record writes based on record generation.
     */
    class GenerationPolicy {
        /**
         * None means: Do not use record generation to restrict writes.
         */
        public static function none(): \GenerationPolicy {}

        /**
         * ExpectGenEqual means: Update/delete record if expected generation is equal to server
         * generation. Otherwise, fail.
         */
        public static function expectGenEqual(): \GenerationPolicy {}

        /**
         * ExpectGenGreater means: Update/delete record if expected generation greater than the server
         * generation. Otherwise, fail. This is useful for restore after backup.
         */
        public static function expectGenGreater(): \GenerationPolicy {}
    }   

    /* Key and bin names used in batch read commands where variable bins are needed for each key. */
    class BatchRead {
        public function __construct(mixed $key, ?array $bins) {}
        
        /* Will contain the record after the batch read operation. */
        public function record(): ?\Record {}
    }

    /**
     * Query statement parameters.
     */
    class Statement {
        public $filters;

        public function __construct(string $namespace, string $setname, ?array $bins) {}

        public function getFilters(): ?array {}

        public function setFilters(?array $filters) {}
    }

    /**
     * Virtual collection of records retrieved through queries and scans. During a query/scan,
     * multiple threads will retrieve records from the server nodes and put these records on an
     * internal queue managed by the recordset. The single user thread consumes these records from the
     * queue.
     */
    class Recordset {
        public $active;

        public function close() {}

        public function getActive(): bool {}

        public function next(): ?\Record {}
    }

    /* Secondary index collection type. */
    class CollectionIndexType {

        /* Normal, scalar index. */
        public static function default(): \CollectionIndexType {}

        /* Index list elements. */
        public static function list(): \CollectionIndexType {}

        /* Index map keys. */
        public static function mapKeys(): \CollectionIndexType {}

        /* Index map values. */
        public static function mapValues(): \CollectionIndexType {}
    }

    class Client {
        public function hosts(): string {}

        public function close(): mixed {}

        /**
         * Write record bin(s). The policy specifies the transaction timeout, record expiration and
         * how the transaction is handled when the record already exists.
         */
        public function put(\WritePolicy $policy, \Key $key, array $bins): mixed {}

        /**
         * Read record for the specified key. Depending on the bins value provided, all record bins,
         * only selected record bins or only the record headers will be returned. The policy can be
         * used to specify timeouts.
         */
        public function get(\ReadPolicy $policy, \Key $key, ?array $bins): \Record {}

        /**
         * Add integer bin values to existing record bin values. The policy specifies the transaction
         * timeout, record expiration and how the transaction is handled when the record already
         * exists. This call only works for integer values.
         */
        public function add(\WritePolicy $policy, \Key $key, array $bins): mixed {}

        /**
         * Append bin string values to existing record bin values. The policy specifies the
         * transaction timeout, record expiration and how the transaction is handled when the record
         * already exists. This call only works for string values.
         */
        public function append(\WritePolicy $policy, \Key $key, array $bins): mixed {}

        /**
         * Prepend bin string values to existing record bin values. The policy specifies the
         * transaction timeout, record expiration and how the transaction is handled when the record
         * already exists. This call only works for string values.
         */
        public function prepend(\WritePolicy $policy, \Key $key, array $bins): mixed {}

        /**
         * Delete record for specified key. The policy specifies the transaction timeout.
         * The call returns `true` if the record existed on the server before deletion.
         */
        public function delete(\WritePolicy $policy, \Key $key): bool {}

        /**
         * Reset record's time to expiration using the policy's expiration. Fail if the record does
         * not exist.
         */
        public function touch(\WritePolicy $policy, \Key $key): mixed {}

        /**
         * Determine if a record key exists. The policy can be used to specify timeouts.
         */
        public function exists(\WritePolicy $policy, \Key $key): bool {}

        /**
         * Removes all records in the specified namespace/set efficiently.
         */
        public function truncate(string $namespace, string $set_name, ?int $before_nanos): mixed {}

        /**
         * Read all records in the specified namespace and set and return a record iterator. The scan
         * executor puts records on a queue in separate threads. The calling thread concurrently pops
         * records off the queue through the record iterator. Up to `policy.max_concurrent_nodes`
         * nodes are scanned in parallel. If concurrent nodes is set to zero, the server nodes are
         * read in series.
         */
        public function scan(\ScanPolicy $policy, string $namespace, string $setname, ?array $bins): \Recordset {}

        /**
         * Execute a query on all server nodes and return a record iterator. The query executor puts
         * records on a queue in separate threads. The calling thread concurrently pops records off
         * the queue through the record iterator.
         */
        public function query(\QueryPolicy $policy, \Statement $statement): \Recordset {}

        /**
         * Create a secondary index on a bin containing scalar values. This asynchronous server call
         * returns before the command is complete.
         */
        public function createIndex(string $namespace, string $set_name, string $bin_name, string $index_name, \IndexType $index_type, ?\CollectionIndexType $cit): mixed {}

        public function dropIndex(string $namespace, string $set_name, string $index_name): mixed {}

        public function batchGet(\BatchPolicy $policy, array $batch_reads): array {}
    }

    /**
     * Filter expression, which can be applied to most commands, to control which records are
     * affected by the command.
     */
    class FilterExpression {
        /**
         * Create a record key expression of specified type.
         */
        public static function key(mixed $exp_type): \FilterExpression {}

        /**
         * Create function that returns if the primary key is stored in the record meta data
         * as a boolean expression. This would occur when `send_key` is true on record write.
         */
        public static function keyExists(): \FilterExpression {}

        /**
         * Create 64 bit int bin expression.
         */
        public static function intBin(string $name): \FilterExpression {}

        /**
         * Create string bin expression.
         */
        public static function stringBin(string $name): \FilterExpression {}

        /**
         * Create blob bin expression.
         */
        public static function blobBin(string $name): \FilterExpression {}

        /**
         * Create 64 bit float bin expression.
         */
        public static function floatBin(string $name): \FilterExpression {}

        /**
         * Create geo bin expression.
         */
        public static function geoBin(string $name): \FilterExpression {}

        /**
         * Create list bin expression.
         */
        public static function listBin(string $name): \FilterExpression {}

        /**
         * Create map bin expression.
         */
        public static function mapBin(string $name): \FilterExpression {}

        /**
         * Create a HLL bin expression
         */
        public static function hllBin(string $name): \FilterExpression {}

        /**
         * Create function that returns if bin of specified name exists.
         */
        public static function binExists(string $name): \FilterExpression {}

        /**
         * Create function that returns bin's integer particle type.
         */
        public static function binType(string $name): \FilterExpression {}

        /**
         * Create function that returns record set name string.
         */
        public static function setName(): \FilterExpression {}

        /**
         * Create function that returns record size on disk.
         * If server storage-engine is memory, then zero is returned.
         */
        public static function deviceSize(): \FilterExpression {}

        /**
         * Create function that returns record last update time expressed as 64 bit integer
         * nanoseconds since 1970-01-01 epoch.
         */
        public static function lastUpdate(): \FilterExpression {}

        /**
         * Create expression that returns milliseconds since the record was last updated.
         * This expression usually evaluates quickly because record meta data is cached in memory.
         */
        public static function sinceUpdate(): \FilterExpression {}

        /**
         * Create function that returns record expiration time expressed as 64 bit integer
         * nanoseconds since 1970-01-01 epoch.
         */
        public static function voidTime(): \FilterExpression {}

        /**
         * Create function that returns record expiration time (time to live) in integer seconds.
         */
        public static function ttl(): \FilterExpression {}

        /**
         * Create expression that returns if record has been deleted and is still in tombstone state.
         * This expression usually evaluates quickly because record meta data is cached in memory.
         */
        public static function isTombstone(): \FilterExpression {}

        /**
         * Create function that returns record digest modulo as integer.
         */
        public static function digestModulo(int $modulo): \FilterExpression {}

        /**
         * Create function like regular expression string operation.
         */
        public static function regexCompare(string $regex, int $flags, mixed $bin): \FilterExpression {}

        /**
         * Create compare geospatial operation.
         */
        public static function geoCompare(mixed $left, mixed $right): \FilterExpression {}

        /**
         * Creates 64 bit integer value
         */
        public static function intVal(int $val): \FilterExpression {}

        /**
         * Creates a Boolean value
         */
        public static function boolVal(bool $val): \FilterExpression {}

        /**
         * Creates String bin value
         */
        public static function stringVal(string $val): \FilterExpression {}

        /**
         * Creates 64 bit float bin value
         */
        public static function floatVal(float $val): \FilterExpression {}

        /**
         * Creates Blob bin value
         */
        public static function blobVal(array $val): \FilterExpression {}

        /**
         * Create List bin PHPValue
         * Not Supported in pre-alpha release
         * Create Map bin PHPValue
         * Not Supported in pre-alpha release
         * Create geospatial json string value.
         */
        public static function geoVal(string $val): \FilterExpression {}

        /**
         * Create a Nil PHPValue
         */
        public static function nil(): \FilterExpression {}

        /**
         * Create "not" operator expression.
         */
        public static function not(mixed $exp): \FilterExpression {}

        /**
         * Create "and" (&&) operator that applies to a variable number of expressions.
         * // (a > 5 || a == 0) && b < 3
         */
        public static function and(array $exps): \FilterExpression {}

        /**
         * Create "or" (||) operator that applies to a variable number of expressions.
         */
        public static function or(array $exps): \FilterExpression {}

        /**
         * Create "xor" (^) operator that applies to a variable number of expressions.
         */
        public static function xor(array $exps): \FilterExpression {}

        /**
         * Create equal (==) expression.
         */
        public static function eq(mixed $left, mixed $right): \FilterExpression {}

        /**
         * Create not equal (!=) expression
         */
        public static function ne(mixed $left, mixed $right): \FilterExpression {}

        /**
         * Create greater than (>) operation.
         */
        public static function gt(mixed $left, mixed $right): \FilterExpression {}

        /**
         * Create greater than or equal (>=) operation.
         */
        public static function ge(mixed $left, mixed $right): \FilterExpression {}

        /**
         * Create less than (<) operation.
         */
        public static function lt(mixed $left, mixed $right): \FilterExpression {}

        /**
         * Create less than or equals (<=) operation.
         */
        public static function le(mixed $left, mixed $right): \FilterExpression {}

        /**
         * Create "add" (+) operator that applies to a variable number of expressions.
         * Return sum of all `FilterExpressions` given. All arguments must resolve to the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function numAdd(array $exps): \FilterExpression {}

        /**
         * Create "subtract" (-) operator that applies to a variable number of expressions.
         * If only one `FilterExpressions` is provided, return the negation of that argument.
         * Otherwise, return the sum of the 2nd to Nth `FilterExpressions` subtracted from the 1st
         * `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function numSub(array $exps): \FilterExpression {}

        /**
         * Create "multiply" (*) operator that applies to a variable number of expressions.
         * Return the product of all `FilterExpressions`. If only one `FilterExpressions` is supplied, return
         * that `FilterExpressions`. All `FilterExpressions` must resolve to the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function numMul(array $exps): \FilterExpression {}

        /**
         * Create "divide" (/) operator that applies to a variable number of expressions.
         * If there is only one `FilterExpressions`, returns the reciprocal for that `FilterExpressions`.
         * Otherwise, return the first `FilterExpressions` divided by the product of the rest.
         * All `FilterExpressions` must resolve to the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function numDiv(array $exps): \FilterExpression {}

        /**
         * Create "power" operator that raises a "base" to the "exponent" power.
         * All arguments must resolve to floats.
         * Requires server version 5.6.0+.
         */
        public static function numPow(mixed $base, mixed $exponent): \FilterExpression {}

        /**
         * Create "log" operator for logarithm of "num" with base "base".
         * All arguments must resolve to floats.
         * Requires server version 5.6.0+.
         */
        public static function numLog(mixed $num, mixed $base): \FilterExpression {}

        /**
         * Create "modulo" (%) operator that determines the remainder of "numerator"
         * divided by "denominator". All arguments must resolve to integers.
         * Requires server version 5.6.0+.
         */
        public static function numMod(mixed $numerator, mixed $denominator): \FilterExpression {}

        /**
         * Create operator that returns absolute value of a number.
         * All arguments must resolve to integer or float.
         * Requires server version 5.6.0+.
         */
        public static function numAbs(mixed $value): \FilterExpression {}

        /**
         * Create expression that rounds a floating point number down to the closest integer value.
         * The return type is float.
         */
        public static function numFloor(mixed $num): \FilterExpression {}

        /**
         * Create expression that rounds a floating point number up to the closest integer value.
         * The return type is float.
         * Requires server version 5.6.0+.
         */
        public static function numCeil(mixed $num): \FilterExpression {}

        /**
         * Create expression that converts an integer to a float.
         * Requires server version 5.6.0+.
         */
        public static function toInt(mixed $num): \FilterExpression {}

        /**
         * Create expression that converts a float to an integer.
         * Requires server version 5.6.0+.
         */
        public static function toFloat(mixed $num): \FilterExpression {}

        /**
         * Create integer "and" (&) operator that is applied to two or more integers.
         * All arguments must resolve to integers.
         * Requires server version 5.6.0+.
         */
        public static function intAnd(array $exps): \FilterExpression {}

        /**
         * Create integer "or" (|) operator that is applied to two or more integers.
         * All arguments must resolve to integers.
         * Requires server version 5.6.0+.
         */
        public static function intOr(array $exps): \FilterExpression {}

        /**
         * Create integer "xor" (^) operator that is applied to two or more integers.
         * All arguments must resolve to integers.
         * Requires server version 5.6.0+.
         */
        public static function intXor(array $exps): \FilterExpression {}

        /**
         * Create integer "not" (~) operator.
         * Requires server version 5.6.0+.
         */
        public static function intNot(mixed $exp): \FilterExpression {}

        /**
         * Create integer "left shift" (<<) operator.
         * Requires server version 5.6.0+.
         */
        public static function intLshift(mixed $value, mixed $shift): \FilterExpression {}

        /**
         * Create integer "logical right shift" (>>>) operator.
         * Requires server version 5.6.0+.
         */
        public static function intRshift(mixed $value, mixed $shift): \FilterExpression {}

        /**
         * Create integer "arithmetic right shift" (>>) operator.
         * The sign bit is preserved and not shifted.
         * Requires server version 5.6.0+.
         */
        public static function intArshift(mixed $value, mixed $shift): \FilterExpression {}

        /**
         * Create expression that returns count of integer bits that are set to 1.
         * Requires server version 5.6.0+
         */
        public static function intCount(mixed $exp): \FilterExpression {}

        /**
         * Create expression that scans integer bits from left (most significant bit) to
         * right (least significant bit), looking for a search bit value. When the
         * search value is found, the index of that bit (where the most significant bit is
         * index 0) is returned. If "search" is true, the scan will search for the bit
         * value 1. If "search" is false it will search for bit value 0.
         * Requires server version 5.6.0+.
         */
        public static function intLscan(mixed $value, mixed $search): \FilterExpression {}

        /**
         * Create expression that scans integer bits from right (least significant bit) to
         * left (most significant bit), looking for a search bit value. When the
         * search value is found, the index of that bit (where the most significant bit is
         * index 0) is returned. If "search" is true, the scan will search for the bit
         * value 1. If "search" is false it will search for bit value 0.
         * Requires server version 5.6.0+.
         */
        public static function intRscan(mixed $value, mixed $search): \FilterExpression {}

        /**
         * Create expression that returns the minimum value in a variable number of expressions.
         * All arguments must be the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function min(array $exps): \FilterExpression {}

        /**
         * Create expression that returns the maximum value in a variable number of expressions.
         * All arguments must be the same type (integer or float).
         * Requires server version 5.6.0+.
         */
        public static function max(array $exps): \FilterExpression {}

        /**
         * Conditionally select an expression from a variable number of expression pairs
         * followed by default expression action.
         * Requires server version 5.6.0+.
         * ```
         * // Args Format: bool exp1, action exp1, bool exp2, action exp2, ..., action-default
         * // Apply operator based on type.
         */
        public static function cond(array $exps): \FilterExpression {}

        /**
         * Define variables and expressions in scope.
         * Requires server version 5.6.0+.
         * ```
         * // 5 < a < 10
         */
        public static function expLet(array $exps): \FilterExpression {}

        /**
         * Assign variable to an expression that can be accessed later.
         * Requires server version 5.6.0+.
         * ```
         * // 5 < a < 10
         */
        public static function def(string $name, mixed $value): \FilterExpression {}

        /**
         * Retrieve expression value from a variable.
         * Requires server version 5.6.0+.
         */
        public static function var(string $name): \FilterExpression {}

        /**
         * Create unknown value. Used to intentionally fail an expression.
         * The failure can be ignored with `ExpWriteFlags` `EVAL_NO_FAIL`
         * or `ExpReadFlags` `EVAL_NO_FAIL`.
         * Requires server version 5.6.0+.
         */
        public static function unknown(): \FilterExpression {}
    }

    class BatchPolicy {
        public $concurrency;

        public $allow_inline;

        public $send_set_name;

        public $filter_expression;

        public $base_policy;

        public function __construct() {}

        public function getBasePolicy(): \BasePolicyWrapper {}

        public function setBasePolicy(mixed $base_policy) {}

        public function getConcurrency(): \Concurrency {}

        public function setConcurrency(mixed $concurrency) {}

        public function getAllowInline(): bool {}

        public function setSendSetName(bool $send_set_name) {}

        public function getSendSetName(): bool {}

        public function setAllowInline(bool $allow_inline) {}

        public function getFilterExpression(): ?\FilterExpression {}

        public function setFilterExpression(?mixed $filter_expression) {}
    }

    /* `WritePolicy` encapsulates parameters for all write operations. */
    class WritePolicy {
        public $generation_policy;

        public $expiration;

        public $durable_delete;

        public $filter_expression;

        public $commit_level;

        public $generation;

        public $respond_per_each_op;

        public $base_policy;

        public $send_key;

        public $record_exists_action;

        public function __construct() {}

        public function getBasePolicy(): \BasePolicyWrapper {}

        public function setBasePolicy(mixed $base_policy) {}

        public function getRecordExistsAction(): \RecordExistsAction {}

        public function setRecordExistsAction(mixed $record_exists_action) {}

        public function getGenerationPolicy(): \GenerationPolicy {}

        public function setGenerationPolicy(mixed $generation_policy) {}

        public function getCommitLevel(): \CommitLevel {}

        public function setCommitLevel(mixed $commit_level) {}

        public function getGeneration(): int {}

        public function setGeneration(int $generation) {}

        public function getExpiration(): \Expiration {}

        public function setExpiration(mixed $expiration) {}

        public function getSendKey(): bool {}

        public function setSendKey(bool $send_key) {}

        public function getRespondPerEachOp(): bool {}

        public function setRespondPerEachOp(bool $respond_per_each_op) {}

        public function getDurableDelete(): bool {}

        public function setDurableDelete(bool $durable_delete) {}

        public function getFilterExpression(): ?\FilterExpression {}

        public function setFilterExpression(?mixed $filter_expression) {}
    }

    /**
     * Container object for a database record.
     */
    class Record {
        public $generation;

        public $bins;

        public $key;

        public function bin(string $name): ?mixed {}

        public function getBins(): ?mixed {}

        public function getGeneration(): ?int {}

        public function getKey(): ?\Key {}
    }

    class RecordExistsAction {
        /**
         * Update means: Create or update record.
         * Merge write command bins with existing bins.
         */
        public static function update(): \RecordExistsAction {}

        /**
         * UpdateOnly means: Update record only. Fail if record does not exist.
         * Merge write command bins with existing bins.
         */
        public static function updateOnly(): \RecordExistsAction {}

        /**
         * Replace means: Create or replace record.
         * Delete existing bins not referenced by write command bins.
         * Supported by Aerospike 2 server versions >= 2.7.5 and
         * Aerospike 3 server versions >= 3.1.6.
         */
        public static function replace(): \RecordExistsAction {}

        /**
         * ReplaceOnly means: Replace record only. Fail if record does not exist.
         * Delete existing bins not referenced by write command bins.
         * Supported by Aerospike 2 server versions >= 2.7.5 and
         * Aerospike 3 server versions >= 3.1.6.
         */
        public static function replaceOnly(): \RecordExistsAction {}

        /**
         * CreateOnly means: Create only. Fail if record exists.
         */
        public static function createOnly(): \RecordExistsAction {}
    }

    /* 
    `ReadPolicy` excapsulates parameters for transaction policy attributes
    used in all database operation calls. 
    */
    class ReadPolicy {
        public $max_retries;

        public $filter_expression;

        public $timeout;

        public $priority;

        public function __construct() {}

        public function getPriority(): \Priority {}

        public function setPriority(mixed $priority) {}

        public function getMaxRetries(): ?int {}

        public function setMaxRetries(?int $max_retries) {}

        public function getTimeout(): int {}

        public function setTimeout(int $timeout_in_millis) {}

        public function getFilterExpression(): ?\FilterExpression {}

        public function setFilterExpression(?mixed $filter_expression) {}
    }

    /**
        * Unique record identifier. Records can be identified using a specified namespace, an optional
        * set name and a user defined key which must be uique within a set. Records can also be
        * identified by namespace/digest, which is the combination used on the server.
     */
    class Key {
        public $namespace;

        public $digest;

        public $value;

        public $setname;

        /**
            *Construct a new key given a namespace, a set name and a user key value.
            *# Panics
            *Only integers, strings and blobs (`Vec<u8>`) can be used as user keys. The constructor will
            *panic if any other value type is passed.
        */
        public function __construct(string $namespace, string $set, mixed $key) {}

        public function getNamespace(): string {}

        public function getSetname(): string {}

        public function getValue(): ?mixed {}

        public function getDigest(): ?string {}
    }

    /**
     * `ConsistencyLevel` indicates how replicas should be consulted in a read
     * operation to provide the desired consistency guarantee.
     */
    class ConsistencyLevel {
        /**
         * ConsistencyOne indicates only a single replica should be consulted in
         * the read operation.
         */
        public static function consistencyOne(): \ConsistencyLevel {}

        /**
         * ConsistencyAll indicates that all replicas should be consulted in
         * the read operation.
         */
        public static function consistencyAll(): \ConsistencyLevel {}
    }
}
