<?php

/**
 * Class aliases for 1.x names this client merely renamed.
 *
 * Only for names whose *shape* is unchanged — same methods, same arguments — so an
 * alias is the whole of the compatibility. Anything that needed behaviour is a real
 * class in one of the other files, and what could not be aliased is called out below.
 *
 * These land in `Aerospike\`, so old code needs no edit at all.
 *
 * # Why this file is short
 *
 * **PHP class names are case-insensitive**, so several 1.x names need nothing:
 * `Aerospike\GeoJSON` already *is* `Aerospike\GeoJson`, `HLL` is `Hll`, `BLOB` is
 * `Blob`. `class_alias` on any of those would be refused as a duplicate.
 *
 * And the extension carries the 1.x static factories on its own enums, so the eight
 * aliases below are pure renames: `ListOrderType::Ordered()` resolves to
 * `ListOrder::Ordered()`, which the extension defines.
 */

declare(strict_types=1);

// A rename and nothing more. Each target is an extension enum that already answers
// to the 1.x factory names, so the alias completes the job.
foreach ([
    'Aerospike\ListOrder' => 'Aerospike\ListOrderType',
    'Aerospike\MapOrder' => 'Aerospike\MapOrderType',
    'Aerospike\ListReturn' => 'Aerospike\ListReturnType',
    'Aerospike\MapReturn' => 'Aerospike\MapReturnType',
    'Aerospike\CollectionIndex' => 'Aerospike\IndexCollectionType',
    'Aerospike\BitResize' => 'Aerospike\BitwiseResizeFlags',
    'Aerospike\BitOverflow' => 'Aerospike\BitwiseOverflowAction',
    'Aerospike\BinWriteMode' => 'Aerospike\BinPolicyWriteMode',
] as $current => $legacy) {
    if (!class_exists($legacy, false) && !enum_exists($legacy, false)) {
        class_alias($current, $legacy);
    }
}

/*
 * Deliberately not aliased, and why:
 *
 * - `Recordset` → `RecordSet`: `next()` means different things, and the 1.x name is
 *   case-insensitively taken, so it cannot even be defined. See `collections.php`.
 * - `Context` → `Ctx`: the shape matches, but 1.x also had `listOrderFlag()`, so
 *   `collections.php` defines a class rather than an alias.
 * - `BitwiseOp` → `BitOp`: the argument order differs, so `ops.php` defines the 1.x
 *   name as a real class. Same for `ListOp`, `MapOp` and `HllOp`, whose 1.x names
 *   *are* case-insensitively taken — those live in `Aerospike\Compat\`.
 * - `ReadModeAP`, `ReadModeSC`, `CommitLevel`, `GenerationPolicy`,
 *   `RecordExistsAction`, `IndexType`, `UdfLanguage`, `MapWriteMode`, `ExpType`:
 *   the extension registers these under the 1.x name already.
 * - `ResultCode` → `Status`: this client's is a PHP enum, and `class_alias` on an
 *   enum gives a name that cannot be used as one — `ResultCode::KEY_NOT_FOUND_ERROR`
 *   would not resolve. `result_code.php` defines it as integer constants, which is
 *   what 1.x had and what `$e->getResultCode()` returns.
 */
