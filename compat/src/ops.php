<?php

/**
 * The 1.x client's CDT operations.
 *
 * Two differences, and they are mechanical:
 *
 * 1. **Argument order.** 1.x put the policy first and the context path last, on
 *    every call: `ListOp::append($policy, $bin, $values, $ctx)`. This client puts
 *    the bin first and attaches the path fluently:
 *    `ListOp::append($bin, $values, $policy)->context($ctx)`.
 * 2. **Return types.** 1.x passed a `ListReturnType` instance; this client takes a
 *    `ListReturn` enum case. {@see \Aerospike\ListReturnType} bridges that, so a
 *    value from old code arrives here already correct.
 *
 * Only the operations whose signature actually differs are here. Anything 1.x
 * spelled the same way this client does is reached through the real
 * `Aerospike\ListOp` — there is no reason to wrap it.
 *
 * ```php
 * use Aerospike\Compat\ListOp;
 *
 * $client->operate($policy, $key, [
 *     ListOp::append(null, 'history', ['viewed'], null),
 *     ListOp::size('history', null),
 * ]);
 * ```
 */

declare(strict_types=1);

namespace Aerospike\Compat;

use Aerospike\ListOp as NewListOp;
use Aerospike\ListPolicy as NewListPolicy;
use Aerospike\MapOp as NewMapOp;
use Aerospike\MapPolicy as NewMapPolicy;
use Aerospike\BitOp as NewBitOp;
use Aerospike\HllOp as NewHllOp;
use Aerospike\Operation;

/**
 * A 1.x list policy: an order plus an OR-ed bitmask of write flags.
 *
 * This client's `ListPolicy` takes the flags as named booleans, which is what a
 * bitmask crossing a language boundary should have been all along. This decomposes
 * the old integer.
 */
class ListPolicy
{
    public function __construct(
        public mixed $order = null,
        public int $flags = 0,
    ) {
    }

    public function build(): NewListPolicy
    {
        return new NewListPolicy(
            order: $this->order,
            addUnique: (bool) ($this->flags & \Aerospike\ListWriteFlags::ADD_UNIQUE),
            insertBounded: (bool) ($this->flags & \Aerospike\ListWriteFlags::INSERT_BOUNDED),
            noFail: (bool) ($this->flags & \Aerospike\ListWriteFlags::NO_FAIL),
            partial: (bool) ($this->flags & \Aerospike\ListWriteFlags::PARTIAL),
        );
    }
}

/** A 1.x map policy: a write mode plus an OR-ed bitmask of flags. */
class MapPolicy
{
    public function __construct(
        public mixed $order = null,
        public mixed $writeMode = null,
        public int $flags = 0,
    ) {
    }

    public function build(): NewMapPolicy
    {
        return new NewMapPolicy(
            order: $this->order,
            writeMode: $this->writeMode,
            noFail: (bool) ($this->flags & \Aerospike\MapWriteFlags::NO_FAIL),
            partial: (bool) ($this->flags & \Aerospike\MapWriteFlags::PARTIAL),
        );
    }
}

/** Attach a 1.x context path, if there is one. */
function withPath(Operation $op, ?array $ctx): Operation
{
    return ($ctx === null || $ctx === []) ? $op : $op->context($ctx);
}

/** 1.x {@see \Aerospike\ListOp}: policy first, context last. */
class ListOp
{
    public static function create(string $bin, mixed $order, bool $pad, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::create($bin, $order, $pad), $ctx);
    }

    public static function setOrder(string $bin, mixed $order, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::setOrder($bin, $order), $ctx);
    }

    public static function append(mixed $policy, string $bin, array $values, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::appendItems($bin, $values, built($policy)), $ctx);
    }

    public static function insert(mixed $policy, string $bin, int $index, array $values, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::insertItems($bin, $index, $values, built($policy)), $ctx);
    }

    public static function pop(string $bin, int $index, ?array $ctx = null): Operation
    {
        return withPath(
            NewListOp::removeByIndex($bin, $index, \Aerospike\ListReturn::Values),
            $ctx
        );
    }

    public static function popRange(string $bin, int $index, int $count, ?array $ctx = null): Operation
    {
        return withPath(
            NewListOp::removeByIndexRange($bin, $index, $count, \Aerospike\ListReturn::Values),
            $ctx
        );
    }

    public static function popRangeFrom(string $bin, int $index, ?array $ctx = null): Operation
    {
        return withPath(
            NewListOp::removeByIndexRange($bin, $index, null, \Aerospike\ListReturn::Values),
            $ctx
        );
    }

    /** 1.x `removeValues`, which this client calls `removeByValueList`. */
    public static function removeValues(string $bin, array $values, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::removeByValueList($bin, $values, $returnType), $ctx);
    }

    public static function removeRangeFrom(string $bin, int $index, ?array $ctx = null): Operation
    {
        return withPath(
            NewListOp::removeByIndexRange($bin, $index, null, \Aerospike\ListReturn::None),
            $ctx
        );
    }

    /** 1.x `getByValues`, which this client calls `getByValueList`. */
    public static function getByValues(string $bin, array $values, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::getByValueList($bin, $values, $returnType), $ctx);
    }

    public static function size(string $bin, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::size($bin), $ctx);
    }

    /**
     * 1.x `sort`, whose flags were an OR-ed bitmask.
     *
     * This client takes the two flags as booleans, which is what they are.
     */
    public static function sort(string $bin, int $sortFlags = 0, ?array $ctx = null): Operation
    {
        return withPath(
            NewListOp::sort(
                $bin,
                (bool) ($sortFlags & \Aerospike\ListSortFlags::DESCENDING),
                (bool) ($sortFlags & \Aerospike\ListSortFlags::DROP_DUPLICATES),
            ),
            $ctx
        );
    }

    /**
     * The 1.x `*RangeCount` operations.
     *
     * This client collapsed each pair into one method with `count: ?int`, because
     * that is what the pair meant — `null` is "to the end".
     */
    public static function removeByIndexRangeCount(string $bin, int $index, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::removeByIndexRange($bin, $index, $count, $returnType), $ctx);
    }

    public static function removeByRankRangeCount(string $bin, int $rank, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::removeByRankRange($bin, $rank, $count, $returnType), $ctx);
    }

    public static function getByIndexRangeCount(string $bin, int $index, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::getByIndexRange($bin, $index, $count, $returnType), $ctx);
    }

    public static function getByRankRangeCount(string $bin, int $rank, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewListOp::getByRankRange($bin, $rank, $count, $returnType), $ctx);
    }

    public static function removeByValueRelativeRankRangeCount(string $bin, mixed $value, int $rank, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(
            NewListOp::removeByValueRelativeRankRange($bin, $value, $rank, $count, $returnType),
            $ctx
        );
    }

    public static function getByValueRelativeRankRangeCount(string $bin, mixed $value, int $rank, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(
            NewListOp::getByValueRelativeRankRange($bin, $value, $rank, $count, $returnType),
            $ctx
        );
    }

    /**
     * Anything 1.x spelled the way this client does.
     *
     * Forwarded rather than listed: those signatures differ only in the trailing
     * context, which this adds. A name the real `ListOp` does not have is the real
     * class's error to report, and its message names the class.
     */
    public static function __callStatic(string $name, array $arguments): Operation
    {
        $ctx = null;
        if ($arguments !== [] && (is_array(end($arguments)) || end($arguments) === null)) {
            $last = end($arguments);
            if ($last === null || ($last !== [] && is_object(reset($last)))) {
                $ctx = array_pop($arguments);
            }
        }
        return withPath(NewListOp::$name(...$arguments), $ctx);
    }
}

/** 1.x {@see \Aerospike\MapOp}: policy first, context last. */
class MapOp
{
    public static function put(mixed $policy, string $bin, mixed $key, mixed $value, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::put($bin, $key, $value, built($policy)), $ctx);
    }

    public static function putItems(mixed $policy, string $bin, mixed $items, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::putItems($bin, $items, built($policy)), $ctx);
    }

    /** 1.x `increment`, which this client calls `incrementValue`. */
    public static function increment(mixed $policy, string $bin, mixed $key, mixed $delta, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::incrementValue($bin, $key, $delta, built($policy)), $ctx);
    }

    /**
     * 1.x `decrement`, which this client calls `decrementValue`.
     *
     * The delta is checked here rather than left to the server, because a
     * non-numeric one is a mistake in the calling code and the server's answer to it
     * names a bin rather than an argument.
     */
    public static function decrement(mixed $policy, string $bin, mixed $key, mixed $delta, ?array $ctx = null): Operation
    {
        if (!is_int($delta) && !is_float($delta)) {
            throw new \InvalidArgumentException(
                'decrement() needs a number to subtract, but was given ' . get_debug_type($delta)
            );
        }
        return withPath(NewMapOp::decrementValue($bin, $key, $delta, built($policy)), $ctx);
    }

    public static function size(string $bin, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::size($bin), $ctx);
    }

    /** 1.x `removeByKeys`, which this client calls `removeByKeyList`. */
    public static function removeByKeys(string $bin, array $keys, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::removeByKeyList($bin, $keys, $returnType), $ctx);
    }

    /** 1.x `removeByValues`, which this client calls `removeByValueList`. */
    public static function removeByValues(string $bin, array $values, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::removeByValueList($bin, $values, $returnType), $ctx);
    }

    /** 1.x `getByKeys`, which this client calls `getByKeyList`. */
    public static function getByKeys(string $bin, array $keys, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::getByKeyList($bin, $keys, $returnType), $ctx);
    }

    /** 1.x `getByValues`, which this client calls `getByValueList`. */
    public static function getByValues(string $bin, array $values, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::getByValueList($bin, $values, $returnType), $ctx);
    }

    public static function removeByIndexRangeCount(string $bin, int $index, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::removeByIndexRange($bin, $index, $count, $returnType), $ctx);
    }

    public static function removeByRankRangeCount(string $bin, int $rank, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::removeByRankRange($bin, $rank, $count, $returnType), $ctx);
    }

    public static function getByIndexRangeCount(string $bin, int $index, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::getByIndexRange($bin, $index, $count, $returnType), $ctx);
    }

    public static function getByRankRangeCount(string $bin, int $rank, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(NewMapOp::getByRankRange($bin, $rank, $count, $returnType), $ctx);
    }

    public static function removeByKeyRelativeIndexRangeCount(string $bin, mixed $key, int $index, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(
            NewMapOp::removeByKeyRelativeIndexRange($bin, $key, $index, $count, $returnType),
            $ctx
        );
    }

    public static function getByKeyRelativeIndexRangeCount(string $bin, mixed $key, int $index, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(
            NewMapOp::getByKeyRelativeIndexRange($bin, $key, $index, $count, $returnType),
            $ctx
        );
    }

    public static function removeByValueRelativeRankRangeCount(string $bin, mixed $value, int $rank, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(
            NewMapOp::removeByValueRelativeRankRange($bin, $value, $rank, $count, $returnType),
            $ctx
        );
    }

    public static function getByValueRelativeRankRangeCount(string $bin, mixed $value, int $rank, int $count, mixed $returnType = null, ?array $ctx = null): Operation
    {
        return withPath(
            NewMapOp::getByValueRelativeRankRange($bin, $value, $rank, $count, $returnType),
            $ctx
        );
    }

    /** As {@see ListOp::__callStatic}. */
    public static function __callStatic(string $name, array $arguments): Operation
    {
        $ctx = null;
        if ($arguments !== [] && (is_array(end($arguments)) || end($arguments) === null)) {
            $last = end($arguments);
            if ($last === null || ($last !== [] && is_object(reset($last)))) {
                $ctx = array_pop($arguments);
            }
        }
        return withPath(NewMapOp::$name(...$arguments), $ctx);
    }
}

/**
 * 1.x `BitwiseOp`, which this client calls `BitOp`.
 *
 * 1.x took the policy first on every modify operation and a context last; this
 * client takes the bin first and the policy last, and the bitwise operations have
 * no context at all — a blob has no nested structure to point into. A context
 * passed here is therefore refused rather than dropped.
 */
class BitwiseOp
{
    public static function resize(mixed $policy, string $bin, int $byteSize, mixed $resizeFlags = null, ?array $ctx = null): Operation
    {
        self::noContext($ctx, 'resize');
        return NewBitOp::resize($bin, $byteSize, $resizeFlags, $policy);
    }

    public static function insert(mixed $policy, string $bin, int $byteOffset, mixed $value, ?array $ctx = null): Operation
    {
        self::noContext($ctx, 'insert');
        return NewBitOp::insert($bin, $byteOffset, $value, $policy);
    }

    public static function remove(mixed $policy, string $bin, int $byteOffset, int $byteSize, ?array $ctx = null): Operation
    {
        self::noContext($ctx, 'remove');
        return NewBitOp::remove($bin, $byteOffset, $byteSize, $policy);
    }

    public static function set(mixed $policy, string $bin, int $bitOffset, int $bitSize, mixed $value, ?array $ctx = null): Operation
    {
        self::noContext($ctx, 'set');
        return NewBitOp::set($bin, $bitOffset, $bitSize, $value, $policy);
    }

    public static function or(mixed $policy, string $bin, int $bitOffset, int $bitSize, mixed $value, ?array $ctx = null): Operation
    {
        self::noContext($ctx, 'or');
        return NewBitOp::or($bin, $bitOffset, $bitSize, $value, $policy);
    }

    public static function xor(mixed $policy, string $bin, int $bitOffset, int $bitSize, mixed $value, ?array $ctx = null): Operation
    {
        self::noContext($ctx, 'xor');
        return NewBitOp::xor($bin, $bitOffset, $bitSize, $value, $policy);
    }

    public static function and(mixed $policy, string $bin, int $bitOffset, int $bitSize, mixed $value, ?array $ctx = null): Operation
    {
        self::noContext($ctx, 'and');
        return NewBitOp::and($bin, $bitOffset, $bitSize, $value, $policy);
    }

    public static function not(mixed $policy, string $bin, int $bitOffset, int $bitSize, ?array $ctx = null): Operation
    {
        self::noContext($ctx, 'not');
        return NewBitOp::not($bin, $bitOffset, $bitSize, $policy);
    }

    private static function noContext(?array $ctx, string $method): void
    {
        if ($ctx !== null && $ctx !== []) {
            throw new \InvalidArgumentException(sprintf(
                'BitwiseOp::%s() was given a context path, and the bitwise operations do not take '
                . 'one: they act on a blob, which has no nested structure to point into. The 1.x '
                . 'client accepted the argument and ignored it',
                $method
            ));
        }
    }

    /** As {@see ListOp::__callStatic}, minus the context handling. */
    public static function __callStatic(string $name, array $arguments): Operation
    {
        return NewBitOp::$name(...$arguments);
    }
}

/**
 * 1.x `HllOp`.
 *
 * Same shape difference as the bitwise operations: policy first there, last here,
 * and no context.
 */
class HllOp
{
    public static function init(mixed $policy, string $bin, int $indexBitCount, int $minHashBitCount = -1): Operation
    {
        return NewHllOp::init($bin, $indexBitCount, $minHashBitCount, $policy);
    }

    public static function add(mixed $policy, string $bin, array $list, int $indexBitCount = -1, int $minHashBitCount = -1): Operation
    {
        return NewHllOp::add($bin, $list, $indexBitCount, $minHashBitCount, $policy);
    }

    /** As {@see ListOp::__callStatic}. */
    public static function __callStatic(string $name, array $arguments): Operation
    {
        return NewHllOp::$name(...$arguments);
    }
}

// The extension registers `Aerospike\BitOp`, so the 1.x name `Aerospike\BitwiseOp`
// is free and can be the real one: old code that writes it needs no edit at all.
// `ListOp`, `MapOp`, `HllOp`, `ListPolicy` and `MapPolicy` get no such alias, because
// the extension already owns those exact names — reach them through `Compat\`.
if (!class_exists('Aerospike\BitwiseOp', false)) {
    class_alias('Aerospike\Compat\BitwiseOp', 'Aerospike\BitwiseOp');
}
