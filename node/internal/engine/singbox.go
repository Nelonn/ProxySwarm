package engine

import (
	"context"
	"encoding/json"
	"fmt"
	"proxyswarm/node/internal/pb"
	"net"
	"net/netip"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
)

var SingBoxBinary = "sing-box"

func normalizedSingBoxRuleValues(values []string) []string {
	normalized := make([]string, 0, len(values))
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value == "" {
			continue
		}
		normalized = append(normalized, value)
	}
	if len(normalized) == 0 {
		return nil
	}
	return normalized
}

func normalizedSingBoxRuleTransports(values []string) []string {
	normalized := normalizedSingBoxRuleValues(values)
	if len(normalized) == 0 {
		return nil
	}
	transports := make([]string, 0, len(normalized))
	seen := make(map[string]struct{}, len(normalized))
	for _, value := range normalized {
		for _, part := range strings.Split(value, ",") {
			token := strings.ToLower(strings.TrimSpace(part))
			if token != "tcp" && token != "udp" {
				continue
			}
			if _, ok := seen[token]; ok {
				continue
			}
			seen[token] = struct{}{}
			transports = append(transports, token)
		}
	}
	if len(transports) == 0 {
		return nil
	}
	return transports
}

func singVlessTransport(transmission string) map[string]any {
	var t string
	switch transmission {
	case "HTTP":
		t = "http"
	case "gRPC":
		t = "grpc"
	case "WebSocket":
		t = "ws"
	case "HttpUpgrade":
		t = "httpupgrade"
	default:
		return nil
	}
	return map[string]any{"type": t}
}

func parseSingBoxMasquerade(value string) (any, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return nil, nil
	}
	if strings.HasPrefix(value, "{") || strings.HasPrefix(value, "[") {
		var parsed any
		if err := json.Unmarshal([]byte(value), &parsed); err != nil {
			return nil, fmt.Errorf("invalid hysteria2 masquerade JSON: %w", err)
		}
		return parsed, nil
	}
	return value, nil
}

type SingBoxEngine struct {
	mu     sync.Mutex
	cmd    *exec.Cmd
	name   string
	tmpDir string
	rx     uint64
	tx     uint64
}

func NewSingBoxEngine(name string) *SingBoxEngine {
	return &SingBoxEngine{name: name}
}

func (e *SingBoxEngine) UpdateConfig(ctx context.Context, config *pb.InboundConfig, accounts []*pb.Account, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig, certificates []*pb.CertificateConfig) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	_ = dns

	cfg, err := e.convertToConfig(config, accounts, outbounds, rules, certificates)
	if err != nil {
		return fmt.Errorf("failed to convert config: %w", err)
	}

	configBytes, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal sing-box config: %w", err)
	}

	if e.cmd != nil && e.cmd.Process != nil {
		_ = e.cmd.Process.Kill()
		e.cmd = nil
	}
	if e.tmpDir != "" {
		_ = os.RemoveAll(e.tmpDir)
		e.tmpDir = ""
	}

	tmpDir, err := os.MkdirTemp("", "singbox-"+e.name)
	if err != nil {
		return fmt.Errorf("failed to create temp dir for sing-box: %w", err)
	}
	e.tmpDir = tmpDir

	configPath := filepath.Join(tmpDir, "config.json")
	if err := os.WriteFile(configPath, configBytes, 0644); err != nil {
		return fmt.Errorf("failed to write sing-box config: %w", err)
	}

	cmd := exec.Command(SingBoxBinary, "run", "-c", configPath)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to start sing-box subprocess: %w", err)
	}

	e.cmd = cmd
	return nil
}

func (e *SingBoxEngine) convertToConfig(config *pb.InboundConfig, accounts []*pb.Account, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, certificates []*pb.CertificateConfig) (map[string]any, error) {
	singCfg := map[string]any{
		"inbounds":  []map[string]any{},
		"outbounds": []map[string]any{},
		"route": map[string]any{
			"rules": []map[string]any{},
		},
		"experimental": map[string]any{
			"v2ray_api": map[string]any{
				"stats": map[string]any{
					"enabled":  true,
					"inbounds": []string{config.Name},
				},
			},
		},
	}

	addr, err := netip.ParseAddr(config.Listen)
	if err != nil {
		return nil, fmt.Errorf("invalid listen address: %w", err)
	}

	inbound := map[string]any{
		"tag":         config.Name,
		"listen":      addr.String(),
		"listen_port": config.Port,
	}

	tlsConfig := inboundTLSConfig(config)
	tlsCertificate, err := resolveInboundTLSCertificate(tlsConfig, certificates)
	if err != nil {
		return nil, err
	}
	var tlsOptions map[string]any
	if tlsConfig != nil && tlsConfig.Enabled {
		tlsOptions = map[string]any{
			"enabled": true,
		}
		if tlsConfig.ServerName != "" {
			tlsOptions["server_name"] = tlsConfig.ServerName
		}
		if tlsCertificate != nil && tlsCertificate.CertificatePath != "" && tlsCertificate.KeyPath != "" {
			tlsOptions["certificate_path"] = tlsCertificate.CertificatePath
			tlsOptions["key_path"] = tlsCertificate.KeyPath
		}

		switch p := config.Protocol.(type) {
		case *pb.InboundConfig_Vless:
			if p.Vless.Security == pb.SecurityMode_REALITY {
				realityCfg := p.Vless.Reality
				if realityCfg == nil {
					break
				}
				reality := map[string]any{
					"enabled": true,
					"handshake": map[string]any{
						"server":      realityCfg.Dest,
						"server_port": 443,
					},
					"private_key": realityCfg.PrivateKey,
				}
				if len(realityCfg.ShortId) > 0 {
					reality["short_id"] = realityCfg.ShortId
				}
				host, portStr, splitErr := net.SplitHostPort(realityCfg.Dest)
				if splitErr == nil {
					if p, parseErr := strconv.Atoi(portStr); parseErr == nil && p > 0 && p <= 65535 {
						reality["handshake"] = map[string]any{
							"server":      host,
							"server_port": p,
						}
					}
				}
				tlsOptions["reality"] = reality
			}
		}
	}

	switch p := config.Protocol.(type) {
	case *pb.InboundConfig_Vless:
		inbound["type"] = "vless"
		users := make([]map[string]any, 0, len(accounts))
		for _, acc := range accounts {
			users = append(users, map[string]any{
				"name": acc.Name,
				"uuid": acc.Token,
			})
		}
		inbound["users"] = users
		if tr := singVlessTransport(p.Vless.Transmission); tr != nil {
			inbound["transport"] = tr
		}
		if p.Vless.Security != pb.SecurityMode_REALITY && tlsOptions != nil && (tlsCertificate == nil || strings.TrimSpace(tlsCertificate.CertificatePath) == "" || strings.TrimSpace(tlsCertificate.KeyPath) == "") {
			return nil, fmt.Errorf("vless tls requires tls certificate_name with valid certificate_path and key_path")
		}
		if tlsOptions != nil {
			inbound["tls"] = tlsOptions
		}

	case *pb.InboundConfig_Hysteria2:
		inbound["type"] = "hysteria2"
		inbound["up_mbps"] = p.Hysteria2.UpMbps
		inbound["down_mbps"] = p.Hysteria2.DownMbps
		inbound["ignore_client_bandwidth"] = p.Hysteria2.IgnoreClientBandwidth
		inbound["brutal_debug"] = p.Hysteria2.BrutalDebug
		if strings.TrimSpace(p.Hysteria2.BbrProfile) != "" {
			inbound["bbr_profile"] = p.Hysteria2.BbrProfile
		}
		if strings.TrimSpace(p.Hysteria2.ObfsType) != "" {
			inbound["obfs"] = map[string]any{
				"type":     p.Hysteria2.ObfsType,
				"password": p.Hysteria2.ObfsPassword,
			}
		}
		if masquerade, err := parseSingBoxMasquerade(p.Hysteria2.Masquerade); err != nil {
			return nil, err
		} else if masquerade != nil {
			inbound["masquerade"] = masquerade
		}
		users := make([]map[string]any, 0, len(accounts))
		for _, acc := range accounts {
			password := p.Hysteria2.Password
			if password == "" {
				password = acc.Token
			}
			users = append(users, map[string]any{
				"name":     acc.Name,
				"password": password,
			})
		}
		inbound["users"] = users
		if tlsOptions != nil && (tlsCertificate == nil || strings.TrimSpace(tlsCertificate.CertificatePath) == "" || strings.TrimSpace(tlsCertificate.KeyPath) == "") {
			return nil, fmt.Errorf("hysteria2 requires tls certificate_name with valid certificate_path and key_path")
		}
		if tlsOptions != nil {
			inbound["tls"] = tlsOptions
		}

	case *pb.InboundConfig_Naiveproxy:
		inbound["type"] = "naive"
		users := make([]map[string]any, 0, len(accounts))
		for _, acc := range accounts {
			users = append(users, map[string]any{
				"username": acc.Name,
				"password": acc.Token,
			})
		}
		inbound["users"] = users
		if tlsOptions != nil && (tlsCertificate == nil || strings.TrimSpace(tlsCertificate.CertificatePath) == "" || strings.TrimSpace(tlsCertificate.KeyPath) == "") {
			return nil, fmt.Errorf("naiveproxy tls requires tls certificate_name with valid certificate_path and key_path")
		}
		if tlsOptions != nil {
			inbound["tls"] = tlsOptions
		}

	case *pb.InboundConfig_Socks5:
		inbound["type"] = "socks"
		if p.Socks5.Username != "" {
			inbound["users"] = []map[string]any{
				{
					"username": p.Socks5.Username,
					"password": p.Socks5.Password,
				},
			}
		}

	default:
		return nil, fmt.Errorf("unsupported sing-box protocol for inbound %s", config.Name)
	}

	singCfg["inbounds"] = append(singCfg["inbounds"].([]map[string]any), inbound)

	convertedOutbounds := make([]map[string]any, 0, len(outbounds))
	for _, out := range outbounds {
		o := map[string]any{
			"tag": out.Tag,
		}
		switch out.Type {
		case pb.OutboundType_DIRECT:
			o["type"] = "direct"
		case pb.OutboundType_BLOCK:
			o["type"] = "block"
		case pb.OutboundType_VLESS:
			o["type"] = "vless"
			v := out.GetVless()
			o["server"] = v.Server
			o["server_port"] = v.Port
			o["uuid"] = v.Uuid
			if v.Flow != "" {
				o["flow"] = v.Flow
			}
			if tr := singVlessTransport(v.Transmission); tr != nil {
				o["transport"] = tr
			}
			if v.Security == pb.SecurityMode_TLS {
				o["tls"] = map[string]any{
					"enabled": true,
				}
			}
		case pb.OutboundType_WIREGUARD:
			o["type"] = "wireguard"
			w := out.GetWireguard()
			addresses := make([]string, 0, len(w.Addresses))
			for _, addr := range w.Addresses {
				if _, err := netip.ParsePrefix(addr); err == nil {
					addresses = append(addresses, addr)
				}
			}
			reserved := make([]uint32, 0, len(w.Reserved))
			for _, r := range w.Reserved {
				reserved = append(reserved, r)
			}
			peers := make([]map[string]any, 0, len(w.Peers))
			for _, peer := range w.Peers {
				address := ""
				port := int32(0)
				if host, portStr, splitErr := net.SplitHostPort(peer.Endpoint); splitErr == nil {
					address = host
					if p, parseErr := strconv.Atoi(portStr); parseErr == nil {
						port = int32(p)
					}
				}
				peerAllowedIPs := make([]string, 0, len(peer.AllowedIps))
				for _, ip := range peer.AllowedIps {
					peerAllowedIPs = append(peerAllowedIPs, ip)
				}
				if len(peerAllowedIPs) == 0 {
					peerAllowedIPs = []string{"0.0.0.0/0", "::/0"}
				}
				peerReserved := make([]uint32, 0, len(peer.Reserved))
				for _, value := range peer.Reserved {
					peerReserved = append(peerReserved, value)
				}
				if len(peerReserved) == 0 {
					peerReserved = reserved
				}
				peerObject := map[string]any{
					"public_key":  peer.PublicKey,
					"address":     address,
					"port":        port,
					"reserved":    peerReserved,
					"allowed_ips": peerAllowedIPs,
				}
				if peer.PreSharedKey != "" {
					peerObject["pre_shared_key"] = peer.PreSharedKey
				}
				if peer.Keepalive > 0 {
					peerObject["persistent_keepalive_interval"] = peer.Keepalive
				}
				peers = append(peers, peerObject)
			}
			o["private_key"] = w.PrivateKey
			o["address"] = addresses
			o["mtu"] = w.Mtu
			if len(peers) > 0 {
				o["peers"] = peers
			}
		case pb.OutboundType_SOCKS5:
			o["type"] = "socks"
			s := out.GetSocks5()
			o["server"] = s.Server
			o["server_port"] = s.Port
			o["version"] = "5"
			if s.Username != "" {
				o["username"] = s.Username
				o["password"] = s.Password
			}
		default:
			continue
		}
		convertedOutbounds = append(convertedOutbounds, o)
	}
	singCfg["outbounds"] = convertedOutbounds

	convertedRules := make([]map[string]any, 0, len(rules))
	for _, rule := range rules {
		r := map[string]any{
			"type":     "default",
			"outbound": rule.OutboundTag,
		}
		if len(rule.Domain) > 0 {
			r["domain"] = rule.Domain
		}
		if protocol := normalizedSingBoxRuleValues(rule.Protocol); len(protocol) > 0 {
			r["protocol"] = protocol
		}
		network := normalizedSingBoxRuleTransports(rule.Transport)
		if len(network) > 0 {
			r["network"] = network
		}

		ipCidr := make([]string, 0, len(rule.Ip))
		geoip := make([]string, 0, len(rule.Ip))
		for _, ip := range rule.Ip {
			if strings.HasPrefix(ip, "geoip:") {
				geoip = append(geoip, strings.TrimPrefix(ip, "geoip:"))
			} else {
				ipCidr = append(ipCidr, ip)
			}
		}
		if len(ipCidr) > 0 {
			r["ip_cidr"] = ipCidr
		}
		if len(geoip) > 0 {
			r["geoip"] = geoip
		}

		if len(rule.Port) > 0 {
			ports := make([]int, 0, len(rule.Port))
			for _, ps := range rule.Port {
				if p, parseErr := strconv.Atoi(ps); parseErr == nil && p > 0 && p <= 65535 {
					ports = append(ports, p)
				}
			}
			if len(ports) > 0 {
				r["port"] = ports
			}
		}
		convertedRules = append(convertedRules, r)
	}
	singCfg["route"] = map[string]any{
		"rules": convertedRules,
	}

	return singCfg, nil
}

func (e *SingBoxEngine) GetMetrics(ctx context.Context) (*RuntimeMetrics, error) {
	return &RuntimeMetrics{
		Inbound: &pb.InboundStatus{
			Name: e.name,
			Traffic: &pb.TrafficStats{
				Rx: atomic.LoadUint64(&e.rx),
				Tx: atomic.LoadUint64(&e.tx),
			},
			Connections: &pb.ConnectionStats{},
		},
	}, nil
}

func (e *SingBoxEngine) Stop(ctx context.Context) error {
	e.mu.Lock()
	defer e.mu.Unlock()

	if e.cmd != nil && e.cmd.Process != nil {
		_ = e.cmd.Process.Kill()
		e.cmd = nil
	}
	if e.tmpDir != "" {
		_ = os.RemoveAll(e.tmpDir)
		e.tmpDir = ""
	}
	return nil
}
