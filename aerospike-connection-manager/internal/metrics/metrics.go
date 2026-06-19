// Package metrics owns the Prometheus instrumentation of the Aerospike
// Connection Manager.
//
// It bundles three layers of metrics behind a single registry:
//
//   - Go runtime metrics (goroutines, GC, memory) via the standard Go collector.
//   - Process metrics (CPU, resident memory, open file descriptors) where the
//     platform exposes them.
//   - gRPC server metrics (per-method request counts and a latency histogram)
//     via a server interceptor.
//
// Per-cluster Aerospike connection-pool metrics are added by RegisterAerospike.
//
// The instrumentation is always collected; whether it is exposed is decided by
// the management server, which only mounts Handler when the metrics endpoint is
// enabled.
package metrics

import (
	"net/http"

	grpcprom "github.com/grpc-ecosystem/go-grpc-middleware/providers/prometheus"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/collectors"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"google.golang.org/grpc"
)

// grpcLatencyBuckets are tuned for the asld gRPC profile: sub-millisecond
// median, low-millisecond tail. The default Prometheus buckets start at 5ms and
// would collapse almost every Aerospike call into a single bucket, making the
// histogram useless for latency analysis.
var grpcLatencyBuckets = []float64{
	0.0001, 0.00025, 0.0005,
	0.001, 0.0025, 0.005,
	0.01, 0.025, 0.05,
	0.1, 0.25, 0.5, 1.0,
}

// Metrics owns the Prometheus registry and the instrumentation registered on
// it. A single instance is shared across all gRPC servers in the process.
type Metrics struct {
	registry    *prometheus.Registry
	grpcMetrics *grpcprom.ServerMetrics
	aerospike   *aerospikeCollector
}

// New constructs a Metrics instance with the Go runtime, process and gRPC
// server collectors pre-registered.
func New() *Metrics {
	registry := prometheus.NewRegistry()
	registry.MustRegister(
		collectors.NewGoCollector(),
		collectors.NewProcessCollector(collectors.ProcessCollectorOpts{}),
	)

	grpcMetrics := grpcprom.NewServerMetrics(
		grpcprom.WithServerHandlingTimeHistogram(
			grpcprom.WithHistogramBuckets(grpcLatencyBuckets),
		),
	)
	registry.MustRegister(grpcMetrics)

	aerospike := newAerospikeCollector()
	registry.MustRegister(aerospike)

	return &Metrics{
		registry:    registry,
		grpcMetrics: grpcMetrics,
		aerospike:   aerospike,
	}
}

// Registry exposes the underlying Prometheus registry, mainly for testing.
func (m *Metrics) Registry() *prometheus.Registry {
	return m.registry
}

// UnaryServerInterceptor returns the gRPC unary interceptor that records
// per-method request counts and latencies.
func (m *Metrics) UnaryServerInterceptor() grpc.UnaryServerInterceptor {
	return m.grpcMetrics.UnaryServerInterceptor()
}

// StreamServerInterceptor returns the gRPC stream interceptor counterpart.
func (m *Metrics) StreamServerInterceptor() grpc.StreamServerInterceptor {
	return m.grpcMetrics.StreamServerInterceptor()
}

// InitializeServer pre-registers every method of srv with a zero count so that
// dashboards are populated immediately after deploy rather than only once each
// method has been exercised for the first time.
func (m *Metrics) InitializeServer(srv *grpc.Server) {
	m.grpcMetrics.InitializeMetrics(srv)
}

// RegisterAerospike adds a cluster's client to the connection-pool collector.
// Stats are read lazily on each scrape, so this only needs to be called once
// per cluster before the metrics endpoint starts serving.
func (m *Metrics) RegisterAerospike(cluster string, provider StatsProvider) {
	m.aerospike.register(cluster, provider)
}

// Handler returns the HTTP handler that serves the registry in the Prometheus
// text and OpenMetrics formats.
func (m *Metrics) Handler() http.Handler {
	return promhttp.HandlerFor(m.registry, promhttp.HandlerOpts{
		EnableOpenMetrics: true,
		Registry:          m.registry,
	})
}
