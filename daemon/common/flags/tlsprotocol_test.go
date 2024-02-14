package flags

import (
	"crypto/tls"
	"testing"

	"github.com/aerospike/php-client/asld/common/client"
	"github.com/stretchr/testify/suite"
)

type TLSModeTestSuite struct {
	suite.Suite
}

func (suite *FlagsTestSuite) TestTLSProtocolsFlag() {
	testCases := []struct {
		input  string
		output TLSProtocolsFlag
		err    bool
	}{
		{
			"",
			TLSProtocolsFlag{
				min: client.VersionTLSDefaultMin,
				max: client.VersionTLSDefaultMax,
			},
			false,
		},
		{
			"all",
			TLSProtocolsFlag{
				min: tls.VersionTLS10,
				max: tls.VersionTLS13,
			},
			false,
		},
		{
			"all -TLSv1",
			TLSProtocolsFlag{
				min: tls.VersionTLS11,
				max: tls.VersionTLS13,
			},
			false,
		},
		{
			"all -TLSv1.2",
			TLSProtocolsFlag{
				min: tls.VersionTLS10,
				max: tls.VersionTLS13,
			},
			false,
		},
		{
			"+TLSv1",
			TLSProtocolsFlag{
				min: tls.VersionTLS10,
				max: tls.VersionTLS10,
			},
			false,
		},
		{
			"+TLSv1.1",
			TLSProtocolsFlag{
				min: tls.VersionTLS11,
				max: tls.VersionTLS11,
			},
			false,
		},
		{
			"+TLSv1.2",
			TLSProtocolsFlag{
				min: tls.VersionTLS12,
				max: tls.VersionTLS12,
			},
			false,
		},
		{
			"+TLSv1.3",
			TLSProtocolsFlag{
				min: tls.VersionTLS13,
				max: tls.VersionTLS13,
			},
			false,
		},
		{
			"all -TLSv1.1",
			TLSProtocolsFlag{
				min: tls.VersionTLS12,
				max: tls.VersionTLS13,
			},
			true,
		},
	}

	for _, tc := range testCases {
		suite.T().Run(tc.input, func(t *testing.T) {
			var actual TLSProtocolsFlag
			err := actual.Set(tc.input)
			if tc.err {
				suite.Error(err)
			} else {
				suite.NoError(err)
				suite.Equal(tc.output, actual)
			}
		})
	}
}

// In order for 'go test' to run this suite, we need to create
// a normal test function and pass our suite to suite.Run
func TestRunTLSModeTestSuite(t *testing.T) {
	suite.Run(t, new(AuthModeTestSuite))
}
