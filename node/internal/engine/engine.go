package engine

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"proxyswarm/node/internal/acme"
	"proxyswarm/node/internal/logging"
	"proxyswarm/node/internal/pb"
	"strings"
	"sync"
	"time"

	"google.golang.org/protobuf/encoding/protojson"
	"google.golang.org/protobuf/proto"
)

type Engine interface {
	UpdateConfig(ctx context.Context, inbounds []*pb.InboundConfig, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig, certificates []*pb.CertificateConfig) error
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

type AcmeIssueResult struct {
	Logs            []string
	CertificatePath string
	KeyPath         string
	ExpiryTime      time.Time
}

const sharedXrayEngineKey = "xray"

var defaultDataDir = "./data"

func NewManager() *Manager {
	dataRoot := defaultDataRoot()
	manager := &Manager{
		engines:      make(map[string]Engine),
		acmeManager:  acme.NewManager(dataRoot),
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

func acmeChallengePort(challengeType string, tlsPort, httpPort int32) int32 {
	switch strings.ToUpper(strings.TrimSpace(challengeType)) {
	case "TLS":
		if tlsPort > 0 {
			return tlsPort
		}
		return 443
	case "DNS":
		return 0
	default:
		if httpPort > 0 {
			return httpPort
		}
		if tlsPort > 0 {
			return tlsPort
		}
		return 80
	}
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

	newEngineKeys := make(map[string]bool)
	groupedInbounds := make(map[string][]*pb.InboundConfig)
	for _, inbound := range config.Inbounds {
		if inbound == nil {
			continue
		}
		if !inbound.Enabled {
			continue
		}
		engineKey := engineKeyForInbound(inbound)
		if engineKey == "" {
			return fmt.Errorf("no engine available for inbound %s", inbound.Name)
		}
		newEngineKeys[engineKey] = true
		groupedInbounds[engineKey] = append(groupedInbounds[engineKey], preparedInboundConfig(inbound, activeAccounts))
	}

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
			acmeChallengePort(cert.AcmeType, cert.AcmePort, cert.AcmeHttpPort),
			cert.CertificatePath,
			cert.KeyPath,
		)
		for _, line := range logs {
			fmt.Printf("[acme][%s] %s\n", cert.AcmeDomain, line)
		}
	}

	for engineKey, inbounds := range groupedInbounds {
		var e Engine
		var exists bool
		if e, exists = m.engines[engineKey]; exists && !engineMatchesInbounds(e, inbounds) {
			_ = e.Stop(ctx)
			delete(m.engines, engineKey)
			exists = false
		}
		if !exists {
			e = newEngineForInbounds(inbounds)
			if e == nil {
				return fmt.Errorf("no engine available for group %s", engineKey)
			}
			m.engines[engineKey] = e
		}
		inboundNames := make([]string, 0, len(inbounds))
		for _, inbound := range inbounds {
			inboundNames = append(inboundNames, inbound.GetName())
		}
		logging.Infof("[engine] key=%s inbounds=%s engine=%T", engineKey, strings.Join(inboundNames, ","), e)

		if err := e.UpdateConfig(ctx, inbounds, config.Outbounds, config.RoutingRules, config.Dns, config.Certificates); err != nil {
			return fmt.Errorf("failed to update engine %s: %w", engineKey, err)
		}
	}

	for name, engine := range m.engines {
		if !newEngineKeys[name] {
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

func preparedInboundConfig(inbound *pb.InboundConfig, activeAccounts []*pb.Account) *pb.InboundConfig {
	cloned := cloneProtoMessage(inbound)
	if cloned == nil {
		return nil
	}
	if len(cloned.GetAccounts()) == 0 {
		cloned.Accounts = cloneProtoSlice(activeAccounts)
	}
	return cloned
}

func engineKeyForInbound(inbound *pb.InboundConfig) string {
	if inbound == nil {
		return ""
	}
	if _, ok := inbound.Protocol.(*pb.InboundConfig_Trusttunnel); ok {
		return "trusttunnel:" + inbound.Name
	}
	if _, ok := inbound.Protocol.(*pb.InboundConfig_Wireguard); ok {
		return sharedXrayEngineKey
	}
	switch inbound.Core {
	case pb.CoreType_SING_BOX:
		return "singbox:" + inbound.Name
	case pb.CoreType_XRAY:
		return sharedXrayEngineKey
	default:
		return ""
	}
}

func newEngineForInbounds(inbounds []*pb.InboundConfig) Engine {
	if len(inbounds) == 0 || inbounds[0] == nil {
		return nil
	}
	inbound := inbounds[0]
	if _, ok := inbound.Protocol.(*pb.InboundConfig_Trusttunnel); ok {
		return NewTrustTunnelEngine(inbound.Name)
	}
	if _, ok := inbound.Protocol.(*pb.InboundConfig_Wireguard); ok {
		return NewXrayEngine(sharedXrayEngineKey)
	}
	switch inbound.Core {
	case pb.CoreType_SING_BOX:
		return NewSingBoxEngine(inbound.Name)
	case pb.CoreType_XRAY:
		return NewXrayEngine(sharedXrayEngineKey)
	default:
		return nil
	}
}

func engineMatchesInbounds(e Engine, inbounds []*pb.InboundConfig) bool {
	if len(inbounds) == 0 {
		return false
	}
	for _, inbound := range inbounds {
		if inbound == nil {
			return false
		}
		if !engineMatchesInbound(e, inbound) {
			return false
		}
	}
	return true
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

func (m *Manager) IssueAcmeCertificate(ctx context.Context, params AcmeIssueParams) (*AcmeIssueResult, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	result, err := m.acmeManager.IssueWithLogs(
		params.Email,
		params.Domain,
		params.ChallengeType,
		params.CA,
		params.Port,
		params.CertificatePath,
		params.KeyPath,
	)
	if result == nil {
		return &AcmeIssueResult{}, err
	}
	return &AcmeIssueResult{
		Logs:            result.Logs,
		CertificatePath: result.CertificatePath,
		KeyPath:         result.KeyPath,
		ExpiryTime:      result.ExpiryTime,
	}, err
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
