package engine

import (
	"context"
	"fmt"
	"hash/fnv"
	"os"
	"path/filepath"
	"proxyswarm/node/internal/pb"
	"sort"
	"strconv"
	"strings"
	"sync"
)

type trustTunnelOutboundProcess struct {
	supervisor *supervisedProcess
	tmpDir     string
	signature  string
}

type trustTunnelOutboundManager struct {
	mu        sync.Mutex
	processes map[string]*trustTunnelOutboundProcess
}

func newTrustTunnelOutboundManager() *trustTunnelOutboundManager {
	return &trustTunnelOutboundManager{processes: make(map[string]*trustTunnelOutboundProcess)}
}

func trustTunnelOutboundSocksPort(tag string) int {
	h := fnv.New32a()
	_, _ = h.Write([]byte(strings.ToLower(strings.TrimSpace(tag))))
	return 20000 + int(h.Sum32()%20000)
}

func (m *trustTunnelOutboundManager) sync(ctx context.Context, outbounds []*pb.OutboundConfig) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	desired := make(map[string]*pb.TrustTunnelConfig)
	for _, out := range outbounds {
		if out == nil || out.GetType() != pb.OutboundType_TRUSTTUNNEL {
			continue
		}
		tag := strings.TrimSpace(out.GetTag())
		cfg := out.GetTrusttunnel()
		if tag == "" || cfg == nil {
			continue
		}
		desired[tag] = cfg
	}

	tags := make([]string, 0, len(desired))
	for tag := range desired {
		tags = append(tags, tag)
	}
	sort.Strings(tags)

	for _, tag := range tags {
		cfg := desired[tag]
		sig := trustTunnelOutboundSignature(cfg)
		if existing, ok := m.processes[tag]; ok && existing != nil && existing.signature == sig {
			continue
		}
		started, err := startTrustTunnelOutboundProcess(ctx, tag, cfg)
		if err != nil {
			return err
		}
		if old := m.processes[tag]; old != nil {
			stopTrustTunnelOutboundProcess(old)
		}
		m.processes[tag] = started
	}

	for tag, process := range m.processes {
		if _, ok := desired[tag]; ok {
			continue
		}
		stopTrustTunnelOutboundProcess(process)
		delete(m.processes, tag)
	}

	return nil
}

func (m *trustTunnelOutboundManager) stopAll() {
	m.mu.Lock()
	defer m.mu.Unlock()
	for tag, process := range m.processes {
		stopTrustTunnelOutboundProcess(process)
		delete(m.processes, tag)
	}
}

func startTrustTunnelOutboundProcess(ctx context.Context, tag string, cfg *pb.TrustTunnelConfig) (*trustTunnelOutboundProcess, error) {
	if cfg == nil {
		return nil, fmt.Errorf("trusttunnel outbound %q config is nil", tag)
	}
	hostname := strings.TrimSpace(cfg.GetEndpointHostname())
	if hostname == "" {
		return nil, fmt.Errorf("trusttunnel outbound %q requires endpoint_hostname", tag)
	}
	addresses := make([]string, 0, len(cfg.GetEndpointAddresses()))
	for _, value := range cfg.GetEndpointAddresses() {
		value = strings.TrimSpace(value)
		if value != "" {
			addresses = append(addresses, value)
		}
	}
	if len(addresses) == 0 {
		return nil, fmt.Errorf("trusttunnel outbound %q requires endpoint_addresses", tag)
	}
	username := strings.TrimSpace(cfg.GetUsername())
	password := strings.TrimSpace(cfg.GetPassword())
	if username == "" || password == "" {
		return nil, fmt.Errorf("trusttunnel outbound %q requires username and password", tag)
	}
	protocol := strings.ToLower(strings.TrimSpace(cfg.GetUpstreamProtocol()))
	if protocol != "http3" {
		protocol = "http2"
	}

	tmpDir, err := os.MkdirTemp("", "trusttunnel-outbound-"+sanitizeTrustTunnelTag(tag)+"-")
	if err != nil {
		return nil, err
	}

	configPath := filepath.Join(tmpDir, "trusttunnel_client.toml")
	port := trustTunnelOutboundSocksPort(tag)
	if err := os.WriteFile(configPath, []byte(buildTrustTunnelClientConfig(hostname, addresses, username, password, cfg, protocol, port)), 0o600); err != nil {
		_ = os.RemoveAll(tmpDir)
		return nil, fmt.Errorf("failed to write trusttunnel client config: %w", err)
	}

	process := &trustTunnelOutboundProcess{
		supervisor: newSupervisedProcess("trusttunnel-outbound-" + tag),
		tmpDir:     tmpDir,
		signature:  trustTunnelOutboundSignature(cfg),
	}
	if err := process.supervisor.restart(ctx, []string{TrustTunnelClientBinary, "--config", configPath}); err != nil {
		stopTrustTunnelOutboundProcess(process)
		return nil, fmt.Errorf("failed to start trusttunnel outbound %q: %w", tag, err)
	}
	return process, nil
}

func stopTrustTunnelOutboundProcess(process *trustTunnelOutboundProcess) {
	if process == nil {
		return
	}
	if process.supervisor != nil {
		process.supervisor.stop()
	}
	if process.tmpDir != "" {
		_ = os.RemoveAll(process.tmpDir)
	}
}

func buildTrustTunnelClientConfig(hostname string, addresses []string, username, password string, cfg *pb.TrustTunnelConfig, protocol string, socksPort int) string {
	var b strings.Builder
	b.WriteString("loglevel = \"info\"\n")
	b.WriteString("vpn_mode = \"selective\"\n")
	b.WriteString("killswitch_enabled = false\n")
	b.WriteString("post_quantum_group_enabled = false\n")
	b.WriteString("exclusions = []\n\n")
	b.WriteString("[endpoint]\n")
	b.WriteString("hostname = " + tomlQuote(hostname) + "\n")
	b.WriteString("addresses = [")
	for i, address := range addresses {
		if i > 0 {
			b.WriteString(", ")
		}
		b.WriteString(tomlQuote(address))
	}
	b.WriteString("]\n")
	b.WriteString("has_ipv6 = true\n")
	b.WriteString("username = " + tomlQuote(username) + "\n")
	b.WriteString("password = " + tomlQuote(password) + "\n")
	b.WriteString("client_random = \"\"\n")
	b.WriteString("skip_verification = " + strconv.FormatBool(cfg.GetSkipVerification()) + "\n")
	b.WriteString("certificate = " + tomlQuote(strings.TrimSpace(cfg.GetCertificatePem())) + "\n")
	b.WriteString("upstream_protocol = " + tomlQuote(protocol) + "\n")
	b.WriteString("anti_dpi = " + strconv.FormatBool(cfg.GetAntiDpi()) + "\n")
	b.WriteString("custom_sni = " + tomlQuote(strings.TrimSpace(cfg.GetCustomSni())) + "\n\n")
	b.WriteString("[listener.socks]\n")
	b.WriteString("address = " + tomlQuote(fmt.Sprintf("127.0.0.1:%d", socksPort)) + "\n")
	b.WriteString("username = \"\"\n")
	b.WriteString("password = \"\"\n\n")
	b.WriteString("[listener.tun]\n")
	b.WriteString("included_routes = []\n")
	b.WriteString("change_system_dns = false\n")
	b.WriteString("mtu_size = 1280\n")
	return b.String()
}

func trustTunnelOutboundSignature(cfg *pb.TrustTunnelConfig) string {
	if cfg == nil {
		return ""
	}
	return strings.Join([]string{
		strings.TrimSpace(cfg.GetEndpointHostname()),
		strings.Join(cfg.GetEndpointAddresses(), ","),
		strings.TrimSpace(cfg.GetUsername()),
		strings.TrimSpace(cfg.GetPassword()),
		strings.TrimSpace(cfg.GetCertificatePem()),
		strconv.FormatBool(cfg.GetSkipVerification()),
		strings.ToLower(strings.TrimSpace(cfg.GetUpstreamProtocol())),
		strconv.FormatBool(cfg.GetAntiDpi()),
		strings.TrimSpace(cfg.GetCustomSni()),
	}, "|")
}

func sanitizeTrustTunnelTag(tag string) string {
	tag = strings.TrimSpace(strings.ToLower(tag))
	if tag == "" {
		return "default"
	}
	var b strings.Builder
	for _, r := range tag {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') || r == '-' || r == '_' {
			b.WriteRune(r)
		} else {
			b.WriteByte('-')
		}
	}
	return strings.Trim(b.String(), "-")
}

func tomlQuote(value string) string {
	return strconv.Quote(value)
}
