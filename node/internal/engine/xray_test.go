package engine

import (
	"encoding/json"
	"testing"

	"proxyswarm/node/internal/pb"
)

func TestBuildXrayRoutingRulesSkipsRulesWithoutMatchers(t *testing.T) {
	rawRules := buildXrayRoutingRules([]*pb.RoutingRule{
		{OutboundTag: "direct"},
		{Domain: []string{"", "example.com"}, OutboundTag: " proxy "},
	})
	if got := len(rawRules); got != 1 {
		t.Fatalf("expected 1 routing rule, got %d", got)
	}

	var rule map[string]any
	if err := json.Unmarshal(rawRules[0], &rule); err != nil {
		t.Fatalf("failed to decode routing rule: %v", err)
	}
	if rule["outboundTag"] != "proxy" {
		t.Fatalf("expected trimmed outbound tag, got %#v", rule["outboundTag"])
	}
	domain, ok := rule["domain"].([]any)
	if !ok || len(domain) != 1 || domain[0] != "example.com" {
		t.Fatalf("expected normalized domain matcher, got %#v", rule["domain"])
	}
}

func TestBuildXrayRoutingRulesUsesTransportField(t *testing.T) {
	rawRules := buildXrayRoutingRules([]*pb.RoutingRule{
		{
			InboundTag:  []string{"vless"},
			Transport:   []string{"tcp, udp"},
			OutboundTag: "warp",
		},
	})
	if got := len(rawRules); got != 1 {
		t.Fatalf("expected 1 routing rule, got %d", got)
	}

	var rule map[string]any
	if err := json.Unmarshal(rawRules[0], &rule); err != nil {
		t.Fatalf("failed to decode routing rule: %v", err)
	}

	if got := rule["network"]; got != "tcp,udp" {
		t.Fatalf("expected network=tcp,udp, got %#v", got)
	}
	if _, ok := rule["protocol"]; ok {
		t.Fatalf("expected protocol matcher to be omitted for tcp/udp, got %#v", rule["protocol"])
	}
}

func TestBuildXrayRoutingRulesKeepsProtocolAndTransportMatchers(t *testing.T) {
	rawRules := buildXrayRoutingRules([]*pb.RoutingRule{
		{
			Protocol:    []string{"http"},
			Transport:   []string{"udp", "tcp"},
			OutboundTag: "proxy",
		},
	})
	if got := len(rawRules); got != 1 {
		t.Fatalf("expected 1 routing rule, got %d", got)
	}

	var rule map[string]any
	if err := json.Unmarshal(rawRules[0], &rule); err != nil {
		t.Fatalf("failed to decode routing rule: %v", err)
	}

	protocol, ok := rule["protocol"].([]any)
	if !ok || len(protocol) != 1 || protocol[0] != "http" {
		t.Fatalf("expected protocol matcher to keep non-transport values, got %#v", rule["protocol"])
	}
	if got := rule["network"]; got != "udp,tcp" {
		t.Fatalf("expected network=udp,tcp, got %#v", got)
	}
}

func TestBuildXrayRoutingRulesNoLegacyProtocolToNetworkMapping(t *testing.T) {
	rawRules := buildXrayRoutingRules([]*pb.RoutingRule{
		{
			Protocol:    []string{"tcp,udp"},
			OutboundTag: "proxy",
		},
	})
	if got := len(rawRules); got != 1 {
		t.Fatalf("expected 1 routing rule, got %d", got)
	}

	var rule map[string]any
	if err := json.Unmarshal(rawRules[0], &rule); err != nil {
		t.Fatalf("failed to decode routing rule: %v", err)
	}
	if _, ok := rule["network"]; ok {
		t.Fatalf("expected network matcher to require transport field, got %#v", rule["network"])
	}
	if got := rule["protocol"]; got == nil {
		t.Fatalf("expected protocol matcher to keep protocol field value")
	}
}

func TestBuildXrayDNSConfig(t *testing.T) {
	disableCache := true
	serveStale := true
	serveExpired := uint32(300)
	dns, err := buildXrayDNSConfig(&pb.DnsConfig{
		Servers: []*pb.DnsServerConfig{
			{
				Address:         "1.1.1.1",
				Port:            53,
				Domains:         []string{" geosite:google ", ""},
				ExpectIps:       []string{" geoip:private ", ""},
				UnexpectedIps:   []string{" geoip:cn "},
				DisableCache:    &disableCache,
				ServeStale:      &serveStale,
				ServeExpiredTtl: &serveExpired,
			},
		},
		Hosts: []*pb.DnsHostMapping{
			{
				Domain: "example.com",
				Values: []string{"1.2.3.4", "5.6.7.8"},
			},
		},
		QueryStrategy: "UseIP",
	})
	if err != nil {
		t.Fatalf("expected dns config to build: %v", err)
	}
	if dns == nil {
		t.Fatal("expected non-nil dns config")
	}
	if got := len(dns.Servers); got != 1 {
		t.Fatalf("expected one server, got %d", got)
	}
	if got := dns.Servers[0].ExpectedIPs; len(got) != 0 {
		t.Fatalf("expected legacy expectedIPs to be empty, got %#v", got)
	}
	if got := dns.Servers[0].ExpectIPs; len(got) != 1 || got[0] != "geoip:private" {
		t.Fatalf("expected expectIPs to be normalized, got %#v", got)
	}
	if dns.Hosts == nil || len(dns.Hosts.Hosts) != 1 {
		t.Fatalf("expected one static host mapping, got %#v", dns.Hosts)
	}
}

func TestBuildXrayDNSConfigRejectsInvalidServerPort(t *testing.T) {
	_, err := buildXrayDNSConfig(&pb.DnsConfig{
		Servers: []*pb.DnsServerConfig{
			{Address: "8.8.8.8", Port: 70000},
		},
	})
	if err == nil {
		t.Fatal("expected invalid DNS port to fail")
	}
}

func TestXrayNeedsRestartForConfigShapeChanges(t *testing.T) {
	engine := NewXrayEngine("main")
	baseConfig := &pb.InboundConfig{
		Name:    "main",
		Listen:  "0.0.0.0",
		Port:    443,
		Enabled: true,
		Protocol: &pb.InboundConfig_Vless{
			Vless: &pb.VlessConfig{Transmission: "TCP"},
		},
	}
	baseOutbounds := []*pb.OutboundConfig{
		{Tag: "direct", Type: pb.OutboundType_DIRECT},
	}
	baseRules := []*pb.RoutingRule{
		{OutboundTag: "direct", Domain: []string{"example.com"}},
	}
	baseDNS := &pb.DnsConfig{
		Servers: []*pb.DnsServerConfig{{Address: "1.1.1.1", Port: 53}},
	}

	engine.lastConfigs = cloneProtoSlice([]*pb.InboundConfig{baseConfig})
	engine.lastDnsConfig = cloneProtoMessage(baseDNS)
	engine.lastOutboundsSnapshot = cloneProtoSlice(baseOutbounds)
	engine.lastRules = cloneProtoSlice(baseRules)

	if engine.needsRestart([]*pb.InboundConfig{baseConfig}, baseOutbounds, baseRules, baseDNS) {
		t.Fatal("expected no restart when xray-relevant config is unchanged")
	}

	changedConfig := cloneProtoMessage(baseConfig)
	changedConfig.Port = 8443
	if !engine.needsRestart([]*pb.InboundConfig{changedConfig}, baseOutbounds, baseRules, baseDNS) {
		t.Fatal("expected restart when inbound config changes")
	}

	changedOutbounds := cloneProtoSlice(baseOutbounds)
	changedOutbounds[0].Tag = "proxy"
	if !engine.needsRestart([]*pb.InboundConfig{baseConfig}, changedOutbounds, baseRules, baseDNS) {
		t.Fatal("expected restart when outbounds change")
	}

	changedRules := cloneProtoSlice(baseRules)
	changedRules[0].OutboundTag = "proxy"
	if !engine.needsRestart([]*pb.InboundConfig{baseConfig}, baseOutbounds, changedRules, baseDNS) {
		t.Fatal("expected restart when routing rules change")
	}

	changedDNS := cloneProtoMessage(baseDNS)
	changedDNS.Servers[0].Address = "8.8.8.8"
	if !engine.needsRestart([]*pb.InboundConfig{baseConfig}, baseOutbounds, baseRules, changedDNS) {
		t.Fatal("expected restart when dns config changes")
	}
}

func TestBuildXrayInboundConfigTunnel(t *testing.T) {
	config := &pb.InboundConfig{
		Name:    "reverse-25565",
		Listen:  "0.0.0.0",
		Port:    25565,
		Enabled: true,
		Protocol: &pb.InboundConfig_Tunnel{
			Tunnel: &pb.TunnelConfig{
				AllowedNetwork: "tcp,udp",
			},
		},
	}
	certs := NewCertificatesManager()
	inbound, err := buildXrayInboundConfig(config, certs, map[string]string{}, map[string]struct{}{})
	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}
	if inbound.Protocol != "tunnel" {
		t.Fatalf("expected protocol tunnel, got %q", inbound.Protocol)
	}
	if inbound.Settings == nil {
		t.Fatal("expected tunnel settings to be present")
	}
}

func TestBuildXrayReverseConfigPortalUsesUserReverse(t *testing.T) {
	reverseConfig, rules, reverseTags, portalUserTags, err := buildXrayReverseConfig([]*pb.InboundConfig{{
		Name: "reverse-portal",
		Protocol: &pb.InboundConfig_VlessReverseProxy{VlessReverseProxy: &pb.VlessReverseProxyConfig{
			Tag:              "r-outbound",
			PortalInboundTag: "portal",
			PortalUserId:     "ac40ca0f",
		}},
	}})
	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}
	if reverseConfig != nil {
		t.Fatalf("expected no top-level reverse config for portal mode, got %#v", reverseConfig)
	}
	if len(rules) != 0 {
		t.Fatalf("expected no auto-generated routing rules, got %d", len(rules))
	}
	if _, ok := reverseTags["r-outbound"]; !ok {
		t.Fatal("expected reverse tag to be allowed")
	}
	if got := portalUserTags["ac40ca0f"]; got != "r-outbound" {
		t.Fatalf("expected portal user tag mapping, got %q", got)
	}
}
