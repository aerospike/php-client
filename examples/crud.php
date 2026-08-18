<?php

/**
 * Client basics: put, get, touch, header read, exists, operate, delete.
 *
 * Port of the Rust client's `crud` example. There is no separate `crud_sync`
 * here — see the note in this directory's README: from PHP every command is
 * synchronous, so this *is* the blocking example.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{Bin, Bins, Key, Op};

$client = example_client();
$ns = example_namespace();
$key = new Key($ns, 'crud', 'demo');
$started = microtime(true);

section('put and get');
$client->put(null, $key, [new Bin('int', 999), new Bin('str', 'Hello, World!')]);
$record = $client->get(null, $key);
out('bins', $record->bins());
out('generation', $record->generation());

section('touch — bump the generation without changing data');
$client->touch(null, $key);
out('generation', $client->get(null, $key)->generation());

section('header only — no bin data crosses the wire');
$header = $client->getHeader(null, $key);
out('bins returned', count($header->bins()));
out('generation still readable', $header->generation());

section('exists');
out('exists', $client->exists(null, $key));

section('operate — several operations, one round trip, atomic');
$result = $client->operate(null, $key, [
    Op::put(new Bin('int', 123)),
    Op::getBin('int'),
]);
out('put then read back', $result->bin('int'));

section('delete');
out('first delete found the record', $client->delete(null, $key));
out('second delete found nothing', $client->delete(null, $key));

printf("\ntotal time: %.1f ms\n", (microtime(true) - $started) * 1000);
