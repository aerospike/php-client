# Changelog

All notable changes to this project will be documented in this file.

## [1.0.0] - 2024-03-28

This will be the GA release for Aerospike PHP Client v1.0.0

- **New Features**:

  - Added support for Scan and Query
  - Added support for UDF
  - Added support for CDT
  - Added support for authentication and security
  - Added AerospikeExcetpions
  - Added benchmarks

- **Improvements**:

  - Added more unit tests
  - Cleaned up the build for Aerospike connection manager


## [0.5.0] - 2024-02-21

- **New Features**:

  - Added aerospike connection manager
  - Supports server v7

## [0.4.0] - 2023-12-04

- **Improvements**:

  - Authentication performance issue has been fixed
  - Added and fixed Unit tests

## [0.3.0] - 2023-11-16

- **Improvements**:

  - Authentication issue has been fixed
  - Support aerospike server 6.3
  - Fixed build failure for ARM platform

## [0.2.0] - 2023-10-25

- **New Features**:

  - Introduce dedicated namespace "Aerospike"
  - Added support for all PHP versions above 8

- **Improvements**:

  - Added phpunit tests
  - Minor code cleanups and security improvements

- **Update**:
  
  - Updated `client.exists` api to take `ReadPolicy` as an argument instead of `WritePolicy`.
  - Added improvements use HLL and GeoJSON Values.
  
## [0.1.0] - 2023-09-15

  - Initial Release.