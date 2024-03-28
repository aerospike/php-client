FROM golang:1.20-alpine AS builder

ARG VERSION=1.10.0

ADD . $GOPATH/src/github.com/aerospike/php-client/asld
WORKDIR $GOPATH/src/github.com/aerospike/php-client/asld
RUN go build -ldflags="-X 'main.version=$VERSION'" -o aerospike-connection-manager . \
    && cp aerospike-connection-manager /aerospike-connection-manager

FROM alpine:latest

COPY --from=builder /aerospike-connection-manager /usr/bin/aerospike-connection-manager
COPY asld.toml.template /etc/aerospike-connection-manager/asld.toml.template
COPY docker-entrypoint.sh /docker-entrypoint.sh

RUN apk add gettext libintl \
    && chmod +x /docker-entrypoint.sh

EXPOSE 9145

ENTRYPOINT [ "/docker-entrypoint.sh" ]
CMD ["aerospike-connection-manager", "--config", "/etc/aerospike-connection-manager/asld.toml"]
