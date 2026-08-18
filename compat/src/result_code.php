<?php

/**
 * `ResultCode`, and the other 1.x names with no counterpart in this client.
 *
 * These are the leftovers of the audit: 1.x classes the extension does not register
 * under any spelling, so a PHP file is free to define them. They fall into three
 * groups, and the group a class is in is stated on the class.
 *
 * - **Still exactly as useful.** `ResultCode` — the numbers are the server's, and
 *   `AerospikeException::resultCode()` still returns one of them.
 * - **A different shape here.** `BitwisePolicy`, `HllPolicy`, `BatchRecord`,
 *   `UdfMeta`, `Json` — the information exists, arranged differently, and these
 *   bridge to it.
 * - **Gone, and the reason matters.** `PartitionStatus` — see the class.
 */

declare(strict_types=1);

namespace Aerospike;

/**
 * The server's result codes, and this client's negative client-side ones.
 *
 * Unchanged from 1.x, because they are the protocol's numbers rather than any
 * client's. `AerospikeException::resultCode()` returns one, so:
 *
 * ```php
 * try { $client->get(null, $key); }
 * catch (Aerospike\AerospikeException $e) {
 *     if ($e->resultCode() === Aerospike\ResultCode::KEY_NOT_FOUND_ERROR) { … }
 * }
 * ```
 *
 * This client also has `Aerospike\Status`, a PHP enum over the same numbers. That is
 * the better thing to compare against in new code; this exists because a
 * `class_alias` onto an enum yields a name whose `::KEY_NOT_FOUND_ERROR` does not
 * resolve, so old code needs the constants.
 */
class ResultCode
{
    // Client-side, negative — raised here rather than by the server.
    public const GRPC_ERROR = -21;
    public const BATCH_FAILED = -20;
    public const NO_RESPONSE = -19;
    public const NETWORK_ERROR = -18;
    public const COMMON_ERROR = -17;
    public const MAX_RETRIES_EXCEEDED = -16;
    public const MAX_ERROR_RATE = -15;
    public const RACK_NOT_DEFINED = -13;
    public const INVALID_CLUSTER_PARTITION_MAP = -12;
    public const SERVER_NOT_AVAILABLE = -11;
    public const CLUSTER_NAME_MISMATCH_ERROR = -10;
    public const RECORDSET_CLOSED = -9;
    public const NO_AVAILABLE_CONNECTIONS_TO_NODE = -8;
    public const TYPE_NOT_SUPPORTED = -7;
    public const COMMAND_REJECTED = -6;
    public const QUERY_TERMINATED = -5;
    public const SCAN_TERMINATED = -4;
    public const INVALID_NODE_ERROR = -3;
    public const PARSE_ERROR = -2;
    public const SERIALIZE_ERROR = -1;

    // The server's.
    public const OK = 0;
    public const SERVER_ERROR = 1;
    public const KEY_NOT_FOUND_ERROR = 2;
    public const GENERATION_ERROR = 3;
    public const PARAMETER_ERROR = 4;
    public const KEY_EXISTS_ERROR = 5;
    public const BIN_EXISTS_ERROR = 6;
    public const CLUSTER_KEY_MISMATCH = 7;
    public const SERVER_MEM_ERROR = 8;
    public const TIMEOUT = 9;
    public const ALWAYS_FORBIDDEN = 10;
    public const PARTITION_UNAVAILABLE = 11;
    public const BIN_TYPE_ERROR = 12;
    public const RECORD_TOO_BIG = 13;
    public const KEY_BUSY = 14;
    public const SCAN_ABORT = 15;
    public const UNSUPPORTED_FEATURE = 16;
    public const BIN_NOT_FOUND = 17;
    public const DEVICE_OVERLOAD = 18;
    public const KEY_MISMATCH = 19;
    public const INVALID_NAMESPACE = 20;
    public const BIN_NAME_TOO_LONG = 21;
    public const FAIL_FORBIDDEN = 22;
    public const FAIL_ELEMENT_NOT_FOUND = 23;
    public const FAIL_ELEMENT_EXISTS = 24;
    public const ENTERPRISE_ONLY = 25;
    public const OP_NOT_APPLICABLE = 26;
    public const FILTERED_OUT = 27;
    public const LOST_CONFLICT = 28;
    public const QUERY_END = 50;
    public const SECURITY_NOT_SUPPORTED = 51;
    public const SECURITY_NOT_ENABLED = 52;
    public const SECURITY_SCHEME_NOT_SUPPORTED = 53;
    public const INVALID_COMMAND = 54;
    public const INVALID_FIELD = 55;
    public const ILLEGAL_STATE = 56;
    public const INVALID_USER = 60;
    public const USER_ALREADY_EXISTS = 61;
    public const INVALID_PASSWORD = 62;
    public const EXPIRED_PASSWORD = 63;
    public const FORBIDDEN_PASSWORD = 64;
    public const INVALID_CREDENTIAL = 65;
    public const EXPIRED_SESSION = 66;
    public const INVALID_ROLE = 70;
    public const ROLE_ALREADY_EXISTS = 71;
    public const INVALID_PRIVILEGE = 72;
    public const INVALID_WHITELIST = 73;
    public const QUOTAS_NOT_ENABLED = 74;
    public const INVALID_QUOTA = 75;
    public const NOT_AUTHENTICATED = 80;
    public const ROLE_VIOLATION = 81;
    public const NOT_WHITELISTED = 82;
    public const QUOTA_EXCEEDED = 83;
    public const UDF_BAD_RESPONSE = 100;
    public const BATCH_DISABLED = 150;
    public const BATCH_MAX_REQUESTS_EXCEEDED = 151;
    public const BATCH_QUEUES_FULL = 152;
    public const GEO_INVALID_GEOJSON = 160;
    public const INDEX_FOUND = 200;
    public const INDEX_NOT_FOUND = 201;
    public const INDEX_OOM = 202;
    public const INDEX_NOT_READABLE = 203;
    public const INDEX_GENERIC = 204;
    public const INDEX_NAME_MAX_LEN = 205;
    public const INDEX_MAX_COUNT = 206;
    public const QUERY_ABORTED = 210;
    public const QUERY_QUEUE_FULL = 211;
    public const QUERY_TIMEOUT = 212;
    public const QUERY_GENERIC = 213;
    public const QUERY_NET_IO_ERR = 214;
    public const QUERY_DUPLICATE = 215;
    public const AEROSPIKE_ERR_UDF_NOT_FOUND = 1301;
    public const AEROSPIKE_ERR_LUA_FILE_NOT_FOUND = 1302;
}

/** 1.x bitwise write flags, as the integers the server uses. */
class BitwiseWriteFlags
{
    public const DEFAULT = 0;
    public const CREATE_ONLY = 1;
    public const UPDATE_ONLY = 2;
    public const NO_FAIL = 4;
    public const PARTIAL = 8;

    public static function Default(): int
    {
        return self::DEFAULT;
    }

    public static function Create_Only(): int
    {
        return self::CREATE_ONLY;
    }

    public static function Update_Only(): int
    {
        return self::UPDATE_ONLY;
    }

    public static function No_Fail(): int
    {
        return self::NO_FAIL;
    }

    public static function Partial(): int
    {
        return self::PARTIAL;
    }
}

/**
 * 1.x `BitwisePolicy`, which carried a write-flag bitmask.
 *
 * This client's bitwise operations take the flags where they are used, so this holds
 * the mask and {@see \Aerospike\Compat\BitwiseOp} reads it off. Passing `null` where
 * 1.x wanted one of these works too, which is what most 1.x code did.
 */
class BitwisePolicy
{
    public function __construct(public int $flags = BitwiseWriteFlags::DEFAULT)
    {
    }

    public function getFlags(): int
    {
        return $this->flags;
    }
}

/**
 * 1.x `HllPolicy`, which carried a write-flag bitmask.
 *
 * As {@see BitwisePolicy}, for the HyperLogLog operations.
 */
class HllPolicy
{
    public function __construct(public int $flags = HllWriteFlags::DEFAULT)
    {
    }

    public function getFlags(): int
    {
        return $this->flags;
    }
}

/**
 * 1.x `UdfMeta`, one entry of a `listUdf()` answer.
 *
 * This client returns `Aerospike\UdfModule` objects carrying the same three facts.
 * This wraps one so old getters work; {@see \Aerospike\Compat\Client::listUdf()}
 * wraps what it returns.
 */
class UdfMeta
{
    public function __construct(private UdfModule $inner)
    {
    }

    public function getPackageName(): string
    {
        return $this->inner->name();
    }

    public function getHash(): string
    {
        return $this->inner->hash();
    }

    public function getLanguage(): mixed
    {
        return $this->inner->language();
    }

    /** The wrapped `UdfModule`, for code that has moved on. */
    public function inner(): UdfModule
    {
        return $this->inner;
    }
}

/**
 * 1.x `BatchRecord`, the key-and-record pair a batch answer carried.
 *
 * This client returns `Aerospike\BatchResult` per row, which carries the record, the
 * result code, the in-doubt flag and the server's message — everything 1.x had except
 * the key, because a reply row does not contain one. The rows come back in the order
 * the commands were sent, so {@see \Aerospike\Compat\Client::batch()} pairs each
 * result with the key from the command at the same index and hands back these.
 *
 * Anything not defined here is forwarded to the wrapped `BatchResult`, so
 * `isOk()`, `resultCode()`, `isInDoubt()` and `message()` work on one of these too.
 */
class BatchRecord
{
    public function __construct(private ?Key $key, private BatchResult $inner)
    {
    }

    /** The key this row was for, or `null` if the command did not expose one. */
    public function getKey(): ?Key
    {
        return $this->key;
    }

    public function getRecord(): ?Record
    {
        return $this->inner->record();
    }

    /** The result code, with `0` for success as 1.x reported it. */
    public function getResultCode(): int
    {
        return $this->inner->resultCode() ?? ResultCode::OK;
    }

    /** The wrapped `BatchResult`, for code that has moved on. */
    public function inner(): BatchResult
    {
        return $this->inner;
    }

    public function __call(string $name, array $arguments): mixed
    {
        return $this->inner->$name(...$arguments);
    }
}

/**
 * 1.x `Json`, a bin value written as a map.
 *
 * This client writes a PHP array as a map or a list depending on its keys, and writes
 * an associative array as a map — which is what this meant. So this is a thin
 * identity: construct one and `getValue()` gives the array back, and passing the
 * array straight to `put()` does the same thing.
 */
class Json
{
    public function __construct(private array $value = [])
    {
    }

    public function getValue(): array
    {
        return $this->value;
    }
}

/**
 * 1.x `PartitionStatus`, which let a caller resume a scan from where it stopped.
 *
 * **This client does not resume from one of these**, and that is a consequence of the
 * process model rather than an omission. The traversal's position lives in the
 * daemon, as a cursor the `RecordSet` holds a handle to, because a PHP request does
 * not outlive the traversal and the daemon does. So a partially consumed scan is
 * resumed by keeping the `RecordSet`, not by carrying its per-partition state through
 * PHP and back.
 *
 * The class is here so old code that constructed or type-hinted one still loads, and
 * it still answers the three questions it answered: which partition, what digest, and
 * whether it needs retrying. What it will not do is change a scan's starting point.
 * Code that relied on that has to keep the `RecordSet` instead — see the extension's
 * notes on cursors, including that an **empty page is not the end**; only a closed
 * cursor is.
 */
class PartitionStatus
{
    public function __construct(
        private int $id,
        private ?int $bval = null,
        private string $digest = '',
        private bool $retry = false,
    ) {
    }

    public function getPartitionId(): int
    {
        return $this->id;
    }

    public function getBval(): ?int
    {
        return $this->bval;
    }

    public function getDigest(): string
    {
        return $this->digest;
    }

    public function getRetry(): bool
    {
        return $this->retry;
    }
}
