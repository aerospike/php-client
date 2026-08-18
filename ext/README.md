# Aerospike PHP extension

A PHP extension that talks to Aerospike through `aerospike-php-daemon` over
shared memory. The extension itself contains **no database client**: it
translates PHP values into the wire contract in `../ipc` and hands them to the
local daemon, which owns the real client, the cluster state and the
connections.

That split is what makes the extension viable under PHP's process model.
Nothing here has to survive a request, warm a connection pool, or discover a
cluster.

The single-record verbs are all here: `ping`, `put`, `get`, `delete`, `touch`,
`exists`, `add`, `append` and `prepend`.

```php
use Aerospike\{Client, Key, Bin, Bins, WritePolicy, Expiration, RecordExistsAction};

$client = new Client();            // the "default" instance
$client = new Client("analytics"); // a named instance

$key = new Key("test", "users", "alice");

$client->put(null, $key, [new Bin("name", "Alice"), new Bin("age", 30)]);

$record = $client->get(null, $key);
$record->bins();        // ["age" => 30, "name" => "Alice"]
$record->bin("name");   // "Alice"
$record->generation();  // 1
$record->ttl();         // 2592000, or null when the record never expires
                        // get() itself returns null when the record is absent

$client->add(null, $key, [new Bin("age", 1)]);         // 30 -> 31
$client->append(null, $key, [new Bin("name", " B.")]); // "Alice B."
$client->touch(new WritePolicy(expiration: Expiration::seconds(3600)), $key);
$client->exists(null, $key);                            // true
$client->delete(null, $key);                            // true, then false

$client->ping()->version();    // "3.0.0-alpha.1"
$client->ping()->instances();  // ["default"]
```

| Method | Returns | An absent record |
| --- | --- | --- |
| `put(?WritePolicy $policy, Key $key, array $bins): void` | — | created |
| `get(?ReadPolicy $policy, Key $key, ?Bins $bins = null): ?Record` | `Record` or `null` | `null` |
| `delete(?WritePolicy $policy, Key $key): bool` | did it exist? | `false` |
| `touch(?WritePolicy $policy, Key $key): void` | — | **throws** |
| `exists(?ReadPolicy $policy, Key $key): bool` | — | `false` |
| `add(?WritePolicy $policy, Key $key, array $bins): void` | — | created |
| `append(?WritePolicy $policy, Key $key, array $bins): void` | — | created |
| `prepend(?WritePolicy $policy, Key $key, array $bins): void` | — | created |
| `operate(?WritePolicy $policy, Key $key, array $ops): ?Record` | `Record` or `null` | depends on the operations |
| `batch(?ReadPolicy $policy, array $rows): BatchResult[]` | one result per row | a row failure, code 2 |
| `ping(): DaemonInfo` | — | — |

`$bins` is a list of `Aerospike\Bin`; `$ops` is a list of
`Aerospike\Operation` — see [Operations](#operations).

`delete()` and `exists()` treat a missing record as an answer, not a failure —
the end state is the one that was asked for. `touch()` throws, because there is
no record whose life could be extended and a silent no-op would suggest
otherwise.

## The shape of the API

Two decisions run through all of it.

**Everything the Rust client models as a type is a class or an enum here.** A
`Key`, a `Bin`, a `Bins` selection, a `Record`, a `ReadPolicy`, a `WritePolicy`, a
`QueryPolicy`, an `AdminPolicy`, a `TxnVerifyPolicy`, a `TxnRollPolicy`, a
`Statement`, a `Filter`, a `PartitionFilter`, a `Task`, a `UdfModule`, a `Node`, a
`Transaction`, a `User`, a `Role`, a `Privilege`, an `Expiration`, an `Expression`
and the seven builders that make one (`Exp`, `ExpList`, `ExpMap`, `ExpBit`,
`ExpHll`, `ExpStr`, `ExpPath`); `Replica`, `ReadModeAP`, `ReadModeSC`, `ExpType`,
`StringNumericType`, `LoopVarPart`,
`RecordExistsAction`, `GenerationPolicy`, `CommitLevel`, `CollectionIndex`,
`IndexType`, `TaskStatus`, `UdfLanguage`, `TxnState`, `CommitStatus`,
`AbortStatus`, `PrivilegeCode` and `Status` are real PHP `enum`s. There are no
associative arrays in the API surface. This is not symmetry for its own sake — it
is what moves a mistake from the daemon's reply to the call site:

```php
$client->put(new ReadPolicy(), $key, $bins);
// TypeError: $policy must be of type ?Aerospike\WritePolicy, Aerospike\ReadPolicy given

new WritePolicy(durableDeletes: true);
// Error: Unknown named parameter $durableDeletes

new WritePolicy(replica: 'MASTER');
// TypeError: $replica must be of type ?Aerospike\Replica, string given
```

**Arguments are in `aerospike-core`'s order**: policy first, then the key, then
whatever the verb operates on. `put($policy, $key, $bins)` is the Rust client's
`put(&policy, &key, &bins)`, so code and examples move between the two without
being rearranged. The policy is *nullable* rather than *optional* — it holds the
first position whether or not you have one — and `null` means "use the daemon's
configured settings", which is what `&WritePolicy::default()` means in Rust and
what `null` means in the Java client.

### A note on how the types are enforced

PHP does not verify argument types for *internal* functions: the declared types
are real, and reflection and IDEs see them, but the engine leaves the checking to
the extension. ext-php-rs checks a non-nullable parameter, and for a **nullable**
one it structurally cannot — "absent" and "present but wrong" reach it as the
same value, so a wrong type silently becomes "not set".

Silently, that is, unless something restores the distinction. `src/arg.rs` does:
every optional typed argument is read through a wrapper whose conversion cannot
fail, so a wrong type survives to be reported as a `TypeError` worded like the
engine's own. Without it, `new WritePolicy(replica: 'MASTER')` would be accepted
and quietly do nothing — worse than the associative array this API replaced,
which at least refused an unknown key.

## Prerequisites

- **PHP 8.1 or later**, with `php-config` on `PATH`. Developed and verified
  against **PHP 8.5.9 NTS** (Homebrew). ext-php-rs 0.15.15 supports PHP 8.5.
- **Rust 1.87 or later.**
- A C toolchain and `libclang`, which ext-php-rs' `bindgen` needs to read the
  Zend headers. On macOS the Xcode command line tools are enough; on Debian or
  Ubuntu, `apt install libclang-dev php-dev`.
- **`aerospike-php-daemon` running**, configured with a `[cluster.<instance>]`
  section for the instance you use. Every method throws
  `Aerospike\AerospikeException` if the daemon is not there.

To build against a specific PHP, point `PATH` at that installation's
`php-config` — for example `PATH="$(brew --prefix php@8.3)/bin:$PATH"`.

## Building

This crate is **deliberately excluded from the repository's Cargo
workspace** (see `exclude` in the root `Cargo.toml`) because it links the Zend
API. A plain `cargo build` at the repository root must not require a PHP
toolchain, so this directory carries its own `Cargo.toml` and `Cargo.lock` and
is built from here:

```sh
cd aerospike-php/ext
cargo build --release
```

The extension lands in `target/release`:

| Platform | File |
| --- | --- |
| macOS | `target/release/libaerospike_php.dylib` |
| Linux | `target/release/libaerospike_php.so` |
| Windows | `target/release/aerospike_php.dll` |

`.cargo/config.toml` passes `-undefined dynamic_lookup` on macOS. A PHP
extension resolves every Zend symbol from the already-loaded PHP binary at
`dlopen` time; Apple's linker will not allow that by default, and without those
flags the build fails with a long list of undefined `_zend_*` symbols. This is
the standard ext-php-rs macOS setup, not a workaround for anything here.

## Installing

Add the built library to your `php.ini`:

```ini
; macOS
extension=/path/to/aerospike-php/ext/target/release/libaerospike_php.dylib
; Linux
extension=/path/to/aerospike-php/ext/target/release/libaerospike_php.so
```

Alternatively copy it into PHP's extension directory (`php-config
--extension-dir`) and use the bare filename. Confirm it loaded with:

```sh
php -m | grep aerospike
```

## Settings

All are read once, when a `Aerospike\Client` is constructed — so an
`ini_set()` earlier in the same request is honoured, but changing a value
after a client exists does not affect that client.

| Setting | Default | Meaning |
| --- | --- | --- |
| `aerospike.instance` | `default` | Instance used when the constructor is given none. |
| `aerospike.timeout_ms` | `1000` | How long a call waits for the daemon's reply. Clamped to 1..600000. |
| `aerospike.spin_iters` | `50000` | Iterations to spin before yielding while waiting. |
| `aerospike.yield_iters` | `100` | Iterations to yield before sleeping. |
| `aerospike.initial_sleep_us` | `10` | First sleep once yielding is exhausted. |
| `aerospike.max_sleep_us` | `50` | Ceiling the sleep grows to. |

The four spin/sleep knobs map onto the contract's `Backoff`, whose own
`Default` supplies the values above, so this table cannot drift from the
contract. They govern the `shm-poll` build, where a waiting worker polls shared
memory; in the default build it blocks on an event instead and the knobs only
bound how a wakeup is confirmed. `new Aerospike\Client("name")` overrides
`aerospike.instance`.

These tune **this side** of the hop. The daemon's own serving loop has its own
ladder, `[daemon.receive-backoff]` in its configuration file, and they are
deliberately separate: a worker waits for one reply and can block on an event,
while iceoryx2 request/response ports cannot be attached to a `WaitSet`, so the
daemon has no choice but to poll whichever build the workers use.

One daemon setting is worth knowing from this side: **`max-workers` (default 64)
is a hard ceiling on attached workers**, fixed when the shared-memory service is
created. A PHP-FPM pool larger than it leaves the extra workers unable to attach
at all, and the exception they raise names the setting, says the ceiling cannot
grow while the daemon runs, and says the restart only takes effect once no worker
is still attached to the old service.

**It does not need padding for deaths.** A worker killed outright — the OOM
killer, `kill -9`, FPM terminating one that overran — runs no destructors, but
its slot is reclaimed when the next worker starts, so a pool sized exactly at
`max-workers` stays at full strength across hard kills. `tests/concurrency.php`
fills a pool to its ceiling, kills a worker with `SIGKILL`, and asserts the
replacement attaches.

The one shape where that reclamation does *not* happen is a process that has
**itself attached** and then forks workers: a hard-killed sibling's slot stays
occupied for its replacement. PHP-FPM never does this — `MINIT` deliberately does
not attach, so the master holds no ports — but a CLI script that uses the client
and then forks a worker pool can. Fork first and let each child attach.

## Features

- `shm-poll` — forwards to `aerospike-php-ipc/shm-poll`. A waiting worker polls
  its response port in shared memory instead of blocking on an iceoryx2 event,
  so nothing but shared memory is involved: measured, ~10µs less latency for
  ~17× the CPU in the waiting process. Right for low worker concurrency or a
  hard shared-memory-only requirement, wrong for a busy FPM pool. **The daemon
  must be built with the matching feature.**

  The default build blocks on a per-worker event port, which under
  `ipc::Service` is a unix datagram socket — so the *wakeup* path leaves shared
  memory, while every payload still travels through it.

## Values

PHP has one array type — an ordered hash map — where Aerospike has both lists
and maps, so writing an array applies the conventional rule, the same one
`json_encode` uses:

- keys exactly `0, 1, .., n-1` **in that order** ⇒ an Aerospike **list**
- anything else — string keys, gaps, a different order ⇒ an Aerospike **map**
- the empty array ⇒ a list
- nested arrays recurse; map keys may be ints or strings

`null`, `bool`, `int`, `float` and `string` map to the matching Aerospike
types, and writing `null` to a bin **deletes** it. A PHP string is a byte string
with no way to say whether it was meant as text or as binary, so **every bare
PHP string is written as a text value** — and one that is not valid UTF-8 is
refused rather than corrupted. Three small classes cover the shapes PHP has no
literal for:

```php
new Aerospike\Blob($bytes);       // binary, not text
new Aerospike\GeoJson($document); // geometry the server indexes and queries
new Aerospike\Hll($sketch);       // an opaque HyperLogLog sketch
```

Each wraps a payload and exposes it again — `bytes()` on `Blob` and `Hll`,
`json()` on `GeoJson`, plus `__toString()` — and nothing else. A record key may
be an int, a string, or an `Aerospike\Blob` for a binary key; nothing else, since
a float has no ordering the server defines and an array has no digest.

### The two map kinds an array cannot express

A PHP array is an unordered Aerospike map, because that is all a PHP array can
say. `Aerospike\SortedMap` and `Aerospike\OrderedMap` say the other two, and are
accepted anywhere a map is — a bin value, nested inside a list or another map,
an operation argument, or the items of `MapOp::putItems`.

```php
$scores = new Aerospike\SortedMap(['bob' => 20, 'alice' => 10]);
$client->put(null, $key, [new Aerospike\Bin('scores', $scores)]);

$stored = $client->get(null, $key)->bin('scores');   // a SortedMap
foreach ($stored as $name => $score) { /* the server's key order */ }
$stored->get('alice');   // 10
$stored['alice'];        // the same, through ArrayAccess
count($stored);          // 2
$stored->toArray();      // ['alice' => 10, 'bob' => 20]
```

Both implement `Iterator`, `ArrayAccess` and `Countable`, and carry
`set`, `get`, `has`, `remove`, `keys`, `values`, `toArray`, `count` and
`isEmpty`. `set` on a key that is already there **replaces it in place**:
changing a value is not a reordering.

Two things to know, and they are the reasons these classes exist:

- **A key-ordered map reads back as a `SortedMap`**, not as an array. Key order
  is real storage — it is what makes the rank, index and key-range operations
  run in log time — and an array written back would be stored *unordered*, with
  nothing said. The same argument as for `Blob`, and the same answer. An
  `OrderedMap` cannot round trip: the server has no insertion-ordered map, so it
  is written unordered and reads back as a plain array.
- **A map key can be something a PHP array key cannot hold** — a blob, a float,
  a boolean. `foreach` hands those back as they are, because `Iterator::key()`
  is not restricted the way an array key is. `toArray()` *refuses* such a map
  rather than flattening it: you are holding the faithful thing and would be
  throwing it away.

Neither class sorts locally. The server is the authority on how values order —
its rule spans every type, `nil < bool < int < string < list < map < blob <
float < GeoJSON` — and a second implementation of that here could only disagree
with it. So a `SortedMap` you build iterates in the order you built it, and one
the server sent iterates in the server's order, which is the one that matters.

A `Bin` converts its value **when it is constructed**, not when the command runs,
so a value Aerospike cannot store is reported against the bin that holds it
rather than at some later call — and building your bins once, outside a loop,
costs nothing per operation.

Reading back, those come back **as their wrapper class** rather than as a bare
string. That is deliberate: handing back a PHP string would round trip once and
then silently rewrite the blob as text on the next write, which is data loss
that surfaces long after the code that caused it.

| Aerospike value | PHP |
| --- | --- |
| nil | `null` |
| bool, int, double, string | `bool`, `int`, `float`, `string` |
| blob | `Aerospike\Blob` |
| GeoJSON | `Aerospike\GeoJson` |
| HyperLogLog | `Aerospike\Hll` |
| list | packed array |
| map | associative array |
| key-ordered map | `Aerospike\SortedMap` |
| several results for one bin name | packed array, in operation order |
| a map read that returned keys *and* values | list of `[key, value]` pairs |
| a particle type this build cannot decode | `["particle_type" => int, "data" => Aerospike\Blob]` |

The last row is the point of the contract's `Unknown` variant: a newer server's
data type degrades to something a caller can inspect and log, instead of failing
the whole read.

`Aerospike\Infinity` and `Aerospike\Wildcard` exist for CDT range selections in
a later phase. They are classes rather than constants because a PHP class
constant can only hold a scalar, and any scalar sentinel is a value somebody
will one day store for real — at which point their range selection quietly
changes meaning.

A **list round trips in order** — its order is data. A **map comes back in the
server's key order**, not in PHP insertion order, because Aerospike has no
insertion-ordered map type at all: an associative array is stored key-ordered and
read back that way. So `["theme" => "dark", "lang" => "en"]` reads back as
`["lang" => "en", "theme" => "dark"]`. Do not depend on the insertion order of a
map you wrote, in either direction.

Anything PHP can hold but Aerospike cannot — an object of some other class, a
resource, a string that is not valid UTF-8 — is rejected with an exception
naming the bin and the position inside it, for example:

```text
bin "profile"["tags"][2] is an instance of stdClass, which Aerospike cannot store.
```

## Batch

A batch is **not** many gets. Its rows may each be a read, a write, a delete or
a UDF call, against any key in any namespace, and the client routes them to the
nodes that own them — one round trip *per node* instead of one per record.

```php
use Aerospike\{BatchRead, BatchWrite, BatchDelete, BatchUdf};

$results = $client->batch(null, [
    BatchRead::all($alice),
    BatchRead::some($bob, ['name']),
    BatchRead::header($carol),                     // does it exist?
    BatchRead::ops($dave, [ListOp::size('items')]), // collection reads
    BatchWrite::ops($erin, [Op::add(new Bin('hits', 1))]),
    BatchDelete::key($frank),
    BatchUdf::call($grace, 'example', 'touch'),
]);
```

**A failed row is not a failed batch.** `batch()` returns one
`Aerospike\BatchResult` per row, in the order the rows were given, and a row's
failure belongs to that row — so **check `isOk()` per row** rather than relying
on an exception. Only a failure that stops the batch being sent at all throws.

```php
foreach ($results as $result) {
    if (!$result->isOk()) {
        $result->resultCode();   // 2 missing, 27 filtered out, 5 create-only…
        $result->isInDoubt();    // only ever true for a write row
        continue;
    }
    $result->record()?->bin('name');
}
```

**A missing record is a row failure with result code 2**, not the `null` that
`get()` returns. That difference is the server's, and it is the one thing about
batch worth remembering.

Every row kind takes its own per-row settings — a `filter` on any of them, and
`recordExistsAction`, `generationPolicy`, `expiration`, `commitLevel`, `sendKey`
and `durableDelete` where they apply. The `$policy` argument to `batch()` is a
`ReadPolicy` carrying only what every row *shares*: timeouts, retries, replica
choice, a filter applied to all of them.

## Scans and queries

One method, `query()`, as in `aerospike-core`. A `Statement` with no `Filter`
visits every record of a set — a scan — and one with a filter uses the secondary
index the filter names. Two methods would suggest two mechanisms; there is one.

```php
use Aerospike\{Statement, Filter, PartitionFilter, QueryPolicy, CollectionIndex};

// A scan.
foreach ($client->query(null, null, new Statement('test', 'users')) as $record) {
    echo $record->key()?->userKey(), ' ', $record->bin('name'), "\n";
}

// A query, with a page size, a ceiling and a rate limit.
$statement = new Statement('test', 'users', filter: Filter::range('age', 20, 30));
$policy    = new QueryPolicy(maxRecords: 10_000, pageSize: 500, recordsPerSecond: 1_000);
foreach ($client->query($policy, null, $statement) as $record) { … }
```

`Filter` has one static method per thing that can be compared — `equal`, `range`,
`geoWithinRegion`, `geoWithinRadius`, `geoContains` — each with a `…ByIndex`
form for naming an index rather than a bin, and each taking an optional
`CollectionIndex` for an index built over list elements or map keys or values.
`->context([…])` reaches inside a nested collection and `->expression(…)` names an
expression-based index.

**A query needs the index to exist.** A filter on a bin with no index is the
server's result code 201 (`INDEX_NOTFOUND`), never an empty result: a query that
quietly became a full-set scan would be far worse than one that failed.

### Pages, not a stream

`query()` returns an `Aerospike\RecordSet`, an `Iterator`. Each page is one round
trip; `pageSize` sets how many records that is, and iterating past the end of a
page fetches the next one. `foreach` hides all of it.

Nothing streams, and that is deliberate: iceoryx2 answers one request with many
responses by overwriting the oldest once the buffer fills, and **tells the sender
nothing** — measured, 50 sends produced 50 `Ok`s and 16 arrivals. A page plus a
cursor is the only shape that cannot lose records silently.

Two things follow from a `RecordSet` being a *position* rather than a collection:

```php
$records = $client->query(null, null, $statement);
foreach ($records as $record) { … }
foreach ($records as $record) { … }   // throws: a RecordSet is read once

foreach ($client->query(null, null, $statement) as $record) {
    if (enough($record)) { break; }   // fine: the cursor is released on drop
}
```

`close()` releases the daemon's cursor explicitly, and dropping the object does it
too — so abandoning a scan costs nothing. One abandoned in a way that reaches
neither (a killed worker) expires on the daemon instead.

### Which record is this?

A scanned record carries its own identity, which a single-record read does not
need to:

```php
$record->digest();          // Aerospike\Blob, 20 bytes — always present
$record->key();             // ?Aerospike\Key — only if the write set sendKey
```

The digest is what every scanned record has. The **user key comes back only if the
write that created the record stored it** (`new WritePolicy(sendKey: true)`) — a
digest cannot be reversed into a key. That is the server's rule; a scan that needs
keys has to have been written for it.

`includeBinData: false` returns identity and metadata with no bins, which is how a
set is counted or its digests collected without moving its data.

### Dividing one scan between workers

`PartitionFilter` narrows a traversal to part of the ring, which is how a scan is
parallelised without any coordination: give each worker a range and no record is
seen twice.

```php
PartitionFilter::byRange(0, 1024);   // a quarter of the 4096 partitions
PartitionFilter::byId(7);
PartitionFilter::after($key);        // records after a key's digest, that partition only
```

## Users, roles and privileges

Fourteen verbs, all of which need **`security { enable-security true }`** on the
cluster. Without it every one fails with result code **52**, `SecurityNotEnabled` —
not gated client-side, because it is a configuration the operator chose and the
server names it exactly.

```php
use Aerospike\{Privilege, PrivilegeCode};

$client->createRole(null, 'auditor', [
    new Privilege(PrivilegeCode::Read, 'test'),          // one namespace
    new Privilege(PrivilegeCode::Read, 'test', 'users'), // one set
], allowlist: ['10.0.0.0/8'], readQuota: 1000, writeQuota: 0);

$client->createUser(null, 'alice', 'secret', ['auditor']);
$client->grantRoles(null, 'alice', ['read-write']);

foreach ($client->queryUsers(null) as $user) {
    echo $user->name(), ': ', implode(', ', $user->roles()), "\n";
}
```

The rest: `createPkiUser`, `dropUser`, `changePassword`, `revokeRoles`, `dropRole`,
`queryRoles`, `grantPrivileges`, `revokePrivileges`, `setAllowlist`, `setQuotas`.

### The privilege scope rule

`Aerospike\PrivilegeCode` has thirteen cases, and they split in two:

- **Cluster-wide**: `UserAdmin`, `SysAdmin`, `DataAdmin`, `UdfAdmin`, `SIndexAdmin`,
  `MaskingAdmin`. These act on the cluster, so **they cannot be confined to a
  namespace** — `new Privilege(PrivilegeCode::SysAdmin, 'test')` throws. The server
  refuses it too, with a parameter error that names neither the privilege nor the
  reason, which is why this is checked where the privilege is written.
- **Scopable**: `Read`, `ReadWrite`, `ReadWriteUdf`, `Write`, `Truncate`,
  `ReadMasked`, `WriteMasked`. Any of these may be confined to a namespace, or to a
  set within one. A set **without** a namespace is refused: a set scope is *within* a
  namespace.

### Empty means something, in both directions

Most of this API refuses an empty list. Two places do not, and the difference is
load-bearing:

| | empty means |
| --- | --- |
| `createRole`'s `$privileges` | **refused** — a role with no privileges permits nothing |
| `grantRoles`/`grantPrivileges` | **refused** — nothing to grant |
| `setAllowlist`'s `$allowlist` | **clears the restriction** — this is how "anywhere" is said |
| `setQuotas`' `0` | **lifts the quota** — this is how "unlimited" is said |

Reading back, an unlimited quota comes through as `null` rather than `0`: zero is the
server's encoding, and a caller doing arithmetic on it would conclude the opposite.

### What a user reports

`readInfo()` and `writeInfo()` are the server's own statistic lists, in its order —
quota, single-record rate, scan/query record rate, count of limitless scans. Lists
rather than named accessors because a future server release may append to them, and
they may be empty when the server reported none.

### PKI users

`createPkiUser()` makes a user the client certificate identifies, with no password.
Connecting as one needs `auth = "PKI"` and a `[cluster.<name>.tls]` section giving
both a `cert-file` and a `key-file` — the server takes the identity *from* the
certificate, so TLS on its own is not enough, and the daemon refuses that
combination at startup rather than failing later.

## Transactions

A multi-record transaction spans several commands against several records, and
either all of its writes land or none do.

```php
$txn = $client->beginTransaction();
try {
    $client->put(new WritePolicy(txn: $txn), $from, [new Bin('balance', 70)]);
    $client->put(new WritePolicy(txn: $txn), $to,   [new Bin('balance', 30)]);
    $txn->commit();
} catch (Throwable $e) {
    $txn->abort();
    throw $e;
}
```

Commands join by carrying the transaction **on their policy**, which is where
`aerospike-core` puts it too (`base_policy.txn`) — so every single-record verb and
`batch()` can be transactional with no separate API.

**Requires server 8.0+ and a strong-consistency namespace.** The first is checked
when the transaction opens and refused with the offending node's version; the
second is the server's to enforce and shows up as a failure on the first write. A
development namespace usually is *not* SC, which is the most common reason a
transaction that looks right fails.

### An unfinished transaction rolls back

`Aerospike\Transaction` aborts itself when it is destroyed, so the `catch` above is
belt-and-braces: `break`, `return`, an uncaught throw, or the request simply ending
all release the transaction's record locks.

That matters more here than for a scan cursor. An open transaction holds locks on
everything it has written, so every other writer of those records waits behind an
abandoned one. Three things bound it:

1. the object's destructor, immediately;
2. the daemon's idle sweep, which **aborts** rather than forgets (`txn_idle_timeout`,
   30s by default);
3. the server's own transaction timeout, if the daemon dies too — settable with
   `beginTransaction($timeoutMs)`.

### Reads matter as much as writes

A commit **verifies** that every record the transaction read is still at the
version it saw, and fails the whole transaction if any has changed. That is what
makes it a transaction rather than a batch — and it means a transaction that only
reads is still doing something.

### Statuses, and the two that are not errors

Every `CommitStatus` and `AbortStatus` is a success. `Ok` means everything was
tidied up; the others mean the writes landed (or were rolled back) and the server
was left some bookkeeping. **None of them is a reason to try again.**

- Committing twice returns `AlreadyCommitted`, so a `finally` block can commit
  without checking. Committing one that was *aborted* throws — answering "already
  committed" for writes that are gone would be the worst available lie.
- `Status::TxnExpired` on a command means the transaction is no longer open: it was
  committed, aborted, or aborted for you after sitting idle. The writes are not
  applied and the work has to start again. A command that fails *inside* a
  still-open transaction is an ordinary failure, and that transaction can still be
  aborted.

### Finishing one with explicit policies

A commit is **two** batch commands — verify the version of every record the
transaction read, then roll its writes forward — so it takes two policies, and an
abort takes one because it has nothing to verify:

```php
$txn->commitWithPolicies(
    new Aerospike\TxnVerifyPolicy(totalTimeoutMs: 30_000, maxRetries: 8),
    new Aerospike\TxnRollPolicy(totalTimeoutMs: 30_000, maxRetries: 8),
);

$txn->abortWithPolicy(new Aerospike\TxnRollPolicy(maxRetries: 10));
```

`null` for either takes the client's own default, which is what `commit()` and
`abort()` pass. **Those defaults are tuned, and raising them is the safe
direction:** linearized SC reads for the verify, the partition master for both, 5
retries, a 3s socket and 10s total timeout, a 1s pause between retries. Unlike a
`ReadPolicy`, they do *not* inherit the daemon's `default_timeout` — that is sized
for a single-record command, and a commit that gives up leaves a transaction
half-finished with its locks still held.

Both classes carry the eight shared fields and no others. There is deliberately no
`filter` (this batch reads or writes the records the *transaction* chose, and
skipping one would mean not verifying or not rolling it), no `txn` (the transaction
being finished is the one whose method is being called), and no write fields on the
roll policy — it moves writes the transaction already made, so a TTL or a
create/replace guard would have nothing to apply to.

### Not for scans or queries

A transaction covers records named by key, and a traversal names none. So
`QueryPolicy` has **no `txn` parameter at all** — the type system refuses first —
and the daemon refuses one reaching it another way rather than silently running the
query outside the transaction. Read the keys with a query, then read or write them
by key inside the transaction.

## Managing the cluster

Secondary indexes, UDF modules, truncation, info and node listing. Every one of
these takes an `Aerospike\AdminPolicy` in first position — which carries a single
field, the socket timeout, because that is all `aerospike-core`'s own
`AdminPolicy` has.

### Long-running commands return a Task, and the waiting is yours

Registering a UDF or creating an index returns as soon as **one** node has
accepted it. The work then propagates, and an index over a large set takes
minutes. So those commands return an `Aerospike\Task`:

```php
$client->createIndexOnBin(null, 'test', 'users', 'age', 'age_idx', Aerospike\IndexType::Numeric)
       ->waitTillComplete(30_000);            // up to 30s, polling every second

$task = $client->registerUdf(null, file_get_contents('example.lua'), 'example.lua');
$task->status();                              // TaskStatus::InProgress | Complete | NotFound
```

**The polling happens in this process, not in the daemon.** A blocking wait served
by the daemon would be a request held open for minutes by a worker whose reply
deadline is measured in seconds; polling puts the timeout where the caller sets
it, and every individual request stays short.

**A task is a description, not a registration** — a namespace and an index name, a
package name, a task id. Nothing is stored in the daemon, so a task survives the
daemon restarting and the worker that made it exiting.

Two things the server's answers cannot tell you, both worth knowing before
branching on a status:

- **A background job that finished and one that never existed both report
  `Complete`.** The server tracks *running* jobs, so "nobody is running it" is all
  it can say.
- **An index that does not exist throws** rather than reporting `NotFound` —
  result code 201, `no index`, which is more useful than "not yet".

### Secondary indexes

```php
// Over a bin, over a list's elements, over something nested.
$client->createIndexOnBin(null, 'test', 'users', 'age', 'age_idx', IndexType::Numeric);
$client->createIndexOnBin(null, 'test', 'users', 'tags', 'tags_idx', IndexType::Text,
                          CollectionIndex::ListElements);
$client->createIndexOnBin(null, 'test', 'users', 'meta', 'inner_idx', IndexType::Numeric,
                          CollectionIndex::MapValues, [Ctx::mapKey('scores')]);

// Over an expression — a query must then name the index, not a bin.
$client->createIndexUsingExpression(null, 'test', 'orders', 'total_idx',
    IndexType::Numeric, null, Expression::ael('$.price:INT * $.quantity:INT'));

$client->dropIndex(null, 'test', 'users', 'age_idx');
```

`IndexType::Text` is the string index: `String` is a reserved type name in PHP, so
that one case reads differently from the Rust variant. **The index type has to
match the bin's contents** — a numeric index over a string bin is not an error, it
just indexes nothing, and the query finds no records.

Until the build finishes, **a query using the index returns incomplete results
rather than an error**. Wait on the task before relying on it.

### User-defined functions

```php
$client->registerUdf(null, file_get_contents('example.lua'), 'example.lua')
       ->waitTillComplete(10_000);

foreach ($client->listUdf(null) as $module) {
    echo $module->name(), ' ', $module->hash(), ' ', $module->language(), "\n";
}

// One record. Note: the package name drops the `.lua`.
$result = $client->executeUdf(null, $key, 'example', 'bump', ['views', 1]);

// Every record a statement matches, in the background.
$client->queryExecuteUdf(null, new Statement('test', 'users'), 'example', 'bump', ['views', 1])
       ->waitTillComplete(60_000);

$client->removeUdf(null, 'example.lua');
```

Three things that catch people:

- **`$source` is the module's text, not a path.** The daemon may not share a
  filesystem with this worker, so a path resolved on its side could register a
  different file. Read the file here.
- **The package name drops the extension in a call but not in a registration** —
  `example.lua` to register, `example` to call. That asymmetry is the server's;
  getting it wrong is a result-code-100 failure.
- **`executeUdf()` is a write** whatever the function does, because the server
  takes a write lock on the record before it can know what the Lua will do. It is
  not a cheap way to read.

A UDF that raises an error arrives as an `AerospikeException` with
`getStatus() === Status::Server` and result code 100 — not as a success with a
`null`. A background job's *per-record* failures, though, are reported nowhere: the
task says only whether the job finished. For work that has to account for each
record, use `batch()`.

`listUdf()` is the one method here with no counterpart in `aerospike-core` — it has
no `list_udf`, so the daemon issues `udf-list` and parses the server's record
format once, rather than leaving every caller to.

### Truncate, info and nodes

```php
$client->truncate(null, 'test', 'users', null);       // every record of the set
$client->truncate(null, 'test', '', null);            // the whole namespace
$client->truncate(null, 'test', 'users', $nanos);     // only records older than $nanos

$info = $client->info(null, ['build', 'namespaces']);
$info['build'];                                       // "8.1.3.0"
$info = $client->info(null, ['statistics'], $node->name());   // per-node commands

foreach ($client->nodes() as $node) {
    echo $node->name(), '@', $node->address(), ' ', $node->version(),
         $node->isActive() ? '' : ' (inactive)', "\n";
}
```

`truncate()` returns immediately and reads stop seeing the records at once, even
though the server reclaims the space in the background. `$beforeNanos` is
**nanoseconds since the Unix epoch** — `time() * 1_000_000_000`, not `hrtime()`,
which is monotonic and unrelated. A cutoff ahead of the *server's* clock is
refused by the server, so "now" computed on the client is a race; use a cutoff you
know is in the past.

`info()` answers are keyed by command **in the order asked**. Most info commands
answer for the whole cluster whichever node is asked; `statistics` and `latencies`
are the ones worth naming a node for.

`nodes()` is the daemon's own view rather than a fresh query — where a command
would be routed right now. A node the daemon has stopped believing in is still
listed, with `isActive()` false, because a list that quietly omitted it would look
like a smaller cluster.

## Operations

`operate()` runs several operations against one record, **in the order given and
atomically** — nobody else's write can interleave with them. That is the whole
reason it exists rather than a sequence of calls.

Operations are built by static methods on a class per family, named exactly as
`aerospike-core`'s `operations::scalar` and `operations::lists` functions are:

```php
use Aerospike\{Op, ListOp, Ctx, ListReturn, ListOrder, ListPolicy};

$record = $client->operate(null, $key, [
    Op::add(new Aerospike\Bin('views', 1)),      // read-modify-write, atomically
    ListOp::append('history', $event),
    ListOp::getByIndexRange('history', -5, null, ListReturn::Values),
    Op::getBin('name'),
]);
```

| Family | Class | Methods |
| --- | --- | --- |
| scalar | `Op` | `get`, `getHeader`, `getBin`, `put`, `append`, `prepend`, `add`, `touch`, `delete` |
| list | `ListOp` | `create`, `setOrder`, `append`, `appendItems`, `insert`, `insertItems`, `pop`, `popRange`, `remove`, `removeRange`, `removeByValue`, `removeByValueList`, `removeByValueRange`, `removeByValueRelativeRankRange`, `removeByIndex`, `removeByIndexRange`, `removeByRank`, `removeByRankRange`, `set`, `trim`, `clear`, `increment`, `sort`, `size`, `get`, `getRange`, `getByValue`, `getByValueList`, `getByValueRange`, `getByValueRelativeRankRange`, `getByIndex`, `getByIndexRange`, `getByRank`, `getByRankRange` |
| map | `MapOp` | `create`, `setOrder`, `setPolicy`, `put`, `putItems`, `incrementValue`, `decrementValue`, `clear`, `size`, and `removeBy…`/`getBy…` for `Key`, `KeyList`, `KeyRange`, `KeyRelativeIndexRange`, `Value`, `ValueList`, `ValueRange`, `ValueRelativeRankRange`, `Index`, `IndexRange`, `Rank`, `RankRange` |
| bitwise | `BitOp` | `resize`, `insert`, `remove`, `set`, `or`, `xor`, `and`, `not`, `lshift`, `rshift`, `add`, `subtract`, `setInt`, `get`, `count`, `lscan`, `rscan`, `getInt` |
| HyperLogLog | `HllOp` | `init`, `add`, `setUnion`, `refreshCount`, `fold`, `getCount`, `getUnion`, `getUnionCount`, `getIntersectCount`, `getSimilarity`, `describe` |
| expression | `ExpOp` | `read`, `write` |

Four things to know:

- **Indexes and ranks may be negative**, counting from the end of the list and
  from the largest value. `ListOp::getByRank('items', -1, ListReturn::Values)`
  is the largest item.
- **A `null` count means "to the end".** Where the Rust client has a pair of
  functions — `get_range` and `get_range_from` — this has one method with a
  nullable `$count`, because that is what the pair means.
- **`$returnType` decides the shape of the answer**, and `inverted: true`
  selects everything the range did *not* name — which for a remove operation
  removes the rest. It defaults to `ListReturn::Values`: an operation whose
  result vanished silently is the more surprising outcome, and a caller who
  wants nothing back can say `ListReturn::None` and mean it.
- **Two results for one bin come back as a list**, in operation order, because
  one bin name cannot hold two answers. That is the server's own shape for it.

### Nesting

Any list operation can be aimed at a collection *inside* a bin by attaching a
path, exactly as the Rust client's `.context(..)` does. `Ctx` has a static
method per step: `listIndex`, `listIndexCreate`, `listRank`, `listValue`,
`mapIndex`, `mapRank`, `mapKey`, `mapKeyCreate`, `mapValue`.

```php
// The list at $record['profile']['roles']
ListOp::append('profile', 'admin')->context([Ctx::mapKey('roles')]);
```

`context()` returns a **new** operation — an operation is a value — and refuses
to attach a path to a scalar operation, which would silently run against the bin
instead.

### Maps have two halves

A map entry is a key *and* a value, so most map operations come in both
flavours — `getByKey` and `getByValue`, `removeByKeyRange` and
`removeByValueRange` — and `MapReturn` says which half comes back:

```php
MapOp::getByRank('scores', -1, MapReturn::Key);       // "bob"
MapOp::getByRank('scores', -1, MapReturn::Value);     // 20
MapOp::getByRank('scores', -1, MapReturn::KeyValue);  // [["bob", 20]]

// The whole map, as a map rather than as pairs
MapOp::getByKeyRange('scores', null, null, MapReturn::UnorderedMap);
```

It defaults to `MapReturn::Value`. `MapOp::putItems()` takes a PHP map of key to
value, and `incrementValue`/`decrementValue` land on an entry's value, creating
the entry if it is missing.

`MapPolicy` carries the order and a `MapWriteMode` — `Update`, `UpdateOnly` or
`CreateOnly` — plus `noFail` and `partial`. The same trap as the list flags
applies: `UpdateOnly` on a key that is not there is an error, and adding
`noFail` makes it *silently do nothing*.

The Rust client has both a write mode and a flag bitmask that replaces the mode
when non-zero; this exposes one mode plus the two flags that say something the
mode cannot, and the daemon combines them — so asking for `noFail` cannot
accidentally downgrade an `UpdateOnly` to a plain update.

**A key-ordered map is distinguishable after storage**, unlike an
insertion-ordered one: write it with `MapOrder::KeyOrdered` and it reads back in
key order.

### Expressions

`ExpOp` evaluates an expression **on the server**, against the record as it is
at that moment — inside the same atomic `operate()` as everything else in the
call. `write` stores the result in a bin; `read` just hands it back under a
label.

```php
$client->operate(null, $key, [
    ExpOp::write('total', Expression::ael('$.price:INT * $.quantity:INT')),
    ExpOp::read('doubled', Expression::ael('$.total:INT * 2')),
]);
```

**Annotate the bin types.** This is the one thing to know about expression
operations. In a filter, `$.age > 25` needs no annotation because the `> 25`
says what the bin is — but an expression operation has no comparison to infer
from, so a bare `$.price` is a **parameter error (result code 4)** and
`$.price:INT` is not. The types are `INT`, `STRING`, `FLOAT`, `BOOL`, `BLOB`,
`HLL`, `LIST`, `MAP` and `GEO`.

An `Aerospike\Expression` is built one of three ways, and the three are
interchangeable wherever an expression is *used* — a policy filter, an expression
operation, an expression-based index, a query filter:

```php
Exp::numAdd([Exp::intBin('a'), Exp::intVal(1)]);  // built here, packed by the client
Expression::ael('$.a:INT + 1');                   // source; the server compiles it
Expression::base64($packedElsewhere);             // an expression another client packed
```

**Prefer the builder.** AEL text is shorter to write, but it needs **server 8.1.3
or later** — the daemon checks every node and refuses by name rather than letting
an old server reject the text obscurely. A built expression is packed by the
client, so it works against every server this client supports, and a wrong operand
is a `TypeError` at the call site rather than a server error a round trip later. A
packed expression has no version requirement either, since nothing parses it.

Only a built expression can be an **operand** of another: the other two are already
whole expressions, and there is nowhere in the wire format to put text inside a
packed tree.

#### The builder

Six classes, mirroring `aerospike-core`'s `expressions` module:

| class | what it covers |
| --- | --- |
| `Exp` | values, bins, record metadata, comparison, logic, arithmetic, bitwise integers, `cond`/`let`/`def`/`var`, regex, geo, `inList` |
| `ExpList` | the 31 list expressions |
| `ExpMap` | the 37 map expressions |
| `ExpBit` | the 18 bitwise expressions |
| `ExpHll` | the 12 HyperLogLog expressions |
| `ExpStr` | the 42 string expressions (**server 8.1.3+**, gated by the daemon) |
| `ExpPath` | the 9 CDT path expressions — one operation over *every* node a path reaches (**server 8.1.1+**) |

```php
use Aerospike\{Exp, ExpList, ExpMap, ExpStr, ExpType, ListReturn, MapReturn};

// Active adults whose top score beats 900.
$filter = Exp::and([
    Exp::eq(Exp::stringBin('status'), Exp::stringVal('active')),
    Exp::gt(Exp::intBin('age'), Exp::intVal(21)),
    Exp::gt(
        ExpList::getByRank(ListReturn::Values, ExpType::Integer, Exp::intVal(-1),
            Exp::listBin('scores')),
        Exp::intVal(900),
    ),
]);
$record = $client->get(new Aerospike\ReadPolicy(filterExp: $filter), $key);
```

Lists of expressions are **arrays** — `Exp::and([$a, $b])` — the same shape
`operate()` takes for its operations. Every argument of a collection expression is
itself an expression, which is what lets a filter say "the element whose index is
in another bin"; attach a path into a nested collection with `->context([...])`,
exactly as an `Operation` does. Modify operations return the whole modified
collection, so they compose:

```php
// The size of "scores" with one more element, without writing anything.
ExpList::size(ExpList::append(null, Exp::intVal(1), Exp::listBin('scores')));
```

#### Path expressions, which fan out

Everything above addresses **one** node. A path expression addresses **many**,
because its path contains a fan-out step — `Ctx::allChildren()`, or
`Ctx::allChildrenWithFilter()` to visit only the children a filter accepts. "The
price of every book" is a path expression; "the price of the first book" is an
`ExpMap` read. **Needs server 8.1.1 or later**, since the fan-out is the server
walking the collection.

```php
use Aerospike\{Exp, ExpMap, ExpPath, ExpType, Ctx, LoopVarPart, MapReturn};

// The prices of every book over 20 — the filter runs per child, and the loop
// variable is the child being tested.
$dear = ExpPath::selectValues(ExpType::ListType, Exp::mapBin('books'), [
    Ctx::allChildrenWithFilter(Exp::gt(
        ExpMap::getByKey(MapReturn::Value, ExpType::Integer, Exp::stringVal('price'),
            Exp::loopVar(ExpType::MapType, LoopVarPart::Value)),
        Exp::intVal(20),
    )),
    Ctx::mapKey('price'),
]);
```

The path is an argument here rather than `->context()`: for a path expression the
path *is* the operation, so an empty one is refused. `selectValues`,
`selectMapKeys`, `selectMapEntries` and `selectMatchingTree` are `selectByPath`
with a `SelectFlag` filled in — the same server opcode — and `modify`/`modifyNoFail`
stand in the same relation to `modifyByPath` and `ModifyFlag`. A modify expression
produces each node's new value; return `Exp::removeResult()` from it to delete that
node instead, which is a conditional removal in one pass.

A policy takes either spelling: `filter:` for AEL text, `filterExp:` for a built or
packed one. **Naming both is refused** — they set the same field, and whichever won
would be invisible. Reading back, `filter()` is `null` for a built expression (a
tree never was text) and `filterExp()` returns it whatever the form.

`ExpOp::write` takes a `BinWriteMode` plus `allowDelete` (an expression that
evaluates to null deletes the bin instead of reporting that it did not apply),
`noFail` and `evalNoFail`.

### Bits, and the units trap

`BitOp` works on a **blob** bin — an `Aerospike\Blob`, not a string — treating it
as a flat field of bits.

```php
$client->put(null, $key, [new Bin('flags', new Blob("\x01\x02\x03\x04"))]);

$client->operate(null, $key, [
    BitOp::count('flags', 0, 32),          // 5 — the set bits
    BitOp::getInt('flags', 0, 8, false),   // 1 — the first byte as an integer
    BitOp::set('flags', 0, 8, new Blob("\xff")),
]);
```

**Watch the units.** `resize`, `insert` and `remove` work in whole **bytes**;
everything else works in **bits**. The parameter names say which — `$byteOffset`
against `$bitOffset` — because that is the mistake this API can least afford: a
byte offset used as a bit offset addresses the right blob at the wrong place and
*succeeds*.

`add` and `subtract` operate on the integer held in a bit field, and
`$overflow` says what happens when the result does not fit:
`BitOverflow::Fail` (the default), `Saturate` (clamp) or `Wrap`. Failing by
default is deliberate — a counter that silently wrapped or stuck is worse than
one that says it could not.

### HyperLogLog

A sketch answers "roughly how many **distinct** things have I seen" in a fixed
few kilobytes, however many things there were. Every count it returns is an
**estimate**; `$indexBitCount` buys accuracy with space.

```php
$client->operate(null, $key, [
    HllOp::add('visitors', [$userId], indexBitCount: 12),
    HllOp::getCount('visitors'),
]);

// Set arithmetic between sketches, without reading either set
$other = $client->get(null, $otherKey)->bin('visitors');   // an Aerospike\Hll
$client->operate(null, $key, [
    HllOp::getUnionCount('visitors', [$other]),
    HllOp::getIntersectCount('visitors', [$other]),
    HllOp::getSimilarity('visitors', [$other]),
]);
```

`add` reports how many *registers* it changed, not how many values were new —
a sketch cannot know that. The bit counts are nullable wherever the Rust client
has a family of constructors for them, and `null` means "leave it to the sketch
that is already there".

Both families share `BinPolicy` for their write rules: a `BinWriteMode`
(`Update`, `UpdateOnly`, `CreateOnly`) plus `noFail`, and then `partial` for
bitwise or `allowFold` for HyperLogLog. It is the one place this API collapses
two of the client's types into one, because they differ only in that third flag.

### List write flags

`ListPolicy` carries the order a created list gets and the write flags. The
flags interact in a way worth stating plainly, because only one of the three
outcomes is the one people expect:

```php
$items = [5, 1, 5, 3];   // note the duplicate

new ListPolicy(order: ListOrder::Ordered, addUnique: true);
// the duplicate makes the server refuse the operation: result code 26

new ListPolicy(order: ListOrder::Ordered, addUnique: true, noFail: true);
// no error — and the *whole batch* is discarded. Nothing is written.

new ListPolicy(order: ListOrder::Ordered, addUnique: true, noFail: true, partial: true);
// [1, 3, 5] — the acceptable items are kept
```

## Policies

`ReadPolicy` governs `get` and `exists`; `WritePolicy` governs `put`, `delete`,
`touch`, `add`, `append` and `prepend` — the same split `aerospike-core` makes,
and the reason a read cannot be handed a `recordExistsAction` at all.

Every field is optional, and an unset field means **do not override**: the
daemon's own configured default applies, including whatever the `[defaults]`
section of its configuration file sets. A policy object that filled its fields
with plausible defaults would silently defeat that file, so `null` is the initial
state of all of them. Build one with named arguments:

```php
$client->put(new WritePolicy(
    totalTimeoutMs:     500,
    maxRetries:         1,
    expiration:         Expiration::seconds(3600),
    recordExistsAction: RecordExistsAction::UpdateOnly,
    generationPolicy:   GenerationPolicy::ExpectGenEqual,
    generation:         $record->generation(),
    durableDelete:      true,
    replica:            Replica::Master,
    readModeSc:         ReadModeSC::Linearize,
    sendKey:            false,
    filter:             '$.age > 21',
), $key, [new Bin("age", 31)]);
```

| Field | Type | On |
| --- | --- | --- |
| `totalTimeoutMs`, `socketTimeoutMs`, `maxRetries`, `sleepBetweenRetriesMs` | `?int`, 0 .. 4294967295 | both |
| `replica` | `?Replica` | both |
| `readModeAp` / `readModeSc` | `?ReadModeAP` / `?ReadModeSC` | both |
| `useCompression` | `?bool` | both |
| `filter` | `?string`, an Aerospike Expression Language filter | both |
| `recordExistsAction` | `?RecordExistsAction` | write |
| `generationPolicy` + `generation` | `?GenerationPolicy` + `?int` | write |
| `expiration` | `?Expiration` | write |
| `commitLevel` | `?CommitLevel` | write |
| `durableDelete`, `respondPerEachOp`, `sendKey` | `?bool` | write |

Each getter is the field's name — `$policy->totalTimeoutMs()`,
`$policy->expiration()` — and returns `null` for a field that was never set.

Policies are **immutable**: a constructor and getters, no properties and no
setters. Properties registered from Rust are declared to PHP as untyped, so
`$policy->expiration = 3600` would be accepted by the engine and fail deeper in —
the one part of this surface the type system could not check. Named constructor
arguments give the same ergonomics with every argument checked, and an immutable
policy is safe to keep in a static and reuse across requests, which is what a PHP
worker wants to do with one.

Two rules survive from the array this replaced, because types cannot express
them:

- **`Expiration` is a class, not an int.** Three of its four cases are not
  durations: `Expiration::never()`, `Expiration::namespaceDefault()`,
  `Expiration::dontUpdate()` and `Expiration::seconds($n)`. Other clients encode
  the first three as `-1` and `-2`, which is exactly the detail that gets mixed
  up crossing a language boundary — so a negative `seconds()` is refused rather
  than read as one of them.
- **A generation and a generation policy come as a pair.** A generation with no
  guard does nothing, and a guard with no generation compares against zero;
  either alone is a policy that silently is not the one you wrote, so either
  alone is an error.

`replica` is inert on a write (which always goes to the partition master) and
`sendKey` on a read (there is no key to store). Both are accepted anyway, so one
policy can be shared across verbs.

### Filters throw

`filter` compiles an Aerospike Expression Language filter on the server (8.1.3
or later). A record the filter rejects **fails the call** — result code 27,
`FILTERED_OUT`, status `SERVER` — for reads as much as for writes. "No match" is
therefore control flow a caller catches:

```php
try {
    $record = $client->get(new ReadPolicy(filter: '$.age > 21'), $key);
} catch (Aerospike\AerospikeException $e) {
    if ($e->getResultCode() !== 27) { throw $e; }
    $record = null;   // the record is there; the filter rejected it
}
```

## Errors

Every database failure is an `Aerospike\AerospikeException extends Exception`:

| Accessor | Property | Meaning |
| --- | --- | --- |
| `getMessage()` | `message` | Human-readable detail. |
| `getStatus()` | `status` | An `Aerospike\Status` — whose problem this is. |
| `getResultCode()` | `resultCode` | The server's own result code, or `null`. |
| `isInDoubt()` | `inDoubt` | Whether a failed **write** may nevertheless have been applied. Do not blindly retry a non-idempotent operation when this is true. |
| `getCode()` | `code` | The server result code when there is one, otherwise the numeric status. |

`Status` is an enum, so a failure can be handled exhaustively:

```php
match ($e->getStatus()) {
    Status::Timeout    => $retryLater($e->isInDoubt()),
    Status::Connection => $failOver(),
    Status::Server     => $handle($e->getResultCode()),
    default            => throw $e,
};
```

Its cases are `Server`, `RecordNotFound`, `Timeout`, `Connection`,
`UnknownInstance`, `InvalidRequest`, `FrameTooLarge`, `Internal`, `Client` — the
request never reached the daemon — and `Unrecognized`, which a matched pair of
versions cannot produce but which exists because a status read out of shared
memory is data, and turning unexpected data into a panic would be worse than
naming it.

A **wrong argument type is a `TypeError`, not an `AerospikeException`**: it is a
mistake in the calling code, not a database failure, and `catch
(AerospikeException)` around a query should not swallow it.

A record that simply does not exist is **not** an error: `get()` returns
`null`.

Two known limitations of the exception, both inherited from ext-php-rs rather
than from this crate:

- `getFile()` and `getLine()` are empty. ext-php-rs installs its own
  `create_object` handler for the class, so Zend's exception constructor —
  which is what captures the throw site — never runs.
- `AerospikeException::__construct()` takes `($message, $code)` and no
  `$previous`; accepting one and silently dropping it would be worse than not
  offering it.

## Coming from the 1.x client

A good deal of the 1.x surface is here under its own name, because it cost nothing
to add and PHP's case-insensitive class and method names meant a separate
compatibility file could not have supplied it anyway:

- **The whole 1.x expression builder**, as 73 static methods on `Expression`:
  `Expression::gt(Expression::intBin('age'), Expression::intVal(21))` works
  unchanged. Only `Expression::new()`, the raw opcode constructor, is absent.
- **67 1.x enum factories** on 16 enums — `ReadModeAP::one()`,
  `CommitLevel::CommitAll()`, `IndexType::String()`, `ListReturn::Value()`,
  `BitOverflow::Wrap()`, and so on. These have to be here rather than in PHP because
  `Aerospike\ReadModeAP` and `Aerospike\ReadModeAp` are one name, which the
  extension already owns.
- **1.x accessors** on `Record` (`getBins()`, `getGeneration()`, `getTtl()`,
  `getKey()`, `getExpiration()`, and the matching magic properties), on `Key`
  (`getNamespace()`, `getSetname()`, `getValue()`, `getDigest()`,
  `getDigestBytes()`, `getPartitionId()`), on `RecordSet` (`nextRecord()`,
  `getActive()`) and on `BatchResult` (`getRecord()`, `getResultCode()`).
- **1.x verbs** on `Client`: `getHeader()` and
  `scan($policy, $partitions, $ns, $set, $bins)`.

What is *not* here — mutable policy objects, the 1.x CDT argument order,
`Client::connect()` — is in [`../compat`](../compat), a pure-PHP layer with no
extension code in it. Its README explains the two tiers and the one case
(`Recordset::next()`) that cannot be made transparent.

## Two ext-php-rs bugs worked around here

Both are worth knowing about if you read the source and wonder why it is shaped
the way it is. Both are in ext-php-rs 0.15.15.

- **Returning a PHP enum case corrupts its reference count.** `IntoZval` for a
  `RegisteredEnum` decrements the case object before handing it over, which is
  right for an object it owns — but `zend_enum_get_case` returns the one immortal
  instance from the enum's constant table without adding a reference, so the
  decrement steals the class's own. The case is freed as soon as the first zval
  holding it dies, and the next read of it is a use-after-free: a hard
  `EXC_BAD_ACCESS` inside `ZEND_FETCH_OBJ_R`, reached by using one exception's
  `getStatus()` twice in one expression. `enums::Case` is the corrected
  conversion; the PHP-visible types are unchanged.

  **The rule this implies: no `#[php_impl]` method may return a bare enum. Return
  `Case<T>`.** It is easy to forget because the broken version compiles, works, and
  passes a test that calls it — the corruption only shows up later, somewhere else,
  as an unrelated enum's constant reading back as the wrong case. The 67 static
  factories added for 1.x compatibility all hit this before they were wrapped, and
  the symptom was `ConsistencyLevel::ConsistencyOne()` returning a `ListOrder`.
  `tests/smoke.php` now hammers the returning methods and re-checks the singletons.
- **A nullable argument of the wrong type is silently dropped**, as described
  under [the shape of the API](#a-note-on-how-the-types-are-enforced).
  `arg::Given` is the fix.

## Stubs, for an IDE and PHPStan

`aerospike-php.stubs.php` declares all 87 classes and enums with their documentation
comments. It is checked in, so nothing has to be generated to get completion.

**Never autoload it.** The extension provides these classes itself; loading both is a
duplicate-declaration error. It is input to a static analyser, not to a program.

Regenerate after changing the PHP-facing surface:

```sh
make -C .. stubs        # or: tools/gen-stubs.sh
```

Three things about that are worth knowing before you touch it, because none is
guessable and each looks like a different problem than it is:

- **cargo-php has to be built specially on macOS**, and against *this crate's*
  `ext-php-rs` version:

  ```sh
  make -C .. install-cargo-php
  # which is: RUSTFLAGS="-C link-arg=-Wl,-undefined,dynamic_lookup" \
  #             cargo install cargo-php --force
  ```

  It links ext-php-rs into an executable, so it needs PHP's symbols at link time and
  will not build without that flag. And **not `--locked`**, which pins an older
  ext-php-rs: `describe::abi::Vec` crosses the library boundary and is dropped on
  cargo-php's side, so a version skew shows up as `pointer being freed was not
  allocated` at exit — a wild pointer, with no indication that a version is involved.

- **The library it inspects is built with `--features stub-gen`**, into
  `target/stub-gen` so it cannot be confused with a real one. That feature defines
  the Zend globals and functions this library deliberately leaves undefined for PHP
  to supply, because cargo-php `dlopen`s it and cargo-php is not PHP. Without it
  cargo-php 0.1.11 exits **0 having written nothing**, which reads as an extension
  with no classes rather than a library that failed to load.

- **Three post-processing steps**, each fixing something cargo-php cannot know:
  un-typing the two properties `AerospikeException` shadows from `Exception` (a typed
  override of an untyped parent is a parse error); dropping parameter defaults that
  precede a required parameter (`put($policy, $key, $bins)` — you may pass `null`,
  but you may not omit it, which is what PHP's deprecation notice says); and adding
  the 67 static methods declared on enums, which cargo-php does not emit at all. The
  last is read back from the real extension by reflection — see
  `tools/add-enum-statics.php` — because reflection cannot disagree with what PHP
  sees. Without it, every 1.x call site is undefined to an analyser.

## Testing

Rust unit tests cover the parts that do not need a running PHP — the policy and
value mappings, the argument-checking rules and their wording, and the
shared-memory loan sizes:

```sh
cargo test
```

Those tests can only run because `src/lib.rs` defines PHP's own global variables
under `cfg(test)`. ext-php-rs compiles a C shim whose object file refers to them;
inside PHP that is exactly right, but a `cargo test` binary is not inside PHP, and
macOS binds data symbols when the image loads — so without those definitions the
whole harness aborts before the first test with `dyld: symbol not found in flat
namespace '_executor_globals'`. Nothing under `cargo test` reads them. A test that
genuinely needed a PHP runtime is a test that has to be written in PHP, which is
what `tests/smoke.php` is.

The end-to-end test is a plain PHP script. It needs the daemon running, which
from a checkout means:

```sh
# From the repository root: build and start the daemon against a local server.
cargo build -p aerospike-php-daemon
cp aerospike-php/daemon/aerospike-daemon.toml.example my-daemon.toml   # set host
./target/debug/aerospike-php-daemon --config my-daemon.toml --check     # validate
./target/debug/aerospike-php-daemon --config my-daemon.toml &

# Then, from this directory:
php -d extension=$(pwd)/target/release/libaerospike_php.dylib tests/smoke.php
```

`AEROSPIKE_NAMESPACE` (default `test`) chooses the namespace for the bulk of it, and
`AEROSPIKE_SC_NAMESPACE` (default: the same one) the namespace for the transaction
block. **They can need to differ**: transactions require a strong-consistency
namespace, and a strong-consistency namespace forbids non-durable deletes — which
several tests deliberately pin. On a cluster with one AP namespace everything
passes and the transaction block self-skips, saying why.

```sh
AEROSPIKE_NAMESPACE=testap AEROSPIKE_SC_NAMESPACE=test \
  php -d extension=$(pwd)/target/release/libaerospike_php.dylib tests/smoke.php
```

It PINGs — checking that the daemon's version is exactly this extension's — does
a `put`/`get` round trip over every supported value shape, exercises `delete`,
`exists`, `touch`, `add`, `append` and `prepend`, round trips a `Blob` and a
`GeoJson`, drives policies against the server (a TTL, `CreateOnly`, a
generation-guarded write, a filter), and covers bin selection, missing records,
integer and binary keys, bin deletion and the rejection paths. A whole section
asserts the *class* of failure for a wrong argument, because a test that only
checked "it failed somehow" would keep passing if the types were given up on.
It prints what it found and exits non-zero if anything failed. Override the
target with `AEROSPIKE_INSTANCE`, `AEROSPIKE_NAMESPACE` and `AEROSPIKE_SET`
(defaults: `default`, `test`, `smoke`).

Run without a daemon it stops at the PING and tells you so, with exit code 1 —
which is itself a useful check that the failure path is clear.

### The process model, with real processes

`smoke.php` runs in one process and never forks, so it cannot reach any of what
`src/transport.rs` is actually about. `tests/concurrency.php` forks:

```sh
php -d extension=$(pwd)/target/release/libaerospike_php.dylib tests/concurrency.php
```

Four sections. **A dead worker's slot** fills a pool to the `max-workers`
ceiling, kills a worker with `SIGKILL` and requires the replacement to attach.
**Fork safety** has an already-attached parent fork children that use the client,
then checks the parent still works — plus a grandchild, a child that touches
nothing, and a child that abandons a transaction, since each takes a different
path through the pid recheck and `discard_if_forked`. **The `max-workers`
ceiling** attaches one process past the limit and asserts the refusal names the
setting rather than reporting an iceoryx2 enum. **Contention** runs many processes
against one daemon, each checking it sees *its own* replies — the only way to
exercise the notifier cache and the ticket map under load.

It needs `pcntl` and `posix`, which are CLI-only; that is why it is separate from
the suite everyone runs. Two sections need the daemon's ceiling stated and skip
without it, because attaching one process past a limit nobody named proves
nothing. `tests/concurrency.sh` starts a throwaway daemon under its own instance
name with a small `max-workers` so those sections are cheap to run:

```sh
tests/concurrency.sh        # max-workers = 4
tests/concurrency.sh 16
```

## Diagnostics

The extension writes nothing to stderr. iceoryx2 logs informationally there by
default — "No config file was loaded", node bookkeeping — which in a PHP
extension would land in a web server's error log on every worker that
attaches, so its log level is raised to `Error` on first use. Anything that
matters becomes an `Aerospike\AerospikeException`; genuine iceoryx2 errors
still print, which keeps a broken shared-memory setup diagnosable.

## Notes on the process model

Two things about PHP shape this extension, and both are documented at length in
`src/transport.rs`:

- **Fork safety.** PHP-FPM runs `MINIT` in the *master* process and only then
  forks its workers. iceoryx2 state cannot cross `fork()`. So the extension
  opens nothing in `MINIT`; it attaches lazily on first use — which under FPM
  is always inside a worker — records the pid it attached under, and starts
  over if that pid ever changes. Inherited state is leaked rather than
  dropped, because running iceoryx2's destructors in a child would release
  resources the parent still owns.
- **One request in flight.** PHP is synchronous, so a worker keeps a single
  monotonic sequence number, sends, and then polls for the reply. A reply
  whose sequence is older than the current request is discarded, so an answer
  arriving after a timeout can never be mistaken for the next call's result.
