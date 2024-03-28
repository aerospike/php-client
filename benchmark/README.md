# Aerospike PHP client benchmark

This guide provides step-by-step instructions on running the benchmark for the Aerospike php client.

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

For more instructions on using phpbench tool visit [phpbenc](https://phpbench.readthedocs.io)