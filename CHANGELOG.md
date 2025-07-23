# Changelog

All notable changes to this project will be documented in this file.

## [1.3.0] - 2025-02-23

- **New Features**:
  - [CLIENT_3542] Support inserting binary data as `Value::blob`.

## [1.2.0] - 2025-02-28

 - **Improvements**: 
  - [CLIENT-3351] Update ext-php-rs to v0.13.0 to support PHP v8.4.
  - [CLIENT-3334] Old PHP client encodes boolean and null values improperly.
    Updates the Go client to v7.9.0 that supports decoding the old PHP7 improperly encoded boolean and null values.
  - [CLIENT-3230] Create new build / install scripts, improve READMEs, add test pipelines

## [1.1.0] - 2024-06-04
- **Download package**
  - https://aerospike.com/download/?software=client-php
  
- **New Features**:

  - [CLIENT-2834] Added support for readTouchTTlPercent to support Aerospike 7.1.
  - [CLIENT-2969] Added support for LongValue in KVS service.
  - [CLIENT-2989] Added support version check between connection manager and php client.

 - **Improvements**: 
  - [CLIENT-2990] Added support for big records (128 MiB for memory namespaces).

- **Fixes**:

  - [CLIENT-2991] Fix deb and rpm post install script.

## [1.0.2] - 2024-05-01
- **Download package**
  - https://aerospike.com/download/?software=client-php
  
- **New Features**:

  - [CLIENT-2844] Added support for environment variables and file for the client configuration.
  - [CLIENT-2846] Added support for building from deb and rpm packages.
  

- **Fixes**:

  - [CLIENT-2906] Cleaned up the build for Aerospike PHP client and Aerospike connection manager.

## [1.0.1] - 2024-04-16

- **Fixes**
  - [CLIENT-2871] Set `durable_delete` to `false` by default.


## [1.0.0] - 2024-03-28

This will be the GA release for Aerospike PHP Client v1.0.0.

- **New Features**:

  - Added support for Scan and Query.
  - Added support for UDF.
  - Added support for CDT.
  - Added support for authentication and security.
  - Added AerospikeExcetpions.
  - Added benchmarks.

- **Improvements**:

  - Added more unit tests.
  - Cleaned up the build for Aerospike connection manager.


## [0.5.0] - 2024-02-21

- **New Features**:

  - Added aerospike connection manager.
  - Supports server v7.

## [0.4.0] - 2023-12-04

- **Improvements**:

  - Authentication performance issue has been fixed.
  - Added and fixed Unit tests.

## [0.3.0] - 2023-11-16

- **Improvements**:

  - Authentication issue has been fixed.
  - Support aerospike server 6.3.
  - Fixed build failure for ARM platform.

## [0.2.0] - 2023-10-25

- **New Features**:

  - Introduce dedicated namespace "Aerospike".
  - Added support for all PHP versions above 8.

- **Improvements**:

  - Added phpunit tests.
  - Minor code cleanups and security improvements.

- **Update**:
  
  - Updated `client.exists` api to take `ReadPolicy` as an argument instead of `WritePolicy`.
  - Added improvements use HLL and GeoJSON Values.
  
## [0.1.0] - 2023-09-15

  - Initial Release.
