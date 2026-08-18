<?php

/**
 * List and map (CDT) operations, including nested documents.
 *
 * Port of the Rust client's `cdt_operations` example, which ports the Java
 * client's OperateList, OperateMap and ListMap examples.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{Bin, Ctx, Key, ListOp, ListReturn, MapOp, MapReturn, Op, OrderedMap};

$client = example_client();
$ns = example_namespace();
$key = new Key($ns, 'cdt_ops', 'demo');
$client->delete(null, $key);

section('lists');
$client->put(null, $key, [new Bin('scores', [10, 20, 30])]);

// Append two values and read the resulting size — one atomic operate call.
$result = $client->operate(null, $key, [
    ListOp::appendItems('scores', [40, 50]),
    ListOp::size('scores'),
]);
out('append then size', $result->bin('scores'));

// Rank -1 is the highest-ranked (largest) element.
$result = $client->operate(null, $key, [ListOp::getByRank('scores', -1, ListReturn::Values)]);
out('highest ranked value', $result->bin('scores'));

$result = $client->operate(null, $key, [
    ListOp::removeByIndexRange('scores', 0, 2, ListReturn::Values),
    Op::getBin('scores'),
]);
out('after removing the first two', $result->bin('scores'));

section('maps');
$client->operate(null, $key, [
    MapOp::putItems('counters', ['alpha' => 1, 'beta' => 2, 'gamma' => 3]),
]);

$result = $client->operate(null, $key, [
    MapOp::incrementValue('counters', 'beta', 40),
    MapOp::getByKey('counters', 'beta', MapReturn::Value),
]);
out('beta after +40', $result->bin('counters'));

$result = $client->operate(null, $key, [MapOp::getByRank('counters', -1, MapReturn::KeyValue)]);
out('highest entry', $result->bin('counters'));

section('nested documents — a context names the path');
// { "prices": [1, 2, 3], "meta": { "owner": "alice" } }
$client->put(null, $key, [new Bin('doc', new OrderedMap([
    'prices' => [1, 2, 3],
    'meta' => new OrderedMap(['owner' => 'alice']),
]))]);

// Append 4 to doc["prices"] — the operation targets the bin, the context the path.
$client->operate(null, $key, [
    ListOp::append('doc', 4)->context([Ctx::mapKey('prices')]),
]);
$result = $client->operate(null, $key, [
    ListOp::get('doc', 3)->context([Ctx::mapKey('prices')]),
]);
out('doc["prices"][3]', $result->bin('doc'));

$result = $client->operate(null, $key, [
    MapOp::getByKey('doc', 'owner', MapReturn::Value)->context([Ctx::mapKey('meta')]),
]);
out('doc["meta"]["owner"]', $result->bin('doc'));

// A list index works the same way on an outer list.
$client->put(null, $key, [new Bin('matrix', [[1, 2], [3, 4]])]);
$result = $client->operate(null, $key, [
    ListOp::getByIndex('matrix', 0, ListReturn::Values)->context([Ctx::listIndex(1)]),
]);
out('matrix[1][0]', $result->bin('matrix'));

out('final record', $client->get(null, $key)->bins());
$client->delete(null, $key);
