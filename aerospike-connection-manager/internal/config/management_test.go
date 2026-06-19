package config

import (
	"flag"
	"testing"
)

func TestDefault(t *testing.T) {
	d := Default()
	if !d.Enabled {
		t.Fatal("management server should be enabled by default")
	}
	if d.Address != DefaultAddress {
		t.Fatalf("default address = %q, want %q", d.Address, DefaultAddress)
	}
	if d.Pprof.Enabled {
		t.Fatal("pprof must be disabled by default for security")
	}
	if d.Liveness.Path != "/livez" || d.Readiness.Path != "/readyz" || d.Health.Path != "/healthz" {
		t.Fatalf("unexpected default probe paths: %+v", d)
	}
}

func TestResolvePrecedence(t *testing.T) {
	file := map[string]any{
		"address": ":1111",
		"metrics": map[string]any{"path": "/file-metrics", "enabled": true},
	}
	env := map[string]string{
		keyAddress:     ":2222",
		keyMetricsPath: "/env-metrics",
	}
	flags := map[string]string{
		keyAddress: ":3333",
	}

	m, err := Resolve(file, env, flags)
	if err != nil {
		t.Fatalf("Resolve: %v", err)
	}

	if m.Address != ":3333" {
		t.Errorf("address = %q, want flag value :3333", m.Address)
	}
	if m.Metrics.Path != "/env-metrics" {
		t.Errorf("metrics path = %q, want env value /env-metrics", m.Metrics.Path)
	}
	if !m.Metrics.Enabled {
		t.Error("metrics enabled should remain true from file")
	}
	if m.Readiness.Path != "/readyz" {
		t.Errorf("readiness path = %q, want default /readyz", m.Readiness.Path)
	}
}

func TestResolveBoolOverride(t *testing.T) {
	m, err := Resolve(nil, map[string]string{keyPprofEnabled: "true"}, map[string]string{keyMetricsEnabled: "false"})
	if err != nil {
		t.Fatalf("Resolve: %v", err)
	}
	if !m.Pprof.Enabled {
		t.Error("pprof should be enabled via env")
	}
	if m.Metrics.Enabled {
		t.Error("metrics should be disabled via flag")
	}
}

func TestResolveInvalidBool(t *testing.T) {
	if _, err := Resolve(nil, map[string]string{keyEnabled: "notabool"}, nil); err == nil {
		t.Fatal("expected error for invalid boolean")
	}
}

func TestValidate(t *testing.T) {
	tests := []struct {
		name    string
		env     map[string]string
		wantErr bool
	}{
		{"empty address", map[string]string{keyAddress: "  "}, true},
		{"path without slash", map[string]string{keyMetricsPath: "metrics"}, true},
		{"duplicate paths", map[string]string{keyLivenessPath: "/x", keyReadinessPath: "/x"}, true},
		{"disabled duplicate is fine", map[string]string{keyLivenessPath: "/x", keyReadinessPath: "/x", keyReadinessEnabled: "false"}, false},
		{"disabled server skips validation", map[string]string{keyEnabled: "false", keyAddress: ""}, false},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			_, err := Resolve(nil, tc.env, nil)
			if (err != nil) != tc.wantErr {
				t.Fatalf("Resolve err = %v, wantErr = %v", err, tc.wantErr)
			}
		})
	}
}

func TestResolveFileSource(t *testing.T) {
	file := map[string]any{
		"address":   ":7000",
		"metrics":   map[string]any{"enabled": false},
		"readiness": map[string]any{"path": "/ready"},
	}
	m, err := Resolve(file, nil, nil)
	if err != nil {
		t.Fatalf("Resolve: %v", err)
	}
	if m.Address != ":7000" || m.Metrics.Enabled || m.Readiness.Path != "/ready" {
		t.Fatalf("file source not applied: %+v", m)
	}
}

func TestFromEnv(t *testing.T) {
	values := map[string]string{
		"ASLD_MANAGEMENT_ADDRESS": ":9999",
		"ASLD_PPROF_ENABLED":      "true",
	}
	lookup := func(k string) (string, bool) {
		v, ok := values[k]
		return v, ok
	}
	src := FromEnv(lookup)
	if src[keyAddress] != ":9999" {
		t.Errorf("address = %q", src[keyAddress])
	}
	if src[keyPprofEnabled] != "true" {
		t.Errorf("pprof enabled = %q", src[keyPprofEnabled])
	}
	if _, ok := src[keyMetricsPath]; ok {
		t.Error("unset env var must not appear in source map")
	}
}

func TestRegisterFlagsExplicitOnly(t *testing.T) {
	fs := flag.NewFlagSet("test", flag.ContinueOnError)
	collect := RegisterFlags(fs)

	if err := fs.Parse([]string{"-management-address=:8888", "-pprof-enabled=true"}); err != nil {
		t.Fatalf("parse: %v", err)
	}

	src := collect()
	if len(src) != 2 {
		t.Fatalf("expected only the 2 explicitly-set flags, got %+v", src)
	}
	if src[keyAddress] != ":8888" || src[keyPprofEnabled] != "true" {
		t.Fatalf("unexpected collected flags: %+v", src)
	}

	m, err := Resolve(nil, nil, src)
	if err != nil {
		t.Fatalf("Resolve: %v", err)
	}
	if m.Address != ":8888" || !m.Pprof.Enabled {
		t.Fatalf("flags not applied: %+v", m)
	}
	if m.Metrics.Path != "/metrics" {
		t.Errorf("metrics path = %q, want default", m.Metrics.Path)
	}
}
