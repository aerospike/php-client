package metrics

import (
	"errors"
	"strings"
	"testing"

	"github.com/prometheus/client_golang/prometheus/testutil"
)

type fakeStats struct {
	stats map[string]interface{}
	err   error
}

func (f fakeStats) Stats() (map[string]interface{}, error) {
	return f.stats, f.err
}

func healthyStats() map[string]interface{} {
	return map[string]interface{}{
		"open-connections": 12,
		"total-nodes":      3,
		"cluster-aggregated-stats": map[string]interface{}{
			"connections-attempts":      float64(100),
			"connections-failed":        float64(2),
			"connections-pool-empty":    float64(5),
			"connections-pool-overflow": float64(1),
			"connections-idle-dropped":  float64(7),
			"tends-failed":              float64(0),
		},
	}
}

func TestAerospikeCollectorHealthy(t *testing.T) {
	m := New()
	m.RegisterAerospike("main", fakeStats{stats: healthyStats()})

	expected := `
# HELP asld_aerospike_open_connections Current number of open connections from asld to Aerospike nodes.
# TYPE asld_aerospike_open_connections gauge
asld_aerospike_open_connections{cluster="main"} 12
# HELP asld_aerospike_up Whether the last Aerospike stats scrape for the cluster succeeded (1) or failed (0).
# TYPE asld_aerospike_up gauge
asld_aerospike_up{cluster="main"} 1
`
	if err := testutil.GatherAndCompare(m.Registry(), strings.NewReader(expected),
		"asld_aerospike_open_connections", "asld_aerospike_up"); err != nil {
		t.Fatal(err)
	}
}

func TestAerospikeCollectorCounters(t *testing.T) {
	m := New()
	m.RegisterAerospike("main", fakeStats{stats: healthyStats()})

	expected := `
# HELP asld_aerospike_connections_failed_total Cumulative failed connections from asld to Aerospike.
# TYPE asld_aerospike_connections_failed_total counter
asld_aerospike_connections_failed_total{cluster="main"} 2
# HELP asld_aerospike_tends_failed_total Cumulative failed cluster-tend cycles (asld could not reach an Aerospike node).
# TYPE asld_aerospike_tends_failed_total counter
asld_aerospike_tends_failed_total{cluster="main"} 0
`
	if err := testutil.GatherAndCompare(m.Registry(), strings.NewReader(expected),
		"asld_aerospike_connections_failed_total", "asld_aerospike_tends_failed_total"); err != nil {
		t.Fatal(err)
	}
}

func TestAerospikeCollectorScrapeError(t *testing.T) {
	m := New()
	m.RegisterAerospike("down", fakeStats{err: errors.New("not connected")})

	expected := `
# HELP asld_aerospike_up Whether the last Aerospike stats scrape for the cluster succeeded (1) or failed (0).
# TYPE asld_aerospike_up gauge
asld_aerospike_up{cluster="down"} 0
`
	if err := testutil.GatherAndCompare(m.Registry(), strings.NewReader(expected), "asld_aerospike_up"); err != nil {
		t.Fatal(err)
	}

	// A failed scrape must not emit the pool gauges for that cluster.
	if n := testutil.CollectAndCount(m.Registry(), "asld_aerospike_open_connections"); n != 0 {
		t.Errorf("open_connections series count = %d, want 0 for a failed scrape", n)
	}
}

func TestAerospikeCollectorMultiCluster(t *testing.T) {
	m := New()
	m.RegisterAerospike("a", fakeStats{stats: healthyStats()})
	m.RegisterAerospike("b", fakeStats{stats: healthyStats()})

	if n := testutil.CollectAndCount(m.Registry(), "asld_aerospike_up"); n != 2 {
		t.Errorf("up series count = %d, want 2 (one per cluster)", n)
	}
}
