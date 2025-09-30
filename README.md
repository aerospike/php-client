[![PHP version](https://img.shields.io/badge/php-%3E%3D%208.1-8892BF.svg)](https://github.com/aerospike/php-client)
# Aerospike PHP 8 Client (v1.3.0)

An [Aerospike](https://www.aerospike.com/) client library for PHP 8

## PHP-Client Introduction

This is the documentation for the Aerospike `PHP-Client`. The `PHP-Client` comprises of two essential components: 
* the client itself, written in Rust as a PHP extension
* the connection manager (the "Aerospike Connection Manager" or "ACM") written in Go which serves as a shared resource among PHP processes. The ACM efficiently handles all requests and responses between the PHP processes and the Aerospike server, and can be configured to run as a daemonized service.

## Dependencies
***NOTE:*** Any missing dependencies will be installed by the installation script

* PHP (v8.1-8.4)
* Cargo (Rust package manager)
* Aerospike server
* Linux or MacOS (Darwin)
* PHPUnit
* rustc (Rust compiler) >= v1.74
* Go Toolchain [Go Toolchains - The Go Programming Language](https://go.dev/doc/toolchain)
* Protobuf Compiler [protoc-gen-go command - google.golang.org/protobuf/cmd/protoc-gen-go - Go Packages](https://pkg.go.dev/google.golang.org/protobuf/cmd/protoc-gen-go)
* ext-php-rs (PHP extension) v0.13.1 [github repository link](https://github.com/davidcole1340/ext-php-rs/tree/master)

## Build & Installation
There are 2 ways to build and install the `PHP-Client`:
1. direct script download and execution (also clones the repo for you)
2. manually clone the repo first and then run script from there

The install script builds both the `PHP-Client` and the ACM, as well as installing all of the dependencies.
## Automatic Method: Direct download and execution of installation script:
The installation script will clone the repo into a subfolder so execute this command directly above where you want the repo to go

For MacOS (Darwin):
```shell
curl -O https://raw.githubusercontent.com/aerospike/php-client/refs/heads/main/build/install_as_php_client_mac.zsh; chmod +x install_as_php_client_mac.zsh; ./install_as_php_client_mac.zsh
```
***NOTE***: the default MacOS installation is an all user-local installation, requiring no root or sudo access

For Linux:
```shell
curl -O https://raw.githubusercontent.com/aerospike/php-client/refs/heads/main/build/install_as_php_client_linux.sh; chmod +x install_as_php_client_linux.sh; sudo ./install_as_php_client_linux.sh
```
### Manual method: repo clone followed by execution of the installation script:
1. Clone the repo
2. Run the installation script for your system:

    for MacOS (darwin):
	```shell
	. ./php-client/build/install_as_php_client_mac.zsh
	 ```
	 or for linux:
	```shell
	sudo ./php-client/build/install_as_php_client_linux.sh
	 ```

***NOTE***: the default linux installation contains system-wide installations and will require root / sudo access 

After the installation script completes, re-source your shell env config files to make sure your `PATH` is updated.  Eg, on MacOS:
```shell
. ~/.zshrc
```

If you encounter errors during installation, you can try running the install script again as the install scripts attempt to be idempotent.  As a last resort, the script commands can be run manually one-by-one as needed.



### Configuring the Aerospike Connection Manager:

***NOTE:*** Please view the README.md in the [`php-client/aerospike-connection-manager`](./aerospike-connection-manager/README.md) directory for more information about the setting up the aerospike-connection-manager  and configuring the client policy.

***NOTE:*** You should have an [Aerospike server](https://aerospike.com/download/) up and running to test against.

### Manual Build and Install of the PHP-Client (optional)

In case the installation script fails to build the `PHP-Client`, or if you just want to run specific build commands, you may do so manually:
* To manually build and install the `PHP-Client` in the default paths run the makefile

	Note: sudo is only needed when running a system-wide php installation, which is not the default install for MacOS (although it could be)
	```shell
	cd php-client
	make
	```
* To build and install the `PHP-Client` in manually, run the following commands:
	```shell 
	cd php-client
	cargo clean && cargo build --release
	```

- Once the build is successful, copy the file from `target/release/libaerospike$(EXTENSION)` [$EXTENSION = .so for Linux and .dylib for Mac and Windows] to the PHP extension directory path. 
- Add `extension=libaerospike$(EXTENSION)` to your `php.ini` file. 
- Run  `phpunit tests/` to ensure the setup and build were successful. 

***NOTE***: The Aerospike server must be running for the tests to run successfully. 

### Running your PHP Project

  - Before running your project PHP scripts, the following must be running:
  	- An Aerospike Connection Manager (ACM)
	- An Aerospike Server that the ACM can connect to 
  - Once the build is successful and all the pre-requisites are met, import the Aerospike namespace to your PHP script:
	```PHP
	namespace Aerospike;
	```
  - To connect to the Aerospike server via the ACM add:
	```PHP
	$socket = "/tmp/asld_grpc.sock";
	$client = Client::connect($socket); 
	```
  - Run the php script
  If there are no Errors then you have successfully connected to the Aerospike DB. 

	***NOTE:*** If the connection manager daemon crashes, you will have to manually remove the file `/tmp/asld_grpc.sock` from its path.
	```shell
	sudo rm -r /tmp/asld_grpc.sock
	```

  - Policy Configuration (Read, Write, Batch and Info) - These policies can be set via getters and setter in the php code. On creation of an object of that policy class (eg, WritePolicy), the default values are initialized for that policy & can be overidden with associated setters. For example: 

	```php
	// Instantiate the WritePolicy object
	$writePolicy = new WritePolicy();

	$writePolicy->setRecordExistsAction(RecordExistsAction::Update);
	$writePolicy->setGenerationPolicy(GenerationPolicy::ExpectGenEqual);
	$writePolicy->setExpiration(Expiration::Seconds(3600)); // Expiring in 1 hour
	$writePolicy->setMaxRetries(3);
	$writePolicy->setSocketTimeout(5000);
	```

## Documentation

* Reference Documentation can be found [here] (https://aerospike.github.io/php-client/)
* Aerospike Documentation can be found [here](https://aerospike.com/docs/)

## Issues

If there are any bugs, feature requests or feedback -> please create an issue on [GitHub](https://github.com/aerospike/php-client/issues). Issues will be regularly reviewed by the Aerospike Client Engineering Team.

## Usage

**The following is a very simple example of CRUD operations in an Aerospike database.**

```php
<?php
namespace Aerospike;

try {
  $socket = "/tmp/asld_grpc.sock";
  $client = Client::connect($socket);
  var_dump($client->socket);
}
catch(AerospikeException $e) {
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
