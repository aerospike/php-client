package flags

import (
	"bytes"
	"encoding/base64"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
)

const (
	DefaultMaxLineLength = 65
)

// UsageFormatter provides a method for modifying the usage text of the
// flags returned by SetAerospikeFlags.
type UsageFormatter func(string) string

func WrapString(val string, lineLen int) string {
	tokens := strings.Split(val, " ")
	currentLen := 0

	for i, tok := range tokens {
		if currentLen+len(tok) > lineLen {
			if i != 0 {
				tok = "\n" + tok
			}

			tokens[i] = tok
			currentLen = 0

			continue
		}

		currentLen += len(tok) + 1 // '\n'
	}

	return strings.Join(tokens, " ")
}

func DefaultWrapHelpString(val string) string {
	return WrapString(val, DefaultMaxLineLength)
}

func decode64(b64Val string) (string, error) {
	byteVal, err := base64.StdEncoding.DecodeString(b64Val)

	if err != nil {
		return "", err
	}

	// Not sure why but b64Val can have \n.  Maybe because of env var?
	if byteVal[len(byteVal)-1] == '\n' {
		return string(byteVal[0 : len(byteVal)-1]), nil
	}

	return string(byteVal), nil
}

// Read content from file
func readFromFile(filePath string, removeTrailingNewLine bool) ([]byte, error) {
	filePath, _ = filepath.Abs(filePath)
	data, err := os.ReadFile(filePath)

	if err != nil {
		return nil, fmt.Errorf("failed to read from file `%s`: `%v`", filePath, err)
	}

	if removeTrailingNewLine {
		data = bytes.TrimSuffix(data, []byte("\n"))
	}

	return data, nil
}

// Read content from path
func readFromPath(filePath string, removeTrailingNewLine bool) ([][]byte, error) {
	filePath, _ = filepath.Abs(filePath)
	fileSysInfo, err := ioutil.ReadDir(filePath)

	if err != nil {
		return nil, fmt.Errorf("failed to read from file `%s`: `%v`", filePath, err)
	}

	result := make([][]byte, len(fileSysInfo))

	for i, file := range fileSysInfo {
		data, err := readFromFile(filepath.Join(filePath, file.Name()), removeTrailingNewLine)

		if err != nil {
			return nil, fmt.Errorf("failed to read from file `%s`: `%v`", file.Name(), err)
		}

		result[i] = data
	}

	return result, nil
}
