# Observability and Operations

The Aerospike Connection Manager (ACM) exposes operational endpoints — Prometheus
metrics and Kubernetes-style health probes — on a single management HTTP server,
separate from the gRPC data path served over unix sockets.

- [Management server](#management-server)
- [Configuration](#configuration)
  - [Precedence](#precedence)
  - [Reference](#reference)
  - [Examples](#examples)
- [Endpoints](#endpoints)
- [Metrics](#metrics)
- [Kubernetes integration](#kubernetes-integration)
- [Security](#security)

## Management server

When enabled (the default), ACM starts one HTTP server that hosts every
operational endpoint on one port. This follows the common Go and Kubernetes
convention of a dedicated admin port: probes and metric scraping share a single
address while the data plane is untouched.

The default listen address is `:9145`, matching the `EXPOSE` directive in the
image. Each endpoint can be enabled, disabled and re-pathed independently.

The server shuts down gracefully on `SIGINT`/`SIGTERM`, draining in-flight
requests before the process exits.

## Configuration

Every setting can be provided through three sources, and each is optional.

### Precedence

From lowest to highest priority:

```
built-in defaults  <  [management] TOML section  <  ASLD_* env vars  <  CLI flags
```

The source closest to the process invocation wins: an explicit command-line flag
overrides an environment variable, which overrides the config file, which
overrides the built-in default. A flag only takes effect when it is actually
passed, so it never silently masks a lower-priority source with its default.

### Reference

| Setting          | TOML (`[management]`)        | Environment variable      | CLI flag               | Default          |
|------------------|------------------------------|---------------------------|------------------------|------------------|
| Server enabled   | `enabled`                    | `ASLD_MANAGEMENT_ENABLED` | `-management-enabled`  | `true`           |
| Listen address   | `address`                    | `ASLD_MANAGEMENT_ADDRESS` | `-management-address`  | `:9145`          |
| Metrics enabled  | `[management.metrics] enabled`   | `ASLD_METRICS_ENABLED`    | `-metrics-enabled`     | `true`           |
| Metrics path     | `[management.metrics] path`      | `ASLD_METRICS_PATH`       | `-metrics-path`        | `/metrics`       |
| Liveness enabled | `[management.liveness] enabled`  | `ASLD_LIVENESS_ENABLED`   | `-liveness-enabled`    | `true`           |
| Liveness path    | `[management.liveness] path`     | `ASLD_LIVENESS_PATH`      | `-liveness-path`       | `/livez`         |
| Readiness enabled| `[management.readiness] enabled` | `ASLD_READINESS_ENABLED`  | `-readiness-enabled`   | `true`           |
| Readiness path   | `[management.readiness] path`    | `ASLD_READINESS_PATH`     | `-readiness-path`      | `/readyz`        |
| Health enabled   | `[management.health] enabled`    | `ASLD_HEALTH_ENABLED`     | `-health-enabled`      | `true`           |
| Health path      | `[management.health] path`       | `ASLD_HEALTH_PATH`        | `-health-path`         | `/healthz`       |
| pprof enabled    | `[management.pprof] enabled`     | `ASLD_PPROF_ENABLED`      | `-pprof-enabled`       | `false`          |
| pprof path       | `[management.pprof] path`        | `ASLD_PPROF_PATH`         | `-pprof-path`          | `/debug/pprof/`  |

The configuration is validated at startup. A non-empty listen address is
required when the server is enabled, every enabled endpoint path must start with
`/`, and two enabled endpoints may not share a path.

### Examples

Config file — the `[management]` table lives in the same TOML file as the
cluster definitions:

```toml
[management]
address = ":9145"

[management.metrics]
enabled = true
path = "/metrics"

[management.pprof]
enabled = false
```

Environment variables:

```sh
export ASLD_MANAGEMENT_ADDRESS=":9145"
export ASLD_METRICS_ENABLED=true
export ASLD_PPROF_ENABLED=false
```

Command-line flags (override both of the above):

```sh
aerospike-connection-manager -config-file /etc/aerospike-connection-manager/asld.toml \
  -management-address ":9145" \
  -pprof-enabled=false
```

## Endpoints

| Path            | Purpose                                                                                          |
|-----------------|--------------------------------------------------------------------------------------------------|
| `/metrics`      | Prometheus / OpenMetrics exposition.                                                             |
| `/livez`        | Liveness. Cheap, dependency-free. A failure means the process should be restarted.               |
| `/readyz`       | Readiness. Fails while any configured Aerospike cluster is disconnected.                          |
| `/healthz`      | Aggregate of liveness and readiness, for tooling that expects a single combined endpoint.        |
| `/debug/pprof/` | `net/http/pprof` profiling index (opt-in).                                                       |

Liveness and readiness are deliberately distinct. Liveness must not depend on
Aerospike: if it did, a cluster outage would restart otherwise-healthy pods
instead of just draining them. Dependency checks belong on readiness, which
removes the pod from load balancing without a restart.

The probes return `200` when healthy and `503` otherwise, with a JSON body:

```json
{
  "status": "error",
  "checks": {
    "main": "not connected"
  }
}
```

## Metrics

All metrics are collected unconditionally; the `/metrics` endpoint only exposes
them when enabled.

**Go runtime** — `go_goroutines`, `go_threads`, `go_gc_duration_seconds`,
`go_memstats_*`, `go_info`, and related collectors.

**Process** (on platforms that expose process statistics, e.g. Linux) —
`process_cpu_seconds_total`, `process_resident_memory_bytes`,
`process_open_fds`, `process_max_fds`.

**gRPC server** — labelled by `grpc_service`, `grpc_method`, `grpc_type` (and
`grpc_code` where applicable):

- `grpc_server_started_total`
- `grpc_server_handled_total`
- `grpc_server_msg_received_total`
- `grpc_server_msg_sent_total`
- `grpc_server_handling_seconds_*` (histogram; buckets tuned for ACM's
  sub-millisecond profile)

Every method is pre-registered with a zero count, so dashboards are populated
from the first scrape after a deploy rather than only once a method is first
called.

**Aerospike connection pool** — one series per cluster, labelled `cluster`,
read from the Aerospike client at scrape time:

| Metric                                          | Type    | Meaning                                                            |
|-------------------------------------------------|---------|--------------------------------------------------------------------|
| `asld_aerospike_up`                             | gauge   | `1` if the last stats scrape succeeded, `0` otherwise.             |
| `asld_aerospike_open_connections`               | gauge   | Open connections to Aerospike nodes.                               |
| `asld_aerospike_nodes_total`                    | gauge   | Aerospike nodes currently tracked.                                 |
| `asld_aerospike_connections_attempts_total`     | counter | Connection attempts.                                               |
| `asld_aerospike_connections_failed_total`       | counter | Failed connections.                                                |
| `asld_aerospike_connections_pool_empty_total`   | counter | Times the pool was exhausted and ACM waited or opened a new conn.  |
| `asld_aerospike_connections_pool_overflow_total`| counter | Connections dropped because the pool was at capacity.              |
| `asld_aerospike_connections_idle_dropped_total` | counter | Connections closed due to idle timeout.                            |
| `asld_aerospike_tends_failed_total`             | counter | Failed cluster-tend cycles (a node could not be reached).          |

A cluster whose stats scrape fails reports `asld_aerospike_up{cluster="…"} 0`
and omits its pool gauges, so one unreachable cluster never hides the others.

## Kubernetes integration

Pod spec — expose the management port and wire the probes:

```yaml
ports:
  - name: management
    containerPort: 9145
livenessProbe:
  httpGet:
    path: /livez
    port: management
readinessProbe:
  httpGet:
    path: /readyz
    port: management
```

Scraping — with the Prometheus Operator:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: PodMonitor
metadata:
  name: aerospike-connection-manager
spec:
  selector:
    matchLabels:
      app: aerospike-connection-manager
  podMetricsEndpoints:
    - port: management
      path: /metrics
```

## Security

- `/metrics` and the probes are unauthenticated. Restrict access to the
  monitoring network with a `NetworkPolicy` or a service-mesh ACL.
- pprof is disabled by default. It exposes process internals (heap, goroutines,
  CPU profiles) without authentication; only enable it for debugging and never
  expose it publicly.
- The metric values do not contain request or record contents, but they do
  reveal traffic patterns and error rates.
