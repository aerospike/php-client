# Build the Aerospike Connection manager


# Determine the operating system
UNAME_S := $(shell uname -s)
EXT_DIR_PATH := $(shell php -r 'echo ini_get("extension_dir");')
PHP_INI_PATH := $(shell php -r 'echo php_ini_loaded_file();')

ifeq ($(UNAME_S),Darwin)
    EXTENSION := .dylib
	PHP_VERSION := $(shell php -v | head -n 1 | awk '{print $$2}' | cut -d. -f1,2)
    RESTART_COMMAND := brew services restart php@$(PHP_VERSION)
else ifeq ($(UNAME_S),Linux)
    EXTENSION := .so
	PHP_VERSION := $(shell php -i | grep -Po '(?<=PHP Version => ).*' | uniq)
    RESTART_COMMAND := sudo systemctl restart php$(PHP_VERSION)-fpm && sudo systemctl restart apache2
else
    $(error Unsupported operating system: $(UNAME_S))
endif

# Check if PHP version is greater than 8.0
ifeq ($(shell awk 'BEGIN{ print ("$(PHP_VERSION)" >= "8.0") }'), 0)
    $(error PHP version must be greater than or equal to 8.0)
endif

.PHONY: build install test clean
all: lint build install test clean

lint:
	cargo clippy

build-dev:
	cargo build

build:
	cargo build --release

install-dev: build-dev
	sudo cp -f target/debug/libaerospike_php$(EXTENSION) $(EXT_DIR_PATH)
	echo "extension=libaerospike_php$(EXTENSION)" | sudo tee -a $(PHP_INI_PATH)

install: build
	sudo cp -f target/release/libaerospike_php$(EXTENSION) $(EXT_DIR_PATH)
	echo "extension=libaerospike_php$(EXTENSION)" | sudo tee -a $(PHP_INI_PATH)

restart: install
	$(RESTART_COMMAND)

test-dev: install-dev
	sudo ./vendor/phpunit/phpunit/phpunit tests/

test: install
	phpunit tests/

clean:
	cargo clean
