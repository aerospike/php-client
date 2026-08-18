# Aerospike PHP 1.x compatibility layer

Pure PHP. No extension code, no wire changes. Load it beside the extension and
most of a 1.x script runs; the rest needs one `use` line changed per file.

```php
require_once 'aerospike-php/compat/src/bootstrap.php';
```

## Why a layer at all, and why it cannot be complete

The 1.x client and this one have the same vocabulary and different shapes. Four
differences run through everything:

| | 1.x | now |
| --- | --- | --- |
| policies | `new WritePolicy()` then `setExpiration(...)`, 17 public properties | one named-argument constructor, immutable |
| enums | classes with static factories — `CommitLevel::CommitAll()` | real PHP `enum`s — `CommitLevel::CommitAll` |
| CDT operations | `ListOp::append($policy, $bin, $values, $ctx)` | `ListOp::append($bin, $values, $policy)->context($ctx)` |
| connection | `Client::connect('/path/to.sock')` | `new Client('instance')` |

**The hard constraint is PHP's, not ours**, and it is sharper than it first
appears. Both clients live in `namespace Aerospike`, the extension registers its
classes at load time, and **PHP class and method names are case-insensitive**. So
`Aerospike\Recordset` *is* `Aerospike\RecordSet`, `GeoJSON` *is* `GeoJson`, `HLL`
*is* `Hll` — a PHP file cannot define any of those names, and does not need to for
the ones whose shape did not change. What it means for the ones whose shape *did*
change is the first of the notes further down.

The layer therefore works in two tiers:

**Tier 1 — drop-in.** For names the extension does *not* use, the old name is
defined in `Aerospike\` and old code needs no edit at all. That covers
`ListOrderType`, `MapReturnType`, `IndexCollectionType`, `BitwiseResizeFlags`,
`BitwiseOverflowAction`, `BitwiseOp`, `ResultCode`, the four flag-set classes, the
value helpers, `BatchRecord`, `UdfMeta`, `Json`, and the settings this client
replaced (`Concurrency`, `ConsistencyLevel`, `QueryDuration`).

**Tier 2 — one line per file.** For names the extension owns, the shim lives in
`Aerospike\Compat\`. Migration is changing the import:

```php
-use Aerospike\WritePolicy;
+use Aerospike\Compat\WritePolicy;
```

The body of the code is untouched — `->setExpiration(3600)` still works.

## What is drop-in already, with no shim

These needed no compatibility layer because the extension itself carries the 1.x
spellings (see the extension's own docs):

- **The whole 1.x expression builder.** `Expression::gt(Expression::intBin('age'),
  Expression::intVal(21))` works unchanged — all 73 static methods, same argument
  order. Only `Expression::new()`, the raw opcode constructor, is gone.
- **Every 1.x enum factory the extension's own enums can carry.**
  `ReadModeAP::one()`, `ReadModeSC::Session()`, `CommitLevel::CommitAll()`,
  `GenerationPolicy::*`, `RecordExistsAction::*`, `IndexType::String()`,
  `UdfLanguage::Lua()`, `MapWriteMode::*`, `ExpType::Int()`, `ListOrder::*`,
  `MapOrder::*`, `ListReturn::*`, `MapReturn::*`, `CollectionIndex::Default()`,
  `BitResize::Default()`, `BitOverflow::*` — 67 static methods on 16 enums. This
  had to be in the extension rather than here, because the case-insensitivity above
  means a PHP file cannot define `Aerospike\ReadModeAP` while the extension owns
  `Aerospike\ReadModeAp`.
- `Record::getBins()`, `getGeneration()`, `getTtl()`, `getKey()`, `getExpiration()`,
  and `$record->bins` / `->generation` / `->ttl` / `->key` / `->expiration`.
- `Key::getNamespace()`, `getSetname()`, `getValue()`, `getDigest()`,
  `getDigestBytes()`, `partitionId()`, and the matching properties.
- `Client::getHeader()` and `Client::scan($policy, $partitions, $ns, $set, $bins)`.
- `RecordSet::nextRecord()` and `getActive()`, and `BatchResult::getRecord()` and
  `getResultCode()`.

## What this layer provides

| file | provides |
| --- | --- |
| `aliases.php` | `class_alias` for the eight renames whose shape is unchanged and whose 1.x name is free |
| `enums.php` | the 1.x flag sets (`ListSortFlags`, `ListWriteFlags`, `MapWriteFlags`, `HllWriteFlags`) and the replaced settings (`Concurrency`, `ConsistencyLevel`, `QueryDuration`, `ParticleType`) |
| `result_code.php` | `ResultCode`, `BitwiseWriteFlags`, `BitwisePolicy`, `HllPolicy`, `UdfMeta`, `BatchRecord`, `Json`, `PartitionStatus` |
| `policies.php` | `Compat\{Write,Read,Query,Scan,Batch,Info,Admin}Policy` — mutable, with `get*`/`set*` and public properties, and a `build()` that produces the immutable one |
| `ops.php` | `Compat\{ListOp,MapOp,HllOp}` and `Aerospike\BitwiseOp` — the 1.x argument order, translating to the fluent form |
| `collections.php` | `Compat\Recordset` (a wrapper, not an alias — see below), `Value`, `Context`, `UserRole` |
| `client.php` | `Compat\Client` — `connect()`, `socket()`, `createIndex()`, `dropUdf()`, `udfExecute()`, `batch()` returning 1.x `BatchRecord`s, and policy unwrapping on every verb |

## Four things it deliberately does not pretend

**`Recordset` cannot be made to work, and fails loudly instead.** The 1.x
`Recordset::next()` *returns* the next record; this client's `RecordSet` implements
PHP's `Iterator`, whose `next()` returns void. And because the 1.x name is
case-insensitively the extension's own, no PHP file can intercept it. So old code
calling `next()` on a native `RecordSet` gets `null` and iterates **zero times** —
silently. To make that loud, the wrapper lives at `Aerospike\Compat\Recordset` and
`Compat\Client::scan()` returns it: an `Aerospike\Recordset` type hint then raises a
`TypeError` naming both classes. Fix it with `foreach`, which was always fine, or
with `nextRecord()` on the native class.

**A locally built `Key` has no digest.** The 1.x client computed the digest in the
extension; this one computes it in the daemon, so `$key->getDigest()` is `null`
until the key has been to the server and back. A key from a scan has it. There is
no way around this short of duplicating RIPEMD-160 in the extension.

**Policies are not the same objects.** `Compat\WritePolicy` is a *builder*. If you
hand one to the real `Aerospike\Client` you get a `TypeError`; hand it to
`Compat\Client`, which unwraps it, or call `->build()` yourself. This is deliberate:
a silent conversion would hide the one thing worth knowing during a migration.

**`PartitionStatus` no longer resumes a scan.** The traversal's position lives in
the daemon, as a cursor the `RecordSet` holds a handle to, because a PHP request
does not outlive a scan and the daemon does. The class still answers its three
questions, but a partly consumed scan is resumed by keeping the `RecordSet`, not by
carrying per-partition state through PHP and back.

## Migration order that works

1. Load `bootstrap.php`. Run the test suite. Everything that was tier 1 now passes.
2. Replace `new Aerospike\Client(...)`/`Client::connect(...)` with
   `Aerospike\Compat\Client::connect(...)`.
3. For each remaining failure, change the `use` line to `Aerospike\Compat\…`.
4. Then, at leisure, delete the `Compat\` imports one file at a time and move to
   the native API. Nothing forces step 4.

## Tests

```sh
php -d extension=../ext/target/release/libaerospike_php.dylib tests/compat.php
```

Needs the daemon running. Every assertion is written in the **old** idiom on
purpose: a test written in the new one would pass without the layer loaded at all.
97 checks.
