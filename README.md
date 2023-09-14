# Aerospike PHP@8.1 Client

An [Aerospike](https://www.aerospike.com/) client library for PHP@8.1.

## Requirements

* PHP 8.1
* Composer
* cargo
* Aerospike server v5.7+ 

## Setup

How to setup:
* Follow [this guide](https://davidcole1340.github.io/ext-php-rs/getting-started/installation.html) and install PHP 8.1 *from source*.
* Build and run the code via: `cargo build && php -d extension=./target/debug/libaerospike.so test.php` for linux or `cargo build && php -d extension=./target/debug/libaerospike.dylib test.php` for darwin
* Use Aerospike Server v5.7 for testing; The Rust client does not support the newer servers entirely.

## Documentation
* Php stubs and documentation can be found [here](https://github.com/aerospike/php-client/blob/php-rs/php_code_stubs/php_stubs.php)
* GeoFilter examples can be found [here](https://github.com/aerospike/php-client/php-rs/blob/examples/geoQueryFilter.php)

## Usage
The following is a very simple example of CRUD operations in an Aerospike database.

```php

<?
$cp = new ClientPolicy();
$client = Aerospike($cp, "127.0.0.1:3000");
$key = new Key("test", "test", 1);

$wp = new WritePolicy();
$bin1 = new Bin("bin1", 111);
$client->put($wp, $key, [$bin1]);

$client->prepend($wp, $key, [new Bin("bin2", "prefix_")]);

$client->append($wp, $key, [new Bin("bin2", "_suffix")]);

$rp = new ReadPolicy();
$record = $client->get($rp, $key);
var_dump($record->bins);

$deleted = $client->delete($wp, $key);
var_dump($deleted);

$exists = $client->exists($wp, $key);
var_dump($exists);

$client->close();

```





