package config

import (
	"os"
	"path/filepath"
	"testing"

	as "github.com/aerospike/aerospike-client-go/v7"

	"github.com/aerospike/php-client/asld/common/client"
)

func writeConfig(t *testing.T, body string) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), "asld.toml")
	if err := os.WriteFile(path, []byte(body), 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	return path
}

func TestReadSkipsManagementSection(t *testing.T) {
	path := writeConfig(t, `
[cluster]
host = "127.0.0.1:3000"

[management]
address = ":9145"
metrics-enabled = true
`)

	clusters, _, err := Read(path)
	if err != nil {
		t.Fatalf("Read: %v", err)
	}

	if _, ok := clusters["management"]; ok {
		t.Fatal("[management] must not be parsed as an Aerospike cluster")
	}

	if _, ok := clusters["cluster"]; !ok {
		t.Fatal("real cluster section should be present")
	}
}

func TestReadSkipsNonClusterTables(t *testing.T) {
	// [uda]/[uda_instance] declare neither host nor socket, so they must not be
	// mistaken for Aerospike clusters.
	path := writeConfig(t, `
[c]
host = "127.0.0.1:3000"

[uda]
agent-port = 8001
store-file = "default1.store"

[uda_instance]
store-file = "test.store"
`)

	clusters, _, err := Read(path)
	if err != nil {
		t.Fatalf("Read: %v", err)
	}

	if len(clusters) != 1 {
		t.Fatalf("expected only the real cluster, got %d: %v", len(clusters), keysOf(clusters))
	}

	if _, ok := clusters["c"]; !ok {
		t.Fatalf("real cluster missing, got %v", keysOf(clusters))
	}
}

func TestReadSocketOnlyTableIsCluster(t *testing.T) {
	// A table with only a socket (host falls back to the default seed) is still
	// a usable cluster and must be kept.
	clusters, _, err := Read(writeConfig(t, "[c]\nsocket = \"/tmp/c.sock\"\n"))
	if err != nil {
		t.Fatalf("Read: %v", err)
	}

	if _, ok := clusters["c"]; !ok {
		t.Fatalf("socket-only cluster should be kept, got %v", keysOf(clusters))
	}
}

func keysOf(m map[string]*client.AerospikeConfig) []string {
	out := make([]string, 0, len(m))
	for k := range m {
		out = append(out, k)
	}

	return out
}

func TestReadScalarsAndDurations(t *testing.T) {
	path := writeConfig(t, `
[c]
socket = "/tmp/c.sock"
host = "127.0.0.1:3000"
user = "alice"
cluster-name = "prod"
timeout = "30s"
idle-timeout = "5s"
connection-queue-size = 100
max-error-rate = 7
limit-connections-to-queue-size = true
fail-if-not-connected = true
rack-aware = true
rack-ids = [1, 2, 3]
`)

	ac := mustCluster(t, path)
	if ac.Socket != "/tmp/c.sock" {
		t.Errorf("socket = %q", ac.Socket)
	}

	if ac.User != "alice" {
		t.Errorf("user = %q", ac.User)
	}

	if ac.ClusterName != "prod" {
		t.Errorf("cluster-name = %q", ac.ClusterName)
	}

	if ac.Timeout.String() != "30s" || ac.IdleTimeout.String() != "5s" {
		t.Errorf("durations = %s / %s", ac.Timeout, ac.IdleTimeout)
	}

	if ac.ConnectionQueueSize != 100 || ac.MaxErrorRate != 7 {
		t.Errorf("ints = %d / %d", ac.ConnectionQueueSize, ac.MaxErrorRate)
	}

	if !ac.LimitConnectionsToQueueSize || !ac.FailIfNotConnected || !ac.RackAware {
		t.Errorf("bools not set: %+v", ac)
	}

	if len(ac.RackIds) != 3 || ac.RackIds[0] != 1 || ac.RackIds[2] != 3 {
		t.Errorf("rack-ids = %v", ac.RackIds)
	}
}

func TestReadPasswordClearAndB64(t *testing.T) {
	plain := mustCluster(t, writeConfig(t, "[c]\nhost=\"127.0.0.1\"\npassword=\"s3cret\"\n"))
	if plain.Password != "s3cret" {
		t.Errorf("clear password = %q", plain.Password)
	}

	b64 := mustCluster(t, writeConfig(t, "[c]\nhost=\"127.0.0.1\"\npassword=\"b64:dGVzdA==\"\n"))
	if b64.Password != "test" {
		t.Errorf("b64 password = %q, want test", b64.Password)
	}
}

func TestReadPasswordFromEnv(t *testing.T) {
	t.Setenv("ASLD_TEST_SECRET", "envpass")

	ac := mustCluster(t, writeConfig(t, "[c]\nhost=\"127.0.0.1\"\npassword=\"env:ASLD_TEST_SECRET\"\n"))
	if ac.Password != "envpass" {
		t.Errorf("env password = %q, want envpass", ac.Password)
	}
}

func TestReadAuthMode(t *testing.T) {
	ac := mustCluster(t, writeConfig(t, "[c]\nhost=\"127.0.0.1\"\nauth=\"EXTERNAL\"\n"))
	if ac.AuthMode != as.AuthModeExternal {
		t.Errorf("auth mode = %v, want EXTERNAL", ac.AuthMode)
	}
}

func TestReadHostParsing(t *testing.T) {
	tests := []struct {
		name     string
		host     string
		wantHost string
		wantPort int
		wantTLS  string
		wantLen  int
	}{
		{"host only, default port", "1.2.3.4", "1.2.3.4", 3000, "", 1},
		{"host and port", "1.2.3.4:5000", "1.2.3.4", 5000, "", 1},
		{"host tls port", "1.2.3.4:tlsname:5001", "1.2.3.4", 5001, "tlsname", 1},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			ac := mustCluster(t, writeConfig(t, "[c]\nhost=\""+tc.host+"\"\n"))
			if len(ac.Seeds) != tc.wantLen {
				t.Fatalf("seeds = %d, want %d", len(ac.Seeds), tc.wantLen)
			}

			s := ac.Seeds[0]
			if s.Host != tc.wantHost || s.Port != tc.wantPort || s.TLSName != tc.wantTLS {
				t.Errorf("seed = %+v, want host=%s port=%d tls=%s", s, tc.wantHost, tc.wantPort, tc.wantTLS)
			}
		})
	}
}

func TestReadMultipleSeeds(t *testing.T) {
	ac := mustCluster(t, writeConfig(t, "[c]\nhost=\"1.1.1.1:3001,2.2.2.2:3002\"\n"))
	if len(ac.Seeds) != 2 {
		t.Fatalf("seeds = %d, want 2", len(ac.Seeds))
	}

	if ac.Seeds[0].Port != 3001 || ac.Seeds[1].Port != 3002 {
		t.Errorf("ports = %d / %d", ac.Seeds[0].Port, ac.Seeds[1].Port)
	}
}

func TestReadTLSCAFileGatedByEnable(t *testing.T) {
	// b64-encoded "ca-bytes" exercises the cert parser without touching the filesystem.
	body := func(enable bool) string {
		e := "false"
		if enable {
			e = "true"
		}

		return "[c]\nhost=\"127.0.0.1\"\ntls-enable=" + e + "\ntls-cafile=\"b64:Y2EtYnl0ZXM=\"\n"
	}

	on := mustCluster(t, writeConfig(t, body(true)))
	if len(on.RootCA) != 1 || string(on.RootCA[0]) != "ca-bytes" {
		t.Errorf("RootCA with tls-enable = %v", on.RootCA)
	}

	off := mustCluster(t, writeConfig(t, body(false)))
	if len(off.RootCA) != 0 {
		t.Errorf("RootCA must stay empty when tls-enable is false, got %v", off.RootCA)
	}
}

func TestReadClustersNamespace(t *testing.T) {
	path := writeConfig(t, `
[clusters.a]
host = "1.1.1.1:3001"

[clusters.b]
host = "2.2.2.2:3002"
`)

	clusters, legacy, err := Read(path)
	if err != nil {
		t.Fatalf("Read: %v", err)
	}

	if len(clusters) != 2 || clusters["a"] == nil || clusters["b"] == nil {
		t.Fatalf("expected clusters a and b, got %v", keysOf(clusters))
	}

	if len(legacy) != 0 {
		t.Errorf("namespaced clusters must not be reported as legacy, got %v", legacy)
	}
}

func TestReadLegacyReportsDeprecation(t *testing.T) {
	path := writeConfig(t, `
[cluster_one]
host = "1.1.1.1:3001"

[cluster_two]
host = "2.2.2.2:3002"
`)

	clusters, legacy, err := Read(path)
	if err != nil {
		t.Fatalf("Read: %v", err)
	}

	if len(clusters) != 2 {
		t.Fatalf("expected 2 clusters, got %v", keysOf(clusters))
	}

	if len(legacy) != 2 || legacy[0] != "cluster_one" || legacy[1] != "cluster_two" {
		t.Errorf("legacy clusters = %v, want sorted [cluster_one cluster_two]", legacy)
	}
}

func TestReadDuplicateClusterAcrossForms(t *testing.T) {
	path := writeConfig(t, `
[dup]
host = "1.1.1.1:3001"

[clusters.dup]
host = "2.2.2.2:3002"
`)

	if _, _, err := Read(path); err == nil {
		t.Fatal("expected an error for a cluster name used by both forms")
	}
}

func mustCluster(t *testing.T, path string) *client.AerospikeConfig {
	t.Helper()

	clusters, _, err := Read(path)
	if err != nil {
		t.Fatalf("Read: %v", err)
	}

	ac, ok := clusters["c"]
	if !ok {
		t.Fatalf("cluster \"c\" not found")
	}

	return ac
}
