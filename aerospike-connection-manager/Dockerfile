FROM golang:1.23-alpine AS builder

ARG VERSION=1.10.0

WORKDIR /src
COPY . .
RUN go build -ldflags="-X 'main.version=$VERSION'" -o /aerospike-connection-manager .

FROM alpine:latest

COPY --from=builder /aerospike-connection-manager /usr/bin/aerospike-connection-manager
COPY asld.toml /etc/aerospike-connection-manager/asld.toml
COPY asld.toml.template /etc/aerospike-connection-manager/asld.toml.template

# Management HTTP server (metrics, health/readiness) default port.
EXPOSE 9145

ENTRYPOINT ["aerospike-connection-manager"]
CMD ["-config-file", "/etc/aerospike-connection-manager/asld.toml"]
