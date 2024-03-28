package flags

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/aerospike/php-client/asld/common/client"
)

const (
	DefaultPort = 3000
	DefaultIPv4 = "127.0.0.1"
)

// A cobra PFlag to parse and display help info for the host[:tls-name][:port]
// input option.  It implements the pflag Value and SliceValue interfaces to
// enable automatic parsing by cobra.
type HostTLSPortSliceFlag struct {
	useDefault bool
	Seeds      client.HostTLSPortSlice
}

func NewHostTLSPortSliceFlag() HostTLSPortSliceFlag {
	return HostTLSPortSliceFlag{
		useDefault: true,
		Seeds: client.HostTLSPortSlice{
			client.NewDefaultHostTLSPort(),
		},
	}
}

func parseHostTLSPort(v string) (*client.HostTLSPort, error) {
	host := &client.HostTLSPort{}
	ipv6HostPattern := `^\[(?P<host>.*)\]`
	hostPattern := `^(?P<host>[^:]*)` // matched ipv4 and hostname
	tlsNamePattern := `(?P<tlsName>.*)`
	portPattern := `(?P<port>\d+)$`
	reIPv6Host := regexp.MustCompile(fmt.Sprintf("%s$", ipv6HostPattern))
	reIPv6HostPort := regexp.MustCompile(fmt.Sprintf("%s:%s", ipv6HostPattern, portPattern))
	reIPv6HostnamePort := regexp.MustCompile(fmt.Sprintf("%s:%s:%s", ipv6HostPattern, tlsNamePattern, portPattern))
	reIPv4Host := regexp.MustCompile(fmt.Sprintf("%s$", hostPattern))
	reIPv4HostPort := regexp.MustCompile(fmt.Sprintf("%s:%s", hostPattern, portPattern))
	reIPv4HostnamePort := regexp.MustCompile(fmt.Sprintf("%s:%s:%s", hostPattern, tlsNamePattern, portPattern))

	regexsAndNames := []struct {
		regex      *regexp.Regexp
		groupNames []string
	}{
		// The order is important since the ipv4 pattern also matches ipv6
		{reIPv6HostnamePort, reIPv6HostnamePort.SubexpNames()},
		{reIPv6HostPort, reIPv6HostPort.SubexpNames()},
		{reIPv6Host, reIPv6Host.SubexpNames()},
		{reIPv4HostnamePort, reIPv4HostnamePort.SubexpNames()},
		{reIPv4HostPort, reIPv4HostPort.SubexpNames()},
		{reIPv4Host, reIPv4Host.SubexpNames()},
	}

	for _, r := range regexsAndNames {
		regex := r.regex
		groupNames := r.groupNames

		if matchs := regex.FindStringSubmatch(v); matchs != nil {
			for idx, match := range matchs {
				var err error

				name := groupNames[idx]

				switch {
				case name == "host":
					host.Host = match
				case name == "tlsName":
					host.TLSName = match
				case name == "port":
					var intPort int64

					intPort, err = strconv.ParseInt(match, 0, 0)

					if err == nil {
						host.Port = int(intPort)
					}
				}

				if err != nil {
					return host, fmt.Errorf("failed to parse %s : %s", name, err)
				}
			}

			return host, nil
		}
	}

	return host, fmt.Errorf("does not match any expected formats")
}

// Append adds the specified value to the end of the flag value list.
func (slice *HostTLSPortSliceFlag) Append(val string) error {
	host, err := parseHostTLSPort(val)

	if err != nil {
		return err
	}

	slice.Seeds = append(slice.Seeds, host)

	return nil
}

// Replace will fully overwrite any data currently in the flag value list.
func (slice *HostTLSPortSliceFlag) Replace(vals []string) error {
	slice.Seeds = client.HostTLSPortSlice{}

	for _, val := range vals {
		if err := slice.Append(val); err != nil {
			return err
		}
	}

	return nil
}

// GetSlice returns the flag value list as an array of strings.
func (slice *HostTLSPortSliceFlag) GetSlice() []string {
	strs := []string{}

	for _, elem := range slice.Seeds {
		strs = append(strs, elem.String())
	}

	return strs
}

func (slice *HostTLSPortSliceFlag) Set(commaSepVal string) error {
	vals := strings.Split(commaSepVal, ",")

	if slice.useDefault {
		slice.useDefault = false
		return slice.Replace(vals)
	}

	for _, val := range vals {
		if err := slice.Append(val); err != nil {
			return err
		}
	}

	return nil
}

func (slice *HostTLSPortSliceFlag) Type() string {
	return "host[:tls-name][:port][,...]"
}

func (slice *HostTLSPortSliceFlag) String() string {
	if slice.useDefault {
		// displayed in help
		return DefaultIPv4
	}

	return slice.Seeds.String()
}
