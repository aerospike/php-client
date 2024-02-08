package main

import (
	"log"
	"net"
	"os"
	"os/signal"
	"syscall"

	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/grpc/reflection"

	aero "github.com/aerospike/aerospike-client-go/v7"

	pb "github.com/aerospike/php-client/asld/proto"
)

const (
	// unix socket
	PROTOCOL = "unix"
	SOCKET   = "/tmp/asld_grpc.sock"

	// tcp protocol
	PROTOCOL_TCP = "tcp"
	ADDR         = "localhost:8080"
)

var client *aero.Client

func main() {
	var err error
	if client, err = aero.NewClient("localhost", 3000); err != nil {
		log.Fatalln(err)
	}

	key, _ := aero.NewKey("test", "test", 1)
	if err := client.Put(nil, key, aero.BinMap{
		"int":   1,
		"float": 11.11,
		"str":   "hello world!",
		"bytes": []byte{1, 2, 3, 4, 5},
		"map": map[any]any{
			1:      1,
			2:      "hello",
			"map":  map[any]any{1: 1, 2: 2, 3: 3.1},
			"list": []any{1, 2, 3},
		}},
	); err != nil {
		panic(err)
	}

	// runtime.GOMAXPROCS(2)

	ln, err := net.Listen(PROTOCOL, SOCKET)
	if err != nil {
		log.Fatal(err)
	}

	// tcpLn, err := net.Listen(PROTOCOL_TCP, ADDR)
	// if err != nil {
	// 	log.Fatal(err)
	// }

	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt, syscall.SIGTERM)
	go func() {
		<-c
		os.Remove(SOCKET)
		os.Exit(1)
	}()

	srv := grpc.NewServer()
	grpc_health_v1.RegisterHealthServer(srv, health.NewServer())
	pb.RegisterKVSServer(srv, &server{})
	reflection.Register(srv)

	// go func() {
	// 	log.Printf("grpc ran on tcp protocol %s", ADDR)
	// 	log.Fatal(srv.Serve(tcpLn))
	// }()

	log.Printf("grpc ran on unix socket protocol %s", SOCKET)
	log.Fatal(srv.Serve(ln))
}
