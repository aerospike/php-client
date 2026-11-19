// Package config resolves the operational ("management") configuration of the
// Aerospike Connection Manager from layered sources.
//
// The management configuration controls the HTTP admin server that exposes
// Prometheus metrics and Kubernetes-style health probes. It is intentionally
// separate from the per-cluster Aerospike configuration handled by
// common/config, because these settings are process-global rather than
// per-cluster.
//
// Resolution precedence, from lowest to highest, is:
//
//	built-in defaults  <  [management] TOML section  <  ASLD_* env vars  <  CLI flags
//
// The closer a source is to the invocation of the process, the higher its
// priority: an explicit command-line flag always wins, an environment variable
// overrides the config file, and the config file overrides the defaults.
//
// Source merging and struct mapping are delegated to knadh/koanf; this package
// only declares the field schema and the conventional env/flag names.
package config

import (
	"flag"
	"fmt"
	"strings"

	"github.com/go-viper/mapstructure/v2"
	"github.com/knadh/koanf/providers/confmap"
	"github.com/knadh/koanf/v2"
)

// DefaultAddress is the listen address of the management HTTP server when no
// other source overrides it. It matches the EXPOSE directive in the Dockerfile.
const DefaultAddress = ":9145"

// keyDelim separates nested keys in the koanf store and in the logical keys
// produced by the source adapters (e.g. "metrics.enabled").
const keyDelim = "."

// Endpoint describes a single HTTP endpoint served by the management server.
type Endpoint struct {
	Enabled bool `koanf:"enabled"`
	// Path is the URL the endpoint is served on; it must start with "/".
	Path string `koanf:"path"`
}

// Management is the fully resolved operational configuration.
type Management struct {
	// Enabled is the master switch for the management HTTP server. When false,
	// no admin server is started regardless of the individual endpoint toggles.
	Enabled bool `koanf:"enabled"`
	// Address is the host:port the management HTTP server listens on.
	Address string `koanf:"address"`

	// Metrics serves Prometheus metrics (Go runtime, process, gRPC, Aerospike).
	Metrics Endpoint `koanf:"metrics"`
	// Liveness answers "is the process alive" — cheap, no dependency checks.
	Liveness Endpoint `koanf:"liveness"`
	// Readiness answers "can the process serve traffic" — checks Aerospike
	// connectivity for every configured cluster.
	Readiness Endpoint `koanf:"readiness"`
	// Health is the aggregate of liveness and readiness checks.
	Health Endpoint `koanf:"health"`
	// Pprof serves net/http/pprof debug endpoints. Disabled by default because
	// it exposes process internals without authentication.
	Pprof Endpoint `koanf:"pprof"`
}

// Default returns the built-in management configuration used as the base of the
// resolution chain.
func Default() Management {
	return Management{
		Enabled:   true,
		Address:   DefaultAddress,
		Metrics:   Endpoint{Enabled: true, Path: "/metrics"},
		Liveness:  Endpoint{Enabled: true, Path: "/livez"},
		Readiness: Endpoint{Enabled: true, Path: "/readyz"},
		Health:    Endpoint{Enabled: true, Path: "/healthz"},
		Pprof:     Endpoint{Enabled: false, Path: "/debug/pprof/"},
	}
}

// Logical keys identify a single configurable field across all sources. They
// double as koanf paths, so nesting them with keyDelim maps straight onto the
// struct via the koanf tags above.
const (
	keyEnabled          = "enabled"
	keyAddress          = "address"
	keyMetricsEnabled   = "metrics.enabled"
	keyMetricsPath      = "metrics.path"
	keyLivenessEnabled  = "liveness.enabled"
	keyLivenessPath     = "liveness.path"
	keyReadinessEnabled = "readiness.enabled"
	keyReadinessPath    = "readiness.path"
	keyHealthEnabled    = "health.enabled"
	keyHealthPath       = "health.path"
	keyPprofEnabled     = "pprof.enabled"
	keyPprofPath        = "pprof.path"
)

// envKeys maps logical keys to their ASLD_* environment variable names.
var envKeys = map[string]string{
	keyEnabled:          "ASLD_MANAGEMENT_ENABLED",
	keyAddress:          "ASLD_MANAGEMENT_ADDRESS",
	keyMetricsEnabled:   "ASLD_METRICS_ENABLED",
	keyMetricsPath:      "ASLD_METRICS_PATH",
	keyLivenessEnabled:  "ASLD_LIVENESS_ENABLED",
	keyLivenessPath:     "ASLD_LIVENESS_PATH",
	keyReadinessEnabled: "ASLD_READINESS_ENABLED",
	keyReadinessPath:    "ASLD_READINESS_PATH",
	keyHealthEnabled:    "ASLD_HEALTH_ENABLED",
	keyHealthPath:       "ASLD_HEALTH_PATH",
	keyPprofEnabled:     "ASLD_PPROF_ENABLED",
	keyPprofPath:        "ASLD_PPROF_PATH",
}

// flagKeys maps logical keys to their CLI flag names.
var flagKeys = map[string]string{
	keyEnabled:          "management-enabled",
	keyAddress:          "management-address",
	keyMetricsEnabled:   "metrics-enabled",
	keyMetricsPath:      "metrics-path",
	keyLivenessEnabled:  "liveness-enabled",
	keyLivenessPath:     "liveness-path",
	keyReadinessEnabled: "readiness-enabled",
	keyReadinessPath:    "readiness-path",
	keyHealthEnabled:    "health-enabled",
	keyHealthPath:       "health-path",
	keyPprofEnabled:     "pprof-enabled",
	keyPprofPath:        "pprof-path",
}

// boolKeys is the set of logical keys whose value is a boolean. Used to choose
// the flag type during registration.
var boolKeys = map[string]bool{
	keyEnabled:          true,
	keyMetricsEnabled:   true,
	keyLivenessEnabled:  true,
	keyReadinessEnabled: true,
	keyHealthEnabled:    true,
	keyPprofEnabled:     true,
}

// Resolve overlays the file, environment and flag sources onto the built-in
// defaults and returns the validated configuration. The file source is the
// parsed [management] TOML table (nil if absent); env and flag sources are
// logical-key maps produced by FromEnv and the RegisterFlags collector. Each
// source overrides the previous one.
func Resolve(file map[string]any, env, flags map[string]string) (Management, error) {
	k := koanf.New(keyDelim)

	if len(file) > 0 {
		if err := k.Load(confmap.Provider(file, keyDelim), nil); err != nil {
			return Management{}, fmt.Errorf("load file config: %w", err)
		}
	}
	if err := k.Load(confmap.Provider(toAny(env), keyDelim), nil); err != nil {
		return Management{}, fmt.Errorf("load env config: %w", err)
	}
	if err := k.Load(confmap.Provider(toAny(flags), keyDelim), nil); err != nil {
		return Management{}, fmt.Errorf("load flag config: %w", err)
	}

	// Unmarshal onto the defaults so any field absent from every source keeps
	// its built-in value. WeaklyTypedInput lets the string-valued env and flag
	// sources decode into bool fields ("true" -> true).
	m := Default()
	if err := k.UnmarshalWithConf("", &m, koanf.UnmarshalConf{
		Tag: "koanf",
		DecoderConfig: &mapstructure.DecoderConfig{
			WeaklyTypedInput: true,
			Result:           &m,
		},
	}); err != nil {
		return Management{}, fmt.Errorf("decode config: %w", err)
	}

	if err := m.validate(); err != nil {
		return Management{}, err
	}
	return m, nil
}

// toAny widens a string-keyed map for confmap, which works with any-valued
// maps. The dotted keys are un-nested by koanf using keyDelim.
func toAny(in map[string]string) map[string]any {
	out := make(map[string]any, len(in))
	for k, v := range in {
		out[k] = v
	}
	return out
}

// validate rejects configurations that would fail at runtime, e.g. an empty
// listen address or two enabled endpoints sharing the same path (which would
// panic the HTTP mux on registration).
func (m Management) validate() error {
	if !m.Enabled {
		return nil
	}
	if strings.TrimSpace(m.Address) == "" {
		return fmt.Errorf("management address must not be empty when the management server is enabled")
	}

	endpoints := []struct {
		name string
		ep   Endpoint
	}{
		{"metrics", m.Metrics},
		{"liveness", m.Liveness},
		{"readiness", m.Readiness},
		{"health", m.Health},
		{"pprof", m.Pprof},
	}

	seen := make(map[string]string, len(endpoints))
	for _, e := range endpoints {
		if !e.ep.Enabled {
			continue
		}
		if !strings.HasPrefix(e.ep.Path, "/") {
			return fmt.Errorf("%s path %q must start with %q", e.name, e.ep.Path, "/")
		}
		if other, dup := seen[e.ep.Path]; dup {
			return fmt.Errorf("%s and %s endpoints share the same path %q", other, e.name, e.ep.Path)
		}
		seen[e.ep.Path] = e.name
	}
	return nil
}

// FromEnv extracts management settings from environment variables using the
// supplied lookup function (typically os.LookupEnv). Only variables that are
// actually set contribute to the result.
func FromEnv(lookup func(string) (string, bool)) map[string]string {
	out := make(map[string]string)
	for logical, env := range envKeys {
		if v, ok := lookup(env); ok {
			out[logical] = v
		}
	}
	return out
}

// RegisterFlags defines the management CLI flags on the given flag set and
// returns a collector that, once flags are parsed, yields only the flags that
// were explicitly set. Relying on explicit-set semantics (via FlagSet.Visit)
// is what lets a flag override env and file only when the user actually passes
// it, leaving lower-priority sources intact otherwise.
func RegisterFlags(fs *flag.FlagSet) func() map[string]string {
	def := Default()
	for logical, name := range flagKeys {
		if boolKeys[logical] {
			fs.Bool(name, defaultBool(def, logical), usage(logical))
		} else {
			fs.String(name, defaultString(def, logical), usage(logical))
		}
	}

	flagToLogical := make(map[string]string, len(flagKeys))
	for logical, name := range flagKeys {
		flagToLogical[name] = logical
	}

	return func() map[string]string {
		out := make(map[string]string)
		fs.Visit(func(f *flag.Flag) {
			if logical, ok := flagToLogical[f.Name]; ok {
				out[logical] = f.Value.String()
			}
		})
		return out
	}
}

func usage(logical string) string {
	return fmt.Sprintf("management setting %q (overrides %s and the config file)", logical, envKeys[logical])
}

func defaultBool(m Management, logical string) bool {
	switch logical {
	case keyEnabled:
		return m.Enabled
	case keyMetricsEnabled:
		return m.Metrics.Enabled
	case keyLivenessEnabled:
		return m.Liveness.Enabled
	case keyReadinessEnabled:
		return m.Readiness.Enabled
	case keyHealthEnabled:
		return m.Health.Enabled
	case keyPprofEnabled:
		return m.Pprof.Enabled
	default:
		return false
	}
}

func defaultString(m Management, logical string) string {
	switch logical {
	case keyAddress:
		return m.Address
	case keyMetricsPath:
		return m.Metrics.Path
	case keyLivenessPath:
		return m.Liveness.Path
	case keyReadinessPath:
		return m.Readiness.Path
	case keyHealthPath:
		return m.Health.Path
	case keyPprofPath:
		return m.Pprof.Path
	default:
		return ""
	}
}
