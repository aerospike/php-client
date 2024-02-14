package flags

import (
	as "github.com/aerospike/aerospike-client-go/v7"
	"github.com/aerospike/php-client/asld/common/client"
	"github.com/spf13/pflag"
)

// AerospikeFlags defines the storage backing
// for Aerospike pflags.FlagSet returned from SetAerospikeFlags.
type AerospikeFlags struct {
	Seeds          HostTLSPortSliceFlag `mapstructure:"host"`
	DefaultPort    int                  `mapstructure:"port"`
	User           string               `mapstructure:"user"`
	Password       PasswordFlag         `mapstructure:"password"`
	AuthMode       AuthModeFlag         `mapstructure:"auth"`
	TLSEnable      bool                 `mapstructure:"tls-enable"`
	TLSName        string               `mapstructure:"tls-name"`
	TLSProtocols   TLSProtocolsFlag     `mapstructure:"tls-protocols"`
	TLSRootCAFile  CertFlag             `mapstructure:"tls-cafile"`
	TLSRootCAPath  CertPathFlag         `mapstructure:"tls-capath"`
	TLSCertFile    CertFlag             `mapstructure:"tls-certfile"`
	TLSKeyFile     CertFlag             `mapstructure:"tls-keyfile"`
	TLSKeyFilePass PasswordFlag         `mapstructure:"tls-keyfile-password"`
}

func NewDefaultAerospikeFlags() *AerospikeFlags {
	return &AerospikeFlags{
		Seeds:        NewHostTLSPortSliceFlag(),
		DefaultPort:  DefaultPort,
		TLSProtocols: NewDefaultTLSProtocolsFlag(),
	}
}

// NewAerospikeFlagSet returns a new pflag.FlagSet with Aerospike flags defined.
// Values set in the returned FlagSet will be stored in the AerospikeFlags argument.
func (af *AerospikeFlags) NewFlagSet(fmtUsage UsageFormatter) *pflag.FlagSet {
	f := &pflag.FlagSet{}
	f.VarP(&af.Seeds, "host", "H", fmtUsage("The Aerospike host."))
	f.IntVarP(&af.DefaultPort, "port", "P", 3000, fmtUsage("The default Aerospike port."))
	f.StringVar(&af.User, "user", "", fmtUsage("The Aerospike user to use to connect to the Aerospike cluster."))
	f.Var(&af.Password, "password", fmtUsage("The Aerospike password to use to connect to the Aerospike cluster."))
	f.Var(&af.AuthMode, "auth", fmtUsage("The authentication mode used by the Aerospike server."+
		" INTERNAL uses standard user/pass. EXTERNAL uses external methods (like LDAP)"+
		" which are configured on the server. EXTERNAL requires TLS. PKI allows TLS"+
		" authentication and authorization based on a certificate. No user name needs to be configured."))
	f.BoolVar(&af.TLSEnable, "tls-enable", false, fmtUsage("Enable TLS authentication with Aerospike."+
		" If false, other tls options are ignored.",
	))
	f.StringVar(&af.TLSName, "tls-name", "", fmtUsage("The server TLS context to use to"+
		" authenticate the connection to Aerospike.",
	))
	f.Var(&af.TLSRootCAFile, "tls-cafile", fmtUsage("The CA used when connecting to Aerospike."))
	f.Var(&af.TLSRootCAPath, "tls-capath", fmtUsage("A path containing CAs for connecting to Aerospike."))
	f.Var(&af.TLSCertFile, "tls-certfile", fmtUsage("The certificate file for mutual TLS authentication with Aerospike."))
	f.Var(&af.TLSKeyFile, "tls-keyfile", fmtUsage("The key file used for mutual TLS authentication with Aerospike."))
	f.Var(&af.TLSKeyFilePass, "tls-keyfile-password", fmtUsage("The password used to decrypt the key-file if encrypted."))
	f.Var(&af.TLSProtocols, "tls-protocols", fmtUsage(
		"Set the TLS protocol selection criteria. This format is the same as"+
			" Apache's SSLProtocol documented at https://httpd.apache.org/docs/current/mod/mod_ssl.html#ssl protocol.",
	))

	return f
}

func (af *AerospikeFlags) NewAerospikeConfig() *client.AerospikeConfig {
	aerospikeConf := client.NewDefaultAerospikeConfig()
	aerospikeConf.Seeds = af.Seeds.Seeds
	aerospikeConf.User = af.User
	aerospikeConf.Password = string(af.Password)
	aerospikeConf.AuthMode = as.AuthMode(af.AuthMode)

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
