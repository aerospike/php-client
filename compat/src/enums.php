<?php

/**
 * The 1.x flag sets and settings the extension does not itself provide.
 *
 * Most of what this file would have held is gone, because the extension now carries
 * the 1.x static factories on its **own** enums: `ReadModeAP::one()`,
 * `ReadModeSC::Session()`, `CommitLevel::CommitAll()`, `GenerationPolicy::*`,
 * `RecordExistsAction::*`, `IndexType::String()`, `UdfLanguage::Lua()`,
 * `MapWriteMode::*`, `ExpType::Int()`, `ListOrderType`/`MapOrderType`'s cases,
 * `ListReturnType`/`MapReturnType`'s cases, `IndexCollectionType::Default()`,
 * `BitwiseResizeFlags::Default()` and `BitwiseOverflowAction::*` all resolve with no
 * help from here.
 *
 * That had to be done in the extension rather than in PHP, for two reasons:
 *
 * - **PHP class names are case-insensitive.** `Aerospike\ReadModeAP` and
 *   `Aerospike\ReadModeAp` are the *same* name, so a PHP file cannot define the 1.x
 *   spelling at all — the extension already owns it. The same is true of `GeoJSON`
 *   and `HLL`.
 * - A PHP enum's cases cannot be given methods from outside, and a sibling class
 *   would be a second answer to the same question.
 *
 * What remains is the handful of 1.x classes that were never enums here: bitmask
 * flag sets, and settings this client moved into the daemon's configuration or
 * replaced outright. All land in `Aerospike\`, so old code needs no edit.
 */

declare(strict_types=1);

namespace Aerospike;

/**
 * 1.x list sort flags, as the integers the server uses.
 *
 * This client takes the two as booleans on `ListOp::sort($bin, $descending,
 * $dropDuplicates)` — which is what they are — so these exist for code that OR-ed
 * them together and passes the result to {@see \Aerospike\Compat\ListOp::sort()},
 * which decomposes it.
 */
class ListSortFlags
{
    public const DEFAULT = 0;
    public const DESCENDING = 1;
    public const DROP_DUPLICATES = 2;

    public static function Default(): int
    {
        return self::DEFAULT;
    }

    public static function Descending(): int
    {
        return self::DESCENDING;
    }

    public static function DropDuplicates(): int
    {
        return self::DROP_DUPLICATES;
    }
}

/**
 * 1.x list write flags, as the integers the server uses.
 *
 * This client models them as the named booleans of `Aerospike\ListPolicy`;
 * {@see \Aerospike\Compat\ListPolicy} decomposes an OR-ed bitmask into them.
 */
class ListWriteFlags
{
    public const DEFAULT = 0;
    public const ADD_UNIQUE = 1;
    public const INSERT_BOUNDED = 2;
    public const NO_FAIL = 4;
    public const PARTIAL = 8;

    public static function Default(): int
    {
        return self::DEFAULT;
    }

    public static function AddUnique(): int
    {
        return self::ADD_UNIQUE;
    }

    public static function InsertBounded(): int
    {
        return self::INSERT_BOUNDED;
    }

    public static function NoFail(): int
    {
        return self::NO_FAIL;
    }

    public static function Partial(): int
    {
        return self::PARTIAL;
    }
}

/** 1.x map write flags, as the integers the server uses. */
class MapWriteFlags
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

    public static function CreateOnly(): int
    {
        return self::CREATE_ONLY;
    }

    public static function UpdateOnly(): int
    {
        return self::UPDATE_ONLY;
    }

    public static function NoFail(): int
    {
        return self::NO_FAIL;
    }

    public static function Partial(): int
    {
        return self::PARTIAL;
    }
}

/** 1.x HLL write flags, as the integers the server uses. */
class HllWriteFlags
{
    public const DEFAULT = 0;
    public const CREATE_ONLY = 1;
    public const UPDATE_ONLY = 2;
    public const NO_FAIL = 4;
    public const ALLOW_FOLD = 8;

    public static function Default(): int
    {
        return self::DEFAULT;
    }

    public static function CreateOnly(): int
    {
        return self::CREATE_ONLY;
    }

    public static function UpdateOnly(): int
    {
        return self::UPDATE_ONLY;
    }

    public static function NoFail(): int
    {
        return self::NO_FAIL;
    }

    public static function AllowFold(): int
    {
        return self::ALLOW_FOLD;
    }
}

/**
 * 1.x batch concurrency.
 *
 * Accepted and inert. The daemon decides batch concurrency from its own
 * configuration, because the right number depends on the host's cores and the
 * cluster's size rather than on any one call. Kept so old code keeps parsing.
 */
class Concurrency
{
    public static function Sequential(): int
    {
        return 0;
    }

    public static function Parallel(): int
    {
        return 1;
    }

    public static function MaxThreads(int $threads): int
    {
        return $threads;
    }
}

/**
 * 1.x consistency level.
 *
 * Superseded by the server before the 1.x client shipped: it became the AP and SC
 * read modes, which are `Aerospike\ReadModeAp` and `Aerospike\ReadModeSc`. These map
 * onto the AP mode, which is what the old setting meant in practice.
 */
class ConsistencyLevel
{
    public static function ConsistencyOne(): ReadModeAp
    {
        return ReadModeAp::One;
    }

    public static function ConsistencyAll(): ReadModeAp
    {
        return ReadModeAp::All;
    }
}

/**
 * 1.x query duration hint.
 *
 * Accepted and inert. This client pages a traversal explicitly, with `pageSize` on
 * `Aerospike\QueryPolicy` — the same control stated as a quantity rather than as a
 * hint the server interprets.
 */
class QueryDuration
{
    public static function long(): int
    {
        return 0;
    }

    public static function short(): int
    {
        return 1;
    }

    public static function longRelaxAP(): int
    {
        return 2;
    }
}

/**
 * The server's particle-type numbers.
 *
 * 1.x exposed these because `Expression::binType()` returns one, and this client's
 * returns the same integers — so these constants are still how you read that answer.
 */
class ParticleType
{
    public const NULL = 0;
    public const INTEGER = 1;
    public const FLOAT = 2;
    public const STRING = 3;
    public const BLOB = 4;
    public const DIGEST = 6;
    public const BOOL = 17;
    public const HLL = 18;
    public const MAP = 19;
    public const LIST = 20;
    public const GEOJSON = 23;

    public static function null(): int
    {
        return self::NULL;
    }

    public static function integer(): int
    {
        return self::INTEGER;
    }

    public static function float(): int
    {
        return self::FLOAT;
    }

    public static function string(): int
    {
        return self::STRING;
    }

    public static function blob(): int
    {
        return self::BLOB;
    }

    public static function digest(): int
    {
        return self::DIGEST;
    }

    public static function bool(): int
    {
        return self::BOOL;
    }

    public static function hll(): int
    {
        return self::HLL;
    }

    public static function map(): int
    {
        return self::MAP;
    }

    public static function list(): int
    {
        return self::LIST;
    }

    public static function geoJson(): int
    {
        return self::GEOJSON;
    }
}
