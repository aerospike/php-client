package main

import (
	"flag"
	"io/fs"
	"log"
	"net"
	"os"
	"os/signal"
	"runtime/debug"
	"syscall"
	"time"

	"github.com/grpc-ecosystem/go-grpc-middleware/v2/interceptors/recovery"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/grpc/reflection"
	"google.golang.org/grpc/status"

	aero "github.com/aerospike/aerospike-client-go/v7"

	"github.com/aerospike/php-client/asld/common/client"
	"github.com/aerospike/php-client/asld/common/config"
	pb "github.com/aerospike/php-client/asld/proto"
)

var (
	version    = "0.1.0"
	revision   = "N/A"
	lastCommit time.Time
)

// TODO: Finish logging and make sure logs have prefixes for different clusters

func main() {
	var (
		configFile  = flag.String("config-file", "/etc/aerospike-connection-manager/asld.toml", "Config File")
		showUsage   = flag.Bool("h", false, "Show usage information")
		showVersion = flag.Bool("v", false, "Print version")
	)

	flag.Parse()
	if *showUsage {
		flag.Usage()
		os.Exit(0)
	}

	if *showVersion {
		println(version)
		os.Exit(0)
	}

	log.Printf("Aerospike Local Proxy `%s`.", version)

	conf, err := config.Read(*configFile)
	if err != nil {
		log.Fatalln(err)
	}
	defer cleanUp(conf)

	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt, syscall.SIGTERM)
	go func() {
		<-c
		cleanUp(conf)
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
		log.Printf("cleaning up socket `%s`.", ac.Socket)
		err := os.Remove(ac.Socket)
		if err != nil && err != os.ErrNotExist && err != fs.ErrNotExist {
			log.Printf("Socket %s was not cleaned up: %s.", ac.Socket, err)
		}
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

	grpcPanicRecoveryHandler := func(p any) (err error) {
		log.Println("recovered from panic", "panic", p, "stack", string(debug.Stack()))
		return status.Errorf(codes.Internal, "%s", p)
	}

	srv := grpc.NewServer(
		// set the maximum message size possible for a record: 128MiB for memory namespaces, with overhead
		grpc.MaxRecvMsgSize(130*1024*1024),
		grpc.MaxSendMsgSize(130*1024*1024),
		grpc.ChainUnaryInterceptor(recovery.UnaryServerInterceptor(recovery.WithRecoveryHandler(grpcPanicRecoveryHandler))),
		grpc.ChainStreamInterceptor(recovery.StreamServerInterceptor(recovery.WithRecoveryHandler(grpcPanicRecoveryHandler))),
	)

	grpc_health_v1.RegisterHealthServer(srv, health.NewServer())
	pb.RegisterKVSServer(srv, &server{client: client})
	reflection.Register(srv)

	// go func() {
	// 	log.Printf("grpc ran on tcp protocol %s", ADDR)
	// 	log.Fatal(srv.Serve(tcpLn))
	// }()

	log.Printf("Cake is ready for unix socket protocol: %s", ac.Socket)
	log.Println(srv.Serve(ln))
}

func init() {
	if info, ok := debug.ReadBuildInfo(); ok {
		for _, kv := range info.Settings {
			if kv.Value == "" {
				continue
			}
			switch kv.Key {
			case "vcs.revision":
				revision = kv.Value
			case "vcs.time":
				lastCommit, _ = time.Parse(time.RFC3339, kv.Value)
			}
		}
	}
}
