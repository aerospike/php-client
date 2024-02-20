package flags

import (
	"fmt"
	"os"
	"strings"
)

type flagFormat uint8

const (
	flagFormatEnv    = flagFormat(1)
	flagFormatEnvB64 = flagFormat(1 << 1)
	flagFormatB64    = flagFormat(1 << 2)
	flagFormatFile   = flagFormat(1 << 3)
)

var (
	ErrEnvironmentVariableNotFound = fmt.Errorf("environment variable not found")
)

func fromEnv(v string) (string, error) {
	result := os.Getenv(v)
	if result == "" {
		return "", ErrEnvironmentVariableNotFound
	}

	return result, nil
}

func fromBase64(v string) (string, error) {
	return decode64(v)
}

func fromFile(v string) (string, error) {
	resultBytes, err := readFromFile(v, true)
	if err != nil {
		return "", err
	}

	return string(resultBytes), nil
}

func flagFormatParser(val string, mode flagFormat) (string, error) {
	split := strings.SplitN(val, ":", 2)

	if len(split) < 2 {
		return "", nil
	}

	sourceType := split[0]
	name := split[1]

	switch sourceType {
	case "env":
		if (mode & flagFormatEnv) != 0 {
			return fromEnv(name)
		}

		return "", fmt.Errorf("\"env:\" prefix not supported")
	case "env-b64":
		if (mode & flagFormatEnvB64) != 0 {
			b64Val, err := fromEnv(name)
			if err != nil {
				return "", err
			}

			return fromBase64(b64Val)
		}

		return "", fmt.Errorf("\"env-b64:\" prefix not supported")
	case "b64":
		if (mode & flagFormatB64) != 0 {
			return fromBase64(name)
		}

		return "", fmt.Errorf("\"b64:\" prefix not supported")
	case "file":
		if (mode & flagFormatFile) != 0 {
			return fromFile(name)
		}

		return "", fmt.Errorf("\"file:\" prefix not supported")
	}

	return "", nil
}
