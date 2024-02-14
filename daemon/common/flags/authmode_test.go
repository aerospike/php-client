package flags

import (
	"strings"
	"testing"

	as "github.com/aerospike/aerospike-client-go/v7"
	"github.com/stretchr/testify/suite"
)

type AuthModeTestSuite struct {
	suite.Suite
}

func (suite *AuthModeTestSuite) TestAuthModeFlag() {
	testCases := []struct {
		input  string
		output AuthModeFlag
	}{
		{
			"INTERNAL",
			AuthModeFlag(as.AuthModeInternal),
		},
		{
			"EXTERNAL",
			AuthModeFlag(as.AuthModeExternal),
		},
		{
			"PKI",
			AuthModeFlag(as.AuthModePKI),
		},
		{
			"internal",
			AuthModeFlag(as.AuthModeInternal),
		},
		{
			"external",
			AuthModeFlag(as.AuthModeExternal),
		},
		{
			"pki",
			AuthModeFlag(as.AuthModePKI),
		},
	}

	for _, tc := range testCases {
		suite.T().Run(tc.input, func(t *testing.T) {
			var actual AuthModeFlag

			suite.NoError(actual.Set(tc.input))
			suite.Equal(actual, tc.output)
			suite.Equal(actual.String(), strings.ToUpper(tc.input))
			suite.Equal(actual.Type(), "INTERNAL,EXTERNAL,PKI")
		})
	}
}

// In order for 'go test' to run this suite, we need to create
// a normal test function and pass our suite to suite.Run
func TestRunAuthModeTestSuite(t *testing.T) {
	suite.Run(t, new(AuthModeTestSuite))
}
