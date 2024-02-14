package client

import (
	"crypto/tls"
	"crypto/x509"
	"encoding/pem"
	"fmt"

	as "github.com/aerospike/aerospike-client-go/v7"
)

// AerospikeConfig represents the intermediate configuration for an Aerospike
// client. This can be constructed directly using flags.AerospikeFlags or
type AerospikeConfig struct {
	Socket                 string
	Seeds                  HostTLSPortSlice
	User                   string
	Password               string
	AuthMode               as.AuthMode
	RootCA                 [][]byte
	Cert                   []byte
	Key                    []byte
	KeyPass                []byte
	TLSProtocolsMinVersion TLSProtocol
	TLSProtocolsMaxVersion TLSProtocol
	// TLSCipherSuites        []uint16 // TODO
}

// NewDefaultAerospikeConfig creates a new default AerospikeConfig instance.
func NewDefaultAerospikeConfig() *AerospikeConfig {
	return &AerospikeConfig{
		Seeds: HostTLSPortSlice{NewDefaultHostTLSPort()},
	}
}

// NewClientPolicy creates a new Aerospike client policy based on the
// AerospikeConfig.

func (ac *AerospikeConfig) NewClientPolicy() (*as.ClientPolicy, error) {
	clientPolicy := as.NewClientPolicy()
	clientPolicy.User = ac.User
	clientPolicy.Password = ac.Password
	clientPolicy.AuthMode = ac.AuthMode

	tlsConfig, err := ac.newTLSConfig()
	if err != nil {
		return nil, err
	}

	clientPolicy.TlsConfig = tlsConfig

	return clientPolicy, nil
}

func (ac *AerospikeConfig) NewHosts() []*as.Host {
	hosts := []*as.Host{}

	for _, seed := range ac.Seeds {
		host := as.NewHost(seed.Host, seed.Port)

		if seed.TLSName != "" {
			host.TLSName = seed.TLSName
		}

		hosts = append(hosts, host)
	}

	return hosts
}

func (ac *AerospikeConfig) newTLSConfig() (*tls.Config, error) {
	if len(ac.RootCA) == 0 && len(ac.Cert) == 0 && len(ac.Key) == 0 {
		return nil, nil
	}

	var (
		clientPool []tls.Certificate
		serverPool *x509.CertPool
		err        error
	)

	serverPool = loadCACerts(ac.RootCA)

	if len(ac.Cert) > 0 || len(ac.Key) > 0 {
		clientPool, err = loadServerCertAndKey(ac.Cert, ac.Key, ac.KeyPass)
		if err != nil {
			return nil, fmt.Errorf("failed to load client authentication certificate and key `%s`", err)
		}
	}

	tlsConfig := &tls.Config{ //nolint:gosec // aerospike default tls version is TLSv1.2
		Certificates:             clientPool,
		RootCAs:                  serverPool,
		InsecureSkipVerify:       false,
		PreferServerCipherSuites: true,
		MinVersion:               uint16(ac.TLSProtocolsMinVersion),
		MaxVersion:               uint16(ac.TLSProtocolsMaxVersion),
	}

	return tlsConfig, nil
}

// loadCACerts returns CA set of certificates (cert pool)
// reads CA certificate based on the certConfig and adds it to the pool
func loadCACerts(certsBytes [][]byte) *x509.CertPool {
	certificates, err := x509.SystemCertPool()
	if certificates == nil || err != nil {
		certificates = x509.NewCertPool()
	}

	for _, cert := range certsBytes {
		if len(cert) > 0 {
			certificates.AppendCertsFromPEM(cert)
		}
	}

	return certificates
}

// loadServerCertAndKey reads server certificate and associated key file based on certConfig and keyConfig
// returns parsed server certificate
// if the private key is encrypted, it will be decrypted using key file passphrase
func loadServerCertAndKey(certFileBytes, keyFileBytes, keyPassBytes []byte) ([]tls.Certificate, error) {
	var certificates []tls.Certificate

	// Decode PEM data
	keyBlock, _ := pem.Decode(keyFileBytes)

	if keyBlock == nil {
		return nil, fmt.Errorf("failed to decode PEM data for key or certificate")
	}

	// Check and Decrypt the Key Block using passphrase
	if x509.IsEncryptedPEMBlock(keyBlock) { //nolint:staticcheck,lll // This needs to be addressed by aerospike as multiple projects require this functionality
		decryptedDERBytes, err := x509.DecryptPEMBlock(keyBlock, keyPassBytes) //nolint:staticcheck,lll // This needs to be addressed by aerospike as multiple projects require this functionality
		if err != nil {
			return nil, fmt.Errorf("failed to decrypt PEM Block: `%s`", err)
		}

		keyBlock.Bytes = decryptedDERBytes
		keyBlock.Headers = nil
	}

	// Encode PEM data
	keyPEM := pem.EncodeToMemory(keyBlock)

	if keyPEM == nil {
		return nil, fmt.Errorf("failed to encode PEM data for key or certificate")
	}

	cert, err := tls.X509KeyPair(certFileBytes, keyPEM)
	if err != nil {
		return nil, fmt.Errorf("failed to add certificate and key to the pool: `%s`", err)
	}

	certificates = append(certificates, cert)

	return certificates, nil
}
