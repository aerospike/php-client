<?php

use Aerospike\Client;
use Aerospike\Key;
use Aerospike\Bin;
use Aerospike\WritePolicy;
use Aerospike\BatchWritePolicy;
use Aerospike\BatchReadPolicy;
use Aerospike\BatchDeletePolicy;
use Aerospike\Operation;
use Aerospike\BatchWrite;
use Aerospike\BatchRead;
use Aerospike\BatchDelete;
use Aerospike\BatchPolicy;
use Aerospike\ReadPolicy;

$namespace = "test";
$set = "users";
$socket = "/tmp/asld_grpc.sock";

// Connect to Aerospike server
$client = Client::connect($socket);
echo "* Connected to the local daemon: {$client->socket} \n";

// Truncate the set (clear any existing data)
$ip = new InfoPolicy();
$client->truncate($ip, $namespace, $set);
usleep(100);

// Define sample user data
$users = [
    [
        "username" => "john_doe",
        "age" => 30,
        "email" => "john.doe@example.com"
    ],
    [
        "username" => "jane_smith",
        "age" => 25,
        "email" => "jane.smith@example.com"
    ]
];

// Insert initial user data into Aerospike
$wp = new WritePolicy();
foreach ($users as $userData) {
    $key = new Key($namespace, $set, $userData['username']);
    $bins = [
        new Bin("username", $userData['username']),
        new Bin("age", $userData['age']),
        new Bin("email", $userData['email'])
    ];
    $client->put($wp, $key, $bins);
}

// Perform a batch operation to update all users' ages by 1 year
$batchWritePolicy = new BatchWritePolicy();
$ops = [Operation::add(new Bin("age", 1))]; // Increment age by 1
foreach ($users as $userData) {
    $key = new Key($namespace, $set, $userData['username']);
    $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);
    $batchPolicy = new BatchPolicy();
    $client->batch($batchPolicy, [$batchWrite]);
}

// Retrieve and display the modified user records
$rp = new ReadPolicy();
foreach ($users as $userData) {
    $key = new Key($namespace, $set, $userData['username']);
    $recs = $client->get($rp, $key);
    echo "User: " . $userData['username'] . ", New Age: " . $recs->bins["age"] . "\n";
}
