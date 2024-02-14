package flags

// Basic imports
import (
	"bytes"
	"crypto/tls"
	"crypto/x509"
	"encoding/pem"
	"os"
	"path"
	"strings"
	"testing"
	"text/template"

	as "github.com/aerospike/aerospike-client-go/v7"
	"github.com/aerospike/php-client/asld/common/config"
	"github.com/aerospike/php-client/asld/common/flags"
	"github.com/aerospike/php-client/asld/common/testutils"
	"github.com/spf13/cobra"
	"github.com/stretchr/testify/suite"
)

/*
This file tests the loading of the configuration file and command line arguments.
It tests the behavior of the viper module, cobra, and our own custom built flags.
**/

var confTomlFile = "conf_test.conf"
var confTomlTemplate = `
# -----------------------------------
# Aerospike tools configuration file.
# -----------------------------------
[cluster]
host = "1.1.1.1:3001,2.2.2.2:3002,3.3.3.3"
user = "default-user"
password = "default-password"
auth = "EXTERNAL"

[cluster_tls]
port = 4333
host = "3.3.3.3"
tls-name = "tls-name"
tls-enable = true
tls-capath = "{{.RootCAPath}}"
tls-cafile = "{{.RootCAFile}}"
tls-certfile = "{{.CertFile}}"
tls-keyfile = "{{.KeyFile}}"

[cluster_instance]
host = "3.3.3.3:3003,4.4.4.4:3004"
user = "test-user"
password = "test-password"

[cluster_env]
host = "5.5.5.5:env-tls-name:1000"
password = "env:AEROSPIKE_TEST"

[cluster_envb64]
host = "6.6.6.6:env-tls-name:1000"
password = "env-b64:AEROSPIKE_TEST"

[cluster_b64]
host = "7.7.7.7:env-tls-name:1000"
password = "b64:dGVzdC1wYXNzd29yZAo="

[cluster_file]
host = "1.1.1.1"
password = "file:{{.PassFile}}"

[uda]
agent-port = 8001
store-file = "default1.store"

[uda_instance]
store-file = "test.store"
`

var confYamlFile = "conf_test.yaml"
var confYamlTemplate = `
# -----------------------------------
# Aerospike tools configuration file.
# -----------------------------------
cluster:
  host: "1.1.1.1:3001,2.2.2.2:3002,3.3.3.3"
  user: "default-user"
  password: "default-password"
  auth: "EXTERNAL"

cluster_tls:
  port: 4333
  host: "3.3.3.3"
  tls-name: "tls-name"
  tls-enable: true
  tls-capath: "{{.RootCAPath}}"
  tls-cafile: "{{.RootCAFile}}"
  tls-certfile: "{{.CertFile}}"
  tls-keyfile: "{{.KeyFile}}"

cluster_instance:
  host: "3.3.3.3:3003,4.4.4.4:3004"
  user: "test-user"
  password: "test-password"

cluster_env:
  host: "5.5.5.5:env-tls-name:1000"
  password: "env:AEROSPIKE_TEST"

cluster_envb64:
  host: "6.6.6.6:env-tls-name:1000"
  password: "env-b64:AEROSPIKE_TEST"

cluster_b64:
  host: "7.7.7.7:env-tls-name:1000"
  password: "b64:dGVzdC1wYXNzd29yZAo="

cluster_file:
  host: "1.1.1.1"
  password: "file:{{.PassFile}}"

uda:
  agent-port: 8001
  store-file: "default1.store"

uda_instance:
  store-file: "test.store"

`

type ConfTestSuite struct {
	suite.Suite
	files  []string
	tmpDir string

	passFile    string
	passFileTxt string
	rootCAPath  string
	rootCAFile  string
	rootCATxt   string
	rootCAFile2 string
	rootCATxt2  string
	certFile    string
	certTxt     string
	keyFile     string
	keyTxt      string
	keyPassFile string
	keyPassTxt  string

	configFile         string
	configFileTemplate string
}

func (suite *ConfTestSuite) SetupSuite() {
	// Encode private key to PKCS#1 ASN.1 PEM.
	rootCertPEM, err := testutils.GenerateCert()
	if err != nil {
		suite.FailNow("Failed to generate cert: %w", err)
	}

	rootCertPEM2, err := testutils.GenerateCert()
	if err != nil {
		suite.FailNow("Failed to generate cert: %w", err)
	}

	certFileBytes, err := testutils.GenerateCert()
	if err != nil {
		suite.FailNow("Failed to generate cert: %w", err)
	}

	suite.passFile = "file_test.txt"
	suite.passFileTxt = "password-file\n"
	suite.rootCAPath = "root-ca-path/"
	suite.rootCAFile = "root-ca-path/root-ca.pem"
	suite.rootCATxt = string(rootCertPEM)
	suite.rootCAFile2 = "root-ca-path/root-ca2.pem"
	suite.rootCATxt2 = string(rootCertPEM2)
	suite.certFile = "cert.pem"
	suite.certTxt = string(certFileBytes)
	suite.keyFile = "key.pem"
	suite.keyTxt = string(testutils.KeyFileBytes)
	suite.keyPassFile = "key-pass.txt"
	suite.keyPassTxt = "key-pass"

	wd, err := os.Getwd()
	if err != nil {
		suite.FailNow("Failed to get working directory: %w", err)
	}

	suite.tmpDir = path.Join(wd, "test-tmp")

	err = os.MkdirAll(path.Join(suite.tmpDir, suite.rootCAPath), os.ModePerm)
	if err != nil {
		suite.FailNow("Failed to create directory: %w", err)
	}

	files := []struct {
		file *string
		txt  string
	}{
		{&suite.passFile, suite.passFileTxt},
		{&suite.rootCAFile, suite.rootCATxt},
		{&suite.rootCAFile2, suite.rootCATxt2},
		{&suite.certFile, suite.certTxt},
		{&suite.keyFile, suite.keyTxt},
		{&suite.keyPassFile, suite.keyPassTxt},
	}

	for _, file := range files {
		*file.file = path.Join(suite.tmpDir, *file.file)
		err := os.WriteFile(*file.file, []byte(file.txt), 0o0600)

		if err != nil {
			suite.FailNow("Failed to write file", err)
		}

		suite.files = append(suite.files, *file.file)
	}

	suite.rootCAPath = path.Join(suite.tmpDir, suite.rootCAPath)
	configFileTxt := bytes.Buffer{}
	t := template.Must(template.New("conf").Parse(suite.configFileTemplate))
	err = t.Execute(
		&configFileTxt,
		struct {
			RootCAPath  string
			RootCAFile  string
			CertFile    string
			KeyFile     string
			KeyPassFile string
			PassFile    string
		}{
			RootCAPath:  suite.rootCAPath,
			RootCAFile:  suite.rootCAFile,
			CertFile:    suite.certFile,
			KeyFile:     suite.keyFile,
			KeyPassFile: suite.keyPassFile,
			PassFile:    suite.passFile,
		},
	)

	suite.NoError(err)

	suite.configFile = path.Join(suite.tmpDir, suite.configFile)

	err = os.WriteFile(suite.configFile, configFileTxt.Bytes(), 0o0600)
	if err != nil {
		suite.FailNow("Failed to write file: %s", err)
	}

	suite.files = append(suite.files, []string{suite.certFile, suite.rootCAPath, suite.tmpDir}...)
}

func (suite *ConfTestSuite) TearDownSuite() {
	os.RemoveAll(suite.tmpDir)
}

func (suite *ConfTestSuite) NewTestCmd() (*cobra.Command, *flags.ConfFileFlags, *flags.AerospikeFlags) {
	configFileFlags := flags.NewConfFileFlags()
	aerospikeFlags := flags.NewDefaultAerospikeFlags()

	testCmd := &cobra.Command{
		Use:   "test",
		Short: "test cmd",
		Run: func(cmd *cobra.Command, args []string) {
			_, err := config.InitConfig(configFileFlags.File, configFileFlags.Instance, cmd.Flags())
			if err != nil {
				suite.FailNow("Failed to initialize config", err)
			}
		},
	}

	cfFlagSet := configFileFlags.NewFlagSet(flags.DefaultWrapHelpString)
	asFlagSet := aerospikeFlags.NewFlagSet(flags.DefaultWrapHelpString)

	testCmd.PersistentFlags().AddFlagSet(cfFlagSet)

	// This is what connects the flags to fields of the same name in the config file.
	config.BindPFlags(asFlagSet, "cluster")

	testCmd.PersistentFlags().AddFlagSet(asFlagSet)
	flags.SetupRoot(testCmd, "Test App")

	return testCmd, configFileFlags, aerospikeFlags
}

func (suite *ConfTestSuite) SetupTest() {
	config.Reset()
}

func (suite *ConfTestSuite) TestSetupRootVersion() {
	testCmd, _, _ := suite.NewTestCmd()
	testCmd.Version = "1.1.1"
	stdout := &bytes.Buffer{}

	testCmd.SetArgs([]string{"--version"})
	testCmd.SetOut(stdout)

	suite.NoError(testCmd.Execute())

	suite.Equal("Test App\nVersion 1.1.1\n", stdout.String())

	testCmd.SetArgs([]string{"-V"})

	stdout = &bytes.Buffer{}

	testCmd.SetOut(stdout)
	suite.NoError(testCmd.Execute())

	suite.Equal("Test App\nVersion 1.1.1\n", stdout.String())
}

func (suite *ConfTestSuite) TestSetupRootHelp() {
	stdout := &bytes.Buffer{}
	testCmd, _, _ := suite.NewTestCmd()

	testCmd.SetArgs([]string{"-u"})
	testCmd.SetErr(stdout)
	testCmd.SetOut(stdout)
	suite.NoError(testCmd.Execute())

	suite.Equal("test cmd", strings.Split(stdout.String(), "\n")[0])

	stdout = &bytes.Buffer{}

	testCmd.SetArgs([]string{"--help"})
	testCmd.SetErr(stdout)
	testCmd.SetOut(stdout)
	suite.NoError(testCmd.Execute())

	suite.Equal("test cmd", strings.Split(stdout.String(), "\n")[0])
}

func (suite *ConfTestSuite) TestConfigFileDefault() {
	testCmd, _, asFlags := suite.NewTestCmd()
	expectedClientConf := as.NewClientPolicy()
	expectedClientConf.User = "default-user"
	expectedClientConf.Password = "default-password"
	expectedClientConf.AuthMode = as.AuthModeExternal
	expectedClientHosts := []*as.Host{
		{Name: "1.1.1.1", Port: 3001},
		{Name: "2.2.2.2", Port: 3002},
		{Name: "3.3.3.3", Port: 3003},
	}

	output := bytes.Buffer{}
	testCmd.SetErr(&output)
	testCmd.SetArgs([]string{"test", "--config-file", suite.configFile, "-p", "3003"})
	suite.NoError(testCmd.Execute())

	if output.String() != "" {
		suite.Fail("Unexpected error: %s", output.String())
	}

	aerospikeConf := asFlags.NewAerospikeConfig()

	actualClientConf, err := aerospikeConf.NewClientPolicy()

	suite.NoError(err)
	suite.Equal(expectedClientConf, actualClientConf)

	actualClientHosts := aerospikeConf.NewHosts()
	suite.Equal(expectedClientHosts, actualClientHosts)
}

func (suite *ConfTestSuite) TestConfigFileTLS() {
	testCmd, _, asFlags := suite.NewTestCmd()
	expectedClientConf := as.NewClientPolicy()

	expectedServerPool, err := x509.SystemCertPool()
	if err != nil {
		suite.FailNow("Failed to get system cert pool: %s", err)
	}

	suite.True(expectedServerPool.AppendCertsFromPEM([]byte(suite.rootCATxt)))
	suite.True(expectedServerPool.AppendCertsFromPEM([]byte(suite.rootCATxt2)))

	block, _ := pem.Decode([]byte(suite.keyTxt))

	keyPem := pem.EncodeToMemory(block)
	cert, _ := tls.X509KeyPair([]byte(suite.certTxt), keyPem)
	expectedClientConf.TlsConfig = &tls.Config{
		MinVersion:               tls.VersionTLS12,
		MaxVersion:               tls.VersionTLS13,
		PreferServerCipherSuites: true,
		RootCAs:                  expectedServerPool,
		Certificates:             []tls.Certificate{cert},
	}

	expectedClientHosts := []*as.Host{
		{Name: "3.3.3.3", TLSName: "tls-name", Port: 4333},
	}

	testCmd.SetArgs([]string{"test", "--config-file", suite.configFile, "--instance", "tls"})
	suite.NoError(testCmd.Execute())

	aerospikeConf := asFlags.NewAerospikeConfig()

	actualClientConf, err := aerospikeConf.NewClientPolicy()

	suite.NoError(err)
	suite.Assert().True(expectedClientConf.TlsConfig.RootCAs.Equal(actualClientConf.TlsConfig.RootCAs))
	suite.True(len(expectedClientConf.TlsConfig.Certificates) == len(actualClientConf.TlsConfig.Certificates))
	suite.Assert().True(
		expectedClientConf.TlsConfig.Certificates[0].Leaf.Equal(actualClientConf.TlsConfig.Certificates[0].Leaf),
	)
	suite.False(expectedClientConf.TlsConfig.InsecureSkipVerify)
	suite.Equal(expectedClientConf.TlsConfig.MinVersion, actualClientConf.TlsConfig.MinVersion)
	suite.Equal(expectedClientConf.TlsConfig.MaxVersion, actualClientConf.TlsConfig.MaxVersion)

	actualClientHosts := aerospikeConf.NewHosts()
	suite.Equal(expectedClientHosts, actualClientHosts)
}

func (suite *ConfTestSuite) TestConfigFileWithInstance() {
	testCmd, _, asFlags := suite.NewTestCmd()
	expectedClientConf := as.NewClientPolicy()
	expectedClientConf.User = "test-user"
	expectedClientConf.Password = "test-password"
	expectedClientHosts := []*as.Host{
		{Name: "3.3.3.3", Port: 3003},
		{Name: "4.4.4.4", Port: 3004},
	}

	output := bytes.Buffer{}
	testCmd.SetErr(&output)
	testCmd.SetArgs([]string{"test", "--config-file", suite.configFile, "--instance", "instance"})
	suite.NoError(testCmd.Execute())

	if output.String() != "" {
		suite.Fail("Unexpected error: %s", output.String())
	}

	aerospikeConf := asFlags.NewAerospikeConfig()

	actualClientConf, err := aerospikeConf.NewClientPolicy()

	suite.NoError(err)
	suite.Equal(expectedClientConf, actualClientConf)

	actualClientHosts := aerospikeConf.NewHosts()
	suite.Equal(expectedClientHosts, actualClientHosts)
}

func (suite *ConfTestSuite) TestConfigFileWithEnv() {
	testCmd, _, asFlags := suite.NewTestCmd()
	expectedClientConf := as.NewClientPolicy()
	expectedClientConf.User = "env-user"
	expectedClientConf.Password = "test-password"
	expectedClientHosts := []*as.Host{
		{Name: "5.5.5.5", TLSName: "env-tls-name", Port: 1000},
	}

	os.Setenv("AEROSPIKE_TEST", "test-password")

	output := bytes.Buffer{}
	testCmd.SetErr(&output)
	testCmd.SetArgs([]string{"test", "--config-file", suite.configFile, "--instance", "env", "--user", "env-user"})
	suite.NoError(testCmd.Execute())

	if output.String() != "" {
		suite.Fail("Unexpected error: %s", output.String())
	}

	aerospikeConf := asFlags.NewAerospikeConfig()

	actualClientConf, err := aerospikeConf.NewClientPolicy()

	suite.NoError(err)
	suite.Equal(expectedClientConf, actualClientConf)

	actualClientHosts := aerospikeConf.NewHosts()
	suite.Equal(expectedClientHosts, actualClientHosts)
}

func (suite *ConfTestSuite) TestConfigFileWithEnvB64() {
	testCmd, _, asFlags := suite.NewTestCmd()
	expectedClientConf := as.NewClientPolicy()
	expectedClientConf.User = "env-user"
	expectedClientConf.Password = "test-password"
	expectedClientHosts := []*as.Host{
		{Name: "6.6.6.6", TLSName: "env-tls-name", Port: 1000},
	}

	output := bytes.Buffer{}
	testCmd.SetErr(&output)
	os.Setenv("AEROSPIKE_TEST", "dGVzdC1wYXNzd29yZAo=")
	testCmd.SetArgs([]string{"test", "--config-file", suite.configFile, "--instance", "envb64", "--user", "env-user"})
	suite.NoError(testCmd.Execute())

	if output.String() != "" {
		suite.Fail("Unexpected error: %s", output.String())
	}

	aerospikeConf := asFlags.NewAerospikeConfig()

	actualClientConf, err := aerospikeConf.NewClientPolicy()

	suite.NoError(err)
	suite.Equal(expectedClientConf, actualClientConf)

	actualClientHosts := aerospikeConf.NewHosts()
	suite.Equal(expectedClientHosts, actualClientHosts)
}

func (suite *ConfTestSuite) TestConfigFileWithB64() {
	testCmd, _, asFlags := suite.NewTestCmd()
	expectedClientConf := as.NewClientPolicy()
	expectedClientConf.User = "env-user"
	expectedClientConf.Password = "test-password"
	expectedClientHosts := []*as.Host{
		{
			Name:    "7.7.7.7",
			TLSName: "env-tls-name",
			Port:    1000,
		},
	}

	output := bytes.Buffer{}
	testCmd.SetErr(&output)
	testCmd.SetArgs([]string{"test", "--config-file", suite.configFile, "--instance", "b64", "--user", "env-user"})
	suite.NoError(testCmd.Execute())

	if output.String() != "" {
		suite.Fail("Unexpected error: %s", output.String())
	}

	aerospikeConf := asFlags.NewAerospikeConfig()

	actualClientConf, err := aerospikeConf.NewClientPolicy()

	suite.NoError(err)
	suite.Equal(expectedClientConf, actualClientConf)

	actualClientHosts := aerospikeConf.NewHosts()
	suite.Equal(expectedClientHosts, actualClientHosts)
}

func (suite *ConfTestSuite) TestConfigFileWithFile() {
	testCmd, _, asFlags := suite.NewTestCmd()
	expectedClientConf := as.NewClientPolicy()
	expectedClientConf.User = "user"
	expectedClientConf.Password = "password-file"
	expectedClientHosts := []*as.Host{
		{
			Name:    "1.1.1.1",
			TLSName: "",
			Port:    0,
		},
	}

	output := bytes.Buffer{}
	testCmd.SetErr(&output)
	testCmd.SetArgs([]string{"test", "--config-file", suite.configFile, "--instance", "file", "--user", "user", "-p", "0"})
	suite.NoError(testCmd.Execute())

	if output.String() != "" {
		suite.Fail("Unexpected error: %s", output.String())
	}

	aerospikeConf := asFlags.NewAerospikeConfig()

	actualClientConf, err := aerospikeConf.NewClientPolicy()

	suite.NoError(err)
	suite.Equal(expectedClientConf, actualClientConf)

	actualClientHosts := aerospikeConf.NewHosts()
	suite.Equal(expectedClientHosts, actualClientHosts)
}

// In order for 'go test' to run this suite, we need to create
// a normal test function and pass our suite to suite.Run
func TestRunConfTestSuite(t *testing.T) {
	configs := []struct {
		file     string
		template string
	}{
		{confTomlFile, confTomlTemplate},
		{confYamlFile, confYamlTemplate},
	}

	for _, config := range configs {
		cts := new(ConfTestSuite)
		cts.configFile = config.file
		cts.configFileTemplate = config.template
		suite.Run(t, cts)
	}
}
