package flags

import (
	"crypto/tls"
	"os"
	"testing"

	as "github.com/aerospike/aerospike-client-go/v7"
	"github.com/aerospike/php-client/asld/common/client"
	"github.com/stretchr/testify/suite"
)

var testTmp = "test-tmp"

var passFile = testTmp + "/file_test.txt"
var passFileTxt = "password-file\n"

var rootCAPath = testTmp + "/root-ca-path/"
var rootCAFile = testTmp + "/root-ca-path/root-ca.pem"
var rootCATxt = "root-ca-cert"
var rootCAFile2 = testTmp + "/root-ca-path/root-ca2.pem"
var rootCATxt2 = "root-ca-cert2"

var certFile = testTmp + "/cert.pem"
var certTxt = "cert"

var keyFile = testTmp + "/key.pem"
var keyTxt = "key"

type FlagsTestSuite struct {
	suite.Suite
}

func (suite *FlagsTestSuite) TestNewDefaultAerospikeFlags() {
	defaultAerospikeFlags := NewDefaultAerospikeFlags()

	suite.Equal(
		&AerospikeFlags{
			Seeds:        NewHostTLSPortSliceFlag(),
			DefaultPort:  3000,
			TLSProtocols: NewDefaultTLSProtocolsFlag(),
		},
		defaultAerospikeFlags,
	)
}

func (suite *FlagsTestSuite) TestNewAerospikeFlagSet() {
	files := []struct {
		file string
		txt  string
	}{
		{passFile, passFileTxt},
		{rootCAFile, rootCATxt},
		{rootCAFile2, rootCATxt2},
		{certFile, certTxt},
		{keyFile, keyTxt},
	}

	err := os.MkdirAll(rootCAPath, 0o0777)
	if err != nil {
		suite.FailNow("Failed to create root CA path", err)
	}

	for _, file := range files {
		err := os.WriteFile(file.file, []byte(file.txt), 0o0600)
		suite.NoError(err)
	}

	defer func() {
		os.RemoveAll(testTmp)
	}()

	actualFlags := NewDefaultAerospikeFlags()

	flagSet := actualFlags.NewFlagSet(func(str string) string { return str })

	expectedSeeds := NewHostTLSPortSliceFlag()

	err = expectedSeeds.Set("1.1.1.1:TLS-NAME:3002")
	suite.NoError(err)

	expectedAuthMode := AuthModeFlag(0)

	err = expectedAuthMode.Set("EXTERNAL")
	suite.NoError(err)

	expectedFlags := &AerospikeFlags{
		Seeds:       expectedSeeds,
		DefaultPort: 3001,
		User:        "admin",
		Password:    []byte("admin"),
		AuthMode:    expectedAuthMode,
		TLSEnable:   true,
		TLSName:     "tls-name",
		TLSProtocols: TLSProtocolsFlag{
			min: tls.VersionTLS13,
			max: tls.VersionTLS13,
		},
		TLSRootCAFile:  []byte(rootCATxt),
		TLSRootCAPath:  [][]byte{[]byte(rootCATxt), []byte(rootCATxt2)},
		TLSCertFile:    []byte(certTxt),
		TLSKeyFile:     []byte(keyTxt),
		TLSKeyFilePass: []byte("key-pass"),
	}

	err = flagSet.Parse([]string{
		"--host", "1.1.1.1:TLS-NAME:3002",
		"--port", "3001",
		"--user", "admin",
		"--password", "admin",
		"--auth", "EXTERNAL",
		"--tls-enable",
		"--tls-name", "tls-name",
		"--tls-protocols", "-all +TLSv1.3",
		"--tls-cafile", rootCAFile,
		"--tls-capath", rootCAPath,
		"--tls-certfile", certFile,
		"--tls-keyfile", keyFile,
		"--tls-keyfile-password", "key-pass"},
	)

	suite.NoError(err)
	suite.Equal(expectedFlags, actualFlags)
}

func (suite *FlagsTestSuite) TestNewAerospikeConfig() {
	testCases := []struct {
		input  *AerospikeFlags
		output *client.AerospikeConfig
	}{
		{
			&AerospikeFlags{
				Seeds: HostTLSPortSliceFlag{
					useDefault: false,
					Seeds: client.HostTLSPortSlice{
						{
							Host: "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
						},
					},
				},
				DefaultPort:    3001,
				User:           "admin",
				Password:       []byte("admin"),
				TLSEnable:      true,
				AuthMode:       AuthModeFlag(as.AuthModeExternal),
				TLSRootCAFile:  []byte("root-ca"),
				TLSRootCAPath:  [][]byte{[]byte("root-ca2"), []byte("root-ca3")},
				TLSCertFile:    []byte("cert"),
				TLSKeyFile:     []byte("key"),
				TLSKeyFilePass: []byte("key-pass"),
				TLSName:        "tls-name-1",
				TLSProtocols: TLSProtocolsFlag{
					min: tls.VersionTLS11,
					max: tls.VersionTLS13,
				},
			},
			&client.AerospikeConfig{
				Seeds: client.HostTLSPortSlice{
					{
						Host:    "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
						TLSName: "tls-name-1",
						Port:    3001,
					},
				},
				User:                   "admin",
				Password:               "admin",
				AuthMode:               as.AuthModeExternal,
				RootCA:                 [][]byte{[]byte("root-ca"), []byte("root-ca2"), []byte("root-ca3")},
				Cert:                   []byte("cert"),
				Key:                    []byte("key"),
				KeyPass:                []byte("key-pass"),
				TLSProtocolsMinVersion: tls.VersionTLS11,
				TLSProtocolsMaxVersion: tls.VersionTLS13,
			},
		},
		{
			&AerospikeFlags{
				Seeds: HostTLSPortSliceFlag{
					useDefault: false,
					Seeds: client.HostTLSPortSlice{
						{
							Host: "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
						},
					},
				},
				DefaultPort:    3001,
				User:           "admin",
				Password:       []byte("admin"),
				TLSEnable:      false,
				AuthMode:       AuthModeFlag(as.AuthModeExternal),
				TLSRootCAFile:  []byte("root-ca"),
				TLSCertFile:    []byte("cert"),
				TLSKeyFile:     []byte("key"),
				TLSKeyFilePass: []byte("key-pass"),
				TLSName:        "tls-name-1",
				TLSProtocols: TLSProtocolsFlag{
					min: tls.VersionTLS11,
					max: tls.VersionTLS13,
				},
			},
			&client.AerospikeConfig{
				Seeds: client.HostTLSPortSlice{
					{
						Host:    "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
						TLSName: "tls-name-1",
						Port:    3001,
					},
				},
				User:     "admin",
				Password: "admin",
				AuthMode: as.AuthModeExternal,
			},
		},
		{
			&AerospikeFlags{
				Seeds: HostTLSPortSliceFlag{
					useDefault: false,
					Seeds: client.HostTLSPortSlice{
						{
							Host: "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
							Port: 3002,
						},
					},
				},
				DefaultPort:    3000,
				User:           "admin",
				Password:       []byte("admin"),
				AuthMode:       AuthModeFlag(as.AuthModeExternal),
				TLSEnable:      true,
				TLSRootCAFile:  []byte("root-ca"),
				TLSCertFile:    []byte("cert"),
				TLSKeyFile:     []byte("key"),
				TLSKeyFilePass: []byte("key-pass"),
				TLSProtocols: TLSProtocolsFlag{
					min: tls.VersionTLS11,
					max: tls.VersionTLS13,
				},
			},
			&client.AerospikeConfig{
				Seeds: client.HostTLSPortSlice{
					{
						Host: "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
						Port: 3002,
					},
				},
				User:                   "admin",
				Password:               "admin",
				AuthMode:               as.AuthModeExternal,
				RootCA:                 [][]byte{[]byte("root-ca")},
				Cert:                   []byte("cert"),
				Key:                    []byte("key"),
				KeyPass:                []byte("key-pass"),
				TLSProtocolsMinVersion: tls.VersionTLS11,
				TLSProtocolsMaxVersion: tls.VersionTLS13,
			},
		},
		{
			&AerospikeFlags{
				Seeds: HostTLSPortSliceFlag{
					useDefault: false,
					Seeds: client.HostTLSPortSlice{
						{
							Host:    "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
							TLSName: "tls-name",
						},
					},
				},
				DefaultPort:    3000,
				User:           "admin",
				Password:       []byte("admin"),
				AuthMode:       AuthModeFlag(as.AuthModeExternal),
				TLSEnable:      true,
				TLSRootCAFile:  []byte("root-ca"),
				TLSCertFile:    []byte("cert"),
				TLSKeyFile:     []byte("key"),
				TLSKeyFilePass: []byte("key-pass"),
				TLSName:        "not-tls-name",
				TLSProtocols: TLSProtocolsFlag{
					min: tls.VersionTLS11,
					max: tls.VersionTLS13,
				},
			},
			&client.AerospikeConfig{
				Seeds: client.HostTLSPortSlice{
					{
						Host:    "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
						TLSName: "tls-name",
						Port:    3000,
					},
				},
				User:                   "admin",
				Password:               "admin",
				AuthMode:               as.AuthModeExternal,
				RootCA:                 [][]byte{[]byte("root-ca")},
				Cert:                   []byte("cert"),
				Key:                    []byte("key"),
				KeyPass:                []byte("key-pass"),
				TLSProtocolsMinVersion: tls.VersionTLS11,
				TLSProtocolsMaxVersion: tls.VersionTLS13,
			},
		},
	}

	for _, tc := range testCases {
		suite.T().Run("", func(t *testing.T) {
			actual := tc.input.NewAerospikeConfig()
			suite.Equal(tc.output, actual)
		})
	}
}

// In order for 'go test' to run this suite, we need to create
// a normal test function and pass our suite to suite.Run
func TestRunFlagsTestSuite(t *testing.T) {
	suite.Run(t, new(FlagsTestSuite))
}
