How to setup:
* Follow [this guide](https://davidcole1340.github.io/ext-php-rs/getting-started/installation.html) and install PHP 8.1 *from source*.
* Build and run the code via: `cargo build && php -d extension=./target/debug/libaerospike.so test.php`
* Use Aerospike Server v5.7 for testing; The Rust client does not support the newer servers entirely.