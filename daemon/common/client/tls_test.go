package client

import (
	"crypto/tls"
	"testing"
)

func TestTLSProtocol_String(t *testing.T) {
	tests := []struct {
		protocol TLSProtocol
		expected string
	}{
		{TLSProtocol(tls.VersionTLS10), "TLSV1"},
		{TLSProtocol(tls.VersionTLS11), "TLSV1.1"},
		{TLSProtocol(tls.VersionTLS12), "TLSV1.2"},
		{TLSProtocol(tls.VersionTLS13), "TLSV1.3"},
	}

	for _, test := range tests {
		result := test.protocol.String()
		if result != test.expected {
			t.Errorf("Expected TLSProtocol.String() to return '%s', but got '%s'", test.expected, result)
		}
	}
}
