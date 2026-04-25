package acme

import (
	"context"
	"crypto/x509"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"os"
	"path/filepath"
	"proxyswarm/node/internal/logging"
	"strings"
	"sync"
	"time"

	"github.com/caddyserver/certmagic"
	"go.uber.org/zap"
	"go.uber.org/zap/zapcore"
)

const (
	defaultStatePath         = "acme_state.json"
	defaultStoragePath       = "acme_storage"
	renewBefore              = 30 * 24 * time.Hour
	failedRenewRetryInterval = time.Hour
	minScheduleDelay         = time.Minute
)

type ManagedCertificate struct {
	Email           string    `json:"email"`
	Domain          string    `json:"domain"`
	ChallengeType   string    `json:"challenge_type"`
	CA              string    `json:"ca"`
	Port            int32     `json:"port"`
	CertificatePath string    `json:"certificate_path"`
	KeyPath         string    `json:"key_path"`
	NextRenewAt     time.Time `json:"next_renew_at"`
	LastIssuedAt    time.Time `json:"last_issued_at"`
	LastError       string    `json:"last_error"`
}

type IssueResult struct {
	Resource        []byte
	Logs            []string
	CertificatePath string
	KeyPath         string
	ExpiryTime      time.Time
}

type Manager struct {
	mu        sync.Mutex
	statePath string
	storageDir string
	managed   map[string]ManagedCertificate
	timers    map[string]*time.Timer
}

type acmeLogCapture struct {
	domain string
	lines  *[]string
}

func (c *acmeLogCapture) append(line string) {
	line = strings.TrimSpace(line)
	if line == "" {
		return
	}
	*c.lines = append(*c.lines, line)
	fmt.Printf("[acme][%s] %s\n", c.domain, line)
}

func (c *acmeLogCapture) Write(p []byte) (int, error) {
	for _, line := range strings.Split(string(p), "\n") {
		c.append(line)
	}
	return len(p), nil
}

func (c *acmeLogCapture) Sync() error {
	return nil
}

func challengePort(challengeType string, configuredPort int32) int32 {
	switch normalizedChallengeType(challengeType) {
	case "TLS":
		if configuredPort > 0 {
			return configuredPort
		}
		return 443
	case "DNS":
		return 0
	default:
		if configuredPort > 0 {
			return configuredPort
		}
		return 80
	}
}

func NewManager(dataRoot string) *Manager {
	statePath := defaultStatePath
	storageDir := defaultStoragePath
	if strings.TrimSpace(dataRoot) != "" {
		statePath = filepath.Join(dataRoot, defaultStatePath)
		storageDir = filepath.Join(dataRoot, defaultStoragePath)
	}
	m := &Manager{
		statePath: statePath,
		storageDir: storageDir,
		managed:   make(map[string]ManagedCertificate),
		timers:    make(map[string]*time.Timer),
	}
	m.loadState()
	m.restoreSchedules()
	return m
}

func (m *Manager) Issue(email, domain string, port, httpPort int32) ([]byte, error) {
	result, err := m.IssueWithLogs(email, domain, "HTTP", "letsencrypt", port, "", "")
	return result.Resource, err
}

func (m *Manager) EnsureManagedCertificate(email, domain, challengeType, ca string, port int32, certificatePath, keyPath string) []string {
	m.mu.Lock()
	defer m.mu.Unlock()

	logs := []string{
		fmt.Sprintf("Ensuring managed certificate for %s", domain),
	}

	entry := ManagedCertificate{
		Email:           email,
		Domain:          domain,
		ChallengeType:   normalizedChallengeType(challengeType),
		CA:              normalizedCA(ca),
		Port:            challengePort(challengeType, port),
		CertificatePath: certificatePath,
		KeyPath:         keyPath,
	}
	entry = m.withDefaultPaths(entry)
	logs = append(logs, fmt.Sprintf("Using certificate path %s", entry.CertificatePath))
	logs = append(logs, fmt.Sprintf("Using key path %s", entry.KeyPath))

	existingCert, certErr := readCertificateExpiry(entry.CertificatePath)
	if certErr != nil {
		logs = append(logs, fmt.Sprintf("Existing certificate not usable: %v", certErr))
	} else {
		entry.LastIssuedAt = time.Now()
		entry.NextRenewAt = renewalTime(existingCert.NotAfter)
		logs = append(logs, fmt.Sprintf("Existing certificate valid until %s", existingCert.NotAfter.Format(time.RFC3339)))
		logs = append(logs, fmt.Sprintf("Next renewal scheduled for %s", entry.NextRenewAt.Format(time.RFC3339)))
	}

	if certErr != nil || time.Until(entry.NextRenewAt) <= 0 {
		result, err := m.issueWithLogsLocked(entry)
		logs = append(logs, result.Logs...)
		if err != nil {
			entry.LastError = err.Error()
			entry.NextRenewAt = time.Now().Add(failedRenewRetryInterval)
			logs = append(logs, fmt.Sprintf("Renew retry scheduled for %s", entry.NextRenewAt.Format(time.RFC3339)))
		} else if result != nil && !result.ExpiryTime.IsZero() {
				entry.LastIssuedAt = time.Now()
				entry.NextRenewAt = renewalTime(result.ExpiryTime)
				entry.LastError = ""
				logs = append(logs, fmt.Sprintf("Renewal scheduled for %s", entry.NextRenewAt.Format(time.RFC3339)))
		}
	}

	m.managed[entry.Domain] = entry
	m.scheduleLocked(entry)
	_ = m.saveStateLocked()
	return logs
}

func (m *Manager) IssueWithLogs(email, domain, challengeType, ca string, port int32, certificatePath, keyPath string) (*IssueResult, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.issueWithLogsLocked(ManagedCertificate{
		Email:           email,
		Domain:          domain,
		ChallengeType:   normalizedChallengeType(challengeType),
		CA:              normalizedCA(ca),
		Port:            challengePort(challengeType, port),
		CertificatePath: certificatePath,
		KeyPath:         keyPath,
	})
}

func (m *Manager) issueWithLogsLocked(entry ManagedCertificate) (*IssueResult, error) {
	entry = m.withDefaultPaths(entry)
	logs := []string{
		fmt.Sprintf("Preparing ACME request for %s", entry.Domain),
		fmt.Sprintf("Challenge type: %s", entry.ChallengeType),
		fmt.Sprintf("Certificate authority: %s", entry.CA),
	}
	capture := &acmeLogCapture{
		domain: entry.Domain,
		lines:  &logs,
	}
	logger := zap.New(zapcore.NewCore(
		zapcore.NewConsoleEncoder(zap.NewProductionEncoderConfig()),
		zapcore.AddSync(capture),
		logging.ZapLevel(),
	))
	storage := &certmagic.FileStorage{Path: m.storageDir}
	magic := certmagic.NewDefault()
	magic.Storage = storage
	magic.Logger = logger
	magic.Issuers = nil

	issuerTemplate := certmagic.ACMEIssuer{
		Email:  entry.Email,
		Agreed: true,
		Logger: logger,
	}
	if caURL, ok := caDirectoryURL(entry.CA); ok {
		issuerTemplate.CA = caURL
		logs = append(logs, fmt.Sprintf("Using CA directory: %s", caURL))
	} else {
		issuerTemplate.CA = certmagic.LetsEncryptProductionCA
		logs = append(logs, fmt.Sprintf("Unknown CA %q, falling back to Let's Encrypt", entry.CA))
	}
	switch entry.ChallengeType {
	case "", "HTTP":
		issuerTemplate.DisableHTTPChallenge = false
		issuerTemplate.DisableTLSALPNChallenge = true
		issuerTemplate.AltHTTPPort = int(entry.Port)
		logs = append(logs, fmt.Sprintf("HTTP-01 challenge enabled on port %d", entry.Port))
	case "TLS":
		issuerTemplate.DisableHTTPChallenge = true
		issuerTemplate.DisableTLSALPNChallenge = false
		issuerTemplate.AltTLSALPNPort = int(entry.Port)
		logs = append(logs, fmt.Sprintf("TLS-ALPN-01 challenge enabled on port %d", entry.Port))
	case "DNS":
		err := fmt.Errorf("DNS challenge requires provider-specific credentials and is not implemented yet")
		logs = append(logs, err.Error())
		return &IssueResult{Logs: logs}, err
	default:
		err := fmt.Errorf("unsupported ACME challenge type: %s", entry.ChallengeType)
		logs = append(logs, err.Error())
		return &IssueResult{Logs: logs}, err
	}
	issuer := certmagic.NewACMEIssuer(magic, issuerTemplate)
	magic.Issuers = []certmagic.Issuer{issuer}

	logs = append(logs, "Starting certificate obtain flow")
	if err := magic.ObtainCertSync(context.Background(), entry.Domain); err != nil {
		logs = append(logs, fmt.Sprintf("Certificate obtain failed: %v", err))
		return &IssueResult{
			Logs:            logs,
			CertificatePath: entry.CertificatePath,
			KeyPath:         entry.KeyPath,
		}, err
	}
	logs = append(logs, "Certificate obtained successfully")

	certKey := certmagic.StorageKeys.SiteCert(issuer.IssuerKey(), entry.Domain)
	keyKey := certmagic.StorageKeys.SitePrivateKey(issuer.IssuerKey(), entry.Domain)
	entry.CertificatePath = filepath.Join(storage.Path, filepath.FromSlash(certKey))
	entry.KeyPath = filepath.Join(storage.Path, filepath.FromSlash(keyKey))
	resource, err := storage.Load(context.Background(), certKey)
	if err != nil {
		logs = append(logs, fmt.Sprintf("Failed to load certificate from certmagic storage: %v", err))
		return &IssueResult{
			Logs:            logs,
			CertificatePath: entry.CertificatePath,
			KeyPath:         entry.KeyPath,
		}, err
	}
	if _, err := storage.Load(context.Background(), keyKey); err != nil {
		logs = append(logs, fmt.Sprintf("Failed to load private key from certmagic storage: %v", err))
		return &IssueResult{
			Resource:        resource,
			Logs:            logs,
			CertificatePath: entry.CertificatePath,
			KeyPath:         entry.KeyPath,
		}, err
	}

	logs = append(logs, fmt.Sprintf("Using certmagic certificate path %s", entry.CertificatePath))
	logs = append(logs, fmt.Sprintf("Using certmagic private key path %s", entry.KeyPath))

	result := &IssueResult{
		Resource:        resource,
		Logs:            logs,
		CertificatePath: entry.CertificatePath,
		KeyPath:         entry.KeyPath,
	}
	if leaf, parseErr := leafCertificate(resource); parseErr == nil {
		result.ExpiryTime = leaf.NotAfter
		entry.LastIssuedAt = time.Now()
		entry.NextRenewAt = renewalTime(leaf.NotAfter)
		entry.LastError = ""
		m.managed[entry.Domain] = entry
		m.scheduleLocked(entry)
		if err := m.saveStateLocked(); err != nil {
			logs = append(logs, fmt.Sprintf("Failed to persist renewal state: %v", err))
		} else {
			logs = append(logs, fmt.Sprintf("Auto-renew scheduled for %s", entry.NextRenewAt.Format(time.RFC3339)))
		}
	}

	return result, nil
}

func (m *Manager) restoreSchedules() {
	m.mu.Lock()
	defer m.mu.Unlock()

	for _, entry := range m.managed {
		m.scheduleLocked(entry)
	}
}

func (m *Manager) scheduleLocked(entry ManagedCertificate) {
	if existing := m.timers[entry.Domain]; existing != nil {
		existing.Stop()
	}

	delay := time.Until(entry.NextRenewAt)
	if delay <= 0 {
		delay = minScheduleDelay
	}

	domain := entry.Domain
	m.timers[domain] = time.AfterFunc(delay, func() {
		m.runScheduledRenewal(domain)
	})
}

func (m *Manager) runScheduledRenewal(domain string) {
	m.mu.Lock()
	entry, ok := m.managed[domain]
	m.mu.Unlock()
	if !ok {
		return
	}

	result, err := m.IssueWithLogs(
		entry.Email,
		entry.Domain,
		entry.ChallengeType,
		entry.CA,
		entry.Port,
		entry.CertificatePath,
		entry.KeyPath,
	)
	logs := result.Logs

	m.mu.Lock()
	defer m.mu.Unlock()

	current, ok := m.managed[domain]
	if !ok {
		return
	}

	if err != nil {
		current.LastError = err.Error()
		current.NextRenewAt = time.Now().Add(failedRenewRetryInterval)
	} else if result != nil && result.Resource != nil {
		if !result.ExpiryTime.IsZero() {
			current.LastIssuedAt = time.Now()
			current.NextRenewAt = renewalTime(result.ExpiryTime)
			current.LastError = ""
		} else {
			current.LastError = "certificate expiry unavailable after renewal"
			current.NextRenewAt = time.Now().Add(failedRenewRetryInterval)
			err = fmt.Errorf(current.LastError)
		}
	}

	m.managed[domain] = current
	m.scheduleLocked(current)
	_ = m.saveStateLocked()

	for _, line := range logs {
		fmt.Printf("[acme][%s] %s\n", domain, line)
	}
	if err != nil {
		fmt.Printf("[acme][%s] renewal failed: %v\n", domain, err)
	} else {
		fmt.Printf("[acme][%s] next renewal at %s\n", domain, current.NextRenewAt.Format(time.RFC3339))
	}
}

func (m *Manager) loadState() {
	m.mu.Lock()
	defer m.mu.Unlock()

	data, err := os.ReadFile(m.statePath)
	if err != nil {
		return
	}

	var entries []ManagedCertificate
	if err := json.Unmarshal(data, &entries); err != nil {
		fmt.Printf("[acme] failed to load persisted state: %v\n", err)
		return
	}

	for _, entry := range entries {
		if strings.TrimSpace(entry.Domain) == "" {
			continue
		}
		m.managed[entry.Domain] = entry
	}
}

func (m *Manager) saveStateLocked() error {
	entries := make([]ManagedCertificate, 0, len(m.managed))
	for _, entry := range m.managed {
		entries = append(entries, entry)
	}

	data, err := json.MarshalIndent(entries, "", "  ")
	if err != nil {
		return err
	}

	dir := filepath.Dir(m.statePath)
	if dir != "." && dir != "" {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return err
		}
	}

	return os.WriteFile(m.statePath, data, 0o600)
}

func readCertificateExpiry(certificatePath string) (*x509.Certificate, error) {
	data, err := os.ReadFile(certificatePath)
	if err != nil {
		return nil, err
	}
	return leafCertificate(data)
}

func leafCertificate(pemData []byte) (*x509.Certificate, error) {
	block, _ := pem.Decode(pemData)
	if block == nil {
		return nil, fmt.Errorf("certificate PEM not found")
	}
	return x509.ParseCertificate(block.Bytes)
}

func renewalTime(notAfter time.Time) time.Time {
	renewAt := notAfter.Add(-renewBefore)
	if renewAt.Before(time.Now().Add(minScheduleDelay)) {
		return time.Now().Add(minScheduleDelay)
	}
	return renewAt
}

func normalizedChallengeType(value string) string {
	switch strings.ToUpper(strings.TrimSpace(value)) {
	case "TLS":
		return "TLS"
	case "DNS":
		return "DNS"
	default:
		return "HTTP"
	}
}

func (m *Manager) withDefaultPaths(entry ManagedCertificate) ManagedCertificate {
	if strings.TrimSpace(entry.Domain) == "" {
		return entry
	}
	storage := &certmagic.FileStorage{Path: m.storageDir}
	issuerKey, ok := caDirectoryURL(entry.CA)
	if !ok {
		issuerKey = certmagic.LetsEncryptProductionCA
	}
	issuer := certmagic.ACMEIssuer{CA: issuerKey}
	certKey := certmagic.StorageKeys.SiteCert(issuer.IssuerKey(), entry.Domain)
	keyKey := certmagic.StorageKeys.SitePrivateKey(issuer.IssuerKey(), entry.Domain)
	entry.CertificatePath = filepath.Join(storage.Path, filepath.FromSlash(certKey))
	entry.KeyPath = filepath.Join(storage.Path, filepath.FromSlash(keyKey))
	return entry
}

func normalizedCA(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "zerossl":
		return "zerossl"
	case "google":
		return "google"
	case "buypass":
		return "buypass"
	case "sslcom":
		return "sslcom"
	default:
		return "letsencrypt"
	}
}

func caDirectoryURL(ca string) (string, bool) {
	switch normalizedCA(ca) {
	case "letsencrypt":
		return certmagic.LetsEncryptProductionCA, true
	case "zerossl":
		return "https://acme.zerossl.com/v2/DV90", true
	case "google":
		return "https://dv.acme-v02.api.pki.goog/directory", true
	case "buypass":
		return "https://api.buypass.com/acme/directory", true
	case "sslcom":
		return "https://acme.ssl.com/sslcom-dv-rsa", true
	default:
		return "", false
	}
}
