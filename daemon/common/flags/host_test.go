package flags

import (
	"testing"

	"github.com/aerospike/php-client/asld/common/client"
	"github.com/stretchr/testify/suite"
)

type HostTestSuite struct {
	suite.Suite
}

func (suite *HostTestSuite) TestHostTLSPortSetGet() {
	testCases := []struct {
		input  string
		output HostTLSPortSliceFlag
		slice  []string
	}{
		{
			"127.0.0.1",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host: "127.0.0.1",
					},
				},
			},
			[]string{"127.0.0.1"},
		},
		{
			"127.0.0.1,127.0.0.2",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host: "127.0.0.1",
					},
					{
						Host: "127.0.0.2",
					},
				},
			},
			[]string{"127.0.0.1", "127.0.0.2"},
		},
		{
			"127.0.0.2:3002",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host: "127.0.0.2",
						Port: 3002,
					},
				},
			},
			[]string{"127.0.0.2:3002"},
		},
		{
			"127.0.0.2:3002,127.0.0.3:3003",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host: "127.0.0.2",
						Port: 3002,
					},
					{
						Host: "127.0.0.3",
						Port: 3003,
					},
				},
			},
			[]string{"127.0.0.2:3002", "127.0.0.3:3003"},
		},
		{
			"127.0.0.3:tls-name:3003",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host:    "127.0.0.3",
						TLSName: "tls-name",
						Port:    3003,
					},
				},
			},
			[]string{"127.0.0.3:tls-name:3003"},
		},
		{
			"127.0.0.3:tls-name:3003,127.0.0.4:tls-name4:3004",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host:    "127.0.0.3",
						TLSName: "tls-name",
						Port:    3003,
					},
					{
						Host:    "127.0.0.4",
						TLSName: "tls-name4",
						Port:    3004,
					},
				},
			},
			[]string{"127.0.0.3:tls-name:3003", "127.0.0.4:tls-name4:3004"},
		},
		{
			"127.0.0.3:3003,127.0.0.4:tls-name4:3004",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host: "127.0.0.3",
						Port: 3003,
					},
					{
						Host:    "127.0.0.4",
						TLSName: "tls-name4",
						Port:    3004,
					},
				},
			},
			[]string{"127.0.0.3:3003", "127.0.0.4:tls-name4:3004"},
		},
		{
			"[2001:0db8:85a3:0000:0000:8a2e:0370:7334]",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host: "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
					},
				},
			},
			[]string{"2001:0db8:85a3:0000:0000:8a2e:0370:7334"},
		},
		{
			"[fe80::1ff:fe23:4567:890a]:3002",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host: "fe80::1ff:fe23:4567:890a",
						Port: 3002,
					},
				},
			},
			[]string{"fe80::1ff:fe23:4567:890a:3002"},
		},
		{
			"[100::]:tls-name:3003",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host:    "100::",
						TLSName: "tls-name",
						Port:    3003,
					},
				},
			},
			[]string{"100:::tls-name:3003"},
		},
	}

	for _, tc := range testCases {
		suite.T().Run(tc.input, func(t *testing.T) {
			actual := NewHostTLSPortSliceFlag()

			suite.NoError(actual.Set(tc.input))
			suite.Equal(tc.output, actual)
			suite.Equal(tc.slice, actual.GetSlice())
		})
	}
}

func (suite *HostTestSuite) TestHostTLSPortAppend() {
	testCases := []struct {
		input  string
		append string
		output HostTLSPortSliceFlag
	}{
		{
			"127.0.0.1",
			"127.0.0.2",
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host: "127.0.0.1",
					},
					{
						Host: "127.0.0.2",
					},
				},
			},
		},
	}

	for _, tc := range testCases {
		suite.T().Run(tc.input, func(t *testing.T) {
			actual := NewHostTLSPortSliceFlag()

			suite.NoError(actual.Set(tc.input))
			suite.NoError(actual.Set(tc.append))
			suite.Equal(tc.output, actual)
		})
	}
}

func (suite *HostTestSuite) TestHostTLSPortString() {
	testCases := []struct {
		input  HostTLSPortSliceFlag
		output string
	}{
		{
			HostTLSPortSliceFlag{
				useDefault: false,
				Seeds: client.HostTLSPortSlice{
					{
						Host: "127.0.0.1",
						Port: 3000,
					},
					{
						Host:    "127.0.0.2",
						TLSName: "tls-name",
						Port:    3002,
					},
				},
			},
			"[127.0.0.1:3000, 127.0.0.2:tls-name:3002]",
		},
	}

	for _, tc := range testCases {
		suite.T().Run(tc.output, func(t *testing.T) {
			suite.Equal(tc.output, tc.input.String())
		})
	}
}

// In order for 'go test' to run this suite, we need to create
// a normal test function and pass our suite to suite.Run
func TestRunHostTestSuite(t *testing.T) {
	suite.Run(t, new(HostTestSuite))
}
