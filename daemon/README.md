# Go Local Daemon Setup Guide

This guide provides step-by-step instructions on setting up the Go Local Daemon.

## Prerequisites

- Go programming language installed on your system.
- `make` utility installed.

## Setup Instructions

1. **Change Directory**: Navigate into the `php-client/daemon` directory:
   ```bash
   cd php-client/daemon
   ```

2. **Run Makefile**: Execute the Makefile to build the daemon:
   ```bash
   sudo make
   ```

3. **Verify Build**: Successful build logs should resemble the following:
```bash
rm -f asld
rm -f memprofile.out profile.out
rm -rf proto asld_kvs.pb.go asld_kvs_grpc.pb.go
find . -name "*.coverprofile" -exec rm {} +
protoc --go-grpc_out=. --go_out=. asld_kvs.proto --experimental_allow_proto3_optional
go build -o asld -v .
github.com/aerospike/php-client/asld
./asld
2024/02/13 10:41:30 grpc ran on unix socket protocol /tmp/asld_grpc.sock
```

## Configuring Client Policy 
Aerospike's client policy allows for flexible control over read and write operations, including optimistic concurrency, time-to-live settings, and conditional writes based on record existence.

1. **Create a Config File**: Utilize a .toml configuration file to define the client policy settings. There already exists the default configs on the `asld.toml` file. 
2. **Running the local daemon with new Client Policy**: Add the `path-to-your-.toml-file` in the makefile. 
```bash
run: clean proto
$(GOBUILD) -o $(BINARY_NAME) -v .
./$(BINARY_NAME) -config-file <path-to-your-.toml-file>
```


## Daemonize the process
You can daemonize the process using utilities like systemd on Linux or launchd on macOS using the following:
```bash

# Go parameters
GOCMD=go
GOBUILD=$(GOCMD) build
GOGET=$(GOCMD) get -u
BINARY_NAME=asld
SERVICE_NAME=asld

.PHONY: test test-rest lint lint-insane clean docs deps modtidy check
all: run

proto:
    protoc --go-grpc_out=. --go_out=. asld_kvs.proto --experimental_allow_proto3_optional

check:
    go vet ./...

clean:
    rm -f $(BINARY_NAME)
    rm -f memprofile.out profile.out
    rm -rf proto asld_kvs.pb.go asld_kvs_grpc.pb.go
    find . -name "*.coverprofile" -exec rm {} +

build:
    $(GOBUILD) .

run: clean proto
    $(GOBUILD) -o $(BINARY_NAME) -v .
    ./$(BINARY_NAME) -config-file asld.toml

daemonize: clean proto
    $(GOBUILD) -o $(BINARY_NAME) -v .
    sudo cp $(BINARY_NAME) /usr/local/bin/$(BINARY_NAME)
    sudo cp asld.toml /etc/$(BINARY_NAME).toml
    sudo cp $(SERVICE_NAME).service /etc/systemd/system/$(SERVICE_NAME).service
    sudo systemctl daemon-reload
    sudo systemctl enable $(SERVICE_NAME)
    sudo systemctl start $(SERVICE_NAME)

deps:
    $(GOGET) -u .

modtidy:
    $(GOCMD) mod tidy

```