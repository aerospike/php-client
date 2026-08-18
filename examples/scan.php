<?php

/**
 * Scans: reading a whole set, with paging and partition subsets.
 *
 * Port of the Rust client's `scan` example. A scan is a query with no filter —
 * `scan()` is the convenience spelling, and `query()` with a filterless
 * `Statement` does the same thing.
 *
 * One difference from the Rust client worth stating: **there is no resumable page
 * token here.** The traversal's position lives in the daemon, as a cursor the
 * `RecordSet` holds a handle to, because a PHP request does not outlive a scan and
 * the daemon does. So a partly consumed scan is resumed by keeping the `RecordSet`,
 * not by persisting a filter and passing it back.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{Bin, Bins, Key, PartitionFilter, QueryPolicy, Statement};

$client = example_client();
$ns = example_namespace();
$set = 'scan_demo';
$records = 300;

section("set up: $records records");
for ($i = 0; $i < $records; $i++) {
    $client->put(null, new Key($ns, $set, $i), [new Bin('n', $i)]);
}
out('written', $records);

section('full scan');
$count = 0;
foreach ($client->scan(null, null, $ns, $set) as $ignored) {
    $count++;
}
out('records', $count);

section('the same thing as a filterless query');
$count = 0;
foreach ($client->query(null, null, new Statement($ns, $set)) as $ignored) {
    $count++;
}
out('records', $count);

section('paged — pageSize caps one round trip, foreach hides the rest');
$paged = new QueryPolicy(pageSize: 100);
$recordSet = $client->scan($paged, null, $ns, $set);
$count = 0;
foreach ($recordSet as $ignored) {
    $count++;
}
out('records over pages of 100', $count);
out('seen() reports progress', $recordSet->seen());

section('metadata only — a count without moving bin data');
// `scan()` takes bin *names*, and an empty array cannot mean "no bins" — the
// extension refuses it and says to use `Bins::none()`, which only `query()` accepts.
// So this is one case where the filterless `query()` spelling is the shorter one.
$count = 0;
foreach ($client->query(null, null, new Statement($ns, $set, Bins::none())) as $ignored) {
    $count++;
}
out('records counted with no bins requested', $count);

section('a subset of partitions — how you divide a scan between workers');
// Aerospike has 4096 partitions. Give each worker a disjoint range and every
// record is visited exactly once, with no coordination between them.
$workers = 4;
$perWorker = intdiv(4096, $workers);
$total = 0;
for ($w = 0; $w < $workers; $w++) {
    $begin = $w * $perWorker;
    $slice = $client->query(null, PartitionFilter::byRange($begin, $perWorker),
        new Statement($ns, $set, Bins::none()));
    $seen = 0;
    foreach ($slice as $ignored) {
        $seen++;
    }
    out("worker $w (partitions $begin.." . ($begin + $perWorker - 1) . ')', $seen);
    $total += $seen;
}
out('total across workers', $total);

section('stopping early releases the cursor');
$recordSet = $client->scan(null, null, $ns, $set);
$seen = 0;
foreach ($recordSet as $ignored) {
    if (++$seen === 5) {
        break;
    }
}
$recordSet->close();
out('read 5 records, then closed', $recordSet->isOpen() ? 'still open' : 'cursor released');

section('cleanup');
$client->truncate(null, $ns, $set);
out('set truncated');
