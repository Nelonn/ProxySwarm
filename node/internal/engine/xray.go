package engine

import (
	"context"
	"encoding/json"
	"fmt"
	"proxyswarm/node/internal/pb"
	"log"
	"net"
	"slices"
	"strings"
	"sync"

	"github.com/xtls/xray-core/app/proxyman/command"
	_ "github.com/xtls/xray-core/app/proxyman/inbound"
	_ "github.com/xtls/xray-core/app/proxyman/outbound"
	statsService "github.com/xtls/xray-core/app/stats/command"
	xnet "github.com/xtls/xray-core/common/net"
	"github.com/xtls/xray-core/common/protocol"
	"github.com/xtls/xray-core/common/serial"
	"github.com/xtls/xray-core/core"
	"github.com/xtls/xray-core/infra/conf"
	_ "github.com/xtls/xray-core/proxy/dokodemo"
	hyaccount "github.com/xtls/xray-core/proxy/hysteria/account"
	_ "github.com/xtls/xray-core/proxy/loopback"
	"github.com/xtls/xray-core/proxy/vless"
	_ "github.com/xtls/xray-core/proxy/vless/inbound"
	_ "github.com/xtls/xray-core/proxy/wireguard"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/protobuf/proto"
)

type XrayEngine struct {
	mu                    sync.Mutex
	instance              *core.Instance
	name                  string
	apiPort               int
	lastConfig            *pb.InboundConfig
	lastDnsConfig         *pb.DnsConfig
	lastRules             []*pb.RoutingRule
	lastAccounts          map[string]string // email -> token
	lastOutboundsSnapshot []*pb.OutboundConfig
	lastOutbounds         map[string]*pb.OutboundStatus
}

type WireGuardPeerConfig struct {
	PublicKey    string   `json:"publicKey,omitempty"`
	PreSharedKey string   `json:"preSharedKey,omitempty"`
	Endpoint     string   `json:"endpoint,omitempty"`
	KeepAlive    uint32   `json:"keepAlive,omitempty"`
	AllowedIps   []string `json:"allowedIps,omitempty"`
}

type WireGuardDeviceConfig struct {
	SecretKey      string                `json:"secretKey,omitempty"`
	Address        []string              `json:"address,omitempty"`
	Peers          []WireGuardPeerConfig `json:"peers,omitempty"`
	Mtu            int32                 `json:"mtu,omitempty"`
	NumWorkers     int32                 `json:"workers,omitempty"`
	Reserved       []int                 `json:"reserved,omitempty"`
	DomainStrategy string                `json:"domainStrategy,omitempty"`
	NoKernelTun    bool                  `json:"noKernelTun,omitempty"`
}

func NewXrayEngine(name string) *XrayEngine {
	return &XrayEngine{
		name:          name,
		lastAccounts:  make(map[string]string),
		lastOutbounds: make(map[string]*pb.OutboundStatus),
	}
}

func (e *XrayEngine) UpdateConfig(ctx context.Context, config *pb.InboundConfig, accounts []*pb.Account, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig, certificates []*pb.CertificateConfig) error {
	e.mu.Lock()
	defer e.mu.Unlock()

	return e.restart(config, accounts, outbounds, rules, dns, certificates)
}

func (e *XrayEngine) needsRestart(config *pb.InboundConfig, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig) bool {
	if _, ok := config.Protocol.(*pb.InboundConfig_Wireguard); ok {
		return true
	}
	if e.lastConfig == nil {
		return true
	}
	if !proto.Equal(e.lastConfig, config) {
		return true
	}
	if !proto.Equal(e.lastDnsConfig, dns) {
		return true
	}
	if !equalProtoSlices(e.lastOutboundsSnapshot, outbounds) {
		return true
	}
	if !equalProtoSlices(e.lastRules, rules) {
		return true
	}
	return false
}

func (e *XrayEngine) restart(config *pb.InboundConfig, accounts []*pb.Account, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig, certificates []*pb.CertificateConfig) error {
	if e.instance != nil {
		e.instance.Close()
	}

	apiPort, err := e.getFreePort()
	if err != nil {
		return err
	}
	e.apiPort = apiPort

	coreConfig, err := e.convertToConfig(config, accounts, outbounds, rules, dns, certificates, apiPort)
	if err != nil {
		return fmt.Errorf("failed to convert xray config: %w", err)
	}

	instance, err := core.New(coreConfig)
	if err != nil {
		return fmt.Errorf("failed to create xray instance: %w", err)
	}

	if err := instance.Start(); err != nil {
		return fmt.Errorf("failed to start xray: %w", err)
	}

	e.instance = instance
	e.lastConfig = cloneProtoMessage(config)
	e.lastDnsConfig = cloneProtoMessage(dns)
	e.lastOutboundsSnapshot = cloneProtoSlice(outbounds)
	e.lastRules = cloneProtoSlice(rules)
	e.lastAccounts = make(map[string]string)
	e.lastOutbounds = make(map[string]*pb.OutboundStatus)
	for _, acc := range accounts {
		e.lastAccounts[acc.Name] = acc.Token
	}
	for _, outbound := range outbounds {
		if outbound == nil || strings.TrimSpace(outbound.Tag) == "" {
			continue
		}
		e.lastOutbounds[outbound.Tag] = &pb.OutboundStatus{
			Name:               outbound.Tag,
			Type:               outbound.Type.String(),
			ExcludedFromTotals: outbound.Type == pb.OutboundType_BLOCK,
			Traffic:            &pb.TrafficStats{},
		}
	}
	return nil
}

func cloneProtoMessage[T proto.Message](value T) T {
	var zero T
	if any(value) == nil {
		return zero
	}
	cloned, ok := proto.Clone(value).(T)
	if !ok {
		return zero
	}
	return cloned
}

func cloneProtoSlice[T proto.Message](items []T) []T {
	if len(items) == 0 {
		return nil
	}
	cloned := make([]T, len(items))
	for i, item := range items {
		cloned[i] = cloneProtoMessage(item)
	}
	return cloned
}

func equalProtoSlices[T proto.Message](left []T, right []T) bool {
	if len(left) != len(right) {
		return false
	}
	for i := range left {
		if !proto.Equal(left[i], right[i]) {
			return false
		}
	}
	return true
}

func (e *XrayEngine) syncAccounts(ctx context.Context, config *pb.InboundConfig, accounts []*pb.Account) error {
	addr := fmt.Sprintf("127.0.0.1:%d", e.apiPort)
	conn, err := grpc.Dial(addr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return err
	}
	defer conn.Close()

	client := command.NewHandlerServiceClient(conn)

	newAccounts := make(map[string]string)
	for _, acc := range accounts {
		credential := strings.TrimSpace(acc.Token)
		if hy2Cfg := config.GetHysteria2(); hy2Cfg != nil && credential == "" {
			credential = strings.TrimSpace(hy2Cfg.Password)
		}
		newAccounts[acc.Name] = credential
	}

	flow := ""
	if vlessCfg := config.GetVless(); vlessCfg != nil {
		flow = vlessCfg.Flow
	}

	// Add new or updated accounts
	for email, token := range newAccounts {
		if oldId, ok := e.lastAccounts[email]; !ok || oldId != token {
			if ok {
				// Remove old one first if ID changed
				client.AlterInbound(ctx, &command.AlterInboundRequest{
					Tag: e.name,
					Operation: serial.ToTypedMessage(&command.RemoveUserOperation{
						Email: email,
					}),
				})
			}

			// Add user
			var account *serial.TypedMessage
			if config.GetHysteria2() != nil {
				account = serial.ToTypedMessage(&hyaccount.Account{
					Auth: token,
				})
			} else {
				account = serial.ToTypedMessage(&vless.Account{
					Id:   token,
					Flow: flow,
				})
			}

			client.AlterInbound(ctx, &command.AlterInboundRequest{
				Tag: e.name,
				Operation: serial.ToTypedMessage(&command.AddUserOperation{
					User: &protocol.User{
						Email:   email,
						Account: account,
					},
				}),
			})
		}
	}

	// Remove deleted accounts
	for email := range e.lastAccounts {
		if _, ok := newAccounts[email]; !ok {
			client.AlterInbound(ctx, &command.AlterInboundRequest{
				Tag: e.name,
				Operation: serial.ToTypedMessage(&command.RemoveUserOperation{
					Email: email,
				}),
			})
		}
	}

	e.lastAccounts = newAccounts
	return nil
}

func (e *XrayEngine) getFreePort() (int, error) {
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return 0, err
	}
	defer l.Close()
	return l.Addr().(*net.TCPAddr).Port, nil
}

func toRaw(v any) json.RawMessage {
	b, _ := json.Marshal(v)
	return json.RawMessage(b)
}

func toRawPtr(v any) *json.RawMessage {
	rm := toRaw(v)
	return &rm
}

func normalizedXrayRuleValues(values []string) []string {
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

func normalizedXrayRuleTransports(values []string) []string {
	normalized := normalizedXrayRuleValues(values)
	if len(normalized) == 0 {
		return nil
	}

	transports := make([]string, 0, len(normalized))
	seen := make(map[string]struct{}, len(normalized))
	for _, value := range normalized {
		parts := strings.Split(value, ",")
		for _, part := range parts {
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

func xrayRuleHasMatchers(rule *pb.RoutingRule) bool {
	return len(normalizedXrayRuleValues(rule.Domain)) > 0 ||
		len(normalizedXrayRuleValues(rule.InboundTag)) > 0 ||
		len(normalizedXrayRuleValues(rule.Ip)) > 0 ||
		len(normalizedXrayRuleValues(rule.Port)) > 0 ||
		len(normalizedXrayRuleTransports(rule.Transport)) > 0 ||
		len(normalizedXrayRuleValues(rule.Protocol)) > 0
}

func buildXrayRoutingRules(rules []*pb.RoutingRule) []json.RawMessage {
	rawRules := make([]json.RawMessage, 0, len(rules))
	for _, rule := range rules {
		if rule == nil || strings.TrimSpace(rule.OutboundTag) == "" {
			continue
		}
		if !xrayRuleHasMatchers(rule) {
			continue
		}
		rawRule := map[string]any{
			"type":        "field",
			"outboundTag": strings.TrimSpace(rule.OutboundTag),
		}
		if inbound := normalizedXrayRuleValues(rule.InboundTag); len(inbound) > 0 {
			rawRule["inboundTag"] = inbound
		}
		if domain := normalizedXrayRuleValues(rule.Domain); len(domain) > 0 {
			rawRule["domain"] = domain
		}
		if ip := normalizedXrayRuleValues(rule.Ip); len(ip) > 0 {
			rawRule["ip"] = ip
		}
		if network := normalizedXrayRuleTransports(rule.Transport); len(network) > 0 {
			rawRule["network"] = strings.Join(network, ",")
		}
		if protocol := normalizedXrayRuleValues(rule.Protocol); len(protocol) > 0 {
			rawRule["protocol"] = protocol
		}
		if port := normalizedXrayRuleValues(rule.Port); len(port) > 0 {
			rawRule["port"] = port
		}
		rawRules = append(rawRules, toRaw(rawRule))
	}
	return rawRules
}

func xrayVlessNetwork(transmission string) *conf.TransportProtocol {
	var network conf.TransportProtocol
	switch transmission {
	case "HTTP":
		network = "http"
	case "gRPC":
		network = "grpc"
	case "WebSocket":
		network = "ws"
	case "mKCP":
		network = "kcp"
	case "HttpUpgrade":
		network = "httpupgrade"
	case "SplitHTTP":
		network = "splithttp"
	default:
		network = "tcp"
	}
	return &network
}

func xrayHysteriaNetwork() *conf.TransportProtocol {
	network := conf.TransportProtocol("hysteria")
	return &network
}

func parseXrayMasquerade(value string) (conf.Masquerade, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return conf.Masquerade{}, nil
	}
	if strings.HasPrefix(value, "{") || strings.HasPrefix(value, "[") {
		var parsed conf.Masquerade
		if err := json.Unmarshal([]byte(value), &parsed); err != nil {
			return conf.Masquerade{}, fmt.Errorf("invalid hysteria2 masquerade JSON: %w", err)
		}
		return parsed, nil
	}
	return conf.Masquerade{
		Type: "url",
		Url:  value,
	}, nil
}

func hysteriaBandwidth(mbps uint32) conf.Bandwidth {
	if mbps == 0 {
		return ""
	}
	return conf.Bandwidth(fmt.Sprintf("%d mbps", mbps))
}

func buildXrayHostAddress(values []string) (*conf.HostAddress, error) {
	values = normalizedXrayRuleValues(values)
	if len(values) == 0 {
		return nil, nil
	}
	var payload []byte
	var err error
	if len(values) == 1 {
		payload, err = json.Marshal(values[0])
	} else {
		payload, err = json.Marshal(values)
	}
	if err != nil {
		return nil, err
	}
	var addr conf.HostAddress
	if err := json.Unmarshal(payload, &addr); err != nil {
		return nil, err
	}
	return &addr, nil
}

func buildXrayDNSConfig(dnsConfig *pb.DnsConfig) (*conf.DNSConfig, error) {
	if dnsConfig == nil {
		return nil, nil
	}

	dns := &conf.DNSConfig{
		Tag:                    strings.TrimSpace(dnsConfig.Tag),
		QueryStrategy:          strings.TrimSpace(dnsConfig.QueryStrategy),
		DisableCache:           dnsConfig.DisableCache,
		ServeStale:             dnsConfig.ServeStale,
		ServeExpiredTTL:        dnsConfig.ServeExpiredTtl,
		DisableFallback:        dnsConfig.DisableFallback,
		DisableFallbackIfMatch: dnsConfig.DisableFallbackIfMatch,
		EnableParallelQuery:    dnsConfig.EnableParallelQuery,
		UseSystemHosts:         dnsConfig.UseSystemHosts,
	}
	if clientIP := strings.TrimSpace(dnsConfig.ClientIp); clientIP != "" {
		dns.ClientIP = &conf.Address{Address: xnet.ParseAddress(clientIP)}
	}

	for _, server := range dnsConfig.Servers {
		if server == nil {
			continue
		}
		address := strings.TrimSpace(server.Address)
		if address == "" {
			continue
		}
		if server.Port > 65535 {
			return nil, fmt.Errorf("invalid dns server port %d for %q", server.Port, address)
		}
		nameServer := &conf.NameServerConfig{
			Address:       &conf.Address{Address: xnet.ParseAddress(address)},
			Port:          uint16(server.Port),
			SkipFallback:  server.SkipFallback,
			Domains:       normalizedXrayRuleValues(server.Domains),
			ExpectIPs:     normalizedXrayRuleValues(server.ExpectIps),
			QueryStrategy: strings.TrimSpace(server.QueryStrategy),
			Tag:           strings.TrimSpace(server.Tag),
			TimeoutMs:     server.TimeoutMs,
			FinalQuery:    server.FinalQuery,
			UnexpectedIPs: normalizedXrayRuleValues(server.UnexpectedIps),
		}
		if clientIP := strings.TrimSpace(server.ClientIp); clientIP != "" {
			nameServer.ClientIP = &conf.Address{Address: xnet.ParseAddress(clientIP)}
		}
		if server.DisableCache != nil {
			value := server.GetDisableCache()
			nameServer.DisableCache = &value
		}
		if server.ServeStale != nil {
			value := server.GetServeStale()
			nameServer.ServeStale = &value
		}
		if server.ServeExpiredTtl != nil {
			value := server.GetServeExpiredTtl()
			nameServer.ServeExpiredTTL = &value
		}
		dns.Servers = append(dns.Servers, nameServer)
	}

	if len(dnsConfig.Hosts) > 0 {
		hosts := make(map[string]*conf.HostAddress, len(dnsConfig.Hosts))
		for _, host := range dnsConfig.Hosts {
			if host == nil {
				continue
			}
			domain := strings.TrimSpace(host.Domain)
			if domain == "" {
				continue
			}
			addr, err := buildXrayHostAddress(host.Values)
			if err != nil {
				return nil, fmt.Errorf("invalid dns host mapping for %q: %w", domain, err)
			}
			if addr == nil {
				continue
			}
			hosts[domain] = addr
		}
		if len(hosts) > 0 {
			dns.Hosts = &conf.HostsWrapper{Hosts: hosts}
		}
	}

	if len(dns.Servers) == 0 &&
		dns.Hosts == nil &&
		dns.ClientIP == nil &&
		dns.Tag == "" &&
		dns.QueryStrategy == "" &&
		!dns.DisableCache &&
		!dns.ServeStale &&
		dns.ServeExpiredTTL == 0 &&
		!dns.DisableFallback &&
		!dns.DisableFallbackIfMatch &&
		!dns.EnableParallelQuery &&
		!dns.UseSystemHosts {
		return nil, nil
	}

	return dns, nil
}

func (e *XrayEngine) convertToConfig(config *pb.InboundConfig, accounts []*pb.Account, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dnsConfig *pb.DnsConfig, certificates []*pb.CertificateConfig, apiPort int) (*core.Config, error) {
	c := &conf.Config{}

	c.LogConfig = &conf.LogConfig{
		LogLevel: "debug",
	}

	xrayDNS, err := buildXrayDNSConfig(dnsConfig)
	if err != nil {
		return nil, err
	}
	c.DNSConfig = xrayDNS

	c.API = &conf.APIConfig{
		Tag:      "api",
		Services: []string{"HandlerService", "StatsService"},
	}
	c.Stats = &conf.StatsConfig{}
	c.Policy = &conf.PolicyConfig{
		Levels: map[uint32]*conf.Policy{
			0: {
				StatsUserUplink:   true,
				StatsUserDownlink: true,
				StatsUserOnline:   true,
			},
		},
		System: &conf.SystemPolicy{
			StatsInboundUplink:    true,
			StatsInboundDownlink:  true,
			StatsOutboundUplink:   true,
			StatsOutboundDownlink: true,
		},
	}

	apiInbound := conf.InboundDetourConfig{
		Tag:      "api-in",
		ListenOn: &conf.Address{Address: xnet.ParseAddress("127.0.0.1")},
		PortList: &conf.PortList{Range: []conf.PortRange{{From: uint32(apiPort), To: uint32(apiPort)}}},
		Protocol: "dokodemo-door",
		Settings: toRawPtr(map[string]any{
			"address": "127.0.0.1",
			"network": "tcp",
		}),
	}
	c.InboundConfigs = append(c.InboundConfigs, apiInbound)

	inbound := conf.InboundDetourConfig{
		Tag:      config.Name,
		ListenOn: &conf.Address{Address: xnet.ParseAddress(config.Listen)},
		PortList: &conf.PortList{
			Range: []conf.PortRange{
				{From: uint32(config.Port), To: uint32(config.Port)},
			},
		},
	}

	if vlessCfg := config.GetVless(); vlessCfg != nil {
		inbound.StreamSetting = &conf.StreamConfig{
			Network: xrayVlessNetwork(vlessCfg.Transmission),
		}
	} else if hy2Cfg := config.GetHysteria2(); hy2Cfg != nil {
		masquerade, err := parseXrayMasquerade(hy2Cfg.Masquerade)
		if err != nil {
			return nil, err
		}
		inbound.StreamSetting = &conf.StreamConfig{
			Network:  xrayHysteriaNetwork(),
			Security: "tls",
			HysteriaSettings: &conf.HysteriaConfig{
				Version:        2,
				UdpIdleTimeout: 60,
				Masquerade:     masquerade,
			},
			FinalMask: &conf.FinalMask{
				QuicParams: &conf.QuicParamsConfig{
					Congestion: "bbr",
					Debug:      hy2Cfg.BrutalDebug,
					BrutalUp:   hysteriaBandwidth(hy2Cfg.UpMbps),
					BrutalDown: hysteriaBandwidth(hy2Cfg.DownMbps),
				},
			},
		}
		if strings.TrimSpace(hy2Cfg.ObfsType) != "" && strings.EqualFold(strings.TrimSpace(hy2Cfg.ObfsType), "salamander") {
			maskSettings := toRaw(map[string]any{
				"password": hy2Cfg.ObfsPassword,
			})
			inbound.StreamSetting.FinalMask.Udp = []conf.Mask{
				{
					Type:     "salamander",
					Settings: &maskSettings,
				},
			}
		}
	}

	tlsConfig := inboundTLSConfig(config)
	tlsCertificate, err := resolveInboundTLSCertificate(tlsConfig, certificates)
	if err != nil {
		return nil, err
	}
	if tlsConfig != nil && tlsConfig.Enabled {
		if inbound.StreamSetting == nil {
			inbound.StreamSetting = &conf.StreamConfig{}
		}
		stream := inbound.StreamSetting

		switch config.Protocol.(type) {
		case *pb.InboundConfig_Vless:
			vlessCfg := config.Protocol.(*pb.InboundConfig_Vless).Vless
			stream.Network = xrayVlessNetwork(vlessCfg.Transmission)
			if vlessCfg.Security == pb.SecurityMode_TLS {
				if tlsCertificate == nil || strings.TrimSpace(tlsCertificate.CertificatePath) == "" || strings.TrimSpace(tlsCertificate.KeyPath) == "" {
					return nil, fmt.Errorf("vless tls requires tls certificate_name with valid certificate_path and key_path")
				}
				stream.Security = "tls"
				stream.TLSSettings = &conf.TLSConfig{
					Certs: []*conf.TLSCertConfig{
						{
							CertFile: tlsCertificate.CertificatePath,
							KeyFile:  tlsCertificate.KeyPath,
						},
					},
				}
			} else if vlessCfg.Security == pb.SecurityMode_REALITY {
				realityCfg := vlessCfg.Reality
				if realityCfg == nil {
					break
				}
				realitySNI := realityCfg.Sni
				if realitySNI == "" {
					realitySNI = tlsConfig.ServerName
				}
				stream.Security = "reality"
				stream.REALITYSettings = &conf.REALITYConfig{
					Show:        true,
					Dest:        toRaw(realityCfg.Dest),
					Xver:        0,
					ServerNames: []string{realitySNI},
					PrivateKey:  realityCfg.PrivateKey,
					ShortIds:    realityCfg.ShortId,
					Fingerprint: realityCfg.Utls,
					SpiderX:     realityCfg.SpiderX,
				}
			}
		case *pb.InboundConfig_Hysteria2:
			if tlsCertificate == nil || strings.TrimSpace(tlsCertificate.CertificatePath) == "" || strings.TrimSpace(tlsCertificate.KeyPath) == "" {
				return nil, fmt.Errorf("hysteria2 requires tls certificate_name with valid certificate_path and key_path")
			}
			stream.Security = "tls"
			stream.TLSSettings = &conf.TLSConfig{
				ServerName: tlsConfig.ServerName,
				Certs: []*conf.TLSCertConfig{
					{
						CertFile: tlsCertificate.CertificatePath,
						KeyFile:  tlsCertificate.KeyPath,
					},
				},
			}
		}
	}

	switch p := config.Protocol.(type) {
	case *pb.InboundConfig_Vless:
		inbound.Protocol = "vless"
		var clients []map[string]any
		for _, acc := range accounts {
			clients = append(clients, map[string]any{
				"id":    acc.Token,
				"email": acc.Name,
				"flow":  p.Vless.Flow,
			})
		}
		inbound.Settings = toRawPtr(map[string]any{
			"clients":    clients,
			"decryption": "none",
		})
	case *pb.InboundConfig_Wireguard:
		inbound.Protocol = "wireguard"
		peers := make([]*conf.WireGuardPeerConfig, 0, len(accounts))
		for _, acc := range accounts {
			allowedIPs := append([]string{}, acc.AllowedIps...)
			peers = append(peers, &conf.WireGuardPeerConfig{
				PublicKey:  acc.Token,
				AllowedIPs: allowedIPs,
			})
		}
		inbound.Settings = toRawPtr(&conf.WireGuardConfig{
			IsClient:  false,
			SecretKey: p.Wireguard.PrivateKey,
			Address:   p.Wireguard.Addresses,
			Peers:     peers,
			MTU:       p.Wireguard.Mtu,
		})
	case *pb.InboundConfig_Hysteria2:
		inbound.Protocol = "hysteria"
		clients := make([]map[string]any, 0, len(accounts))
		for _, acc := range accounts {
			auth := strings.TrimSpace(acc.Token)
			if auth == "" {
				auth = strings.TrimSpace(p.Hysteria2.Password)
			}
			clients = append(clients, map[string]any{
				"auth":  auth,
				"email": acc.Name,
				"level": 0,
			})
		}
		inbound.Settings = toRawPtr(map[string]any{
			"version": 2,
			"clients": clients,
		})
	case *pb.InboundConfig_Socks5:
		inbound.Protocol = "socks"
		settings := map[string]any{
			"auth": "noauth",
			"udp":  p.Socks5.UdpEnabled,
		}
		if p.Socks5.Username != "" {
			settings["auth"] = "password"
			settings["accounts"] = []map[string]any{
				{
					"user": p.Socks5.Username,
					"pass": p.Socks5.Password,
				},
			}
		}
		inbound.Settings = toRawPtr(settings)

	default:
		return nil, fmt.Errorf("protocol not supported by Xray engine")
	}

	c.InboundConfigs = append(c.InboundConfigs, inbound)

	allowedOutboundTags := map[string]struct{}{}
	for _, out := range outbounds {
		if out == nil || strings.TrimSpace(out.Tag) == "" {
			continue
		}
		o := conf.OutboundDetourConfig{
			Tag: strings.TrimSpace(out.Tag),
		}
		switch out.Type {
		case pb.OutboundType_DIRECT:
			o.Protocol = "freedom"
			o.Settings = toRawPtr(conf.FreedomConfig{DomainStrategy: "UseIP"})
		case pb.OutboundType_BLOCK:
			o.Protocol = "blackhole"
		case pb.OutboundType_VLESS:
			o.Protocol = "vless"
			v := out.GetVless()
			clients := []map[string]any{
				{
					"id":   v.Uuid,
					"flow": v.Flow,
				},
			}
			o.Settings = toRawPtr(map[string]any{
				"vnext": []map[string]any{
					{
						"address": v.Server,
						"port":    v.Port,
						"users":   clients,
					},
				},
			})
			stream := &conf.StreamConfig{
				Network: xrayVlessNetwork(v.Transmission),
			}
			if v.Security == pb.SecurityMode_TLS {
				stream.Security = "tls"
				stream.TLSSettings = &conf.TLSConfig{
					AllowInsecure: true,
				}
			}
			o.StreamSetting = stream
		case pb.OutboundType_WIREGUARD:
			o.Protocol = "wireguard"
			w := out.GetWireguard()
			reserved := make([]int, 0, len(w.Reserved))
			for _, r := range w.Reserved {
				reserved = append(reserved, int(r))
			}
			peers := make([]WireGuardPeerConfig, 0, len(w.Peers))
			for _, peer := range w.Peers {
				allowedIPs := make([]string, 0, len(peer.AllowedIps))
				for _, value := range peer.AllowedIps {
					allowedIPs = append(allowedIPs, value)
				}
				if len(allowedIPs) == 0 {
					allowedIPs = []string{"0.0.0.0/0", "::/0"}
				}
				peers = append(peers, WireGuardPeerConfig{
					PublicKey:    peer.PublicKey,
					PreSharedKey: peer.PreSharedKey,
					Endpoint:     peer.Endpoint,
					KeepAlive:    peer.Keepalive,
					AllowedIps:   allowedIPs,
				})
			}
			if len(peers) == 0 {
				continue
			}
			address := make([]string, 0, len(w.Addresses))
			for _, value := range w.Addresses {
				address = append(address, value)
			}
			o.Settings = toRawPtr(&WireGuardDeviceConfig{
				SecretKey: w.PrivateKey,
				Address:   address,
				Peers:     peers,
				Mtu:            int32(w.Mtu),
				NumWorkers: func() int32 {
					if w.Workers > 0 {
						return w.Workers
					}
					return 2
				}(),
				Reserved:       reserved,
				DomainStrategy: func() string {
					switch w.DomainStrategy {
					case "ForceIPv4", "ForceIPv4v6", "ForceIPv6", "ForceIPv6v4":
						return w.DomainStrategy
					default:
						return "ForceIP"
					}
				}(),
				NoKernelTun:    true,
			})
		case pb.OutboundType_SOCKS5:
			o.Protocol = "socks"
			s := out.GetSocks5()
			server := map[string]any{
				"address": s.Server,
				"port":    s.Port,
			}
			if s.Username != "" {
				server["users"] = []map[string]any{
					{
						"user":  s.Username,
						"pass":  s.Password,
						"email": "socks5",
					},
				}
			}
			o.Settings = toRawPtr(map[string]any{
				"servers": []map[string]any{server},
			})
		default:
			// Keep Xray engine resilient: skip unsupported outbound types instead of generating invalid configs.
			continue
		}
		c.OutboundConfigs = append(c.OutboundConfigs, o)
		allowedOutboundTags[o.Tag] = struct{}{}
	}

	c.RouterConfig = &conf.RouterConfig{
		DomainStrategy: func() *string {
			value := "AsIs"
			return &value
		}(),
	}

	if len(rules) > 0 {
		normalized := make([]*pb.RoutingRule, 0, len(rules))
		for _, rule := range rules {
			if rule == nil {
				continue
			}
			tag := strings.TrimSpace(rule.OutboundTag)
			if tag == "" {
				continue
			}
			if _, ok := allowedOutboundTags[tag]; !ok {
				copy := *rule
				copy.OutboundTag = "direct"
				normalized = append(normalized, &copy)
				continue
			}
			normalized = append(normalized, rule)
		}
		c.RouterConfig.RuleList = append(c.RouterConfig.RuleList, buildXrayRoutingRules(normalized)...)
		val := "AsIs"
		c.RouterConfig.DomainStrategy = &val
	}

	if len(c.OutboundConfigs) == 0 {
		c.OutboundConfigs = append(c.OutboundConfigs, conf.OutboundDetourConfig{
			Protocol: "freedom",
			Tag:      "direct",
			Settings: toRawPtr(conf.FreedomConfig{DomainStrategy: "UseIP"}),
		})
	}

	c.OutboundConfigs = append(c.OutboundConfigs, conf.OutboundDetourConfig{
		Protocol: "loopback",
		Tag:      "api",
		Settings: toRawPtr(map[string]any{
			"inboundTag": "api-in",
		}),
	})

	c.RouterConfig.RuleList = append([]json.RawMessage{
		toRaw(map[string]any{
			"inboundTag":  []string{"api-in"},
			"outboundTag": "api",
			"type":        "field",
		}),
	}, c.RouterConfig.RuleList...)

	for _, outbound := range c.OutboundConfigs {
		if strings.TrimSpace(outbound.Tag) == "" {
			return nil, fmt.Errorf("invalid xray outbound: empty tag")
		}
		if strings.TrimSpace(outbound.Protocol) == "" {
			return nil, fmt.Errorf("invalid xray outbound %q: empty protocol", outbound.Tag)
		}
	}

	if dumped, err := json.Marshal(c); err == nil {
		log.Printf("[xray] generated config json=%s", string(dumped))
	} else {
		log.Printf("[xray] failed to dump config: %v", err)
	}

	return c.Build()
}

func (e *XrayEngine) GetMetrics(ctx context.Context) (*RuntimeMetrics, error) {
	e.mu.Lock()
	defer e.mu.Unlock()

	metrics := &RuntimeMetrics{
		Inbound: &pb.InboundStatus{
			Name:        e.name,
			Traffic:     &pb.TrafficStats{},
			Connections: &pb.ConnectionStats{},
		},
	}

	if e.instance == nil {
		return metrics, nil
	}

	client, conn, err := e.statsClient(ctx)
	if err != nil {
		return metrics, nil
	}
	defer conn.Close()

	metrics.Inbound.Traffic = querySingleStatPair(
		ctx,
		client,
		fmt.Sprintf("inbound>>>%s>>>traffic>>>downlink", e.name),
		fmt.Sprintf("inbound>>>%s>>>traffic>>>uplink", e.name),
	)

	onlineUsers := make(map[string]struct{})
	if allOnline, err := client.GetAllOnlineUsers(ctx, &statsService.GetAllOnlineUsersRequest{}); err == nil {
		for _, email := range allOnline.GetUsers() {
			email = strings.TrimSpace(email)
			if email == "" {
				continue
			}
			onlineUsers[email] = struct{}{}
		}
	}

	for accountName := range e.lastAccounts {
		account := &pb.AccountStatus{
			Name: accountName,
			Traffic: querySingleStatPair(
				ctx,
				client,
				fmt.Sprintf("user>>>%s>>>traffic>>>downlink", accountName),
				fmt.Sprintf("user>>>%s>>>traffic>>>uplink", accountName),
			),
		}
		if _, ok := onlineUsers[accountName]; ok {
			account.Online = 1
			if onlineIps, err := client.GetStatsOnlineIpList(ctx, &statsService.GetStatsRequest{
				Name:   fmt.Sprintf("user>>>%s>>>online", accountName),
				Reset_: false,
			}); err == nil {
				for ip := range onlineIps.GetIps() {
					account.Sessions = append(account.Sessions, &pb.UserSessionStatus{
						Ip:        ip,
						UserAgent: "Unknown",
					})
				}
				slices.SortFunc(account.Sessions, func(a, b *pb.UserSessionStatus) int {
					return strings.Compare(a.GetIp(), b.GetIp())
				})
				if len(account.Sessions) > 0 {
					account.Online = uint32(len(account.Sessions))
				}
			}
		} else if onlineIps, err := client.GetStatsOnlineIpList(ctx, &statsService.GetStatsRequest{
			Name:   fmt.Sprintf("user>>>%s>>>online", accountName),
			Reset_: false,
		}); err == nil {
			for ip := range onlineIps.GetIps() {
				account.Sessions = append(account.Sessions, &pb.UserSessionStatus{
					Ip:        ip,
					UserAgent: "Unknown",
				})
			}
			slices.SortFunc(account.Sessions, func(a, b *pb.UserSessionStatus) int {
				return strings.Compare(a.GetIp(), b.GetIp())
			})
			account.Online = uint32(len(account.Sessions))
		} else if online, err := client.GetStatsOnline(ctx, &statsService.GetStatsRequest{
			Name:   fmt.Sprintf("user>>>%s>>>online", accountName),
			Reset_: false,
		}); err == nil && online.GetStat() != nil && online.Stat.Value > 0 {
			account.Online = uint32(online.Stat.Value)
		}
		metrics.Accounts = append(metrics.Accounts, account)
	}

	for tag, outbound := range e.lastOutbounds {
		metrics.Outbounds = append(metrics.Outbounds, &pb.OutboundStatus{
			Name:               tag,
			Type:               outbound.Type,
			ExcludedFromTotals: outbound.ExcludedFromTotals,
			Traffic: querySingleStatPair(
				ctx,
				client,
				fmt.Sprintf("outbound>>>%s>>>traffic>>>downlink", tag),
				fmt.Sprintf("outbound>>>%s>>>traffic>>>uplink", tag),
			),
		})
	}

	return metrics, nil
}

func (e *XrayEngine) statsClient(ctx context.Context) (statsService.StatsServiceClient, *grpc.ClientConn, error) {
	addr := fmt.Sprintf("127.0.0.1:%d", e.apiPort)
	conn, err := grpc.Dial(addr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, nil, err
	}
	return statsService.NewStatsServiceClient(conn), conn, nil
}

func querySingleStatPair(ctx context.Context, client statsService.StatsServiceClient, downlinkName, uplinkName string) *pb.TrafficStats {
	traffic := &pb.TrafficStats{}
	resp, err := client.GetStats(ctx, &statsService.GetStatsRequest{
		Name:   downlinkName,
		Reset_: false,
	})
	if err == nil && resp.GetStat() != nil {
		traffic.Rx = uint64(resp.Stat.Value)
	}

	resp, err = client.GetStats(ctx, &statsService.GetStatsRequest{
		Name:   uplinkName,
		Reset_: false,
	})
	if err == nil && resp.GetStat() != nil {
		traffic.Tx = uint64(resp.Stat.Value)
	}

	return traffic
}

func (e *XrayEngine) Stop(ctx context.Context) error {
	e.mu.Lock()
	defer e.mu.Unlock()

	if e.instance != nil {
		err := e.instance.Close()
		e.instance = nil
		return err
	}
	return nil
}
