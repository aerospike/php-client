FROM golang:1.20-alpine AS builder

ARG VERSION=1.10.0

ADD . $GOPATH/src/github.com/aerospike/php-client/asld
WORKDIR $GOPATH/src/github.com/aerospike/php-client/asld
RUN go build -ldflags="-X 'main.version=$VERSION'" -o aerospike-local-daemon . \
    && cp aerospike-local-daemon /aerospike-local-daemon

FROM alpine:latest

COPY --from=builder /aerospike-local-daemon /usr/bin/aerospike-local-daemon
COPY asld.toml.template /etc/aerospike-local-daemon/asld.toml.template
COPY docker-entrypoint.sh /docker-entrypoint.sh

RUN apk add gettext libintl \
    && chmod +x /docker-entrypoint.sh

EXPOSE 9145

ENTRYPOINT [ "/docker-entrypoint.sh" ]
CMD ["aerospike-local-daemon", "--config", "/etc/aerospike-local-daemon/asld.toml"]
