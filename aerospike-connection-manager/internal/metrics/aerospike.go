package metrics

import (
	"sync"

	"github.com/prometheus/client_golang/prometheus"
)

const (
	aerospikeNamespace = "asld"
	aerospikeSubsystem = "aerospike"
)

// StatsProvider is the subset of the Aerospike client used to read
// connection-pool statistics. Keeping it as a local interface (returning the
// standard error type) leaves this package free of an Aerospike dependency and
// lets tests drive the collector with a fake; the main package adapts
// *aerospike.Client onto it.
type StatsProvider interface {
	Stats() (map[string]interface{}, error)
}

// aerospikeCollector is a prometheus.Collector that reads connection-pool
// statistics from one or more Aerospike clients at scrape time. Reading on
// scrape (rather than polling on a timer) keeps the values fresh, removes a
// background goroutine, and lets Prometheus drive the collection cadence.
type aerospikeCollector struct {
	mu        sync.RWMutex
	providers map[string]StatsProvider

	up              *prometheus.Desc
	openConnections *prometheus.Desc
	nodesTotal      *prometheus.Desc
	connAttempts    *prometheus.Desc
	connFailed      *prometheus.Desc
	poolEmpty       *prometheus.Desc
	poolOverflow    *prometheus.Desc
	idleDropped     *prometheus.Desc
	tendsFailed     *prometheus.Desc
}

func newAerospikeCollector() *aerospikeCollector {
	labels := []string{"cluster"}
	desc := func(name, help string) *prometheus.Desc {
		return prometheus.NewDesc(
			prometheus.BuildFQName(aerospikeNamespace, aerospikeSubsystem, name),
			help, labels, nil,
		)
	}
	return &aerospikeCollector{
		providers:       make(map[string]StatsProvider),
		up:              desc("up", "Whether the last Aerospike stats scrape for the cluster succeeded (1) or failed (0)."),
		openConnections: desc("open_connections", "Current number of open connections from asld to Aerospike nodes."),
		nodesTotal:      desc("nodes_total", "Number of Aerospike nodes currently tracked by the client."),
		connAttempts:    desc("connections_attempts_total", "Cumulative connection attempts from asld to Aerospike."),
		connFailed:      desc("connections_failed_total", "Cumulative failed connections from asld to Aerospike."),
		poolEmpty:       desc("connections_pool_empty_total", "Cumulative times the connection pool was exhausted and asld had to wait or open a new connection."),
		poolOverflow:    desc("connections_pool_overflow_total", "Cumulative connections dropped because the pool was already at capacity."),
		idleDropped:     desc("connections_idle_dropped_total", "Cumulative connections closed because they exceeded the idle timeout."),
		tendsFailed:     desc("tends_failed_total", "Cumulative failed cluster-tend cycles (asld could not reach an Aerospike node)."),
	}
}

// register adds a cluster's stats provider to the collector. Safe to call
// concurrently with scrapes.
func (c *aerospikeCollector) register(cluster string, p StatsProvider) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.providers[cluster] = p
}

// Describe implements prometheus.Collector.
func (c *aerospikeCollector) Describe(ch chan<- *prometheus.Desc) {
	ch <- c.up
	ch <- c.openConnections
	ch <- c.nodesTotal
	ch <- c.connAttempts
	ch <- c.connFailed
	ch <- c.poolEmpty
	ch <- c.poolOverflow
	ch <- c.idleDropped
	ch <- c.tendsFailed
}

// Collect implements prometheus.Collector. A cluster whose Stats call fails is
// reported with up=0 and otherwise omitted, so a single unreachable cluster
// never blocks the metrics of the others.
func (c *aerospikeCollector) Collect(ch chan<- prometheus.Metric) {
	c.mu.RLock()
	providers := make(map[string]StatsProvider, len(c.providers))
	for name, p := range c.providers {
		providers[name] = p
	}
	c.mu.RUnlock()

	for cluster, provider := range providers {
		stats, err := provider.Stats()
		if err != nil || stats == nil {
			ch <- prometheus.MustNewConstMetric(c.up, prometheus.GaugeValue, 0, cluster)
			continue
		}
		ch <- prometheus.MustNewConstMetric(c.up, prometheus.GaugeValue, 1, cluster)

		ch <- prometheus.MustNewConstMetric(c.openConnections, prometheus.GaugeValue, statFloat(stats, "open-connections"), cluster)
		ch <- prometheus.MustNewConstMetric(c.nodesTotal, prometheus.GaugeValue, statFloat(stats, "total-nodes"), cluster)

		agg := aggregatedStats(stats)
		ch <- prometheus.MustNewConstMetric(c.connAttempts, prometheus.CounterValue, statFloat(agg, "connections-attempts"), cluster)
		ch <- prometheus.MustNewConstMetric(c.connFailed, prometheus.CounterValue, statFloat(agg, "connections-failed"), cluster)
		ch <- prometheus.MustNewConstMetric(c.poolEmpty, prometheus.CounterValue, statFloat(agg, "connections-pool-empty"), cluster)
		ch <- prometheus.MustNewConstMetric(c.poolOverflow, prometheus.CounterValue, statFloat(agg, "connections-pool-overflow"), cluster)
		ch <- prometheus.MustNewConstMetric(c.idleDropped, prometheus.CounterValue, statFloat(agg, "connections-idle-dropped"), cluster)
		ch <- prometheus.MustNewConstMetric(c.tendsFailed, prometheus.CounterValue, statFloat(agg, "tends-failed"), cluster)
	}
}

// aggregatedStats returns the cluster-aggregated-stats sub-map produced by the
// Aerospike client, or an empty map when it is absent.
func aggregatedStats(stats map[string]interface{}) map[string]interface{} {
	if agg, ok := stats["cluster-aggregated-stats"].(map[string]interface{}); ok {
		return agg
	}
	return map[string]interface{}{}
}

// statFloat reads a numeric stat regardless of whether it arrived as a native
// int (top-level values set by the client) or a float64 (everything that has
// been JSON round-tripped through the client's Stats implementation).
func statFloat(m map[string]interface{}, key string) float64 {
	switch v := m[key].(type) {
	case float64:
		return v
	case int:
		return float64(v)
	case int64:
		return float64(v)
	default:
		return 0
	}
}
