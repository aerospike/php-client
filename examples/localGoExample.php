<?php

namespace Aerospike;

$namespace = "test";
$set = "test";
$socket = "/tmp/asld_grpc.sock";

$client = Client::connect($socket);
echo "* Connected to the local daemon: $client->hosts \n";
$ip = new InfoPolicy();
$client->truncate($ip, $namespace, $set);
usleep(100);


$key = new Key($namespace, $set, "bins");
$wp = new WritePolicy();
$client->put($wp, $key, [new Bin("bin1", 1), new Bin("bin2", 2)]);

$batchWritePolicy = new BatchWritePolicy();
$exp = Expression::lt(Expression::intBin("bin1"), Expression::intVal(1));
// $batchWritePolicy->setFilterExpression($exp);
$ops = [Operation::put(new Bin("bin3", 3))];
$batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

$batchPolicy = new BatchPolicy();
$client->batch($batchPolicy, [$batchWrite]);

$rp = new ReadPolicy();
$recs = $client->get($rp, $key);
var_dump(count($recs->bins));

