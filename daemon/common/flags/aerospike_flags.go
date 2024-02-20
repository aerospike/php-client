package flags

import (
	"time"

	as "github.com/aerospike/aerospike-client-go/v7"
	"github.com/aerospike/php-client/asld/common/client"
)

// AerospikeFlags defines the storage backing
type AerospikeFlags struct {
	Socket         string               `toml:"socket"`
	Seeds          HostTLSPortSliceFlag `toml:"host"`
	DefaultPort    int                  `toml:"port"`
	User           string               `toml:"user"`
	Password       PasswordFlag         `toml:"password"`
	AuthMode       AuthModeFlag         `toml:"auth"`
	TLSEnable      bool                 `toml:"tls-enable"`
	TLSName        string               `toml:"tls-name"`
	TLSProtocols   TLSProtocolsFlag     `toml:"tls-protocols"`
	TLSRootCAFile  CertFlag             `toml:"tls-cafile"`
	TLSRootCAPath  CertPathFlag         `toml:"tls-capath"`
	TLSCertFile    CertFlag             `toml:"tls-certfile"`
	TLSKeyFile     CertFlag             `toml:"tls-keyfile"`
	TLSKeyFilePass PasswordFlag         `toml:"tls-keyfile-password"`

	ClusterName                 string        `toml:"cluster-name"`
	Timeout                     time.Duration `toml:"timeout"`
	IdleTimeout                 time.Duration `toml:"idle-timeout"`
	LoginTimeout                time.Duration `toml:"login-timeout"`
	ConnectionQueueSize         int           `toml:"connection-queue-size"`
	MinConnectionsPerNode       int           `toml:"min-connections-per-node"`
	MaxErrorRate                int           `toml:"max-error-rate"`
	ErrorRateWindow             int           `toml:"error-rate-window"`
	LimitConnectionsToQueueSize bool          `toml:"limit-connections-to-queue-size"`
	OpeningConnectionThreshold  int           `toml:"opening-connection-threshold"`
	FailIfNotConnected          bool          `toml:"fail-if-not-connected"`
	TendInterval                time.Duration `toml:"tend-interval"`
	UseServicesAlternate        bool          `toml:"use-services-alternate"`
	RackAware                   bool          `toml:"rack-aware"`
	RackIds                     []int         `toml:"rack-ids"`
	IgnoreOtherSubnetAliases    bool          `toml:"ignore-other-subnet-aliases"`
	SeedOnlyCluster             bool          `toml:"seed-only-cluster"`
}

func NewDefaultAerospikeFlags() *AerospikeFlags {
	return &AerospikeFlags{
		Seeds:        NewHostTLSPortSliceFlag(),
		DefaultPort:  DefaultPort,
		TLSProtocols: NewDefaultTLSProtocolsFlag(),
	}
}

func (af *AerospikeFlags) NewAerospikeConfig() *client.AerospikeConfig {
	aerospikeConf := client.NewDefaultAerospikeConfig()
	aerospikeConf.Socket = af.Socket
	aerospikeConf.Seeds = af.Seeds.Seeds
	aerospikeConf.User = af.User
	aerospikeConf.Password = string(af.Password)
	aerospikeConf.AuthMode = as.AuthMode(af.AuthMode)

	aerospikeConf.ClusterName = af.ClusterName
	aerospikeConf.Timeout = af.Timeout
	aerospikeConf.IdleTimeout = af.IdleTimeout
	aerospikeConf.LoginTimeout = af.LoginTimeout
	aerospikeConf.ConnectionQueueSize = af.ConnectionQueueSize
	aerospikeConf.MinConnectionsPerNode = af.MinConnectionsPerNode
	aerospikeConf.MaxErrorRate = af.MaxErrorRate
	aerospikeConf.ErrorRateWindow = af.ErrorRateWindow
	aerospikeConf.LimitConnectionsToQueueSize = af.LimitConnectionsToQueueSize
	aerospikeConf.OpeningConnectionThreshold = af.OpeningConnectionThreshold
	aerospikeConf.FailIfNotConnected = af.FailIfNotConnected
	aerospikeConf.TendInterval = af.TendInterval
	aerospikeConf.UseServicesAlternate = af.UseServicesAlternate
	aerospikeConf.RackAware = af.RackAware
	aerospikeConf.RackIds = af.RackIds
	aerospikeConf.IgnoreOtherSubnetAliases = af.IgnoreOtherSubnetAliases
	aerospikeConf.SeedOnlyCluster = af.SeedOnlyCluster

	if af.TLSEnable {
		aerospikeConf.Cert = af.TLSCertFile
		aerospikeConf.Key = af.TLSKeyFile
		aerospikeConf.KeyPass = af.TLSKeyFilePass
		aerospikeConf.TLSProtocolsMinVersion = af.TLSProtocols.min
		aerospikeConf.TLSProtocolsMaxVersion = af.TLSProtocols.max

		aerospikeConf.RootCA = [][]byte{}

		if len(af.TLSRootCAFile) != 0 {
			aerospikeConf.RootCA = append(aerospikeConf.RootCA, af.TLSRootCAFile)
		}

		aerospikeConf.RootCA = append(aerospikeConf.RootCA, af.TLSRootCAPath...)
	}

	for _, elem := range aerospikeConf.Seeds {
		if elem.Port == 0 {
			elem.Port = af.DefaultPort
		}

		if elem.TLSName == "" && af.TLSName != "" {
			elem.TLSName = af.TLSName
		}
	}

	return aerospikeConf
}
