# Aerospike PHP 8+ Client (v1.0.0)

An [Aerospike](https://www.aerospike.com/) client library for PHP 8+.

## PHP-Client Introduction 

This is the documentation for the Aerospike PHP Client. The PHP client comprises of two essential components. Firstly, we have a robust PHP client written in Rust and a connection manager written in Go, serving as a shared resource among PHP processes. The connection manager daemon efficiently handles all requests and responses between the PHP processes and the Aerospike server.


## Requirements

* PHP v8.1+
* Cargo
* Aerospike server
* Linux or Darwin 
* PHPUnit v10
* Rustc >= v1.74 
* Go Toolchain [Go Toolchains - The Go Programming Language](https://go.dev/doc/toolchain)
* Protobuf Compiler [protoc-gen-go command - google.golang.org/protobuf/cmd/protoc-gen-go - Go Packages](https://pkg.go.dev/google.golang.org/protobuf/cmd/protoc-gen-go)
* ext-php-rs v0.12.0 [github repository link](https://github.com/davidcole1340/ext-php-rs/tree/master)
NOTE: Please see instruction for setting up ext-php-rs for windows [here] 

## Setup

* Clone the PHP-Client repository
```bash https://github.com/aerospike/php-client.git
cd php-client
```

### Setting up the Aerospike client connection manager: 

#### Installing up the dependencies and Running Aerospike Connection manager
1. Make sure the go toolchain has been installed. Download the package from [The Go Programming Language](https://golang.org/dl/). Follow the steps to correctly install Go.
   **NOTE:** Ensure that the PATH variable has been updated with the GOBIN path.
2. Install protobuf compiler:
```bash
   go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
```
3. Change directory into php-client/aerospike-connection-manager 
```bash 
cd php-client/aerospike-connection-manager 
```
4. Build and run the aerospike-connection-manager 
```bash
sudo make
```
**NOTE:** Please view the README.md in the [`php-client/aerospike-connection-manager`](./aerospike-connection-manager/README.md) directory for more information about the setting up the aerospike-connection-manager  and configuring the client policy.

### Build and Install the PHP-Client
* Check the php version 
```bash 
php -v
```
* Install and setup the Aerospike [Aerospike server](https://aerospike.com/download/) 
* To build and install the `PHP-Client` in the default paths run the makefile
```bash
cd php-client
sudo make
```
* To build and install the `PHP-Client` in manually, run the following commands:
```bash 
cd php-client
cargo clean && cargo build --release
```
- Once the build is successful copy the file from `target/release/libaerospike$(EXTENSION)` [$EXTENSION = .so for Linux and .dylib for Mac and Windows] to the PHP extension directory path. 
- Add `extension=libaerospike$(EXTENSION)` to your `php.ini` file. 
- Run  `phpunit tests/` to ensure the setup and build was successful. 

***NOTE***: The Aerospike server must be running for the tests to run successfully. 

### Running your PHP Project

1. Running your PHP script:
  - Before running your script pre-requisites are Aerospike connection manager and Aerospike Server must be running.  
  - Once the build is successful and all the pre-requisites are met, import the Aerospike namespace to your PHP script. 
  - To connect to the Aerospike connection manager add:
```PHP
	$socket = "/tmp/asld_grpc.sock";
	$client = Client::connect($socket); 
```
  - Run the php script
  If there are no Errors then you have successfully connected to the Aerospike DB. 

***NOTE:*** If the connection manager daemon crashes, you will have to manually remove the file `/tmp/asld_grpc.sock` from its path.
```bash 
sudo rm -r /tmp/asld_grpc.soc
```

   - Configuring policies: Configuring Policies (Read, Write, Batch and Info) - These policies can be set via getters and setter in the php code. On creating an object of that policy class, the default values are initialized for that policy. For example: 

```php
	// Instantiate the WritePolicy object
	$writePolicy = new WritePolicy();

	$writePolicy->setRecordExistsAction(RecordExistsAction::Update);
	$writePolicy->setGenerationPolicy(GenerationPolicy::ExpectGenEqual);
	$writePolicy->setExpiration(Expiration::seconds(3600)); // Expiring in 1 hour
	$writePolicy->setMaxRetries(3);
	$writePolicy->setSocketTimeout(5000);
```

## Documentation

* Reference documnetaion can be found [here] (https://aerospike.github.io/php-client/)
* Aerospike Documentation can be found [here](https://aerospike.com/docs/)

## Issues

If there are any bugs, feature requests or feedback -> please create an issue on [GitHub](https://github.com/aerospike/php-client/issues). We will review it. 

## Usage

**The following is a very simple example of CRUD operations in an Aerospike database.**

```php
<?php
namespace Aerospike;

try{
  $socket = "/tmp/asld_grpc.sock";
  $client = Client::connect($socket);
  var_dump($client->socket);
}catch(AerospikeException $e){
  var_dump($e);
}


$key = new Key("namespace", "set_name", 1);

//PUT on differnet types of values
$wp = new WritePolicy();
$bin1 = new Bin("bin1", 111);
$bin2 = new Bin("bin2", "string");
$bin3 = new Bin("bin3", 333.333);
$bin4 = new Bin("bin4", [
	"str", 
	1984, 
	333.333, 
	[1, "string", 5.1], 
	[
		"integer" => 1984, 
		"float" => 333.333, 
		"list" => [1, "string", 5.1]
	] 
]);

$bin5 = new Bin("bin5", [
	"integer" => 1984, 
	"float" => 333.333, 
	"list" => [1, "string", 5.1], 
	null => [
		"integer" => 1984, 
		"float" => 333.333, 
		"list" => [1, "string", 5.1]
	],
	"" => [ 1, 2, 3 ],
]);

$client->put($wp, $key, [$bin1, $bin2, $bin3, $bin4, $bin5]);

//GET
$rp = new ReadPolicy();
$record = $client->get($rp, $key);
var_dump($record->bins);

//UPDATE
$client->prepend($wp, $key, [new Bin("bin2", "prefix_")]);
$client->append($wp, $key, [new Bin("bin2", "_suffix")]);

//DELETE
$deleted = $client->delete($wp, $key);
var_dump($deleted);

$client->close()
```

**Batch Operations Examples:**

```php
<?php

namespace Aerospike;

$namespace = "test";
$set = "test";
$socket = "/tmp/asld_grpc.sock";

$client = Client::connect($socket);
echo "* Connected to the local daemon: $client->hosts \n";

$key = new Key($namespace, $set, 1);

$wp = new WritePolicy();
$client->put($wp, $key, [new Bin("bini", 1), new Bin("bins", "b"), new Bin("bin1", [1, 2, 3, 4])]);

$bwp = new BatchWritePolicy();
$exp = Expression::lt(Expression::intBin("bin1"), Expression::intVal(1));
$batchWritePolicy->setFilterExpression($exp);
$ops = [Operation::put(new Bin("put_op", "put_val"))];
$bw = new BatchWrite($bwp, $key, $ops);

$brp = new BatchReadPolicy();
$br = new BatchRead($brp, $key, []);

$bdp = new BatchDeletePolicy();
$bd = new BatchDelete($bdp, $key);

$bp = new BatchPolicy();
$recs = $client->batch($bp, [$bw, $br, $bd]);
var_dump($recs);

```

For more detailed examples you can see the examples direcotry [php-client/examples](./examples/)