package flags

// PasswordFlag defines a Cobra compatible
// flag for password related options.
// examples include
// --password
// --tls-keyfile-password
type PasswordFlag []byte

func (flag *PasswordFlag) Set(val string) error {
	result, err := flagFormatParser(val, flagFormatB64|flagFormatEnvB64|flagFormatFile|flagFormatEnv)

	if err != nil {
		return err
	}

	if err == nil && result == "" {
		result = val
	}

	*flag = PasswordFlag(result)

	return nil
}

func (flag *PasswordFlag) Type() string {
	return "\"env-b64:<env-var>,b64:<b64-pass>,file:<pass-file>,<clear-pass>\""
}

func (flag *PasswordFlag) String() string {
	return string(*flag)
}
