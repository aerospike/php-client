package flags

import (
	"crypto/tls"
	"fmt"
	"strings"

	"github.com/aerospike/php-client/asld/common/client"
)

// TLSProtocolsFlag defines a Cobra compatible flag
// for dealing with tls protocols.
// Example flags include.
// --tls--protocols
type TLSProtocolsFlag struct {
	min client.TLSProtocol
	max client.TLSProtocol
}

func NewDefaultTLSProtocolsFlag() TLSProtocolsFlag {
	return TLSProtocolsFlag{
		min: client.VersionTLSDefaultMin,
		max: client.VersionTLSDefaultMax,
	}
}

func (flag *TLSProtocolsFlag) Set(val string) error {
	if val == "" {
		*flag = NewDefaultTLSProtocolsFlag()
		return nil
	}

	tlsV1 := uint8(1 << 0)
	tlsV1_1 := uint8(1 << 1)
	tlsV1_2 := uint8(1 << 2)
	tlsV1_3 := uint8(1 << 3)
	tlsAll := tlsV1 | tlsV1_1 | tlsV1_2 | tlsV1_3
	tokens := strings.Fields(val)
	protocols := uint8(0)
	protocolSlice := []client.TLSProtocol{
		tls.VersionTLS10,
		tls.VersionTLS11,
		tls.VersionTLS12,
		tls.VersionTLS13,
	}

	for _, tok := range tokens {
		var (
			sign    byte
			current uint8
		)

		if tok[0] == '+' || tok[0] == '-' {
			sign = tok[0]
			tok = tok[1:]
		}

		switch tok {
		case "SSLv2":
			return fmt.Errorf("SSLv2 not supported (RFC 6176)")
		case "SSLv3":
			return fmt.Errorf("SSLv3 not supported")
		case "TLSv1":
			current |= tlsV1
		case "TLSv1.1":
			current |= tlsV1_1
		case "TLSv1.2":
			current |= tlsV1_2
		case "TLSv1.3":
			current |= tlsV1_3
		case "all":
			current |= tlsAll
		default:
			return fmt.Errorf("unknown protocol version %s", tok)
		}

		switch sign {
		case '+':
			protocols |= current
		case '-':
			protocols &= ^current
		default:
			if protocols != 0 {
				return fmt.Errorf("TLS protocol %s overrides already set parameters. Check if a +/- prefix is missing", tok)
			}

			protocols = current
		}
	}

	if protocols == tlsAll {
		flag.min = tls.VersionTLS10
		flag.max = tls.VersionTLS13

		return nil
	}

	if (protocols&tlsV1) != 0 && (protocols&tlsV1_2) != 0 {
		// Since golangs tls.Config only support min and max we cannot specify 1 & 1.2 without 1.1
		return fmt.Errorf("you may only specify a range of protocols")
	}

	for i, p := range protocolSlice {
		if protocols&(1<<i) != 0 {
			flag.min = p
			break
		}
	}

	for i := 0; i < len(protocolSlice); i++ {
		p := protocolSlice[len(protocolSlice)-1-i]
		if protocols&((1<<(len(protocolSlice)-1))>>i) != 0 {
			flag.max = p
			break
		}
	}

	return nil
}

func (flag *TLSProtocolsFlag) Type() string {
	return "\"[[+][-]all] [[+][-]TLSv1] [[+][-]TLSv1.1] [[+][-]TLSv1.2] [[+][-]TLSv1.3]\""
}

func (flag *TLSProtocolsFlag) String() string {
	if flag.min == flag.max {
		return strings.Replace(flag.max.String(), "V", "v", 1)
	}

	if flag.min == tls.VersionTLS10 && flag.max == tls.VersionTLS13 {
		return "all"
	}

	protocolSlice := []client.TLSProtocol{
		tls.VersionTLS10,
		tls.VersionTLS11,
		tls.VersionTLS12,
		tls.VersionTLS13,
	}

	protocols := []string{}
	on := false

	for _, p := range protocolSlice {
		if p == flag.min {
			on = true
		}

		if p == flag.max {
			break
		}

		if on {
			protocols = append(protocols, "+"+strings.Replace(p.String(), "V", "v", 1))
		}
	}

	return strings.Join(protocols, " ")
}
