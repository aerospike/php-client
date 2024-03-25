<?php

namespace Aerospike;

$namespace = "test";
$set = "test";
$socket = "/tmp/asld_grpc.sock";

$client = Client::connect($socket);
echo "* Connected to the local daemon: $client->socket \n";

$key = new Key($namespace, $set, 1);

$bwp = new BatchWritePolicy();
$ops = [Operation::put(new Bin("list", [1, 2, 3, 4])), Operation::put(new Bin("map", ["1" => 1, "2" => 2, "3" => 3, "4" => 4]))];
$bw = new BatchWrite($bwp, $key, $ops);

$brp = new BatchReadPolicy();
$br = new BatchRead($brp, $key, []);
$bp = new BatchPolicy();
$recs = $client->batch($bp, [$bw, $br]);


$lp = new ListPolicy(ListOrderType::unordered());
$mp = new MapPolicy(MapOrderType::unordered());
$ops = [ListOp::append($lp, "list", [999]), MapOp::put($mp, "map", ["999" => 999])];
$bw = new BatchWrite($bwp, $key, $ops);
$recs = $client->batch($bp, [$bw]);
// echo "\n Record after append: ";
// $listBinData = $recs[0]->record->bins;

// echo "\n Count: ".$listBinData["list"];

$rp = new ReadPolicy();
$record = $client->get($rp, $key);
echo "\n Record: ";
$array = $record->bins['list'];
echo "Count: ".count($array);

$lp = new ListPolicy(ListOrderType::unordered());
$ops = [ListOp::removeValues("list", [1, 3])];
$bw = new BatchWrite($bwp, $key, $ops);
$recs = $client->batch($bp, [$bw]);

$rp = new ReadPolicy();
$record = $client->get($rp, $key);
