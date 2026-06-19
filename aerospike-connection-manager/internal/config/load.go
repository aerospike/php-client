package config

import (
	"os"

	"github.com/pelletier/go-toml/v2"
)

// FileSection is the reserved TOML table name that holds management settings.
// common/config skips this section so it is not mistaken for an Aerospike
// cluster definition.
const FileSection = "management"

// readFileSection reads the [management] table from the TOML config file. A
// missing file or a missing section is not an error: it simply means no
// file-level overrides are present and the defaults (plus env and flags) apply.
func readFileSection(configFile string) (map[string]any, error) {
	doc, err := os.ReadFile(configFile)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}

	cfg := map[string]map[string]any{}
	if err := toml.Unmarshal(doc, &cfg); err != nil {
		return nil, err
	}
	return cfg[FileSection], nil
}

// Load resolves the management configuration from the config file, environment
// variables and CLI flags, in that ascending order of priority. collectFlags
// is the closure returned by RegisterFlags after flag.Parse has run; pass nil
// when no flags are wired (the file and environment are still applied).
func Load(configFile string, collectFlags func() map[string]string) (Management, error) {
	section, err := readFileSection(configFile)
	if err != nil {
		return Management{}, err
	}

	var flags map[string]string
	if collectFlags != nil {
		flags = collectFlags()
	}

	return Resolve(section, FromEnv(os.LookupEnv), flags)
}
