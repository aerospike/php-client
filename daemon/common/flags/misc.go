package flags

import (
	"fmt"

	"github.com/spf13/cobra"
)

// SetupRoot sets up the root command with common flags and options.
// It takes the rootCmd and appLongName as parameters.
// It registers the "help" alias for the "usage" flag.
// It adds the "version" as uppercase "V" flag to the rootCmd.
// It sets the version template for the rootCmd using appLongName. If
// appLongName is "Unique Data Agent", the version template will be:
//
// Unique Data Agent
// Version 1.2.3
func SetupRoot(rootCmd *cobra.Command, appLongName string) {
	rootCmd.PersistentFlags().BoolP("help", "h", false, "Display help information")
	rootCmd.SetVersionTemplate(fmt.Sprintf("%s\n{{printf \"Version %%s\" .Version}}\n", appLongName))
	rootCmd.PersistentFlags().BoolP("version", "v", false, "Display version.") // All tools use -V
}
