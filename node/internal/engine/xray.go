package engine

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"proxyswarm/node/internal/logging"
	"proxyswarm/node/internal/pb"
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
	_ "github.com/xtls/xray-core/proxy/shadowsocks"
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
	lastConfigs           []*pb.InboundConfig
	lastDnsConfig         *pb.DnsConfig
	lastRules             []*pb.RoutingRule
	lastAccounts          map[string]map[string]string // inbound tag -> email -> token
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
		lastAccounts:  make(map[string]map[string]string),
		lastOutbounds: make(map[string]*pb.OutboundStatus),
	}
}

func (e *XrayEngine) UpdateConfig(ctx context.Context, inbounds []*pb.InboundConfig, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig, certificates *CertificatesManager) error {
	e.mu.Lock()
	defer e.mu.Unlock()

	if e.instance != nil && !e.needsRestart(inbounds, outbounds, rules, dns) {
		if err := e.syncInboundAccounts(ctx, inbounds); err != nil {
			return err
		}
		e.lastConfigs = normalizeInboundsForRestart(inbounds)
		return nil
	}

	return e.restart(inbounds, outbounds, rules, dns, certificates)
}

func (e *XrayEngine) needsRestart(configs []*pb.InboundConfig, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig) bool {
	for _, config := range configs {
		if config == nil {
			return true
		}
		if _, ok := config.Protocol.(*pb.InboundConfig_Wireguard); ok {
			return true
		}
	}
	if len(e.lastConfigs) == 0 {
		return true
	}
	if !equalProtoSlices(e.lastConfigs, normalizeInboundsForRestart(configs)) {
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

func (e *XrayEngine) restart(inbounds []*pb.InboundConfig, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dns *pb.DnsConfig, certificates *CertificatesManager) error {
	if len(inbounds) == 0 {
		return fmt.Errorf("xray engine requires at least one inbound")
	}
	if assetDir := defaultXrayAssetRoot(); assetDir != "" {
		_ = os.Setenv("xray.location.asset", assetDir)
	}
	if e.instance != nil {
		e.instance.Close()
	}

	apiPort, err := e.getFreePort()
	if err != nil {
		return err
	}
	e.apiPort = apiPort

	coreConfig, err := e.convertToConfig(inbounds, outbounds, rules, dns, certificates, apiPort)
	if err != nil {
		return fmt.Errorf("failed to convert xray config: %w", err)
	}

	instance, err := core.New(coreConfig)
	if err != nil {
		return fmt.Errorf("failed to create xray instance: %w", err)
	}
	if err := registerXrayCustomOutbounds(instance, outbounds, rules); err != nil {
		return fmt.Errorf("failed to register custom xray outbounds: %w", err)
	}

	if err := instance.Start(); err != nil {
		return fmt.Errorf("failed to start xray: %w", err)
	}

	e.instance = instance
	e.lastConfigs = normalizeInboundsForRestart(inbounds)
	e.lastDnsConfig = cloneProtoMessage(dns)
	e.lastOutboundsSnapshot = cloneProtoSlice(outbounds)
	e.lastRules = cloneProtoSlice(rules)
	e.lastAccounts = make(map[string]map[string]string)
	e.lastOutbounds = make(map[string]*pb.OutboundStatus)
	for _, inbound := range inbounds {
		if inbound == nil || strings.TrimSpace(inbound.Name) == "" {
			continue
		}
		accounts := make(map[string]string)
		for _, acc := range inbound.GetAccounts() {
			id := acc.GetId()
			if id == "" {
				continue
			}
			accounts[id] = acc.Token
		}
		if len(accounts) > 0 {
			e.lastAccounts[inbound.Name] = accounts
		}
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

func normalizeInboundsForRestart(inbounds []*pb.InboundConfig) []*pb.InboundConfig {
	if len(inbounds) == 0 {
		return nil
	}
	cloned := cloneProtoSlice(inbounds)
	for _, inbound := range cloned {
		if inbound == nil {
			continue
		}
		inbound.Accounts = nil
	}
	return cloned
}

func (e *XrayEngine) syncInboundAccounts(ctx context.Context, inbounds []*pb.InboundConfig) error {
	seenTags := make(map[string]struct{})
	for _, inbound := range inbounds {
		if inbound == nil || strings.TrimSpace(inbound.Name) == "" {
			continue
		}
		tag := strings.TrimSpace(inbound.Name)
		seenTags[tag] = struct{}{}
		if inbound.GetVless() == nil && inbound.GetHysteria2() == nil {
			continue
		}
		if err := e.syncAccounts(ctx, inbound, inbound.GetAccounts()); err != nil {
			return err
		}
	}
	for tag := range e.lastAccounts {
		if _, ok := seenTags[tag]; !ok {
			delete(e.lastAccounts, tag)
		}
	}
	return nil
}

func (e *XrayEngine) syncAccounts(ctx context.Context, config *pb.InboundConfig, accounts []*pb.Account) error {
	if config == nil || strings.TrimSpace(config.Name) == "" {
		return nil
	}
	addr := fmt.Sprintf("127.0.0.1:%d", e.apiPort)
	conn, err := grpc.Dial(addr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return err
	}
	defer conn.Close()

	client := command.NewHandlerServiceClient(conn)
	tag := strings.TrimSpace(config.Name)

	newAccounts := make(map[string]string)
	for _, acc := range accounts {
		id := acc.GetId()
		if id == "" {
			continue
		}
		credential := strings.TrimSpace(acc.Token)
		if hy2Cfg := config.GetHysteria2(); hy2Cfg != nil && credential == "" {
			credential = strings.TrimSpace(hy2Cfg.Password)
		}
		newAccounts[id] = credential
	}
	oldAccounts := e.lastAccounts[tag]
	if oldAccounts == nil {
		oldAccounts = make(map[string]string)
	}

	flow := ""
	if vlessCfg := config.GetVless(); vlessCfg != nil {
		flow = vlessCfg.Flow
	}

	// Add new or updated accounts
	for email, token := range newAccounts {
		if oldId, ok := oldAccounts[email]; !ok || oldId != token {
			if ok {
				// Remove old one first if ID changed
				client.AlterInbound(ctx, &command.AlterInboundRequest{
					Tag: tag,
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
				Tag: tag,
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
	for email := range oldAccounts {
		if _, ok := newAccounts[email]; !ok {
			client.AlterInbound(ctx, &command.AlterInboundRequest{
				Tag: tag,
				Operation: serial.ToTypedMessage(&command.RemoveUserOperation{
					Email: email,
				}),
			})
		}
	}

	if len(newAccounts) == 0 {
		delete(e.lastAccounts, tag)
	} else {
		e.lastAccounts[tag] = newAccounts
	}
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

func normalizedXrayServerNames(values ...string) []string {
	serverNames := make([]string, 0, len(values))
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		for _, part := range strings.Split(value, ",") {
			token := strings.TrimSpace(part)
			if token == "" {
				continue
			}
			if _, ok := seen[token]; ok {
				continue
			}
			seen[token] = struct{}{}
			serverNames = append(serverNames, token)
		}
	}
	if len(serverNames) == 0 {
		return nil
	}
	return serverNames
}

func normalizedRealityShortIDs(values []string) []string {
	shortIDs := make([]string, 0, len(values)+1)
	seen := make(map[string]struct{}, len(values)+1)
	seen[""] = struct{}{}
	shortIDs = append(shortIDs, "")
	for _, value := range values {
		token := strings.TrimSpace(value)
		if _, ok := seen[token]; ok {
			continue
		}
		seen[token] = struct{}{}
		shortIDs = append(shortIDs, token)
	}
	return shortIDs
}

func buildXrayInboundConfig(config *pb.InboundConfig, certificates *CertificatesManager, portalUserTags map[string]string, matchedPortalUsers map[string]struct{}) (conf.InboundDetourConfig, error) {
	inbound := conf.InboundDetourConfig{
		Tag:      config.Name,
		ListenOn: &conf.Address{Address: xnet.ParseAddress(config.Listen)},
		PortList: &conf.PortList{
			Range: []conf.PortRange{
				{From: uint32(config.Port), To: uint32(config.Port)},
			},
		},
	}

	accounts := config.GetAccounts()
	if vlessCfg := config.GetVless(); vlessCfg != nil {
		inbound.StreamSetting = &conf.StreamConfig{
			Network: xrayVlessNetwork(vlessCfg.Transmission),
		}
	} else if hy2Cfg := config.GetHysteria2(); hy2Cfg != nil {
		masquerade, err := parseXrayMasquerade(hy2Cfg.Masquerade)
		if err != nil {
			return inbound, err
		}
		inbound.StreamSetting = &conf.StreamConfig{
			Network:  xrayHysteriaNetwork(),
			Security: "tls",
			HysteriaSettings: &conf.HysteriaConfig{
				Version:        2,
				Auth:           "password",
				UdpIdleTimeout: 60,
				Masquerade:     masquerade,
			},
		}
		if strings.TrimSpace(hy2Cfg.BbrProfile) != "" {
			inbound.StreamSetting.FinalMask = &conf.FinalMask{
				QuicParams: &conf.QuicParamsConfig{
					Congestion: "bbr",
					Debug:      hy2Cfg.BrutalDebug,
					BrutalUp:   hysteriaBandwidth(hy2Cfg.UpMbps),
					BrutalDown: hysteriaBandwidth(hy2Cfg.DownMbps),
				},
			}
		}
		if strings.TrimSpace(hy2Cfg.ObfsType) != "" && strings.EqualFold(strings.TrimSpace(hy2Cfg.ObfsType), "salamander") {
			maskSettings := toRaw(map[string]any{
				"password": hy2Cfg.ObfsPassword,
			})
			if inbound.StreamSetting.FinalMask == nil {
				inbound.StreamSetting.FinalMask = &conf.FinalMask{}
			}
			inbound.StreamSetting.FinalMask.Udp = []conf.Mask{{
				Type:     "salamander",
				Settings: &maskSettings,
			}}
		}
	}

	tlsConfig := inboundTLSConfig(config)
	tlsCertificate, err := resolveInboundTLSCertificate(tlsConfig, certificates)
	if err != nil {
		return inbound, err
	}
	switch config.Protocol.(type) {
	case *pb.InboundConfig_Vless:
		vlessCfg := config.Protocol.(*pb.InboundConfig_Vless).Vless
		if inbound.StreamSetting == nil {
			inbound.StreamSetting = &conf.StreamConfig{}
		}
		stream := inbound.StreamSetting
		stream.Network = xrayVlessNetwork(vlessCfg.Transmission)
		if vlessCfg.Security == pb.SecurityMode_TLS {
			if tlsConfig == nil || !tlsConfig.Enabled {
				return inbound, fmt.Errorf("vless tls requires tls to be enabled")
			}
			if tlsCertificate == nil || strings.TrimSpace(tlsCertificate.CertificatePath) == "" || strings.TrimSpace(tlsCertificate.KeyPath) == "" {
				return inbound, fmt.Errorf("vless tls requires tls certificate_name with valid certificate_path and key_path")
			}
			alpn := conf.StringList{"h2", "http/1.1"}
			if isXHTTPTransmission(vlessCfg.Transmission) {
				alpn = conf.StringList{"h3", "h2", "http/1.1"}
			}
			stream.Security = "tls"
			stream.TLSSettings = &conf.TLSConfig{
				ServerName: tlsConfig.ServerName,
				ALPN:       &alpn,
				Certs: []*conf.TLSCertConfig{{
					CertFile: tlsCertificate.CertificatePath,
					KeyFile:  tlsCertificate.KeyPath,
				}},
			}
		} else if vlessCfg.Security == pb.SecurityMode_REALITY {
			realityCfg := vlessCfg.Reality
			if realityCfg == nil {
				return inbound, fmt.Errorf("vless reality requires reality settings")
			}
			serverNames := normalizedXrayServerNames(realityCfg.Sni)
			if len(serverNames) == 0 && tlsConfig != nil {
				serverNames = normalizedXrayServerNames(tlsConfig.ServerName)
			}
			stream.Security = "reality"
			stream.REALITYSettings = &conf.REALITYConfig{
				Show:        false,
				Target:      toRaw(realityCfg.Dest),
				Dest:        toRaw(realityCfg.Dest),
				Xver:        0,
				ServerNames: serverNames,
				PrivateKey:  realityCfg.PrivateKey,
				ShortIds:    normalizedRealityShortIDs(realityCfg.ShortId),
				Fingerprint: realityCfg.Utls,
				SpiderX:     realityCfg.SpiderX,
			}
		}
	case *pb.InboundConfig_Hysteria2:
		if tlsConfig == nil || !tlsConfig.Enabled {
			return inbound, fmt.Errorf("hysteria2 requires tls to be enabled")
		}
		if inbound.StreamSetting == nil {
			inbound.StreamSetting = &conf.StreamConfig{}
		}
		stream := inbound.StreamSetting
		stream.Network = xrayHysteriaNetwork()
		if tlsCertificate == nil || strings.TrimSpace(tlsCertificate.CertificatePath) == "" || strings.TrimSpace(tlsCertificate.KeyPath) == "" {
			return inbound, fmt.Errorf("hysteria2 requires tls certificate_name with valid certificate_path and key_path")
		}
		stream.Security = "tls"
		stream.TLSSettings = &conf.TLSConfig{
			ServerName: tlsConfig.ServerName,
			ALPN:       &conf.StringList{"h3"},
			Certs: []*conf.TLSCertConfig{{
				CertFile: tlsCertificate.CertificatePath,
				KeyFile:  tlsCertificate.KeyPath,
			}},
		}
		stream.HysteriaSettings = &conf.HysteriaConfig{
			Version: 2,
		}
	}

	switch p := config.Protocol.(type) {
	case *pb.InboundConfig_Vless:
		inbound.Protocol = "vless"
		var clients []map[string]any
		for _, acc := range accounts {
			id := acc.GetId()
			if id == "" {
				continue
			}
			client := map[string]any{
				"id":    acc.Token,
				"email": id,
				"flow":  p.Vless.Flow,
			}
			if reverseTag, ok := portalUserTags[strings.TrimSpace(acc.Id)]; ok {
				client["reverse"] = map[string]any{"tag": reverseTag}
				matchedPortalUsers[strings.TrimSpace(acc.Id)] = struct{}{}
			}
			clients = append(clients, client)
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
			id := acc.GetId()
			if id == "" {
				continue
			}
			clients = append(clients, map[string]any{
				"auth":  strings.TrimSpace(acc.Token),
				"email": id,
				"level": 0,
			})
		}
		inbound.Settings = toRawPtr(map[string]any{
			"version": 2,
			"clients": clients,
		})
	case *pb.InboundConfig_Tunnel:
		inbound.Protocol = "tunnel"
		network := strings.TrimSpace(p.Tunnel.AllowedNetwork)
		if network == "" {
			network = "tcp"
		}
		inbound.Settings = toRawPtr(&conf.DokodemoConfig{
			Network: xrayNetworkList(network),
		})
	case *pb.InboundConfig_Socks5:
		inbound.Protocol = "socks"
		settings := map[string]any{
			"auth": "noauth",
			"udp":  p.Socks5.UdpEnabled,
		}
		accountsList := make([]map[string]any, 0, len(accounts))
		for _, acc := range accounts {
			id := acc.GetId()
			if id == "" {
				continue
			}
			accountsList = append(accountsList, map[string]any{
				"user": id,
				"pass": acc.Token,
			})
		}
		if len(accountsList) > 0 {
			settings["auth"] = "password"
			settings["accounts"] = accountsList
		} else if p.Socks5.Username != "" {
			settings["auth"] = "password"
			settings["accounts"] = []map[string]any{{
				"user": p.Socks5.Username,
				"pass": p.Socks5.Password,
			}}
		}
		inbound.Settings = toRawPtr(settings)
	case *pb.InboundConfig_Shadowsocks:
		inbound.Protocol = "shadowsocks"
		users := make([]map[string]any, 0, len(accounts))
		for _, acc := range accounts {
			id := acc.GetId()
			if id == "" {
				continue
			}
			password := strings.TrimSpace(acc.Token)
			if password == "" {
				password = strings.TrimSpace(p.Shadowsocks.Password)
			}
			users = append(users, map[string]any{
				"email":    id,
				"method":   p.Shadowsocks.Method,
				"password": password,
			})
		}
		settings := map[string]any{"network": "tcp"}
		if p.Shadowsocks.UdpEnabled {
			settings["network"] = "tcp,udp"
		}
		if len(users) > 0 {
			settings["clients"] = users
		} else {
			settings["method"] = p.Shadowsocks.Method
			settings["password"] = p.Shadowsocks.Password
		}
		inbound.Settings = toRawPtr(settings)
	case *pb.InboundConfig_Tproxy:
		inbound.Protocol = "dokodemo-door"
		network := strings.TrimSpace(p.Tproxy.Network)
		if network == "" {
			network = "tcp,udp"
		}
		inbound.Settings = toRawPtr(&conf.DokodemoConfig{
			Network:        xrayNetworkList(network),
			FollowRedirect: true,
		})
		inbound.StreamSetting = &conf.StreamConfig{
			SocketSettings: &conf.SocketConfig{
				TProxy: "tproxy",
				Mark:   p.Tproxy.SocketMark,
			},
		}
		if p.Tproxy.SniffingEnabled {
			destOverride := conf.StringList(p.Tproxy.SniffingDestOverride)
			if len(destOverride) == 0 {
				destOverride = conf.StringList{"http", "tls", "quic"}
			}
			inbound.SniffingConfig = &conf.SniffingConfig{
				Enabled:      true,
				DestOverride: &destOverride,
				RouteOnly:    p.Tproxy.SniffingRouteOnly,
			}
		}
	default:
		return inbound, fmt.Errorf("protocol not supported by Xray engine")
	}

	return inbound, nil
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

func buildXrayReverseConfig(configs []*pb.InboundConfig) (*conf.ReverseConfig, []json.RawMessage, map[string]struct{}, map[string]string, error) {
	reverseTags := map[string]struct{}{}
	portalUserTags := map[string]string{}

	for _, inbound := range configs {
		if inbound == nil {
			continue
		}
		r := inbound.GetVlessReverseProxy()
		if r == nil {
			continue
		}
		tag := strings.TrimSpace(r.Tag)
		if tag == "" {
			tag = strings.TrimSpace(inbound.Name)
		}
		if tag == "" {
			return nil, nil, nil, nil, fmt.Errorf("reverse proxy requires tag")
		}
		reverseTags[tag] = struct{}{}
		portalUserID := strings.TrimSpace(r.PortalUserId)
		if portalUserID == "" {
			return nil, nil, nil, nil, fmt.Errorf("vless reverse proxy %q requires portal_user_id", tag)
		}
		if existingTag, ok := portalUserTags[portalUserID]; ok && existingTag != tag {
			return nil, nil, nil, nil, fmt.Errorf("vless reverse proxy user %q already assigned to tag %q", portalUserID, existingTag)
		}
		portalUserTags[portalUserID] = tag
	}
	return nil, nil, reverseTags, portalUserTags, nil
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
		network = "xhttp"
	case "XHTTP":
		network = "xhttp"
	default:
		network = "tcp"
	}
	return &network
}

func isXHTTPTransmission(transmission string) bool {
	switch strings.TrimSpace(strings.ToUpper(transmission)) {
	case "SPLITHTTP", "XHTTP":
		return true
	default:
		return false
	}
}

func xrayHysteriaNetwork() *conf.TransportProtocol {
	network := conf.TransportProtocol("hysteria")
	return &network
}

func xrayNetworkList(value string) *conf.NetworkList {
	parts := strings.Split(value, ",")
	networks := make(conf.NetworkList, 0, len(parts))
	for _, part := range parts {
		part = strings.ToLower(strings.TrimSpace(part))
		switch part {
		case "tcp", "udp":
			networks = append(networks, conf.Network(part))
		}
	}
	if len(networks) == 0 {
		networks = conf.NetworkList{conf.Network("tcp"), conf.Network("udp")}
	}
	return &networks
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

func (e *XrayEngine) convertToConfig(configs []*pb.InboundConfig, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule, dnsConfig *pb.DnsConfig, certificates *CertificatesManager, apiPort int) (*core.Config, error) {
	c := &conf.Config{}

	c.LogConfig = &conf.LogConfig{
		LogLevel: logging.XrayLogLevel(),
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
	portalUserTags := map[string]string{}
	matchedPortalUsers := map[string]struct{}{}
	reverseConfig, reverseRules, reverseTags, portalAssignments, err := buildXrayReverseConfig(configs)
	if err != nil {
		return nil, err
	}
	portalUserTags = portalAssignments

	for _, config := range configs {
		if config == nil {
			continue
		}
		if config.GetVlessReverseProxy() != nil {
			continue
		}
		inbound, err := buildXrayInboundConfig(config, certificates, portalUserTags, matchedPortalUsers)
		if err != nil {
			return nil, err
		}
		c.InboundConfigs = append(c.InboundConfigs, inbound)
	}
	for portalUserID := range portalUserTags {
		if _, ok := matchedPortalUsers[portalUserID]; !ok {
			return nil, fmt.Errorf("reverse portal user %q requires at least one VLESS inbound with that account", portalUserID)
		}
	}

	allowedOutboundTags := map[string]struct{}{}
	c.Reverse = reverseConfig
	for tag := range reverseTags {
		allowedOutboundTags[tag] = struct{}{}
	}
	for _, out := range outbounds {
		if out == nil || strings.TrimSpace(out.Tag) == "" {
			continue
		}
		if out.Type == pb.OutboundType_CUSTOM {
			if out.GetCustom() == nil || strings.TrimSpace(out.GetCustom().HandlerName) == "" {
				continue
			}
			allowedOutboundTags[strings.TrimSpace(out.Tag)] = struct{}{}
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
					"id":         v.Uuid,
					"flow":       v.Flow,
					"encryption": "none",
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
				serverName := ""
				if tlsCfg := v.GetTls(); tlsCfg != nil {
					serverName = strings.TrimSpace(tlsCfg.ServerName)
				}
				alpn := conf.StringList{"h2", "http/1.1"}
				if isXHTTPTransmission(v.Transmission) {
					alpn = conf.StringList{"h3", "h2", "http/1.1"}
				}
				stream.Security = "tls"
				stream.TLSSettings = &conf.TLSConfig{
					AllowInsecure: true,
					ServerName:    serverName,
					ALPN:          &alpn,
				}
			} else if v.Security == pb.SecurityMode_REALITY {
				realityCfg := v.GetReality()
				if realityCfg == nil {
					continue
				}
				serverName := strings.TrimSpace(realityCfg.Sni)
				if serverName == "" {
					if tlsCfg := v.GetTls(); tlsCfg != nil {
						serverName = strings.TrimSpace(tlsCfg.ServerName)
					}
				}
				stream.Security = "reality"
				stream.REALITYSettings = &conf.REALITYConfig{
					Show:        false,
					Fingerprint: realityCfg.Utls,
					ServerName:  serverName,
					PublicKey:   strings.TrimSpace(realityCfg.PublicKey),
					ShortId:     strings.Join(normalizedRealityShortIDs(realityCfg.ShortId), ""),
					SpiderX:     realityCfg.SpiderX,
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
				Mtu:       int32(w.Mtu),
				NumWorkers: func() int32 {
					if w.Workers > 0 {
						return w.Workers
					}
					return 2
				}(),
				Reserved: reserved,
				DomainStrategy: func() string {
					switch w.DomainStrategy {
					case "ForceIPv4", "ForceIPv4v6", "ForceIPv6", "ForceIPv6v4":
						return w.DomainStrategy
					default:
						return "ForceIP"
					}
				}(),
				NoKernelTun: true,
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
		case pb.OutboundType_TRUSTTUNNEL:
			o.Protocol = "socks"
			o.Settings = toRawPtr(map[string]any{
				"servers": []map[string]any{
					{
						"address": "127.0.0.1",
						"port":    trustTunnelOutboundSocksPort(strings.TrimSpace(out.Tag)),
					},
				},
			})
		case pb.OutboundType_SHADOWSOCKS:
			o.Protocol = "shadowsocks"
			s := out.GetShadowsocks()
			server := map[string]any{
				"address":  s.Server,
				"port":     s.Port,
				"method":   s.Method,
				"password": s.Password,
			}
			if strings.TrimSpace(s.Plugin) != "" {
				server["plugin"] = s.Plugin
			}
			if pluginOpts := appendShadowsocksPrefix(s.PluginOpts, s.Prefix); pluginOpts != "" {
				server["plugin_opts"] = pluginOpts
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
	c.RouterConfig.RuleList = append(reverseRules, c.RouterConfig.RuleList...)

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
		logging.Debugf("[xray] generated config json=%s", string(dumped))
	} else {
		logging.Warnf("[xray] failed to dump config: %v", err)
	}

	return c.Build()
}

func (e *XrayEngine) GetMetrics(ctx context.Context) (*RuntimeMetrics, error) {
	e.mu.Lock()
	defer e.mu.Unlock()

	metrics := &RuntimeMetrics{}
	for _, inbound := range e.lastConfigs {
		if inbound == nil {
			continue
		}
		metrics.Inbounds = append(metrics.Inbounds, &pb.InboundStatus{
			Name:        inbound.Name,
			Traffic:     &pb.TrafficStats{},
			Connections: &pb.ConnectionStats{},
		})
	}

	if e.instance == nil {
		return metrics, nil
	}

	client, conn, err := e.statsClient(ctx)
	if err != nil {
		return metrics, nil
	}
	defer conn.Close()

	for _, inbound := range metrics.Inbounds {
		inbound.Traffic = querySingleStatPair(
			ctx,
			client,
			fmt.Sprintf("inbound>>>%s>>>traffic>>>downlink", inbound.Name),
			fmt.Sprintf("inbound>>>%s>>>traffic>>>uplink", inbound.Name),
		)
	}

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

	seenAccounts := make(map[string]struct{})
	for _, accounts := range e.lastAccounts {
		for accountID := range accounts {
			if _, ok := seenAccounts[accountID]; ok {
				continue
			}
			seenAccounts[accountID] = struct{}{}
			account := &pb.AccountStatus{
				Id: accountID,
				Traffic: querySingleStatPair(
					ctx,
					client,
					fmt.Sprintf("user>>>%s>>>traffic>>>downlink", accountID),
					fmt.Sprintf("user>>>%s>>>traffic>>>uplink", accountID),
				),
			}
			if _, ok := onlineUsers[accountID]; ok {
				account.Online = 1
				if onlineIps, err := client.GetStatsOnlineIpList(ctx, &statsService.GetStatsRequest{
					Name:   fmt.Sprintf("user>>>%s>>>online", accountID),
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
				Name:   fmt.Sprintf("user>>>%s>>>online", accountID),
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
				Name:   fmt.Sprintf("user>>>%s>>>online", accountID),
				Reset_: false,
			}); err == nil && online.GetStat() != nil && online.Stat.Value > 0 {
				account.Online = uint32(online.Stat.Value)
			}
			metrics.Accounts = append(metrics.Accounts, account)
		}
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
