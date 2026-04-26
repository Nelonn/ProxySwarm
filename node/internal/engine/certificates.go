package engine

import (
	"fmt"
	"os"
	"path/filepath"
	"proxyswarm/node/internal/acme"
	"proxyswarm/node/internal/pb"
	"strings"
	"sync"

	"github.com/caddyserver/certmagic"
	"google.golang.org/protobuf/proto"
)

type CertificatesManager struct {
	mu     sync.RWMutex
	byName map[string]*pb.CertificateConfig
}

func NewCertificatesManager() *CertificatesManager {
	return &CertificatesManager{
		byName: make(map[string]*pb.CertificateConfig),
	}
}

func (m *CertificatesManager) Sync(certificates []*pb.CertificateConfig, acmeManager *acme.Manager) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	nextByName := make(map[string]*pb.CertificateConfig, len(certificates))

	for _, cert := range certificates {
		if cert == nil {
			continue
		}
		cloned, ok := proto.Clone(cert).(*pb.CertificateConfig)
		if !ok {
			continue
		}

		switch strings.ToUpper(strings.TrimSpace(cloned.CertType)) {
		case "ACME":
			if acmeManager != nil {
				acmeManager.EnsureManagedCertificate(
					cloned.AcmeEmail,
					cloned.AcmeDomain,
					coalesceString(cloned.AcmeType, "HTTP"),
					coalesceString(cloned.AcmeCa, "letsencrypt"),
					acmeChallengePort(cloned.AcmeType, cloned.AcmePort, cloned.AcmeHttpPort),
					cloned.CertificatePath,
					cloned.KeyPath,
				)
			}
		case "", "CUSTOM":
		default:
			return fmt.Errorf("unsupported certificate type %q for %s", cloned.CertType, cloned.Name)
		}

		key := strings.TrimSpace(cloned.Name)
		if key == "" {
			return fmt.Errorf("certificate name is required")
		}
		nextByName[key] = cloned
	}

	m.byName = nextByName
	return nil
}

func (m *CertificatesManager) GetCertificatePaths(name string) (string, string, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	key := strings.TrimSpace(name)
	cert := m.byName[key]
	if cert == nil {
		return "", "", fmt.Errorf("certificate %q not found", key)
	}
	switch strings.ToUpper(strings.TrimSpace(cert.CertType)) {
	case "ACME":
		certificatePath, keyPath := certmagicCertificatePaths(cert.AcmeCa, cert.AcmeDomain)
		cert.CertificatePath = certificatePath
		cert.KeyPath = keyPath
	case "", "CUSTOM":
		if err := materializeInlineCertificate(cert); err != nil {
			return "", "", fmt.Errorf("failed to materialize certificate %s: %w", cert.Name, err)
		}
	default:
		return "", "", fmt.Errorf("unsupported certificate type %q for %s", cert.CertType, cert.Name)
	}
	if strings.TrimSpace(cert.CertificatePath) == "" || strings.TrimSpace(cert.KeyPath) == "" {
		return "", "", fmt.Errorf("certificate %q has empty certificate/key path", key)
	}
	return cert.CertificatePath, cert.KeyPath, nil
}

func materializeInlineCertificate(cert *pb.CertificateConfig) error {
	if cert == nil {
		return nil
	}
	if strings.TrimSpace(cert.CertificatePem) == "" && strings.TrimSpace(cert.KeyPem) == "" {
		return nil
	}
	dir := filepath.Join(defaultManagedCertsDir(), sanitizeCertificateName(cert.Name))
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	if strings.TrimSpace(cert.CertificatePem) != "" {
		path := filepath.Join(dir, "certificate.pem")
		if err := os.WriteFile(path, []byte(cert.CertificatePem), 0o600); err != nil {
			return err
		}
		cert.CertificatePath = path
	}
	if strings.TrimSpace(cert.KeyPem) != "" {
		path := filepath.Join(dir, "private.key")
		if err := os.WriteFile(path, []byte(cert.KeyPem), 0o600); err != nil {
			return err
		}
		cert.KeyPath = path
	}
	return nil
}

func sanitizeCertificateName(value string) string {
	value = strings.TrimSpace(value)
	if value == "" {
		return "unnamed"
	}
	replacer := strings.NewReplacer("\\", "-", "/", "-", ":", "-", "*", "-", "?", "-", "\"", "-", "<", "-", ">", "-", "|", "-")
	value = replacer.Replace(value)
	value = strings.Trim(value, ". ")
	if value == "" {
		return "unnamed"
	}
	return value
}

func certmagicCertificatePaths(ca string, domain string) (string, string) {
	storage := &certmagic.FileStorage{Path: filepath.Join(defaultDataRoot(), "acme_storage")}
	caURL, ok := acmeDirectoryURL(ca)
	if !ok {
		caURL = certmagic.LetsEncryptProductionCA
	}
	issuer := certmagic.ACMEIssuer{CA: caURL}
	certKey := certmagic.StorageKeys.SiteCert(issuer.IssuerKey(), domain)
	keyKey := certmagic.StorageKeys.SitePrivateKey(issuer.IssuerKey(), domain)
	return filepath.Join(storage.Path, filepath.FromSlash(certKey)), filepath.Join(storage.Path, filepath.FromSlash(keyKey))
}

func acmeDirectoryURL(value string) (string, bool) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "zerossl":
		return "https://acme.zerossl.com/v2/DV90", true
	case "google":
		return "https://dv.acme-v02.api.pki.goog/directory", true
	case "buypass":
		return "https://api.buypass.com/acme/directory", true
	case "sslcom":
		return "https://acme.ssl.com/sslcom-dv-rsa", true
	case "", "letsencrypt":
		return certmagic.LetsEncryptProductionCA, true
	default:
		return "", false
	}
}
