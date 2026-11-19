package management

import (
	"context"
	"net/http"
	"testing"
	"time"

	"github.com/aerospike/php-client/asld/internal/config"
	"github.com/aerospike/php-client/asld/internal/health"
)

func stubMetrics() http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write([]byte("# metrics"))
	})
}

func allEnabled() config.Management {
	c := config.Default()
	c.Address = "127.0.0.1:0"
	c.Pprof.Enabled = true
	return c
}

func startServer(t *testing.T, cfg config.Management) *Server {
	t.Helper()
	checker := health.NewChecker()
	checker.AddReadiness("aerospike", health.Connected(connFake{true}))

	s := New(cfg, stubMetrics(), checker, nil)
	if err := s.Start(); err != nil {
		t.Fatalf("Start: %v", err)
	}
	t.Cleanup(func() {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		_ = s.Shutdown(ctx)
	})
	return s
}

type connFake struct{ ok bool }

func (c connFake) IsConnected() bool { return c.ok }

func get(t *testing.T, url string) int {
	t.Helper()
	resp, err := http.Get(url)
	if err != nil {
		t.Fatalf("GET %s: %v", url, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode
}

func TestServerRoutesEnabled(t *testing.T) {
	s := startServer(t, allEnabled())
	base := "http://" + s.Addr()

	for path, want := range map[string]int{
		"/metrics":             http.StatusOK,
		"/livez":               http.StatusOK,
		"/readyz":              http.StatusOK,
		"/healthz":             http.StatusOK,
		"/debug/pprof/":        http.StatusOK,
		"/debug/pprof/cmdline": http.StatusOK,
	} {
		if code := get(t, base+path); code != want {
			t.Errorf("GET %s = %d, want %d", path, code, want)
		}
	}
}

func TestServerDisabledEndpointReturns404(t *testing.T) {
	cfg := allEnabled()
	cfg.Metrics.Enabled = false
	s := startServer(t, cfg)

	if code := get(t, "http://"+s.Addr()+"/metrics"); code != http.StatusNotFound {
		t.Errorf("disabled /metrics = %d, want 404", code)
	}
	// Probes remain available.
	if code := get(t, "http://"+s.Addr()+"/readyz"); code != http.StatusOK {
		t.Errorf("/readyz = %d, want 200", code)
	}
}

func TestServerReadinessReflectsDependency(t *testing.T) {
	checker := health.NewChecker()
	checker.AddReadiness("aerospike", health.Connected(connFake{false}))

	cfg := allEnabled()
	s := New(cfg, stubMetrics(), checker, nil)
	if err := s.Start(); err != nil {
		t.Fatalf("Start: %v", err)
	}
	defer func() {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		_ = s.Shutdown(ctx)
	}()

	if code := get(t, "http://"+s.Addr()+"/readyz"); code != http.StatusServiceUnavailable {
		t.Errorf("/readyz with disconnected dependency = %d, want 503", code)
	}
}

func TestServerDisabledIsInert(t *testing.T) {
	cfg := config.Default()
	cfg.Enabled = false
	s := New(cfg, stubMetrics(), health.NewChecker(), nil)
	if err := s.Start(); err != nil {
		t.Fatalf("Start on disabled server: %v", err)
	}
	if s.Addr() != "" {
		t.Errorf("disabled server should not bind, got %q", s.Addr())
	}
	if err := s.Shutdown(context.Background()); err != nil {
		t.Errorf("Shutdown on disabled server: %v", err)
	}
}

func TestServerStartBindError(t *testing.T) {
	cfg := config.Default()
	cfg.Address = "127.0.0.1:0"
	first := New(cfg, stubMetrics(), health.NewChecker(), nil)
	if err := first.Start(); err != nil {
		t.Fatalf("first Start: %v", err)
	}
	defer func() { _ = first.Shutdown(context.Background()) }()

	// Re-binding the already-taken address must surface synchronously.
	clash := config.Default()
	clash.Address = first.Addr()
	second := New(clash, stubMetrics(), health.NewChecker(), nil)
	if err := second.Start(); err == nil {
		_ = second.Shutdown(context.Background())
		t.Fatal("expected bind error on duplicate address")
	}
}
