<?php

namespace Aerospike;

$namespace = "test";
$set = "test";
$socket = "/tmp/asld_grpc.sock";

$client = Client::connect($socket);

$bins = [
    new Bin("AerospikeBin1", random_int(0, 1000))
];

$key = new Key("test", "test", "key1");
$wp = new WritePolicy();

var_dump($key->computeDigest());

// $client->put($wp, $key, $bins);


// $pf = PartitionFilter::all();

// $sp = new ScanPolicy();
// $sp->maxRecords = 5;

// $rs = $client->scan($sp, $pf, "test", "test");

// $count = 0;
// while ($rec = $rs->next()) {
//     $count++;
// 	var_dump($rec);
// }

// echo "Count: ",$count;