package client

import (
	"testing"
)

func TestNewDefaultHostTLSPort(t *testing.T) {
	expected := NewHostTLSPort()
	expected.Host = DefaultIPv4
	expected.Port = DefaultPort
	expected.TLSName = ""

	result := NewDefaultHostTLSPort()

	if result.Host != expected.Host {
		t.Errorf("Expected Host %s, but got %s", expected.Host, result.Host)
	}

	if result.TLSName != expected.TLSName {
		t.Errorf("Expected TLSName %s, but got %s", expected.TLSName, result.TLSName)
	}

	if result.Port != expected.Port {
		t.Errorf("Expected Port %d, but got %d", expected.Port, result.Port)
	}
}

func TestHostTLSPortString(t *testing.T) {
	host := "example.com"
	tlsName := "example"
	port := 8080

	addr := &HostTLSPort{
		Host:    host,
		TLSName: tlsName,
		Port:    port,
	}

	expected := "example.com:example:8080"
	result := addr.String()

	if result != expected {
		t.Errorf("Expected %s, but got %s", expected, result)
	}
}

func TestHostTLSPortSliceString(t *testing.T) {
	addr1 := &HostTLSPort{
		Host:    "example1.com",
		TLSName: "example1",
		Port:    8080,
	}

	addr2 := &HostTLSPort{
		Host:    "example2.com",
		TLSName: "example2",
		Port:    9090,
	}

	slice := HostTLSPortSlice{addr1, addr2}

	expected := "[example1.com:example1:8080, example2.com:example2:9090]"
	result := slice.String()

	if result != expected {
		t.Errorf("Expected %s, but got %s", expected, result)
	}
}
