# Go parameters
hosts ?= ""
host ?= localhost
port ?= 3000 
user ?= ""
pass ?= ""
ns ?= "test"

.PHONY: test clean install
all: lint build test clean

lint:
	cargo clippy

build:
	cargo build

test: build
	php -d extension=./target/debug/libaerospike.so test.php

clean:
	cargo clean

install: build
	sudo cp -f target/debug/libaerospike.so /usr/lib/php/20210902/
	sudo systemctl restart php8.1-fpm
	sudo systemctl restart apache2
