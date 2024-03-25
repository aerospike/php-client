<?php

namespace Aerospike;

$namespace = "test";
$set = "test";
$socket = "/tmp/asld_grpc.sock";

$client = Client::connect($socket);
echo "* Connected to the local daemon: $client->socket \n";

$ap = new AdminPolicy();
$client->createUser($ap, "user1", "password", ["read-write"]);