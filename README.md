> [!WARNING]
> THIS IS STRICTLY A PREVIEW CODEBASE, RELEASED FOR CUSTOMER FEEDBACK.
> DO NOT USE THIS IN ANY CAPACITY.


# Aerospike for PHP

[![PHP version](https://img.shields.io/badge/php-%3E%3D%208.1-8892BF.svg)](https://www.php.net/)
[![Rust version](https://img.shields.io/badge/rust-%3E%3D%201.87-dea584.svg)](https://www.rust-lang.org/)

A PHP client for [Aerospike](https://www.aerospike.com/) built on the Rust client,
split into a long-lived daemon that owns the database connections and a thin PHP
extension that talks to it over shared memory.

**Status: complete but unreleased.** Every command family is implemented except
metrics: the single-record verbs, `operate` with all six operation families,
heterogeneous `batch`, paged scan and query, cluster management (UDF, secondary
indexes, truncate, info, nodes), multi-record transactions, the security commands,
TLS — so `EXTERNAL` and `PKI` authentication work — and a client-side expression
builder covering `aerospike-core`'s whole expression surface: the general, list,
map, bitwise, HLL, string and CDT path-expression families. Scripts written against
the **1.x client** mostly run unchanged; see [coming from the 1.x
client](#coming-from-the-1x-client).

Not done, and worth knowing before you depend on it:

- **The metrics API**, the one missing command family.
- **The security commands are implemented but not verified live.** They need
  `enable-security true` on the cluster; without it that block of the suite self-skips
  rather than passes, which is the one remaining `skipped` in a full run.
  (Transactions have the same shape of requirement — a strong-consistency namespace —
  but that one is satisfied here: point `AEROSPIKE_SC_NAMESPACE` at an SC namespace,
  as `make test` does, and commit, abort and self-rollback are all asserted live.)
- **Not packaged.** No PECL release, no Composer package; you build both halves and
  load the extension by path.

```text
┌──────────────── one host ──────────────────────────────────┐
│  php-fpm worker 1 ─┐                                       │
│  php-fpm worker 2 ─┼── iceoryx2 shared memory ──┐          │
│  php-fpm worker N ─┘                             │         │
│                                        ┌─────────▼───────┐ │
│                                        │ aerospike-php-  │ │
│                                        │ daemon          │ ├──▶ Aerospike
│                                        │ (aerospike-core)│ │    cluster
│                                        └─────────────────┘ │
└────────────────────────────────────────────────────────────┘
```

## Why a daemon

The Rust client is a *smart* client: it discovers the cluster, runs a tend
loop, and keeps per-node connection pools. PHP's per-request execution model
is hostile to that — with 200 FPM workers each linking the client directly you
would get 200 tend loops, 200× the connections, and cluster discovery latency
every time a worker spawns.

One daemon means one cluster view, one connection pool and one place to watch,
and the extension stays a thin proxy. The cost is an IPC hop, which zero-copy
shared memory puts in the low microseconds against a network operation of
50–200µs.

## The three crates

| Crate | What it is |
| --- | --- |
| [`ipc/`](ipc) | The wire contract: typed headers, request/reply bodies, status codes, service names, wait strategies. Depends on neither the database client nor PHP, so the whole transport is testable without either. |
| [`daemon/`](daemon) | The daemon. Owns `aerospike-core` clients (one per configured cluster) and serves workers over shared memory. |
| [`ext/`](ext) | The PHP extension (`ext-php-rs`). Marshals PHP values, sends a request, waits for the reply, raises PHP exceptions. |

Plus [`compat/`](compat), which is not a crate: pure PHP, no extension code, a
compatibility layer for scripts written against the 1.x client. Optional — load it
or don't.

## A self-contained workspace

This directory is **its own cargo workspace**, and `ipc` and `daemon` are its members. 

**`ext` is deliberately excluded** from even this workspace: 
it links the Zend API, so building it needs PHP headers and `php-config`,
and the contract and the daemon must remain buildable and testable on a machine with no
PHP at all. That is why there are two target directories and two lock files.

### What the move requires

One line — the `aerospike-core` dependency in [`Cargo.toml`](Cargo.toml), which is a
relative path while the two share a repository:

```toml
[workspace.dependencies.aerospike-core]
git = "https://github.com/aerospike/aerospike-client-rust"
branch = "v3"
features = ["rt-tokio", "tls"]
```

Two things are already prepared for it and should not be undone:

- **Both `Cargo.lock` files are committed.** Right for this directory and wrong for the
  Rust client, which ignores its own: a library lets downstream users resolve their
  own versions, while this builds a daemon binary and an extension that **must be the
  same version as each other**, so a reproducible resolution is part of the contract.
- **[`.gitignore`](.gitignore) re-includes the `Makefile`.** The Rust client's root
  ignore file has a bare `Makefile` line, which matches at every level and would
  silently drop this one from git. That negation can go once the root's rules no longer
  reach here — and until then, deleting it loses the file with no warning.

## One version, both halves

The extension and the daemon must be **exactly the same version**. There is no
negotiation and no compatibility window; three mechanisms keep a mismatched pair
apart rather than letting them half-work:

| Mechanism | Catches |
| --- | --- |
| the shared-memory service name embeds the version | a mismatched pair meeting at all — the extension cannot open a service another version created |
| a version fingerprint in every frame header | a stale mapping, or a daemon restarted from a different build under a name that already resolved |
| `ping()` reports the daemon's version | telling you the readable versions once something is already wrong |

The version comes from one place — `aerospike-php-ipc`'s package version — and
both halves assert at compile time that their own version equals it, so they
cannot be bumped independently.

This is what makes the wire contract cheap to change: nothing in it has to carry
a compatibility shim, because no build ever talks to a different one. The cost is
one operational rule — **upgrade both halves together and restart the daemon** —
and a clear failure when you don't:

```text
cannot attach to the Aerospike daemon for instance "default" [..]
A daemon for instance "default" is running, but it is version 3.0.0-alpha.1 and
this extension is 3.1.0. The two must be exactly the same version: install them
together and restart the daemon.
```

That message is why the extension enumerates the shared-memory services when an
attach fails: to a caller, "the wrong daemon" and "no daemon" look identical —
the process is running and the configuration is right — and it is the mismatch
that is hardest to guess.

## Transport

Every request and reply — payload *and* metadata — crosses through iceoryx2
shared memory. The fixed part of each frame is a `#[repr(C)]`,
`ZeroCopySend` user header, so opcode, sequence, status, result code,
generation and TTL are typed fields written straight into shared memory with no
serialization at all. Only the variable tail (namespace, set, key, bin names
and recursively nested bin values) is encoded, with postcard — Aerospike values
nest arbitrarily, so they cannot be a fixed-size POD.

How a waiting worker learns its reply is ready is a compile-time choice:

| Build | Wakeup | Transport |
| --- | --- | --- |
| default | block on an iceoryx2 event port | shared memory, plus a unix datagram socket for wakeups |
| `--features shm-poll` | poll the response port, spinning then yielding then sleeping in capped steps | **shared memory only** |

Measured, polling buys about 10µs of latency for roughly **17× the CPU** in the
waiting process — ~192µs per operation against ~11µs — because a polling worker
pins a core for the whole round trip ([`bench/`](bench)). That is free at one
worker and ruinous at two hundred, so blocking on an event is the default.
Choose `shm-poll` when worker concurrency is low, latency is critical, or the
transport must provably stay in shared memory.

Note that the daemon polls its request port either way: iceoryx2
request/response ports cannot be attached to a `WaitSet`, so only the *worker's*
wait changes. **Both sides must be built the same way.**

## Prerequisites

- **PHP 8.1 or later** with `php-config` on `PATH`. Developed against PHP 8.5 NTS.
- **Rust 1.87 or later**, and a C toolchain with `libclang` — ext-php-rs' `bindgen`
  reads the Zend headers. macOS: the Xcode command line tools. Debian or Ubuntu:
  `apt install libclang-dev php-dev`.
- **An Aerospike server** to talk to. [Download](https://aerospike.com/download/).
- **Linux or macOS.** No Windows: iceoryx2's shared-memory transport is POSIX.

Nothing else — no Go toolchain, no protobuf compiler, no gRPC. The daemon and the
extension are both Rust and speak to each other through shared memory. See
[`ext/README.md`](ext/README.md#prerequisites) for building against a specific PHP.

## Quick start

```bash
cd aerospike-php
make build            # both halves, release
make daemon           # copies the example config the first time, then serves
```

`make daemon` stops the first time and asks you to point `host` at your cluster;
rerun it after editing. `make help` lists everything else. The equivalent by hand:

```bash
cp daemon/aerospike-daemon.toml.example aerospike-daemon.toml
$EDITOR aerospike-daemon.toml                      # point `host` at your cluster
cargo run -p aerospike-php-daemon -- --config aerospike-daemon.toml
cd ext && cargo build --release
```

For editor completion and PHPStan, point them at
[`ext/aerospike-php.stubs.php`](ext/aerospike-php.stubs.php) — declarations for all 87
classes and enums, with the documentation comments. **Do not autoload it**: the
extension already provides these classes, so loading both is a duplicate-declaration
error. It is checked in, and `ext/tools/gen-stubs.sh` regenerates it.

```php
<?php
$client = new Aerospike\Client();            // the "default" instance
$key    = new Aerospike\Key("test", "users", "alice");

$client->put(null, $key, [
    new Aerospike\Bin("name", "Alice"),
    new Aerospike\Bin("age", 30),
]);

$record = $client->get(null, $key);
$record->bin("age");       // 30
$record->generation();     // 1
```

Every command takes its arguments in `aerospike-core`'s order — policy, key, then
whatever the verb operates on — and everything the Rust client models as a type
is a class or a PHP `enum` here. `null` as a policy means "use the daemon's
configured settings".

Several operations against one record, in order and atomically:

```php
$record = $client->operate(null, $key, [
    Aerospike\Op::add(new Aerospike\Bin("views", 1)),
    Aerospike\ListOp::append("history", "viewed"),
    Aerospike\ListOp::size("history"),
]);
```

Many records in one round trip, and the rows need not be the same kind of work or
even the same namespace:

```php
use Aerospike\{BatchRead, BatchWrite, BatchDelete, Op, Bin};

$results = $client->batch(null, [
    BatchRead::all($alice),
    BatchWrite::ops($bob, [Op::add(new Bin("visits", 1)), Op::getBin("visits")]),
    BatchDelete::key($stale),
]);

foreach ($results as $i => $row) {
    if (!$row->isOk()) {
        echo "row $i: result code ", $row->resultCode(), "\n";   // 2 = no such record
        continue;
    }
    echo "row $i: ", json_encode($row->record()?->bins() ?? []), "\n";
}
```

**A failed row is not a failed batch.** The rows come back in the order they were
sent, each with its own result code, and the call itself succeeds — so `isOk()` is not
optional. That is why the batch is *not* an array of records: a read that found
nothing and a row that failed both have no record, and only the result code tells
them apart.

Scan a set, or query a secondary index — one method for both, as in
`aerospike-core`, because a statement with no filter *is* a scan:

```php
$statement = new Aerospike\Statement("test", "users");
foreach ($client->query(null, null, $statement) as $record) {
    echo $record->key()?->userKey(), " ", $record->bin("name"), "\n";
}
```

Records arrive a page at a time — one round trip per page, `pageSize` on
`Aerospike\QueryPolicy` records each — and `foreach` hides that. Nothing streams:
iceoryx2 drops the oldest of a burst of responses without telling the sender, so
a page plus a cursor is the only shape that cannot lose records silently.

Build a server-side expression — as a policy filter, an operation, or an index —
with `Aerospike\Exp` and its five collection siblings:

```php
use Aerospike\{Exp, ExpList, ExpType, ListReturn};

$filter = Exp::and([
    Exp::gt(Exp::intBin("age"), Exp::intVal(21)),
    Exp::gt(
        ExpList::getByRank(ListReturn::Values, ExpType::Integer, Exp::intVal(-1),
            Exp::listBin("scores")),
        Exp::intVal(900),
    ),
]);
$record = $client->get(new Aerospike\ReadPolicy(filterExp: $filter), $key);
```

The tree is packed by the **client**, so it works against every server this client
supports — unlike the Aerospike Expression Language text form
(`Expression::ael('$.age > 21')`), which the server compiles and which needs
server 8.1.3. `ExpList`, `ExpMap`, `ExpBit`, `ExpHll`, `ExpStr` and `ExpPath` cover
the collection, blob, sketch, string and path families. Two of those do have a
server requirement, and the daemon checks both: the string expressions need 8.1.3,
and a path expression's fan-out over every child of a collection needs 8.1.1.

Managing the cluster — indexes, UDF modules, truncation, info, nodes. The
long-running commands return an `Aerospike\Task`, because the server accepts the
work and then does it:

```php
$client->createIndexOnBin(null, "test", "users", "age", "age_idx", Aerospike\IndexType::Numeric)
       ->waitTillComplete(30_000);

$client->registerUdf(null, file_get_contents("example.lua"), "example.lua")
       ->waitTillComplete(10_000);
$client->executeUdf(null, $key, "example", "bump", ["views", 1]);

$client->info(null, ["build"])["build"];   // "8.1.3.0"
```

The waiting happens in the PHP process, not in the daemon: a task is a
*description* of the work, so nothing is stored on the daemon's side and no
request is held open for minutes.

Users, roles and privileges — on a cluster with security enabled:

```php
use Aerospike\{Privilege, PrivilegeCode};

$client->createRole(null, "auditor", [new Privilege(PrivilegeCode::Read, "test")]);
$client->createUser(null, "alice", "secret", ["auditor"]);
foreach ($client->queryUsers(null) as $user) {
    echo $user->name(), ": ", implode(", ", $user->roles()), "\n";
}
```

A multi-record transaction, where either all the writes land or none do:

```php
$txn = $client->beginTransaction();
$client->put(new Aerospike\WritePolicy(txn: $txn), $from, [new Aerospike\Bin("balance", 70)]);
$client->put(new Aerospike\WritePolicy(txn: $txn), $to,   [new Aerospike\Bin("balance", 30)]);
$txn->commit();
```

Commands join by carrying the transaction on their policy, as they do in the Rust
client. **An unfinished one rolls back**: the object aborts itself when it is
destroyed, so a request that throws does not leave record locks behind. Needs
server 8.0+ and a strong-consistency namespace.

A commit is two batch commands — verify every version the transaction read, then
roll its writes forward — so `commitWithPolicies(?TxnVerifyPolicy, ?TxnRollPolicy)`
takes one policy for each, and `abortWithPolicy(?TxnRollPolicy)` takes the one an
abort has (it has nothing to verify). `commit()` and `abort()` pass the client's own
tuned defaults, which is almost always right.

Run it with the extension loaded:

```bash
php -d extension=$(pwd)/ext/target/release/libaerospike_php.dylib script.php   # .so on Linux
```

Or install it permanently by adding one line to your `php.ini`, and check it took:

```ini
extension=/path/to/aerospike-php/ext/target/release/libaerospike_php.dylib
```

```bash
php -m | grep aerospike
```

See [`ext/README.md`](ext/README.md) for the ini settings and the PHP value-mapping
rules.

### Declaring the namespace

Every class is in `Aerospike\`. Three ways to write that, all equivalent — the first
is what 1.x code does, and it keeps working:

```php
namespace Aerospike;                       // everything unqualified: new Key(...)
use Aerospike\{Client, Key, Bin};          // name what you use
$client = new Aerospike\Client();          // qualify at each use
```

## Coming from the 1.x client

Most of a 1.x script runs unchanged, and the rest needs one `use` line changed per
file. Two things get you there, and which one carries a given name is decided by a
constraint of PHP's rather than by preference: **class and method names are
case-insensitive**, so `Aerospike\Recordset` *is* `Aerospike\RecordSet` and a PHP
file cannot define a name the extension already owns under any spelling.

So the extension itself carries the 1.x names it owns:

- **The whole 1.x expression builder** — `Expression::gt(Expression::intBin('age'),
  Expression::intVal(21))`, all 73 static methods with the same argument order. Only
  `Expression::new()`, the raw opcode constructor, is absent.
- **67 1.x enum factories** on 16 enums: `ReadModeAP::one()`,
  `CommitLevel::CommitAll()`, `IndexType::String()`, `ListReturn::Value()`,
  `BitOverflow::Wrap()` and the rest.
- **1.x accessors** — `Record::getBins()`, `getGeneration()`, `getTtl()`,
  `getExpiration()` and the matching `$record->bins` properties; `Key::getNamespace()`,
  `getSetname()`, `getValue()`, `getDigest()`; `RecordSet::nextRecord()`; and
  `BatchResult::getRecord()`.
- **1.x verbs** — `Client::getHeader()` and
  `scan($policy, $partitions, $ns, $set, $bins)`.

And [`compat/`](compat) — pure PHP, no extension code — carries the rest: mutable
policy objects with `set*`/`get*`, the 1.x CDT argument order, `Client::connect()`
taking a socket path, `ResultCode`, the flag sets, and `BatchRecord`. Load it or
don't:

```php
require_once 'aerospike-php/compat/src/bootstrap.php';
```

One thing cannot be made transparent, and it fails loudly rather than quietly: 1.x
`Recordset::next()` *returns* a record, while `Iterator::next()` returns void and PHP
forbids widening that. Since the 1.x name is case-insensitively taken, old
`while ($r = $rs->next())` code against the native class would iterate **zero times
in silence** — so the wrapper is `Aerospike\Compat\Recordset` and an
`Aerospike\Recordset` type hint raises a `TypeError` naming both classes. Use
`foreach`, which was always fine, or `nextRecord()`.

[`compat/README.md`](compat/README.md) has the full mapping and the migration order.

## TLS, and the two auth modes it unblocks

A `[cluster.<name>.tls]` section enables TLS for that cluster. Its **presence** is
the switch — there is no `enabled` flag, because the CA is not optional in any
deployment worth calling configured:

```toml
[cluster.default]
host = "node1:node1.cluster.example:4333"   # the TLS name belongs to the seed
auth = "PKI"

[cluster.default.tls]
ca-file   = "/etc/aerospike/certs/ca.pem"
cert-file = "/etc/aerospike/certs/client.pem"   # required for PKI
key-file  = "/etc/aerospike/certs/client.key"
```

That is what makes `EXTERNAL` (which sends the password) and `PKI` (where the
client certificate *is* the identity) usable — both were refused at startup
before. The certificate files are read while the configuration is validated, so
`--check` catches a wrong path, which is the only time anyone is looking.

Deliberately not offered: cipher-suite and protocol selection — rustls picks safe
defaults and refuses unsafe ones, so there is nothing useful to choose — and any
"skip verification" switch, because a daemon that can be configured not to check
certificates is one whose TLS means nothing.

## Configuring clusters

The daemon's configuration file defines any number of clusters as
`[cluster.<name>]` sections; `<name>` is the *instance* PHP asks for, and
`default` is used when PHP names none.

```toml
[daemon]
instance = "default"       # the shared-memory service name; PHP must match

[defaults]                 # inherited by every cluster below
timeout = "30s"

[cluster.default]
host = "127.0.0.1:3000"

[cluster.analytics]
host = "10.0.0.10:3000,10.0.0.11:3000"
use-services-alternate = true
```

```php
$analytics = new Aerospike\Client("analytics");
```

Validate a file without starting anything:

```bash
cargo run -p aerospike-php-daemon -- --config aerospike-daemon.toml --check
```

`aerospike-daemon.toml.example` documents every key, and **every field of
`ClientPolicy` is reachable** — timeouts, pooling, the buffer pool, rack
awareness, address rewriting, the identity the client reports, TLS. Two keys are
accepted for compatibility but have no effect, and the daemon says so at startup:
`limit-connections-to-queue-size` (the Rust pool always behaves as `true`) and
`ignore-other-subnet-aliases` (no counterpart in the Rust client).

Unknown keys are a startup error rather than a warning, so a typo cannot silently
do nothing — and values that could never work are refused with the key named: a
buffer-pool size that is not a power of two, a set-scoped privilege that must be
cluster-wide, `rack-aware = true` with no racks.

### The one setting to change on a busy host

```toml
[daemon]
max-workers = 250      # default 64
```

It is the iceoryx2 service's client limit, **fixed when the service is created**,
so a PHP-FPM pool larger than this leaves the extra workers unable to attach at
all — and raising it means restarting the daemon with no worker attached. The
default suits development, not a real pool.

Set it to the pool size; it needs no padding for workers that die. A process
killed outright runs no destructors, but its slot is reclaimed when the next
worker starts, so a pool sized exactly at `max-workers` survives hard kills at
full strength. `ext/tests/concurrency.php` asserts exactly that.

`[daemon.receive-backoff]` tunes how the serving loop waits when idle. It is
separate from PHP's own `aerospike.*` ini settings on purpose: a worker waits for
one reply and can block on an event, while iceoryx2 request/response ports cannot
be attached to a `WaitSet`, so the daemon's loop has no choice but to poll — about
255µs of CPU per operation whichever wakeup mode the workers use. Raise its sleep
tiers for a **remote** cluster, where a round trip is milliseconds and spinning
through it wastes a core.

## Testing

```bash
make test            # unit tests, then the live suites
make test-all        # the same in both wakeup modes, restoring the default build
```

`make help` lists the rest. Individually:

```bash
make test-unit       # Rust: contract, daemon (needs a cluster), extension
make smoke           # the extension against a running daemon
make concurrency     # the process model, with real forked processes
make compat          # the 1.x compatibility layer, in the old idiom throughout
make examples        # every example in examples/, as a test
```

`make smoke` and `make compat` need a daemon running — `make daemon`, which copies
the documented example configuration the first time and tells you to point `host` at
your cluster. `make concurrency` and the shm-poll live suites start their own.

### Which cluster the tests run against

No test names a server. The suites that bring up their own daemon — the Rust
integration tests, `make concurrency`, the shm-poll targets — take the cluster from
these variables, and `make` fills them in from `aerospike-daemon.toml`, the file
`make daemon` already has you point at your cluster. So one cluster serves
everything, and `make help` prints what is in effect:

| variable | `make` | what it is |
| --- | --- | --- |
| `AEROSPIKE_HOSTS` | `hosts=` | seed host(s), `host:port` (default `127.0.0.1:3000`) |
| `AEROSPIKE_USE_SERVICES_ALTERNATE` | `alternate=` | tend the `services-alternate` list — needed for a NAT'ed or containerised node that advertises an address you cannot reach |
| `AEROSPIKE_USER`, `AEROSPIKE_PASSWORD` | `user=`, `password=` | credentials, for a secured cluster |
| `AEROSPIKE_AUTH_MODE` | `auth=` | `INTERNAL`, `EXTERNAL`, `EXTERNAL_INSECURE` or `PKI` |

```bash
make test                                        # the cluster in aerospike-daemon.toml
make test hosts=10.0.0.5:3000 alternate=false    # somewhere else, just this once
AEROSPIKE_HOSTS=10.0.0.5:3000 make test          # the same, from the environment
```

On a **secured** cluster the credentials need the privileges the suite exercises —
read-write, `sys-admin` for indexes and UDFs, `data-admin` for truncate, `user-admin`
for the security commands. The stock `admin` user has `user-admin` alone, and a run
with it fails on `RoleViolation` at the first write; the tests say so when they see
that code, rather than reporting a permissions problem as a broken feature.

**Both wakeup modes matter, and `make test-shm-poll` is the way to test the other
one.** The mode is a compile-time feature on *both* halves, and a mismatched pair
does not report an error — the worker simply never learns its reply is ready. So that
target rebuilds both halves and brings up its own daemon under a separate instance
name, printing the mode it is actually serving with:

```text
  [with-daemon] instance 'shmpoll', wakeup: shm-poll
```

The daemon's integration tests drive the real server loop over real shared memory
with no PHP involved, which is what lets the transport be validated independently of
the extension.

### Two namespaces, because transactions need a different one

Both suites take `AEROSPIKE_NAMESPACE` (default `test`) for the bulk of the tests
and `AEROSPIKE_SC_NAMESPACE` (default: the same one) for the transaction block.
They can need to differ, and the reason is not a preference:

- multi-record transactions require a **strong-consistency** namespace;
- a strong-consistency namespace **forbids non-durable deletes** (result code 22,
  `FailForbidden`), and durable deletes leave tombstones that change what
  `delete()` and `exists()` report — which several tests deliberately pin.

So on a cluster with one AP namespace, everything passes and the transaction block
self-skips, saying so. With both:

```bash
export AEROSPIKE_NAMESPACE=testap        # availability mode: the bulk
export AEROSPIKE_SC_NAMESPACE=test       # strong consistency: transactions
```

A single-node SC namespace also needs a roster before it will serve anything, which
is a one-off after each fresh start:

```bash
# through this client, since it can already send info commands
$node = $client->info(null, ['node'])['node'];
$client->info(null, ["roster-set:namespace=test;nodes=$node"]);
$client->info(null, ['recluster:']);
```

Pin `node-id` in the server's `service` stanza and set `auto-revive true` on the
namespace, or a container that gets a new MAC on restart will leave the roster
naming a node that no longer exists — every partition dead and every command
answering `Invalid cluster node`.

The daemon's integration tests drive the real server loop over real shared
memory with no PHP involved, which is what lets the transport be validated
independently of the extension.

## Current limitations

- **Operations**: the single-record verbs — `ping`, `put`, `get`, `delete`,
  `touch`, `exists`, `add`, `append`, `prepend` — plus `operate` with **every
  operation family**: scalar, list, map, bitwise, HyperLogLog and expression,
  with nested-collection paths — `batch`, whose rows may each be a read, write,
  delete or UDF call against any key in any namespace, `query`, which covers scans
  and secondary-index queries in pages, and the management commands: UDF
  registration and calls (single-record and background), secondary-index create
  and drop, `truncate`, `info` and `nodes` — multi-record transactions, and the
  security commands: users, roles and privileges. **Metrics is the one family not
  implemented.**
- **The security commands need `enable-security true`** on the cluster. Without it
  every one of the fourteen fails with result code 52, `SecurityNotEnabled`, which
  the server names exactly — so this client does not probe for it. The privilege
  *scope* rule is checked here, though: the six administrative codes act on the
  cluster and cannot be confined to a namespace, and the server's own refusal names
  neither the privilege nor the reason.
- **Transactions need server 8.0+ and a strong-consistency namespace.** The first
  is checked when the transaction opens; the second is the server's to enforce and
  shows up as a failure on the first write, which is the most common reason a
  transaction that looks right does not work. A transaction cannot cover a scan or
  a query — a traversal names no keys — so `QueryPolicy` has no `txn` at all.
- **An unfinished transaction rolls back**, three ways over: the PHP object's
  destructor, the daemon's idle sweep (which *aborts* rather than forgets, because
  an open transaction holds record locks), and the server's own transaction
  timeout.
- **A query needs its index to exist.** A filter on an unindexed bin is reported as
  the server's own result code 201, not as an empty result — it never quietly
  becomes a scan. Create one with `createIndexOnBin()` and wait on the task: until
  the build finishes the index returns *incomplete* results rather than an error.
- **A long-running command returns a task, and the waiting is the caller's.**
  Nothing in the daemon blocks, so no request outlives a worker's reply deadline. A
  task is a description — a namespace and index name, a package name, a job id — so
  it survives a daemon restart. Two answers are less informative than they look: a
  background job that finished and one that never existed both read `Complete`, and
  an index that does not exist throws rather than reporting `NotFound`.
- **Scan cursors live in the daemon.** A page hands PHP a cursor number and the
  daemon keeps the progress record; it holds no connection or server-side
  resource between pages. Dropping the `RecordSet` releases it, and one that is
  abandoned anyway expires (`cursor_idle_timeout`, 60s by default) with at most
  `max_cursors` open at once.
- **Values**: null, bool, int, float, string, list, map (unordered, key-ordered
  and insertion-ordered), blob, GeoJSON and HLL. A PHP string cannot say whether
  it is text or binary, so a bare string is always written as text;
  `Aerospike\Blob` writes bytes.
- **Map kinds**: `Aerospike\SortedMap` and `Aerospike\OrderedMap` are classes
  with iteration and array access, accepted anywhere a map is. A key-ordered map
  reads back as a `SortedMap`; an insertion-ordered one cannot round trip,
  because the server has no such map type.
- **TLS**: configured per cluster with a `[cluster.<name>.tls]` section, which is
  what makes `EXTERNAL` and `PKI` auth work. Cipher-suite selection and any
  skip-verification switch are deliberately absent.
- **Exceptions**: `getMessage()`, `getStatus()`, `getResultCode()`,
  `isInDoubt()` and `getCode()` work; `getFile()`/`getLine()` are empty, because
  `ext-php-rs` installs its own object constructor and Zend never captures the
  throw site.
- **Map ordering**: the server returns map bins in its own canonical key
  order, so the order in which pairs were written does not survive a round
  trip. Contents do.
- **`compat/` is loaded by path**, not through an autoloader: it has a
  `composer.json` but is not published, so `require_once` on `src/bootstrap.php` is
  how you get it. Two 1.x settings in it are **accepted and inert** —
  `Concurrency` (the daemon decides batch concurrency) and `QueryDuration` (replaced
  by explicit `pageSize`) — so a migration passing one loses it without being told.

## Troubleshooting

**Every method throws if the daemon is not there.** The message distinguishes the two
cases that look identical from PHP — the process is running and the configuration is
right — by enumerating the shared-memory services: *no daemon for this instance* is a
different failure from *a daemon of the wrong version*, and the second is the one
nobody guesses.

**A daemon killed outright needs no cleanup.** Unlike the 1.x client, whose connection
manager left a `/tmp/asld_grpc.sock` you had to remove by hand, a `SIGKILL`ed daemon
here can be restarted immediately under the same instance name — iceoryx2 reclaims the
service. Verified, because it is the first thing anyone would go looking for. What
does accumulate under `/tmp/iceoryx2/nodes` is a stale entry per hard-killed process:
tens of kilobytes, safe to delete when nothing is running, and never a reason a
daemon fails to start.

**A worker cannot attach.** `max-workers` is iceoryx2's client limit and is **fixed
when the service is created**, so a PHP-FPM pool larger than it leaves the extra
workers unable to attach at all. Raising it means restarting the daemon with no worker
attached. The refusal names the setting.

**Commands hang rather than fail.** The two halves were built with different wakeup
modes. The mode is a compile-time feature on each, and a mismatched pair has nothing
to notify — see [Transport](#transport). Rebuild both: `make build`.

**`php -m` does not list aerospike.** The extension is not loaded, which is a
different problem from the daemon being absent, and PHP reports it as an undefined
class rather than as a missing extension.

## Documentation

- [Aerospike documentation](https://aerospike.com/docs/) — the server, its data model
  and the operations this client exposes.
- [`ext/README.md`](ext/README.md) — the PHP API in full: value mapping, every command
  family, policies, errors, and the process model.
- [`ext/aerospike-php.stubs.php`](ext/aerospike-php.stubs.php) — all 87 classes with
  their documentation comments, for an IDE or PHPStan.
- [`examples/`](examples) — 13 worked examples, one per feature area, each runnable
  on its own and all of them run as tests by `make examples`.
- [`compat/README.md`](compat/README.md) — migrating from the 1.x client.
- [`daemon/aerospike-daemon.toml.example`](daemon/aerospike-daemon.toml.example) —
  every configuration key, documented in place.
- The wire contract has no README of its own: it is documented in
  [`ipc/src/lib.rs`](ipc/src/lib.rs)'s module docs, next to the types, and readable
  with `cargo doc -p aerospike-php-ipc --open`.

## Issues

Bugs, feature requests and feedback:
[GitHub issues](https://github.com/aerospike/aerospike-client-rust/issues).

Two things are worth including, because they are what the answer usually turns on —
the daemon's version and the instances it serves:

```bash
php -d extension=<lib> -r '$i = (new Aerospike\Client())->ping();
  printf("daemon %s, instances: %s\n", $i->version(), implode(",", $i->instances()));'
```

```text
daemon 3.0.0-alpha.1, instances: default
```

and whether both halves were built from the same commit, with the same features.
(`var_dump()` on the result shows an empty object: `DaemonInfo`'s data is behind
accessors, as everything registered from Rust is.)
