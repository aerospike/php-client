package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"io/fs"
	"log/slog"
	"net"
	"os"
	"os/signal"
	"runtime/debug"
	"syscall"
	"time"

	"github.com/grpc-ecosystem/go-grpc-middleware/v2/interceptors/recovery"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	grpchealth "google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/grpc/reflection"
	"google.golang.org/grpc/status"

	aero "github.com/aerospike/aerospike-client-go/v7"

	"github.com/aerospike/php-client/asld/common/client"
	"github.com/aerospike/php-client/asld/common/config"
	mgmtconfig "github.com/aerospike/php-client/asld/internal/config"
	"github.com/aerospike/php-client/asld/internal/health"
	"github.com/aerospike/php-client/asld/internal/management"
	"github.com/aerospike/php-client/asld/internal/metrics"
	pb "github.com/aerospike/php-client/asld/proto"
)

var (
	version  = "0.1.0"
	revision = "N/A"
)

// shutdownTimeout bounds the graceful shutdown of the management server before
// the process forces its way down.
const shutdownTimeout = 15 * time.Second

// defaultConnectionQueueSize is applied when the cluster policy leaves the
// connection queue unset, matching the historical asld default.
const defaultConnectionQueueSize = 32

// clusterServer bundles everything needed to serve and later tear down a single
// Aerospike cluster's gRPC endpoint.
type clusterServer struct {
	name     string
	grpc     *grpc.Server
	listener net.Listener
	client   *aero.Client
}

func main() {
	configFile := flag.String("config-file", "/etc/aerospike-connection-manager/asld.toml", "Config File")
	showUsage := flag.Bool("h", false, "Show usage information")
	showVersion := flag.Bool("v", false, "Print version")
	collectManagementFlags := mgmtconfig.RegisterFlags(flag.CommandLine)

	flag.Parse()
	if *showUsage {
		flag.Usage()
		os.Exit(0)
	}
	if *showVersion {
		fmt.Println(version)
		os.Exit(0)
	}

	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo}))
	slog.SetDefault(logger)
	logger.Info("starting Aerospike Connection Manager", "version", version, "revision", revision)

	clusters, legacyClusters, err := config.Read(*configFile)
	if err != nil {
		logger.Error("failed to read config", "file", *configFile, "err", err)
		os.Exit(1)
	}
	if len(clusters) == 0 {
		logger.Error("no Aerospike clusters defined in config", "file", *configFile)
		os.Exit(1)
	}
	if len(legacyClusters) > 0 {
		logger.Warn("deprecated cluster configuration: declare clusters under [clusters.<name>]; "+
			"top-level cluster tables are deprecated and support will be removed in a future release",
			"legacy_clusters", legacyClusters)
	}

	managementCfg, err := mgmtconfig.Load(*configFile, collectManagementFlags)
	if err != nil {
		logger.Error("failed to resolve management config", "err", err)
		os.Exit(1)
	}

	m := metrics.New()
	checker := health.NewChecker()

	servers := make([]*clusterServer, 0, len(clusters))
	for name, ac := range clusters {
		cs, err := setupCluster(name, ac, m, logger)
		if err != nil {
			logger.Error("failed to set up cluster", "cluster", name, "err", err)
			shutdownServers(servers)
			cleanUp(clusters, logger)
			os.Exit(1)
		}
		checker.AddReadiness(name, health.Connected(cs.client))
		m.RegisterAerospike(name, aerospikeStats{cs.client})
		servers = append(servers, cs)
	}

	managementSrv := management.New(managementCfg, m.Handler(), checker, logger)
	if err := managementSrv.Start(); err != nil {
		logger.Error("failed to start management server", "err", err)
		shutdownServers(servers)
		cleanUp(clusters, logger)
		os.Exit(1)
	}

	serveErr := make(chan error, len(servers))
	for _, cs := range servers {
		cs := cs
		go func() {
			logger.Info("serving cluster", "cluster", cs.name, "socket", cs.listener.Addr().String())
			if err := cs.grpc.Serve(cs.listener); err != nil && !errors.Is(err, grpc.ErrServerStopped) {
				serveErr <- fmt.Errorf("cluster %q: %w", cs.name, err)
			}
		}()
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	select {
	case <-ctx.Done():
		logger.Info("shutdown signal received, draining")
	case err := <-serveErr:
		logger.Error("gRPC server failed, shutting down", "err", err)
	}

	shutdownCtx, cancel := context.WithTimeout(context.Background(), shutdownTimeout)
	defer cancel()
	if err := managementSrv.Shutdown(shutdownCtx); err != nil {
		logger.Error("management server shutdown error", "err", err)
	}
	shutdownServers(servers)
	cleanUp(clusters, logger)
	logger.Info("shutdown complete")
}

// setupCluster builds, but does not start, the gRPC server for a single
// cluster. The Aerospike client, the unix socket listener and the gRPC server
// (with metrics and panic-recovery interceptors) are all created here so that
// any failure is reported to the caller before serving begins.
func setupCluster(name string, ac *client.AerospikeConfig, m *metrics.Metrics, logger *slog.Logger) (*clusterServer, error) {
	cp, err := ac.NewClientPolicy()
	if err != nil {
		return nil, fmt.Errorf("client policy: %w", err)
	}
	if cp.ConnectionQueueSize == 0 {
		cp.ConnectionQueueSize = defaultConnectionQueueSize
	}

	c, aerr := aero.NewClientWithPolicyAndHost(cp, ac.NewHosts()...)
	if aerr != nil {
		return nil, fmt.Errorf("connect: %w", aerr)
	}
	if _, werr := c.WarmUp(-1); werr != nil {
		logger.Warn("connection pool warm-up incomplete", "cluster", name, "err", werr)
	}

	ln, err := net.Listen("unix", ac.Socket)
	if err != nil {
		c.Close()
		return nil, fmt.Errorf("listen on socket %s: %w", ac.Socket, err)
	}

	recoveryHandler := func(p any) error {
		logger.Error("recovered from panic", "cluster", name, "panic", p, "stack", string(debug.Stack()))
		return status.Errorf(codes.Internal, "%s", p)
	}

	srv := grpc.NewServer(
		// Allow the largest record possible: 128MiB for memory namespaces, with overhead.
		grpc.MaxRecvMsgSize(130*1024*1024),
		grpc.MaxSendMsgSize(130*1024*1024),
		grpc.ChainUnaryInterceptor(
			m.UnaryServerInterceptor(),
			recovery.UnaryServerInterceptor(recovery.WithRecoveryHandler(recoveryHandler)),
		),
		grpc.ChainStreamInterceptor(
			m.StreamServerInterceptor(),
			recovery.StreamServerInterceptor(recovery.WithRecoveryHandler(recoveryHandler)),
		),
	)

	grpc_health_v1.RegisterHealthServer(srv, grpchealth.NewServer())
	pb.RegisterKVSServer(srv, &server{client: c})
	reflection.Register(srv)
	m.InitializeServer(srv)

	return &clusterServer{name: name, grpc: srv, listener: ln, client: c}, nil
}

// shutdownServers gracefully stops the gRPC servers and closes their Aerospike
// clients, draining in-flight RPCs.
func shutdownServers(servers []*clusterServer) {
	for _, cs := range servers {
		cs.grpc.GracefulStop()
		cs.client.Close()
	}
}

func cleanUp(conf map[string]*client.AerospikeConfig, logger *slog.Logger) {
	for _, ac := range conf {
		if err := os.Remove(ac.Socket); err != nil && !errors.Is(err, os.ErrNotExist) && !errors.Is(err, fs.ErrNotExist) {
			logger.Warn("socket was not cleaned up", "socket", ac.Socket, "err", err)
		}
	}
}

// aerospikeStats adapts *aero.Client onto metrics.StatsProvider. The Aerospike
// client returns its own error type, so a thin wrapper is needed to satisfy the
// standard-error interface used by the metrics package.
type aerospikeStats struct {
	c *aero.Client
}

func (a aerospikeStats) Stats() (map[string]interface{}, error) {
	return a.c.Stats()
}

func init() {
	if info, ok := debug.ReadBuildInfo(); ok {
		for _, kv := range info.Settings {
			if kv.Key == "vcs.revision" && kv.Value != "" {
				revision = kv.Value
			}
		}
	}
}
