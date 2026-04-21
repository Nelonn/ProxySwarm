package engine

import (
	"context"
	"encoding/json"
	"proxyswarm/node/internal/pb"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	gnet "github.com/shirou/gopsutil/v3/net"
	"google.golang.org/protobuf/proto"
)

const metricsSampleWindow = 2 * time.Second

type trafficTotals struct {
	Rx uint64
	Tx uint64
}

type inboundConnKinds struct {
	name string
	tcp  bool
	udp  bool
}

type RuntimeMetrics struct {
	Inbound   *pb.InboundStatus
	Accounts  []*pb.AccountStatus
	Outbounds []*pb.OutboundStatus
}

type persistedMetricsState struct {
	SampleWindowSeconds  uint32                        `json:"sample_window_seconds"`
	TotalInboundTraffic  *pb.TrafficStats              `json:"total_inbound_traffic"`
	TotalOutboundTraffic *pb.TrafficStats              `json:"total_outbound_traffic"`
	Connections          *pb.ConnectionStats           `json:"connections"`
	Inbounds             map[string]*pb.InboundStatus  `json:"inbounds"`
	Accounts             map[string]*pb.AccountStatus  `json:"accounts"`
	Outbounds            map[string]*pb.OutboundStatus `json:"outbounds"`
}

func newPersistedMetricsState() *persistedMetricsState {
	return &persistedMetricsState{
		SampleWindowSeconds:  uint32(metricsSampleWindow / time.Second),
		TotalInboundTraffic:  &pb.TrafficStats{},
		TotalOutboundTraffic: &pb.TrafficStats{},
		Connections:          &pb.ConnectionStats{},
		Inbounds:             make(map[string]*pb.InboundStatus),
		Accounts:             make(map[string]*pb.AccountStatus),
		Outbounds:            make(map[string]*pb.OutboundStatus),
	}
}

func loadPersistedMetrics(path string) (*persistedMetricsState, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	state := newPersistedMetricsState()
	if err := json.Unmarshal(data, state); err != nil {
		return nil, err
	}
	state.ensure()
	return state, nil
}

func (s *persistedMetricsState) ensure() {
	if s.SampleWindowSeconds == 0 {
		s.SampleWindowSeconds = uint32(metricsSampleWindow / time.Second)
	}
	if s.TotalInboundTraffic == nil {
		s.TotalInboundTraffic = &pb.TrafficStats{}
	}
	if s.TotalOutboundTraffic == nil {
		s.TotalOutboundTraffic = &pb.TrafficStats{}
	}
	if s.Connections == nil {
		s.Connections = &pb.ConnectionStats{}
	}
	if s.Inbounds == nil {
		s.Inbounds = make(map[string]*pb.InboundStatus)
	}
	if s.Accounts == nil {
		s.Accounts = make(map[string]*pb.AccountStatus)
	}
	if s.Outbounds == nil {
		s.Outbounds = make(map[string]*pb.OutboundStatus)
	}
	for name, inbound := range s.Inbounds {
		if inbound == nil {
			inbound = &pb.InboundStatus{Name: name}
			s.Inbounds[name] = inbound
		}
		if inbound.Name == "" {
			inbound.Name = name
		}
		if inbound.Traffic == nil {
			inbound.Traffic = &pb.TrafficStats{}
		}
		if inbound.Connections == nil {
			inbound.Connections = &pb.ConnectionStats{}
		}
	}
	for name, account := range s.Accounts {
		if account == nil {
			account = &pb.AccountStatus{Name: name}
			s.Accounts[name] = account
		}
		if account.Name == "" {
			account.Name = name
		}
		if account.Traffic == nil {
			account.Traffic = &pb.TrafficStats{}
		}
	}
	for name, outbound := range s.Outbounds {
		if outbound == nil {
			outbound = &pb.OutboundStatus{Name: name}
			s.Outbounds[name] = outbound
		}
		if outbound.Name == "" {
			outbound.Name = name
		}
		if outbound.Traffic == nil {
			outbound.Traffic = &pb.TrafficStats{}
		}
	}
}

func (m *Manager) runMetricsSampler() {
	ticker := time.NewTicker(metricsSampleWindow)
	defer ticker.Stop()

	m.sampleMetrics()
	for range ticker.C {
		m.sampleMetrics()
	}
}

func (m *Manager) sampleMetrics() {
	ctx, cancel := context.WithTimeout(context.Background(), metricsSampleWindow)
	defer cancel()

	m.mu.Lock()
	engines := make(map[string]Engine, len(m.engines))
	for name, engine := range m.engines {
		engines[name] = engine
	}
	var config *pb.FullConfig
	if m.currentConfig != nil {
		config, _ = protoCloneFullConfig(m.currentConfig)
	}
	m.metricsState.ensure()
	resetCurrentSampleLocked(m.metricsState)
	m.mu.Unlock()

	type sampledEngine struct {
		name    string
		metrics *RuntimeMetrics
	}
	snapshots := make([]sampledEngine, 0, len(engines))
	for name, engine := range engines {
		metrics, err := engine.GetMetrics(ctx)
		if err != nil || metrics == nil {
			continue
		}
		snapshots = append(snapshots, sampledEngine{name: name, metrics: metrics})
	}

	inboundConnections, totalConnections := sampleConnectionStats(config)

	m.mu.Lock()
	defer m.mu.Unlock()

	m.metricsState.ensure()
	resetCurrentSampleLocked(m.metricsState)
	seenRaw := make(map[string]struct{})

	windowSeconds := float64(metricsSampleWindow) / float64(time.Second)
	for _, snapshot := range snapshots {
		if inbound := snapshot.metrics.Inbound; inbound != nil {
			stateInbound := ensureInboundMetric(m.metricsState, inbound.Name)
			sourceKey := "inbound:" + snapshot.name
			seenRaw[sourceKey] = struct{}{}
			delta := applyTrafficDelta(m.lastRaw, sourceKey, inbound.Traffic, stateInbound.Traffic, windowSeconds)
			addTrafficDelta(m.metricsState.TotalInboundTraffic, delta, windowSeconds)
		}

		for _, account := range snapshot.metrics.Accounts {
			if account == nil {
				continue
			}
			stateAccount := ensureAccountMetric(m.metricsState, account.Name)
			stateAccount.Online += account.Online
			stateAccount.Sessions = cloneUserSessions(account.Sessions)
			sourceKey := "account:" + snapshot.name + ":" + account.Name
			seenRaw[sourceKey] = struct{}{}
			delta := applyTrafficDelta(m.lastRaw, sourceKey, account.Traffic, stateAccount.Traffic, windowSeconds)
			if stateAccount.Online == 0 && (delta.Rx > 0 || delta.Tx > 0) {
				stateAccount.Online = 1
			}
		}

		for _, outbound := range snapshot.metrics.Outbounds {
			if outbound == nil {
				continue
			}
			stateOutbound := ensureOutboundMetric(m.metricsState, outbound.Name)
			stateOutbound.Type = outbound.Type
			stateOutbound.ExcludedFromTotals = outbound.ExcludedFromTotals
			sourceKey := "outbound:" + snapshot.name + ":" + outbound.Name
			seenRaw[sourceKey] = struct{}{}
			delta := applyTrafficDelta(m.lastRaw, sourceKey, outbound.Traffic, stateOutbound.Traffic, windowSeconds)
			if !outbound.ExcludedFromTotals {
				addTrafficDelta(m.metricsState.TotalOutboundTraffic, delta, windowSeconds)
			}
		}
	}

	for key := range m.lastRaw {
		if _, ok := seenRaw[key]; !ok {
			delete(m.lastRaw, key)
		}
	}

	for inboundName, connections := range inboundConnections {
		stateInbound := ensureInboundMetric(m.metricsState, inboundName)
		stateInbound.Connections = cloneConnectionStats(connections)
	}
	m.metricsState.Connections = cloneConnectionStats(totalConnections)

	_ = m.savePersistedMetricsLocked()
}

func resetCurrentSampleLocked(state *persistedMetricsState) {
	state.TotalInboundTraffic.RxRate = 0
	state.TotalInboundTraffic.TxRate = 0
	state.TotalOutboundTraffic.RxRate = 0
	state.TotalOutboundTraffic.TxRate = 0
	state.Connections.Tcp = 0
	state.Connections.Udp = 0
	for _, inbound := range state.Inbounds {
		if inbound == nil {
			continue
		}
		if inbound.Traffic != nil {
			inbound.Traffic.RxRate = 0
			inbound.Traffic.TxRate = 0
		}
		if inbound.Connections != nil {
			inbound.Connections.Tcp = 0
			inbound.Connections.Udp = 0
		}
	}
	for _, account := range state.Accounts {
		if account == nil {
			continue
		}
		if account.Traffic != nil {
			account.Traffic.RxRate = 0
			account.Traffic.TxRate = 0
		}
		account.Online = 0
		account.Sessions = nil
	}
	for _, outbound := range state.Outbounds {
		if outbound == nil || outbound.Traffic == nil {
			continue
		}
		outbound.Traffic.RxRate = 0
		outbound.Traffic.TxRate = 0
	}
}

func applyTrafficDelta(lastRaw map[string]trafficTotals, sourceKey string, current *pb.TrafficStats, aggregate *pb.TrafficStats, windowSeconds float64) trafficTotals {
	if aggregate == nil {
		return trafficTotals{}
	}
	if current == nil {
		return trafficTotals{}
	}
	prev := lastRaw[sourceKey]
	delta := trafficTotals{
		Rx: current.Rx,
		Tx: current.Tx,
	}
	if current.Rx >= prev.Rx {
		delta.Rx = current.Rx - prev.Rx
	}
	if current.Tx >= prev.Tx {
		delta.Tx = current.Tx - prev.Tx
	}
	lastRaw[sourceKey] = trafficTotals{Rx: current.Rx, Tx: current.Tx}
	addTrafficDelta(aggregate, delta, windowSeconds)
	return delta
}

func addTrafficDelta(aggregate *pb.TrafficStats, delta trafficTotals, windowSeconds float64) {
	if aggregate == nil {
		return
	}
	aggregate.Rx += delta.Rx
	aggregate.Tx += delta.Tx
	aggregate.RxRate += float64(delta.Rx) / windowSeconds
	aggregate.TxRate += float64(delta.Tx) / windowSeconds
}

func ensureInboundMetric(state *persistedMetricsState, name string) *pb.InboundStatus {
	state.ensure()
	inbound := state.Inbounds[name]
	if inbound == nil {
		inbound = &pb.InboundStatus{
			Name:        name,
			Traffic:     &pb.TrafficStats{},
			Connections: &pb.ConnectionStats{},
		}
		state.Inbounds[name] = inbound
	}
	if inbound.Traffic == nil {
		inbound.Traffic = &pb.TrafficStats{}
	}
	if inbound.Connections == nil {
		inbound.Connections = &pb.ConnectionStats{}
	}
	return inbound
}

func ensureAccountMetric(state *persistedMetricsState, name string) *pb.AccountStatus {
	state.ensure()
	account := state.Accounts[name]
	if account == nil {
		account = &pb.AccountStatus{
			Name:    name,
			Traffic: &pb.TrafficStats{},
		}
		state.Accounts[name] = account
	}
	if account.Traffic == nil {
		account.Traffic = &pb.TrafficStats{}
	}
	return account
}

func ensureOutboundMetric(state *persistedMetricsState, name string) *pb.OutboundStatus {
	state.ensure()
	outbound := state.Outbounds[name]
	if outbound == nil {
		outbound = &pb.OutboundStatus{
			Name:    name,
			Traffic: &pb.TrafficStats{},
		}
		state.Outbounds[name] = outbound
	}
	if outbound.Traffic == nil {
		outbound.Traffic = &pb.TrafficStats{}
	}
	return outbound
}

func (m *Manager) savePersistedMetricsLocked() error {
	if m.metricsState == nil {
		return nil
	}
	data, err := json.MarshalIndent(m.metricsState, "", "  ")
	if err != nil {
		return err
	}
	dir := filepath.Dir(m.metricsPath)
	if dir != "." && dir != "" {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return err
		}
	}
	return os.WriteFile(m.metricsPath, data, 0o600)
}

func (m *Manager) inboundStatusesLocked() []*pb.InboundStatus {
	m.metricsState.ensure()
	names := make([]string, 0)
	if m.currentConfig != nil {
		for _, inbound := range m.currentConfig.Inbounds {
			if inbound == nil || !inbound.Enabled {
				continue
			}
			names = append(names, inbound.Name)
		}
	} else {
		for name := range m.metricsState.Inbounds {
			names = append(names, name)
		}
		sort.Strings(names)
	}
	result := make([]*pb.InboundStatus, 0, len(names))
	for _, name := range names {
		if inbound := m.metricsState.Inbounds[name]; inbound != nil {
			result = append(result, &pb.InboundStatus{
				Name:        inbound.Name,
				Traffic:     cloneTrafficStats(inbound.Traffic),
				Connections: cloneConnectionStats(inbound.Connections),
			})
		}
	}
	return result
}

func (m *Manager) accountStatusesLocked() []*pb.AccountStatus {
	m.metricsState.ensure()
	names := make([]string, 0)
	if m.currentConfig != nil {
		for _, account := range m.currentConfig.Accounts {
			if account == nil {
				continue
			}
			names = append(names, account.Name)
		}
	} else {
		for name := range m.metricsState.Accounts {
			names = append(names, name)
		}
		sort.Strings(names)
	}
	result := make([]*pb.AccountStatus, 0, len(names))
	for _, name := range names {
		if account := m.metricsState.Accounts[name]; account != nil {
			result = append(result, &pb.AccountStatus{
				Name:     account.Name,
				Traffic:  cloneTrafficStats(account.Traffic),
				Online:   account.Online,
				Sessions: cloneUserSessions(account.Sessions),
			})
		}
	}
	return result
}

func (m *Manager) outboundStatusesLocked() []*pb.OutboundStatus {
	m.metricsState.ensure()
	names := make([]string, 0)
	if m.currentConfig != nil {
		for _, outbound := range m.currentConfig.Outbounds {
			if outbound == nil {
				continue
			}
			names = append(names, outbound.Tag)
		}
	} else {
		for name := range m.metricsState.Outbounds {
			names = append(names, name)
		}
		sort.Strings(names)
	}
	result := make([]*pb.OutboundStatus, 0, len(names))
	for _, name := range names {
		if outbound := m.metricsState.Outbounds[name]; outbound != nil {
			result = append(result, &pb.OutboundStatus{
				Name:               outbound.Name,
				Type:               outbound.Type,
				Traffic:            cloneTrafficStats(outbound.Traffic),
				ExcludedFromTotals: outbound.ExcludedFromTotals,
			})
		}
	}
	return result
}

func cloneTrafficStats(in *pb.TrafficStats) *pb.TrafficStats {
	if in == nil {
		return &pb.TrafficStats{}
	}
	return &pb.TrafficStats{
		Rx:     in.Rx,
		Tx:     in.Tx,
		RxRate: in.RxRate,
		TxRate: in.TxRate,
	}
}

func cloneConnectionStats(in *pb.ConnectionStats) *pb.ConnectionStats {
	if in == nil {
		return &pb.ConnectionStats{}
	}
	return &pb.ConnectionStats{
		Tcp: in.Tcp,
		Udp: in.Udp,
	}
}

func cloneUserSessions(in []*pb.UserSessionStatus) []*pb.UserSessionStatus {
	if len(in) == 0 {
		return nil
	}
	out := make([]*pb.UserSessionStatus, 0, len(in))
	for _, session := range in {
		if session == nil {
			continue
		}
		out = append(out, &pb.UserSessionStatus{
			Ip:        session.Ip,
			UserAgent: session.UserAgent,
		})
	}
	return out
}

func protoCloneFullConfig(config *pb.FullConfig) (*pb.FullConfig, bool) {
	if config == nil {
		return nil, false
	}
	cloned, ok := proto.Clone(config).(*pb.FullConfig)
	return cloned, ok
}

func sampleConnectionStats(config *pb.FullConfig) (map[string]*pb.ConnectionStats, *pb.ConnectionStats) {
	perInbound := make(map[string]*pb.ConnectionStats)
	total := &pb.ConnectionStats{}
	if config == nil {
		return perInbound, total
	}

	ports := make(map[uint32][]inboundConnKinds)
	for _, inbound := range config.Inbounds {
		if inbound == nil || !inbound.Enabled {
			continue
		}
		entry := inboundConnKinds{name: inbound.Name}
		switch inbound.Protocol.(type) {
		case *pb.InboundConfig_Hysteria2, *pb.InboundConfig_Wireguard:
			entry.udp = true
		case *pb.InboundConfig_Socks5:
			entry.tcp = true
			if inbound.GetSocks5() != nil && inbound.GetSocks5().UdpEnabled {
				entry.udp = true
			}
		default:
			entry.tcp = true
		}
		ports[uint32(inbound.Port)] = append(ports[uint32(inbound.Port)], entry)
		perInbound[inbound.Name] = &pb.ConnectionStats{}
	}

	addTCPConnections(perInbound, total, ports)
	addUDPConnections(perInbound, total, ports)
	return perInbound, total
}

func addTCPConnections(perInbound map[string]*pb.ConnectionStats, total *pb.ConnectionStats, ports map[uint32][]inboundConnKinds) {
	conns, err := gnet.Connections("tcp")
	if err != nil {
		return
	}
	for _, conn := range conns {
		if conn.Status == "LISTEN" || conn.Laddr.Port == 0 {
			continue
		}
		if strings.TrimSpace(conn.Raddr.IP) == "" || conn.Raddr.Port == 0 {
			continue
		}
		for _, inbound := range ports[conn.Laddr.Port] {
			if !inbound.tcp {
				continue
			}
			perInbound[inbound.name].Tcp++
			total.Tcp++
		}
	}
}

func addUDPConnections(perInbound map[string]*pb.ConnectionStats, total *pb.ConnectionStats, ports map[uint32][]inboundConnKinds) {
	conns, err := gnet.Connections("udp")
	if err != nil {
		return
	}
	for _, conn := range conns {
		if conn.Laddr.Port == 0 {
			continue
		}
		if strings.TrimSpace(conn.Raddr.IP) == "" && conn.Raddr.Port == 0 {
			continue
		}
		for _, inbound := range ports[conn.Laddr.Port] {
			if !inbound.udp {
				continue
			}
			perInbound[inbound.name].Udp++
			total.Udp++
		}
	}
}
