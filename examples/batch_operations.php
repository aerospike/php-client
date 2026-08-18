<?php

/**
 * Batch reads, writes, deletes and UDF calls — many records, one round trip.
 *
 * Port of the Rust client's `batch_operations` example. The rows of a batch need
 * not be the same kind of work, or against the same namespace.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{BatchDelete, BatchRead, BatchResult, BatchUdf, BatchWrite, Bin, Bins, Key, Op};

$client = example_client();
$ns = example_namespace();
$set = 'batch_ops';

$udf = <<<'LUA'
function echo(rec, val)
    return val
end
LUA;

section('register a UDF for the batch UDF rows');
$client->registerUdf(null, $udf, 'batch_example.lua')->waitTillComplete(10_000);
out('registered batch_example.lua');

$keys = [
    new Key($ns, $set, 1),
    new Key($ns, $set, 2),
    new Key($ns, $set, 3),
    new Key($ns, $set, -1),   // deliberately never written
];

/** Print one result per row, so a failed row is visible as a failed row. */
function show(array $results): void
{
    foreach ($results as $i => $row) {
        /** @var BatchResult $row */
        if (!$row->isOk()) {
            out("row $i", "failed, result code {$row->resultCode()}");
            continue;
        }
        $record = $row->record();
        out("row $i", $record === null ? 'ok, no record' : $record->bins());
    }
}

$writeOps = [Op::put(new Bin('a', 'a value')), Op::put(new Bin('b', 'another')), Op::put(new Bin('c', 42))];

// A write row's record is the *operation results*, one per op — and a plain `put`
// has no result, so these come back as nulls. That is not an empty record: it is
// three successful writes reporting nothing to return.
section('batch write');
show($client->batch(null, [
    BatchWrite::ops($keys[0], $writeOps),
    BatchWrite::ops($keys[1], $writeOps),
    BatchWrite::ops($keys[2], $writeOps),
]));

section('batch read — every row a different bin selection');
show($client->batch(null, [
    BatchRead::some($keys[0], ['a']),                 // named bins
    BatchRead::all($keys[1]),                         // every bin
    BatchRead::header($keys[2]),                      // metadata only
    BatchRead::ops($keys[2], [Op::getBin('a'), Op::getBin('b')]),
    BatchRead::all($keys[3]),                         // no such record
]));

section('batch UDF');
show($client->batch(null, [
    BatchUdf::call($keys[0], 'batch_example', 'echo', [1]),
    BatchUdf::call($keys[1], 'batch_example', 'echo', [2]),
    // The key that was never written. This row *succeeds*: a UDF is server-side
    // code, and Aerospike runs it whether or not the record exists — `echo` never
    // touches the record, so it returns its argument. Compare the read and delete
    // rows for the same key, which do fail with result code 2.
    BatchUdf::call($keys[3], 'batch_example', 'echo', [4]),
]));

section('batch delete');
show($client->batch(null, [
    BatchDelete::key($keys[0]),
    BatchDelete::key($keys[1]),
    BatchDelete::key($keys[2]),
    BatchDelete::key($keys[3]),   // no such record
]));

section('cleanup');
$client->removeUdf(null, 'batch_example.lua')->waitTillComplete(10_000);
out('removed batch_example.lua');
