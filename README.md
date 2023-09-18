## This project is pre-alpha, and should not be used in production. If you're an enterprise customer feel free to reach out to our support with feedback and feature requests. We appreciate feedback from the Aerospike community on issues related to the new PHP client.

# Aerospike PHP@8.1 Client

An [Aerospike](https://www.aerospike.com/) client library for PHP@8.1.

## Requirements

* PHP 8.1
* Composer
* cargo
* Aerospike server v5.7+ 

### NOTE: Does not support Aerospike server 6.4 features.
### NOTE: Does not support Windows platform.

## Setup

### Installation via Composer

* Add the following in the 'require' section of composer.json
    ``` "aerospike/aerospike-php": "v0.1.0-alpha1" ```
* Run ```composer upgrade```
* ```cd vendor/aerospike/aerospike-php && sudo composer install```

### Manual Installation:
* Follow [this guide](https://davidcole1340.github.io/ext-php-rs/getting-started/installation.html) and install PHP 8.1 *from source*.
* Clone the repository ```git clone https://github.com/aerospike/php-client.git```
* ```cd php-client```
* Build and run the test code via: `cargo build && php -d extension=./target/debug/libaerospike.so test.php` for linux or `cargo build && php -d extension=./target/debug/libaerospike.dylib test.php` for darwin
* Add the extension file[.dylib or .so] to the php.ini file and move the extension file to your local php extesnion dir [ex: /usr/lib/php/20210902]
* To clean your repository you can run ```cargo clean```
### NOTE: Use Aerospike Server v5.7 for testing; The Rust client does not support the newer servers entirely.



## Documentation
* Php stubs and API documentation can be found [here](https://github.com/aerospike/php-client/blob/php-rs/php_code_stubs/php_stubs.php)
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





