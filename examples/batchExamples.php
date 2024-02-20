<?php

namespace Aerospike;

$namespace = "test";
$set = "test";
$socket = "/tmp/asld_grpc.sock";

// $client = Aerospike($socket);
$client = Client::connect($socket);
echo "* Connected to the local daemon: $client->hosts \n";

$key = new Key($namespace, $set, 1);

$wp = new WritePolicy();
$client->put($wp, $key, [new Bin("bini", 1), new Bin("bins", "b"), new Bin("bin1", [1, 2, 3, 4])]);

$start_time = microtime(true);

$COUNT = 1;

$i = 0;

$rp = new ReadPolicy();
while ($i < $COUNT) {
    $i++;

    $client->add($wp, $key, [new Bin("bini", 1)]);
    $client->append($wp, $key, [new Bin("bins", "a")]);
    $client->prepend($wp, $key, [new Bin("bins", "a")]);

    $record = $client->get($rp, $key);


    $bwp = new BatchWritePolicy();
    $ops = [Operation::put(new Bin("put_op", "put_val"))];
    $bw = new BatchWrite($bwp, $key, $ops);

    $brp = new BatchReadPolicy();
    $br = new BatchRead($brp, $key, []);

    $bdp = new BatchDeletePolicy();
    $bd = new BatchDelete($bdp, $key);

    $bp = new BatchPolicy();
    $recs = $client->batch($bp, [$bw, $br, $bd]);
    echo "#############################################";
    var_dump($recs);

    // $exists = $client->exists($rp, $key);
    // echo "===============\nDoes record exist after delete op? $exists\n";
    // var_dump($exists);

    // $record = $client->get($rp, $key);
    // echo "===============\nRecord?\n";
    // var_dump($record);

    // $client->touch($wp, $key);
    // echo "===============\nTouched\n";


    // $existed = $client->delete($wp, $key);
    // echo "===============\nDeleted. Did it exist? $existed\n";
    // var_dump($existed);

    // $exists = $client->exists($rp, $key);
    // echo "===============\nDoes record exist after delete? $exists\n";
    // var_dump($exists);
}

// echo "===============\n\n";

$end_time = microtime(true);
$total_time = $end_time - $start_time;
$per_tran = ($total_time / $COUNT) * 1000000;

echo " * total time: $total_time s\n * per transaction: $per_tran us\n";

// $wp = new WritePolicy();
// $client->put($wp, $key, [new Bin("bini", 1), new Bin("bins", "b"), new Bin("bin1", [1, 2, 3, 4])]);

// $ip = new InfoPolicy();
// $client->truncate($ip, "test", "test");

// // let truncate to finish
// sleep(5);

// $exists = $client->get($rp, $key);
// echo "===============\nDoes record exist after Truncate? $exists\n";
// var_dump($exists);

// var_dump($record);