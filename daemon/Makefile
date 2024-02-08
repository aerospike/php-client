# Go parameters

GOCMD=go
GOBUILD=$(GOCMD) build
GOGET=$(GOCMD) get -u
BINARY_NAME=asld

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
	./$(BINARY_NAME)
deps:
	$(GOGET) -u .
modtidy:
	$(GOCMD) mod tidy
