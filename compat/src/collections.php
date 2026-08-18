<?php

/**
 * The 1.x value helpers, context steps, and the record-set wrapper.
 *
 * # The one place this layer cannot be transparent
 *
 * 1.x `Recordset::next()` **returns** the next record. This client's `RecordSet`
 * implements PHP's `Iterator`, whose `next()` returns void and only advances. And
 * because **PHP class names are case-insensitive**, `Aerospike\Recordset` already
 * *is* `Aerospike\RecordSet` — this file cannot define the 1.x name, and the
 * extension cannot give `next()` the 1.x meaning without breaking `Iterator`.
 *
 * So old code that writes `while (($r = $rs->next()) !== null)` against the native
 * class gets `null` immediately and iterates **zero times**, silently. That is the
 * worst failure mode available, so the wrapper below lives in `Aerospike\Compat\`
 * and {@see \Aerospike\Compat\Client} returns it: a `Aerospike\Recordset` type
 * hint then raises a `TypeError` — loud, at the call, naming both classes — instead
 * of a loop that quietly does nothing.
 *
 * Code that only uses `foreach` is unaffected either way. Code that pulls records
 * has two fixes: use `foreach`, or call `nextRecord()`, which the extension provides
 * on the native class with 1.x semantics.
 */

declare(strict_types=1);

namespace Aerospike\Compat {

    use Aerospike\Record;

    // `Aerospike\RecordSet` is deliberately *not* imported: an import is a local name,
    // and a local `RecordSet` would be the same name as the `Recordset` declared just
    // below — PHP refuses that. Hence the fully qualified references throughout.

    /**
     * 1.x `Recordset`, wrapping this client's {@see \Aerospike\RecordSet}.
     *
     * Both idioms work on this class:
     *
     * ```php
     * while (($record = $recordset->next()) !== null) { … }   // 1.x
     * foreach ($recordset as $record) { … }                  // either
     * ```
     *
     * It is an `IteratorAggregate` rather than an `Iterator` for exactly the reason
     * this whole class exists: `Iterator::next()` is declared `: void`, and PHP does
     * not let an implementation widen that to `: ?Record`. Aggregating hands `foreach`
     * a separate iterator and leaves `next()` free to mean what 1.x meant by it.
     *
     * See this file's own note for why it is here rather than in `Aerospike\`.
     */
    class Recordset implements \IteratorAggregate
    {
        /** Whether `next()` has been used, so `foreach` does not double-advance. */
        private bool $pulled = false;

        public function __construct(private \Aerospike\RecordSet $inner)
        {
        }

        /**
         * The next record, or `null` at the end — the 1.x meaning of `next()`.
         *
         * Note that this is **not** `Iterator::next()`'s meaning. Mixing the two on
         * one object would skip records, so this tracks that it was used and the
         * iterator methods below account for it.
         */
        public function next(): ?Record
        {
            if ($this->pulled) {
                $this->inner->next();
            }
            $this->pulled = true;
            return $this->inner->valid() ? $this->inner->current() : null;
        }

        /** Whether the traversal is still open. */
        public function getActive(): bool
        {
            return $this->inner->valid();
        }

        /** Release the traversal's cursor. */
        public function close(): void
        {
            $this->inner->close();
        }

        /**
         * The traversal, for `foreach`.
         *
         * This is the *inner* record set, so `foreach` costs nothing extra and a
         * `foreach` that `break`s leaves the cursor where 1.x would have. Do not mix
         * this with `next()` on one object: they would each advance it.
         */
        public function getIterator(): \Iterator
        {
            return $this->inner;
        }

        /** Whether there is a record to read. */
        public function valid(): bool
        {
            return $this->inner->valid();
        }

        /** The wrapped `RecordSet`, for code that has moved on. */
        public function inner(): \Aerospike\RecordSet
        {
            return $this->inner;
        }
    }
}

namespace Aerospike {

    use Aerospike\Ctx;

    /**
     * 1.x `Value`, which named the type of a bin value explicitly.
     *
     * This client infers the type from the PHP value and has wrapper classes for
     * the ones PHP has no literal for, so most of these are the identity. The three
     * that are not — `blob`, `geoJson`, `hll` — return the wrapper.
     */
    class Value
    {
        public static function nil(): mixed
        {
            return null;
        }

        public static function int(int $value): int
        {
            return $value;
        }

        /**
         * 1.x accepted an unsigned 64-bit integer here.
         *
         * Aerospike stores a signed 64-bit integer, and PHP has no unsigned one, so
         * 1.x could only have been passing the same bits through. This is the
         * identity, and a negative number is not an error because that is what a
         * large unsigned value looks like in PHP.
         */
        public static function uint(int $value): int
        {
            return $value;
        }

        public static function float(float $value): float
        {
            return $value;
        }

        public static function bool(bool $value): bool
        {
            return $value;
        }

        public static function string(string $value): string
        {
            return $value;
        }

        public static function list(array $value): array
        {
            return $value;
        }

        public static function map(mixed $value): mixed
        {
            return $value;
        }

        /** A byte string. 1.x took an array of byte integers; a string works too. */
        public static function blob(array|string $value): Blob
        {
            return new Blob(is_string($value) ? $value : self::bytes($value));
        }

        public static function geoJson(string $value): GeoJson
        {
            return new GeoJson($value);
        }

        /** A HyperLogLog sketch. Only ever bytes the server produced. */
        public static function hll(array|string $value): Hll
        {
            return new Hll(is_string($value) ? $value : self::bytes($value));
        }

        /**
         * 1.x `json`, which stored an array as a map.
         *
         * That is what writing an array does here, so this is the identity — see
         * the extension's documentation for the list-versus-map rule.
         */
        public static function json(array $value): array
        {
            return $value;
        }

        /** An array of byte integers as a string, for `blob` and `hll`. */
        private static function bytes(array $value): string
        {
            $out = '';
            foreach ($value as $index => $byte) {
                if (!is_int($byte) || $byte < 0 || $byte > 255) {
                    throw new \InvalidArgumentException(sprintf(
                        'byte %s is %s, and a byte is an int from 0 to 255',
                        $index,
                        get_debug_type($byte)
                    ));
                }
                $out .= chr($byte);
            }
            return $out;
        }
    }

    /**
     * 1.x `Context`, which this client calls {@see \Aerospike\Ctx}.
     *
     * Same factory names and the same arguments, so these forward directly. Kept as
     * a class rather than a `class_alias` only because of `listOrderFlag`, which
     * this client has no equivalent for.
     */
    class Context
    {
        public static function listIndex(int $index): Ctx
        {
            return Ctx::listIndex($index);
        }

        public static function listIndexCreate(int $index, mixed $order, bool $pad): Ctx
        {
            return Ctx::listIndexCreate($index, $order, $pad);
        }

        public static function listRank(int $rank): Ctx
        {
            return Ctx::listRank($rank);
        }

        public static function listValue(mixed $value): Ctx
        {
            return Ctx::listValue($value);
        }

        public static function mapIndex(int $index): Ctx
        {
            return Ctx::mapIndex($index);
        }

        public static function mapRank(int $rank): Ctx
        {
            return Ctx::mapRank($rank);
        }

        public static function mapKey(mixed $key): Ctx
        {
            return Ctx::mapKey($key);
        }

        public static function mapKeyCreate(mixed $key, mixed $order): Ctx
        {
            return Ctx::mapKeyCreate($key, $order);
        }

        public static function mapValue(mixed $value): Ctx
        {
            return Ctx::mapValue($value);
        }

        /**
         * The 1.x flag arithmetic for a created nested list.
         *
         * Exposed there because the caller had to compute the flag byte; here
         * `Ctx::listIndexCreate()` takes the order and the pad flag and does it. The
         * numbers are the server's, kept so old code that computed one still gets
         * the same answer.
         */
        public static function listOrderFlag(mixed $order, bool $pad): int
        {
            $ordered = $order === ListOrder::Ordered
                || (is_object($order) && method_exists($order, 'flag') && $order->flag() === 1);
            if ($ordered) {
                return 0xc0;
            }
            return $pad ? 0x80 : 0x40;
        }
    }

    /**
     * 1.x `UserRole`, the pair a `queryUsers()` answer carried.
     *
     * This client returns `Aerospike\User` objects, whose `roles()` is the list of
     * role names — the same information without a second class. This exists so old
     * code that type-hinted the pair still loads.
     */
    class UserRole
    {
        public function __construct(
            public string $user = '',
            public array $roles = [],
        ) {
        }

        public function getUser(): string
        {
            return $this->user;
        }

        public function getRoles(): array
        {
            return $this->roles;
        }
    }
}
