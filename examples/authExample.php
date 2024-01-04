<?php

namespace Aerospike;

/* This example needs to have security enbaled in aerospike.conf.
For more info please visit  - "https://docs.aerospike.com/server/operations/configure/security/access-control"
*/

$iterations = 100;
$startTime = microtime(true);
$cp = new ClientPolicy();

for ($i = 0; $i < $iterations; $i++) {

    $cp->setUser("admin");
    $cp->setPassword("admin");
    $client = Aerospike($cp, "127.0.0.1:3000");
}

$connected = $client->isConnected();

$endTime = microtime(true);
$duration = $endTime - $startTime;
echo "Duration for setting user and password {$iterations} times: {$duration} seconds\n";

var_dump($connected);