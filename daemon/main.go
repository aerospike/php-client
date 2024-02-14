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
	"github.com/spf13/cobra"

	"github.com/aerospike/php-client/asld/common/config"
	"github.com/aerospike/php-client/asld/common/flags"
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
	configFileFlags := flags.NewConfFileFlags()
	aerospikeFlags := flags.NewDefaultAerospikeFlags()

	appCmd := &cobra.Command{
		Use:     "asld",
		Short:   "Aerospike Local Daemon",
		Version: "0.1.0",
		Run: func(cmd *cobra.Command, args []string) {
			_, err := config.InitConfig(configFileFlags.File, configFileFlags.Instance, cmd.Flags())
			if err != nil {
				log.Fatalln("Failed to initialize config:", err)
			}
		},
	}

	cfFlagSet := configFileFlags.NewFlagSet(flags.DefaultWrapHelpString)
	asFlagSet := aerospikeFlags.NewFlagSet(flags.DefaultWrapHelpString)

	appCmd.PersistentFlags().AddFlagSet(cfFlagSet)

	// This is what connects the flags to fields of the same name in the config file.
	config.BindPFlags(asFlagSet, "cluster")

	appCmd.PersistentFlags().AddFlagSet(asFlagSet)
	flags.SetupRoot(appCmd, "Aerospike Local Daemon")

	if err := appCmd.Execute(); err != nil {
		os.Exit(1)
	}

	ac := aerospikeFlags.NewAerospikeConfig()
	cp, err2 := ac.NewClientPolicy()
	if err2 != nil {
		log.Fatalln(err2)
	}

	seeds := ac.NewHosts()

	var err error
	if client, err = aero.NewClientWithPolicyAndHost(cp, seeds...); err != nil {
		log.Fatalln(err)
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
