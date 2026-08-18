# Examples

Worked examples for the Aerospike PHP client, one per feature area. Each is a
standalone script — read one, run one, copy from one.

These are ports of the Rust client's [`examples/`](../../examples), which are in turn
ports of the Java client's examples. So the same program exists in three clients, and
comparing them is the fastest way to see what changes between languages and what does
not.

## Running them

```sh
make examples                 # all of them, from aerospike-php/
```

Or one at a time:

```sh
php -d extension=../ext/target/release/libaerospike_php.dylib crud.php
```

They need the daemon running (`make daemon`). Environment:

| | |
| --- | --- |
| `AEROSPIKE_INSTANCE` | daemon instance to attach to (default `default`) |
| `AEROSPIKE_NAMESPACE` | namespace for the example records (default `test`) |
| `AEROSPIKE_SC_NAMESPACE` | strong-consistency namespace, for `transaction.php` |

Every example cleans up the records it wrote.

## They are run as tests

`run-all.php` executes all of them and fails if any does, which is what `make examples`
does. This mirrors the Rust client, whose examples are compiled and executed by the
integration test suite: an example nobody runs rots silently as the API moves, and an
example is the first thing a new user copies.

An example that *cannot* run here prints `SKIPPED` and exits 0 — a transaction without
a strong-consistency namespace, a path expression against a server older than 8.1.1.
The summary counts skips separately, so a skip cannot hide inside a pass:

```text
ok    crud.php                     23 lines of output
skip  transaction.php              namespace 'test' is not strong-consistency; …

12 passed, 1 skipped, 0 failed
```

That distinction matters more than it looks. Transactions need a strong-consistency
namespace, and on a cluster without one the transaction example would otherwise report
success having tested nothing.

## The examples

| | |
| --- | --- |
| [`crud.php`](crud.php) | put, get, touch, header read, exists, operate, delete |
| [`record_operations.php`](record_operations.php) | add, append, prepend, TTL, generation CAS, replace, send-key, bin deletion |
| [`cdt_operations.php`](cdt_operations.php) | list and map operations, and nested documents through a context |
| [`bit_operations.php`](bit_operations.php) | bitwise operations on a blob bin |
| [`batch_operations.php`](batch_operations.php) | reads, writes, deletes and UDF calls in one round trip |
| [`query.php`](query.php) | secondary-index queries: equality, range, paging, expression filters, rate limiting, collection indexes |
| [`scan.php`](scan.php) | whole-set reads, paging, and dividing a scan between workers by partition |
| [`geo_query.php`](geo_query.php) | geospatial queries against a geo2dsphere index |
| [`path_expression.php`](path_expression.php) | JSONPath-style selection over a nested document (server 8.1.1+) |
| [`transaction.php`](transaction.php) | multi-record transactions: commit, abort, and self-rollback (server 8.0+, SC namespace) |
| [`udf.php`](udf.php) | register a Lua module, run it per record, and run it in the background |
| [`server_info.php`](server_info.php) | the info protocol, and the cluster's nodes |
| [`timeout_configuration.php`](timeout_configuration.php) | socket and total timeouts, and where each applies |

## Two of the Rust examples have no counterpart here

**`crud_sync`.** The Rust client is async, so it has a second CRUD example for its
blocking client. From PHP every command is synchronous — the extension sends a request
and waits — so [`crud.php`](crud.php) *is* the blocking example. There is nothing for a
second one to show.

**`query_aggregate`.** Stream-UDF aggregation runs the reduce phase in an embedded Lua
interpreter *inside the client*, which the Rust client compiles in behind its `lua`
feature. The daemon does not expose it: the aggregation would have to run in the
daemon, and its result — an arbitrary Lua value, produced incrementally — is not
something the wire contract carries. Aggregate with `operate` and expressions, or
with a background UDF, both of which run server-side.

## Things the examples turned up, which are worth knowing before you hit them

- **A strong-consistency namespace forbids non-durable deletes** (result code 22,
  `FailForbidden`). Since transactions require an SC namespace, every delete in
  `transaction.php` sets `durableDelete: true`. This is the first thing that bites when
  you point a working script at an SC namespace.
- **Two ways to test a failure, and they are not interchangeable.** A few outcomes are
  promoted to their own `Aerospike\Status` case, and for those `getResultCode()` is
  `null` — `Status::RecordNotFound` is one. Everything else the server rejects is
  `Status::Server` plus a result code. `record_operations.php` shows both.
- **A batch UDF row succeeds against a key that does not exist.** A UDF is server-side
  code and Aerospike runs it either way; only the read and delete rows for that key
  fail. See `batch_operations.php`.
- **`scan()` cannot ask for zero bins.** It takes bin *names*, and an empty array is
  refused with a message telling you to use `Bins::none()` — which only `query()`
  accepts. `scan.php` shows the filterless-query spelling for that case.
- **`CollectionIndex::List` is a method, not a case.** PHP reserves `List`, so the case
  is `CollectionIndex::ListElements`; the method exists for 1.x compatibility. Same
  shape for `IndexType::Text` and `BitResize::AtEnd`.
