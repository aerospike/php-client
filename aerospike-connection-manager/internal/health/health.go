// Package health provides Kubernetes-style liveness and readiness probes.
//
// The distinction follows standard practice and matters operationally:
//
//   - Liveness ("/livez") answers "is the process still working?". It must be
//     cheap and must not depend on external systems — if it did, a transient
//     dependency outage would make Kubernetes restart otherwise-healthy pods.
//   - Readiness ("/readyz") answers "can the process serve traffic right now?".
//     This is where dependency checks belong (Aerospike connectivity): when it
//     fails the pod is removed from load balancing but not restarted.
//   - Health ("/healthz") is the aggregate of both, kept for compatibility with
//     tooling that expects the legacy combined endpoint.
package health

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"sort"
	"sync"
	"time"
)

// defaultTimeout bounds how long the whole set of checks for one request may
// run, so a single hung check cannot wedge a probe indefinitely.
const defaultTimeout = 2 * time.Second

// Check reports the health of a single subsystem. A nil error means healthy.
type Check func(ctx context.Context) error

// Connectivity is anything that can report whether it is currently connected,
// such as the Aerospike client. It keeps this package free of an Aerospike
// dependency while still offering a ready-made readiness check.
type Connectivity interface {
	IsConnected() bool
}

// Connected adapts a Connectivity into a Check that fails when the dependency
// reports itself as disconnected.
func Connected(c Connectivity) Check {
	return func(context.Context) error {
		if !c.IsConnected() {
			return fmt.Errorf("not connected")
		}
		return nil
	}
}

type namedCheck struct {
	name  string
	check Check
}

// Checker aggregates liveness and readiness checks and exposes them as HTTP
// handlers. The zero value is not usable; call NewChecker.
type Checker struct {
	// Timeout bounds the execution of all checks for a single request.
	Timeout time.Duration

	mu        sync.RWMutex
	liveness  []namedCheck
	readiness []namedCheck
}

// NewChecker returns an empty Checker with the default per-request timeout.
func NewChecker() *Checker {
	return &Checker{Timeout: defaultTimeout}
}

// AddLiveness registers a liveness check. Liveness checks should be cheap and
// free of external dependencies.
func (c *Checker) AddLiveness(name string, check Check) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.liveness = append(c.liveness, namedCheck{name, check})
}

// AddReadiness registers a readiness check, typically a dependency probe.
func (c *Checker) AddReadiness(name string, check Check) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.readiness = append(c.readiness, namedCheck{name, check})
}

// LivenessHandler serves the liveness probe.
func (c *Checker) LivenessHandler() http.Handler {
	return c.handler(func() []namedCheck { return c.snapshot(true, false) })
}

// ReadinessHandler serves the readiness probe.
func (c *Checker) ReadinessHandler() http.Handler {
	return c.handler(func() []namedCheck { return c.snapshot(false, true) })
}

// HealthHandler serves the aggregate of liveness and readiness checks.
func (c *Checker) HealthHandler() http.Handler {
	return c.handler(func() []namedCheck { return c.snapshot(true, true) })
}

// snapshot returns a copy of the requested check sets so checks run without
// holding the lock.
func (c *Checker) snapshot(live, ready bool) []namedCheck {
	c.mu.RLock()
	defer c.mu.RUnlock()
	out := make([]namedCheck, 0, len(c.liveness)+len(c.readiness))
	if live {
		out = append(out, c.liveness...)
	}
	if ready {
		out = append(out, c.readiness...)
	}
	return out
}

type response struct {
	Status string            `json:"status"`
	Checks map[string]string `json:"checks,omitempty"`
}

func (c *Checker) handler(selector func() []namedCheck) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		timeout := c.Timeout
		if timeout <= 0 {
			timeout = defaultTimeout
		}
		ctx, cancel := context.WithTimeout(r.Context(), timeout)
		defer cancel()

		checks := selector()
		results := make(map[string]string, len(checks))
		healthy := true
		for _, nc := range checks {
			if err := nc.check(ctx); err != nil {
				results[nc.name] = err.Error()
				healthy = false
			} else {
				results[nc.name] = "ok"
			}
		}

		body := response{Status: "ok", Checks: results}
		code := http.StatusOK
		if !healthy {
			body.Status = "error"
			code = http.StatusServiceUnavailable
		}

		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(code)
		_ = json.NewEncoder(w).Encode(body)
	}
}

// CheckNames returns the registered readiness check names, sorted. Intended for
// diagnostics and tests.
func (c *Checker) CheckNames() []string {
	c.mu.RLock()
	defer c.mu.RUnlock()
	names := make([]string, 0, len(c.readiness))
	for _, nc := range c.readiness {
		names = append(names, nc.name)
	}
	sort.Strings(names)
	return names
}
