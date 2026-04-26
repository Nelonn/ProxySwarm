package engine

import (
	"fmt"
	"os"
	"path/filepath"
	"proxyswarm/node/internal/acme"
	"proxyswarm/node/internal/logging"
	"proxyswarm/node/internal/pb"
	"strings"
	"sync"

	"github.com/caddyserver/certmagic"
	"google.golang.org/protobuf/proto"
)

type CertificatesManager struct {
	mu     sync.RWMutex
	byName map[string]*pb.CertificateConfig
	paths  map[string]resolvedCustomCertificatePaths
}

type resolvedCustomCertificatePaths struct {
	certificatePath string
	keyPath         string
}

func NewCertificatesManager() *CertificatesManager {
	return &CertificatesManager{
		byName: make(map[string]*pb.CertificateConfig),
		paths:  make(map[string]resolvedCustomCertificatePaths),
	}
}

func (m *CertificatesManager) Sync(certificates []*pb.CertificateConfig, acmeManager *acme.Manager) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	nextByName := make(map[string]*pb.CertificateConfig, len(certificates))
	nextPaths := make(map[string]resolvedCustomCertificatePaths, len(certificates))

	for _, cert := range certificates {
		if cert == nil {
			continue
		}
		cloned, ok := proto.Clone(cert).(*pb.CertificateConfig)
		if !ok {
			continue
		}

		switch kind := cloned.Kind.(type) {
		case *pb.CertificateConfig_Acme:
			logging.Debugf("[cert] sync name=%q kind=acme domain=%q ca=%q", cloned.Name, kind.Acme.AcmeDomain, kind.Acme.AcmeCa)
			if acmeManager != nil {
				acmeManager.EnsureManagedCertificate(
					kind.Acme.AcmeEmail,
					kind.Acme.AcmeDomain,
					coalesceString(kind.Acme.AcmeType, "HTTP"),
					coalesceString(kind.Acme.AcmeCa, "letsencrypt"),
					acmeChallengePort(kind.Acme.AcmeType, kind.Acme.AcmePort, kind.Acme.AcmeHttpPort),
					"",
					"",
				)
			}
		case *pb.CertificateConfig_Custom:
			logging.Debugf("[cert] sync name=%q kind=custom cert_pem=%d key_pem=%d", cloned.Name, len(kind.Custom.CertificatePem), len(kind.Custom.KeyPem))
		case nil:
			logging.Warnf("[cert] sync name=%q has nil kind", cloned.Name)
		default:
			return fmt.Errorf("unsupported certificate kind for %s", cloned.Name)
		}

		key := strings.TrimSpace(cloned.Name)
		if key == "" {
			return fmt.Errorf("certificate name is required")
		}
		nextByName[key] = cloned
		if cached, ok := m.paths[key]; ok {
			nextPaths[key] = cached
		}
	}

	m.byName = nextByName
	m.paths = nextPaths
	return nil
}

func (m *CertificatesManager) GetCertificatePaths(name string) (string, string, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	key := strings.TrimSpace(name)
	cert := m.byName[key]
	if cert == nil {
		logging.Debugf("[cert] resolve name=%q result=not_found", key)
		return "", "", fmt.Errorf("certificate %q not found", key)
	}
	switch kind := cert.Kind.(type) {
	case *pb.CertificateConfig_Acme:
		certificatePath, keyPath := certmagicCertificatePaths(kind.Acme.AcmeCa, kind.Acme.AcmeDomain)
		logging.Debugf("[cert] resolve name=%q kind=acme domain=%q cert=%q key=%q", key, kind.Acme.AcmeDomain, certificatePath, keyPath)
		return certificatePath, keyPath, nil
	case *pb.CertificateConfig_Custom:
		certificatePath, keyPath, err := m.materializeInlineCertificate(cert)
		if err != nil {
			logging.Debugf("[cert] resolve name=%q kind=custom error=%v", key, err)
			return "", "", fmt.Errorf("failed to materialize certificate %s: %w", cert.Name, err)
		}
		logging.Debugf("[cert] resolve name=%q kind=custom cert=%q key=%q", key, certificatePath, keyPath)
		return certificatePath, keyPath, nil
	case nil:
		logging.Debugf("[cert] resolve name=%q result=nil_kind", key)
		return "", "", fmt.Errorf("certificate %q has no kind", key)
	default:
		return "", "", fmt.Errorf("unsupported certificate kind for %s", cert.Name)
	}
}

func (m *CertificatesManager) materializeInlineCertificate(cert *pb.CertificateConfig) (string, string, error) {
	if cert == nil {
		return "", "", nil
	}
	custom, ok := cert.Kind.(*pb.CertificateConfig_Custom)
	if !ok || custom.Custom == nil {
		return "", "", nil
	}
	if strings.TrimSpace(custom.Custom.CertificatePem) == "" || strings.TrimSpace(custom.Custom.KeyPem) == "" {
		return "", "", fmt.Errorf("custom certificate %q requires certificate_pem and key_pem", cert.Name)
	}
	key := strings.TrimSpace(cert.Name)
	if cached, ok := m.paths[key]; ok &&
		strings.TrimSpace(cached.certificatePath) != "" &&
		strings.TrimSpace(cached.keyPath) != "" {
		logging.Debugf("[cert] cache-hit name=%q cert=%q key=%q", key, cached.certificatePath, cached.keyPath)
		return cached.certificatePath, cached.keyPath, nil
	}
	dir := filepath.Join(defaultManagedCertsDir(), sanitizeCertificateName(cert.Name))
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", "", err
	}
	var certificatePath string
	if strings.TrimSpace(custom.Custom.CertificatePem) != "" {
		file, err := os.CreateTemp(dir, "certificate-*.pem")
		if err != nil {
			return "", "", err
		}
		path := file.Name()
		if _, err := file.WriteString(custom.Custom.CertificatePem); err != nil {
			_ = file.Close()
			return "", "", err
		}
		if err := file.Close(); err != nil {
			return "", "", err
		}
		certificatePath = path
	}
	var keyPath string
	if strings.TrimSpace(custom.Custom.KeyPem) != "" {
		file, err := os.CreateTemp(dir, "private-*.key")
		if err != nil {
			return "", "", err
		}
		path := file.Name()
		if _, err := file.WriteString(custom.Custom.KeyPem); err != nil {
			_ = file.Close()
			return "", "", err
		}
		if err := file.Close(); err != nil {
			return "", "", err
		}
		keyPath = path
	}
	if strings.TrimSpace(certificatePath) == "" || strings.TrimSpace(keyPath) == "" {
		return "", "", fmt.Errorf("custom certificate %q has empty materialized certificate/key path", cert.Name)
	}
	m.paths[key] = resolvedCustomCertificatePaths{
		certificatePath: certificatePath,
		keyPath:         keyPath,
	}
	logging.Debugf("[cert] cache-store name=%q cert=%q key=%q", key, certificatePath, keyPath)
	return certificatePath, keyPath, nil
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
