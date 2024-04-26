# Aerospike PHP Client Package

This package contains Aerospike PHP client library installers for development
and runtime.
Version - 1.0.2

## Contents

* aerospike-php-client-<version>-<arch>
  Aerospike php client library

## Prerequisites

The following are the prerequistes for the PHP client library.
1. Go ^v1.21.3
2. PHP ^v8.0.0

## Installation

When the .deb or .rpm package is installed the php library (libaerospike.so) will be placed in the php extensions directory. The aerospike connection manager binary (asld) will be placed in /usr/bin by default. The config file (asld.toml) for the Aerospike connection manager will be placed /etc/ by default. 

On succesful installation to run the aerospike connection manager run the following command:
```bash
asld -config-file <path-to-config-file>
```

### Architecure Support 

| Package Name                          	| Architecture 	| Supported Distros                               |
|-----------------------------------------------|---------------|--------------------------------------|
| aerospike-php-client_1.0.2_arm64.deb 		| arm64        	| debian10, debian11, debian12, ubuntu20.04, ubuntu22.04 |
| aerospike-php-client_1.0.2_x86_64.deb 	| amd64        	| debian10, debian11, debian12, ubuntu20.04, ubuntu22.04 |
| aerospike-php-client-1.0.2-1.noarch.rpm 	| noarch    	| el8, el9, amzn2023                              |

