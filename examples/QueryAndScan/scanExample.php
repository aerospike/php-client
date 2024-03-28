<?php

use Aerospike\Client;
use Aerospike\WritePolicy;
use Aerospike\ScanPolicy;
use Aerospike\Bin;
use Aerospike\Key;
use Aerospike\PartitionFilter;


$socket = "/tmp/asld_grpc.sock";
// Establish connection to Aerospike server
$client = Client::connect($socket);

// Define namespace and set
$namespace = "test";
$set = "users";

// Define bins (attributes) for user data
$userBins = [
    new Bin("username", "john_doe"),
    new Bin("email", "john.doe@example.com"),
    new Bin("age", 30)
];

// Define write policy for adding data
$writePolicy = new WritePolicy();
$writePolicy->sendKey = true; // Ensure server returns the key upon write

// Add sample user data to the server
$userKeys = [];
for ($i = 0; $i < 5; $i++) {
    $key = new Key($namespace, $set, "user_" . $i);
    $client->put($writePolicy, $key, $userBins);
    $userKeys[] = $key; // Store keys for later retrieval
}

// Define bins to retrieve during scan
$scanBins = ["username", "email", "age"];

// Define scan policy
$scanPolicy = new ScanPolicy();
$pf = PartitionFilter::all();

// Perform scan
$recordSet = $client->scan($scanPolicy, $pf, $namespace, $set, $scanBins);

// Iterate over scan results
while ($record = $recordSet->next()) {
    // Access bin values of each record
    $username = $record->bins["username"];
    $email = $record->bins["email"];
    $age = $record->bins["age"];
    
    // Display retrieved data
    echo "Username: $username, Email: $email, Age: $age\n";
}

// Close record set after processing
$recordSet->close();
