# Go parameters
hosts ?= ""
host ?= localhost
port ?= 3000 
user ?= ""
pass ?= ""
ns ?= "test"

# Determine the operating system
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
    EXTENSION := .dylib
	EXT_DIR_PATH := /opt/homebrew/opt/php@8.1/pecl/20210902
    PHP_INI_PATH := /opt/homebrew/etc/php/8.1/php.ini
    RESTART_COMMAND := brew services restart php@8.1
else ifeq ($(UNAME_S),Linux)
    EXTENSION := .so
	EXT_DIR_PATH := /usr/lib/php/20210902/
    PHP_INI_PATH := /etc/php/8.1/cli/php.ini
    RESTART_COMMAND := sudo systemctl restart php8.1-fpm && sudo systemctl restart apache2
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
