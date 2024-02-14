package config

import (
	"fmt"
	"os"
	"path"
	"testing"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
	"github.com/stretchr/testify/suite"
)

const tomlConfigTxt = `
[group1]
str="localhost:3000"
int=3000
bool=true

[group2]
str2="localhost:4000"
int2=4000
bool2=false

[group1_instance]
str="localhost:3000 instance"
int=5000
bool=true
`

const yamlConfigTxt = `
group1:
  str: "localhost:3000"
  int:  3000
  bool:  true

group2:
  str2: "localhost:4000"
  int2: 4000
  bool2: false

group1_instance:
  str: "localhost:3000 instance"
  int: 5000
  bool: true
  `

type ConfigTestSuite struct {
	suite.Suite
	tmpDir        string
	file          string
	fileTxt       string
	actualCfgFile string
}

func (suite *ConfigTestSuite) SetupSuite() {
	wd, _ := os.Getwd()
	suite.tmpDir = path.Join(wd, "test-tmp")
	suite.file = path.Join(wd, "test-tmp", suite.file)

	err := os.MkdirAll(suite.tmpDir, 0o0777)
	if err != nil {
		suite.FailNow("Failed to create tmp dir", err)
	}

	err = os.WriteFile(suite.file, []byte(suite.fileTxt), 0o0600)
	if err != nil {
		suite.FailNow("Failed to write config file", err)
	}
}

func (suite *ConfigTestSuite) TearDownSuite() {
	os.RemoveAll(suite.tmpDir)
}

func (suite *ConfigTestSuite) SetupTest() {
	Reset()
}

func (suite *ConfigTestSuite) NewCmds(file, instance string) (rootCmd, cmd1, cmd2 *cobra.Command) {
	rootCmd = &cobra.Command{
		Use: "test",
		PersistentPreRunE: func(cmd *cobra.Command, args []string) error {
			cfgFileTmp, err := InitConfig(file, instance, cmd.Flags())
			if err != nil {
				return fmt.Errorf("Failed to initialize config: %s", err)
			}

			suite.actualCfgFile = cfgFileTmp

			return nil
		},
	}

	cmd1 = &cobra.Command{
		Use: "test1",
		Run: func(cmd *cobra.Command, args []string) {
		},
	}

	cmd2 = &cobra.Command{
		Use: "test2",
		Run: func(cmd *cobra.Command, args []string) {
		},
	}

	rootCmd.AddCommand(cmd1)
	rootCmd.AddCommand(cmd2)

	return rootCmd, cmd1, cmd2
}

// Tests the whether different flags on different cmcan read the same values
// from the config file.
func (suite *ConfigTestSuite) TestInitConfigWithDuplicateFlags() {
	rootCmd, cmd1, cmd2 := suite.NewCmds(suite.file, "")

	flagSet := &pflag.FlagSet{}
	flagSet.String("str", "", "string flag")
	flagSet.Int("int", 0, "int flag")
	flagSet.Bool("bool", false, "bool flag")
	BindPFlags(flagSet, "group1")

	flagSet2 := &pflag.FlagSet{}
	flagSet2.String("str", "", "string flag")
	flagSet2.Int("int", 0, "int flag")
	flagSet2.Bool("bool", false, "bool flag")
	BindPFlags(flagSet2, "group1")

	cmd1.Flags().AddFlagSet(flagSet)
	cmd2.Flags().AddFlagSet(flagSet2)

	// Cmd1
	rootCmd.SetArgs([]string{"test1"})
	err := rootCmd.Execute()

	suite.NoError(err)

	str, err := cmd1.Flags().GetString("str")
	suite.NoError(err)
	suite.Equal("localhost:3000", str)

	intVal, err := cmd1.Flags().GetInt("int")
	suite.NoError(err)
	suite.Equal(3000, intVal)

	boolVal, err := cmd1.Flags().GetBool("bool")
	suite.NoError(err)
	suite.Equal(true, boolVal)

	// Cmd2
	rootCmd.SetArgs([]string{"test2"})
	err = rootCmd.Execute()

	suite.NoError(err)

	str, err = cmd2.Flags().GetString("str")
	suite.NoError(err)
	suite.Equal("localhost:3000", str)

	intVal, err = cmd2.Flags().GetInt("int")
	suite.NoError(err)
	suite.Equal(3000, intVal)

	boolVal, err = cmd2.Flags().GetBool("bool")
	suite.NoError(err)
	suite.Equal(true, boolVal)

	suite.Equal(suite.actualCfgFile, suite.file)
}

func (suite *ConfigTestSuite) TestInitConfigWithMultiSections() {
	rootCmd, cmd1, _ := suite.NewCmds(suite.file, "")

	flagSet := &pflag.FlagSet{}
	flagSet.String("str", "", "string flag")
	flagSet.Int("int", 0, "int flag")
	flagSet.Bool("bool", false, "bool flag")
	BindPFlags(flagSet, "group1")

	flagSet2 := &pflag.FlagSet{}
	flagSet2.String("str2", "", "string flag")
	flagSet2.Int("int2", 0, "int flag")
	flagSet2.Bool("bool2", false, "bool flag")
	BindPFlags(flagSet2, "group2")

	cmd1.Flags().AddFlagSet(flagSet2)
	cmd1.Flags().AddFlagSet(flagSet)

	// Cmd1
	rootCmd.SetArgs([]string{"test1"})
	err := rootCmd.Execute()

	suite.NoError(err)

	str, err := cmd1.Flags().GetString("str")
	suite.NoError(err)
	suite.Equal("localhost:3000", str)

	intVal, err := cmd1.Flags().GetInt("int")
	suite.NoError(err)
	suite.Equal(3000, intVal)

	boolVal, err := cmd1.Flags().GetBool("bool")
	suite.NoError(err)
	suite.Equal(true, boolVal)

	str, err = cmd1.Flags().GetString("str2")
	suite.NoError(err)
	suite.Equal("localhost:4000", str)

	intVal, err = cmd1.Flags().GetInt("int2")
	suite.NoError(err)
	suite.Equal(4000, intVal)

	boolVal, err = cmd1.Flags().GetBool("bool2")
	suite.NoError(err)
	suite.Equal(false, boolVal)

	suite.Equal(suite.actualCfgFile, suite.file)
}

func (suite *ConfigTestSuite) TestInitConfigWithInstance() {
	rootCmd, cmd1, _ := suite.NewCmds(suite.file, "instance")

	flagSet := &pflag.FlagSet{}
	flagSet.String("str", "", "string flag")
	flagSet.Int("int", 0, "int flag")
	flagSet.Bool("bool", false, "bool flag")
	BindPFlags(flagSet, "group1")

	cmd1.Flags().AddFlagSet(flagSet)

	// Cmd1
	rootCmd.SetArgs([]string{"test1"})
	err := rootCmd.Execute()

	suite.NoError(err)

	str, err := cmd1.Flags().GetString("str")
	suite.NoError(err)
	suite.Equal("localhost:3000 instance", str)

	intVal, err := cmd1.Flags().GetInt("int")
	suite.NoError(err)
	suite.Equal(5000, intVal)

	boolVal, err := cmd1.Flags().GetBool("bool")
	suite.NoError(err)
	suite.Equal(true, boolVal)

	suite.Equal(suite.actualCfgFile, suite.file)
}

func (suite *ConfigTestSuite) TestInitConfigWithFlagsOverwrite() {
	rootCmd, cmd1, _ := suite.NewCmds(suite.file, "")

	flagSet := &pflag.FlagSet{}
	flagSet.String("str", "", "string flag")
	flagSet.Int("int", 0, "int flag")
	flagSet.Bool("bool", false, "bool flag")
	BindPFlags(flagSet, "group1")

	cmd1.Flags().AddFlagSet(flagSet)

	// Cmd1
	rootCmd.SetArgs([]string{"test1", "--str", "overwrite"})
	err := rootCmd.Execute()

	suite.NoError(err)

	str, err := cmd1.Flags().GetString("str")
	suite.NoError(err)
	suite.Equal("overwrite", str)

	intVal, err := cmd1.Flags().GetInt("int")
	suite.NoError(err)
	suite.Equal(3000, intVal)

	boolVal, err := cmd1.Flags().GetBool("bool")
	suite.NoError(err)
	suite.Equal(true, boolVal)
}

func (suite *ConfigTestSuite) TestInitConfigWithFlagsDefaults() {
	rootCmd, cmd1, _ := suite.NewCmds(suite.file, "DNE")

	flagSet := &pflag.FlagSet{}
	flagSet.String("str", "foo", "string flag")
	flagSet.Int("int", 99, "int flag")
	flagSet.Bool("bool", true, "bool flag")
	BindPFlags(flagSet, "group1")

	cmd1.Flags().AddFlagSet(flagSet)

	// Cmd1
	rootCmd.SetArgs([]string{"test1"})
	err := rootCmd.Execute()

	suite.NoError(err)

	str, err := cmd1.Flags().GetString("str")
	suite.NoError(err)
	suite.Equal("foo", str)

	intVal, err := cmd1.Flags().GetInt("int")
	suite.NoError(err)
	suite.Equal(99, intVal)

	boolVal, err := cmd1.Flags().GetBool("bool")
	suite.NoError(err)
	suite.Equal(true, boolVal)

	suite.Equal(suite.actualCfgFile, suite.file)
}

func TestRunConfigTestSuite(t *testing.T) {
	files := []struct {
		file    string
		fileTxt string
	}{
		{
			"basic.conf",
			tomlConfigTxt,
		},
		{
			"basic.yaml",
			yamlConfigTxt,
		},
	}

	for _, file := range files {
		cts := new(ConfigTestSuite)
		cts.file = file.file
		cts.fileTxt = file.fileTxt
		suite.Run(t, cts)
	}
}
