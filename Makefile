# Go parameters
hosts ?= ""
host ?= localhost
port ?= 3000 
user ?= ""
pass ?= ""
ns ?= "test"

# Determine the operating system
UNAME_S := $(shell uname -s)
PHP_API_VERSION := $(shell php -i | grep 'PHP API' | awk -F' => ' '{print $$2}')
PHP_VERSION := $(shell php -v | head -n 1 | awk '{print $$2}' | cut -d. -f1,2)

# Check if PHP version is greater than 8.0
ifeq ($(shell awk 'BEGIN{ print ("$(PHP_VERSION)" >= "8.0") }'), 0)
    $(error PHP version must be greater than or equal to 8.0)
endif


ifeq ($(UNAME_S),Darwin)
    EXTENSION := .dylib
	EXT_DIR_PATH := /opt/homebrew/opt/php@$(PHP_VERSION)/pecl/$(PHP_API_VERSION)
    PHP_INI_PATH := /opt/homebrew/etc/php/$(PHP_VERSION)/php.ini
    RESTART_COMMAND := brew services restart php@$(PHP_VERSION)
else ifeq ($(UNAME_S),Linux)
    EXTENSION := .so
	EXT_DIR_PATH := /usr/lib/php/$(PHP_API_VERSION)/
    PHP_INI_PATH := /etc/php/$(PHP_VERSION)/cli/php.ini
    RESTART_COMMAND := sudo systemctl restart php$(PHP_VERSION)-fpm && sudo systemctl restart apache2
else
    $(error Unsupported operating system: $(UNAME_S))
endif

.PHONY: build test install clean 
all: lint build test install clean

lint:
	cargo clippy

build:
	cargo build --release

test: build
	php -d extension=./target/release/libaerospike$(EXTENSION) test.php

install: build
	sudo cp -f target/release/libaerospike$(EXTENSION) $(EXT_DIR_PATH)
	echo "extension=libaerospike$(EXTENSION)" | sudo tee -a $(PHP_INI_PATH)
	$(RESTART_COMMAND)

clean:
	cargo clean
