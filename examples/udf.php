<?php

/**
 * User-defined functions: register, run against one record, and run in the
 * background over a whole set.
 *
 * Port of the Rust client's `udf` example.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{Bin, Key, Statement};

$client = example_client();
$ns = example_namespace();

$module = <<<'LUA'
function double_bin(rec, name)
    rec[name] = rec[name] * 2
    aerospike:update(rec)
end

function echo(rec, val)
    return val
end
LUA;

section('register the module');
// Registering is cluster-wide and asynchronous: the server accepts the module and
// then distributes it, so the task is what tells you it is everywhere. The waiting
// happens in this process — nothing is held open on the daemon.
$client->registerUdf(null, $module, 'example_udf.lua')->waitTillComplete(10_000);
out('registered example_udf.lua');
// `listUdf()` returns every module on the cluster, which on a shared development
// cluster is a long list — so check for ours rather than printing all of them.
$modules = array_map(fn($m) => $m->name(), $client->listUdf(null));
out('modules on the cluster', count($modules));
out('ours is among them', in_array('example_udf.lua', $modules, true));

section('run against one record');
$key = new Key($ns, 'udf_demo', 'record-1');
$client->put(null, $key, [new Bin('n', 21)]);
$client->executeUdf(null, $key, 'example_udf', 'double_bin', ['n']);
out('double_bin(21)', $client->get(null, $key)->bin('n'));

// A UDF can also return a value rather than modify the record.
out('echo("pong")', $client->executeUdf(null, $key, 'example_udf', 'echo', ['pong']));

section('run in the background over a whole set');
$backgroundSet = 'udf_demo_bg';
for ($i = 0; $i < 10; $i++) {
    $client->put(null, new Key($ns, $backgroundSet, $i), [new Bin('n', $i)]);
}
// Nothing comes back: the work happens on the server, record by record, and the
// task reports when it has finished.
$client->queryExecuteUdf(null, new Statement($ns, $backgroundSet),
    'example_udf', 'double_bin', ['n'])->waitTillComplete(30_000);
out('background UDF applied to every record');
out('record 7 (was 7)', $client->get(null, new Key($ns, $backgroundSet, 7))->bin('n'));

section('cleanup');
$client->removeUdf(null, 'example_udf.lua')->waitTillComplete(10_000);
$client->delete(null, $key);
$client->truncate(null, $ns, $backgroundSet);
out('module removed, records deleted');
