package acme

import (
	"crypto"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/go-acme/lego/v4/certificate"
	"github.com/go-acme/lego/v4/challenge/http01"
	"github.com/go-acme/lego/v4/challenge/tlsalpn01"
	"github.com/go-acme/lego/v4/lego"
	"github.com/go-acme/lego/v4/registration"
)

const (
	defaultStatePath         = "acme_state.json"
	renewBefore              = 30 * 24 * time.Hour
	failedRenewRetryInterval = time.Hour
	minScheduleDelay         = time.Minute
)

type User struct {
	Email        string
	Registration *registration.Resource
	key          crypto.PrivateKey
}

func (u *User) GetEmail() string {
	return u.Email
}

func (u *User) GetRegistration() *registration.Resource {
	return u.Registration
}

func (u *User) GetPrivateKey() crypto.PrivateKey {
	return u.key
}

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

type Manager struct {
	mu        sync.Mutex
	statePath string
	managed   map[string]ManagedCertificate
	timers    map[string]*time.Timer
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

func NewManager() *Manager {
	m := &Manager{
		statePath: defaultStatePath,
		managed:   make(map[string]ManagedCertificate),
		timers:    make(map[string]*time.Timer),
	}
	m.loadState()
	m.restoreSchedules()
	return m
}

func (m *Manager) Issue(email, domain string, port, httpPort int32) (*certificate.Resource, error) {
	resource, _, err := m.IssueWithLogs(email, domain, "HTTP", "letsencrypt", port, "", "")
	return resource, err
}

func (m *Manager) EnsureManagedCertificate(email, domain, challengeType, ca string, port int32, certificatePath, keyPath string) []string {
	m.mu.Lock()
	defer m.mu.Unlock()

	logs := []string{
		fmt.Sprintf("Ensuring managed certificate for %s", domain),
	}

	if strings.TrimSpace(certificatePath) == "" || strings.TrimSpace(keyPath) == "" {
		logs = append(logs, "Certificate path or key path missing, auto-renew disabled for this certificate")
		return logs
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

	existingCert, certErr := readCertificateExpiry(certificatePath)
	if certErr != nil {
		logs = append(logs, fmt.Sprintf("Existing certificate not usable: %v", certErr))
	} else {
		entry.LastIssuedAt = time.Now()
		entry.NextRenewAt = renewalTime(existingCert.NotAfter)
		logs = append(logs, fmt.Sprintf("Existing certificate valid until %s", existingCert.NotAfter.Format(time.RFC3339)))
		logs = append(logs, fmt.Sprintf("Next renewal scheduled for %s", entry.NextRenewAt.Format(time.RFC3339)))
	}

	if certErr != nil || time.Until(entry.NextRenewAt) <= 0 {
		resource, issueLogs, err := m.issueWithLogsLocked(entry)
		logs = append(logs, issueLogs...)
		if err != nil {
			entry.LastError = err.Error()
			entry.NextRenewAt = time.Now().Add(failedRenewRetryInterval)
			logs = append(logs, fmt.Sprintf("Renew retry scheduled for %s", entry.NextRenewAt.Format(time.RFC3339)))
		} else if resource != nil {
			if leaf, parseErr := leafCertificate(resource.Certificate); parseErr == nil {
				entry.LastIssuedAt = time.Now()
				entry.NextRenewAt = renewalTime(leaf.NotAfter)
				entry.LastError = ""
				logs = append(logs, fmt.Sprintf("Renewal scheduled for %s", entry.NextRenewAt.Format(time.RFC3339)))
			}
		}
	}

	m.managed[entry.Domain] = entry
	m.scheduleLocked(entry)
	_ = m.saveStateLocked()
	return logs
}

func (m *Manager) IssueWithLogs(email, domain, challengeType, ca string, port int32, certificatePath, keyPath string) (*certificate.Resource, []string, error) {
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

func (m *Manager) issueWithLogsLocked(entry ManagedCertificate) (*certificate.Resource, []string, error) {
	logs := []string{
		fmt.Sprintf("Preparing ACME request for %s", entry.Domain),
		fmt.Sprintf("Challenge type: %s", entry.ChallengeType),
		fmt.Sprintf("Certificate authority: %s", entry.CA),
	}

	privateKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		logs = append(logs, fmt.Sprintf("Failed to generate private key: %v", err))
		return nil, logs, err
	}
	logs = append(logs, "Generated account key")

	user := User{
		Email: entry.Email,
		key:   privateKey,
	}

	config := lego.NewConfig(&user)
	if caURL, ok := caDirectoryURL(entry.CA); ok {
		config.CADirURL = caURL
		logs = append(logs, fmt.Sprintf("Using CA directory: %s", caURL))
	} else {
		config.CADirURL = lego.LEDirectoryProduction
		logs = append(logs, fmt.Sprintf("Unknown CA %q, falling back to Let's Encrypt", entry.CA))
	}

	client, err := lego.NewClient(config)
	if err != nil {
		logs = append(logs, fmt.Sprintf("Failed to create ACME client: %v", err))
		return nil, logs, err
	}
	logs = append(logs, "ACME client created")

	reg, err := client.Registration.Register(registration.RegisterOptions{TermsOfServiceAgreed: true})
	if err != nil {
		logs = append(logs, fmt.Sprintf("Registration failed: %v", err))
		return nil, logs, err
	}
	user.Registration = reg
	logs = append(logs, "ACME account registered")

	switch entry.ChallengeType {
	case "", "HTTP":
		challengePort := strconv.Itoa(int(entry.Port))
		if err = client.Challenge.SetHTTP01Provider(http01.NewProviderServer("", challengePort)); err != nil {
			logs = append(logs, fmt.Sprintf("Failed to configure HTTP-01 provider: %v", err))
			return nil, logs, err
		}
		logs = append(logs, fmt.Sprintf("HTTP-01 provider bound to port %s", challengePort))
	case "TLS":
		if err = client.Challenge.SetTLSALPN01Provider(tlsalpn01.NewProviderServer("", "443")); err != nil {
			logs = append(logs, fmt.Sprintf("Failed to configure TLS-ALPN-01 provider: %v", err))
			return nil, logs, err
		}
		logs = append(logs, "TLS-ALPN-01 provider bound to port 443")
	case "DNS":
		err = fmt.Errorf("DNS challenge requires provider-specific credentials and is not implemented yet")
		logs = append(logs, err.Error())
		return nil, logs, err
	default:
		err = fmt.Errorf("unsupported ACME challenge type: %s", entry.ChallengeType)
		logs = append(logs, err.Error())
		return nil, logs, err
	}

	request := certificate.ObtainRequest{
		Domains: []string{entry.Domain},
		Bundle:  true,
	}

	logs = append(logs, "Starting certificate obtain flow")
	resource, err := client.Certificate.Obtain(request)
	if err != nil {
		logs = append(logs, fmt.Sprintf("Certificate obtain failed: %v", err))
		return nil, logs, err
	}
	logs = append(logs, "Certificate obtained successfully")

	if entry.CertificatePath != "" {
		if err := os.MkdirAll(filepath.Dir(entry.CertificatePath), 0o755); err != nil {
			logs = append(logs, fmt.Sprintf("Failed to prepare certificate directory: %v", err))
			return resource, logs, err
		}
		if err := os.WriteFile(entry.CertificatePath, resource.Certificate, 0o600); err != nil {
			logs = append(logs, fmt.Sprintf("Failed to write certificate: %v", err))
			return resource, logs, err
		}
		logs = append(logs, fmt.Sprintf("Certificate written to %s", entry.CertificatePath))
	}

	if entry.KeyPath != "" {
		if err := os.MkdirAll(filepath.Dir(entry.KeyPath), 0o755); err != nil {
			logs = append(logs, fmt.Sprintf("Failed to prepare key directory: %v", err))
			return resource, logs, err
		}
		if err := os.WriteFile(entry.KeyPath, resource.PrivateKey, 0o600); err != nil {
			logs = append(logs, fmt.Sprintf("Failed to write private key: %v", err))
			return resource, logs, err
		}
		logs = append(logs, fmt.Sprintf("Private key written to %s", entry.KeyPath))
	}

	if entry.CertificatePath != "" && entry.KeyPath != "" {
		if leaf, parseErr := leafCertificate(resource.Certificate); parseErr == nil {
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
	}

	return resource, logs, nil
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

	resource, logs, err := m.IssueWithLogs(
		entry.Email,
		entry.Domain,
		entry.ChallengeType,
		entry.CA,
		entry.Port,
		entry.CertificatePath,
		entry.KeyPath,
	)

	m.mu.Lock()
	defer m.mu.Unlock()

	current, ok := m.managed[domain]
	if !ok {
		return
	}

	if err != nil {
		current.LastError = err.Error()
		current.NextRenewAt = time.Now().Add(failedRenewRetryInterval)
	} else if resource != nil {
		if leaf, parseErr := leafCertificate(resource.Certificate); parseErr == nil {
			current.LastIssuedAt = time.Now()
			current.NextRenewAt = renewalTime(leaf.NotAfter)
			current.LastError = ""
		} else {
			current.LastError = parseErr.Error()
			current.NextRenewAt = time.Now().Add(failedRenewRetryInterval)
			err = parseErr
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
		return lego.LEDirectoryProduction, true
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
