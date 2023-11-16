<?php

namespace Aerospike;

$cp = new ClientPolicy();

$cp->setUser("admin");
$cp->setPassword("admin");

$client = Aerospike($cp, "127.0.0.1:3000");
$connected = $client->isConnected();

var_dump($connected);