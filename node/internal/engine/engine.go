package engine

import (
	"context"
	"errors"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"proxyswarm/node/internal/acme"
	"proxyswarm/node/internal/pb"
	"strings"
	"sync"
	"time"

	"google.golang.org/protobuf/encoding/protojson"
	"google.golang.org/protobuf/proto"
)

type Engine interface {
	UpdateConfig(ctx context.Context, config *pb.InboundConfig, accounts []*pb.Account, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig, certificates []*pb.CertificateConfig) error
	GetMetrics(ctx context.Context) (*RuntimeMetrics, error)
	Stop(ctx context.Context) error
}

type Manager struct {
	mu            sync.Mutex
	engines       map[string]Engine
	acmeManager   *acme.Manager
	statePath     string
	metricsPath   string
	currentConfig *pb.FullConfig
	metricsState  *persistedMetricsState
	lastRaw       map[string]trafficTotals
}

type AcmeIssueParams struct {
	Email           string
	Domain          string
	ChallengeType   string
	CA              string
	Port            int32
	CertificatePath string
	KeyPath         string
}

var defaultDataDir = "./data"

func NewManager() *Manager {
	manager := &Manager{
		engines:      make(map[string]Engine),
		acmeManager:  acme.NewManager(),
		statePath:    defaultManagerStatePath(),
		metricsPath:  defaultManagerMetricsPath(),
		metricsState: newPersistedMetricsState(),
		lastRaw:      make(map[string]trafficTotals),
	}
	if state, err := loadPersistedMetrics(manager.metricsPath); err == nil && state != nil {
		manager.metricsState = state
	}
	go manager.runMetricsSampler()
	return manager
}

func defaultManagerStatePath() string {
	if dataDir := defaultDataRoot(); dataDir != "" {
		return filepath.Join(dataDir, "deployed_config.json")
	}
	return "deployed_config.json"
}

func defaultManagerMetricsPath() string {
	if dataDir := defaultDataRoot(); dataDir != "" {
		return filepath.Join(dataDir, "deployed_metrics.json")
	}
	return "deployed_metrics.json"
}

func defaultManagedCertsDir() string {
	if dataDir := defaultDataRoot(); dataDir != "" {
		return filepath.Join(dataDir, "certs")
	}
	return "certs"
}

func defaultDataRoot() string {
	if envDir := strings.TrimSpace(os.Getenv("PS_NODE_DATA_DIR")); envDir != "" {
		return envDir
	}
	if buildDir := strings.TrimSpace(defaultDataDir); buildDir != "" {
		return buildDir
	}
	return ""
}

func materializeInlineCertificate(cert *pb.CertificateConfig) error {
	if cert == nil {
		return nil
	}
	if strings.TrimSpace(cert.CertificatePem) == "" && strings.TrimSpace(cert.KeyPem) == "" {
		return nil
	}
	dir := filepath.Join(defaultManagedCertsDir(), cert.Id)
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

func inboundTLSConfig(inbound *pb.InboundConfig) *pb.TLSConfig {
	if inbound == nil {
		return nil
	}
	switch p := inbound.Protocol.(type) {
	case *pb.InboundConfig_Vless:
		if p.Vless != nil {
			return p.Vless.Tls
		}
	case *pb.InboundConfig_Hysteria2:
		if p.Hysteria2 != nil {
			return p.Hysteria2.Tls
		}
	case *pb.InboundConfig_Trusttunnel:
		if p.Trusttunnel != nil {
			return p.Trusttunnel.Tls
		}
	case *pb.InboundConfig_Naiveproxy:
		if p.Naiveproxy != nil {
			return p.Naiveproxy.Tls
		}
	}
	return nil
}

func findCertificateByName(certificates []*pb.CertificateConfig, name string) *pb.CertificateConfig {
	needle := strings.TrimSpace(name)
	if needle == "" {
		return nil
	}
	for _, cert := range certificates {
		if cert == nil {
			continue
		}
		if strings.TrimSpace(cert.Name) == needle {
			return cert
		}
	}
	return nil
}

func resolveInboundTLSCertificate(tlsConfig *pb.TLSConfig, certificates []*pb.CertificateConfig) (*pb.CertificateConfig, error) {
	if tlsConfig == nil || !tlsConfig.Enabled {
		return nil, nil
	}
	certificateName := strings.TrimSpace(tlsConfig.CertificateName)
	if certificateName == "" {
		return nil, nil
	}
	cert := findCertificateByName(certificates, certificateName)
	if cert == nil {
		return nil, fmt.Errorf("tls certificate %q not found", certificateName)
	}
	return cert, nil
}

func coalesceString(value string, fallback string) string {
	if strings.TrimSpace(value) == "" {
		return fallback
	}
	return value
}

func (m *Manager) Update(ctx context.Context, config *pb.FullConfig) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	now := time.Now().Unix()
	var activeAccounts []*pb.Account
	for _, acc := range config.Accounts {
		if acc == nil {
			continue
		}
		if acc.ExpiryTime == 0 || acc.ExpiryTime > now {
			activeAccounts = append(activeAccounts, acc)
		}
	}

	newInboundNames := make(map[string]bool)
	for _, inbound := range config.Inbounds {
		if inbound == nil {
			continue
		}
		if !inbound.Enabled {
			continue
		}
		newInboundNames[inbound.Name] = true

		for _, cert := range config.Certificates {
			if cert == nil || strings.ToUpper(strings.TrimSpace(cert.CertType)) != "ACME" {
				if err := materializeInlineCertificate(cert); err != nil {
					return fmt.Errorf("failed to materialize certificate %s: %w", cert.Name, err)
				}
				continue
			}
			logs := m.acmeManager.EnsureManagedCertificate(
				cert.AcmeEmail,
				cert.AcmeDomain,
				coalesceString(cert.AcmeType, "HTTP"),
				coalesceString(cert.AcmeCa, "letsencrypt"),
				cert.AcmePort,
				cert.CertificatePath,
				cert.KeyPath,
			)
			for _, line := range logs {
				fmt.Printf("[acme][%s] %s\n", cert.AcmeDomain, line)
			}
		}

		var e Engine
		var exists bool
		if e, exists = m.engines[inbound.Name]; exists && !engineMatchesInbound(e, inbound) {
			_ = e.Stop(ctx)
			delete(m.engines, inbound.Name)
			exists = false
		}
		if !exists {
			e = newEngineForInbound(inbound)
			if e == nil {
				return fmt.Errorf("no engine available for inbound %s", inbound.Name)
			}
			m.engines[inbound.Name] = e
		}
		log.Printf("[engine] inbound=%s core=%s engine=%T", inbound.Name, inbound.Core.String(), e)

		inboundAccounts := inbound.GetAccounts()
		if len(inboundAccounts) == 0 {
			inboundAccounts = activeAccounts
		}

		if err := e.UpdateConfig(ctx, inbound, inboundAccounts, config.Outbounds, config.RoutingRules, config.Dns, config.Certificates); err != nil {
			return fmt.Errorf("failed to update inbound %s: %w", inbound.Name, err)
		}
	}

	// Remove old inbounds
	for name, engine := range m.engines {
		if !newInboundNames[name] {
			engine.Stop(ctx)
			delete(m.engines, name)
		}
	}

	if err := m.savePersistedConfig(config); err != nil {
		return fmt.Errorf("failed to persist deployed config: %w", err)
	}
	if cloned, ok := proto.Clone(config).(*pb.FullConfig); ok {
		m.currentConfig = cloned
	}

	return nil
}

func (m *Manager) RestoreLastConfig(ctx context.Context) (bool, error) {
	config, err := m.loadPersistedConfig()
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return false, nil
		}
		return false, err
	}
	if config == nil {
		return false, nil
	}
	if err := m.Update(ctx, config); err != nil {
		return false, err
	}
	return true, nil
}

func (m *Manager) GetInboundStatus(ctx context.Context) []*pb.InboundStatus {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.inboundStatusesLocked()
}

func (m *Manager) GetAccountStatus(ctx context.Context) []*pb.AccountStatus {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.accountStatusesLocked()
}

func (m *Manager) GetOutboundStatus(ctx context.Context) []*pb.OutboundStatus {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.outboundStatusesLocked()
}

func (m *Manager) GetTotalInboundTraffic(ctx context.Context) *pb.TrafficStats {
	m.mu.Lock()
	defer m.mu.Unlock()
	return cloneTrafficStats(m.metricsState.TotalInboundTraffic)
}

func (m *Manager) GetTotalOutboundTraffic(ctx context.Context) *pb.TrafficStats {
	m.mu.Lock()
	defer m.mu.Unlock()
	return cloneTrafficStats(m.metricsState.TotalOutboundTraffic)
}

func (m *Manager) GetConnectionStats(ctx context.Context) *pb.ConnectionStats {
	m.mu.Lock()
	defer m.mu.Unlock()
	return cloneConnectionStats(m.metricsState.Connections)
}

func (m *Manager) GetSampleWindowSeconds() uint32 {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.metricsState.SampleWindowSeconds
}

func newEngineForInbound(inbound *pb.InboundConfig) Engine {
	if _, ok := inbound.Protocol.(*pb.InboundConfig_Trusttunnel); ok {
		return NewTrustTunnelEngine(inbound.Name)
	}
	if _, ok := inbound.Protocol.(*pb.InboundConfig_Wireguard); ok {
		return NewXrayEngine(inbound.Name)
	}
	switch inbound.Core {
	case pb.CoreType_SING_BOX:
		return NewSingBoxEngine(inbound.Name)
	case pb.CoreType_XRAY:
		return NewXrayEngine(inbound.Name)
	default:
		return nil
	}
}

func engineMatchesInbound(e Engine, inbound *pb.InboundConfig) bool {
	switch e.(type) {
	case *TrustTunnelEngine:
		_, ok := inbound.Protocol.(*pb.InboundConfig_Trusttunnel)
		return ok
	case *XrayEngine:
		if _, ok := inbound.Protocol.(*pb.InboundConfig_Wireguard); ok {
			return true
		}
		return inbound.Core == pb.CoreType_XRAY
	case *SingBoxEngine:
		if _, ok := inbound.Protocol.(*pb.InboundConfig_Wireguard); ok {
			return false
		}
		if _, ok := inbound.Protocol.(*pb.InboundConfig_Trusttunnel); ok {
			return false
		}
		return inbound.Core == pb.CoreType_SING_BOX
	default:
		return false
	}
}

func (m *Manager) IssueAcmeCertificate(ctx context.Context, params AcmeIssueParams) ([]string, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	_, logs, err := m.acmeManager.IssueWithLogs(
		params.Email,
		params.Domain,
		params.ChallengeType,
		params.CA,
		params.Port,
		params.CertificatePath,
		params.KeyPath,
	)
	return logs, err
}

func (m *Manager) savePersistedConfig(config *pb.FullConfig) error {
	if config == nil {
		err := os.Remove(m.statePath)
		if err != nil && !errors.Is(err, os.ErrNotExist) {
			return err
		}
		return nil
	}

	configCopy, ok := proto.Clone(config).(*pb.FullConfig)
	if !ok {
		return fmt.Errorf("failed to clone deployed config")
	}
	configCopy.MasterKey = ""

	data, err := protojson.MarshalOptions{
		Multiline:       true,
		Indent:          "  ",
		EmitUnpopulated: true,
	}.Marshal(configCopy)
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

func (m *Manager) loadPersistedConfig() (*pb.FullConfig, error) {
	data, err := os.ReadFile(m.statePath)
	if err != nil {
		return nil, err
	}

	config := &pb.FullConfig{}
	if err := (protojson.UnmarshalOptions{
		DiscardUnknown: true,
	}).Unmarshal(data, config); err != nil {
		return nil, err
	}
	return config, nil
}
