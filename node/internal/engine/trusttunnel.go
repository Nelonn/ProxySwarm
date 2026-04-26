package engine

import (
	"context"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"proxyswarm/node/internal/logging"
	"proxyswarm/node/internal/pb"
	"strings"
	"sync"
	"sync/atomic"
)

var TrustTunnelBinary = "trusttunnel_endpoint"

type TrustTunnelEngine struct {
	mu     sync.Mutex
	cmd    *exec.Cmd
	name   string
	tmpDir string
	rx     uint64
	tx     uint64
}

func withDefaultUint32(value uint32, fallback uint32) uint32 {
	if value == 0 {
		return fallback
	}
	return value
}

func NewTrustTunnelEngine(name string) *TrustTunnelEngine {
	return &TrustTunnelEngine{name: name}
}

func (e *TrustTunnelEngine) UpdateConfig(ctx context.Context, inbounds []*pb.InboundConfig, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig, certificates []*pb.CertificateConfig) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	_ = outbounds
	_ = rules
	_ = dns
	if len(inbounds) != 1 || inbounds[0] == nil {
		return fmt.Errorf("trusttunnel engine requires exactly one inbound")
	}
	config := inbounds[0]
	accounts := config.GetAccounts()

	if e.cmd != nil && e.cmd.Process != nil {
		e.cmd.Process.Kill()
	}

	tmpDir, err := os.MkdirTemp("", "trusttunnel-"+e.name)
	if err != nil {
		return err
	}
	e.tmpDir = tmpDir

	tt := config.GetTrusttunnel()
	if tt == nil {
		return fmt.Errorf("trusttunnel protocol config is required")
	}
	tlsConfig := inboundTLSConfig(config)
	if tlsConfig == nil {
		return fmt.Errorf("tls config is required for trusttunnel")
	}
	cert, err := resolveInboundTLSCertificate(tlsConfig, certificates)
	if err != nil {
		return err
	}
	if cert == nil || strings.TrimSpace(cert.CertificatePath) == "" || strings.TrimSpace(cert.KeyPath) == "" {
		return fmt.Errorf("trusttunnel requires tls certificate_name with valid certificate_path and key_path")
	}

	hostName := strings.TrimSpace(tlsConfig.ServerName)
	if hostName == "" {
		hostName = strings.TrimSpace(config.Listen)
	}
	if hostName == "" {
		hostName = "localhost"
	}

	credsPath := filepath.Join(tmpDir, "credentials.toml")
	var credsBuilder strings.Builder
	for _, acc := range accounts {
		username := strings.TrimSpace(acc.Name)
		if username == "" {
			username = strings.TrimSpace(acc.Id)
		}
		if username == "" {
			continue
		}

		password := strings.TrimSpace(acc.Token)
		if password == "" {
			password = strings.TrimSpace(acc.Id)
		}
		if password == "" {
			continue
		}

		credsBuilder.WriteString("[[client]]\n")
		credsBuilder.WriteString(fmt.Sprintf("username = %q\n", username))
		credsBuilder.WriteString(fmt.Sprintf("password = %q\n\n", password))
	}
	if credsBuilder.Len() == 0 {
		credsBuilder.WriteString("[[client]]\n")
		credsBuilder.WriteString(fmt.Sprintf("username = %q\n", "default"))
		credsBuilder.WriteString(fmt.Sprintf("password = %q\n", "change-me"))
	}
	if err := os.WriteFile(credsPath, []byte(credsBuilder.String()), 0644); err != nil {
		return fmt.Errorf("failed to write trusttunnel credentials.toml: %w", err)
	}

	hostsPath := filepath.Join(tmpDir, "hosts.toml")
	hostsContent := fmt.Sprintf(
		"[[main_hosts]]\nhostname = %q\ncert_chain_path = %q\nprivate_key_path = %q\n",
		hostName,
		cert.CertificatePath,
		cert.KeyPath,
	)
	if err := os.WriteFile(hostsPath, []byte(hostsContent), 0644); err != nil {
		return fmt.Errorf("failed to write trusttunnel hosts.toml: %w", err)
	}

	var rulesPath string
	var rulesBuilder strings.Builder
	for _, r := range rules {
		if r == nil || strings.TrimSpace(r.OutboundTag) == "" {
			continue
		}

		action := "allow"
		switch strings.ToLower(strings.TrimSpace(r.OutboundTag)) {
		case "block", "deny":
			action = "deny"
		}

		cidrs := r.Ip
		if len(cidrs) == 0 {
			rulesBuilder.WriteString("[[rule]]\n")
			rulesBuilder.WriteString(fmt.Sprintf("action = %q\n\n", action))
			continue
		}
		for _, cidr := range cidrs {
			cidr = strings.TrimSpace(cidr)
			if cidr == "" {
				continue
			}
			rulesBuilder.WriteString("[[rule]]\n")
			rulesBuilder.WriteString(fmt.Sprintf("cidr = %q\n", cidr))
			rulesBuilder.WriteString(fmt.Sprintf("action = %q\n\n", action))
		}
	}
	if rulesBuilder.Len() > 0 {
		rulesPath = filepath.Join(tmpDir, "rules.toml")
		if err := os.WriteFile(rulesPath, []byte(rulesBuilder.String()), 0644); err != nil {
			return fmt.Errorf("failed to write trusttunnel rules.toml: %w", err)
		}
	}

	vpnPath := filepath.Join(tmpDir, "vpn.toml")
	var vpnBuilder strings.Builder
	vpnBuilder.WriteString(fmt.Sprintf("listen_address = %q\n", fmt.Sprintf("%s:%d", config.Listen, config.Port)))
	vpnBuilder.WriteString("ipv6_available = true\n")
	vpnBuilder.WriteString("allow_private_network_connections = false\n")
	vpnBuilder.WriteString("tls_handshake_timeout_secs = 10\n")
	vpnBuilder.WriteString("client_listener_timeout_secs = 600\n")
	vpnBuilder.WriteString("connection_establishment_timeout_secs = 30\n")
	vpnBuilder.WriteString("tcp_connections_timeout_secs = 604800\n")
	vpnBuilder.WriteString("udp_connections_timeout_secs = 300\n")
	vpnBuilder.WriteString(fmt.Sprintf("credentials_file = %q\n", credsPath))
	if rulesPath != "" {
		vpnBuilder.WriteString(fmt.Sprintf("rules_file = %q\n", rulesPath))
	}
	vpnBuilder.WriteString("\n[listen_protocols]\n\n")
	vpnBuilder.WriteString("[listen_protocols.http1]\n")
	vpnBuilder.WriteString(fmt.Sprintf("upload_buffer_size = %d\n\n", withDefaultUint32(tt.Http1UploadBufferSize, 32768)))
	vpnBuilder.WriteString("[listen_protocols.http2]\n")
	vpnBuilder.WriteString(fmt.Sprintf("initial_connection_window_size = %d\n", withDefaultUint32(tt.Http2InitialConnectionWindowSize, 8388608)))
	vpnBuilder.WriteString(fmt.Sprintf("initial_stream_window_size = %d\n", withDefaultUint32(tt.Http2InitialStreamWindowSize, 131072)))
	vpnBuilder.WriteString(fmt.Sprintf("max_concurrent_streams = %d\n", withDefaultUint32(tt.Http2MaxConcurrentStreams, 1000)))
	vpnBuilder.WriteString(fmt.Sprintf("max_frame_size = %d\n", withDefaultUint32(tt.Http2MaxFrameSize, 16384)))
	vpnBuilder.WriteString(fmt.Sprintf("header_table_size = %d\n\n", withDefaultUint32(tt.Http2HeaderTableSize, 65536)))
	vpnBuilder.WriteString("[forward_protocol]\n")
	vpnBuilder.WriteString("direct = {}\n")

	if err := os.WriteFile(vpnPath, []byte(vpnBuilder.String()), 0644); err != nil {
		return fmt.Errorf("failed to write trusttunnel vpn.toml: %w", err)
	}

	logging.Debugf("[trusttunnel] generated hosts.toml=%s", hostsContent)
	logging.Debugf("[trusttunnel] generated credentials.toml=%s", credsBuilder.String())
	if rulesBuilder.Len() > 0 {
		logging.Debugf("[trusttunnel] generated rules.toml=%s", rulesBuilder.String())
	}
	logging.Debugf("[trusttunnel] generated vpn.toml=%s", vpnBuilder.String())

	cmd := exec.CommandContext(ctx, TrustTunnelBinary, vpnPath, hostsPath)
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to start trusttunnel_endpoint: %w", err)
	}

	e.cmd = cmd
	return nil
}

func (e *TrustTunnelEngine) GetMetrics(ctx context.Context) (*RuntimeMetrics, error) {
	return &RuntimeMetrics{
		Inbounds: []*pb.InboundStatus{
			{
				Name: e.name,
				Traffic: &pb.TrafficStats{
					Rx: atomic.LoadUint64(&e.rx),
					Tx: atomic.LoadUint64(&e.tx),
				},
				Connections: &pb.ConnectionStats{},
			},
		},
	}, nil
}

func (e *TrustTunnelEngine) Stop(ctx context.Context) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.cmd != nil && e.cmd.Process != nil {
		e.cmd.Process.Kill()
	}
	if e.tmpDir != "" {
		os.RemoveAll(e.tmpDir)
	}
	return nil
}

type CounterWriter struct {
	io.Writer
	count *uint64
}

func (cw *CounterWriter) Write(p []byte) (int, error) {
	n, err := cw.Writer.Write(p)
	atomic.AddUint64(cw.count, uint64(n))
	return n, err
}
