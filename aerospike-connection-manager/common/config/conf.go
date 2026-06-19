package config

import (
	"fmt"
	"os"
	"reflect"
	"sort"

	"github.com/go-viper/mapstructure/v2"
	"github.com/knadh/koanf/providers/confmap"
	"github.com/knadh/koanf/v2"
	"github.com/pelletier/go-toml/v2"

	"github.com/aerospike/php-client/asld/common/client"
	"github.com/aerospike/php-client/asld/common/flags"
)

const (
	// reservedManagementSection holds process-global operational settings. It is
	// parsed separately by internal/config and is never an Aerospike cluster.
	reservedManagementSection = "management"
	// clustersSection is the table whose sub-tables ([clusters.<name>]) are the
	// preferred way to declare Aerospike clusters.
	clustersSection = "clusters"
)

// Read parses the config file and returns the configured Aerospike clusters.
//
// Clusters may be declared under the [clusters.<name>] namespace (preferred) or
// as legacy top-level tables (any table that declares a host or socket). The
// names of the legacy tables are returned separately so the caller can emit a
// deprecation notice; that form is kept only for backward compatibility.
func Read(configFile string) (clusters map[string]*client.AerospikeConfig, legacyClusters []string, err error) {
	doc, err := os.ReadFile(configFile)
	if err != nil {
		return nil, nil, err
	}

	cfg := map[string]map[string]any{}
	if err := toml.Unmarshal(doc, &cfg); err != nil {
		return nil, nil, err
	}

	clusters = make(map[string]*client.AerospikeConfig, len(cfg))

	for section, valMap := range cfg {
		switch {
		case section == reservedManagementSection:
			continue
		case section == clustersSection:
			if err := decodeNamespace(valMap, clusters); err != nil {
				return nil, nil, err
			}
		case isClusterSection(valMap):
			if err := addCluster(clusters, section, valMap); err != nil {
				return nil, nil, err
			}

			legacyClusters = append(legacyClusters, section)
		}
	}

	sort.Strings(legacyClusters)

	return clusters, legacyClusters, nil
}

// decodeNamespace decodes every [clusters.<name>] sub-table into res.
func decodeNamespace(valMap map[string]any, res map[string]*client.AerospikeConfig) error {
	for name, raw := range valMap {
		sub, ok := raw.(map[string]any)
		if !ok {
			return fmt.Errorf("[clusters.%s] must be a table", name)
		}

		if err := addCluster(res, name, sub); err != nil {
			return err
		}
	}

	return nil
}

// addCluster decodes one cluster table and stores it under name, rejecting a
// name used by both the namespaced and the legacy form.
func addCluster(res map[string]*client.AerospikeConfig, name string, valMap map[string]any) error {
	if _, dup := res[name]; dup {
		return fmt.Errorf("duplicate cluster %q", name)
	}

	f := flags.NewDefaultAerospikeFlags()
	if err := decodeCluster(valMap, f); err != nil {
		return fmt.Errorf("cluster %q: %w", name, err)
	}

	res[name] = f.NewAerospikeConfig()

	return nil
}

// isClusterSection reports whether a TOML table defines an Aerospike cluster.
// A cluster must declare where to connect (host) or where to serve (socket);
// tables without either — such as the [uda] agent settings — are not clusters
// and are skipped rather than turned into bogus default-localhost entries.
func isClusterSection(valMap map[string]any) bool {
	if _, ok := valMap["host"]; ok {
		return true
	}

	_, ok := valMap["socket"]

	return ok
}

// decodeCluster maps one TOML cluster table onto the (already toml-tagged)
// AerospikeFlags. The flagValueHook bridges the custom flag types, whose string
// parsing (env/base64/file passwords, host[:tls][:port] seeds, certificate
// loading, auth and TLS protocols) cannot be expressed as plain struct tags.
func decodeCluster(valMap map[string]any, f *flags.AerospikeFlags) error {
	k := koanf.New(".")
	if err := k.Load(confmap.Provider(valMap, "."), nil); err != nil {
		return err
	}

	return k.UnmarshalWithConf("", f, koanf.UnmarshalConf{
		Tag: "toml",
		DecoderConfig: &mapstructure.DecoderConfig{
			DecodeHook: mapstructure.ComposeDecodeHookFunc(
				flagValueHook,
				mapstructure.StringToTimeDurationHookFunc(),
			),
			WeaklyTypedInput: true,
			Result:           f,
		},
	})
}

// flagValueHook decodes a string into any destination type whose pointer
// implements Set(string) error — i.e. the flag.Value types used by
// AerospikeFlags — by delegating to that parser. Other destinations are left
// untouched for the remaining hooks and mapstructure to handle.
func flagValueHook(from, to reflect.Type, data any) (any, error) {
	if from.Kind() != reflect.String {
		return data, nil
	}

	ptr := reflect.New(to)

	setter, ok := ptr.Interface().(interface{ Set(string) error })
	if !ok {
		return data, nil
	}

	if err := setter.Set(data.(string)); err != nil {
		return nil, err
	}

	return ptr.Elem().Interface(), nil
}
