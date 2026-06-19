// Package management hosts the operational HTTP server of the Aerospike
// Connection Manager.
//
// It serves Prometheus metrics and the liveness/readiness/health probes on a
// single admin port, separate from the gRPC data path. Following the common Go
// and Kubernetes convention of one admin port keeps probe configuration and
// metric scraping simple while leaving the data plane untouched. Each route is
// mounted only when enabled in the resolved configuration.
package management

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"net/http/pprof"
	"path"
	"time"

	"github.com/aerospike/php-client/asld/internal/config"
	"github.com/aerospike/php-client/asld/internal/health"
)

// readHeaderTimeout guards the admin server against slow-loris style stalls. It
// is generous because the endpoints are internal and not latency sensitive.
const readHeaderTimeout = 5 * time.Second

// Server is the management HTTP server. Use New to construct it.
type Server struct {
	cfg    config.Management
	http   *http.Server
	logger *slog.Logger

	listener net.Listener
}

// New assembles the management server from the resolved configuration. The
// metrics handler may be nil when metrics are disabled; the checker may be nil
// when no health endpoints are enabled. When the management server is disabled
// the returned Server is inert and Start/Shutdown are no-ops.
func New(cfg config.Management, metricsHandler http.Handler, checker *health.Checker, logger *slog.Logger) *Server {
	if logger == nil {
		logger = slog.Default()
	}

	mux := http.NewServeMux()

	if cfg.Metrics.Enabled && metricsHandler != nil {
		mux.Handle(cfg.Metrics.Path, metricsHandler)
	}
	if checker != nil {
		if cfg.Liveness.Enabled {
			mux.Handle(cfg.Liveness.Path, checker.LivenessHandler())
		}
		if cfg.Readiness.Enabled {
			mux.Handle(cfg.Readiness.Path, checker.ReadinessHandler())
		}
		if cfg.Health.Enabled {
			mux.Handle(cfg.Health.Path, checker.HealthHandler())
		}
	}
	if cfg.Pprof.Enabled {
		registerPprof(mux, cfg.Pprof.Path)
	}

	return &Server{
		cfg:    cfg,
		logger: logger,
		http: &http.Server{
			Addr:              cfg.Address,
			Handler:           mux,
			ReadHeaderTimeout: readHeaderTimeout,
		},
	}
}

// registerPprof mounts the standard net/http/pprof handlers under prefix,
// mirroring how the package registers itself on the default mux.
func registerPprof(mux *http.ServeMux, prefix string) {
	mux.HandleFunc(prefix, pprof.Index)
	mux.HandleFunc(path.Join(prefix, "cmdline"), pprof.Cmdline)
	mux.HandleFunc(path.Join(prefix, "profile"), pprof.Profile)
	mux.HandleFunc(path.Join(prefix, "symbol"), pprof.Symbol)
	mux.HandleFunc(path.Join(prefix, "trace"), pprof.Trace)
}

// Start binds the listen address and serves in the background. The bind happens
// synchronously so address-in-use errors surface to the caller at startup
// rather than being lost in a goroutine. It is a no-op when the management
// server is disabled.
func (s *Server) Start() error {
	if !s.cfg.Enabled {
		s.logger.Info("management server disabled")
		return nil
	}

	ln, err := net.Listen("tcp", s.http.Addr)
	if err != nil {
		return fmt.Errorf("management listen on %s: %w", s.http.Addr, err)
	}
	s.listener = ln

	s.logger.Info("management server listening", "address", ln.Addr().String())
	go func() {
		if err := s.http.Serve(ln); err != nil && !errors.Is(err, http.ErrServerClosed) {
			s.logger.Error("management server stopped unexpectedly", "err", err)
		}
	}()
	return nil
}

// Shutdown gracefully stops the management server, waiting for in-flight
// requests until ctx is done. It is a no-op when the server was never started.
func (s *Server) Shutdown(ctx context.Context) error {
	if !s.cfg.Enabled || s.listener == nil {
		return nil
	}
	return s.http.Shutdown(ctx)
}

// Addr returns the actual listen address, which is useful when the configured
// address used port 0. It returns an empty string before Start.
func (s *Server) Addr() string {
	if s.listener == nil {
		return ""
	}
	return s.listener.Addr().String()
}
