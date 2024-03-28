# Aerospike PHP client benchmark

This guide provides step-by-step instructions on running the benchmark for the Aerospike php client. The provided PHP code constitutes a benchmarking suite for the Aerospike PHP client library. This benchmark assesses the performance of operations (put and get) on data to and from an Aerospike database.

- **Operations**: The benchmark primarily focuses on two types of operations: put and get. The put operation involves writing data to the Aerospike database, while the get operation involves reading data from the database.
- **Transaction Volume**: Each operation is repeated a 100000 number of times (@Revs(100000)) and each operation is iterated 100 times (@Iterations(100)).
- **Data Size**: Data of different sizes is used for benchmarking. For example, strings of lengths 1, 10, 100, 1000, 10000, and 100000 characters are put into the database to assess the performance under varying data sizes.
- **Namespace/Set**: The benchmark writes data to and reads data from the "test" namespace and various sets within this namespace. Different sets are used for different benchmarks (e.g., "Benchmark_Get_String1", "Benchmark_Get_Integer32", etc.).

## Prerequisites

- composer v2.7.0^
- php v8.1^
- Aeorpsike PHP client v1.0.0
- Aerospike Server

## Running the benchmark

1. **Change Directory**: Navigate into the `php-client/benchmark` directory:
```bash
   cd php-client/daemon
```

2. **Install the phpbench tool**:
```bash
   composer require phpbench/phpbench --dev
```
3. **Run the benchmark**:
```bash
    ./vendor/bin/phpbench run php-client/benchmark/ --report=default
```

For more instructions on using phpbench tool visit [phpbench-benchmarking library](https://phpbench.readthedocs.io)