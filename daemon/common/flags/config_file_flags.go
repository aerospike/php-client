package flags

import (
	"fmt"

	"github.com/aerospike/php-client/asld/common/config"
	"github.com/spf13/pflag"
)

// ConfFileFlags represents the flags related to setting up the config file.
type ConfFileFlags struct {
	File     string // Config file path.
	Instance string // Instance name appended to top-level context e.g. instance=tls will read from cluster_tls.
}

func NewConfFileFlags() *ConfFileFlags {
	return &ConfFileFlags{}
}

// NewFlagSet creates a new pflag.FlagSet with the config file flags.
// The fmtUsage parameter is a function that formats the usage string.
func (cf *ConfFileFlags) NewFlagSet(fmtUsage UsageFormatter) *pflag.FlagSet {
	f := &pflag.FlagSet{}

	f.StringVar(&cf.File, "config-file", "", fmtUsage(fmt.Sprintf("Config file (default is %s/%s)", config.AsToolsConfDir, config.AsToolsConfName)))                                                                                    //nolint:lll //Reason: Wrapping this line would make editing difficult.
	f.StringVar(&cf.Instance, "instance", "", fmtUsage("For support of the aerospike tools toml schema. Sections with the instance are read. e.g in the case where instance 'a' is specified sections 'cluster_a', 'uda_a' are read.")) //nolint:lll //Reason: Wrapping this line would make editing difficult.

	return f
}
