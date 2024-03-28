package config

import (
	"os"
	"time"

	"github.com/aerospike/php-client/asld/common/client"
	"github.com/aerospike/php-client/asld/common/flags"
	"github.com/pelletier/go-toml/v2"
)

func Read(configFile string) (map[string]*client.AerospikeConfig, error) {
	doc, err := os.ReadFile(configFile)
	if err != nil {
		return nil, err
	}

	cfg := map[string]map[string]any{}
	if err := toml.Unmarshal(doc, &cfg); err != nil {
		return nil, err
	}

	res := make(map[string]*client.AerospikeConfig, len(cfg))

	for section, valMap := range cfg {
		f := flags.NewDefaultAerospikeFlags()

		if v, exists := valMap["socket"]; exists {
			f.Socket = v.(string)
		}

		if v, exists := valMap["host"]; exists {
			seeds := flags.NewHostTLSPortSliceFlag()
			if err := seeds.Set(v.(string)); err != nil {
				return nil, err
			} else {
				f.Seeds = seeds
			}
		}

		if v, exists := valMap["port"]; exists {
			f.DefaultPort = int(v.(int64))
		}

		if v, exists := valMap["user"]; exists {
			f.User = v.(string)
		}

		if v, exists := valMap["password"]; exists {
			var pass flags.PasswordFlag
			if err := pass.Set(v.(string)); err != nil {
				return nil, err
			} else {
				f.Password = pass
			}
		}

		if v, exists := valMap["auth"]; exists {
			var auth flags.AuthModeFlag
			if err := auth.Set(v.(string)); err != nil {
				return nil, err
			} else {
				f.AuthMode = auth
			}
		}

		if v, exists := valMap["tls-enable"]; exists {
			f.TLSEnable = v.(bool)
		}

		if v, exists := valMap["tls-name"]; exists {
			f.TLSName = v.(string)
		}

		if v, exists := valMap["tls-protocols"]; exists {
			var cv flags.TLSProtocolsFlag
			if err := cv.Set(v.(string)); err != nil {
				return nil, err
			} else {
				f.TLSProtocols = cv
			}
		}

		if v, exists := valMap["tls-cafile"]; exists {
			var cv flags.CertFlag
			if err := cv.Set(v.(string)); err != nil {
				return nil, err
			} else {
				f.TLSRootCAFile = cv
			}
		}

		if v, exists := valMap["tls-capath"]; exists {
			var cv flags.CertPathFlag
			if err := cv.Set(v.(string)); err != nil {
				return nil, err
			} else {
				f.TLSRootCAPath = cv
			}
		}

		if v, exists := valMap["tls-certfile"]; exists {
			var cv flags.CertFlag
			if err := cv.Set(v.(string)); err != nil {
				return nil, err
			} else {
				f.TLSCertFile = cv
			}
		}

		if v, exists := valMap["tls-keyfile"]; exists {
			var cv flags.CertFlag
			if err := cv.Set(v.(string)); err != nil {
				return nil, err
			} else {
				f.TLSKeyFile = cv
			}
		}

		if v, exists := valMap["tls-keyfile-password"]; exists {
			var cv flags.PasswordFlag
			if err := cv.Set(v.(string)); err != nil {
				return nil, err
			} else {
				f.TLSKeyFilePass = cv
			}
		}

		if v, exists := valMap["cluster-name"]; exists {
			f.ClusterName = v.(string)
		}

		if v, exists := valMap["timeout"]; exists {
			v, err := time.ParseDuration(v.(string))
			if err != nil {
				return nil, err
			}
			f.Timeout = v
		}

		if v, exists := valMap["idle-timeout"]; exists {
			v, err := time.ParseDuration(v.(string))
			if err != nil {
				return nil, err
			}
			f.IdleTimeout = v
		}

		if v, exists := valMap["login-timeout"]; exists {
			v, err := time.ParseDuration(v.(string))
			if err != nil {
				return nil, err
			}
			f.LoginTimeout = v
		}

		if v, exists := valMap["connection-queue-size"]; exists {
			f.ConnectionQueueSize = int(v.(int64))
		}

		if v, exists := valMap["min-connections-per-node"]; exists {
			f.MinConnectionsPerNode = int(v.(int64))
		}

		if v, exists := valMap["max-error-rate"]; exists {
			f.MaxErrorRate = int(v.(int64))
		}

		if v, exists := valMap["error-rate-window"]; exists {
			f.ErrorRateWindow = int(v.(int64))
		}

		if v, exists := valMap["limit-connections-to-queue-size"]; exists {
			f.LimitConnectionsToQueueSize = v.(bool)
		}

		if v, exists := valMap["opening-connection-threshold"]; exists {
			f.OpeningConnectionThreshold = int(v.(int64))
		}

		if v, exists := valMap["fail-if-not-connected"]; exists {
			f.FailIfNotConnected = v.(bool)
		}

		if v, exists := valMap["tend-interval"]; exists {
			v, err := time.ParseDuration(v.(string))
			if err != nil {
				return nil, err
			}
			f.TendInterval = v
		}

		if v, exists := valMap["use-services-alternate"]; exists {
			f.UseServicesAlternate = v.(bool)
		}

		if v, exists := valMap["rack-aware"]; exists {
			f.RackAware = v.(bool)
		}

		if v, exists := valMap["rack-ids"]; exists {
			v := v.([]any)
			res := make([]int, len(v))
			for i := range v {
				res[i] = int(v[i].(int64))
			}
			f.RackIds = res
		}

		if v, exists := valMap["ignore-other-subnet-aliases"]; exists {
			f.IgnoreOtherSubnetAliases = v.(bool)
		}

		if v, exists := valMap["seed-only-cluster"]; exists {
			f.SeedOnlyCluster = v.(bool)
		}

		res[section] = f.NewAerospikeConfig()
	}

	return res, nil
}
