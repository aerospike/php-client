package metrics

import (
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestNewRegistersRuntimeCollectors(t *testing.T) {
	m := New()

	families, err := m.Registry().Gather()
	if err != nil {
		t.Fatalf("Gather: %v", err)
	}
	if len(families) == 0 {
		t.Fatal("expected at least the Go runtime collector to register metrics")
	}

	names := make(map[string]bool, len(families))
	for _, f := range families {
		names[f.GetName()] = true
	}
	if !names["go_goroutines"] {
		t.Errorf("go_goroutines not exposed; got families %v", keys(names))
	}
}

func TestHandlerServesPrometheusFormat(t *testing.T) {
	m := New()
	srv := httptest.NewServer(m.Handler())
	defer srv.Close()

	resp, err := http.Get(srv.URL)
	if err != nil {
		t.Fatalf("GET: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want 200", resp.StatusCode)
	}
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("read body: %v", err)
	}
	if !strings.Contains(string(body), "go_goroutines") {
		t.Errorf("metrics output missing go_goroutines:\n%s", body)
	}
}

func TestInterceptorsNonNil(t *testing.T) {
	m := New()
	if m.UnaryServerInterceptor() == nil {
		t.Error("unary interceptor is nil")
	}
	if m.StreamServerInterceptor() == nil {
		t.Error("stream interceptor is nil")
	}
}

func keys(m map[string]bool) []string {
	out := make([]string, 0, len(m))
	for k := range m {
		out = append(out, k)
	}
	return out
}
