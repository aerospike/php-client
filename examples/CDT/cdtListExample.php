<?php

namespace Aerospike;

$socket = "/tmp/asld_grpc.sock";
$namespace = "test";
$set = "test";

$client = Client::connect($socket);
$key = new Key($namespace, $set, 1);
$ip = new InfoPolicy();
$client->truncate($ip, $namespace, $set);

$list = array();
$bwp = new BatchWritePolicy(); 
$lp = new ListPolicy(ListOrderType::unordered());
$bp = new BatchPolicy();

for ($i = 1; $i <= 10; $i++) {
    $list[] = $i;
    $ops = [ListOp::append($lp, "listBin", [$i])];
    $bw = new BatchWrite($bwp, $key, $ops);
    $client->batch($bp, [$bw]);
}
usleep(100000);
$rp = new ReadPolicy();
$record = $client->get($rp, $key);
var_dump($record->bins);

$listOp = ListOp::getByIndexRange("listBin", 1);
$opsGetSize = [$listOp]; 
$brp = new BatchReadPolicy();
$br = BatchRead::ops($brp, $key, $opsGetSize);
$bw1 = new BatchWrite($bwp, $key, $opsGetSize);
$recs = $client->batch($bp, [$br]);
var_dump($recs[0]->record->bins);

