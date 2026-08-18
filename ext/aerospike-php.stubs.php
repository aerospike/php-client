<?php

// Stubs for aerospike-php

namespace Aerospike {
    /**
     * How an abort ended.
     *
     * As with a commit, every case means the transaction's writes are not going to
     * land; the differences are only in how much tidying the server was left.
     */
    enum AbortStatus: string {
    /**
     * Aborted, and everything was tidied up.
     */
      case Ok = 'OK';
    /**
     * It had already been aborted.
     */
      case AlreadyAborted = 'ALREADY_ABORTED';
    /**
     * Aborted, but rolling the writes back was abandoned. The server will finish.
     */
      case RollBackAbandoned = 'ROLL_BACK_ABANDONED';
    /**
     * Rolled back, but closing the monitor record was abandoned.
     */
      case CloseAbandoned = 'CLOSE_ABANDONED';
    }

    /**
     * Per-call settings for a cluster-management command.
     *
     * Mirrors `aerospike-core`'s `AdminPolicy`, which carries **one** field: the
     * socket timeout for the info command the operation is built on. That is not an
     * oversight to be padded out — registering a UDF or creating an index is one
     * short info exchange with one node, so retries, replica choice and record
     * filters have nothing to act on.
     *
     * ```php
     * $client->createIndexOnBin(new Aerospike\AdminPolicy(timeoutMs: 5_000), …);
     * $client->truncate(null, 'test', 'users', null);   // the daemon's default
     * ```
     *
     * It is a class rather than a bare `?int` so that every command in this API takes
     * **a policy object in first position**, whichever kind of policy it is — and so
     * that a second field, if the client ever grows one, is not a change to every
     * signature.
     */
    class AdminPolicy {
        /**
         * Build one. An omitted timeout leaves the daemon's own setting alone.
         *
         * @param int|null $timeoutMs
         */
        public function __construct(?int $timeoutMs = null) {}

        /**
         * Socket timeout for the command's info exchange, or `null` for the
         * daemon's default.
         *
         * @return int|null
         */
        public function timeoutMs(): ?int {}
    }

    /**
     * Every failure this extension raises.
     *
     * `getStatus()` returns an `Aerospike\Status` — a real PHP enum, so it can be
     * `match`ed exhaustively — which classifies *whose* problem the failure is.
     * `getResultCode()` carries the server's own result code when the daemon
     * attached one, which is the number Aerospike's documentation lists.
     *
     * `getCode()`, which `Exception` declares, is the server result code when
     * there is one and the numeric status otherwise. Prefer the two accessors
     * above: they say which of the two you are looking at.
     */
    class AerospikeException extends \Exception {
        /**
         * Shadows `Exception`'s `code`, for the same reason as `message`.
         *
         * @var int
         */
        public $code;

        /**
         * Whether a failed write may nevertheless have been applied. A `true`
         * here means a non-idempotent operation must not simply be retried.
         *
         * @var bool
         */
        public bool $inDoubt;

        /**
         * Shadows the `message` that `Exception` declares.
         *
         * `Exception::getMessage()` is `final`, and its declared slot is
         * `protected`, so writing that slot from Rust fails Zend's access check.
         * Registering `message` as a property of *this* class instead means
         * ext-php-rs' property handler answers the read before it ever reaches
         * the parent's slot — which is the pattern ext-php-rs documents for
         * stateful exceptions, and keeps `getMessage()` working.
         *
         * @var string
         */
        public $message;

        /**
         * The server's own result code, when the daemon attached one.
         *
         * @var int|null
         */
        public ?int $resultCode = null;

        /**
         * How the failure is classified.
         *
         * A property as well as an accessor, so `var_dump($e)` shows it: an
         * exception whose classification is only reachable through a method call
         * is one nobody sees while debugging.
         *
         * Wrapped in [`Case`] because that is what converts an enum case to PHP
         * without corrupting its reference count; see [`Case`] for the crash that
         * caused.
         *
         * @var \Aerospike\Status
         */
        public \Aerospike\Status $status;

        /**
         * Construct one by hand.
         *
         * The extension always throws fully-populated instances; this exists so
         * that application code can rethrow, and tests can construct, an
         * instance of this class. An instance built here reports
         * [`Status::Client`], since no daemon status applies to it.
         *
         * Unlike `Exception::__construct` there is no `$previous` argument:
         * accepting one and silently dropping it would be worse than not
         * offering it.
         *
         * @param string|null $message
         * @param int|null $code
         */
        public function __construct(?string $message = null, ?int $code = null) {}

        /**
         * The server's result code, or `null` when the failure did not come
         * from the server.
         *
         * @return int|null
         */
        public function getResultCode(): ?int {}

        /**
         * How the failure is classified: an `Aerospike\Status`.
         *
         * @return \Aerospike\Status
         */
        public function getStatus(): \Aerospike\Status {}

        /**
         * Whether a failed write may still have been applied.
         *
         * @return bool
         */
        public function isInDoubt(): bool {}
    }

    /**
     * A batch delete row.
     */
    class BatchDelete {
        public function __construct() {}

        /**
         * Delete one record.
         *
         * @param \Aerospike\Key $key
         * @param \Aerospike\GenerationPolicy|null $generationPolicy
         * @param int|null $generation
         * @param \Aerospike\CommitLevel|null $commitLevel
         * @param bool|null $sendKey
         * @param bool|null $durableDelete
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @return \Aerospike\BatchRow
         */
        public static function key(\Aerospike\Key $key, ?\Aerospike\GenerationPolicy $generationPolicy = null, ?int $generation = null, ?\Aerospike\CommitLevel $commitLevel = null, ?bool $sendKey = null, ?bool $durableDelete = null, ?string $filter = null, ?\Aerospike\Expression $filterExp = null): \Aerospike\BatchRow {}
    }

    /**
     * A batch read row.
     */
    class BatchRead {
        public function __construct() {}

        /**
         * Read every bin of a record.
         *
         * @param \Aerospike\Key $key
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @return \Aerospike\BatchRow
         */
        public static function all(\Aerospike\Key $key, ?string $filter = null, ?\Aerospike\Expression $filterExp = null): \Aerospike\BatchRow {}

        /**
         * Read no bins — the record's metadata only.
         *
         * Useful for asking "which of these keys exist" in one round trip.
         *
         * @param \Aerospike\Key $key
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @return \Aerospike\BatchRow
         */
        public static function header(\Aerospike\Key $key, ?string $filter = null, ?\Aerospike\Expression $filterExp = null): \Aerospike\BatchRow {}

        /**
         * Read by running operations, which lets a batch row use the collection
         * reads — `ListOp::getByIndex`, `MapOp::getByKey` and the rest.
         *
         * Every operation must be a read. A write here is refused rather than sent
         * as a read that quietly does nothing.
         *
         * @param \Aerospike\Key $key
         * @param array $ops
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @return \Aerospike\BatchRow
         */
        public static function ops(\Aerospike\Key $key, array $ops, ?string $filter = null, ?\Aerospike\Expression $filterExp = null): \Aerospike\BatchRow {}

        /**
         * Read the named bins.
         *
         * @param \Aerospike\Key $key
         * @param array $bins
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @return \Aerospike\BatchRow
         */
        public static function some(\Aerospike\Key $key, array $bins, ?string $filter = null, ?\Aerospike\Expression $filterExp = null): \Aerospike\BatchRow {}
    }

    /**
     * One row's answer.
     *
     * There is no constructor: a result describes what a server did, and one
     * nobody asked for would be a fiction.
     */
    class BatchResult {
        public function __construct() {}

        /**
         * The 1.x spelling of [`BatchResult::record`].
         *
         * The 1.x client returned a `BatchRecord` per row, whose `getRecord()` is this.
         * Its `getKey()` has no counterpart here, because a reply row does not carry the
         * key — the rows come back in the order they were sent, so the key is the one on
         * the command at the same index. `Aerospike\Compat\Client::batch()` pairs them
         * up for you.
         *
         * @return \Aerospike\Record|null
         */
        public function getRecord(): ?\Aerospike\Record {}

        /**
         * The 1.x spelling of [`BatchResult::result_code`].
         *
         * Note that 1.x reported success as `0` rather than as `null`, so this returns
         * `0` where [`BatchResult::result_code`] returns `null` — old code compared it
         * against `ResultCode::OK`.
         *
         * @return int
         */
        public function getResultCode(): int {}

        /**
         * Whether a failed **write** row may nevertheless have been applied.
         *
         * Never true for a read row. Do not blindly retry a non-idempotent row
         * when this is true.
         *
         * @return bool
         */
        public function isInDoubt(): bool {}

        /**
         * Whether this row succeeded.
         *
         * **Check this before reading the record.** A batch does not throw for a
         * row that failed, because a failure belongs to the row and not to the
         * batch.
         *
         * @return bool
         */
        public function isOk(): bool {}

        /**
         * The server's explanation, when it sent one.
         *
         * @return string|null
         */
        public function message(): ?string {}

        /**
         * What this row read, or `null`.
         *
         * `null` means one of two things, told apart by [`BatchResult::is_ok`]: a
         * row that failed, or a read row that found no record.
         *
         * @return \Aerospike\Record|null
         */
        public function record(): ?\Aerospike\Record {}

        /**
         * The server's result code for this row, or `null` when it succeeded.
         *
         * The numbers Aerospike's own documentation lists: 2 for a missing record,
         * 27 for one a filter rejected, 3 for a generation mismatch.
         *
         * @return int|null
         */
        public function resultCode(): ?int {}
    }

    /**
     * One row of a batch.
     *
     * Built by the static methods of [`BatchRead`], [`BatchWrite`],
     * [`BatchDelete`] and [`BatchUdf`] — one class per kind, so the arguments a
     * kind needs are the arguments its methods take.
     */
    class BatchRow {
        public function __construct() {}

        /**
         * Whether this row writes.
         *
         * Only a write row can ever be reported in doubt, so this is worth being
         * able to check before sending.
         *
         * @return bool
         */
        public function isWrite(): bool {}

        /**
         * The record this row names.
         *
         * @return \Aerospike\Key
         */
        public function key(): \Aerospike\Key {}
    }

    /**
     * A batch UDF row.
     */
    class BatchUdf {
        public function __construct() {}

        /**
         * Call a registered UDF on one record.
         *
         * `$package` is the module's registered name without the `.lua`. The
         * function must already be registered on the cluster; registering one is
         * not part of this API yet.
         *
         * @param \Aerospike\Key $key
         * @param string $package
         * @param string $function
         * @param array|null $args
         * @param \Aerospike\CommitLevel|null $commitLevel
         * @param \Aerospike\Expiration|null $expiration
         * @param bool|null $sendKey
         * @param bool|null $durableDelete
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @return \Aerospike\BatchRow
         */
        public static function call(\Aerospike\Key $key, string $package, string $function, ?array $args = null, ?\Aerospike\CommitLevel $commitLevel = null, ?\Aerospike\Expiration $expiration = null, ?bool $sendKey = null, ?bool $durableDelete = null, ?string $filter = null, ?\Aerospike\Expression $filterExp = null): \Aerospike\BatchRow {}
    }

    /**
     * A batch write row.
     */
    class BatchWrite {
        public function __construct() {}

        /**
         * Write by running operations against one record.
         *
         * The operations run in the order given, atomically, exactly as in
         * `operate()` — a batch write row *is* an `operate` the batch carries.
         *
         * @param \Aerospike\Key $key
         * @param array $ops
         * @param \Aerospike\RecordExistsAction|null $recordExistsAction
         * @param \Aerospike\GenerationPolicy|null $generationPolicy
         * @param int|null $generation
         * @param \Aerospike\Expiration|null $expiration
         * @param \Aerospike\CommitLevel|null $commitLevel
         * @param bool|null $sendKey
         * @param bool|null $durableDelete
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @return \Aerospike\BatchRow
         */
        public static function ops(\Aerospike\Key $key, array $ops, ?\Aerospike\RecordExistsAction $recordExistsAction = null, ?\Aerospike\GenerationPolicy $generationPolicy = null, ?int $generation = null, ?\Aerospike\Expiration $expiration = null, ?\Aerospike\CommitLevel $commitLevel = null, ?bool $sendKey = null, ?bool $durableDelete = null, ?string $filter = null, ?\Aerospike\Expression $filterExp = null): \Aerospike\BatchRow {}
    }

    /**
     * One bin: a name and a value.
     *
     * The value is converted when the bin is constructed, not when the command
     * runs, so `new Bin("avatar", $resource)` fails naming `avatar` instead of
     * failing later with a position inside a list of bins.
     *
     * ```php
     * $client->put(null, $key, [
     *     new Aerospike\Bin("name", "Alice"),
     *     new Aerospike\Bin("age", 30),
     *     new Aerospike\Bin("tags", ["a", "b"]),          // a list
     *     new Aerospike\Bin("prefs", ["theme" => "dark"]), // a map
     *     new Aerospike\Bin("retired", null),              // deletes the bin
     * ]);
     * ```
     */
    class Bin {
        /**
         * Name a bin and its value.
         *
         * A `null` value is not an omission: writing it **deletes** the bin.
         *
         * @param string $name
         * @param mixed $value
         */
        public function __construct(string $name, mixed $value) {}

        /**
         * The bin name.
         *
         * @return string
         */
        public function name(): string {}

        /**
         * The value, converted back to PHP.
         *
         * @return mixed
         */
        public function value(): mixed {}
    }

    /**
     * Write rules for the bitwise and HyperLogLog operations that write.
     *
     * One class for both families: they carry the same three-way write mode, and
     * differ only in their third flag — `partial` for bits, `allowFold` for
     * sketches. Setting the one that does not apply to the family you are using is
     * simply ignored by that family, which is the one place this collapses two of
     * the client's types into one.
     */
    class BinPolicy {
        /**
         * Build one. Everything is optional; the default creates or overwrites.
         *
         * @param \Aerospike\BinWriteMode|null $writeMode
         * @param bool $noFail
         * @param bool $partial
         * @param bool $allowFold
         */
        public function __construct(?\Aerospike\BinWriteMode $writeMode = null, bool $noFail = false, bool $partial = false, bool $allowFold = false) {}

        /**
         * Whether sketches of different precision may be combined by folding down
         * to the smaller. HyperLogLog only.
         *
         * @return bool
         */
        public function allowFold(): bool {}

        /**
         * Whether a rejected write leaves the operation successful.
         *
         * @return bool
         */
        public function noFail(): bool {}

        /**
         * Whether other operations may commit when this one is rejected. Bitwise
         * only.
         *
         * @return bool
         */
        public function partial(): bool {}
    }

    /**
     * What a bitwise or HyperLogLog write does about the bin already existing.
     *
     * Shared by both families, because both spell the same three-way choice the
     * same way — and because a caller should not have to remember two enums that
     * mean one thing.
     */
    enum BinWriteMode: string {
    /**
     * Create the bin, or overwrite it. The default.
     */
      case Update = 'UPDATE';
    /**
     * Overwrite only; fail if the bin is not there.
     */
      case UpdateOnly = 'UPDATE_ONLY';
    /**
     * Create only; fail if the bin is already there.
     */
      case CreateOnly = 'CREATE_ONLY';
    }

    /**
     * Which bins a read returns: all of them, none of them, or a named few.
     *
     * `aerospike-core` has an enum for this and so does this class, for one
     * reason: the three cases are genuinely different requests, and the shape
     * that used to express them — a nullable array — could not tell "no bins"
     * apart from "an empty list of bins I computed". A read that quietly returns
     * no bins is a bug that surfaces as missing data much later.
     *
     * ```php
     * $client->get(null, $key);                                // every bin
     * $client->get(null, $key, Aerospike\Bins::some(["name", "age"]));
     * $client->get(null, $key, Aerospike\Bins::none());        // metadata only
     * ```
     */
    class Bins {
        public function __construct() {}

        /**
         * Every bin in the record. The same as passing no selector at all.
         *
         * @return \Aerospike\Bins
         */
        public static function all(): \Aerospike\Bins {}

        /**
         * Whether this asks for every bin.
         *
         * @return bool
         */
        public function isAll(): bool {}

        /**
         * Whether this asks for no bins at all.
         *
         * @return bool
         */
        public function isNone(): bool {}

        /**
         * The named bins, or `null` for [`Bins::all`] and [`Bins::none`].
         *
         * @return array|null
         */
        public function names(): ?array {}

        /**
         * No bins: generation and TTL only.
         *
         * Cheaper than reading bins and discarding them, and the reason `exists()`
         * is not the only way to ask a metadata question.
         *
         * @return \Aerospike\Bins
         */
        public static function none(): \Aerospike\Bins {}

        /**
         * Just these bins, given as a list of names.
         *
         * A list rather than PHP variadics: an `int|string ...$names` variadic
         * cannot be typed by the engine — the only variadic form available here is
         * untyped — so an array keeps every name checked as a `string`.
         *
         * Naming no bins is refused rather than treated as [`Bins::none`]: an
         * empty list here is overwhelmingly a list that was *computed* and came
         * out empty, and silently answering with no bins would hide that.
         *
         * @param array $names
         * @return \Aerospike\Bins
         */
        public static function some(array $names): \Aerospike\Bins {}
    }

    /**
     * The bitwise operations: `aerospike-core`'s `operations::bitwise`, method for
     * method.
     *
     * They work on a **blob** bin — `Aerospike\Blob`, not a string — and treat it
     * as a flat field of bits.
     *
     * Watch the units. `resize`, `insert` and `remove` work in whole **bytes**;
     * everything else works in **bits**. The parameter names say which
     * (`$byteOffset` against `$bitOffset`), because that is the mistake this API
     * can least afford: a byte offset used as a bit offset addresses the right
     * blob at the wrong place and succeeds.
     */
    class BitOp {
        public function __construct() {}

        /**
         * Add to the integer held in a bit field.
         *
         * `$overflow` defaults to `BitOverflow::Fail`: a counter that silently
         * wrapped or stuck is worse than one that says it could not.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param int $value
         * @param bool $signed
         * @param \Aerospike\BitOverflow|null $overflow
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function add(string $bin, int $bitOffset, int $bitSize, int $value, bool $signed = false, ?\Aerospike\BitOverflow $overflow = null, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Bitwise AND.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param mixed $value
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function and(string $bin, int $bitOffset, int $bitSize, mixed $value, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Count the set bits in a range.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @return \Aerospike\Operation
         */
        public static function count(string $bin, int $bitOffset, int $bitSize): \Aerospike\Operation {}

        /**
         * Read `$bitSize` bits at `$bitOffset` as a blob.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @return \Aerospike\Operation
         */
        public static function get(string $bin, int $bitOffset, int $bitSize): \Aerospike\Operation {}

        /**
         * Read a bit field as an integer.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param bool $signed
         * @return \Aerospike\Operation
         */
        public static function getInt(string $bin, int $bitOffset, int $bitSize, bool $signed = false): \Aerospike\Operation {}

        /**
         * Insert bytes at `$byteOffset`.
         *
         * @param string $bin
         * @param int $byteOffset
         * @param mixed $value
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function insert(string $bin, int $byteOffset, mixed $value, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * The offset of the first bit equal to `$value`, searching forwards; `-1`
         * if there is none.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param bool $value
         * @return \Aerospike\Operation
         */
        public static function lscan(string $bin, int $bitOffset, int $bitSize, bool $value = true): \Aerospike\Operation {}

        /**
         * Shift a bit field left.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param int $shift
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function lshift(string $bin, int $bitOffset, int $bitSize, int $shift, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Invert `$bitSize` bits at `$bitOffset`.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function not(string $bin, int $bitOffset, int $bitSize, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Bitwise OR into `$bitSize` bits at `$bitOffset`.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param mixed $value
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function or(string $bin, int $bitOffset, int $bitSize, mixed $value, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Remove `$byteSize` bytes at `$byteOffset`.
         *
         * @param string $bin
         * @param int $byteOffset
         * @param int $byteSize
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function remove(string $bin, int $byteOffset, int $byteSize, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Grow or shrink the blob to `$byteSize` bytes.
         *
         * @param string $bin
         * @param int $byteSize
         * @param \Aerospike\BitResize|null $flags
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function resize(string $bin, int $byteSize, ?\Aerospike\BitResize $flags = null, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * The same, searching backwards.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param bool $value
         * @return \Aerospike\Operation
         */
        public static function rscan(string $bin, int $bitOffset, int $bitSize, bool $value = true): \Aerospike\Operation {}

        /**
         * Shift a bit field right.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param int $shift
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function rshift(string $bin, int $bitOffset, int $bitSize, int $shift, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Overwrite `$bitSize` bits at `$bitOffset`.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param mixed $value
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function set(string $bin, int $bitOffset, int $bitSize, mixed $value, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Write an integer into a bit field.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param int $value
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function setInt(string $bin, int $bitOffset, int $bitSize, int $value, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Subtract from the integer held in a bit field.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param int $value
         * @param bool $signed
         * @param \Aerospike\BitOverflow|null $overflow
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function subtract(string $bin, int $bitOffset, int $bitSize, int $value, bool $signed = false, ?\Aerospike\BitOverflow $overflow = null, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Bitwise XOR.
         *
         * @param string $bin
         * @param int $bitOffset
         * @param int $bitSize
         * @param mixed $value
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function xor(string $bin, int $bitOffset, int $bitSize, mixed $value, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}
    }

    /**
     * What `BitOp::add()` and `BitOp::subtract()` do when the result does not fit
     * in the bit field.
     */
    enum BitOverflow: string {
    /**
     * Fail the operation. The default, because a number that silently became a
     * different number is worse than an error.
     */
      case Fail = 'FAIL';
    /**
     * Clamp to the largest or smallest value the field can hold.
     */
      case Saturate = 'SATURATE';
    /**
     * Wrap around, so one past the maximum is the minimum.
     */
      case Wrap = 'WRAP';
        /** The 1.x spelling of a case of this enum. */
        public static function Fail(): \Aerospike\BitOverflow {}

        /** The 1.x spelling of a case of this enum. */
        public static function Saturate(): \Aerospike\BitOverflow {}

        /** The 1.x spelling of a case of this enum. */
        public static function Wrap(): \Aerospike\BitOverflow {}

    }

    /**
     * Which end `BitOp::resize()` changes, and whether it may only grow or shrink.
     *
     * The first case is the one place a case name does *not* mirror the Rust
     * client's: PHP reserves `Default` as an enum case name. `AtEnd` is what the
     * client's `Default` actually means, and it pairs with `FromFront`.
     */
    enum BitResize: string {
    /**
     * Add or remove bytes at the end. The client calls this `Default`.
     */
      case AtEnd = 'DEFAULT';
    /**
     * Add or remove bytes at the front.
     */
      case FromFront = 'FROM_FRONT';
    /**
     * Refuse to shrink.
     */
      case GrowOnly = 'GROW_ONLY';
    /**
     * Refuse to grow.
     */
      case ShrinkOnly = 'SHRINK_ONLY';
        /** The 1.x spelling of a case of this enum. */
        public static function Default(): \Aerospike\BitResize {}

    }

    /**
     * A binary string, written as an Aerospike blob rather than as text.
     *
     * PHP cannot distinguish binary from UTF-8, so a bare PHP string is always
     * written as a text value; wrap it in this to write bytes.
     *
     * ```php
     * $client->put("test", "files", "logo", ["data" => new Aerospike\Blob($bytes)]);
     * $record = $client->get("test", "files", "logo");
     * $bytes  = $record["bins"]["data"]->bytes();   // an Aerospike\Blob
     * ```
     */
    class Blob {
        /**
         * Wrap a PHP string, binary or not.
         *
         * Taking the bytes as a packed binary string rather than as a Rust
         * `String` is the point: a `String` would have to be valid UTF-8, which is
         * exactly the constraint this class exists to escape.
         *
         * @param string $bytes
         */
        public function __construct(string $bytes) {}

        /**
         * The wrapped bytes, so `(string) $blob` and string interpolation work.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * The wrapped bytes, as a PHP string.
         *
         * @return string
         */
        public function bytes(): string {}

        /**
         * How many bytes are wrapped.
         *
         * @return int
         */
        public function length(): int {}
    }

    /**
     * A handle onto one cluster instance owned by the local Aerospike daemon.
     *
     * The extension itself speaks to no database. Every call crosses shared
     * memory to `aerospike-php-daemon`, which owns the real client and the
     * connections; the daemon must be running, and must be **the same version as
     * this extension**, for any method here to succeed.
     *
     * ```php
     * $client = new Aerospike\Client();            // the "default" instance
     * $client = new Aerospike\Client("analytics"); // a named instance
     *
     * $key = new Aerospike\Key("test", "users", "alice");
     * $client->put(null, $key, [
     *     new Aerospike\Bin("name", "Alice"),
     *     new Aerospike\Bin("age", 30),
     * ]);
     *
     * $record = $client->get(null, $key);
     * $record->bin("name");        // "Alice"
     * $record->generation();       // 1
     * ```
     *
     * # The argument order is `aerospike-core`'s
     *
     * Every command takes **the policy first, then the key, then whatever the
     * verb operates on** — `put($policy, $key, $bins)`, exactly as the Rust
     * client's `put(&policy, &key, &bins)`. Code and examples translate between
     * the two clients without reordering anything, which is the point.
     *
     * The policy is nullable, and `null` means "use the daemon's configured
     * settings" — the same thing `&WritePolicy::default()` means in Rust, and the
     * same convention the Java client uses. It is not optional, though: it
     * occupies the first position whether or not you have one to give, because an
     * argument that moves depending on whether it is present is worse than one
     * extra `null`.
     *
     * # How PHP values map to Aerospike values
     *
     * PHP has a single array type — an ordered hash map — where Aerospike has
     * both lists and maps, so writing an array applies the conventional rule,
     * the same one `json_encode` uses:
     *
     * - keys exactly `0, 1, .., n-1` **in that order** become an Aerospike
     *   **list**; the empty array counts as a list
     * - anything else — string keys, gaps, a different order — becomes an
     *   Aerospike **map**, whose keys may be ints or strings
     * - nested arrays recurse
     *
     * `null`, `bool`, `int`, `float` and `string` map to the matching Aerospike
     * types. A PHP string is a byte string with no way to say whether it was meant
     * as text or as binary, so **every bare string is written as a text value**;
     * wrap bytes in `Aerospike\Blob` to write them as binary, and a GeoJSON
     * document in `Aerospike\GeoJson` so the server indexes it as geometry. A
     * string that is not valid UTF-8 is refused rather than written as text.
     *
     * Reading back, those shapes come back **as their wrapper class**, so a value
     * round trips as itself:
     *
     * | Aerospike value | PHP |
     * | --- | --- |
     * | nil | `null` |
     * | bool, int, double, string | `bool`, `int`, `float`, `string` |
     * | blob | `Aerospike\Blob` |
     * | GeoJSON | `Aerospike\GeoJson` |
     * | HyperLogLog | `Aerospike\Hll` |
     * | list | packed array |
     * | map, sorted map | associative array |
     * | several results for one bin | packed array, in operation order |
     * | a map read that returned keys and values | list of `[key, value]` pairs |
     * | a particle type this build cannot decode | `["particle_type" => int, "data" => Aerospike\Blob]` |
     *
     * A **list round trips in order**, because its order is data. A **map comes
     * back in the server's key order**, not in PHP insertion order: Aerospike has
     * no insertion-ordered map type, so an associative array is stored key-ordered
     * and read back that way. This reports that faithfully rather than pretending
     * otherwise — do not rely on the insertion order of a map you wrote, in either
     * direction.
     *
     * Anything else — an object of some other class, a resource — is rejected with
     * an `Aerospike\AerospikeException` naming the bin and the position inside it.
     *
     * # Filters make a call throw
     *
     * `filter` on either policy is an expression in the Aerospike Expression
     * Language, compiled by the server (which needs server 8.1.3 or later). **A
     * record the filter rejects makes the call fail**, on a read as much as on a
     * write: the result code is 27, `FILTERED_OUT`. So "no match" is control flow
     * a caller has to catch:
     *
     * ```php
     * try {
     *     $record = $client->get(new Aerospike\ReadPolicy(filter: '$.age > 21'), $key);
     * } catch (Aerospike\AerospikeException $e) {
     *     if ($e->getResultCode() !== 27) { throw $e; }
     *     $record = null;   // the record exists, but the filter rejected it
     * }
     * ```
     *
     * # Configuration
     *
     * `aerospike.instance` in `php.ini` sets the instance used when the
     * constructor is given none; `aerospike.timeout_ms` sets how long a call
     * waits for the daemon, and `aerospike.spin_iters`,
     * `aerospike.yield_iters`, `aerospike.initial_sleep_us` and
     * `aerospike.max_sleep_us` tune how it waits. All are read when a client is
     * constructed.
     */
    class Client {
        /**
         * Open a client for `instance`, defaulting to `aerospike.instance`.
         *
         * Cheap and non-blocking: nothing is attached until the first call, so
         * a failure to reach the daemon surfaces on `ping()`, `put()` or
         * `get()` rather than here.
         *
         * @param string|null $instance
         */
        public function __construct(?string $instance = null) {}

        /**
         * Add numeric deltas to bins, creating the record or the bins if needed.
         *
         * Every value must be an int or a float — an int adds to an integer bin, a
         * float to a double bin — and mixing the two on one bin is a server error.
         * Deltas may be negative, which is how a counter is decremented.
         *
         * ```php
         * $client->add(null, $key, [
         *     new Aerospike\Bin("views", 1),
         *     new Aerospike\Bin("seconds", 0.5),
         * ]);
         * ```
         *
         * @param \Aerospike\WritePolicy|null $policy
         * @param \Aerospike\Key $key
         * @param array $bins
         * @return void
         */
        public function add(?\Aerospike\WritePolicy $policy, \Aerospike\Key $key, array $bins): void {}

        /**
         * Append to string or blob bins, creating them if needed.
         *
         * Every value must be a string or an `Aerospike\Blob`, and must match the
         * bin's existing type.
         *
         * @param \Aerospike\WritePolicy|null $policy
         * @param \Aerospike\Key $key
         * @param array $bins
         * @return void
         */
        public function append(?\Aerospike\WritePolicy $policy, \Aerospike\Key $key, array $bins): void {}

        /**
         * Run many rows — reads, writes, deletes and UDF calls — in one round trip
         * per node.
         *
         * `$rows` is a list of `Aerospike\BatchRow`, built by the static methods of
         * `Aerospike\BatchRead`, `BatchWrite`, `BatchDelete` and `BatchUdf`. The
         * rows may name any key in any namespace, and each may be a different kind
         * of work — that is what a batch is for.
         *
         * ```php
         * $results = $client->batch(null, [
         *     Aerospike\BatchRead::all($alice),
         *     Aerospike\BatchWrite::ops($bob, [Aerospike\Op::add(new Aerospike\Bin("hits", 1))]),
         *     Aerospike\BatchDelete::key($carol),
         * ]);
         * ```
         *
         * Returns one `Aerospike\BatchResult` per row, **in the order the rows were
         * given**. A row that failed carries its own result code and no record —
         * and the batch as a whole still succeeded, so **check `isOk()` on each row
         * rather than relying on an exception.** Only a failure that stops the
         * batch being sent at all throws.
         *
         * The policy is a `ReadPolicy`: it carries what every row shares —
         * timeouts, retries, replica choice, a filter applied to all of them.
         * Anything that can differ per row belongs on the row.
         *
         * @param \Aerospike\ReadPolicy|null $policy
         * @param array $rows
         * @return array
         */
        public function batch(?\Aerospike\ReadPolicy $policy, array $rows): array {}

        /**
         * Open a multi-record transaction.
         *
         * ```php
         * $txn = $client->beginTransaction();
         * try {
         *     $client->put(new Aerospike\WritePolicy(txn: $txn), $from, [new Aerospike\Bin('balance', 70)]);
         *     $client->put(new Aerospike\WritePolicy(txn: $txn), $to,   [new Aerospike\Bin('balance', 30)]);
         *     $txn->commit();
         * } catch (Throwable $e) {
         *     $txn->abort();
         *     throw $e;
         * }
         * ```
         *
         * Commands join the transaction by carrying it on their policy, and either all
         * of its writes land or none do. **An unfinished transaction rolls back** —
         * `Aerospike\Transaction` aborts itself when it is destroyed, so a request that
         * throws does not leave record locks behind.
         *
         * `$timeoutMs` is how long the **server** holds the transaction open before
         * expiring it itself; `null` uses the server's default. The daemon also aborts
         * one that has been idle too long, which is what bounds the damage a killed
         * worker can do.
         *
         * Needs server **8.0 or later** — refused here, naming the node and its
         * version, if not — and a namespace configured for **strong consistency**,
         * which is the server's to enforce and shows up as a failure on the first
         * write.
         *
         * There is no `Txn` constructor to mirror, because a transaction here has to
         * be opened on a cluster: the daemon holds its read and write sets until the
         * commit, and one built locally would have nowhere to keep them.
         *
         * @param int|null $timeoutMs
         * @return \Aerospike\Transaction
         */
        public function beginTransaction(?int $timeoutMs = null): \Aerospike\Transaction {}

        /**
         * Change a user's password.
         *
         * Changing the password of the user the **daemon** authenticates as will break
         * its connection at the next reconnect, since its own credentials come from its
         * configuration file and are not updated by this.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $user
         * @param string $password
         * @return void
         */
        public function changePassword(?\Aerospike\AdminPolicy $policy, string $user, string $password): void {}

        /**
         * Create a secondary index over a bin.
         *
         * ```php
         * $client->createIndexOnBin(null, 'test', 'users', 'age', 'age_idx', Aerospike\IndexType::Numeric)
         *        ->waitTillComplete(30_000);
         * ```
         *
         * `$collection` says whether the index covers the bin itself or, for a bin
         * holding a collection, its list elements or map keys or values. `$ctx`
         * reaches an index into a *nested* collection, and is a list of
         * `Aerospike\Ctx` steps.
         *
         * Returns an `Aerospike\Task`, because the index then has to be built over
         * every existing record — which on a large set takes minutes, and until it
         * finishes **a query using the index returns incomplete results rather than
         * an error**. So wait, or check the task, before relying on it.
         *
         * The index type must match the bin's contents. A numeric index over a string
         * bin is not an error; it simply indexes nothing, and the query finds no
         * records.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $namespace
         * @param string $set
         * @param string $binName
         * @param string $indexName
         * @param \Aerospike\IndexType $indexType
         * @param \Aerospike\CollectionIndex|null $collection
         * @param array|null $ctx
         * @return \Aerospike\Task
         */
        public function createIndexOnBin(?\Aerospike\AdminPolicy $policy, string $namespace, string $set, string $binName, string $indexName, \Aerospike\IndexType $indexType, ?\Aerospike\CollectionIndex $collection = null, ?array $ctx = null): \Aerospike\Task {}

        /**
         * Create a secondary index over an expression.
         *
         * The index covers whatever the expression produces, so there is no bin to
         * look it up by — **a query must name this index by name**, with
         * `Aerospike\Filter::equalByIndex()` or `rangeByIndex()`.
         *
         * ```php
         * $client->createIndexUsingExpression(
         *     null, 'test', 'users', 'total_idx', Aerospike\IndexType::Numeric, null,
         *     Aerospike\Expression::ael('$.price:INT * $.quantity:INT')
         * );
         * ```
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $namespace
         * @param string $set
         * @param string $indexName
         * @param \Aerospike\IndexType $indexType
         * @param \Aerospike\CollectionIndex|null $collection
         * @param \Aerospike\Expression $expression
         * @return \Aerospike\Task
         */
        public function createIndexUsingExpression(?\Aerospike\AdminPolicy $policy, string $namespace, string $set, string $indexName, \Aerospike\IndexType $indexType, ?\Aerospike\CollectionIndex $collection, \Aerospike\Expression $expression): \Aerospike\Task {}

        /**
         * Create a user the client certificate identifies, with no password.
         *
         * For `PKI` authentication, where the certificate carries the identity. **The
         * daemon has no TLS configuration surface yet**, so it refuses `PKI` auth at
         * startup — a PKI user can be created from here, but this client cannot yet
         * connect *as* one.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $user
         * @param array|null $roles
         * @return void
         */
        public function createPkiUser(?\Aerospike\AdminPolicy $policy, string $user, ?array $roles = null): void {}

        /**
         * Create a role.
         *
         * ```php
         * use Aerospike\{Privilege, PrivilegeCode};
         *
         * $client->createRole(null, 'auditor', [
         *     new Privilege(PrivilegeCode::Read, 'test'),
         * ], ['10.0.0.0/8'], 1000, 0);
         * ```
         *
         * `$allowlist` restricts where a holder may connect from; empty means anywhere.
         * `$readQuota` and `$writeQuota` are records per second, and **`0` means
         * unlimited** — so zero is a value, not an omission.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $role
         * @param array $privileges
         * @param array|null $allowlist
         * @param int|null $readQuota
         * @param int|null $writeQuota
         * @return void
         */
        public function createRole(?\Aerospike\AdminPolicy $policy, string $role, array $privileges, ?array $allowlist = null, ?int $readQuota = null, ?int $writeQuota = null): void {}

        /**
         * Create a user with a password, and optionally some roles.
         *
         * ```php
         * $client->createUser(null, 'alice', 'secret', ['read-write']);
         * ```
         *
         * **Every command in this family needs `security { enable-security true }` on
         * the cluster.** Without it they fail with result code 52,
         * `SecurityNotEnabled` — the server names it exactly, so this client does not
         * probe for it.
         *
         * The password crosses shared memory in the clear, and reaches the server in
         * the clear under `INTERNAL` auth, where the server hashes it. That is no
         * weaker than this process holding the password to begin with, but it is worth
         * knowing: hashing on this side is not an option, because the server decides
         * the hash.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $user
         * @param string $password
         * @param array|null $roles
         * @return void
         */
        public function createUser(?\Aerospike\AdminPolicy $policy, string $user, string $password, ?array $roles = null): void {}

        /**
         * Delete one record, returning whether it existed.
         *
         * `false` means the record was already gone, which is not an error — the
         * end state is the one that was asked for. Set `durableDelete: true` on
         * the policy to leave a tombstone (Enterprise only), which is what stops a
         * deleted record from reappearing after a cold restart.
         *
         * @param \Aerospike\WritePolicy|null $policy
         * @param \Aerospike\Key $key
         * @return bool
         */
        public function delete(?\Aerospike\WritePolicy $policy, \Aerospike\Key $key): bool {}

        /**
         * Drop a secondary index.
         *
         * Returns an `Aerospike\Task` whose completion is the index being *gone*.
         * Dropping one a query is using does not fail the query; it falls back to
         * whatever the server can still do.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $namespace
         * @param string $set
         * @param string $indexName
         * @return \Aerospike\Task
         */
        public function dropIndex(?\Aerospike\AdminPolicy $policy, string $namespace, string $set, string $indexName): \Aerospike\Task {}

        /**
         * Remove a role.
         *
         * Users who held it lose it. Dropping a role that is still granted is allowed:
         * the grant simply disappears with it.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $role
         * @return void
         */
        public function dropRole(?\Aerospike\AdminPolicy $policy, string $role): void {}

        /**
         * Remove a user.
         *
         * Their open connections are not closed, and anything they are running
         * continues — the user simply cannot authenticate again.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $user
         * @return void
         */
        public function dropUser(?\Aerospike\AdminPolicy $policy, string $user): void {}

        /**
         * Run a UDF against one record, and return what it returned.
         *
         * ```php
         * $result = $client->executeUdf(null, $key, 'example', 'bump', ['views', 1]);
         * ```
         *
         * `$package` is the module name **without** its extension — `example` for a
         * module registered as `example.lua`. That asymmetry is the server's.
         *
         * **This is a write, whatever the function does.** The server takes a write
         * lock on the record because it cannot know in advance whether the Lua will
         * change it, which is why the policy is a `WritePolicy` and why a UDF is not a
         * way to do a cheap read.
         *
         * Returns `null` when the function returned nothing. A function that returned
         * Lua `nil` also arrives as `null` — the two are indistinguishable through
         * PHP, as they are through every other client.
         *
         * @param \Aerospike\WritePolicy|null $policy
         * @param \Aerospike\Key $key
         * @param string $package
         * @param string $function
         * @param array|null $args
         * @return mixed
         */
        public function executeUdf(?\Aerospike\WritePolicy $policy, \Aerospike\Key $key, string $package, string $function, ?array $args = null): mixed {}

        /**
         * Whether one record exists.
         *
         * Reads no bins, so this is a metadata-only round trip rather than a
         * `get()` whose result is thrown away.
         *
         * @param \Aerospike\ReadPolicy|null $policy
         * @param \Aerospike\Key $key
         * @return bool
         */
        public function exists(?\Aerospike\ReadPolicy $policy, \Aerospike\Key $key): bool {}

        /**
         * Read one record, or `null` if it does not exist.
         *
         * `$bins` selects what comes back; omitting it reads every bin. See
         * `Aerospike\Bins` for reading a few bins, or none at all.
         *
         * ```php
         * $record = $client->get(null, $key, Aerospike\Bins::some(["name", "age"]));
         * ```
         *
         * The explicit `= null` default is what makes `$bins` *optional* rather
         * than merely nullable, for anything reading the signature — reflection, a
         * generated stub, a static analyser.
         *
         * @param \Aerospike\ReadPolicy|null $policy
         * @param \Aerospike\Key $key
         * @param \Aerospike\Bins|null $bins
         * @return \Aerospike\Record|null
         */
        public function get(?\Aerospike\ReadPolicy $policy, \Aerospike\Key $key, ?\Aerospike\Bins $bins = null): ?\Aerospike\Record {}

        /**
         * Scan a set, or query a secondary index, reading the results a page at a
         * time.
         *
         * One method for both, exactly as `aerospike-core` has one: a
         * `Aerospike\Statement` with no filter visits every record — a scan — and one
         * with a filter uses the index the filter names.
         *
         * ```php
         * $statement = new Aerospike\Statement("test", "users");
         * foreach ($client->query(null, null, $statement) as $record) {
         *     echo $record->bin("name"), "\n";
         * }
         * ```
         *
         * `$partitions` divides the ring, for splitting one traversal between
         * workers; `null` means all of it. The policy carries the page size, the
         * record ceiling and the rate limit — see `Aerospike\QueryPolicy`.
         *
         * Returns an `Aerospike\RecordSet`, which is an `Iterator` and is **readable
         * once**: it is a position in a traversal, not a collection. Abandoning it
         * mid-way is fine — `break` out of the `foreach` and the daemon's cursor is
         * released when the object goes out of scope.
         *
         * The first page is fetched here, so a query whose whole result fits in one
         * page costs a single round trip and leaves nothing open. A failure of that
         * first page throws; a failure of a later one throws from the `foreach`.
         *
         * The first two arguments are **nullable, not optional** — as on every other
         * verb here. PHP cannot give a leading parameter a default when a later one
         * has none, so `query(statement: $s)` is not available however the defaults
         * are declared; pass the two `null`s.
         * A record's metadata, with no bins.
         *
         * The same call as `get($policy, $key, Bins::none())`, under the name the 1.x
         * client used. Worth having as its own verb: reading a generation to guard a
         * write is common, and it should not look like a read that forgot its bins.
         *
         * `null` when the record is absent, as `get()` is.
         *
         * @param \Aerospike\ReadPolicy|null $policy
         * @param \Aerospike\Key $key
         * @return \Aerospike\Record|null
         */
        public function getHeader(?\Aerospike\ReadPolicy $policy, \Aerospike\Key $key): ?\Aerospike\Record {}

        /**
         * Add privileges to a role.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $role
         * @param array $privileges
         * @return void
         */
        public function grantPrivileges(?\Aerospike\AdminPolicy $policy, string $role, array $privileges): void {}

        /**
         * Assign roles to a user.
         *
         * Additive: roles the user already holds stay. Use `revokeRoles()` to take one
         * away.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $user
         * @param array $roles
         * @return void
         */
        public function grantRoles(?\Aerospike\AdminPolicy $policy, string $user, array $roles): void {}

        /**
         * Send info commands to a node, and return its answers.
         *
         * ```php
         * $info = $client->info(null, ['build', 'namespaces']);
         * $info['build'];        // "8.1.3.0"
         * ```
         *
         * The answers come back as an array keyed by command, **in the order the
         * commands were asked** — which matters for the commands whose answer is a
         * long record, since a caller often wants to walk them in order.
         *
         * `$node` names which node to ask, from `nodes()`. Most info commands answer
         * for the whole cluster whichever node is asked; the ones that do not —
         * `statistics`, `latencies` — are exactly the ones worth naming a node for.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param array $commands
         * @param string|null $node
         * @return array
         */
        public function info(?\Aerospike\AdminPolicy $policy, array $commands, ?string $node = null): array {}

        /**
         * Which cluster instance this client talks to.
         *
         * @return string
         */
        public function instance(): string {}

        /**
         * List the UDF modules the cluster holds.
         *
         * The one method here with no counterpart in `aerospike-core` — it has no
         * `list_udf`, so the daemon issues the `udf-list` info command and parses the
         * server's record format, once, rather than leaving every caller to.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @return array
         */
        public function listUdf(?\Aerospike\AdminPolicy $policy = null): array {}

        /**
         * The cluster's nodes, as the daemon's tend loop last saw them.
         *
         * The daemon's view rather than a fresh query, which is the useful answer:
         * this is where a command would be routed right now. A node the daemon has
         * stopped believing in is still listed, with `isActive()` false.
         *
         * @return array
         */
        public function nodes(): array {}

        /**
         * Run several operations against one record, in order, atomically.
         *
         * `$ops` is a list of `Aerospike\Operation`, built by the static methods of
         * `Aerospike\Op` and `Aerospike\ListOp`. The operations run in the order
         * given and nobody else's write can interleave with them, which is the
         * whole reason this exists rather than a sequence of separate calls:
         *
         * ```php
         * $record = $client->operate(null, $key, [
         *     Aerospike\Op::add(new Aerospike\Bin("views", 1)),
         *     Aerospike\ListOp::append("history", $event),
         *     Aerospike\ListOp::size("history"),
         * ]);
         * $record->bin("history");   // the new size, from the last operation
         * ```
         *
         * Returns the results of the operations that produced one, as a `Record`,
         * or `null` when the record does not exist and nothing created it. A call
         * whose operations all write answers with a `Record` holding no bins.
         *
         * **Two reads of the same bin come back as a list**, in operation order,
         * because one bin name cannot hold two answers. That is the server's own
         * shape for it.
         *
         * The policy is a `WritePolicy` whatever the operations are — that is the
         * signature `aerospike-core` has, since `operate` may write. A call that
         * only reads is still checked against the read rules, so a write-only
         * setting on it is refused rather than ignored.
         *
         * @param \Aerospike\WritePolicy|null $policy
         * @param \Aerospike\Key $key
         * @param array $ops
         * @return \Aerospike\Record|null
         */
        public function operate(?\Aerospike\WritePolicy $policy, \Aerospike\Key $key, array $ops): ?\Aerospike\Record {}

        /**
         * Check that the daemon is alive, and learn what it serves.
         *
         * The version it reports is necessarily this extension's own — a daemon of
         * any other version could not have answered, because the shared-memory
         * service they meet on has the version in its name.
         *
         * @return \Aerospike\DaemonInfo
         */
        public function ping(): \Aerospike\DaemonInfo {}

        /**
         * Prepend to string or blob bins, creating them if needed.
         *
         * Every value must be a string or an `Aerospike\Blob`, and must match the
         * bin's existing type.
         *
         * @param \Aerospike\WritePolicy|null $policy
         * @param \Aerospike\Key $key
         * @param array $bins
         * @return void
         */
        public function prepend(?\Aerospike\WritePolicy $policy, \Aerospike\Key $key, array $bins): void {}

        /**
         * Write bins to one record.
         *
         * `$bins` is a list of `Aerospike\Bin`. A bin whose value is `null`
         * **deletes** that bin; bin order is preserved.
         *
         * ```php
         * $client->put(null, $key, [new Aerospike\Bin("age", 31)]);
         * ```
         *
         * @param \Aerospike\WritePolicy|null $policy
         * @param \Aerospike\Key $key
         * @param array $bins
         * @return void
         */
        public function put(?\Aerospike\WritePolicy $policy, \Aerospike\Key $key, array $bins): void {}

        /**
         * @param \Aerospike\QueryPolicy|null $policy
         * @param \Aerospike\PartitionFilter|null $partitions
         * @param \Aerospike\Statement $statement
         * @return \Aerospike\RecordSet
         */
        public function query(?\Aerospike\QueryPolicy $policy, ?\Aerospike\PartitionFilter $partitions, \Aerospike\Statement $statement): \Aerospike\RecordSet {}

        /**
         * Apply a UDF to every record a statement matches, in the background.
         *
         * The server runs it without the client waiting, so this returns an
         * `Aerospike\Task` as soon as the job is accepted. A statement with no filter
         * applies the function to the whole set — which is how a bulk update is done
         * without moving any records to the client.
         *
         * ```php
         * $task = $client->queryExecuteUdf(
         *     null, new Aerospike\Statement('test', 'users'), 'example', 'bump', ['views']
         * );
         * ```
         *
         * **Nothing reports which records failed.** A background job's per-record
         * errors are the server's; the task says only whether the job finished. For
         * work that has to account for each record, use `batch()`.
         *
         * @param \Aerospike\WritePolicy|null $policy
         * @param \Aerospike\Statement $statement
         * @param string $package
         * @param string $function
         * @param array|null $args
         * @return \Aerospike\Task
         */
        public function queryExecuteUdf(?\Aerospike\WritePolicy $policy, \Aerospike\Statement $statement, string $package, string $function, ?array $args = null): \Aerospike\Task {}

        /**
         * Describe one role, or every role.
         *
         * The list includes the server's own built-in roles — `read`, `read-write`,
         * `sys-admin` and the rest — not only the ones created here.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string|null $role
         * @return array
         */
        public function queryRoles(?\Aerospike\AdminPolicy $policy = null, ?string $role = null): array {}

        /**
         * Describe one user, or every user.
         *
         * ```php
         * foreach ($client->queryUsers(null) as $user) {
         *     echo $user->name(), ': ', implode(', ', $user->roles()), "\n";
         * }
         * $alice = $client->queryUsers(null, 'alice')[0] ?? null;
         * ```
         *
         * Naming a user that does not exist is a server error, not an empty list.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string|null $user
         * @return array
         */
        public function queryUsers(?\Aerospike\AdminPolicy $policy = null, ?string $user = null): array {}

        /**
         * Register a UDF module on every node.
         *
         * `$source` is the module's **text**, not a path: the daemon may not share a
         * filesystem with this worker, so a path resolved on its side could register
         * a different file. Read the file here.
         *
         * ```php
         * $task = $client->registerUdf(null, file_get_contents('example.lua'), 'example.lua');
         * $task->waitTillComplete(10_000);
         * ```
         *
         * Returns an `Aerospike\Task`: the call succeeds once one node has taken the
         * module, and it then propagates. **Registering does not overwrite
         * atomically** — during propagation different nodes may briefly run different
         * versions of the module, which matters for a UDF a query is using at the
         * time.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $source
         * @param string $serverPath
         * @param \Aerospike\UdfLanguage|null $language
         * @return \Aerospike\Task
         */
        public function registerUdf(?\Aerospike\AdminPolicy $policy, string $source, string $serverPath, ?\Aerospike\UdfLanguage $language = null): \Aerospike\Task {}

        /**
         * Remove a UDF module from every node.
         *
         * Returns an `Aerospike\Task`, as registering does. A module still referenced
         * by a running background query is removed anyway; the query keeps the copy
         * it started with.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $serverPath
         * @return \Aerospike\Task
         */
        public function removeUdf(?\Aerospike\AdminPolicy $policy, string $serverPath): \Aerospike\Task {}

        /**
         * Take privileges away from a role.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $role
         * @param array $privileges
         * @return void
         */
        public function revokePrivileges(?\Aerospike\AdminPolicy $policy, string $role, array $privileges): void {}

        /**
         * Take roles away from a user.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $user
         * @param array $roles
         * @return void
         */
        public function revokeRoles(?\Aerospike\AdminPolicy $policy, string $user, array $roles): void {}

        /**
         * Every record in a set, or in a whole namespace.
         *
         * A scan **is** a query with no filter — `query()` with a filter-less
         * `Statement` does exactly this — and the 1.x client spelled it as its own
         * method taking the namespace and set directly. Both spellings work; this one
         * saves building a `Statement` for the common case.
         *
         * `$binNames` selects bins by name; `null` reads them all. Records arrive a
         * page at a time, as they do from `query()`.
         *
         * ```php
         * foreach ($client->scan(null, null, 'test', 'users') as $record) { … }
         * ```
         *
         * @param \Aerospike\QueryPolicy|null $policy
         * @param \Aerospike\PartitionFilter|null $partitions
         * @param string $namespace
         * @param string $set
         * @param array|null $binNames
         * @return \Aerospike\RecordSet
         */
        public function scan(?\Aerospike\QueryPolicy $policy, ?\Aerospike\PartitionFilter $partitions, string $namespace, string $set, ?array $binNames = null): \Aerospike\RecordSet {}

        /**
         * Replace a role's address allowlist.
         *
         * **Replaces, not adds** — and an empty array *clears* it, which is how a
         * restriction is removed. There is no other way to say it, so an empty list is
         * accepted here where most of this API refuses one.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $role
         * @param array $allowlist
         * @return void
         */
        public function setAllowlist(?\Aerospike\AdminPolicy $policy, string $role, array $allowlist): void {}

        /**
         * Replace a role's rate quotas, in records per second.
         *
         * **`0` lifts a quota**, so zero is a meaningful value rather than an omission —
         * which is why both arguments are required here.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $role
         * @param int $readQuota
         * @param int $writeQuota
         * @return void
         */
        public function setQuotas(?\Aerospike\AdminPolicy $policy, string $role, int $readQuota, int $writeQuota): void {}

        /**
         * Reset one record's time-to-live, and bump its generation.
         *
         * The new TTL comes from the policy's `expiration`, defaulting to the
         * namespace's. Unlike `delete()` and `exists()` this **throws** when the
         * record is absent: there is nothing to touch, and a silent no-op would
         * leave a caller believing a record's life had been extended.
         *
         * @param \Aerospike\WritePolicy|null $policy
         * @param \Aerospike\Key $key
         * @return void
         */
        public function touch(?\Aerospike\WritePolicy $policy, \Aerospike\Key $key): void {}

        /**
         * Delete every record of a set, or of a whole namespace.
         *
         * ```php
         * $client->truncate(null, 'test', 'users', null);       // the whole set
         * $client->truncate(null, 'test', '', null);            // the whole namespace
         * $client->truncate(null, 'test', 'users', $nanos);     // only older records
         * ```
         *
         * An empty `$set` truncates the namespace. `$beforeNanos` deletes only records
         * last updated before that time, in **nanoseconds since the Unix epoch**
         * (`hrtime()` is monotonic and not this; `time() * 1_000_000_000` is).
         *
         * Returns nothing, and returns quickly: the server marks the set truncated
         * and reclaims the space in the background. Reads stop seeing the records
         * immediately, which is what "deleted" means here.
         *
         * **Not undoable, and not filtered.** There is no per-record condition — that
         * is what a query with a UDF is for.
         *
         * @param \Aerospike\AdminPolicy|null $policy
         * @param string $namespace
         * @param string $set
         * @param int|null $beforeNanos
         * @return void
         */
        public function truncate(?\Aerospike\AdminPolicy $policy, string $namespace, string $set, ?int $beforeNanos = null): void {}
    }

    /**
     * Which part of a collection a secondary index covers.
     *
     * The distinction a query has to make and a scan never does: an index on a bin
     * holding a list can be built over its *elements*, and one on a map over its
     * keys or its values. `Default` is a plain scalar bin.
     *
     * ```php
     * // Records whose "tags" list contains "urgent".
     * Aerospike\Filter::equal("tags", "urgent", Aerospike\CollectionIndex::List);
     * ```
     */
    enum CollectionIndex: string {
    /**
     * A scalar bin: the index covers the bin's own value.
     */
      case Scalar = 'DEFAULT';
    /**
     * List elements.
     *
     * Named `ListElements` in PHP: `list` is a reserved keyword there, so it
     * cannot be a case name however much it would match the Rust variant.
     */
      case ListElements = 'LIST';
    /**
     * Map keys.
     */
      case MapKeys = 'MAP_KEYS';
    /**
     * Map values.
     */
      case MapValues = 'MAP_VALUES';
        /** The 1.x spelling of a case of this enum. */
        public static function Default(): \Aerospike\CollectionIndex {}

        /** The 1.x spelling of a case of this enum. */
        public static function List(): \Aerospike\CollectionIndex {}

        /** The 1.x spelling of a case of this enum. */
        public static function MapKeys(): \Aerospike\CollectionIndex {}

        /** The 1.x spelling of a case of this enum. */
        public static function MapValues(): \Aerospike\CollectionIndex {}

    }

    /**
     * How many replicas must commit before the server answers.
     */
    enum CommitLevel: string {
    /**
     * Master and all replicas.
     */
      case CommitAll = 'COMMIT_ALL';
    /**
     * Master only.
     */
      case CommitMaster = 'COMMIT_MASTER';
        /** The 1.x spelling of a case of this enum. */
        public static function CommitAll(): \Aerospike\CommitLevel {}

        /** The 1.x spelling of a case of this enum. */
        public static function CommitMaster(): \Aerospike\CommitLevel {}

    }

    /**
     * How a commit ended.
     *
     * **Every case is a success**: the transaction committed. The three besides `Ok`
     * say the client left some tidying to the server, which is worth logging and is
     * *not* a reason to commit again.
     */
    enum CommitStatus: string {
    /**
     * Committed, and everything was tidied up.
     */
      case Ok = 'OK';
    /**
     * It had already been committed.
     */
      case AlreadyCommitted = 'ALREADY_COMMITTED';
    /**
     * Committed, but rolling the writes forward was abandoned. The server will
     * finish.
     */
      case RollForwardAbandoned = 'ROLL_FORWARD_ABANDONED';
    /**
     * Committed and rolled forward, but closing the transaction's monitor record
     * was abandoned. The server will.
     */
      case CloseAbandoned = 'CLOSE_ABANDONED';
    }

    /**
     * A step in the path to a collection nested inside a bin.
     *
     * Mirrors the Rust client's `ctx_*` constructors. Pass a list of these to
     * [`Operation::context`].
     *
     * ```php
     * // The list at $record["profile"]["roles"]
     * ListOp::size('profile')->context([Ctx::mapKey('roles')]);
     *
     * // The list at $record["matrix"][0]
     * ListOp::append('matrix', 9)->context([Ctx::listIndex(0)]);
     * ```
     */
    class Ctx {
        public function __construct() {}

        /**
         * The first map entry whose value is this.
         * Every child of the current node — every list element, every map entry.
         *
         * The step that makes a path expression one: the others select a single node,
         * and this fans out so that what follows applies to each child. Needs server
         * 8.1.1 or later, which the daemon checks.
         *
         * ```php
         * // The price of every book, rather than of one book.
         * ExpPath::selectValues(ExpType::ListType, Exp::mapBin('books'),
         *     [Ctx::allChildren(), Ctx::mapKey('price')]);
         * ```
         *
         * @return \Aerospike\Ctx
         */
        public static function allChildren(): \Aerospike\Ctx {}

        /**
         * Every child a filter accepts.
         *
         * The filter runs per child, with `Exp::loopVar()` standing for the child
         * being tested — so it can compare against the child's key, value or index.
         * Needs server 8.1.1 or later.
         *
         * The filter must be a **built** expression: it is evaluated inside a packed
         * tree, and there is nowhere there to put text for the server to parse.
         *
         * @param \Aerospike\Expression $filter
         * @return \Aerospike\Ctx
         */
        public static function allChildrenWithFilter(\Aerospike\Expression $filter): \Aerospike\Ctx {}

        /**
         * The list element at `index`. Negative counts from the end.
         *
         * @param int $index
         * @return \Aerospike\Ctx
         */
        public static function listIndex(int $index): \Aerospike\Ctx {}

        /**
         * The list element at `index`, creating the list if the path is missing.
         *
         * `pad` fills the gap with nils when the index is past the end; without
         * it, an index outside the list is an error.
         *
         * @param int $index
         * @param \Aerospike\ListOrder $order
         * @param bool $pad
         * @return \Aerospike\Ctx
         */
        public static function listIndexCreate(int $index, \Aerospike\ListOrder $order, bool $pad = false): \Aerospike\Ctx {}

        /**
         * The list element at `rank` in value order. Negative counts from the
         * largest.
         *
         * @param int $rank
         * @return \Aerospike\Ctx
         */
        public static function listRank(int $rank): \Aerospike\Ctx {}

        /**
         * The first list element equal to `value`.
         *
         * @param mixed $value
         * @return \Aerospike\Ctx
         */
        public static function listValue(mixed $value): \Aerospike\Ctx {}

        /**
         * The map entry at `index` in key order.
         *
         * @param int $index
         * @return \Aerospike\Ctx
         */
        public static function mapIndex(int $index): \Aerospike\Ctx {}

        /**
         * The map entry with this key.
         *
         * @param mixed $key
         * @return \Aerospike\Ctx
         */
        public static function mapKey(mixed $key): \Aerospike\Ctx {}

        /**
         * The map entry with this key, creating the map if the path is missing.
         *
         * @param mixed $key
         * @param \Aerospike\MapOrder $order
         * @return \Aerospike\Ctx
         */
        public static function mapKeyCreate(mixed $key, \Aerospike\MapOrder $order): \Aerospike\Ctx {}

        /**
         * The map entry at `rank` in value order.
         *
         * @param int $rank
         * @return \Aerospike\Ctx
         */
        public static function mapRank(int $rank): \Aerospike\Ctx {}

        /**
         * @param mixed $value
         * @return \Aerospike\Ctx
         */
        public static function mapValue(mixed $value): \Aerospike\Ctx {}
    }

    /**
     * What the daemon answers `ping()` with.
     *
     * As with [`Record`], there is no constructor: this describes a daemon, and
     * one that no daemon reported would be a fiction.
     */
    class DaemonInfo {
        public function __construct() {}

        /**
         * Every instance the daemon is configured for.
         *
         * The names of the `[cluster.*]` sections in its configuration file, which
         * are what `new Aerospike\Client($instance)` accepts.
         *
         * @return array
         */
        public function instances(): array {}

        /**
         * Whether the daemon serves `$instance`.
         *
         * @param string $instance
         * @return bool
         */
        public function serves(string $instance): bool {}

        /**
         * The daemon's version.
         *
         * Always equal to this extension's, because a daemon of any other version
         * could not have answered: the service name they meet on embeds it.
         *
         * @return string
         */
        public function version(): string {}
    }

    /**
     * The expression builder: `aerospike-core`'s `expressions` module.
     *
     * A namespace of static methods, each returning an `Aerospike\Expression`. See
     * the module documentation for why lists are arrays and why there is no
     * constructor.
     *
     * ```php
     * use Aerospike\Exp;
     *
     * // A record written in the last hour, whose "score" bin is in the top band.
     * Exp::and([
     *     Exp::lt(Exp::sinceUpdate(), Exp::intVal(3_600_000_000_000)),
     *     Exp::ge(Exp::intBin('score'), Exp::intVal(900)),
     * ]);
     * ```
     */
    class Exp {
        public function __construct() {}

        /**
         * All of them.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function and(array $exps): \Aerospike\Expression {}

        /**
         * A bin, read as `expType`.
         *
         * The type is not a hint: the server reads the bin's bytes as the type
         * named, so naming the wrong one compares against nonsense rather than
         * failing. The typed shorthands below are this with the type filled in, and
         * are what most code should use.
         *
         * @param string $name
         * @param \Aerospike\ExpType $expType
         * @return \Aerospike\Expression
         */
        public static function bin(string $name, \Aerospike\ExpType $expType): \Aerospike\Expression {}

        /**
         * Whether a bin is present.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function binExists(string $name): \Aerospike\Expression {}

        /**
         * A bin's particle type, as the integer the server uses.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function binType(string $name): \Aerospike\Expression {}

        /**
         * A byte-string bin.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function blobBin(string $name): \Aerospike\Expression {}

        /**
         * A byte-string literal.
         *
         * @param array $value
         * @return \Aerospike\Expression
         */
        public static function blobVal(array $value): \Aerospike\Expression {}

        /**
         * A boolean bin.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function boolBin(string $name): \Aerospike\Expression {}

        /**
         * A boolean literal.
         *
         * @param bool $value
         * @return \Aerospike\Expression
         */
        public static function boolVal(bool $value): \Aerospike\Expression {}

        /**
         * Condition, result, condition, result, …, default.
         *
         * An **odd** number of expressions: pairs of condition and result, then the
         * default that applies when none matched. An even number is refused here
         * rather than by the server, because the mistake is always the same one — a
         * forgotten default — and the server's answer does not say so.
         *
         * ```php
         * Exp::cond([
         *     Exp::ge(Exp::intBin('score'), Exp::intVal(900)), Exp::stringVal('gold'),
         *     Exp::ge(Exp::intBin('score'), Exp::intVal(500)), Exp::stringVal('silver'),
         *     Exp::stringVal('bronze'),                        // the default
         * ]);
         * ```
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function cond(array $exps): \Aerospike\Expression {}

        /**
         * Define a variable, for use inside a [`let`](Self::let_).
         *
         * @param string $name
         * @param \Aerospike\Expression $value
         * @return \Aerospike\Expression
         */
        public static function def(string $name, \Aerospike\Expression $value): \Aerospike\Expression {}

        /**
         * The record's size on device in bytes. Zero for an in-memory namespace.
         *
         * Superseded by [`record_size`](Self::record_size), which does not depend on
         * where the namespace stores its data. Kept because it is a distinct server
         * opcode and the only one available on older servers.
         *
         * @return \Aerospike\Expression
         */
        public static function deviceSize(): \Aerospike\Expression {}

        /**
         * The record digest modulo `modulo`, for sampling a fraction of a set.
         *
         * `Exp::eq(Exp::digestModulo(100), Exp::intVal(0))` matches about one
         * record in a hundred, and the same records every time — the digest does not
         * change, so a sample taken this way is stable across runs.
         *
         * @param int $modulo
         * @return \Aerospike\Expression
         */
        public static function digestModulo(int $modulo): \Aerospike\Expression {}

        /**
         * Equal.
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function eq(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Exactly one of them — the Java client's spelling of
         * [`xor`](Self::xor).
         *
         * The same server opcode as `xor`, kept because the two names appear in
         * different clients' documentation and a reader of either should find what
         * they expect.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function exclusive(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x client's spelling of [`let`](Self::let_).
         *
         * `let` is what this client calls it — PHP allows the keyword as a method
         * name where Rust does not — and `expLet` is what the previous client had to
         * call it. Both are here so old code reads and new code reads well.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function expLet(array $exps): \Aerospike\Expression {}

        /**
         * A float bin.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function floatBin(string $name): \Aerospike\Expression {}

        /**
         * A float literal.
         *
         * @param float $value
         * @return \Aerospike\Expression
         */
        public static function floatVal(float $value): \Aerospike\Expression {}

        /**
         * Greater than or equal.
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function ge(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * A GeoJSON bin.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function geoBin(string $name): \Aerospike\Expression {}

        /**
         * Whether two GeoJSON regions relate — the documents say how.
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function geoCompare(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * A GeoJSON literal, for [`geo_compare`](Self::geo_compare).
         *
         * @param string $value
         * @return \Aerospike\Expression
         */
        public static function geoVal(string $value): \Aerospike\Expression {}

        /**
         * Greater than.
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function gt(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * A HyperLogLog bin.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function hllBin(string $name): \Aerospike\Expression {}

        /**
         * Whether `value` appears in `list`.
         *
         * One comparison against a list, rather than an `or` of one comparison per
         * element — cheaper on the server and much shorter to write.
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $list
         * @return \Aerospike\Expression
         */
        public static function inList(\Aerospike\Expression $value, \Aerospike\Expression $list): \Aerospike\Expression {}

        /**
         * The value that sorts above every other, for an open-ended range.
         *
         * @return \Aerospike\Expression
         */
        public static function infinity(): \Aerospike\Expression {}

        /**
         * Bitwise AND.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function intAnd(array $exps): \Aerospike\Expression {}

        /**
         * Shift right, preserving the sign bit.
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $shift
         * @return \Aerospike\Expression
         */
        public static function intArshift(\Aerospike\Expression $value, \Aerospike\Expression $shift): \Aerospike\Expression {}

        /**
         * An integer bin.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function intBin(string $name): \Aerospike\Expression {}

        /**
         * Count the set bits.
         *
         * @param \Aerospike\Expression $exp
         * @return \Aerospike\Expression
         */
        public static function intCount(\Aerospike\Expression $exp): \Aerospike\Expression {}

        /**
         * Index of the left-most bit matching `search`.
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $search
         * @return \Aerospike\Expression
         */
        public static function intLscan(\Aerospike\Expression $value, \Aerospike\Expression $search): \Aerospike\Expression {}

        /**
         * Shift left.
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $shift
         * @return \Aerospike\Expression
         */
        public static function intLshift(\Aerospike\Expression $value, \Aerospike\Expression $shift): \Aerospike\Expression {}

        /**
         * Bitwise complement.
         *
         * @param \Aerospike\Expression $exp
         * @return \Aerospike\Expression
         */
        public static function intNot(\Aerospike\Expression $exp): \Aerospike\Expression {}

        /**
         * Bitwise OR.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function intOr(array $exps): \Aerospike\Expression {}

        /**
         * Index of the right-most bit matching `search`.
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $search
         * @return \Aerospike\Expression
         */
        public static function intRscan(\Aerospike\Expression $value, \Aerospike\Expression $search): \Aerospike\Expression {}

        /**
         * Shift right, filling with zeroes.
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $shift
         * @return \Aerospike\Expression
         */
        public static function intRshift(\Aerospike\Expression $value, \Aerospike\Expression $shift): \Aerospike\Expression {}

        /**
         * An integer literal.
         *
         * @param int $value
         * @return \Aerospike\Expression
         */
        public static function intVal(int $value): \Aerospike\Expression {}

        /**
         * Bitwise XOR.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function intXor(array $exps): \Aerospike\Expression {}

        /**
         * Whether this is a tombstone left by a durable delete.
         *
         * @return \Aerospike\Expression
         */
        public static function isTombstone(): \Aerospike\Expression {}

        /**
         * The record's key, read as `expType`.
         *
         * Only present when the record was written with `sendKey`, so
         * [`key_exists`](Self::key_exists) is worth asking first.
         *
         * @param \Aerospike\ExpType $expType
         * @return \Aerospike\Expression
         */
        public static function key(\Aerospike\ExpType $expType): \Aerospike\Expression {}

        /**
         * Whether the record's key was stored with it.
         *
         * A fact about the *write* — `sendKey` on the policy that created the record
         * — not about the record's identity. Every record has a key; not every
         * record has it stored.
         *
         * @return \Aerospike\Expression
         */
        public static function keyExists(): \Aerospike\Expression {}

        /**
         * When the record was last written, in nanoseconds since the Unix epoch.
         *
         * @return \Aerospike\Expression
         */
        public static function lastUpdate(): \Aerospike\Expression {}

        /**
         * Less than or equal.
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function le(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Variable definitions, then the expression that uses them.
         *
         * Every element but the last must be a [`def`](Self::def), and the last is
         * the expression evaluated with those definitions in scope. Worth reaching
         * for when a sub-expression appears more than once: the server evaluates a
         * bound variable once.
         *
         * ```php
         * Exp::let([
         *     Exp::def('total', Exp::numAdd([Exp::intBin('a'), Exp::intBin('b')])),
         *     Exp::gt(Exp::var('total'), Exp::intVal(100)),
         * ]);
         * ```
         *
         * Called `let` here and `exp_let` in the Rust client, where `let` is a
         * keyword. PHP allows it as a method name, so this is the name that reads.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function let(array $exps): \Aerospike\Expression {}

        /**
         * A list bin.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function listBin(string $name): \Aerospike\Expression {}

        /**
         * A list literal, from a PHP array.
         *
         * The array's *values* are used and its keys ignored, which is what makes a
         * PHP list a list. Pass a `SortedMap` or `OrderedMap` for a map literal, or
         * use [`map_val`](Self::map_val).
         *
         * @param mixed $value
         * @return \Aerospike\Expression
         */
        public static function listVal(mixed $value): \Aerospike\Expression {}

        /**
         * The node a fan-out is currently considering, read as `expType`.
         *
         * Only meaningful inside a `Ctx::allChildrenWithFilter()` filter or an
         * `ExpPath` modify expression — outside one there is no node being
         * considered, and the server says so. `part` picks which side of the child is
         * meant: its map key, its value, or its list index.
         *
         * ```php
         * use Aerospike\{Exp, ExpType, LoopVarPart};
         *
         * // Children whose own value exceeds 20.
         * Exp::gt(Exp::loopVar(ExpType::Integer, LoopVarPart::Value), Exp::intVal(20));
         * ```
         *
         * Needs server 8.1.1 or later, like the path expressions it belongs to.
         *
         * @param \Aerospike\ExpType $expType
         * @param \Aerospike\LoopVarPart $part
         * @return \Aerospike\Expression
         */
        public static function loopVar(\Aerospike\ExpType $expType, \Aerospike\LoopVarPart $part): \Aerospike\Expression {}

        /**
         * Less than.
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function lt(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * A map bin.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function mapBin(string $name): \Aerospike\Expression {}

        /**
         * The keys of a map, as a list.
         *
         * @param \Aerospike\Expression $map
         * @return \Aerospike\Expression
         */
        public static function mapKeys(\Aerospike\Expression $map): \Aerospike\Expression {}

        /**
         * A map literal.
         *
         * A plain PHP array with string keys, an `OrderedMap`, or a `SortedMap`.
         * **Pass a `SortedMap` to compare whole maps**: the server compares maps in
         * key order, and only a sorted map is packed with the ordering flag that
         * makes such a comparison meaningful.
         *
         * @param mixed $value
         * @return \Aerospike\Expression
         */
        public static function mapVal(mixed $value): \Aerospike\Expression {}

        /**
         * The values of a map, as a list.
         *
         * @param \Aerospike\Expression $map
         * @return \Aerospike\Expression
         */
        public static function mapValues(\Aerospike\Expression $map): \Aerospike\Expression {}

        /**
         * Largest.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function max(array $exps): \Aerospike\Expression {}

        /**
         * The record's size in memory in bytes. Zero for an on-device namespace.
         *
         * Superseded by [`record_size`](Self::record_size), as
         * [`device_size`](Self::device_size) is.
         *
         * @return \Aerospike\Expression
         */
        public static function memorySize(): \Aerospike\Expression {}

        /**
         * Smallest.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function min(array $exps): \Aerospike\Expression {}

        /**
         * Not equal.
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function ne(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * Nil, the value an absent bin reads as.
         *
         * @return \Aerospike\Expression
         */
        public static function nil(): \Aerospike\Expression {}

        /**
         * Negation.
         *
         * @param \Aerospike\Expression $exp
         * @return \Aerospike\Expression
         */
        public static function not(\Aerospike\Expression $exp): \Aerospike\Expression {}

        /**
         * Absolute value.
         *
         * @param \Aerospike\Expression $value
         * @return \Aerospike\Expression
         */
        public static function numAbs(\Aerospike\Expression $value): \Aerospike\Expression {}

        /**
         * Sum.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function numAdd(array $exps): \Aerospike\Expression {}

        /**
         * Round up.
         *
         * @param \Aerospike\Expression $num
         * @return \Aerospike\Expression
         */
        public static function numCeil(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * Left-to-right quotient.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function numDiv(array $exps): \Aerospike\Expression {}

        /**
         * Round down.
         *
         * @param \Aerospike\Expression $num
         * @return \Aerospike\Expression
         */
        public static function numFloor(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * Logarithm of `num` in `base`.
         *
         * @param \Aerospike\Expression $num
         * @param \Aerospike\Expression $base
         * @return \Aerospike\Expression
         */
        public static function numLog(\Aerospike\Expression $num, \Aerospike\Expression $base): \Aerospike\Expression {}

        /**
         * Remainder of `numerator` divided by `denominator`.
         *
         * @param \Aerospike\Expression $numerator
         * @param \Aerospike\Expression $denominator
         * @return \Aerospike\Expression
         */
        public static function numMod(\Aerospike\Expression $numerator, \Aerospike\Expression $denominator): \Aerospike\Expression {}

        /**
         * Product.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function numMul(array $exps): \Aerospike\Expression {}

        /**
         * `base` raised to `exponent`.
         *
         * @param \Aerospike\Expression $base
         * @param \Aerospike\Expression $exponent
         * @return \Aerospike\Expression
         */
        public static function numPow(\Aerospike\Expression $base, \Aerospike\Expression $exponent): \Aerospike\Expression {}

        /**
         * Left-to-right difference.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function numSub(array $exps): \Aerospike\Expression {}

        /**
         * Any of them.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function or(array $exps): \Aerospike\Expression {}

        /**
         * The record's size in bytes.
         *
         * @return \Aerospike\Expression
         */
        public static function recordSize(): \Aerospike\Expression {}

        /**
         * Match a string bin against a regular expression.
         *
         * The pattern is a literal string rather than an expression, because the
         * server compiles it once. `flags` combines `Aerospike\RegexFlag` cases with
         * `|`; pass `0` for the defaults.
         *
         * ```php
         * use Aerospike\{Exp, RegexFlag};
         * Exp::regexCompare('^a.*z$', RegexFlag::Icase->value, Exp::stringBin('name'));
         * ```
         *
         * @param string $regex
         * @param int $flags
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function regexCompare(string $regex, int $flags, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * "Delete this node", as the result of an `ExpPath` modify expression.
         *
         * A modify expression normally produces the node's new value. Returning this
         * instead removes the node, which is how a conditional removal is written
         * without a second pass over the collection.
         *
         * Needs server 8.1.1 or later.
         *
         * @return \Aerospike\Expression
         */
        public static function removeResult(): \Aerospike\Expression {}

        /**
         * The record's set name, or `""` for a record in no set.
         *
         * @return \Aerospike\Expression
         */
        public static function setName(): \Aerospike\Expression {}

        /**
         * Nanoseconds since the record was last written.
         *
         * @return \Aerospike\Expression
         */
        public static function sinceUpdate(): \Aerospike\Expression {}

        /**
         * A string bin.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function stringBin(string $name): \Aerospike\Expression {}

        /**
         * A string literal.
         *
         * @param string $value
         * @return \Aerospike\Expression
         */
        public static function stringVal(string $value): \Aerospike\Expression {}

        /**
         * Widen an integer to a float.
         *
         * @param \Aerospike\Expression $num
         * @return \Aerospike\Expression
         */
        public static function toFloat(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * Truncate a float to an integer.
         *
         * @param \Aerospike\Expression $num
         * @return \Aerospike\Expression
         */
        public static function toInt(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * Seconds until the record expires. `-1` for a record that never does.
         *
         * @return \Aerospike\Expression
         */
        public static function ttl(): \Aerospike\Expression {}

        /**
         * The value a *write* would have produced.
         *
         * For the expression write operations, where it stands for "whatever the
         * expression evaluates to". Meaningless as a filter, and the server says so.
         *
         * @return \Aerospike\Expression
         */
        public static function unknown(): \Aerospike\Expression {}

        /**
         * A literal, converted the way a bin value is.
         *
         * The one method here that is not in `aerospike-core`: PHP is dynamically
         * typed, so the mapping from a PHP value to an Aerospike one already exists
         * for every bin written by this client, and an expression literal is the
         * same question. Accepts an int, float, string, bool, `null`, an array, or
         * any of `Blob`, `GeoJson`, `Hll`, `Infinity`, `Wildcard`, `OrderedMap` and
         * `SortedMap`.
         *
         * Use the typed constructors below where the type matters and the PHP value
         * does not settle it — a PHP string is a string, never GeoJSON, so
         * `Exp::geoVal()` is how a region is written.
         *
         * @param mixed $value
         * @return \Aerospike\Expression
         */
        public static function val(mixed $value): \Aerospike\Expression {}

        /**
         * A variable an enclosing [`let`](Self::let_) defined.
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function var(string $name): \Aerospike\Expression {}

        /**
         * When the record expires, in nanoseconds since the Unix epoch. Zero for a
         * record that never expires.
         *
         * @return \Aerospike\Expression
         */
        public static function voidTime(): \Aerospike\Expression {}

        /**
         * The value that matches any other, for a selection by example.
         *
         * @return \Aerospike\Expression
         */
        public static function wildcard(): \Aerospike\Expression {}

        /**
         * An odd number of them.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function xor(array $exps): \Aerospike\Expression {}
    }

    /**
     * Bitwise expressions: `aerospike-core`'s `expressions::bitwise`.
     *
     * Operations on a blob bin, bit by bit. Offsets and sizes are expressions, so a
     * filter can read a field whose position is itself stored in the record.
     *
     * No `context()`: a blob has no nested structure to point into, which is why
     * these take no path where the collection families do.
     *
     * ```php
     * use Aerospike\{Exp, ExpBit};
     *
     * // Records whose flags blob has bit 3 set.
     * Exp::eq(
     *     ExpBit::count(Exp::intVal(3), Exp::intVal(1), Exp::blobBin('flags')),
     *     Exp::intVal(1),
     * );
     * ```
     */
    class ExpBit {
        public function __construct() {}

        /**
         * Add to the integer in a bit range.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $value
         * @param bool $signed
         * @param \Aerospike\BitOverflow $action
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function add(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $value, bool $signed, \Aerospike\BitOverflow $action, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * AND a bit range with a value.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function and(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Count the set bits in a bit range.
         *
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function count(\Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Read a bit range as a blob.
         *
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function get(\Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Read a bit range as an integer.
         *
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param bool $signed
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function getInt(\Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, bool $signed, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Insert bytes at a byte offset.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $byteOffset
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function insert(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $byteOffset, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Index of the left-most bit in a range matching `value`.
         *
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function lscan(\Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Shift a bit range left.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $shift
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function lshift(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $shift, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Invert a bit range.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function not(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * OR a bit range with a value.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function or(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Remove bytes at a byte offset.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $byteOffset
         * @param \Aerospike\Expression $byteSize
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function remove(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $byteOffset, \Aerospike\Expression $byteSize, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Grow or shrink to a byte size.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $byteSize
         * @param \Aerospike\BitResize $resizeFlags
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function resize(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $byteSize, \Aerospike\BitResize $resizeFlags, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Index of the right-most bit in a range matching `value`.
         *
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function rscan(\Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Shift a bit range right.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $shift
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function rshift(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $shift, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Overwrite a bit range.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function set(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Write an integer into a bit range.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function setInt(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Subtract from the integer in a bit range.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $value
         * @param bool $signed
         * @param \Aerospike\BitOverflow $action
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function subtract(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $value, bool $signed, \Aerospike\BitOverflow $action, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * XOR a bit range with a value.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $bitOffset
         * @param \Aerospike\Expression $bitSize
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function xor(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $bitOffset, \Aerospike\Expression $bitSize, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}
    }

    /**
     * HyperLogLog expressions: `aerospike-core`'s `expressions::hll`.
     *
     * A sketch answers "how many distinct values" in constant space, with an error
     * bound rather than exactly. The `list` arguments are lists of values to add, or
     * lists of *other sketches* to combine with — each method says which.
     *
     * ```php
     * use Aerospike\{Exp, ExpHll};
     *
     * // Records whose "visitors" sketch estimates more than a thousand.
     * Exp::gt(ExpHll::getCount(Exp::hllBin('visitors')), Exp::intVal(1000));
     * ```
     */
    class ExpHll {
        public function __construct() {}

        /**
         * Add values to a sketch.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function add(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $list, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Add values, creating the sketch with a precision if it does not exist.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $indexBitCount
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function addWithIndex(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $list, \Aerospike\Expression $indexBitCount, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Add values, creating the sketch with a precision and MinHash count.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $indexBitCount
         * @param \Aerospike\Expression $minHashCount
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function addWithIndexAndMinHash(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $list, \Aerospike\Expression $indexBitCount, \Aerospike\Expression $minHashCount, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * The sketch's own parameters: index bit count and MinHash count.
         *
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function describe(\Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * The estimated number of distinct values.
         *
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function getCount(\Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * The estimated size of the intersection.
         *
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function getIntersectCount(\Aerospike\Expression $list, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * The estimated Jaccard similarity, from 0.0 to 1.0.
         *
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function getSimilarity(\Aerospike\Expression $list, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * A sketch that is the union of this one and the others.
         *
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function getUnion(\Aerospike\Expression $list, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * The estimated size of that union, without building it.
         *
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function getUnionCount(\Aerospike\Expression $list, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Create or reset a sketch.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $indexBitCount
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function init(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $indexBitCount, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Create or reset a sketch that also supports similarity estimates.
         *
         * @param \Aerospike\BinPolicy|null $policy
         * @param \Aerospike\Expression $indexBitCount
         * @param \Aerospike\Expression $minHashCount
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function initWithMinHash(?\Aerospike\BinPolicy $policy, \Aerospike\Expression $indexBitCount, \Aerospike\Expression $minHashCount, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Whether a value may be in the sketch.
         *
         * False positives, no false negatives: `false` is certain and `true` is
         * probable, which is what makes it cheap.
         *
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function mayContain(\Aerospike\Expression $list, \Aerospike\Expression $bin): \Aerospike\Expression {}
    }

    /**
     * List expressions: `aerospike-core`'s `expressions::lists`.
     *
     * Every argument is an expression, which is the difference from
     * `Aerospike\ListOp`: an operation takes a literal index, and a list *expression*
     * takes an expression for it. That is what lets a filter say "the element whose
     * index is in another bin".
     *
     * The `bin` argument is the list operated on — usually `Exp::listBin('name')`,
     * but any expression evaluating to a list, including another list expression.
     * Modify operations return the whole modified list, so they compose.
     *
     * ```php
     * use Aerospike\{Exp, ExpList, ListReturn};
     *
     * // A record whose "scores" list contains a value above 900.
     * $filter = Exp::gt(
     *     ExpList::getByValueRange(ListReturn::Count, Exp::intVal(900), null,
     *         Exp::listBin('scores')),
     *     Exp::intVal(0),
     * );
     * ```
     *
     * Add a path into a nested list with `->context([...])`, exactly as an
     * `Operation` does.
     */
    class ExpList {
        public function __construct() {}

        /**
         * Append one value.
         *
         * Returns the whole modified list, which is what makes these composable: the
         * result is a list expression another operation can read.
         *
         * @param \Aerospike\ListPolicy|null $policy
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function append(?\Aerospike\ListPolicy $policy, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Append every element of a list.
         *
         * @param \Aerospike\ListPolicy|null $policy
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function appendItems(?\Aerospike\ListPolicy $policy, \Aerospike\Expression $list, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Remove every element.
         *
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function clear(\Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Select the element at an index.
         *
         * Takes a `valueType` because one element comes back as itself rather than as
         * a list, so the server has to be told how to read it.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\ExpType $valueType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByIndex(\Aerospike\ListReturn $returnType, \Aerospike\ExpType $valueType, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select from an index to the end.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByIndexRange(\Aerospike\ListReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select a bounded number of elements from an index.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByIndexRangeCount(\Aerospike\ListReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select the element at a rank.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\ExpType $valueType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByRank(\Aerospike\ListReturn $returnType, \Aerospike\ExpType $valueType, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select from a rank to the highest.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByRankRange(\Aerospike\ListReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select a bounded number of elements from a rank.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByRankRangeCount(\Aerospike\ListReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select elements equal to a value.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValue(\Aerospike\ListReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select elements equal to any value in a list.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $values
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValueList(\Aerospike\ListReturn $returnType, \Aerospike\Expression $values, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select elements in a value range. `null` bounds are unbounded.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression|null $valueBegin
         * @param \Aerospike\Expression|null $valueEnd
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValueRange(\Aerospike\ListReturn $returnType, ?\Aerospike\Expression $valueBegin, ?\Aerospike\Expression $valueEnd, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select by rank relative to a value.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValueRelativeRankRange(\Aerospike\ListReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Select a bounded number by rank relative to a value.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValueRelativeRankRangeCount(\Aerospike\ListReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $rank, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Add to the number at an index.
         *
         * @param \Aerospike\ListPolicy|null $policy
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function increment(?\Aerospike\ListPolicy $policy, \Aerospike\Expression $index, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Insert one value at an index.
         *
         * @param \Aerospike\ListPolicy|null $policy
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function insert(?\Aerospike\ListPolicy $policy, \Aerospike\Expression $index, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Insert every element of a list at an index.
         *
         * @param \Aerospike\ListPolicy|null $policy
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $list
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function insertItems(?\Aerospike\ListPolicy $policy, \Aerospike\Expression $index, \Aerospike\Expression $list, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Remove the element at an index.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByIndex(\Aerospike\ListReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove from an index to the end.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByIndexRange(\Aerospike\ListReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove a bounded number of elements from an index.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByIndexRangeCount(\Aerospike\ListReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove the element at a rank.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByRank(\Aerospike\ListReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove from a rank to the highest.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByRankRange(\Aerospike\ListReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove a bounded number of elements from a rank.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByRankRangeCount(\Aerospike\ListReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove elements equal to a value.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValue(\Aerospike\ListReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove elements equal to any value in a list.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $values
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValueList(\Aerospike\ListReturn $returnType, \Aerospike\Expression $values, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove elements in a value range.
         *
         * `null` for either bound means unbounded on that side — not a missing
         * argument.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression|null $valueBegin
         * @param \Aerospike\Expression|null $valueEnd
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValueRange(\Aerospike\ListReturn $returnType, ?\Aerospike\Expression $valueBegin, ?\Aerospike\Expression $valueEnd, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove elements by rank relative to a value.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValueRelativeRankRange(\Aerospike\ListReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove a bounded number of elements by rank relative to a value.
         *
         * @param \Aerospike\ListReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValueRelativeRankRangeCount(\Aerospike\ListReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $rank, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Replace the value at an index.
         *
         * @param \Aerospike\ListPolicy|null $policy
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function set(?\Aerospike\ListPolicy $policy, \Aerospike\Expression $index, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * How many elements the list has.
         *
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function size(\Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Sort in place.
         *
         * @param bool $descending
         * @param bool $dropDuplicates
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function sort(bool $descending, bool $dropDuplicates, \Aerospike\Expression $bin): \Aerospike\Expression {}
    }

    /**
     * Map expressions: `aerospike-core`'s `expressions::maps`.
     *
     * As [`ExpList`], for maps — with the four ways a map is addressed (key, value,
     * index, rank) rather than a list's two.
     *
     * ```php
     * use Aerospike\{Exp, ExpMap, ExpType, MapReturn};
     *
     * // A record whose "attrs" map has tier == "gold".
     * $filter = Exp::eq(
     *     ExpMap::getByKey(MapReturn::Value, ExpType::Text, Exp::stringVal('tier'),
     *         Exp::mapBin('attrs')),
     *     Exp::stringVal('gold'),
     * );
     * ```
     */
    class ExpMap {
        public function __construct() {}

        /**
         * Remove every entry.
         *
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function clear(\Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * The entry at an index.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\ExpType $valueType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByIndex(\Aerospike\MapReturn $returnType, \Aerospike\ExpType $valueType, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Entries from an index to the end.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByIndexRange(\Aerospike\MapReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * A bounded number of entries from an index.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByIndexRangeCount(\Aerospike\MapReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * The value at a key.
         *
         * Takes a `valueType` because one value comes back as itself rather than as a
         * collection.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\ExpType $valueType
         * @param \Aerospike\Expression $key
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByKey(\Aerospike\MapReturn $returnType, \Aerospike\ExpType $valueType, \Aerospike\Expression $key, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Entries whose key is in a list.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $keys
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByKeyList(\Aerospike\MapReturn $returnType, \Aerospike\Expression $keys, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Entries whose key is in a range. `null` bounds are unbounded.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression|null $keyBegin
         * @param \Aerospike\Expression|null $keyEnd
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByKeyRange(\Aerospike\MapReturn $returnType, ?\Aerospike\Expression $keyBegin, ?\Aerospike\Expression $keyEnd, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Entries by index relative to a key.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $key
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByKeyRelativeIndexRange(\Aerospike\MapReturn $returnType, \Aerospike\Expression $key, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * A bounded number of entries by index relative to a key.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $key
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByKeyRelativeIndexRangeCount(\Aerospike\MapReturn $returnType, \Aerospike\Expression $key, \Aerospike\Expression $index, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * The entry at a rank.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\ExpType $valueType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByRank(\Aerospike\MapReturn $returnType, \Aerospike\ExpType $valueType, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Entries from a rank to the highest.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByRankRange(\Aerospike\MapReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * A bounded number of entries from a rank.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByRankRangeCount(\Aerospike\MapReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Entries with a value.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValue(\Aerospike\MapReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Entries whose value is in a list.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $values
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValueList(\Aerospike\MapReturn $returnType, \Aerospike\Expression $values, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Entries whose value is in a range.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression|null $valueBegin
         * @param \Aerospike\Expression|null $valueEnd
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValueRange(\Aerospike\MapReturn $returnType, ?\Aerospike\Expression $valueBegin, ?\Aerospike\Expression $valueEnd, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Entries by rank relative to a value.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValueRelativeRankRange(\Aerospike\MapReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * A bounded number of entries by rank relative to a value.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function getByValueRelativeRankRangeCount(\Aerospike\MapReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $rank, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Add to the number at a key.
         *
         * @param \Aerospike\MapPolicy|null $policy
         * @param \Aerospike\Expression $key
         * @param \Aerospike\Expression $incr
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function increment(?\Aerospike\MapPolicy $policy, \Aerospike\Expression $key, \Aerospike\Expression $incr, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Set one key.
         *
         * @param \Aerospike\MapPolicy|null $policy
         * @param \Aerospike\Expression $key
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function put(?\Aerospike\MapPolicy $policy, \Aerospike\Expression $key, \Aerospike\Expression $value, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Set every key of another map.
         *
         * @param \Aerospike\MapPolicy|null $policy
         * @param \Aerospike\Expression $map
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function putItems(?\Aerospike\MapPolicy $policy, \Aerospike\Expression $map, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * Remove the entry at an index.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByIndex(\Aerospike\MapReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove from an index to the end.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByIndexRange(\Aerospike\MapReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove a bounded number of entries from an index.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByIndexRangeCount(\Aerospike\MapReturn $returnType, \Aerospike\Expression $index, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove one key.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $key
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByKey(\Aerospike\MapReturn $returnType, \Aerospike\Expression $key, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove every key in a list.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $keys
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByKeyList(\Aerospike\MapReturn $returnType, \Aerospike\Expression $keys, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove keys in a range. `null` bounds are unbounded.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression|null $keyBegin
         * @param \Aerospike\Expression|null $keyEnd
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByKeyRange(\Aerospike\MapReturn $returnType, ?\Aerospike\Expression $keyBegin, ?\Aerospike\Expression $keyEnd, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove keys by index relative to a key.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $key
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByKeyRelativeIndexRange(\Aerospike\MapReturn $returnType, \Aerospike\Expression $key, \Aerospike\Expression $index, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove a bounded number of keys by index relative to a key.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $key
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByKeyRelativeIndexRangeCount(\Aerospike\MapReturn $returnType, \Aerospike\Expression $key, \Aerospike\Expression $index, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove the entry at a rank.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByRank(\Aerospike\MapReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove from a rank to the highest.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByRankRange(\Aerospike\MapReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove a bounded number of entries from a rank.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByRankRangeCount(\Aerospike\MapReturn $returnType, \Aerospike\Expression $rank, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove entries with a value.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValue(\Aerospike\MapReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove entries whose value is in a list.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $values
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValueList(\Aerospike\MapReturn $returnType, \Aerospike\Expression $values, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove entries whose value is in a range.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression|null $valueBegin
         * @param \Aerospike\Expression|null $valueEnd
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValueRange(\Aerospike\MapReturn $returnType, ?\Aerospike\Expression $valueBegin, ?\Aerospike\Expression $valueEnd, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove entries by rank relative to a value.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValueRelativeRankRange(\Aerospike\MapReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $rank, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * Remove a bounded number of entries by rank relative to a value.
         *
         * @param \Aerospike\MapReturn $returnType
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $rank
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $bin
         * @param bool $inverted
         * @return \Aerospike\Expression
         */
        public static function removeByValueRelativeRankRangeCount(\Aerospike\MapReturn $returnType, \Aerospike\Expression $value, \Aerospike\Expression $rank, \Aerospike\Expression $count, \Aerospike\Expression $bin, bool $inverted = false): \Aerospike\Expression {}

        /**
         * How many entries the map has.
         *
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function size(\Aerospike\Expression $bin): \Aerospike\Expression {}
    }

    /**
     * The expression operations: `aerospike-core`'s `operations::exp`.
     *
     * An expression is evaluated against the record by the **server**, so it sees
     * the record as it is at that moment — inside the same atomic `operate()` as
     * everything else in the call.
     *
     * ```php
     * $client->operate(null, $key, [
     *     // Compute a bin from two others, and store it
     *     ExpOp::write('total', Expression::ael('$.price * $.quantity')),
     *     // Compute something and just read it back, storing nothing
     *     ExpOp::read('discounted', Expression::ael('$.total * 0.9')),
     * ]);
     * ```
     */
    class ExpOp {
        public function __construct() {}

        /**
         * Evaluate, and return the result labelled `$name`.
         *
         * `$name` is **not a bin**: nothing is written, and it is only what the
         * answer comes back under — so it can be anything that reads well.
         *
         * `$evalNoFail` swallows the failure when the expression resolves to
         * nothing usable, leaving the operation out of the result instead.
         *
         * @param string $name
         * @param \Aerospike\Expression $expression
         * @param bool $evalNoFail
         * @return \Aerospike\Operation
         */
        public static function read(string $name, \Aerospike\Expression $expression, bool $evalNoFail = false): \Aerospike\Operation {}

        /**
         * Evaluate, and write the result into `$bin`.
         *
         * `$allowDelete` makes an expression that evaluates to null **delete** the
         * bin; without it, that is reported as an operation that did not apply.
         *
         * @param string $bin
         * @param \Aerospike\Expression $expression
         * @param \Aerospike\BinWriteMode|null $writeMode
         * @param bool $allowDelete
         * @param bool $noFail
         * @param bool $evalNoFail
         * @return \Aerospike\Operation
         */
        public static function write(string $bin, \Aerospike\Expression $expression, ?\Aerospike\BinWriteMode $writeMode = null, bool $allowDelete = false, bool $noFail = false, bool $evalNoFail = false): \Aerospike\Operation {}
    }

    /**
     * CDT path expressions: `aerospike-core`'s `exp_select_*` and `exp_modify_*`.
     *
     * Every other expression class addresses **one** node — a list element, a map
     * entry. These address **many**, because the path handed to them contains a
     * fan-out step: `Ctx::allChildren()`, or `Ctx::allChildrenWithFilter()` to visit
     * only the children a filter accepts. "The price of every book" is a path
     * expression; "the price of the first book" is an `ExpMap` read.
     *
     * **Needs server 8.1.1 or later.** The fan-out is the server walking the
     * collection, not a loop here, and the daemon refuses against an older cluster
     * rather than letting it fail obscurely.
     *
     * ```php
     * use Aerospike\{Exp, ExpPath, ExpType, Ctx, LoopVarPart};
     *
     * // Every book priced over 20 — the filter runs per child, and the loop
     * // variable is the child being tested.
     * $dear = ExpPath::selectValues(ExpType::ListType, Exp::mapBin('books'), [
     *     Ctx::allChildrenWithFilter(
     *         Exp::gt(
     *             ExpMap::getByKey(MapReturn::Value, ExpType::Integer,
     *                 Exp::stringVal('price'), Exp::loopVar(ExpType::MapType, LoopVarPart::Value)),
     *             Exp::intVal(20),
     *         ),
     *     ),
     * ]);
     * ```
     *
     * # The path is an argument here, not `->context()`
     *
     * The collection classes take their path fluently, because it is optional there —
     * most list reads have none. For a path expression the path *is* the operation:
     * without a fan-out step it selects one node and there was no reason to use this
     * class. So it is a required argument, in `aerospike-core`'s position (last).
     */
    class ExpPath {
        public function __construct() {}

        /**
         * Replace each selected node, failing on a type mismatch.
         *
         * @param \Aerospike\ExpType $returnType
         * @param \Aerospike\Expression $bin
         * @param \Aerospike\Expression $modify
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public static function modify(\Aerospike\ExpType $returnType, \Aerospike\Expression $bin, \Aerospike\Expression $modify, array $ctx): \Aerospike\Expression {}

        /**
         * Replace each selected node, with an explicit `ModifyFlag`.
         *
         * `modify` produces the new value and sees the node through
         * [`Exp::loop_var`]. Return [`Exp::remove_result`] from it to delete that
         * node instead, which is how a conditional removal is written in one pass.
         *
         * @param \Aerospike\ExpType $returnType
         * @param int $flag
         * @param \Aerospike\Expression $bin
         * @param \Aerospike\Expression $modify
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public static function modifyByPath(\Aerospike\ExpType $returnType, int $flag, \Aerospike\Expression $bin, \Aerospike\Expression $modify, array $ctx): \Aerospike\Expression {}

        /**
         * Replace each selected node, ignoring type mismatches.
         *
         * @param \Aerospike\ExpType $returnType
         * @param \Aerospike\Expression $bin
         * @param \Aerospike\Expression $modify
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public static function modifyNoFail(\Aerospike\ExpType $returnType, \Aerospike\Expression $bin, \Aerospike\Expression $modify, array $ctx): \Aerospike\Expression {}

        /**
         * Remove each selected node.
         *
         * @param \Aerospike\ExpType $returnType
         * @param \Aerospike\Expression $bin
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public static function remove(\Aerospike\ExpType $returnType, \Aerospike\Expression $bin, array $ctx): \Aerospike\Expression {}

        /**
         * Select from each node the path reached, with an explicit `SelectFlag`.
         *
         * The general form. The four below are this with the flag filled in, and are
         * what most code should use — they are literally the same server opcode.
         *
         * @param \Aerospike\ExpType $returnType
         * @param int $flag
         * @param \Aerospike\Expression $bin
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public static function selectByPath(\Aerospike\ExpType $returnType, int $flag, \Aerospike\Expression $bin, array $ctx): \Aerospike\Expression {}

        /**
         * The key and value of each selected node, as pairs.
         *
         * @param \Aerospike\ExpType $returnType
         * @param \Aerospike\Expression $bin
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public static function selectMapEntries(\Aerospike\ExpType $returnType, \Aerospike\Expression $bin, array $ctx): \Aerospike\Expression {}

        /**
         * The map key of each selected node.
         *
         * @param \Aerospike\ExpType $returnType
         * @param \Aerospike\Expression $bin
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public static function selectMapKeys(\Aerospike\ExpType $returnType, \Aerospike\Expression $bin, array $ctx): \Aerospike\Expression {}

        /**
         * The original structure with everything the path did not match pruned away.
         *
         * The others return a flat collection of what matched; this keeps the shape,
         * so the answer still says *where* each match was.
         *
         * @param \Aerospike\ExpType $returnType
         * @param \Aerospike\Expression $bin
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public static function selectMatchingTree(\Aerospike\ExpType $returnType, \Aerospike\Expression $bin, array $ctx): \Aerospike\Expression {}

        /**
         * The value of each selected node.
         *
         * @param \Aerospike\ExpType $returnType
         * @param \Aerospike\Expression $bin
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public static function selectValues(\Aerospike\ExpType $returnType, \Aerospike\Expression $bin, array $ctx): \Aerospike\Expression {}
    }

    /**
     * String expressions: `aerospike-core`'s `expressions::string`.
     *
     * **Needs server 8.1.3 or later** — these are the string operations, and the
     * daemon checks the cluster's versions rather than letting an older server refuse
     * them obscurely.
     *
     * Offsets and lengths are in **characters**, not bytes, except where the name says
     * otherwise (`byteLength`, `toBlob`): the server works in Unicode code points, so
     * a multi-byte character counts once.
     *
     * `src` is the string operated on — usually `Exp::stringBin('name')`, but any
     * expression evaluating to a string, including another string expression, which is
     * what lets these chain. The modify operations return the whole modified string.
     *
     * ```php
     * use Aerospike\{Exp, ExpStr};
     *
     * // Records whose trimmed, folded name starts with "ali".
     * ExpStr::startsWith(
     *     Exp::stringVal('ali'),
     *     ExpStr::caseFold(false, ExpStr::trim(false, Exp::stringBin('name'))),
     * );
     * ```
     *
     * # `noFail` instead of a policy class
     *
     * The string write policy carries exactly one flag, so the modify methods take a
     * `bool $noFail` rather than an object. A class holding one boolean would be
     * ceremony; `BinPolicy` exists for the bitwise and HLL families because those
     * carry a write *mode* as well.
     */
    class ExpStr {
        public function __construct() {}

        /**
         * Append one string.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function append(bool $noFail, \Aerospike\Expression $value, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Decode base64 into a blob.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function b64Decode(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Length in bytes, which differs from `strlen` outside ASCII.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function byteLength(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Case-fold, for case-insensitive comparison.
         *
         * Not the same as lower-casing: folding is defined for scripts where case does
         * not map one-to-one, which is what makes it the right basis for comparison
         * rather than for display.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function caseFold(bool $noFail, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * The character at an index.
         *
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function charAt(\Aerospike\Expression $index, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Join a list of strings onto the end.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $values
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function concat(bool $noFail, \Aerospike\Expression $values, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Whether a needle appears at all.
         *
         * @param \Aerospike\Expression $needle
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function contains(\Aerospike\Expression $needle, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Whether the string ends with a suffix.
         *
         * @param \Aerospike\Expression $suffix
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function endsWith(\Aerospike\Expression $suffix, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Index of the first occurrence of a needle, or -1.
         *
         * @param \Aerospike\Expression $needle
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function find(\Aerospike\Expression $needle, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Index of the nth occurrence, counting from zero, or -1.
         *
         * @param \Aerospike\Expression $needle
         * @param \Aerospike\Expression $occurrence
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function findNth(\Aerospike\Expression $needle, \Aerospike\Expression $occurrence, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Insert a value at an index.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function insert(bool $noFail, \Aerospike\Expression $index, \Aerospike\Expression $value, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Whether every character is lower case.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function isLower(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Whether the string reads as a number.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function isNumeric(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Whether the string reads as a number of a particular kind.
         *
         * @param \Aerospike\StringNumericType $numericType
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function isNumericTyped(\Aerospike\StringNumericType $numericType, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Whether every character is upper case.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function isUpper(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Lower case.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function lower(bool $noFail, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Normalise to Unicode NFC.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function normalizeNfc(bool $noFail, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Overwrite from an index.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $index
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function overwrite(bool $noFail, \Aerospike\Expression $index, \Aerospike\Expression $value, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Pad the end to a length.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $targetLength
         * @param \Aerospike\Expression $padString
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function padEnd(bool $noFail, \Aerospike\Expression $targetLength, \Aerospike\Expression $padString, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Pad the start to a length.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $targetLength
         * @param \Aerospike\Expression $padString
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function padStart(bool $noFail, \Aerospike\Expression $targetLength, \Aerospike\Expression $padString, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Prepend one string.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function prepend(bool $noFail, \Aerospike\Expression $value, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Whether a pattern matches.
         *
         * The pattern is an expression here, so it is recompiled per record — unlike
         * `Exp::regexCompare`, where it is a literal the server compiles once.
         *
         * @param \Aerospike\Expression $pattern
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function regexCompare(\Aerospike\Expression $pattern, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Whether a pattern matches, with `StringRegexFlag` flags OR-ed together.
         *
         * @param \Aerospike\Expression $pattern
         * @param int $regexFlags
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function regexCompareWithFlags(\Aerospike\Expression $pattern, int $regexFlags, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Replace by pattern. Include the global flag to replace every match.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $pattern
         * @param \Aerospike\Expression $replacement
         * @param int $regexFlags
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function regexReplace(bool $noFail, \Aerospike\Expression $pattern, \Aerospike\Expression $replacement, int $regexFlags, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Repeat the string.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $count
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function repeat(bool $noFail, \Aerospike\Expression $count, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Replace the first occurrence.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $needle
         * @param \Aerospike\Expression $replacement
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function replace(bool $noFail, \Aerospike\Expression $needle, \Aerospike\Expression $replacement, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Replace every occurrence.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $needle
         * @param \Aerospike\Expression $replacement
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function replaceAll(bool $noFail, \Aerospike\Expression $needle, \Aerospike\Expression $replacement, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Cut out a range of characters.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $start
         * @param \Aerospike\Expression $end
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function snip(bool $noFail, \Aerospike\Expression $start, \Aerospike\Expression $end, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Split on whitespace.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function split(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Split on a separator.
         *
         * @param \Aerospike\Expression $separator
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function splitBySeparator(\Aerospike\Expression $separator, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Whether the string starts with a prefix.
         *
         * @param \Aerospike\Expression $prefix
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function startsWith(\Aerospike\Expression $prefix, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Length in characters.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function strlen(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * From an index to the end.
         *
         * @param \Aerospike\Expression $start
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function substr(\Aerospike\Expression $start, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * A bounded range of characters.
         *
         * @param \Aerospike\Expression $start
         * @param \Aerospike\Expression $end
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function substrRange(\Aerospike\Expression $start, \Aerospike\Expression $end, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * The string's bytes, as a blob.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function toBlob(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Parse as a double.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function toDouble(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Parse as an integer.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function toInteger(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Anything as its string form.
         *
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function toString(\Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Trim both ends.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function trim(bool $noFail, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Trim trailing whitespace.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function trimEnd(bool $noFail, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Trim leading whitespace.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function trimStart(bool $noFail, \Aerospike\Expression $src): \Aerospike\Expression {}

        /**
         * Upper case.
         *
         * @param bool $noFail
         * @param \Aerospike\Expression $src
         * @return \Aerospike\Expression
         */
        public static function upper(bool $noFail, \Aerospike\Expression $src): \Aerospike\Expression {}
    }

    /**
     * The type an expression evaluates to.
     *
     * Aerospike needs this where the expression itself does not settle it — reading
     * a bin, reading the key, or pulling a value out of a collection — because the
     * server has to know how to interpret the bytes it finds. Mirrors
     * `aerospike-core`'s `ExpType`.
     *
     * **It is not a hint.** `Exp::bin('age', ExpType::Str)` reads an integer bin's
     * bytes as a string, and the comparison that follows is against nonsense rather
     * than an error. The typed shorthands — `Exp::intBin()`, `Exp::stringBin()` and
     * the rest — exist so that most code never names one of these.
     */
    enum ExpType: string {
    /**
     * Nil, which is what an absent bin reads as.
     */
      case Nil = 'NIL';
    /**
     * Boolean.
     *
     * `Boolean` and not `Bool`: PHP reserves its scalar type names, and a case
     * nobody can write would be worse than one spelled out.
     */
      case Boolean = 'BOOL';
    /**
     * Integer. `Integer` in PHP, for the reason [`ExpType::Bool`] gives.
     */
      case Integer = 'INT';
    /**
     * String. `Text` in PHP, as [`IndexType::String`] is.
     */
      case Text = 'STRING';
    /**
     * List. `ListType` in PHP, which reserves `list` for the language construct.
     */
      case ListType = 'LIST';
    /**
     * Map. `MapType` in PHP, for symmetry with [`ExpType::List`] rather than
     * because PHP requires it.
     */
      case MapType = 'MAP';
    /**
     * Byte string.
     */
      case Blob = 'BLOB';
    /**
     * Double. `Double` in PHP, for the reason [`ExpType::Bool`] gives.
     */
      case Double = 'FLOAT';
    /**
     * GeoJSON.
     */
      case Geo = 'GEO';
    /**
     * HyperLogLog sketch.
     */
      case Hll = 'HLL';
        /** The 1.x spelling of a case of this enum. */
        public static function Nil(): \Aerospike\ExpType {}

        /** The 1.x spelling of a case of this enum. */
        public static function Bool(): \Aerospike\ExpType {}

        /** The 1.x spelling of a case of this enum. */
        public static function Int(): \Aerospike\ExpType {}

        /** The 1.x spelling of a case of this enum. */
        public static function String(): \Aerospike\ExpType {}

        /** The 1.x spelling of a case of this enum. */
        public static function List(): \Aerospike\ExpType {}

        /** The 1.x spelling of a case of this enum. */
        public static function Map(): \Aerospike\ExpType {}

        /** The 1.x spelling of a case of this enum. */
        public static function Blob(): \Aerospike\ExpType {}

        /** The 1.x spelling of a case of this enum. */
        public static function Float(): \Aerospike\ExpType {}

        /** The 1.x spelling of a case of this enum. */
        public static function Geo(): \Aerospike\ExpType {}

        /** The 1.x spelling of a case of this enum. */
        public static function Hll(): \Aerospike\ExpType {}

    }

    /**
     * How long a record lives after the write that carries this.
     *
     * A class rather than an int, because three of the four cases are not
     * durations at all. Other clients encode them as `-1` and `-2`, which is
     * precisely the kind of detail that gets mixed up crossing a language
     * boundary — so here they have names, and a negative number of seconds is an
     * error rather than a sentinel someone meant.
     *
     * ```php
     * Aerospike\Expiration::seconds(3600);      // one hour from now
     * Aerospike\Expiration::never();            // never expires
     * Aerospike\Expiration::namespaceDefault(); // the namespace's default-ttl
     * Aerospike\Expiration::dontUpdate();       // leave the current TTL alone
     * ```
     */
    class Expiration {
        public function __construct() {}

        /**
         * A readable form, for logs and test failures.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * Leave the record's current TTL exactly as it is.
         *
         * The difference from [`Expiration::namespace_default`] matters on an
         * update: this one does not reset the countdown, so a record keeps the
         * life it had.
         *
         * @return \Aerospike\Expiration
         */
        public static function dontUpdate(): \Aerospike\Expiration {}

        /**
         * Whether this is [`Expiration::dont_update`].
         *
         * @return bool
         */
        public function isDontUpdate(): bool {}

        /**
         * Whether this is [`Expiration::namespace_default`].
         *
         * @return bool
         */
        public function isNamespaceDefault(): bool {}

        /**
         * Whether this is [`Expiration::never`].
         *
         * @return bool
         */
        public function isNever(): bool {}

        /**
         * Use the namespace's `default-ttl`.
         *
         * @return \Aerospike\Expiration
         */
        public static function namespaceDefault(): \Aerospike\Expiration {}

        /**
         * Never expire.
         *
         * @return \Aerospike\Expiration
         */
        public static function never(): \Aerospike\Expiration {}

        /**
         * Expire this many seconds from now.
         *
         * `0` is legal and means the namespace default, which is what the server
         * does with a zero TTL; say [`Expiration::namespace_default`] if that is
         * what you mean.
         *
         * @param int $seconds
         * @return \Aerospike\Expiration
         */
        public static function seconds(int $seconds): \Aerospike\Expiration {}

        /**
         * The number of seconds, or `null` for the three named cases.
         *
         * @return int|null
         */
        public function toSeconds(): ?int {}
    }

    /**
     * An expression to evaluate against a record.
     *
     * Two ways to say one, both text:
     *
     * ```php
     * Aerospike\Expression::ael('$.first + $.second');   // the server compiles it
     * Aerospike\Expression::base64($packedFromAnotherClient);
     * ```
     *
     * Three ways to get one, and they are interchangeable wherever an expression is
     * used — a policy filter, an expression operation, an expression-based index:
     *
     * | how | needs | notes |
     * | --- | --- | --- |
     * | `Aerospike\\Exp::*` | nothing | composed here, packed by the client |
     * | `Expression::ael($text)` | server 8.1.3+ | shortest to write; the server parses it |
     * | `Expression::base64($packed)` | nothing | one another client packed |
     *
     * **Prefer the builder** unless the text form is clearly more readable for what
     * you are writing: it works on every server this client supports, and a wrong
     * operand is a `TypeError` at the call site rather than a server error a round
     * trip later. See `Aerospike\\Exp`.
     *
     * Only a built expression can be an operand of another. The other two are
     * already whole expressions — there is nowhere in the wire format to put text
     * inside a packed tree.
     */
    class Expression {
        public function __construct() {}

        /**
         * An expression written in the Aerospike Expression Language.
         *
         * @param string $source
         * @return \Aerospike\Expression
         */
        public static function ael(string $source): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::and`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function and(array $exps): \Aerospike\Expression {}

        /**
         * An expression another Aerospike client packed, base64-encoded.
         *
         * This is the packed *form*, not the source text — passing source here
         * reaches the daemon as base64 that does not decode, and it says so.
         *
         * @param string $packed
         * @return \Aerospike\Expression
         */
        public static function base64(string $packed): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::bin_exists`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function binExists(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::bin_type`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function binType(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::blob_bin`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function blobBin(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::blob_val`].
         *
         * @param array $value
         * @return \Aerospike\Expression
         */
        public static function blobVal(array $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::bool_val`].
         *
         * @param bool $value
         * @return \Aerospike\Expression
         */
        public static function boolVal(bool $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::cond`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function cond(array $exps): \Aerospike\Expression {}

        /**
         * Attach a path into a nested collection.
         *
         * Only meaningful on a list or map expression — those are the ones that take
         * a context — so anything else is refused by name rather than accepting a
         * path that would be dropped. The same spelling as `Operation::context()`, so
         * a caller learns one convention for both.
         *
         * ```php
         * use Aerospike\{Exp, ExpList, Ctx, ListReturn};
         *
         * // The size of the list at attrs["history"].
         * ExpList::size(Exp::mapBin('attrs'))->context([Ctx::mapKey('history')]);
         * ```
         *
         * @param array $ctx
         * @return \Aerospike\Expression
         */
        public function context(array $ctx): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::def`].
         *
         * @param string $name
         * @param \Aerospike\Expression $value
         * @return \Aerospike\Expression
         */
        public static function def(string $name, \Aerospike\Expression $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::device_size`].
         *
         * @return \Aerospike\Expression
         */
        public static function deviceSize(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::digest_modulo`].
         *
         * @param int $modulo
         * @return \Aerospike\Expression
         */
        public static function digestModulo(int $modulo): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::eq`].
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function eq(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::let_`]. The 1.x name for `let()`, which is what this client calls it.
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function expLet(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::float_bin`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function floatBin(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::float_val`].
         *
         * @param float $value
         * @return \Aerospike\Expression
         */
        public static function floatVal(float $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::ge`].
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function ge(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::geo_bin`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function geoBin(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::geo_compare`].
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function geoCompare(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::geo_val`].
         *
         * @param string $value
         * @return \Aerospike\Expression
         */
        public static function geoVal(string $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::gt`].
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function gt(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::hll_bin`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function hllBin(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::infinity`].
         *
         * @return \Aerospike\Expression
         */
        public static function infinity(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_and`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function intAnd(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_arshift`].
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $shift
         * @return \Aerospike\Expression
         */
        public static function intArshift(\Aerospike\Expression $value, \Aerospike\Expression $shift): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_bin`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function intBin(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_count`].
         *
         * @param \Aerospike\Expression $exp
         * @return \Aerospike\Expression
         */
        public static function intCount(\Aerospike\Expression $exp): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_lscan`].
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $search
         * @return \Aerospike\Expression
         */
        public static function intLscan(\Aerospike\Expression $value, \Aerospike\Expression $search): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_lshift`].
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $shift
         * @return \Aerospike\Expression
         */
        public static function intLshift(\Aerospike\Expression $value, \Aerospike\Expression $shift): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_not`].
         *
         * @param \Aerospike\Expression $exp
         * @return \Aerospike\Expression
         */
        public static function intNot(\Aerospike\Expression $exp): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_or`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function intOr(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_rscan`].
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $search
         * @return \Aerospike\Expression
         */
        public static function intRscan(\Aerospike\Expression $value, \Aerospike\Expression $search): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_rshift`].
         *
         * @param \Aerospike\Expression $value
         * @param \Aerospike\Expression $shift
         * @return \Aerospike\Expression
         */
        public static function intRshift(\Aerospike\Expression $value, \Aerospike\Expression $shift): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_val`].
         *
         * @param int $value
         * @return \Aerospike\Expression
         */
        public static function intVal(int $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::int_xor`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function intXor(array $exps): \Aerospike\Expression {}

        /**
         * Whether this is Aerospike Expression Language source.
         *
         * @return bool
         */
        public function isAel(): bool {}

        /**
         * Whether this was composed by `Aerospike\\Exp`'s builder.
         *
         * The three forms are interchangeable wherever an expression is *used*, so
         * this is not something a caller normally has to ask. It matters in one
         * place: only a built expression can be an **operand** of another, because
         * the other two forms are already whole expressions.
         *
         * @return bool
         */
        public function isBuilt(): bool {}

        /**
         * The 1.x spelling of [`Exp::is_tombstone`].
         *
         * @return \Aerospike\Expression
         */
        public static function isTombstone(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::key`].
         *
         * @param \Aerospike\ExpType $expType
         * @return \Aerospike\Expression
         */
        public static function key(\Aerospike\ExpType $expType): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::key_exists`].
         *
         * @return \Aerospike\Expression
         */
        public static function keyExists(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::last_update`].
         *
         * @return \Aerospike\Expression
         */
        public static function lastUpdate(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::le`].
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function le(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::list_bin`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function listBin(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::list_val`].
         *
         * @param mixed $value
         * @return \Aerospike\Expression
         */
        public static function listVal(mixed $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::lt`].
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function lt(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::map_bin`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function mapBin(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::map_val`].
         *
         * @param mixed $value
         * @return \Aerospike\Expression
         */
        public static function mapVal(mixed $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::max`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function max(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::memory_size`].
         *
         * @return \Aerospike\Expression
         */
        public static function memorySize(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::min`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function min(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::ne`].
         *
         * @param \Aerospike\Expression $left
         * @param \Aerospike\Expression $right
         * @return \Aerospike\Expression
         */
        public static function ne(\Aerospike\Expression $left, \Aerospike\Expression $right): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::nil`].
         *
         * @return \Aerospike\Expression
         */
        public static function nil(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::not`].
         *
         * @param \Aerospike\Expression $exp
         * @return \Aerospike\Expression
         */
        public static function not(\Aerospike\Expression $exp): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_abs`].
         *
         * @param \Aerospike\Expression $value
         * @return \Aerospike\Expression
         */
        public static function numAbs(\Aerospike\Expression $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_add`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function numAdd(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_ceil`].
         *
         * @param \Aerospike\Expression $num
         * @return \Aerospike\Expression
         */
        public static function numCeil(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_div`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function numDiv(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_floor`].
         *
         * @param \Aerospike\Expression $num
         * @return \Aerospike\Expression
         */
        public static function numFloor(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_log`].
         *
         * @param \Aerospike\Expression $num
         * @param \Aerospike\Expression $base
         * @return \Aerospike\Expression
         */
        public static function numLog(\Aerospike\Expression $num, \Aerospike\Expression $base): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_mod`].
         *
         * @param \Aerospike\Expression $numerator
         * @param \Aerospike\Expression $denominator
         * @return \Aerospike\Expression
         */
        public static function numMod(\Aerospike\Expression $numerator, \Aerospike\Expression $denominator): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_mul`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function numMul(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_pow`].
         *
         * @param \Aerospike\Expression $base
         * @param \Aerospike\Expression $exponent
         * @return \Aerospike\Expression
         */
        public static function numPow(\Aerospike\Expression $base, \Aerospike\Expression $exponent): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::num_sub`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function numSub(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::or`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function or(array $exps): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::regex_compare`].
         *
         * @param string $regex
         * @param int $flags
         * @param \Aerospike\Expression $bin
         * @return \Aerospike\Expression
         */
        public static function regexCompare(string $regex, int $flags, \Aerospike\Expression $bin): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::set_name`].
         *
         * @return \Aerospike\Expression
         */
        public static function setName(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::since_update`].
         *
         * @return \Aerospike\Expression
         */
        public static function sinceUpdate(): \Aerospike\Expression {}

        /**
         * The text this was built from, or `""` for a built expression.
         *
         * A tree has no source text — it was never text — and inventing one would
         * mean writing an Aerospike Expression Language serialiser whose output
         * nothing reads.
         *
         * @return string
         */
        public function source(): string {}

        /**
         * The 1.x spelling of [`Exp::string_bin`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function stringBin(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::string_val`].
         *
         * @param string $value
         * @return \Aerospike\Expression
         */
        public static function stringVal(string $value): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::to_float`].
         *
         * @param \Aerospike\Expression $num
         * @return \Aerospike\Expression
         */
        public static function toFloat(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::to_int`].
         *
         * @param \Aerospike\Expression $num
         * @return \Aerospike\Expression
         */
        public static function toInt(\Aerospike\Expression $num): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::ttl`].
         *
         * @return \Aerospike\Expression
         */
        public static function ttl(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::unknown`].
         *
         * @return \Aerospike\Expression
         */
        public static function unknown(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::var`].
         *
         * @param string $name
         * @return \Aerospike\Expression
         */
        public static function var(string $name): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::void_time`].
         *
         * @return \Aerospike\Expression
         */
        public static function voidTime(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::wildcard`].
         *
         * @return \Aerospike\Expression
         */
        public static function wildcard(): \Aerospike\Expression {}

        /**
         * The 1.x spelling of [`Exp::xor`].
         *
         * @param array $exps
         * @return \Aerospike\Expression
         */
        public static function xor(array $exps): \Aerospike\Expression {}
    }

    /**
     * A secondary-index filter.
     *
     * One class for all of the Rust client's filter constructors, because they vary
     * in three independent ways and a class can express that: what is compared (the
     * static method), whether a collection index is meant (the `$collection`
     * argument), and whether the index is named by bin or by its own name
     * (`…ByIndex`).
     *
     * ```php
     * Aerospike\Filter::equal("name", "Alice");
     * Aerospike\Filter::range("age", 20, 30);
     * Aerospike\Filter::equal("tags", "urgent", Aerospike\CollectionIndex::List);
     * Aerospike\Filter::equalByIndex("users_by_name", "Alice");
     *
     * // A filter on a value nested inside a map, and one on an expression index.
     * Aerospike\Filter::equal("meta", 1, Aerospike\CollectionIndex::MapValues)
     *     ->context([Aerospike\Ctx::mapKey("inner")]);
     * Aerospike\Filter::range("computed", 0, 10)
     *     ->expression(Aerospike\Expression::ael('$.a + $.b'));
     * ```
     *
     * **A query needs the index to exist.** A filter naming a bin with no secondary
     * index on it is a server error (result code 201, `INDEX_NOTFOUND`), not an
     * empty result: the server cannot answer a query it has no index for, and
     * silently scanning instead would turn a fast query into a full-set read.
     */
    class Filter {
        public function __construct() {}

        /**
         * The same filter, applied inside a nested collection.
         *
         * For an index built on a path — a list inside a map, a map inside a list —
         * the path has to travel with the filter, because it is part of what the
         * index covers. Returns a new filter; this class is immutable, as the
         * policies are.
         *
         * @param array $ctx
         * @return \Aerospike\Filter
         */
        public function context(array $ctx): \Aerospike\Filter {}

        /**
         * How many context steps the filter carries.
         *
         * @return int
         */
        public function contextDepth(): int {}

        /**
         * Records whose indexed value equals `$value`.
         *
         * `$value` is an `int`, a `string`, or an `Aerospike\Blob` for a blob index
         * — the three types Aerospike indexes. Anything else is refused here rather
         * than sent as a filter the server would reject without saying which
         * argument was wrong.
         *
         * @param string $bin
         * @param mixed $value
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function equal(string $bin, mixed $value, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * [`Filter::equal`], against a named secondary index.
         *
         * For a bin with more than one index on it, where the bin name alone would
         * not say which to use.
         *
         * @param string $index
         * @param mixed $value
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function equalByIndex(string $index, mixed $value, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * The same filter, against an expression-based secondary index.
         *
         * The expression is the one the *index* was created with, not a record
         * filter: it identifies which index to use. A record filter belongs on the
         * policy, where it applies to every record the query returns.
         *
         * @param \Aerospike\Expression $expression
         * @return \Aerospike\Filter
         */
        public function expression(\Aerospike\Expression $expression): \Aerospike\Filter {}

        /**
         * Regions that contain a GeoJSON point.
         *
         * The inverse of `Filter::geoWithinRegion()`: there the bin holds points and
         * the filter is a region, here the bin holds regions and the filter is a
         * point.
         *
         * @param string $bin
         * @param string $point
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function geoContains(string $bin, string $point, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * `Filter::geoContains()`, against a named secondary index.
         *
         * @param string $index
         * @param string $point
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function geoContainsByIndex(string $index, string $point, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * Points within `$radius` metres of a longitude and latitude.
         *
         * Longitude first, as GeoJSON orders coordinates — not latitude first, as a
         * map application might. Getting them the wrong way round produces a query
         * that runs and finds nothing, so the order is worth reading twice.
         *
         * @param string $bin
         * @param float $longitude
         * @param float $latitude
         * @param float $radius
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function geoWithinRadius(string $bin, float $longitude, float $latitude, float $radius, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * `Filter::geoWithinRadius()`, against a named secondary index.
         *
         * @param string $index
         * @param float $longitude
         * @param float $latitude
         * @param float $radius
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function geoWithinRadiusByIndex(string $index, float $longitude, float $latitude, float $radius, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * Points inside a GeoJSON region.
         *
         * `$region` is a GeoJSON polygon or a circle, as a document — the same text
         * an `Aerospike\GeoJson` wraps.
         *
         * @param string $bin
         * @param string $region
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function geoWithinRegion(string $bin, string $region, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * `Filter::geoWithinRegion()`, against a named secondary index.
         *
         * @param string $index
         * @param string $region
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function geoWithinRegionByIndex(string $index, string $region, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * Whether the filter names a secondary index rather than a bin.
         *
         * @return bool
         */
        public function isByIndex(): bool {}

        /**
         * Records whose indexed integer is between `$begin` and `$end`, inclusive.
         *
         * Integers only, which is the Rust client's rule and the server's: a string
         * index has no ordering to range over, so a string range would be quietly
         * wrong rather than refused.
         *
         * @param string $bin
         * @param int $begin
         * @param int $end
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function range(string $bin, int $begin, int $end, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * [`Filter::range`], against a named secondary index.
         *
         * @param string $index
         * @param int $begin
         * @param int $end
         * @param \Aerospike\CollectionIndex|null $collection
         * @return \Aerospike\Filter
         */
        public static function rangeByIndex(string $index, int $begin, int $end, ?\Aerospike\CollectionIndex $collection = null): \Aerospike\Filter {}

        /**
         * The bin or index this filter names.
         *
         * @return string
         */
        public function target(): string {}
    }

    /**
     * Whether a write is guarded by the record's generation.
     *
     * Only meaningful together with a generation: the guard says how to compare,
     * and the generation says what to compare against.
     */
    enum GenerationPolicy: string {
    /**
     * No guard.
     */
      case None = 'NONE';
    /**
     * Write only if the generation matches exactly.
     */
      case ExpectGenEqual = 'EXPECT_GEN_EQUAL';
    /**
     * Write only if the record's generation is greater.
     */
      case ExpectGenGreater = 'EXPECT_GEN_GREATER';
        /** The 1.x spelling of a case of this enum. */
        public static function None(): \Aerospike\GenerationPolicy {}

        /** The 1.x spelling of a case of this enum. */
        public static function ExpectGenEqual(): \Aerospike\GenerationPolicy {}

        /** The 1.x spelling of a case of this enum. */
        public static function ExpectGenGreater(): \Aerospike\GenerationPolicy {}

    }

    /**
     * A GeoJSON document.
     *
     * Distinct from a plain string because the server indexes and queries it as
     * geometry: written as text it is inert, and a geospatial index or query will
     * not see it.
     *
     * The document is **not** validated here. The server parses GeoJSON and
     * reports precisely what it did not like; a second, weaker parser in the
     * extension could only disagree with it.
     */
    class GeoJson {
        /**
         * Wrap a GeoJSON document.
         *
         * @param string $json
         */
        public function __construct(string $json) {}

        /**
         * The wrapped document, so `(string) $geo` works.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * The wrapped document.
         *
         * @return string
         */
        public function json(): string {}
    }

    /**
     * A HyperLogLog sketch: opaque bytes only the server's HLL operations may
     * interpret.
     *
     * Wrapped rather than handed over as a PHP string for the same reason as
     * [`Blob`], plus one more: a sketch that was accidentally written back as text
     * is no longer a sketch, and the failure would only surface later, in a
     * cardinality estimate that is quietly wrong.
     */
    class Hll {
        /**
         * Wrap the bytes of a sketch the server produced.
         *
         * @param string $bytes
         */
        public function __construct(string $bytes) {}

        /**
         * The sketch's bytes, as a PHP string.
         *
         * @return string
         */
        public function bytes(): string {}

        /**
         * How many bytes the sketch occupies.
         *
         * @return int
         */
        public function length(): int {}
    }

    /**
     * The HyperLogLog operations: `aerospike-core`'s `operations::hll`, method for
     * method.
     *
     * A HyperLogLog sketch answers "roughly how many *distinct* things have I
     * seen" in a fixed few kilobytes, however many things there were. **Every
     * count it gives back is an estimate** — that is the trade the structure
     * makes, and `$indexBitCount` is what buys accuracy with space.
     *
     * ```php
     * $client->operate(null, $key, [
     *     HllOp::add('visitors', [$userId], indexBitCount: 12),
     *     HllOp::getCount('visitors'),
     * ]);
     * ```
     *
     * The bit counts are nullable wherever the Rust client has a family of
     * constructors for them, and `null` means "leave it to the sketch that is
     * already there".
     */
    class HllOp {
        public function __construct() {}

        /**
         * Add values to the sketch, creating it if it is missing.
         *
         * Returns how many of them caused the sketch to change — not how many were
         * new, which a sketch cannot know.
         *
         * @param string $bin
         * @param array $values
         * @param int|null $indexBitCount
         * @param int|null $minHashBitCount
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function add(string $bin, array $values, ?int $indexBitCount = null, ?int $minHashBitCount = null, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * The sketch's index-bit and MinHash-bit counts, as a two-element list.
         *
         * @param string $bin
         * @return \Aerospike\Operation
         */
        public static function describe(string $bin): \Aerospike\Operation {}

        /**
         * Reduce the sketch's precision to `$indexBitCount`.
         *
         * @param string $bin
         * @param int $indexBitCount
         * @return \Aerospike\Operation
         */
        public static function fold(string $bin, int $indexBitCount): \Aerospike\Operation {}

        /**
         * The estimated number of distinct values.
         *
         * @param string $bin
         * @return \Aerospike\Operation
         */
        public static function getCount(string $bin): \Aerospike\Operation {}

        /**
         * The estimated size of the intersection.
         *
         * @param string $bin
         * @param array $sketches
         * @return \Aerospike\Operation
         */
        public static function getIntersectCount(string $bin, array $sketches): \Aerospike\Operation {}

        /**
         * The estimated Jaccard similarity, from 0.0 to 1.0.
         *
         * @param string $bin
         * @param array $sketches
         * @return \Aerospike\Operation
         */
        public static function getSimilarity(string $bin, array $sketches): \Aerospike\Operation {}

        /**
         * The union of this sketch and the given ones, as a sketch.
         *
         * @param string $bin
         * @param array $sketches
         * @return \Aerospike\Operation
         */
        public static function getUnion(string $bin, array $sketches): \Aerospike\Operation {}

        /**
         * The estimated size of that union.
         *
         * @param string $bin
         * @param array $sketches
         * @return \Aerospike\Operation
         */
        public static function getUnionCount(string $bin, array $sketches): \Aerospike\Operation {}

        /**
         * Create a sketch, or reset the one that is there.
         *
         * @param string $bin
         * @param int $indexBitCount
         * @param int|null $minHashBitCount
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function init(string $bin, int $indexBitCount, ?int $minHashBitCount = null, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Recompute and cache the sketch's count.
         *
         * @param string $bin
         * @return \Aerospike\Operation
         */
        public static function refreshCount(string $bin): \Aerospike\Operation {}

        /**
         * Merge other sketches into this one.
         *
         * @param string $bin
         * @param array $sketches
         * @param \Aerospike\BinPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function setUnion(string $bin, array $sketches, ?\Aerospike\BinPolicy $policy = null): \Aerospike\Operation {}
    }

    /**
     * What kind of value a secondary index covers.
     *
     * The type has to match the bin's: a numeric index over a string bin indexes
     * nothing, and the server does not complain — the query simply finds no records.
     */
    enum IndexType: string {
    /**
     * Integers.
     */
      case Numeric = 'NUMERIC';
    /**
     * Strings.
     *
     * Named `Text` in PHP: `String` collides with the reserved type name, and a
     * case that cannot be written is worse than one that reads differently from
     * the Rust variant.
     */
      case Text = 'STRING';
    /**
     * Geospatial points and regions, on a sphere. Required for the geo filters.
     */
      case Geo2DSphere = 'GEO2DSPHERE';
    /**
     * Byte strings. Needs server 7.0 or later.
     */
      case Blob = 'BLOB';
        /** The 1.x spelling of a case of this enum. */
        public static function Numeric(): \Aerospike\IndexType {}

        /** The 1.x spelling of a case of this enum. */
        public static function String(): \Aerospike\IndexType {}

        /** The 1.x spelling of a case of this enum. */
        public static function Blob(): \Aerospike\IndexType {}

        /** The 1.x spelling of a case of this enum. */
        public static function Geo2DSphere(): \Aerospike\IndexType {}

    }

    /**
     * The value that sorts above every other, for open-ended CDT range
     * selections.
     *
     * A class rather than a constant on purpose: a PHP class constant can only
     * hold a scalar, and any scalar sentinel — `PHP_INT_MAX`, a magic string — is
     * a value someone will one day store for real, at which point the sentinel
     * silently changes what their range selection means. An object of a dedicated
     * class cannot be confused with data.
     *
     * Nothing consumes it yet: CDT operations are a later phase, and writing one
     * into a bin is rejected by the server. It exists so that the value model in
     * the contract and the one in PHP do not have a hole in the same place.
     */
    class Infinity {
        /**
         * The sentinel carries no state.
         */
        public function __construct() {}
    }

    /**
     * Which record a command is aimed at: a namespace, a set, and a user key.
     *
     * The user key is an `int`, a `string`, or an `Aerospike\Blob` for a binary
     * key. Nothing else: a float key would compare by a representation the server
     * does not define an ordering for, and an array has no digest.
     *
     * ```php
     * $key = new Aerospike\Key("test", "users", "alice");
     * $key = new Aerospike\Key("test", "users", 42);
     * $key = new Aerospike\Key("test", "", "in-the-null-set");
     * ```
     *
     * The digest is **not** available here. The daemon computes it, because the
     * daemon is what builds the real key; exposing a digest this side would mean
     * reimplementing RIPEMD-160 in the extension and having two implementations
     * that must agree.
     */
    class Key {
        /**
         * Name a record.
         *
         * An empty `$set` is the null set, which is legal. An empty `$namespace`
         * is not: every record lives in one, and the server would reject the
         * command with a result code that does not say which argument was blank.
         *
         * @param string $namespace
         * @param string $set
         * @param mixed $userKey
         */
        public function __construct(string $namespace, string $set, mixed $userKey) {}

        /**
         * Property access, for the 1.x client's `$key->namespace` style.
         *
         * Read-only by construction, for the reason [`Record::__get`] gives.
         *
         * @param string $name
         * @return mixed
         */
        public function __get(string $name): mixed {}

        /**
         * `namespace:set:key`, for logs and test failures.
         *
         * A binary key is rendered as its length rather than its bytes, which are
         * not printable and would corrupt whatever is reading the log.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * The server's 20-byte digest, for a key a scan or query returned.
         *
         * `null` for a key the caller constructed — see the field's own note. The
         * digest is what a scan identifies a record by whether or not its user key
         * was stored, so this is the identity that always exists *on the server*;
         * it just is not computed on this side.
         *
         * @return \Aerospike\Blob|null
         */
        public function digest(): ?\Aerospike\Blob {}

        /**
         * Alias of [`Key::digest`], as the 1.x client spelled it — but see the
         * difference in [`Key::digest`]: this is `null` for a key built here.
         *
         * @return \Aerospike\Blob|null
         */
        public function getDigest(): ?\Aerospike\Blob {}

        /**
         * Alias of [`Key::digest`]. The 1.x client returned an array of bytes; this
         * returns an `Aerospike\Blob`, whose `(string)` cast is those bytes.
         *
         * @return \Aerospike\Blob|null
         */
        public function getDigestBytes(): ?\Aerospike\Blob {}

        /**
         * Alias of [`Key::namespace`], as the 1.x client spelled it.
         *
         * @return string
         */
        public function getNamespace(): string {}

        /**
         * Alias of [`Key::partition_id`].
         *
         * @return int|null
         */
        public function getPartitionId(): ?int {}

        /**
         * Alias of [`Key::set_name`], as the 1.x client spelled it — `setname`, one
         * word.
         *
         * There is deliberately no `getSetName()` beside it: **PHP method names are
         * case-insensitive**, so the two would be the same method and the second
         * registration fails at load time with "duplicate name". This client's own
         * spelling is [`Key::set_name`], which reads `setName()`.
         *
         * @return string
         */
        public function getSetname(): string {}

        /**
         * Alias of [`Key::user_key`].
         *
         * @return mixed
         */
        public function getValue(): mixed {}

        /**
         * The namespace.
         *
         * @return string
         */
        public function namespace(): string {}

        /**
         * Which of the 4096 partitions this key falls in, or `null`.
         *
         * Derived from the digest, so it is available exactly when [`Key::digest`]
         * is: the server takes the first two bytes little-endian and masks off the
         * top four bits. Useful for splitting a scan across workers by partition.
         *
         * @return int|null
         */
        public function partitionId(): ?int {}

        /**
         * The set name; `""` for the null set.
         *
         * @return string
         */
        public function setName(): string {}

        /**
         * The user key, as the `int`, `string` or `Aerospike\Blob` it was given
         * as.
         *
         * @return mixed
         */
        public function userKey(): mixed {}
    }

    /**
     * The list operations: `aerospike-core`'s `operations::lists`, method for
     * method.
     *
     * Every index and rank may be negative, counting from the end of the list and
     * from the largest value respectively. Where the Rust client has a pair of
     * functions — `get_range` and `get_range_from` — this has one method with a
     * nullable `$count`, and `null` means "to the end of the list".
     *
     * The `...By...` methods take a [`ListReturn`](crate::ListReturn) saying what
     * to give back, and `$inverted` to select everything *except* what was named —
     * which for a remove operation removes the rest.
     */
    class ListOp {
        public function __construct() {}

        /**
         * Append one value.
         *
         * @param string $bin
         * @param mixed $value
         * @param \Aerospike\ListPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function append(string $bin, mixed $value, ?\Aerospike\ListPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Append several values.
         *
         * @param string $bin
         * @param array $values
         * @param \Aerospike\ListPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function appendItems(string $bin, array $values, ?\Aerospike\ListPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Remove every item.
         *
         * @param string $bin
         * @return \Aerospike\Operation
         */
        public static function clear(string $bin): \Aerospike\Operation {}

        /**
         * Create an empty list in this bin.
         *
         * `persistIndex` keeps an index on a top-level list, which makes index and
         * rank operations cheaper at the cost of some storage. `pad` applies to a
         * nested list reached by an index past the end.
         *
         * @param string $bin
         * @param \Aerospike\ListOrder $order
         * @param bool $pad
         * @param bool $persistIndex
         * @return \Aerospike\Operation
         */
        public static function create(string $bin, \Aerospike\ListOrder $order, bool $pad = false, bool $persistIndex = false): \Aerospike\Operation {}

        /**
         * The item at `index`.
         *
         * @param string $bin
         * @param int $index
         * @return \Aerospike\Operation
         */
        public static function get(string $bin, int $index): \Aerospike\Operation {}

        /**
         * The item at `index`.
         *
         * @param string $bin
         * @param int $index
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByIndex(string $bin, int $index, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * `count` items from `index`; `null` for the rest.
         *
         * @param string $bin
         * @param int $index
         * @param int|null $count
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByIndexRange(string $bin, int $index, ?int $count = null, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * The item at `rank`.
         *
         * @param string $bin
         * @param int $rank
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByRank(string $bin, int $rank, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * `count` items from `rank`; `null` for the rest.
         *
         * @param string $bin
         * @param int $rank
         * @param int|null $count
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByRankRange(string $bin, int $rank, ?int $count = null, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Items equal to `value`.
         *
         * @param string $bin
         * @param mixed $value
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByValue(string $bin, mixed $value, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Items equal to any of `values`.
         *
         * @param string $bin
         * @param array $values
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByValueList(string $bin, array $values, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Items from `begin` inclusive to `end` exclusive; `null` for an open end.
         *
         * @param string $bin
         * @param mixed $begin
         * @param mixed $end
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByValueRange(string $bin, mixed $begin = null, mixed $end = null, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Items by rank relative to `value`.
         *
         * @param string $bin
         * @param mixed $value
         * @param int $rank
         * @param int|null $count
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByValueRelativeRankRange(string $bin, mixed $value, int $rank, ?int $count = null, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * `count` items from `index`; `null` for the rest.
         *
         * @param string $bin
         * @param int $index
         * @param int|null $count
         * @return \Aerospike\Operation
         */
        public static function getRange(string $bin, int $index, ?int $count = null): \Aerospike\Operation {}

        /**
         * Add `value` to the item at `index`; `null` adds one.
         *
         * @param string $bin
         * @param int $index
         * @param int|null $value
         * @param \Aerospike\ListPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function increment(string $bin, int $index, ?int $value = null, ?\Aerospike\ListPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Insert one value at `index`.
         *
         * @param string $bin
         * @param int $index
         * @param mixed $value
         * @param \Aerospike\ListPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function insert(string $bin, int $index, mixed $value, ?\Aerospike\ListPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Insert several values at `index`.
         *
         * @param string $bin
         * @param int $index
         * @param array $values
         * @param \Aerospike\ListPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function insertItems(string $bin, int $index, array $values, ?\Aerospike\ListPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Remove and return the item at `index`.
         *
         * @param string $bin
         * @param int $index
         * @return \Aerospike\Operation
         */
        public static function pop(string $bin, int $index): \Aerospike\Operation {}

        /**
         * Remove and return `count` items from `index`; `null` for the rest.
         *
         * @param string $bin
         * @param int $index
         * @param int|null $count
         * @return \Aerospike\Operation
         */
        public static function popRange(string $bin, int $index, ?int $count = null): \Aerospike\Operation {}

        /**
         * Remove the item at `index`.
         *
         * @param string $bin
         * @param int $index
         * @return \Aerospike\Operation
         */
        public static function remove(string $bin, int $index): \Aerospike\Operation {}

        /**
         * Remove the item at `index`.
         *
         * @param string $bin
         * @param int $index
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByIndex(string $bin, int $index, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove `count` items from `index`; `null` for the rest.
         *
         * @param string $bin
         * @param int $index
         * @param int|null $count
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByIndexRange(string $bin, int $index, ?int $count = null, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove the item at `rank`.
         *
         * @param string $bin
         * @param int $rank
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByRank(string $bin, int $rank, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove `count` items from `rank`; `null` for the rest.
         *
         * @param string $bin
         * @param int $rank
         * @param int|null $count
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByRankRange(string $bin, int $rank, ?int $count = null, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove every item equal to `value`.
         *
         * @param string $bin
         * @param mixed $value
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByValue(string $bin, mixed $value, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove every item equal to any of `values`.
         *
         * @param string $bin
         * @param array $values
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByValueList(string $bin, array $values, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove items from `begin` inclusive to `end` exclusive; `null` for an
         * open end.
         *
         * @param string $bin
         * @param mixed $begin
         * @param mixed $end
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByValueRange(string $bin, mixed $begin = null, mixed $end = null, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove items by rank relative to `value`.
         *
         * @param string $bin
         * @param mixed $value
         * @param int $rank
         * @param int|null $count
         * @param \Aerospike\ListReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByValueRelativeRankRange(string $bin, mixed $value, int $rank, ?int $count = null, ?\Aerospike\ListReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove `count` items from `index`; `null` for the rest.
         *
         * @param string $bin
         * @param int $index
         * @param int|null $count
         * @return \Aerospike\Operation
         */
        public static function removeRange(string $bin, int $index, ?int $count = null): \Aerospike\Operation {}

        /**
         * Replace the item at `index`.
         *
         * @param string $bin
         * @param int $index
         * @param mixed $value
         * @param \Aerospike\ListPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function set(string $bin, int $index, mixed $value, ?\Aerospike\ListPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Change an existing list's order.
         *
         * @param string $bin
         * @param \Aerospike\ListOrder $order
         * @param bool $persistIndex
         * @return \Aerospike\Operation
         */
        public static function setOrder(string $bin, \Aerospike\ListOrder $order, bool $persistIndex = false): \Aerospike\Operation {}

        /**
         * How many items the list holds.
         *
         * @param string $bin
         * @return \Aerospike\Operation
         */
        public static function size(string $bin): \Aerospike\Operation {}

        /**
         * Sort the list in place.
         *
         * @param string $bin
         * @param bool $descending
         * @param bool $dropDuplicates
         * @return \Aerospike\Operation
         */
        public static function sort(string $bin, bool $descending = false, bool $dropDuplicates = false): \Aerospike\Operation {}

        /**
         * Keep `count` items from `index` and remove the rest.
         *
         * @param string $bin
         * @param int $index
         * @param int $count
         * @return \Aerospike\Operation
         */
        public static function trim(string $bin, int $index, int $count): \Aerospike\Operation {}
    }

    /**
     * How a list is stored.
     */
    enum ListOrder: string {
    /**
     * Insertion order, which is the default.
     */
      case Unordered = 'UNORDERED';
    /**
     * Sorted by value, which is what makes the rank and value-range
     * operations run in log time.
     */
      case Ordered = 'ORDERED';
        /** The 1.x spelling of a case of this enum. */
        public static function Ordered(): \Aerospike\ListOrder {}

        /** The 1.x spelling of a case of this enum. */
        public static function Unordered(): \Aerospike\ListOrder {}

    }

    /**
     * Order and write rules for the list operations that add items.
     *
     * ```php
     * // A sorted list that refuses duplicates, and says so rather than failing
     * // the whole operate() when one turns up.
     * $policy = new Aerospike\ListPolicy(
     *     order:     Aerospike\ListOrder::Ordered,
     *     addUnique: true,
     *     noFail:    true,
     * );
     * ```
     */
    class ListPolicy {
        /**
         * Build a list policy. Everything is optional and defaults to the
         * server's own behaviour: an unordered list that accepts anything.
         *
         * @param \Aerospike\ListOrder|null $order
         * @param bool $addUnique
         * @param bool $insertBounded
         * @param bool $noFail
         * @param bool $partial
         */
        public function __construct(?\Aerospike\ListOrder $order = null, bool $addUnique = false, bool $insertBounded = false, bool $noFail = false, bool $partial = false) {}

        /**
         * Whether duplicate values are rejected.
         *
         * @return bool
         */
        public function addUnique(): bool {}

        /**
         * Whether an insert outside the list's range is refused.
         *
         * @return bool
         */
        public function insertBounded(): bool {}

        /**
         * Whether the list this policy creates is value-ordered.
         *
         * @return bool
         */
        public function isOrdered(): bool {}

        /**
         * Whether a rejected item leaves the operation successful.
         *
         * @return bool
         */
        public function noFail(): bool {}

        /**
         * Whether acceptable items are applied when others are rejected.
         *
         * @return bool
         */
        public function partial(): bool {}
    }

    /**
     * What a list operation gives back.
     *
     * Every `...By...` operation takes one. `Values` is what most callers want;
     * `None` is what to ask for when the operation is being run for its effect and
     * the result would only be shipped back to be discarded.
     */
    enum ListReturn: string {
    /**
     * Nothing at all.
     */
      case None = 'NONE';
    /**
     * The index of each selected item.
     */
      case Index = 'INDEX';
    /**
     * The index of each selected item, counted from the end.
     */
      case ReverseIndex = 'REVERSE_INDEX';
    /**
     * The rank of each selected item — its position in value order.
     */
      case Rank = 'RANK';
    /**
     * The rank of each selected item, counted from the largest.
     */
      case ReverseRank = 'REVERSE_RANK';
    /**
     * How many items were selected.
     */
      case Count = 'COUNT';
    /**
     * The selected values themselves.
     */
      case Values = 'VALUES';
    /**
     * Whether anything was selected at all.
     */
      case Exists = 'EXISTS';
        /** The 1.x spelling of a case of this enum. */
        public static function None(): \Aerospike\ListReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Index(): \Aerospike\ListReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function ReverseIndex(): \Aerospike\ListReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Rank(): \Aerospike\ListReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function ReverseRank(): \Aerospike\ListReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function count(): \Aerospike\ListReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Value(): \Aerospike\ListReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Exists(): \Aerospike\ListReturn {}

    }

    /**
     * Which side of the child a loop variable refers to.
     *
     * Inside a fan-out, the loop variable stands for the node being considered. A map
     * child has a key, a value and an index; a list child has a value and an index.
     *
     * An enum rather than constants — unlike `SelectFlag` — because these do not
     * combine: a loop variable refers to exactly one part.
     */
    enum LoopVarPart: string {
    /**
     * The child's map key.
     */
      case MapKey = 'MAP_KEY';
    /**
     * The child's value — a list element, or a map value.
     */
      case Value = 'VALUE';
    /**
     * The child's list index.
     */
      case Index = 'INDEX';
    }

    /**
     * The map operations: `aerospike-core`'s `operations::maps`, method for method.
     *
     * A map entry has a key *and* a value, so most operations come in both flavours
     * — `getByKey` and `getByValue`, `removeByKeyRange` and `removeByValueRange` —
     * and `$returnType` decides which half comes back. It defaults to
     * `MapReturn::Value`.
     *
     * As with lists, indexes and ranks may be negative, and a `null` `$count` means
     * "to the end".
     */
    class MapOp {
        public function __construct() {}

        /**
         * Remove every entry.
         *
         * @param string $bin
         * @return \Aerospike\Operation
         */
        public static function clear(string $bin): \Aerospike\Operation {}

        /**
         * Create an empty map in this bin.
         *
         * With a path attached this creates the map *at that path*; without one it
         * is the same as `setOrder`. `persistIndex` applies to a top-level map only.
         *
         * @param string $bin
         * @param \Aerospike\MapOrder $order
         * @param bool $persistIndex
         * @return \Aerospike\Operation
         */
        public static function create(string $bin, \Aerospike\MapOrder $order, bool $persistIndex = false): \Aerospike\Operation {}

        /**
         * Subtract a delta from one entry's value.
         *
         * @param string $bin
         * @param mixed $key
         * @param mixed $delta
         * @param \Aerospike\MapPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function decrementValue(string $bin, mixed $key, mixed $delta, ?\Aerospike\MapPolicy $policy = null): \Aerospike\Operation {}

        /**
         * The entry at this key index.
         *
         * @param string $bin
         * @param int $index
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByIndex(string $bin, int $index, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * `count` entries from this key index; `null` for the rest.
         *
         * @param string $bin
         * @param int $index
         * @param int|null $count
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByIndexRange(string $bin, int $index, ?int $count = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * The entry with this key.
         *
         * @param string $bin
         * @param mixed $key
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByKey(string $bin, mixed $key, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * The entries with any of these keys.
         *
         * @param string $bin
         * @param array $keys
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByKeyList(string $bin, array $keys, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Entries whose keys run from `begin` inclusive to `end` exclusive; `null`
         * for an open end.
         *
         * @param string $bin
         * @param mixed $begin
         * @param mixed $end
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByKeyRange(string $bin, mixed $begin = null, mixed $end = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Entries by key index relative to a key.
         *
         * @param string $bin
         * @param mixed $key
         * @param int $index
         * @param int|null $count
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByKeyRelativeIndexRange(string $bin, mixed $key, int $index, ?int $count = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * The entry at this rank.
         *
         * @param string $bin
         * @param int $rank
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByRank(string $bin, int $rank, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * `count` entries from this rank; `null` for the rest.
         *
         * @param string $bin
         * @param int $rank
         * @param int|null $count
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByRankRange(string $bin, int $rank, ?int $count = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Entries with this value.
         *
         * @param string $bin
         * @param mixed $value
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByValue(string $bin, mixed $value, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Entries with any of these values.
         *
         * @param string $bin
         * @param array $values
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByValueList(string $bin, array $values, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Entries whose values are in a range.
         *
         * @param string $bin
         * @param mixed $begin
         * @param mixed $end
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByValueRange(string $bin, mixed $begin = null, mixed $end = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Entries by rank relative to a value.
         *
         * @param string $bin
         * @param mixed $value
         * @param int $rank
         * @param int|null $count
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function getByValueRelativeRankRange(string $bin, mixed $value, int $rank, ?int $count = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Add a delta to one entry's value, creating the entry if it is missing.
         *
         * @param string $bin
         * @param mixed $key
         * @param mixed $delta
         * @param \Aerospike\MapPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function incrementValue(string $bin, mixed $key, mixed $delta, ?\Aerospike\MapPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Write one entry.
         *
         * @param string $bin
         * @param mixed $key
         * @param mixed $value
         * @param \Aerospike\MapPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function put(string $bin, mixed $key, mixed $value, ?\Aerospike\MapPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Write several entries, given as a PHP array, an `Aerospike\\OrderedMap`
         * or an `Aerospike\\SortedMap`.
         *
         * @param string $bin
         * @param mixed $items
         * @param \Aerospike\MapPolicy|null $policy
         * @return \Aerospike\Operation
         */
        public static function putItems(string $bin, mixed $items, ?\Aerospike\MapPolicy $policy = null): \Aerospike\Operation {}

        /**
         * Remove the entry at this key index.
         *
         * @param string $bin
         * @param int $index
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByIndex(string $bin, int $index, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove `count` entries from this key index; `null` for the rest.
         *
         * @param string $bin
         * @param int $index
         * @param int|null $count
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByIndexRange(string $bin, int $index, ?int $count = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove the entry with this key.
         *
         * @param string $bin
         * @param mixed $key
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByKey(string $bin, mixed $key, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove the entries with any of these keys.
         *
         * @param string $bin
         * @param array $keys
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByKeyList(string $bin, array $keys, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove entries whose keys run from `begin` inclusive to `end` exclusive;
         * `null` for an open end.
         *
         * @param string $bin
         * @param mixed $begin
         * @param mixed $end
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByKeyRange(string $bin, mixed $begin = null, mixed $end = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove entries by key index relative to a key.
         *
         * @param string $bin
         * @param mixed $key
         * @param int $index
         * @param int|null $count
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByKeyRelativeIndexRange(string $bin, mixed $key, int $index, ?int $count = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove the entry at this rank.
         *
         * @param string $bin
         * @param int $rank
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByRank(string $bin, int $rank, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove `count` entries from this rank; `null` for the rest.
         *
         * @param string $bin
         * @param int $rank
         * @param int|null $count
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByRankRange(string $bin, int $rank, ?int $count = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove entries with this value.
         *
         * @param string $bin
         * @param mixed $value
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByValue(string $bin, mixed $value, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove entries with any of these values.
         *
         * @param string $bin
         * @param array $values
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByValueList(string $bin, array $values, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove entries whose values are in a range.
         *
         * @param string $bin
         * @param mixed $begin
         * @param mixed $end
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByValueRange(string $bin, mixed $begin = null, mixed $end = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Remove entries by rank relative to a value.
         *
         * @param string $bin
         * @param mixed $value
         * @param int $rank
         * @param int|null $count
         * @param \Aerospike\MapReturn|null $returnType
         * @param bool $inverted
         * @return \Aerospike\Operation
         */
        public static function removeByValueRelativeRankRange(string $bin, mixed $value, int $rank, ?int $count = null, ?\Aerospike\MapReturn $returnType = null, bool $inverted = false): \Aerospike\Operation {}

        /**
         * Change an existing map's order.
         *
         * @param string $bin
         * @param \Aerospike\MapOrder $order
         * @return \Aerospike\Operation
         */
        public static function setOrder(string $bin, \Aerospike\MapOrder $order): \Aerospike\Operation {}

        /**
         * Replace the map's policy.
         *
         * @param string $bin
         * @param \Aerospike\MapPolicy $policy
         * @return \Aerospike\Operation
         */
        public static function setPolicy(string $bin, \Aerospike\MapPolicy $policy): \Aerospike\Operation {}

        /**
         * How many entries the map holds.
         *
         * @param string $bin
         * @return \Aerospike\Operation
         */
        public static function size(string $bin): \Aerospike\Operation {}
    }

    /**
     * How a map is stored.
     *
     * Only reachable through `Ctx::mapKeyCreate()` for now — the map *operations*
     * arrive with the next phase — because a list nested inside a map has to be
     * able to say what kind of map to create on the way down.
     */
    enum MapOrder: string {
    /**
     * Insertion order.
     */
      case Unordered = 'UNORDERED';
    /**
     * Sorted by key.
     */
      case KeyOrdered = 'KEY_ORDERED';
    /**
     * Sorted by key, then by value.
     */
      case KeyValueOrdered = 'KEY_VALUE_ORDERED';
        /** The 1.x spelling of a case of this enum. */
        public static function Unordered(): \Aerospike\MapOrder {}

        /** The 1.x spelling of a case of this enum. */
        public static function KeyOrdered(): \Aerospike\MapOrder {}

        /** The 1.x spelling of a case of this enum. */
        public static function KeyValueOrdered(): \Aerospike\MapOrder {}

    }

    /**
     * Order and write rules for the map operations that write.
     *
     * ```php
     * // A key-ordered map whose writes must not create new keys, and which
     * // silently skips the ones that would.
     * $policy = new Aerospike\MapPolicy(
     *     order:     Aerospike\MapOrder::KeyOrdered,
     *     writeMode: Aerospike\MapWriteMode::UpdateOnly,
     *     noFail:    true,
     *     partial:   true,
     * );
     * ```
     *
     * The Rust client has a write *mode* and a flag bitmask that replaces the mode
     * when it is non-zero — two ways to say the same thing. This carries the mode
     * plus the two flags that say something the mode cannot, and the daemon
     * combines them.
     */
    class MapPolicy {
        /**
         * Build a map policy. Everything is optional; the default is an unordered
         * map whose writes create or overwrite.
         *
         * @param \Aerospike\MapOrder|null $order
         * @param \Aerospike\MapWriteMode|null $writeMode
         * @param bool $noFail
         * @param bool $partial
         * @param bool $persistIndex
         */
        public function __construct(?\Aerospike\MapOrder $order = null, ?\Aerospike\MapWriteMode $writeMode = null, bool $noFail = false, bool $partial = false, bool $persistIndex = false) {}

        /**
         * Whether a created map is key-ordered or key/value-ordered.
         *
         * @return bool
         */
        public function isOrdered(): bool {}

        /**
         * Whether a rejected entry leaves the operation successful.
         *
         * @return bool
         */
        public function noFail(): bool {}

        /**
         * Whether acceptable entries are applied when others are rejected.
         *
         * @return bool
         */
        public function partial(): bool {}

        /**
         * Whether the map keeps a persistent index.
         *
         * @return bool
         */
        public function persistIndex(): bool {}
    }

    /**
     * What a map operation gives back.
     *
     * Richer than [`ListReturn`] because a map entry has two halves: an operation
     * can hand back the keys, the values, both as pairs, or the selection as a map.
     */
    enum MapReturn: string {
    /**
     * Nothing at all.
     */
      case None = 'NONE';
    /**
     * The key index of each selected entry.
     */
      case Index = 'INDEX';
    /**
     * The key index of each selected entry, counted from the end.
     */
      case ReverseIndex = 'REVERSE_INDEX';
    /**
     * The rank of each selected entry — its position in value order.
     */
      case Rank = 'RANK';
    /**
     * The rank of each selected entry, counted from the largest.
     */
      case ReverseRank = 'REVERSE_RANK';
    /**
     * How many entries were selected.
     */
      case Count = 'COUNT';
    /**
     * The keys.
     */
      case Key = 'KEY';
    /**
     * The values.
     */
      case Value = 'VALUE';
    /**
     * Both, as `[key, value]` pairs.
     */
      case KeyValue = 'KEY_VALUE';
    /**
     * Whether anything was selected at all.
     */
      case Exists = 'EXISTS';
    /**
     * The selection as an unordered map.
     */
      case UnorderedMap = 'UNORDERED_MAP';
    /**
     * The selection as an ordered map.
     */
      case OrderedMap = 'ORDERED_MAP';
        /** The 1.x spelling of a case of this enum. */
        public static function None(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Index(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function ReverseIndex(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Rank(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function ReverseRank(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Count(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Key(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Value(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function KeyValue(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function Exists(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function UnorderedMap(): \Aerospike\MapReturn {}

        /** The 1.x spelling of a case of this enum. */
        public static function OrderedMap(): \Aerospike\MapReturn {}

    }

    /**
     * What a map write does about a key that is, or is not, already there.
     */
    enum MapWriteMode: string {
    /**
     * Create the entry, or overwrite it. The default.
     */
      case Update = 'UPDATE';
    /**
     * Overwrite only; fail if the key is not there.
     */
      case UpdateOnly = 'UPDATE_ONLY';
    /**
     * Create only; fail if the key is already there.
     */
      case CreateOnly = 'CREATE_ONLY';
        /** The 1.x spelling of a case of this enum. */
        public static function Update(): \Aerospike\MapWriteMode {}

        /** The 1.x spelling of a case of this enum. */
        public static function UpdateOnly(): \Aerospike\MapWriteMode {}

        /** The 1.x spelling of a case of this enum. */
        public static function CreateOnly(): \Aerospike\MapWriteMode {}

    }

    /**
     * Flags for [`ExpPath::modify_by_path`], combined with `|`.
     */
    class ModifyFlag {
        /**
         * Fail on a type mismatch.
         */
        const DEFAULT = 0;

        /**
         * Skip nodes of the wrong type instead of failing.
         */
        const NO_FAIL = 16;

        public function __construct() {}
    }

    /**
     * One node of a cluster, as the daemon's tend loop last saw it.
     *
     * Produced by `nodes()`. The daemon's view rather than a fresh query: this is
     * where the client would route a command right now, which is the useful answer.
     */
    class Node {
        public function __construct() {}

        /**
         * `name@address`, for logs.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * The address the daemon reaches it on.
         *
         * @return string
         */
        public function address(): string {}

        /**
         * Whether the daemon currently considers it usable.
         *
         * A node the daemon has stopped believing in is still listed, because
         * "listed but inactive" is information a caller wants — a list that quietly
         * omitted it would look like a smaller cluster.
         *
         * @return bool
         */
        public function isActive(): bool {}

        /**
         * The node's name, which is what `info()`'s `$node` argument takes.
         *
         * @return string
         */
        public function name(): string {}

        /**
         * Its server version, as `major.minor.patch.build`.
         *
         * @return string
         */
        public function version(): string {}
    }

    /**
     * The operations that work on any bin: `aerospike-core`'s
     * `operations::scalar`, method for method.
     */
    class Op {
        public function __construct() {}

        /**
         * Add a numeric delta to a bin. Negative subtracts.
         *
         * @param \Aerospike\Bin $bin
         * @return \Aerospike\Operation
         */
        public static function add(\Aerospike\Bin $bin): \Aerospike\Operation {}

        /**
         * Append to a string or blob bin.
         *
         * @param \Aerospike\Bin $bin
         * @return \Aerospike\Operation
         */
        public static function append(\Aerospike\Bin $bin): \Aerospike\Operation {}

        /**
         * Delete the record.
         *
         * Operations after this one in the same call still run, which is how a
         * record is replaced rather than merged.
         *
         * @return \Aerospike\Operation
         */
        public static function delete(): \Aerospike\Operation {}

        /**
         * Read every bin of the record.
         *
         * @return \Aerospike\Operation
         */
        public static function get(): \Aerospike\Operation {}

        /**
         * Read one bin.
         *
         * @param string $bin
         * @return \Aerospike\Operation
         */
        public static function getBin(string $bin): \Aerospike\Operation {}

        /**
         * Read the record's metadata and no bins.
         *
         * @return \Aerospike\Operation
         */
        public static function getHeader(): \Aerospike\Operation {}

        /**
         * Prepend to a string or blob bin.
         *
         * @param \Aerospike\Bin $bin
         * @return \Aerospike\Operation
         */
        public static function prepend(\Aerospike\Bin $bin): \Aerospike\Operation {}

        /**
         * Write one bin. A `null` value deletes it.
         *
         * @param \Aerospike\Bin $bin
         * @return \Aerospike\Operation
         */
        public static function put(\Aerospike\Bin $bin): \Aerospike\Operation {}

        /**
         * Reset the record's time-to-live, using the policy's `expiration`.
         *
         * @return \Aerospike\Operation
         */
        public static function touch(): \Aerospike\Operation {}
    }

    /**
     * One operation in an `operate()` call.
     *
     * Built by the static methods of [`Op`], [`ListOp`], [`MapOp`], [`BitOp`],
     * [`HllOp`] and [`ExpOp`]; there is no
     * constructor, because an operation with no operation in it is not a thing.
     */
    class Operation {
        public function __construct() {}

        /**
         * The bin this operation reads or writes, or `null` for the whole-record
         * operations.
         *
         * @return string|null
         */
        public function bin(): ?string {}

        /**
         * Aim this operation at a collection nested inside the bin.
         *
         * Returns a **new** operation: an operation is a value, and one that
         * mutated when a path was attached could not be shared between calls.
         *
         * Only the collection operations can be nested. Attaching a path to a
         * scalar operation is refused rather than ignored — the server would run
         * the operation on the bin itself, which is not what the path asked for.
         *
         * @param array $ctx
         * @return \Aerospike\Operation
         */
        public function context(array $ctx): \Aerospike\Operation {}

        /**
         * Whether this operation writes.
         *
         * An `operate()` call with any write in it takes a write lock on the
         * record and bumps its generation, so this is worth being able to check.
         *
         * @return bool
         */
        public function isWrite(): bool {}
    }

    /**
     * A map whose entries keep the order they were given.
     *
     * **The server does not store this order.** Aerospike has no insertion-ordered
     * map, so an `OrderedMap` is written as an unordered map and reads back as a
     * plain PHP array. What it buys is the *sending* side: the entries reach the
     * server in the order you wrote them, which is what the client's own
     * insertion-ordered map type means, and the order is preserved locally for as
     * long as you hold the object.
     *
     * If you want an order the server keeps, use [`SortedMap`].
     */
    class OrderedMap implements \ArrayAccess, \Iterator, \Countable {
        /**
         * Build one, optionally from a PHP array.
         *
         * @param array|null $entries
         */
        public function __construct(?array $entries = null) {}

        /**
         * How many entries the map holds.
         *
         * @return int
         */
        public function count(): int {}

        /**
         * The value at the cursor.
         *
         * @return mixed
         */
        public function current(): mixed {}

        /**
         * One entry's value, or `null` if the key is not there.
         *
         * @param mixed $key
         * @return mixed
         */
        public function get(mixed $key): mixed {}

        /**
         * Whether the map has this key.
         *
         * @param mixed $key
         * @return bool
         */
        public function has(mixed $key): bool {}

        /**
         * Whether the map is empty.
         *
         * @return bool
         */
        public function isEmpty(): bool {}

        /**
         * The key at the cursor. Not restricted to what a PHP array key can hold.
         *
         * @return mixed
         */
        public function key(): mixed {}

        /**
         * The keys, in order.
         *
         * @return array
         */
        public function keys(): array {}

        /**
         * Advance the cursor.
         *
         * @return void
         */
        public function next(): void {}

        /**
         * `isset($map[$key])`.
         *
         * @param mixed $offset
         * @return bool
         */
        public function offsetExists(mixed $offset): bool {}

        /**
         * `$map[$key]`.
         *
         * @param mixed $offset
         * @return mixed
         */
        public function offsetGet(mixed $offset): mixed {}

        /**
         * `$map[$key] = $value`.
         *
         * @param mixed $offset
         * @param mixed $value
         * @return void
         */
        public function offsetSet(mixed $offset, mixed $value): void {}

        /**
         * `unset($map[$key])`.
         *
         * @param mixed $offset
         * @return void
         */
        public function offsetUnset(mixed $offset): void {}

        /**
         * Remove one entry, reporting whether it was there.
         *
         * @param mixed $key
         * @return bool
         */
        public function remove(mixed $key): bool {}

        /**
         * Start an iteration.
         *
         * @return void
         */
        public function rewind(): void {}

        /**
         * Set one entry, replacing an existing key in place.
         *
         * @param mixed $key
         * @param mixed $value
         * @return void
         */
        public function set(mixed $key, mixed $value): void {}

        /**
         * The same entries as a PHP array.
         *
         * Throws for a key a PHP array cannot hold — a blob or a float — since
         * carrying those is one of the two reasons this class exists.
         *
         * @return array
         */
        public function toArray(): array {}

        /**
         * Whether the iteration has an entry to give.
         *
         * @return bool
         */
        public function valid(): bool {}

        /**
         * The values, in order.
         *
         * @return array
         */
        public function values(): array {}
    }

    /**
     * Which partitions a scan or query covers.
     *
     * The whole ring unless told otherwise. The narrower forms are how one scan is
     * divided between several workers: give each a partition range and no record is
     * seen twice, without any coordination between them.
     *
     * ```php
     * Aerospike\PartitionFilter::all();
     * Aerospike\PartitionFilter::byRange(0, 1024);      // a quarter of the ring
     * Aerospike\PartitionFilter::byId(7);
     * Aerospike\PartitionFilter::after($key);           // resume after a key
     * ```
     */
    class PartitionFilter {
        public function __construct() {}

        /**
         * Records after this key's digest, in the partition that holds it.
         *
         * Digest order, which is not key order and not insertion order — so this
         * resumes a scan that stopped at a known record, and is not a way to page
         * through a set in any meaningful sequence.
         *
         * Primary-index scans only: a digest is not enough to resume a
         * secondary-index query, so a statement with a filter must not use this.
         *
         * @param \Aerospike\Key $key
         * @return \Aerospike\PartitionFilter
         */
        public static function after(\Aerospike\Key $key): \Aerospike\PartitionFilter {}

        /**
         * Every partition. The same as passing `null`.
         *
         * @return \Aerospike\PartitionFilter
         */
        public static function all(): \Aerospike\PartitionFilter {}

        /**
         * One partition, by id.
         *
         * Ids run from 0 to 4095. A record's partition is a function of its digest,
         * so this is only useful for dividing a scan up, never for finding a
         * particular record.
         *
         * @param int $id
         * @return \Aerospike\PartitionFilter
         */
        public static function byId(int $id): \Aerospike\PartitionFilter {}

        /**
         * A contiguous range of partitions: `$count` of them, from `$begin`.
         *
         * @param int $begin
         * @param int $count
         * @return \Aerospike\PartitionFilter
         */
        public static function byRange(int $begin, int $count): \Aerospike\PartitionFilter {}

        /**
         * Whether this covers the whole ring.
         *
         * @return bool
         */
        public function isAll(): bool {}
    }

    /**
     * One privilege: what may be done, and where.
     *
     * ```php
     * use Aerospike\{Privilege, PrivilegeCode};
     *
     * new Privilege(PrivilegeCode::ReadWrite);                        // every namespace
     * new Privilege(PrivilegeCode::Read, 'test');                     // one namespace
     * new Privilege(PrivilegeCode::Read, 'test', 'users');            // one set
     * new Privilege(PrivilegeCode::SysAdmin);                         // cluster-wide
     * ```
     *
     * **The administrative codes cannot be confined to a namespace.** `UserAdmin`,
     * `SysAdmin`, `DataAdmin`, `UdfAdmin`, `SIndexAdmin` and `MaskingAdmin` act on the
     * cluster, so a namespace means nothing for them — and the server refuses the
     * combination with a parameter error that names neither the privilege nor the
     * reason. This constructor refuses it instead, and says which codes *can* be
     * scoped.
     */
    class Privilege {
        /**
         * Name a privilege, optionally confined to a namespace and a set.
         *
         * A set without a namespace is refused: a set scope is *within* a namespace,
         * so a set alone describes nothing the server can act on.
         *
         * @param \Aerospike\PrivilegeCode $code
         * @param string|null $namespace
         * @param string|null $setName
         */
        public function __construct(\Aerospike\PrivilegeCode $code, ?string $namespace = null, ?string $setName = null) {}

        /**
         * A readable form, for logs and test failures.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * What is permitted.
         *
         * @return \Aerospike\PrivilegeCode
         */
        public function code(): \Aerospike\PrivilegeCode {}

        /**
         * Whether this privilege names a namespace or a set.
         *
         * @return bool
         */
        public function isScoped(): bool {}

        /**
         * The namespace this is confined to, or `null` for every namespace.
         *
         * @return string|null
         */
        public function namespace(): ?string {}

        /**
         * The set this is confined to, or `null` for every set.
         *
         * @return string|null
         */
        public function setName(): ?string {}
    }

    /**
     * A default privilege the server defines.
     *
     * The six administrative codes act on the **cluster**, so they cannot be confined
     * to a namespace — `Aerospike\Privilege` refuses that, because the server's own
     * refusal names neither the privilege nor the reason. The seven data codes can.
     *
     * ```php
     * new Aerospike\Privilege(Aerospike\PrivilegeCode::ReadWrite, 'test');   // fine
     * new Aerospike\Privilege(Aerospike\PrivilegeCode::SysAdmin, 'test');    // refused
     * ```
     */
    enum PrivilegeCode: string {
    /**
     * Edit and remove other users. Cluster-wide.
     */
      case UserAdmin = 'USER_ADMIN';
    /**
     * Systems administration that is not user administration — server
     * configuration, for instance. Cluster-wide.
     */
      case SysAdmin = 'SYS_ADMIN';
    /**
     * UDF and secondary-index administration. Cluster-wide.
     */
      case DataAdmin = 'DATA_ADMIN';
    /**
     * UDF administration alone. Cluster-wide; needs server 6+.
     */
      case UdfAdmin = 'UDF_ADMIN';
    /**
     * Secondary-index administration alone. Cluster-wide; needs server 6+.
     */
      case SIndexAdmin = 'SINDEX_ADMIN';
    /**
     * Read data. May be confined to a namespace or a set.
     */
      case Read = 'READ';
    /**
     * Read and write data. May be confined.
     */
      case ReadWrite = 'READ_WRITE';
    /**
     * Read and write data through user-defined functions. May be confined.
     */
      case ReadWriteUdf = 'READ_WRITE_UDF';
    /**
     * Write data. May be confined.
     */
      case Write = 'WRITE';
    /**
     * Truncate data. May be confined; needs server 6+.
     */
      case Truncate = 'TRUNCATE';
    /**
     * Data-masking administration. Cluster-wide.
     */
      case MaskingAdmin = 'MASKING_ADMIN';
    /**
     * Read masked data. May be confined.
     */
      case ReadMasked = 'READ_MASKED';
    /**
     * Write masked data. May be confined.
     */
      case WriteMasked = 'WRITE_MASKED';
    }

    /**
     * Per-call settings for a scan or a query.
     *
     * Mirrors `aerospike-core`'s `QueryPolicy`: everything a [`ReadPolicy`] has —
     * a traversal is a read of many records — plus the four settings that only make
     * sense when there is more than one record and more than one page.
     *
     * ```php
     * $policy = new Aerospike\QueryPolicy(
     *     maxRecords:       10_000,   // stop after this many, across every page
     *     pageSize:         500,      // records per round trip
     *     recordsPerSecond: 1_000,    // per node, to leave the cluster room
     *     includeBinData:   false,    // keys and metadata only
     * );
     * foreach ($client->query($policy, null, $statement) as $record) { … }
     * ```
     *
     * # There is no total timeout by default, and that is deliberate
     *
     * A `ReadPolicy` inherits the daemon's `default_timeout` — a second, sized for a
     * single-record command. A scan of a large set legitimately takes longer than
     * that, so this one starts with **no** deadline, exactly as the Rust client's
     * `QueryPolicy` does. Set `totalTimeoutMs` if a page needs a bound; the worker's
     * own `aerospike.timeout_ms` still bounds how long it waits for one.
     */
    class QueryPolicy {
        /**
         * Build a query policy. Every argument is optional; an omitted one leaves
         * the daemon's own setting alone.
         *
         * camelCase parameters and `= null` defaults, for the reasons
         * [`ReadPolicy::__construct`] gives.
         *
         * @param int|null $totalTimeoutMs
         * @param int|null $socketTimeoutMs
         * @param int|null $maxRetries
         * @param int|null $sleepBetweenRetriesMs
         * @param \Aerospike\Replica|null $replica
         * @param \Aerospike\ReadModeAP|null $readModeAp
         * @param \Aerospike\ReadModeSC|null $readModeSc
         * @param bool|null $useCompression
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @param int|null $maxRecords
         * @param int|null $recordsPerSecond
         * @param int|null $pageSize
         * @param bool|null $includeBinData
         */
        public function __construct(?int $totalTimeoutMs = null, ?int $socketTimeoutMs = null, ?int $maxRetries = null, ?int $sleepBetweenRetriesMs = null, ?\Aerospike\Replica $replica = null, ?\Aerospike\ReadModeAP $readModeAp = null, ?\Aerospike\ReadModeSC $readModeSc = null, ?bool $useCompression = null, ?string $filter = null, ?\Aerospike\Expression $filterExp = null, ?int $maxRecords = null, ?int $recordsPerSecond = null, ?int $pageSize = null, ?bool $includeBinData = null) {}

        /**
         * The filter as Aerospike Expression Language text.
         *
         * `null` when there is no filter **or** when it was built with
         * `Aerospike\\Exp` — a built expression never was text, and inventing some
         * would mean writing a serialiser nothing reads. Use
         * [`filter_exp`](Self::filter_exp) to get the filter whatever its form.
         *
         * @return string|null
         */
        public function filter(): ?string {}

        /**
         * The filter, in whichever form it was given.
         *
         * @return \Aerospike\Expression|null
         */
        public function filterExp(): ?\Aerospike\Expression {}

        /**
         * Whether bins come back, or `null` for the default, which is that they do.
         *
         * @return bool|null
         */
        public function includeBinData(): ?bool {}

        /**
         * How many records the whole traversal may return, or `null` for no limit.
         *
         * @return int|null
         */
        public function maxRecords(): ?int {}

        /**
         * Retries after the first attempt.
         *
         * @return int|null
         */
        public function maxRetries(): ?int {}

        /**
         * Records per page, or `null` for the daemon's configured default.
         *
         * @return int|null
         */
        public function pageSize(): ?int {}

        /**
         * AP-namespace read consistency.
         *
         * @return \Aerospike\ReadModeAP|null
         */
        public function readModeAp(): ?\Aerospike\ReadModeAP {}

        /**
         * SC-namespace read consistency.
         *
         * @return \Aerospike\ReadModeSC|null
         */
        public function readModeSc(): ?\Aerospike\ReadModeSC {}

        /**
         * Per-node rate limit, or `null` for unlimited.
         *
         * @return int|null
         */
        public function recordsPerSecond(): ?int {}

        /**
         * Which node to prefer.
         *
         * @return \Aerospike\Replica|null
         */
        public function replica(): ?\Aerospike\Replica {}

        /**
         * Pause between retries.
         *
         * @return int|null
         */
        public function sleepBetweenRetriesMs(): ?int {}

        /**
         * Deadline for one socket operation.
         *
         * @return int|null
         */
        public function socketTimeoutMs(): ?int {}

        /**
         * Deadline for the whole command, retries included.
         *
         * @return int|null
         */
        public function totalTimeoutMs(): ?int {}

        /**
         * Whether to compress a request worth compressing.
         *
         * @return bool|null
         */
        public function useCompression(): ?bool {}
    }

    /**
     * Read consistency in an availability-mode (AP) namespace.
     */
    enum ReadModeAP: string {
    /**
     * One replica.
     */
      case One = 'ONE';
    /**
     * All replicas, so a conflict is detected.
     */
      case All = 'ALL';
        /** The 1.x spelling of a case of this enum. */
        public static function one(): \Aerospike\ReadModeAP {}

        /** The 1.x spelling of a case of this enum. */
        public static function all(): \Aerospike\ReadModeAP {}

    }

    /**
     * Read consistency in a strong-consistency (SC) namespace.
     */
    enum ReadModeSC: string {
    /**
     * Session consistency: never read older than this client has written.
     */
      case Session = 'SESSION';
    /**
     * Linearizable across all clients.
     */
      case Linearize = 'LINEARIZE';
    /**
     * Allow a possibly stale read from a replica.
     */
      case AllowReplica = 'ALLOW_REPLICA';
    /**
     * Allow a read from an unavailable partition.
     */
      case AllowUnavailable = 'ALLOW_UNAVAILABLE';
        /** The 1.x spelling of a case of this enum. */
        public static function Session(): \Aerospike\ReadModeSC {}

        /** The 1.x spelling of a case of this enum. */
        public static function Linearize(): \Aerospike\ReadModeSC {}

        /** The 1.x spelling of a case of this enum. */
        public static function AllowReplica(): \Aerospike\ReadModeSC {}

        /** The 1.x spelling of a case of this enum. */
        public static function AllowUnavailable(): \Aerospike\ReadModeSC {}

    }

    /**
     * Per-call settings for a read: `get` and `exists`.
     *
     * Mirrors `aerospike-core`'s `ReadPolicy`. Pass `null` where one of these is
     * expected to accept the daemon's configuration unchanged, which is what
     * nearly every call should do.
     *
     * ```php
     * $policy = new Aerospike\ReadPolicy(
     *     totalTimeoutMs: 500,
     *     maxRetries:     1,
     *     replica:        Aerospike\Replica::PreferRack,
     *     readModeSc:     Aerospike\ReadModeSC::Linearize,
     * );
     * $record = $client->get($policy, $key);
     * ```
     */
    class ReadPolicy {
        /**
         * Build a read policy. Every argument is optional; an omitted one leaves
         * the daemon's own setting alone.
         *
         * The parameters are camelCase because **PHP named arguments use the
         * parameter name verbatim** — `new ReadPolicy(totalTimeoutMs: 500)` — and
         * a policy is meant to be written with named arguments, since nobody
         * should be counting nine positions. That is also what makes them match
         * the getters, which PHP sees as `totalTimeoutMs()`.
         *
         * The explicit `= null` defaults are what make skipping possible: PHP
         * refuses a named argument that leaves an earlier parameter with no
         * *known* default, so without these `new ReadPolicy(filter: '...')` would
         * be an `ArgumentCountError`.
         *
         * @param int|null $totalTimeoutMs
         * @param int|null $socketTimeoutMs
         * @param int|null $maxRetries
         * @param int|null $sleepBetweenRetriesMs
         * @param \Aerospike\Replica|null $replica
         * @param \Aerospike\ReadModeAP|null $readModeAp
         * @param \Aerospike\ReadModeSC|null $readModeSc
         * @param bool|null $useCompression
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @param \Aerospike\Transaction|null $txn
         */
        public function __construct(?int $totalTimeoutMs = null, ?int $socketTimeoutMs = null, ?int $maxRetries = null, ?int $sleepBetweenRetriesMs = null, ?\Aerospike\Replica $replica = null, ?\Aerospike\ReadModeAP $readModeAp = null, ?\Aerospike\ReadModeSC $readModeSc = null, ?bool $useCompression = null, ?string $filter = null, ?\Aerospike\Expression $filterExp = null, ?\Aerospike\Transaction $txn = null) {}

        /**
         * The filter as Aerospike Expression Language text.
         *
         * `null` when there is no filter **or** when it was built with
         * `Aerospike\\Exp` — a built expression never was text, and inventing some
         * would mean writing a serialiser nothing reads. Use
         * [`filter_exp`](Self::filter_exp) to get the filter whatever its form.
         *
         * @return string|null
         */
        public function filter(): ?string {}

        /**
         * The filter, in whichever form it was given.
         *
         * @return \Aerospike\Expression|null
         */
        public function filterExp(): ?\Aerospike\Expression {}

        /**
         * Retries after the first attempt.
         *
         * @return int|null
         */
        public function maxRetries(): ?int {}

        /**
         * AP-namespace read consistency.
         *
         * @return \Aerospike\ReadModeAP|null
         */
        public function readModeAp(): ?\Aerospike\ReadModeAP {}

        /**
         * SC-namespace read consistency.
         *
         * @return \Aerospike\ReadModeSC|null
         */
        public function readModeSc(): ?\Aerospike\ReadModeSC {}

        /**
         * Which node to prefer.
         *
         * @return \Aerospike\Replica|null
         */
        public function replica(): ?\Aerospike\Replica {}

        /**
         * Pause between retries.
         *
         * @return int|null
         */
        public function sleepBetweenRetriesMs(): ?int {}

        /**
         * Deadline for one socket operation.
         *
         * @return int|null
         */
        public function socketTimeoutMs(): ?int {}

        /**
         * Deadline for the whole command, retries included.
         *
         * @return int|null
         */
        public function totalTimeoutMs(): ?int {}

        /**
         * The id of the transaction this policy joins, or `null`.
         *
         * The id rather than the `Transaction` object: a policy is meant to be
         * reusable, and holding the object would keep an unfinished transaction alive
         * past the point its destructor should have aborted it.
         *
         * @return int|null
         */
        public function txnId(): ?int {}

        /**
         * Whether to compress a request worth compressing.
         *
         * @return bool|null
         */
        public function useCompression(): ?bool {}
    }

    /**
     * One record as the server returned it.
     *
     * Produced by a read; there is deliberately no constructor, because a
     * `Record` that no server produced would report a generation and a TTL that
     * mean nothing.
     *
     * ```php
     * $record = $client->get(null, $key);
     * if ($record !== null) {
     *     $record->bins();          // ["name" => "Alice", "age" => 30]
     *     $record->bin("age");      // 30, or null if the bin is not there
     *     $record->generation();    // 1
     *     $record->ttl();           // 2592000, or null when it never expires
     * }
     * ```
     */
    class Record {
        public function __construct() {}

        /**
         * Property access, for the 1.x client's `$record->bins` style.
         *
         * A magic getter rather than registered properties: a property registered
         * from Rust is declared to PHP as untyped, so `$record->bins = [...]` would
         * be accepted and silently do nothing. `__get` is read-only by construction,
         * and an unknown name is an error naming what is available rather than
         * `null` — which is what PHP would otherwise hand back.
         *
         * @param string $name
         * @return mixed
         */
        public function __get(string $name): mixed {}

        /**
         * One bin's value, or `null` if the record does not have it.
         *
         * A bin holding `null` and an absent bin are indistinguishable through
         * this, because Aerospike does not store a nil bin — writing nil deletes
         * it. Use [`Record::has`] when the difference matters to the caller.
         *
         * @param string $name
         * @return mixed
         */
        public function bin(string $name): mixed {}

        /**
         * The bin names, in the order the server returned them.
         *
         * @return array
         */
        public function binNames(): array {}

        /**
         * Every bin, as a name-to-value map.
         *
         * In the server's order, which for a record read as a whole is its own
         * bin order and not the order they were written in.
         *
         * @return array
         */
        public function bins(): array {}

        /**
         * How many bins the record has.
         *
         * @return int
         */
        public function count(): int {}

        /**
         * The server's 20-byte digest, for a record a scan or query returned.
         *
         * An `Aerospike\Blob`, because it is bytes rather than text —
         * `bin2hex((string) $record->digest())` is the readable form. `null` for a
         * single-record read, whose digest was never sent.
         *
         * This is the identity every scanned record has, whether or not its key was
         * stored, which makes it what a scan can rely on.
         *
         * @return \Aerospike\Blob|null
         */
        public function digest(): ?\Aerospike\Blob {}

        /**
         * When the record expires, as an `Aerospike\Expiration`.
         *
         * The same fact [`Record::ttl`] reports, in the shape a `WritePolicy` takes —
         * so a read-modify-write can carry the record's own lifetime forward without
         * converting anything. `null` for a record that never expires, matching
         * `ttl()`.
         *
         * @return \Aerospike\Expiration|null
         */
        public function expiration(): ?\Aerospike\Expiration {}

        /**
         * The record's generation, which every write increments.
         *
         * @return int
         */
        public function generation(): int {}

        /**
         * Alias of [`Record::bins`], as the 1.x client spelled it.
         *
         * @return array
         */
        public function getBins(): array {}

        /**
         * Alias of [`Record::expiration`].
         *
         * @return \Aerospike\Expiration|null
         */
        public function getExpiration(): ?\Aerospike\Expiration {}

        /**
         * Alias of [`Record::generation`].
         *
         * @return int
         */
        public function getGeneration(): int {}

        /**
         * Alias of [`Record::key`].
         *
         * @return \Aerospike\Key|null
         */
        public function getKey(): ?\Aerospike\Key {}

        /**
         * Alias of [`Record::ttl`].
         *
         * @return int|null
         */
        public function getTtl(): ?int {}

        /**
         * Whether the record has this bin.
         *
         * @param string $name
         * @return bool
         */
        public function has(string $name): bool {}

        /**
         * Which record this is, for one a scan or query returned — or `null`.
         *
         * `null` in two quite different situations, which is why [`Record::digest`]
         * exists beside it:
         *
         * - a single-record read, where the caller already has the key it asked
         *   with and the server does not send it back;
         * - a scanned record whose write did not store its key, because
         *   `sendKey` was not set. The record has a digest and no key, and a digest
         *   cannot be turned back into one.
         *
         * So a scan that needs keys has to have been written with `sendKey: true`.
         * That is the server's rule, not this client's.
         *
         * @return \Aerospike\Key|null
         */
        public function key(): ?\Aerospike\Key {}

        /**
         * Seconds until the record expires, or `null` when it never does.
         *
         * `null` rather than a magic number: "never" is not a quantity of
         * seconds, and a sentinel would invite arithmetic on it.
         *
         * @return int|null
         */
        public function ttl(): ?int {}
    }

    /**
     * What a write does about the record already existing, or not.
     *
     * `put`, `add`, `append` and `prepend` all default to `Update`. Setting this
     * is how a write becomes conditional: `CreateOnly` fails with result code 5
     * when the record is there, `UpdateOnly` with result code 2 when it is not.
     */
    enum RecordExistsAction: string {
    /**
     * Create or update; merge bins.
     */
      case Update = 'UPDATE';
    /**
     * Update only; fail if absent.
     */
      case UpdateOnly = 'UPDATE_ONLY';
    /**
     * Create or replace; drop the bins not named.
     */
      case Replace = 'REPLACE';
    /**
     * Replace only; fail if absent.
     */
      case ReplaceOnly = 'REPLACE_ONLY';
    /**
     * Create only; fail if present.
     */
      case CreateOnly = 'CREATE_ONLY';
        /** The 1.x spelling of a case of this enum. */
        public static function Update(): \Aerospike\RecordExistsAction {}

        /** The 1.x spelling of a case of this enum. */
        public static function UpdateOnly(): \Aerospike\RecordExistsAction {}

        /** The 1.x spelling of a case of this enum. */
        public static function Replace(): \Aerospike\RecordExistsAction {}

        /** The 1.x spelling of a case of this enum. */
        public static function ReplaceOnly(): \Aerospike\RecordExistsAction {}

        /** The 1.x spelling of a case of this enum. */
        public static function CreateOnly(): \Aerospike\RecordExistsAction {}

    }

    /**
     * The records a scan or query returns, read one page at a time.
     *
     * An `Iterator`, so `foreach` reads the whole traversal:
     *
     * ```php
     * foreach ($client->query(null, null, $statement) as $record) {
     *     echo $record->key()?->userKey(), " ", $record->bin("name"), "\n";
     * }
     * ```
     *
     * # Read once, and close what you abandon
     *
     * This is a *position* in a traversal, not a collection of records. Reading it
     * twice throws: the second `foreach` cannot start from the beginning — the
     * records are on the server, and the first loop consumed them — and silently
     * continuing from the middle would be worse than saying so.
     *
     * Abandoning one is fine. `break` out of a `foreach`, or let the variable go out
     * of scope, and the daemon's cursor is released — the object closes itself when
     * it is destroyed. `close()` does it explicitly, for a traversal held in a
     * long-lived variable.
     *
     * # A page boundary is invisible except in timing
     *
     * Every `pageSize` records, `next()` makes a round trip. Nothing about the
     * iteration changes; the only visible effect is that one step in every page
     * takes as long as a database call. A larger `pageSize` trades memory — the page
     * is held in the daemon and then in this process — for fewer round trips.
     */
    class RecordSet implements \Iterator {
        public function __construct() {}

        /**
         * Release the daemon's cursor, ending the traversal here.
         *
         * Idempotent, and safe on a traversal that already finished — a finished one
         * has no cursor to release. Whatever is left of the current page is
         * discarded, so `valid()` is `false` afterwards.
         *
         * @return void
         */
        public function close(): void {}

        /**
         * The record at the current position, or `null` past the end.
         *
         * @return \Aerospike\Record|null
         */
        public function current(): ?\Aerospike\Record {}

        /**
         * Whether the traversal is still producing records.
         *
         * The 1.x spelling of [`RecordSet::valid`].
         *
         * @return bool
         */
        public function getActive(): bool {}

        /**
         * Whether the traversal still has a cursor open on the daemon.
         *
         * `false` for one that finished on its own, as much as for one that was
         * closed: neither is holding anything.
         *
         * @return bool
         */
        public function isOpen(): bool {}

        /**
         * The current position, counted across the whole traversal rather than
         * within the page — so it keeps rising past a page boundary, as a caller
         * would expect and as the pages themselves do not.
         *
         * @return int
         */
        public function key(): int {}

        /**
         * Advance, fetching the next page when the current one runs out.
         *
         * @return void
         */
        public function next(): void {}

        /**
         * The next record, or `null` at the end — a pull rather than an advance.
         *
         * ```php
         * while (($record = $recordSet->nextRecord()) !== null) { … }
         * ```
         *
         * This is what the 1.x client's `Recordset::next()` did, and it is spelled
         * differently here because [`RecordSet::next`] is `Iterator::next`, which PHP
         * declares as returning nothing. Old code that reads `next()`'s return value
         * gets `null` and iterates zero times, so it has to change one way or
         * another; this is the smaller of the two changes, `foreach` being the other.
         *
         * Do not mix this with `foreach` on one traversal: each advances the same
         * position, so together they skip records.
         *
         * @return \Aerospike\Record|null
         */
        public function nextRecord(): ?\Aerospike\Record {}

        /**
         * Start iterating.
         *
         * A no-op the first time, because the first page was already fetched when
         * the traversal opened. A *second* time it throws: see the class docs — a
         * position cannot be rewound to a beginning that no longer exists.
         *
         * @return void
         */
        public function rewind(): void {}

        /**
         * How many records this traversal has produced so far.
         *
         * Not `Countable`: how many a scan *will* produce is not knowable without
         * running it, and a `count()` that answered "so far" while looking like a
         * total would be the most misleading thing this class could offer.
         *
         * @return int
         */
        public function seen(): int {}

        /**
         * Whether there is a record at the current position.
         *
         * @return bool
         */
        public function valid(): bool {}
    }

    /**
     * Flags for [`Exp::regex_compare`], combined with `|`.
     *
     * Constants rather than an enum, because they are a **bitmask**: PHP enum cases
     * cannot be OR-ed together, and the parameter takes one integer.
     *
     * ```php
     * use Aerospike\{Exp, RegexFlag};
     *
     * // Case-insensitive, POSIX extended syntax.
     * Exp::regexCompare('^a(b|c)z$', RegexFlag::ICASE | RegexFlag::EXTENDED, Exp::stringBin('name'));
     * ```
     */
    class RegexFlag {
        /**
         * POSIX Extended Regular Expression syntax.
         */
        const EXTENDED = 1;

        /**
         * Ignore case.
         */
        const ICASE = 2;

        /**
         * Match-any-character operators do not match a newline.
         */
        const NEWLINE = 8;

        /**
         * The regex engine's defaults.
         */
        const NONE = 0;

        /**
         * Do not report the position of matches. Faster when only the yes/no answer
         * is wanted, which for a filter is always.
         */
        const NOSUB = 4;

        public function __construct() {}
    }

    /**
     * Which node a command prefers.
     *
     * Inert on a write, which always goes to the partition master. Accepting it
     * there anyway is deliberate: one policy object is often reused across verbs.
     */
    enum Replica: string {
    /**
     * The partition master.
     */
      case Master = 'MASTER';
    /**
     * Master and proles, chosen at random.
     */
      case MasterProles = 'MASTER_PROLES';
    /**
     * Any node at random.
     */
      case Random = 'RANDOM';
    /**
     * Master first, then proles in sequence on failure.
     */
      case Sequence = 'SEQUENCE';
    /**
     * A node on the client's own rack first, where rack awareness is
     * configured.
     */
      case PreferRack = 'PREFER_RACK';
    }

    /**
     * One role the cluster knows.
     *
     * Produced by `queryRoles()`; `createRole()` makes one.
     */
    class Role {
        public function __construct() {}

        /**
         * `name: privilege, privilege`, for logs.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * Addresses a holder may connect from. Empty means anywhere.
         *
         * @return array
         */
        public function allowlist(): array {}

        /**
         * The role name.
         *
         * @return string
         */
        public function name(): string {}

        /**
         * What the role permits.
         *
         * @return array
         */
        public function privileges(): array {}

        /**
         * Reads per second the role is limited to, or `null` when unlimited.
         *
         * `null` rather than `0`: zero is the server's way of saying "no limit", and a
         * caller doing arithmetic on it would get a limit of nothing.
         *
         * @return int|null
         */
        public function readQuota(): ?int {}

        /**
         * Writes per second the role is limited to, or `null` when unlimited.
         *
         * @return int|null
         */
        public function writeQuota(): ?int {}
    }

    /**
     * Flags for [`ExpPath::select_by_path`], combined with `|`.
     *
     * Constants rather than an enum because `NO_FAIL` combines with the others —
     * PHP enum cases cannot be OR-ed. The four selections have named methods on
     * `ExpPath`, so these are for `NO_FAIL` and for the flag an application computes.
     */
    class SelectFlag {
        /**
         * Synonym for `VALUE`, when the nodes are list elements.
         */
        const LIST_VALUE = 1;

        /**
         * The map key of each selected node.
         */
        const MAP_KEY = 2;

        /**
         * The key and value of each selected node.
         */
        const MAP_KEY_VALUE = 3;

        /**
         * Synonym for `VALUE`, when the nodes are map values.
         */
        const MAP_VALUE = 1;

        /**
         * The tree from the root down, pruned to what matched.
         */
        const MATCHING_TREE = 0;

        /**
         * Skip nodes of the wrong type instead of failing.
         */
        const NO_FAIL = 16;

        /**
         * The value of each selected node.
         */
        const VALUE = 1;

        public function __construct() {}
    }

    /**
     * A map the server stores sorted by key.
     *
     * Unlike [`OrderedMap`], this order is real storage: the server keeps the map
     * key-ordered, which is what lets its rank, index and key-range operations run
     * in log time instead of scanning. A key-ordered map read back from the server
     * therefore arrives **as a `SortedMap`**, in the server's order.
     *
     * The entries are not re-sorted locally. The server is the authority on how
     * values order — its rule spans every type, `nil < bool < int < string < list <
     * map < blob < float < GeoJSON` — and a second implementation of that here
     * could only ever disagree with it. So a `SortedMap` you build iterates in the
     * order you built it, and one the server sent iterates in the server's order,
     * which is the one that matters.
     */
    class SortedMap implements \ArrayAccess, \Iterator, \Countable {
        /**
         * Build one, optionally from a PHP array.
         *
         * @param array|null $entries
         */
        public function __construct(?array $entries = null) {}

        /**
         * How many entries the map holds.
         *
         * @return int
         */
        public function count(): int {}

        /**
         * The value at the cursor.
         *
         * @return mixed
         */
        public function current(): mixed {}

        /**
         * One entry's value, or `null` if the key is not there.
         *
         * @param mixed $key
         * @return mixed
         */
        public function get(mixed $key): mixed {}

        /**
         * Whether the map has this key.
         *
         * @param mixed $key
         * @return bool
         */
        public function has(mixed $key): bool {}

        /**
         * Whether the map is empty.
         *
         * @return bool
         */
        public function isEmpty(): bool {}

        /**
         * The key at the cursor. Not restricted to what a PHP array key can hold.
         *
         * @return mixed
         */
        public function key(): mixed {}

        /**
         * The keys, in order.
         *
         * @return array
         */
        public function keys(): array {}

        /**
         * Advance the cursor.
         *
         * @return void
         */
        public function next(): void {}

        /**
         * `isset($map[$key])`.
         *
         * @param mixed $offset
         * @return bool
         */
        public function offsetExists(mixed $offset): bool {}

        /**
         * `$map[$key]`.
         *
         * @param mixed $offset
         * @return mixed
         */
        public function offsetGet(mixed $offset): mixed {}

        /**
         * `$map[$key] = $value`.
         *
         * @param mixed $offset
         * @param mixed $value
         * @return void
         */
        public function offsetSet(mixed $offset, mixed $value): void {}

        /**
         * `unset($map[$key])`.
         *
         * @param mixed $offset
         * @return void
         */
        public function offsetUnset(mixed $offset): void {}

        /**
         * Remove one entry, reporting whether it was there.
         *
         * @param mixed $key
         * @return bool
         */
        public function remove(mixed $key): bool {}

        /**
         * Start an iteration.
         *
         * @return void
         */
        public function rewind(): void {}

        /**
         * Set one entry, replacing an existing key in place.
         *
         * @param mixed $key
         * @param mixed $value
         * @return void
         */
        public function set(mixed $key, mixed $value): void {}

        /**
         * The same entries as a PHP array.
         *
         * Throws for a key a PHP array cannot hold — a blob or a float — since
         * carrying those is one of the two reasons this class exists.
         *
         * @return array
         */
        public function toArray(): array {}

        /**
         * Whether the iteration has an entry to give.
         *
         * @return bool
         */
        public function valid(): bool {}

        /**
         * The values, in order.
         *
         * @return array
         */
        public function values(): array {}
    }

    /**
     * Which records a scan or query visits: a namespace, a set, which bins, and
     * optionally a filter.
     *
     * ```php
     * new Aerospike\Statement("test", "users");                       // a scan
     * new Aerospike\Statement("test", "users", Aerospike\Bins::some(["name"]));
     * new Aerospike\Statement("test", "users", filter: Aerospike\Filter::equal("age", 30));
     * new Aerospike\Statement("test");                                // every set
     * ```
     *
     * Immutable, like the policies and for the same reason: a statement is worth
     * building once and reusing, including across requests, and one that could be
     * mutated afterwards would be one whose meaning depends on when it was read.
     */
    class Statement {
        /**
         * Name what to visit.
         *
         * An empty or omitted `$set` means **every set in the namespace**, which is
         * not the same as the null set — a single-record `Key` with an empty set
         * names the null set, and this names all of them. That asymmetry is the
         * server's; naming it here is better than the surprise.
         *
         * @param string $namespace
         * @param string|null $set
         * @param \Aerospike\Bins|null $bins
         * @param \Aerospike\Filter|null $filter
         */
        public function __construct(string $namespace, ?string $set = null, ?\Aerospike\Bins $bins = null, ?\Aerospike\Filter $filter = null) {}

        /**
         * Whether this is a scan — a statement with no filter.
         *
         * @return bool
         */
        public function isScan(): bool {}

        /**
         * The namespace.
         *
         * @return string
         */
        public function namespace(): string {}

        /**
         * The set name; `""` for every set in the namespace.
         *
         * @return string
         */
        public function setName(): string {}
    }

    /**
     * How a failure is classified, from `AerospikeException::getStatus()`.
     *
     * This is what an application branches on. It answers "whose problem is
     * this" — the server's, the network's, the daemon's, or this call's arguments
     * — where the server result code answers "what exactly did the server say".
     *
     * ```php
     * try {
     *     $client->put($policy, $key, $bins);
     * } catch (Aerospike\AerospikeException $e) {
     *     match ($e->getStatus()) {
     *         Aerospike\Status::Timeout    => $retryLater($e->isInDoubt()),
     *         Aerospike\Status::Connection => $failOver(),
     *         default                      => throw $e,
     *     };
     * }
     * ```
     */
    enum Status: string {
    /**
     * The server returned a non-OK result code; see `getResultCode()`.
     */
      case Server = 'SERVER';
    /**
     * The record does not exist, where that is a failure rather than an
     * answer.
     */
      case RecordNotFound = 'RECORD_NOT_FOUND';
    /**
     * The deadline passed. Check `isInDoubt()` before retrying a write.
     */
      case Timeout = 'TIMEOUT';
    /**
     * The daemon could not reach the cluster.
     */
      case Connection = 'CONNECTION';
    /**
     * The daemon has no configuration for the instance this client names.
     */
      case UnknownInstance = 'UNKNOWN_INSTANCE';
    /**
     * The request was malformed or asked for something unsupported.
     */
      case InvalidRequest = 'INVALID_REQUEST';
    /**
     * A frame exceeded the protocol's maximum payload.
     */
      case FrameTooLarge = 'FRAME_TOO_LARGE';
    /**
     * The daemon failed internally.
     */
      case Internal = 'INTERNAL';
    /**
     * A scan or query cursor is no longer open, so the traversal has to start
     * again.
     *
     * The one status worth catching separately: a `RecordSet` that reports this
     * mid-iteration has *not* finished, and treating it as the end would drop
     * every record the scan had not reached.
     */
      case CursorExpired = 'CURSOR_EXPIRED';
    /**
     * A multi-record transaction is no longer open, so the work has to start
     * again.
     *
     * Worth catching separately: it means the transaction's writes are *not*
     * applied, and that it was committed, aborted, or aborted for you after
     * sitting idle. A command that failed *inside* a still-open transaction is an
     * ordinary failure, and that transaction can still be aborted.
     */
      case TxnExpired = 'TXN_EXPIRED';
    /**
     * The failure never reached the daemon: it could not be attached to, a
     * value could not be expressed, or the frame did not validate.
     */
      case Client = 'CLIENT';
    /**
     * A status this build of the extension does not know.
     *
     * Unreachable while the daemon and the extension are the same version,
     * which they must be — but a status read out of shared memory is data,
     * and turning unexpected data into a panic would be worse than naming it.
     */
      case Unrecognized = 'UNRECOGNIZED';
    }

    /**
     * Which numbers `ExpStr::isNumericTyped()` accepts.
     */
    enum StringNumericType: string {
    /**
     * An integer or a floating-point number.
     */
      case Any = 'ANY';
    /**
     * Only integers.
     */
      case Integer = 'INT';
    /**
     * Only floating-point numbers.
     */
      case Double = 'FLOAT';
    }

    /**
     * Flags for the string expressions that take a regular expression, combined
     * with `|`.
     *
     * Constants rather than an enum, for the reason `Aerospike\RegexFlag` gives: they
     * are a bitmask. Distinct from `RegexFlag` because these are **ICU** flags, which
     * the string operations use, where `Exp::regexCompare` uses the server's POSIX
     * ones — same idea, different engine, different values.
     */
    class StringRegexFlag {
        /**
         * Case-insensitive matching.
         */
        const CASE_INSENSITIVE = 1;

        /**
         * `.` matches line terminators too.
         */
        const DOT_ALL = 4;

        /**
         * Replace every match. Only meaningful for `regexReplace`.
         */
        const GLOBAL = 16;

        /**
         * `^` and `$` match the start and end of any line.
         */
        const MULTILINE = 2;

        /**
         * ICU defaults.
         */
        const NONE = 0;

        /**
         * Only `\n` is a line terminator.
         */
        const UNIX_LINES = 8;

        public function __construct() {}
    }

    /**
     * A long-running server command, and how far it has got.
     *
     * Returned by `registerUdf`, `removeUdf`, `createIndexOnBin`,
     * `createIndexUsingExpression`, `dropIndex` and `queryExecuteUdf` — each of which
     * returns as soon as the server has *accepted* the work, not when it is done.
     *
     * ```php
     * $task = $client->createIndexOnBin(
     *     null, 'test', 'users', 'age', 'age_idx', Aerospike\IndexType::Numeric
     * );
     * $task->waitTillComplete(30_000);     // up to 30 seconds
     * // …the index is now built on every node, and a query may use it.
     * ```
     *
     * Or without blocking, for a command whose progress a request wants to report:
     *
     * ```php
     * match ($task->status()) {
     *     Aerospike\TaskStatus::Complete   => 'done',
     *     Aerospike\TaskStatus::InProgress => 'building',
     *     Aerospike\TaskStatus::NotFound   => 'gone',
     * };
     * ```
     *
     * There is no constructor: a task describes work some command started, and one
     * built by hand would describe work nobody asked for.
     *
     * # What the three statuses can actually mean
     *
     * The server answers about work it is *doing*, which makes two of the answers
     * less informative than they look — worth knowing before branching on them:
     *
     * - **A background job that finished and one that never existed are
     *   indistinguishable**, and both report `Complete`. The server tracks running
     *   jobs, so "no node is running it" is all it can say. Waiting on a task from a
     *   command that succeeded is therefore safe; a handle for work that was never
     *   started would report `Complete` at once.
     * - **An index that does not exist throws** rather than reporting `NotFound`:
     *   the server answers with result code 201, `no index`, which is a more useful
     *   thing to be told than "not found yet".
     *
     * So `NotFound` is rare in practice. Treat `Complete` as "not running", which is
     * what it means, and rely on the command that produced the task having succeeded.
     */
    class Task {
        public function __construct() {}

        /**
         * What this task is, for a log or a test failure.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * The same, as a method.
         *
         * @return string
         */
        public function describe(): string {}

        /**
         * Whether the command has finished on every node.
         *
         * A convenience for the common test. It is `status() === Complete`, so it
         * costs a round trip too — do not call it in a tight loop; use
         * `Task::waitTillComplete()`, which paces itself.
         *
         * @return bool
         */
        public function isComplete(): bool {}

        /**
         * Ask the server how far the command has got. One round trip; never blocks.
         *
         * @return \Aerospike\TaskStatus
         */
        public function status(): \Aerospike\TaskStatus {}

        /**
         * Block until the command finishes, or until `$timeoutMs` elapses.
         *
         * Polls every `$pollMs` (a second by default). `$timeoutMs` of `null` waits
         * as long as it takes, which is only reasonable in a script — a web request
         * should pass a bound, or poll `status()` across requests instead.
         *
         * **Sleeps before the first check.** A command the server has only just
         * accepted can report `NotFound` for a moment, and treating that first answer
         * as final would fail a command that was about to start. That is what
         * `aerospike-core`'s own wait does, for the same reason.
         *
         * Throws when the timeout passes, and when the server reports `NotFound`
         * after the command should have been running — which means the work is not
         * there to wait for.
         *
         * @param int|null $timeoutMs
         * @param int|null $pollMs
         * @return \Aerospike\TaskStatus
         */
        public function waitTillComplete(?int $timeoutMs = null, ?int $pollMs = null): \Aerospike\TaskStatus {}
    }

    /**
     * How far a long-running server command has got.
     *
     * ```php
     * $task = $client->createIndexOnBin(null, 'test', 'users', 'age', 'age_idx', IndexType::Numeric);
     * while ($task->status() === Aerospike\TaskStatus::InProgress) {
     *     usleep(500_000);
     * }
     * ```
     *
     * `NotFound` right after issuing a command can mean "not started yet" as much as
     * "never existed" — which is why `Task::waitTillComplete()` sleeps before its
     * first check rather than treating the first answer as final.
     */
    enum TaskStatus: string {
    /**
     * The server has no record of the command.
     */
      case NotFound = 'NOT_FOUND';
    /**
     * Still running.
     */
      case InProgress = 'IN_PROGRESS';
    /**
     * Finished on every node.
     */
      case Complete = 'COMPLETE';
    }

    /**
     * A multi-record transaction: several commands against several records, all of
     * which land or none of which do.
     *
     * Opened with `Client::beginTransaction()`, joined by passing it on a policy, and
     * finished with `commit()` or `abort()`:
     *
     * ```php
     * $txn = $client->beginTransaction();
     * $client->put(new Aerospike\WritePolicy(txn: $txn), $key, [new Aerospike\Bin('n', 1)]);
     * $record = $client->get(new Aerospike\ReadPolicy(txn: $txn), $key);
     * $txn->commit();
     * ```
     *
     * **A transaction reads as well as writes**, and its reads matter: the commit
     * verifies that every record it read is still at the version it saw, and fails the
     * whole transaction if not. That is what makes it a transaction rather than a
     * batch.
     *
     * **Not for scans or queries.** A transaction covers records named by key, and a
     * traversal names none — so a `QueryPolicy` has nowhere to put one, and a
     * transaction on a read policy handed to `query()` is refused rather than ignored.
     *
     * There is no constructor: a transaction has to be opened on a cluster, and one
     * built by hand would name no cluster and no server-side state.
     */
    class Transaction {
        public function __construct() {}

        /**
         * `transaction <id>`, for logs and test failures.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * Abort the transaction: discard all of its writes.
         *
         * The records it wrote go back to what they were, and its locks are released.
         * Aborting twice is a local no-op returning `AlreadyAborted`. Aborting one that
         * was *committed* throws: its writes are permanent, and nothing here can undo
         * them.
         *
         * @return \Aerospike\AbortStatus
         */
        public function abort(): \Aerospike\AbortStatus {}

        /**
         * Abort the transaction, with an explicit policy for the roll-back.
         *
         * **One policy, not two.** An abort has nothing to verify — it is discarding
         * the writes, so the versions the transaction read no longer matter — so
         * where `commitWithPolicies` takes a verify policy and a roll policy, this
         * takes only the roll.
         *
         * ```php
         * $txn->abortWithPolicy(new Aerospike\TxnRollPolicy(maxRetries: 10));
         * ```
         *
         * Behaves exactly as `abort()` otherwise.
         *
         * @param \Aerospike\TxnRollPolicy|null $roll
         * @return \Aerospike\AbortStatus
         */
        public function abortWithPolicy(?\Aerospike\TxnRollPolicy $roll = null): \Aerospike\AbortStatus {}

        /**
         * Commit the transaction: make all of its writes permanent.
         *
         * Verifies first — every record the transaction read must still be at the
         * version it saw — and fails the whole transaction if any has changed. That
         * failure leaves the transaction *aborted*, so there is nothing to retry
         * except the work.
         *
         * Every `CommitStatus` this returns is a success. `Ok` means everything was
         * tidied up; the others mean the writes landed and the server was left to
         * finish some bookkeeping. **None of them is a reason to commit again.**
         *
         * Committing twice is a local no-op returning `AlreadyCommitted`, so a
         * `finally` block can commit without checking. Committing one that was
         * *aborted* throws — the writes are gone, and answering "already committed"
         * would be the most dangerous possible lie.
         *
         * @return \Aerospike\CommitStatus
         */
        public function commit(): \Aerospike\CommitStatus {}

        /**
         * Commit the transaction, with explicit policies for its two phases.
         *
         * A commit is two batch commands — **verify** every record the transaction
         * read, then **roll** its writes forward — and the client has a separate
         * policy for each, because they are tuned differently: the verify batch reads
         * linearizably, and both go to the partition master. `null` for either takes
         * the client's own default, which is what `commit()` passes for both.
         *
         * ```php
         * // A cluster under load: give both phases longer before they give up.
         * $txn->commitWithPolicies(
         *     new Aerospike\TxnVerifyPolicy(totalTimeoutMs: 30_000),
         *     new Aerospike\TxnRollPolicy(totalTimeoutMs: 30_000),
         * );
         * ```
         *
         * Behaves exactly as `commit()` otherwise, including the two local answers:
         * committing twice returns `AlreadyCommitted`, and committing an aborted
         * transaction throws.
         *
         * @param \Aerospike\TxnVerifyPolicy|null $verify
         * @param \Aerospike\TxnRollPolicy|null $roll
         * @return \Aerospike\CommitStatus
         */
        public function commitWithPolicies(?\Aerospike\TxnVerifyPolicy $verify = null, ?\Aerospike\TxnRollPolicy $roll = null): \Aerospike\CommitStatus {}

        /**
         * The transaction's id.
         *
         * The server's own — the same number appears in a server log and in the
         * daemon's own logging, which is what makes it worth exposing.
         *
         * @return int
         */
        public function id(): int {}

        /**
         * Whether the transaction can still take commands.
         *
         * Local knowledge, so it costs nothing: `false` once `commit()` or `abort()`
         * has returned. It does **not** mean the daemon has not expired it underneath
         * — that shows up as a failure on the next command, which is the only place it
         * could.
         *
         * @return bool
         */
        public function isOpen(): bool {}

        /**
         * Where the transaction has got to.
         *
         * One round trip; the daemon holds the state, so this is not a question for
         * the cluster. Returns `TxnState::Committed` or `Aborted` from local knowledge
         * once the transaction has finished, since the daemon has forgotten it by
         * then.
         *
         * @return \Aerospike\TxnState
         */
        public function state(): \Aerospike\TxnState {}
    }

    /**
     * Per-call settings for the **roll** half of a commit or an abort.
     *
     * Mirrors `aerospike-core`'s `TxnRollPolicy`: the batch that moves a
     * transaction's writes forward when it commits, or rolls them back when it
     * aborts. Pass `null` to accept the tuned defaults.
     *
     * ```php
     * $roll = new Aerospike\TxnRollPolicy(maxRetries: 10);
     * $txn->commitWithPolicies(null, $roll);   // or $txn->abortWithPolicy($roll)
     * ```
     *
     * # This is the one policy an abort takes, and the reason is the asymmetry
     *
     * An abort has nothing to verify — it is discarding the writes, so the versions
     * the transaction read no longer matter — so `abortWithPolicy` takes one policy
     * where `commitWithPolicies` takes two.
     *
     * # It writes, and yet it has no write fields
     *
     * Rolling a transaction does change records, so the absence of `expiration`,
     * `recordExistsAction` and the rest looks odd. It is not: this batch does not
     * write records the caller described, it moves writes the transaction already
     * made. A TTL or a create/replace guard would have nothing to apply to, and the
     * daemon refuses one rather than ignoring it.
     */
    class TxnRollPolicy {
        /**
         * Build a roll policy. Every argument is optional; an omitted one leaves
         * the client's tuned default alone.
         *
         * camelCase parameters and `= null` defaults, for the reasons
         * [`ReadPolicy::__construct`] gives.
         *
         * @param int|null $totalTimeoutMs
         * @param int|null $socketTimeoutMs
         * @param int|null $maxRetries
         * @param int|null $sleepBetweenRetriesMs
         * @param \Aerospike\Replica|null $replica
         * @param \Aerospike\ReadModeAP|null $readModeAp
         * @param \Aerospike\ReadModeSC|null $readModeSc
         * @param bool|null $useCompression
         */
        public function __construct(?int $totalTimeoutMs = null, ?int $socketTimeoutMs = null, ?int $maxRetries = null, ?int $sleepBetweenRetriesMs = null, ?\Aerospike\Replica $replica = null, ?\Aerospike\ReadModeAP $readModeAp = null, ?\Aerospike\ReadModeSC $readModeSc = null, ?bool $useCompression = null) {}

        /**
         * Retries after the first attempt.
         *
         * @return int|null
         */
        public function maxRetries(): ?int {}

        /**
         * AP-namespace read consistency.
         *
         * @return \Aerospike\ReadModeAP|null
         */
        public function readModeAp(): ?\Aerospike\ReadModeAP {}

        /**
         * SC-namespace read consistency.
         *
         * @return \Aerospike\ReadModeSC|null
         */
        public function readModeSc(): ?\Aerospike\ReadModeSC {}

        /**
         * Which node to prefer.
         *
         * @return \Aerospike\Replica|null
         */
        public function replica(): ?\Aerospike\Replica {}

        /**
         * Pause between retries.
         *
         * @return int|null
         */
        public function sleepBetweenRetriesMs(): ?int {}

        /**
         * Deadline for one socket operation.
         *
         * @return int|null
         */
        public function socketTimeoutMs(): ?int {}

        /**
         * Deadline for the whole roll batch, retries included.
         *
         * @return int|null
         */
        public function totalTimeoutMs(): ?int {}

        /**
         * Whether to compress a request worth compressing.
         *
         * @return bool|null
         */
        public function useCompression(): ?bool {}
    }

    /**
     * Where a multi-record transaction has got to.
     */
    enum TxnState: string {
    /**
     * Accepting commands.
     */
      case Open = 'OPEN';
    /**
     * Every read has been verified; the commit is part-done.
     */
      case Verified = 'VERIFIED';
    /**
     * Committed. Its writes are permanent.
     */
      case Committed = 'COMMITTED';
    /**
     * Aborted. Its writes are gone.
     */
      case Aborted = 'ABORTED';
    }

    /**
     * Per-call settings for the **verify** half of a commit.
     *
     * Mirrors `aerospike-core`'s `TxnVerifyPolicy`. Committing a transaction is two
     * batch commands — check the version of every record the transaction read, then
     * move its writes forward — and this is the policy for the first. Pass `null` to
     * `commitWithPolicies` to accept the tuned defaults, which is what nearly every
     * commit should do.
     *
     * ```php
     * // A cluster under load: give the verify batch longer before it gives up.
     * $txn->commitWithPolicies(
     *     new Aerospike\TxnVerifyPolicy(totalTimeoutMs: 30_000, maxRetries: 8),
     *     null,
     * );
     * ```
     *
     * # The defaults are not the daemon's, and should usually be left alone
     *
     * Unlike a [`ReadPolicy`], this does not start from the daemon's configured
     * timeout. The Rust client's own defaults apply — linearized SC reads, the
     * partition master, 5 retries, a 3s socket and 10s total timeout, a 1s pause
     * between retries — and they are deliberate: a verify batch that gives up leaves
     * a transaction half-finished, holding record locks, so it is tuned to keep
     * trying rather than to be quick.
     *
     * # Fewer fields than a read policy, for two reasons
     *
     * There is no `filter`, because this batch reads records the transaction chose
     * and a filter that skipped one would skip verifying it. And there is no `txn`:
     * the transaction being committed is the one whose method this is.
     */
    class TxnVerifyPolicy {
        /**
         * Build a verify policy. Every argument is optional; an omitted one leaves
         * the client's tuned default alone.
         *
         * camelCase parameters and `= null` defaults, for the reasons
         * [`ReadPolicy::__construct`] gives.
         *
         * @param int|null $totalTimeoutMs
         * @param int|null $socketTimeoutMs
         * @param int|null $maxRetries
         * @param int|null $sleepBetweenRetriesMs
         * @param \Aerospike\Replica|null $replica
         * @param \Aerospike\ReadModeAP|null $readModeAp
         * @param \Aerospike\ReadModeSC|null $readModeSc
         * @param bool|null $useCompression
         */
        public function __construct(?int $totalTimeoutMs = null, ?int $socketTimeoutMs = null, ?int $maxRetries = null, ?int $sleepBetweenRetriesMs = null, ?\Aerospike\Replica $replica = null, ?\Aerospike\ReadModeAP $readModeAp = null, ?\Aerospike\ReadModeSC $readModeSc = null, ?bool $useCompression = null) {}

        /**
         * Retries after the first attempt.
         *
         * @return int|null
         */
        public function maxRetries(): ?int {}

        /**
         * AP-namespace read consistency.
         *
         * @return \Aerospike\ReadModeAP|null
         */
        public function readModeAp(): ?\Aerospike\ReadModeAP {}

        /**
         * SC-namespace read consistency.
         *
         * @return \Aerospike\ReadModeSC|null
         */
        public function readModeSc(): ?\Aerospike\ReadModeSC {}

        /**
         * Which node to prefer.
         *
         * @return \Aerospike\Replica|null
         */
        public function replica(): ?\Aerospike\Replica {}

        /**
         * Pause between retries.
         *
         * @return int|null
         */
        public function sleepBetweenRetriesMs(): ?int {}

        /**
         * Deadline for one socket operation.
         *
         * @return int|null
         */
        public function socketTimeoutMs(): ?int {}

        /**
         * Deadline for the whole verify batch, retries included.
         *
         * @return int|null
         */
        public function totalTimeoutMs(): ?int {}

        /**
         * Whether to compress a request worth compressing.
         *
         * @return bool|null
         */
        public function useCompression(): ?bool {}
    }

    /**
     * The language a UDF module is written in.
     */
    enum UdfLanguage: string {
    /**
     * Lua, which is the only language Aerospike runs.
     */
      case Lua = 'LUA';
        /** The 1.x spelling of a case of this enum. */
        public static function Lua(): \Aerospike\UdfLanguage {}

    }

    /**
     * One UDF module the cluster holds.
     *
     * Produced by `listUdf()`; there is no constructor, because a module that no
     * server reported would be a fiction.
     *
     * ```php
     * foreach ($client->listUdf(null) as $module) {
     *     echo $module->name(), ' ', $module->hash(), "\n";
     * }
     * ```
     */
    class UdfModule {
        public function __construct() {}

        /**
         * `name (language)`, for logs.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * The server's hash of the module's contents.
         *
         * How two nodes agree they hold the same module, and how a caller can tell
         * whether the module on the server is the one it has locally.
         *
         * @return string
         */
        public function hash(): string {}

        /**
         * The language, as the server reported it — a string rather than a
         * `UdfLanguage`, because it is the server's answer and a language this build
         * does not know must still be reportable.
         *
         * @return string
         */
        public function language(): string {}

        /**
         * The module's name on the server, e.g. `example.lua`.
         *
         * This is what `removeUdf()` takes, and what a UDF call names as its
         * package — **without** the `.lua`, in the call.
         *
         * @return string
         */
        public function name(): string {}
    }

    /**
     * One user the cluster knows.
     *
     * Produced by `queryUsers()`. There is no constructor: `createUser()` makes a user,
     * and one built here would describe nobody.
     */
    class User {
        public function __construct() {}

        /**
         * `name (role, role)`, for logs.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * Connections this user currently holds open.
         *
         * @return int
         */
        public function connsInUse(): int {}

        /**
         * Whether they hold this role.
         *
         * @param string $role
         * @return bool
         */
        public function hasRole(string $role): bool {}

        /**
         * The user name.
         *
         * @return string
         */
        public function name(): string {}

        /**
         * Read statistics, in the server's order: the quota in records per second, the
         * single-record rate, the scan/query record rate, and the number of limitless
         * read scans and queries.
         *
         * A list rather than named accessors because it is the *server's* list and a
         * future release may append to it. May be empty, when the server reported none.
         *
         * @return array
         */
        public function readInfo(): array {}

        /**
         * The roles assigned to them.
         *
         * @return array
         */
        public function roles(): array {}

        /**
         * Write statistics, in the same shape as `User::readInfo()`.
         *
         * @return array
         */
        public function writeInfo(): array {}
    }

    /**
     * Matches any value, for CDT selections by example. See [`Infinity`] for why
     * this is a class and not a constant.
     */
    class Wildcard {
        /**
         * The sentinel carries no state.
         */
        public function __construct() {}
    }

    /**
     * Per-call settings for a write: `put`, `delete`, `touch`, `add`, `append` and
     * `prepend`.
     *
     * Mirrors `aerospike-core`'s `WritePolicy`: everything a [`ReadPolicy`] has,
     * plus the fields that only mean something when a record is being changed.
     * Pass `null` where one of these is expected to accept the daemon's
     * configuration unchanged.
     *
     * ```php
     * // A conditional write: only if the record is unchanged since we read it.
     * $policy = new Aerospike\WritePolicy(
     *     generationPolicy: Aerospike\GenerationPolicy::ExpectGenEqual,
     *     generation:       $record->generation(),
     * );
     * $client->put($policy, $key, $bins);   // result code 3 if someone got there first
     * ```
     */
    class WritePolicy {
        /**
         * Build a write policy. Every argument is optional; an omitted one leaves
         * the daemon's own setting alone.
         *
         * camelCase parameters and `= null` defaults, for the reasons
         * [`ReadPolicy::__construct`] gives: PHP named arguments use the parameter
         * name as written, seventeen positional arguments is not an API, and a
         * named argument cannot skip a parameter whose default PHP does not know.
         *
         * @param int|null $totalTimeoutMs
         * @param int|null $socketTimeoutMs
         * @param int|null $maxRetries
         * @param int|null $sleepBetweenRetriesMs
         * @param \Aerospike\Replica|null $replica
         * @param \Aerospike\ReadModeAP|null $readModeAp
         * @param \Aerospike\ReadModeSC|null $readModeSc
         * @param bool|null $useCompression
         * @param string|null $filter
         * @param \Aerospike\Expression|null $filterExp
         * @param \Aerospike\Transaction|null $txn
         * @param \Aerospike\RecordExistsAction|null $recordExistsAction
         * @param \Aerospike\GenerationPolicy|null $generationPolicy
         * @param int|null $generation
         * @param \Aerospike\Expiration|null $expiration
         * @param \Aerospike\CommitLevel|null $commitLevel
         * @param bool|null $durableDelete
         * @param bool|null $respondPerEachOp
         * @param bool|null $sendKey
         */
        public function __construct(?int $totalTimeoutMs = null, ?int $socketTimeoutMs = null, ?int $maxRetries = null, ?int $sleepBetweenRetriesMs = null, ?\Aerospike\Replica $replica = null, ?\Aerospike\ReadModeAP $readModeAp = null, ?\Aerospike\ReadModeSC $readModeSc = null, ?bool $useCompression = null, ?string $filter = null, ?\Aerospike\Expression $filterExp = null, ?\Aerospike\Transaction $txn = null, ?\Aerospike\RecordExistsAction $recordExistsAction = null, ?\Aerospike\GenerationPolicy $generationPolicy = null, ?int $generation = null, ?\Aerospike\Expiration $expiration = null, ?\Aerospike\CommitLevel $commitLevel = null, ?bool $durableDelete = null, ?bool $respondPerEachOp = null, ?bool $sendKey = null) {}

        /**
         * How many replicas must commit before the server answers.
         *
         * @return \Aerospike\CommitLevel|null
         */
        public function commitLevel(): ?\Aerospike\CommitLevel {}

        /**
         * Whether a delete leaves a tombstone (Enterprise only).
         *
         * @return bool|null
         */
        public function durableDelete(): ?bool {}

        /**
         * The record's time-to-live after this write.
         *
         * @return \Aerospike\Expiration|null
         */
        public function expiration(): ?\Aerospike\Expiration {}

        /**
         * The filter as Aerospike Expression Language text.
         *
         * `null` when there is no filter **or** when it was built with
         * `Aerospike\\Exp` — a built expression never was text, and inventing some
         * would mean writing a serialiser nothing reads. Use
         * [`filter_exp`](Self::filter_exp) to get the filter whatever its form.
         *
         * @return string|null
         */
        public function filter(): ?string {}

        /**
         * The filter, in whichever form it was given.
         *
         * @return \Aerospike\Expression|null
         */
        public function filterExp(): ?\Aerospike\Expression {}

        /**
         * The generation the guard compares against.
         *
         * @return int|null
         */
        public function generation(): ?int {}

        /**
         * The generation guard.
         *
         * @return \Aerospike\GenerationPolicy|null
         */
        public function generationPolicy(): ?\Aerospike\GenerationPolicy {}

        /**
         * Retries after the first attempt.
         *
         * @return int|null
         */
        public function maxRetries(): ?int {}

        /**
         * AP-namespace read consistency, for the read a read-modify-write does.
         *
         * @return \Aerospike\ReadModeAP|null
         */
        public function readModeAp(): ?\Aerospike\ReadModeAP {}

        /**
         * SC-namespace read consistency.
         *
         * @return \Aerospike\ReadModeSC|null
         */
        public function readModeSc(): ?\Aerospike\ReadModeSC {}

        /**
         * Create/update/replace semantics.
         *
         * @return \Aerospike\RecordExistsAction|null
         */
        public function recordExistsAction(): ?\Aerospike\RecordExistsAction {}

        /**
         * Which node to prefer. Inert on a write, which always goes to the
         * partition master; accepted so one policy can be shared across verbs.
         *
         * @return \Aerospike\Replica|null
         */
        public function replica(): ?\Aerospike\Replica {}

        /**
         * Whether every operation reports a result, including ones that normally
         * report nothing.
         *
         * @return bool|null
         */
        public function respondPerEachOp(): ?bool {}

        /**
         * Whether the user key is stored alongside the digest.
         *
         * @return bool|null
         */
        public function sendKey(): ?bool {}

        /**
         * Pause between retries.
         *
         * @return int|null
         */
        public function sleepBetweenRetriesMs(): ?int {}

        /**
         * Deadline for one socket operation.
         *
         * @return int|null
         */
        public function socketTimeoutMs(): ?int {}

        /**
         * Deadline for the whole command, retries included.
         *
         * @return int|null
         */
        public function totalTimeoutMs(): ?int {}

        /**
         * The id of the transaction this policy joins, or `null`.
         *
         * The id rather than the `Transaction` object, for the reason
         * [`ReadPolicy::txn_id`] gives: a policy is meant to be reusable, and holding
         * the object would keep an unfinished transaction alive past the point its
         * destructor should have aborted it.
         *
         * @return int|null
         */
        public function txnId(): ?int {}

        /**
         * Whether to compress a request worth compressing.
         *
         * @return bool|null
         */
        public function useCompression(): ?bool {}
    }
}
