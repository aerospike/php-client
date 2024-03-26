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
