package health

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
)

type fakeConn struct{ connected bool }

func (f fakeConn) IsConnected() bool { return f.connected }

func do(t *testing.T, h http.Handler) (int, response) {
	t.Helper()
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/", nil))
	var body response
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatalf("decode body %q: %v", rec.Body.String(), err)
	}
	return rec.Code, body
}

func TestLivenessNoChecks(t *testing.T) {
	c := NewChecker()
	code, body := do(t, c.LivenessHandler())
	if code != http.StatusOK || body.Status != "ok" {
		t.Fatalf("liveness = %d %q, want 200 ok", code, body.Status)
	}
}

func TestReadinessPasses(t *testing.T) {
	c := NewChecker()
	c.AddReadiness("aerospike", Connected(fakeConn{connected: true}))

	code, body := do(t, c.ReadinessHandler())
	if code != http.StatusOK {
		t.Fatalf("status = %d, want 200", code)
	}
	if body.Checks["aerospike"] != "ok" {
		t.Fatalf("check result = %q, want ok", body.Checks["aerospike"])
	}
}

func TestReadinessFails(t *testing.T) {
	c := NewChecker()
	c.AddReadiness("aerospike", Connected(fakeConn{connected: false}))

	code, body := do(t, c.ReadinessHandler())
	if code != http.StatusServiceUnavailable {
		t.Fatalf("status = %d, want 503", code)
	}
	if body.Status != "error" {
		t.Fatalf("status field = %q, want error", body.Status)
	}
	if body.Checks["aerospike"] != "not connected" {
		t.Fatalf("check detail = %q", body.Checks["aerospike"])
	}
}

func TestLivenessIndependentOfReadiness(t *testing.T) {
	c := NewChecker()
	c.AddReadiness("aerospike", Connected(fakeConn{connected: false}))

	// A failing dependency must not affect liveness — otherwise an Aerospike
	// outage would trigger pod restarts instead of just removing from routing.
	if code, _ := do(t, c.LivenessHandler()); code != http.StatusOK {
		t.Fatalf("liveness status = %d, want 200 despite failing readiness", code)
	}
}

func TestHealthAggregates(t *testing.T) {
	c := NewChecker()
	c.AddLiveness("self", func(context.Context) error { return nil })
	c.AddReadiness("aerospike", Connected(fakeConn{connected: false}))

	code, body := do(t, c.HealthHandler())
	if code != http.StatusServiceUnavailable {
		t.Fatalf("healthz status = %d, want 503", code)
	}
	if body.Checks["self"] != "ok" || body.Checks["aerospike"] == "ok" {
		t.Fatalf("healthz should report both checks: %+v", body.Checks)
	}
}

func TestReadinessReportsCheckError(t *testing.T) {
	c := NewChecker()
	c.AddReadiness("custom", func(context.Context) error { return errors.New("boom") })

	code, body := do(t, c.ReadinessHandler())
	if code != http.StatusServiceUnavailable || body.Checks["custom"] != "boom" {
		t.Fatalf("got %d %+v", code, body.Checks)
	}
}

func TestCheckNames(t *testing.T) {
	c := NewChecker()
	c.AddReadiness("b", Connected(fakeConn{connected: true}))
	c.AddReadiness("a", Connected(fakeConn{connected: true}))
	names := c.CheckNames()
	if len(names) != 2 || names[0] != "a" || names[1] != "b" {
		t.Fatalf("CheckNames = %v, want [a b]", names)
	}
}
