GOCMD=go
VERSION=$(shell git describe --tags --abbrev=0 | cut -c 2-)
GOBUILD=$(GOCMD) build -ldflags="-X 'main.version=$(VERSION)'" -o asld
GOGET=$(GOCMD) get -u
BINARY_NAME=asld
SERVICE_NAME=aerospike-connection-manager

.PHONY: test test-rest lint lint-insane clean docs deps modtidy check
all: build

proto:
	rm -rf proto asld_kvs.pb.go asld_kvs_grpc.pb.go
	protoc --go-grpc_out=. --go_out=. asld_kvs.proto --experimental_allow_proto3_optional

check:
	go vet ./...

clean:
	rm -f $(BINARY_NAME)
	rm -f memprofile.out profile.out
	find . -name "*.coverprofile" -exec rm {} +

build:
	$(GOBUILD) .

run: clean
	$(GOBUILD) -v .
	./$(BINARY_NAME) -config-file asld.toml

daemonize: clean
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
