package main

import (
	"flag"
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

	"github.com/aerospike/php-client/asld/common/client"
	"github.com/aerospike/php-client/asld/common/config"
	pb "github.com/aerospike/php-client/asld/proto"
)

const (
	version = "0.1.0"
)

func main() {
	var (
		configFile  = flag.String("config-file", "/etc/aerospike-local-daemon/asld.toml", "Config File")
		showUsage   = flag.Bool("h", false, "Show usage information")
		showVersion = flag.Bool("v", false, "Print version")
	)

	flag.Parse()
	if *showUsage {
		flag.Usage()
		os.Exit(0)
	}

	if *showVersion {
		log.Println(version)
		os.Exit(0)
	}

	conf, err := config.Read(*configFile)
	if err != nil {
		log.Fatalln(err)
	}
	defer cleanUp(conf)

	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt, syscall.SIGTERM)
	go func() {
		<-c
		os.Exit(1)
	}()

	for cluster, ac := range conf {
		go launchServer(cluster, ac)
	}

	e := make(chan struct{}, 1)
	<-e
}

func cleanUp(conf map[string]*client.AerospikeConfig) {
	for _, ac := range conf {
		os.Remove(ac.Socket)
	}
}

func launchServer(name string, ac *client.AerospikeConfig) {
	cp, err := ac.NewClientPolicy()
	if cp.ConnectionQueueSize == 0 {
		cp.ConnectionQueueSize = 32
	}
	if err != nil {
		log.Fatalln(err)
	}

	seeds := ac.NewHosts()

	client, err := aero.NewClientWithPolicyAndHost(cp, seeds...)
	if err != nil {
		log.Fatalln(err)
	}
	client.WarmUp(-1)

	log.Printf("Server is Initializing for cluster `%s`. There will be cake...", name)
	ln, err := net.Listen("unix", ac.Socket)
	if err != nil {
		log.Printf("Server initialization failed: %s", err)
		log.Fatalln("The cake was a lie!")
	}

	defer os.Remove(ac.Socket)

	// tcpLn, err := net.Listen(PROTOCOL_TCP, ADDR)
	// if err != nil {
	// 	log.Fatal(err)
	// }

	srv := grpc.NewServer()
	grpc_health_v1.RegisterHealthServer(srv, health.NewServer())
	pb.RegisterKVSServer(srv, &server{client: client})
	reflection.Register(srv)

	// go func() {
	// 	log.Printf("grpc ran on tcp protocol %s", ADDR)
	// 	log.Fatal(srv.Serve(tcpLn))
	// }()

	log.Printf("Cake is ready for unix socket protocol: %s", ac.Socket)
	log.Fatal(srv.Serve(ln))
}
