<?php

namespace Aerospike;

/* This example needs to have security enbaled in aerospike.conf.
For more info please visit  - "https://docs.aerospike.com/server/operations/configure/security/access-control"
*/

$cp = new ClientPolicy();

$cp->setUser("admin");
$cp->setPassword("admin");

$client = Aerospike($cp, "127.0.0.1:3000");
$connected = $client->isConnected();

var_dump($connected);