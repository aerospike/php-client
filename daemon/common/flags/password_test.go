package flags

import (
	"bytes"
	"os"
	"reflect"
	"testing"
)

func TestPassword(t *testing.T) {
	envVar := "flag_test_password_env"
	envVarVal := "birdistheword"
	envVarB64 := "flag_test_password_envb64"
	envVarB64Val := "bHlsZWx5bGVjcm9jb2RpbGU=" // lylelylecorcodile in plaintext
	envVarB64Bad := "flag_test_password_env_bad_b64"
	envVarB64BadVal := "BadBase64.....!"

	os.Setenv(envVar, envVarVal)
	os.Setenv(envVarB64, envVarB64Val)
	os.Setenv(envVarB64Bad, envVarB64BadVal)

	defer func() {
		os.Unsetenv(envVar)
		os.Unsetenv(envVarB64)
		os.Unsetenv(envVarB64Bad)
	}()

	decodedB64EnvVal, err := decode64(envVarB64Val)
	if err != nil {
		t.Error(err.Error())
	}

	fpath := "./testdata/filedata"

	fdata, err := os.ReadFile(fpath)
	if err != nil {
		t.Error(err.Error())
	}

	// trim the trailing new line to match readFromFile
	fdata = bytes.TrimSuffix(fdata, []byte("\n"))

	testCases := []struct {
		name    string
		input   string
		output  PasswordFlag
		wantErr bool
	}{
		{
			name:    "t1",
			input:   "env-b64:" + envVarB64,
			output:  PasswordFlag(decodedB64EnvVal),
			wantErr: false,
		},
		{
			name:    "t2",
			input:   "env-b64:" + envVarB64Bad,
			output:  PasswordFlag(""),
			wantErr: true,
		},
		{
			name:    "t3",
			input:   "b64:" + envVarB64Val,
			output:  PasswordFlag(decodedB64EnvVal),
			wantErr: false,
		},
		{
			name:    "t4",
			input:   "file:" + fpath,
			output:  PasswordFlag(fdata),
			wantErr: false,
		},
		{
			name:    "t5",
			input:   "file:" + "filedoesnotexist.go",
			output:  PasswordFlag(""),
			wantErr: true,
		},
		{
			name:    "t6",
			input:   "env:" + envVar,
			output:  PasswordFlag(envVarVal),
			wantErr: false,
		},
		{
			name:    "t7",
			input:   fpath,
			output:  PasswordFlag(fpath),
			wantErr: false,
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			actual := PasswordFlag{}
			err := actual.Set(tc.input)
			if (err != nil) != tc.wantErr {
				t.Errorf("flagFormatParser() error = %v, wantErr %v", err, tc.wantErr)
				return
			}
			if !reflect.DeepEqual(actual, tc.output) {
				t.Errorf("flagFormatParser() = %v, want %v", string(actual), string(tc.output))
			}
		})
	}
}
