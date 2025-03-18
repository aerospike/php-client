GOCMD=go
VERSION=$(shell cat version.txt)
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
	cp $(BINARY_NAME) /usr/bin/$(BINARY_NAME)
	mkdir /etc/$(SERVICE_NAME)
	cp asld.toml /etc/$(SERVICE_NAME)/$(BINARY_NAME).toml
	cp $(SERVICE_NAME).service /etc/systemd/system/$(SERVICE_NAME).service
	systemctl daemon-reload
	systemctl enable $(SERVICE_NAME)
	systemctl start $(SERVICE_NAME)

deps:
	$(GOGET) -u .

modtidy:
	$(GOCMD) mod tidy
