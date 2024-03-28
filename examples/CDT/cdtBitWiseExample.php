<?php

namespace Aerospike;

$socket = "/tmp/asld_grpc.sock";
$namespace = "test";
$set = "test-bit";

$client = Client::connect($socket);
$key = new Key($namespace, $set, "key-bit");
$ip = new InfoPolicy();
$client->truncate($ip, $namespace, $set);

$bit0 = [0x80];
$defaultBitPolicy = new BitwisePolicy(BitwiseWriteFlags::default());
$updateBitPolicy = new BitwisePolicy(BitwiseWriteFlags::updateOnly());

$initial = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
$op1 = BitwiseOp::remove($defaultBitPolicy, "bitBin", 2, 3);

$wp = new WritePolicy();
$bins = [new Bin("bitBin", $initial)];
$client->put($wp, $key, $bins);

$rp = new ReadPolicy();
$record = $client->get($rp, $key);
echo "Record Before: ";
var_dump($record->bins["bitBin"]);
echo "\n";

$bwp = new BatchWritePolicy();
$bp = new BatchPolicy();

$batchWrite = new BatchWrite($bwp, $key, [$op1]);
$client->batch($bp, [$batchWrite]);

$record = $client->get($rp, $key);
echo "Record After: ";
var_dump($record->bins["bitBin"]);
echo "\n";