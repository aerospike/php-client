<?php

use Aerospike\Client;
use Aerospike\WritePolicy;
use Aerospike\QueryPolicy;
use Aerospike\Bin;
use Aerospike\Key;
use Aerospike\PartitionFilter;
use Aerospike\Filter;
use Aerospike\Statement;
use Aerospike\IndexType;


$socket = "/tmp/asld_grpc.sock";
// Establish connection to Aerospike server
$client = Client::connect($socket);

// Define namespace and set
$namespace = "test";
$set = "products";

// Define bins (attributes) for product data
$productBins = [
    new Bin("name", "Smartphone"),
    new Bin("price", 599.99),
    new Bin("stock", 100)
];

// Define write policy for adding data
$writePolicy = new WritePolicy();
$writePolicy->setSendKey(true);
$pf = PartitionFilter::all();

// Add sample product data to the server
for ($i = 1; $i <= 10; $i++) {
    $key = new Key($namespace, $set, "product_" . $i);
    $client->put($writePolicy, $key, $productBins);
}

// Define index name
$indexName = $set . "_price_index";

// Create index on 'price' bin
$client->createIndex($writePolicy, $namespace, $set, "price", $indexName, IndexType::Numeric());
usleep(100000);

// Define query policy
$queryPolicy = new QueryPolicy();

// Define filter for the query (e.g., retrieve products with a price less than $500)
$priceFilter = Filter::Range("price", 0, 700);

// Create statement with namespace, set, and filter
$statement = new Statement($namespace, $set, $priceFilter);

// Perform the query
$queryResultSet = $client->query($queryPolicy, $pf, $statement);

// Iterate over the query results
while ($record = $queryResultSet->next()) {
    // Access bin values of each product
    $productName = $record->bins["name"];
    $productPrice = $record->bins["price"];
    $productStock = $record->bins["stock"];
    
    // Display retrieved product data
    echo "Product: $productName, Price: $productPrice, Stock: $productStock\n";
}

// Close the query result set after processing
$queryResultSet->close();