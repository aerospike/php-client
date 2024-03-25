<?php

namespace Aerospike;

$socket = "/tmp/asld_grpc.sock";
$namespace = "test";
$set = "mapCDTTest";

$client = Client::connect($socket);
$key = new Key($namespace, $set, 1);
$ip = new InfoPolicy();
$client->truncate($ip, $namespace, $set);

$bwp = new BatchWritePolicy(); 
$bp = new BatchPolicy();
$mp = new MapPolicy(MapOrderType::unordered());
$ops = [Operation::put(new Bin("map", [1 => 1, 2 => 2, 3 => 3, 4 => 4]))];
$bw = new BatchWrite($bwp, $key, $ops);

// $ops = [MapOp::put($mp, "map", ["mk1" => 1, "mk2" => 2])];
// $bw = new BatchWrite($bwp, $key, $ops);
$recs = $client->batch($bp, [$bw]);
// var_dump($recs[0]->record->bins);

$brp = new BatchReadPolicy();
$ops = [MapOp::getByValueRange($mp, "map", 1, 3)];
$br = BatchRead::ops($brp, $key, $ops);
$recs = $client->batch($bp, [$br]);
var_dump($recs[0]->record->bins);