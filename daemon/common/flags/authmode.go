package flags

import (
	"fmt"
	"strings"

	as "github.com/aerospike/aerospike-client-go/v7"
)

// AuthModeFlag defines a Cobra compatible flag for the
// --auth flag.
type AuthModeFlag as.AuthMode

var authModeMap = map[string]as.AuthMode{
	"INTERNAL": as.AuthModeInternal,
	"EXTERNAL": as.AuthModeExternal,
	"PKI":      as.AuthModePKI,
}

func (mode *AuthModeFlag) Set(val string) error {
	val = strings.ToUpper(val)
	if val, ok := authModeMap[val]; ok {
		*mode = AuthModeFlag(val)
		return nil
	}

	return fmt.Errorf("unrecognized auth mode")
}

func (mode *AuthModeFlag) Type() string {
	return "INTERNAL,EXTERNAL,PKI"
}

func (mode *AuthModeFlag) String() string {
	for k, v := range authModeMap {
		if AuthModeFlag(v) == *mode {
			return k
		}
	}

	return ""
}
