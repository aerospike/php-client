package flags

import (
	"testing"

	"github.com/spf13/cobra"
	"github.com/stretchr/testify/assert"
)

func TestSetupRoot(t *testing.T) {
	rootCmd := &cobra.Command{}
	appLongName := "Some Random Tool"

	SetupRoot(rootCmd, appLongName)

	// Verify that the "help" flag is registered
	helpFlag, err := rootCmd.PersistentFlags().GetBool("help")
	assert.NoError(t, err)
	assert.False(t, helpFlag)

	// Verify the version template is set correctly
	expectedVersionTemplate := "Some Random Tool\n{{printf \"Version %s\" .Version}}\n"
	assert.Equal(t, expectedVersionTemplate, rootCmd.VersionTemplate())

	// Verify that the "version" flag is registered
	versionFlag, err := rootCmd.PersistentFlags().GetBool("version")
	assert.NoError(t, err)
	assert.False(t, versionFlag)
}
