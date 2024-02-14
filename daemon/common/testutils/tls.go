package testutils

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"math/big"
	"net"
	"time"
)

// CATemplate is a template for a self-signed certificate.
var CATemplate = &x509.Certificate{
	SerialNumber: big.NewInt(1),
	Subject: pkix.Name{
		Country:      []string{"SE"},
		Organization: []string{"Company Co."},
		CommonName:   "Root CA",
	},
	NotBefore:             time.Now().Add(-10 * time.Second),
	NotAfter:              time.Now().AddDate(10, 0, 0),
	KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageCRLSign,
	ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	BasicConstraintsValid: true,
	IsCA:                  true,
	MaxPathLen:            2,
	IPAddresses:           []net.IP{net.ParseIP("127.0.0.1")},
}

var CAKey, _ = rsa.GenerateKey(rand.Reader, 2048)

// KeyFileBytes is a PEM encoded private key.
var KeyFileBytes = pem.EncodeToMemory(
	&pem.Block{
		Type:  "RSA PRIVATE KEY",
		Bytes: x509.MarshalPKCS1PrivateKey(CAKey),
	},
)

func GenerateCert() ([]byte, error) {
	// Create a self-signed certificate. template = parent
	// The parent is always allowed to sign a child
	certBytes, err := x509.CreateCertificate(rand.Reader, CATemplate, CATemplate, &CAKey.PublicKey, CAKey)
	if err != nil {
		return nil, err
	}

	certPEM := pem.EncodeToMemory(&pem.Block{
		Type:  "CERTIFICATE",
		Bytes: certBytes,
	})

	return certPEM, nil
}
