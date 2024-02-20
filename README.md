## This project is beta, and should not be used in production. If you're an enterprise customer feel free to reach out to our support with feedback and feature requests. We appreciate feedback from the Aerospike community on issues related to the new PHP client.

# Aerospike PHP 8+ Client

An [Aerospike](https://www.aerospike.com/) client library for PHP 8+.

## Requirements

* PHP 8.1+
* Cargo
* Aerospike server
* Linux or Darwin 
* PHPUnit v10
* Rustc v1.74 =<
* Go Toolchain [Go Toolchains - The Go Programming Language](https://go.dev/doc/toolchain)
* Protobuf Compiler [protoc-gen-go command - google.golang.org/protobuf/cmd/protoc-gen-go - Go Packages](https://pkg.go.dev/google.golang.org/protobuf/cmd/protoc-gen-go)
* ext-php-rs v0.12.0 [github repository link](https://github.com/davidcole1340/ext-php-rs/tree/master)

## Current Limitations

* Does not support Scan/Query API features/
* Does not support CDTs

## Setup

* Clone the PHP-Client repository
```bash https://github.com/aerospike/php-client.git
cd php-client
```

### Setting up the Go Local Daemon: 

1. Make sure the go toolchain has been installed. Download the package from [The Go Programming Language](https://golang.org/dl/). Follow the steps to correctly install Go.
   **NOTE:** Ensure that the PATH variable has been updated with the GOBIN path.
2. Install protobuf compiler:
   ```bash
   go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
   ```
3. Change directory into php-client/daemon 
```bash 
cd php-client/daemon
```
4. Build and run the go local daemon
```bash
sudo make
```
**NOTE:** Please view the README.md in the `php-client/daemon` directory for more information about the setting up the go local daemon and configuring the client policy.

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
  - Before running your script pre-requisites are Go Local Daemon and Aerospike Server 6.3 server must be running.  
  - Once the build is successful and all the pre-requisites are met, import the Aerospike namespace to your PHP script. 
  - To connect to the local Go daemon add:
```php
$socket = "/tmp/asld_grpc.sock";
$client = Client::connect($socket); 
```
  - Run the php script
  If there are no Errors then you have successfully connected to the Aerospike Db. 

## Documentation

* Aerospike Documentation can be found [here](https://aerospike.com/docs/)

## Issues

If there are any issues, please create an issue on [GitHub](https://github.com/aerospike/php-client/issues).

## Usage

**The following is a very simple example of CRUD operations in an Aerospike database.**

```php
<?php
namespace Aerospike;

try{
  $socket = "/tmp/asld_grpc.sock";
  $client = Client::connect($socket);
  var_dump($client->hosts);
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

For more detailed examples you can see the examples direcotry [php-client/examples]