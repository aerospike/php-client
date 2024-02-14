package flags

import "testing"

func TestConfFileFlags_NewFlagSet(t *testing.T) {
	confFileFlags := NewConfFileFlags()
	flagSet := confFileFlags.NewFlagSet(func(str string) string { return str })
	err := flagSet.Parse([]string{"--config-file", "test.toml", "--instance", "a"})

	if err != nil {
		t.Errorf("Expected nil, got %s", err.Error())
	}

	if confFileFlags.File != "test.toml" {
		t.Errorf("Expected %s, got %s", "test.toml", confFileFlags.File)
	}

	if confFileFlags.Instance != "a" {
		t.Errorf("Expected %s, got %s", "a", confFileFlags.Instance)
	}
}
