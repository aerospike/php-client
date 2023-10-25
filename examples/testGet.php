<?php

$cp = new ClientPolicy();

$client = Aerospike($cp, "127.0.0.1:3000");
$key = new Key("test", "test", "filterKey");
$rp = new ReadPolicy();
$record = $client->get($rp, $key, ["ibin"]);
var_dump($record);

$bin = new Bin("ibin", 100);
$wp = new WritePolicy();
$client->put($wp, $key, [$bin]);

$record = $client->get($rp, $key, ["ibin"]);
var_dump($record);