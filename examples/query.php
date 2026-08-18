<?php

/**
 * Secondary-index queries: equality, range, metadata-only, expression filters,
 * rate limiting and collection indexes.
 *
 * Port of the Rust client's `query` example. A query needs its index to exist —
 * a filter on an unindexed bin is reported as the server's result code 201, and
 * never silently becomes a scan.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{Bin, Bins, CollectionIndex, Exp, Filter, IndexType, Key, QueryPolicy, Statement};

$client = example_client();
$ns = example_namespace();
$set = 'query_demo';
$bin = 'n';
$index = 'query_demo_n_idx';

section('set up: 100 records and a numeric index');
for ($i = 0; $i < 100; $i++) {
    $client->put(null, new Key($ns, $set, $i), [new Bin($bin, $i), new Bin('name', "record-$i")]);
}
// Dropping first makes the example repeatable; a missing index throws, so ignore it.
try {
    $client->dropIndex(null, $ns, $set, $index)->waitTillComplete(10_000);
} catch (Aerospike\AerospikeException) {
}
$client->createIndexOnBin(null, $ns, $set, $bin, $index, IndexType::Numeric)
       ->waitTillComplete(20_000);
out('100 records written, index built');

/** Count what a query returns, and show the first few. */
function summarise(Aerospike\RecordSet $records, int $show = 3): void
{
    $count = 0;
    $sample = [];
    foreach ($records as $record) {
        if ($count < $show) {
            $sample[] = $record->bin('n');
        }
        $count++;
    }
    out('records', $count);
    out('first few n values', $sample);
}

section('1. equality');
summarise($client->query(null, null, new Statement($ns, $set, null, Filter::equal($bin, 5))));

section('2. range');
summarise($client->query(null, null, new Statement($ns, $set, null, Filter::range($bin, 0, 9))));

section('3. metadata only — no bin data crosses the wire');
$records = $client->query(null, null, new Statement($ns, $set, Bins::none(), Filter::range($bin, 0, 4)));
$count = 0;
foreach ($records as $record) {
    $count++;
    if ($count === 1) {
        out('first record: bins', count($record->bins()));
        out('           generation', $record->generation());
    }
}
out('records', $count);

section('4. paging — pageSize caps one round trip, foreach hides the rest');
$paged = new QueryPolicy(pageSize: 25);
$records = $client->query($paged, null, new Statement($ns, $set, null, Filter::range($bin, 0, 99)));
$count = 0;
foreach ($records as $ignored) {
    $count++;
}
out('records over pages of 25', $count);
out('note', 'the cursor lives in the daemon; there is no page token to persist here');

section('5. expression filter — applied by the server, on top of the index');
$filtered = new QueryPolicy(filterExp: Exp::eq(Exp::intBin($bin), Exp::intVal(10)));
summarise($client->query($filtered, null, new Statement($ns, $set, null, Filter::range($bin, 0, 99))));

section('6. rate limiting — recordsPerSecond throttles the server');
$limited = new QueryPolicy(recordsPerSecond: 20);
$started = microtime(true);
$records = $client->query($limited, null, new Statement($ns, $set, null, Filter::range($bin, 0, 39)));
$count = 0;
foreach ($records as $ignored) {
    $count++;
}
printf("  %d records in %.2fs at 20/s\n", $count, microtime(true) - $started);

section('7. collection index — matching an element inside a list bin');
$listSet = 'query_demo_list';
$listIndex = 'query_demo_tags_idx';
for ($i = 0; $i < 30; $i++) {
    // Tag 7 appears in every third record.
    $tags = $i % 3 === 0 ? [$i + 100, 7] : [$i + 100];
    $client->put(null, new Key($ns, $listSet, $i), [new Bin('tags', $tags)]);
}
try {
    $client->dropIndex(null, $ns, $listSet, $listIndex)->waitTillComplete(10_000);
} catch (Aerospike\AerospikeException) {
}
// The index is over the list's ELEMENTS, which is what `ListElements` means. (The
// case is not called `List`: PHP reserves that word. `CollectionIndex::List()` exists
// as a 1.x-compatible *method*, which is a different thing from the case.)
$client->createIndexOnBin(null, $ns, $listSet, 'tags', $listIndex,
    IndexType::Numeric, CollectionIndex::ListElements)->waitTillComplete(20_000);

// The collection type on the filter must match the one the index was built with.
$records = $client->query(null, null,
    new Statement($ns, $listSet, null, Filter::equal('tags', 7, CollectionIndex::ListElements)));
$count = 0;
foreach ($records as $ignored) {
    $count++;
}
out('records whose tag list contains 7', $count);

section('cleanup');
$client->dropIndex(null, $ns, $set, $index)->waitTillComplete(10_000);
$client->dropIndex(null, $ns, $listSet, $listIndex)->waitTillComplete(10_000);
$client->truncate(null, $ns, $set);
$client->truncate(null, $ns, $listSet);
out('indexes dropped, sets truncated');
