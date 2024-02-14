package flags

import (
	"bytes"
	"os"
	"testing"
)

func Test_flagFormatParser(t *testing.T) {
	envVar := "flag_test_parse_env"
	envVarVal := "birdistheword"
	envVarB64 := "flag_test_parse_envb64"
	envVarB64Val := "bHlsZWx5bGVjcm9jb2RpbGU=" // lylelylecorcodile in plaintext
	envVarB64Bad := "flag_test_parse_env_bad_b64"
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

	type args struct {
		val  string
		mode flagFormat
	}

	tests := []struct {
		name    string
		args    args
		want    string
		wantErr bool
	}{
		{
			name: "t1",
			args: args{
				val:  "env:" + envVar,
				mode: flagFormatEnv | flagFormatFile,
			},
			want:    envVarVal,
			wantErr: false,
		},
		{
			name: "t2",
			args: args{
				val:  "env:VarDoesNotExist",
				mode: flagFormatEnv,
			},
			want:    "",
			wantErr: true,
		},
		{
			name: "t2",
			args: args{
				val:  "env:EnvVarNotSupported",
				mode: flagFormatFile,
			},
			want:    "",
			wantErr: true,
		},
		{
			name: "t3",
			args: args{
				val:  "env-b64:" + envVarB64,
				mode: flagFormatEnv | flagFormatFile | flagFormatEnvB64,
			},
			want:    decodedB64EnvVal,
			wantErr: false,
		},
		{
			name: "t4",
			args: args{
				val:  "env-b64:VarDoesNotExist",
				mode: flagFormatFile | flagFormatEnvB64,
			},
			want:    "",
			wantErr: true,
		},
		{
			name: "t5",
			args: args{
				val:  "env-b64:" + envVarB64BadVal,
				mode: flagFormatEnvB64,
			},
			want:    "",
			wantErr: true,
		},
		{
			name: "t5",
			args: args{
				val:  "env-b64:Notsupported",
				mode: flagFormatEnvB64,
			},
			want:    "",
			wantErr: true,
		},
		{
			name: "t6",
			args: args{
				val:  "b64:" + envVarB64Val,
				mode: flagFormatB64,
			},
			want:    decodedB64EnvVal,
			wantErr: false,
		},
		{
			name: "t7",
			args: args{
				val:  "b64:" + envVarB64BadVal,
				mode: flagFormatB64,
			},
			want:    "",
			wantErr: true,
		},
		{
			name: "t7",
			args: args{
				val:  "b64:B64NotSupported",
				mode: flagFormatFile,
			},
			want:    "",
			wantErr: true,
		},
		{
			name: "t8",
			args: args{
				val:  "file:" + fpath,
				mode: flagFormatFile,
			},
			want:    string(fdata),
			wantErr: false,
		},
		{
			name: "t9",
			args: args{
				val:  "file:./filedoesnotexist.go",
				mode: flagFormatFile,
			},
			want:    "",
			wantErr: true,
		},
		{
			name: "t10",
			args: args{
				val:  "file:FileNotSupported",
				mode: flagFormatEnv,
			},
			want:    "",
			wantErr: true,
		},
		{
			name: "t11",
			args: args{
				val:  "noSplit:",
				mode: flagFormatFile | flagFormatEnv,
			},
			want:    "",
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := flagFormatParser(tt.args.val, tt.args.mode)
			if (err != nil) != tt.wantErr {
				t.Errorf("flagFormatParser() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if got != tt.want {
				t.Errorf("flagFormatParser() = %v, want %v", got, tt.want)
			}
		})
	}
}
