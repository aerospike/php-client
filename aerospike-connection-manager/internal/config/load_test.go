package config

import (
	"os"
	"path/filepath"
	"testing"
)

func writeTempConfig(t *testing.T, body string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "asld.toml")
	if err := os.WriteFile(path, []byte(body), 0o600); err != nil {
		t.Fatalf("write temp config: %v", err)
	}
	return path
}

func TestLoadFromFile(t *testing.T) {
	path := writeTempConfig(t, `
[cluster]
host = "127.0.0.1:3000"

[management]
address = ":7777"

[management.pprof]
enabled = true

[management.metrics]
path = "/custom-metrics"
`)

	m, err := Load(path, nil)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if m.Address != ":7777" {
		t.Errorf("address = %q, want :7777", m.Address)
	}
	if !m.Pprof.Enabled {
		t.Error("pprof should be enabled from file")
	}
	if m.Metrics.Path != "/custom-metrics" {
		t.Errorf("metrics path = %q", m.Metrics.Path)
	}
}

func TestLoadMissingFile(t *testing.T) {
	m, err := Load(filepath.Join(t.TempDir(), "does-not-exist.toml"), nil)
	if err != nil {
		t.Fatalf("missing file should not error: %v", err)
	}
	if m.Address != DefaultAddress {
		t.Errorf("address = %q, want default", m.Address)
	}
}

func TestLoadMissingManagementSection(t *testing.T) {
	path := writeTempConfig(t, "[cluster]\nhost = \"127.0.0.1:3000\"\n")
	m, err := Load(path, nil)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if m != Default() {
		t.Errorf("config without [management] should equal Default(), got %+v", m)
	}
}
