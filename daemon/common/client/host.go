package client

import (
	"fmt"
	"strings"
)

type HostTLSPort struct {
	Host    string // Can be ipv4, ipv6, or hostname.
	TLSName string
	Port    int
}

const DefaultPort = 3000
const DefaultIPv4 = "127.0.0.1"

func NewHostTLSPort() *HostTLSPort {
	return &HostTLSPort{}
}

func NewDefaultHostTLSPort() *HostTLSPort {
	return &HostTLSPort{
		DefaultIPv4,
		"",
		DefaultPort,
	}
}

func (addr *HostTLSPort) String() string {
	str := addr.Host

	if addr.TLSName != "" {
		str = fmt.Sprintf("%s:%s", str, addr.TLSName)
	}

	if addr.Port != 0 {
		str = fmt.Sprintf("%s:%v", str, addr.Port)
	}

	return str
}

type HostTLSPortSlice []*HostTLSPort

func (slice *HostTLSPortSlice) String() string {
	strs := []string{}

	for _, elem := range *slice {
		strs = append(strs, elem.String())
	}

	if len(strs) == 1 {
		return strs[0]
	}

	str := fmt.Sprintf("[%s]", strings.Join(strs, ", "))

	return str
}
