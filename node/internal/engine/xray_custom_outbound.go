package engine

import (
	"context"
	"encoding/json"
	"fmt"
	stdnet "net"
	"strconv"
	"strings"
	"sync"

	"proxyswarm/node/internal/logging"
	"proxyswarm/node/internal/pb"

	"github.com/xtls/xray-core/common"
	"github.com/xtls/xray-core/common/buf"
	xnet "github.com/xtls/xray-core/common/net"
	"github.com/xtls/xray-core/common/serial"
	"github.com/xtls/xray-core/common/session"
	"github.com/xtls/xray-core/common/task"
	"github.com/xtls/xray-core/core"
	featureoutbound "github.com/xtls/xray-core/features/outbound"
	"github.com/xtls/xray-core/transport"
)

type XrayCustomOutboundSpec struct {
	Tag        string
	Config     *pb.CustomOutboundConfig
	Raw        *pb.OutboundConfig
	Instance   *core.Instance
	Outbounds  []*pb.OutboundConfig
	RouteRules []*pb.RoutingRule
}

type XrayCustomOutboundFactory func(spec XrayCustomOutboundSpec) (featureoutbound.Handler, error)

var (
	xrayCustomOutboundRegistryMu sync.RWMutex
	xrayCustomOutboundRegistry   = map[string]XrayCustomOutboundFactory{}
)

func RegisterXrayCustomOutbound(name string, factory XrayCustomOutboundFactory) {
	name = strings.TrimSpace(strings.ToLower(name))
	if name == "" {
		panic("xray custom outbound name cannot be empty")
	}
	if factory == nil {
		panic("xray custom outbound factory cannot be nil")
	}
	xrayCustomOutboundRegistryMu.Lock()
	defer xrayCustomOutboundRegistryMu.Unlock()
	xrayCustomOutboundRegistry[name] = factory
}

func lookupXrayCustomOutbound(name string) XrayCustomOutboundFactory {
	xrayCustomOutboundRegistryMu.RLock()
	defer xrayCustomOutboundRegistryMu.RUnlock()
	return xrayCustomOutboundRegistry[strings.TrimSpace(strings.ToLower(name))]
}

func registerXrayCustomOutbounds(instance *core.Instance, outbounds []*pb.OutboundConfig, rules []*pb.RoutingRule) error {
	if instance == nil {
		return fmt.Errorf("xray instance is nil")
	}
	manager, ok := instance.GetFeature(featureoutbound.ManagerType()).(featureoutbound.Manager)
	if !ok || manager == nil {
		return fmt.Errorf("xray outbound manager is unavailable")
	}
	for _, outbound := range outbounds {
		if outbound == nil || outbound.GetType() != pb.OutboundType_CUSTOM {
			continue
		}
		tag := strings.TrimSpace(outbound.GetTag())
		if tag == "" {
			return fmt.Errorf("custom outbound requires tag")
		}
		cfg := outbound.GetCustom()
		if cfg == nil {
			return fmt.Errorf("custom outbound %q requires custom settings", tag)
		}
		handlerName := strings.TrimSpace(cfg.GetHandlerName())
		factory := lookupXrayCustomOutbound(handlerName)
		if factory == nil {
			return fmt.Errorf("custom outbound %q references unknown handler %q", tag, handlerName)
		}
		if existing := manager.GetHandler(tag); existing != nil {
			return fmt.Errorf("custom outbound tag %q already exists", tag)
		}
		handler, err := factory(XrayCustomOutboundSpec{
			Tag:        tag,
			Config:     cfg,
			Raw:        outbound,
			Instance:   instance,
			Outbounds:  outbounds,
			RouteRules: rules,
		})
		if err != nil {
			return fmt.Errorf("custom outbound %q: %w", tag, err)
		}
		if err := manager.AddHandler(context.Background(), handler); err != nil {
			return fmt.Errorf("failed to register custom outbound %q: %w", tag, err)
		}
	}
	return nil
}

type redirectCustomOutboundConfig struct {
	Address string `json:"address"`
	Port    int    `json:"port"`
	Network string `json:"network,omitempty"`
}

type redirectCustomOutbound struct {
	tag    string
	config redirectCustomOutboundConfig
}

func newRedirectCustomOutbound(spec XrayCustomOutboundSpec) (featureoutbound.Handler, error) {
	var cfg redirectCustomOutboundConfig
	raw := strings.TrimSpace(spec.Config.GetConfigJson())
	if raw != "" {
		if err := json.Unmarshal([]byte(raw), &cfg); err != nil {
			return nil, fmt.Errorf("invalid redirect config_json: %w", err)
		}
	}
	if strings.TrimSpace(cfg.Network) != "" {
		switch strings.ToLower(strings.TrimSpace(cfg.Network)) {
		case "tcp", "udp":
		default:
			return nil, fmt.Errorf("redirect network must be tcp or udp")
		}
	}
	return &redirectCustomOutbound{tag: spec.Tag, config: cfg}, nil
}

func (o *redirectCustomOutbound) Tag() string {
	return o.tag
}

func (o *redirectCustomOutbound) Dispatch(ctx context.Context, link *transport.Link) {
	if err := o.dispatch(ctx, link); err != nil {
		logging.Warnf("[xray custom outbound:%s] %v", o.tag, err)
		common.Interrupt(link.Writer)
		common.Interrupt(link.Reader)
	}
}

func (o *redirectCustomOutbound) dispatch(ctx context.Context, link *transport.Link) error {
	requestedDest := currentOutboundTarget(ctx)
	destination, err := o.resolveDestination(requestedDest)
	if err != nil {
		return err
	}
	address := destination.Address.String()
	if destination.Address.Family().IsDomain() {
		address = destination.Address.Domain()
	}
	conn, err := stdnet.Dial(destination.Network.SystemString(), stdnet.JoinHostPort(address, strconv.Itoa(int(destination.Port))))
	if err != nil {
		return fmt.Errorf("dial %s: %w", destination.String(), err)
	}
	defer conn.Close()

	requestDone := func() error {
		var writer buf.Writer
		if destination.Network == xnet.Network_TCP {
			writer = buf.NewWriter(conn)
		} else {
			writer = &buf.SequentialWriter{Writer: conn}
		}
		if err := buf.Copy(link.Reader, writer); err != nil {
			return fmt.Errorf("failed to process request: %w", err)
		}
		return nil
	}
	responseDone := func() error {
		var reader buf.Reader
		if destination.Network == xnet.Network_TCP {
			reader = buf.NewReader(conn)
		} else {
			reader = buf.NewPacketReader(conn)
		}
		if err := buf.Copy(reader, link.Writer); err != nil {
			return fmt.Errorf("failed to process response: %w", err)
		}
		return nil
	}
	if err := task.Run(ctx, requestDone, task.OnSuccess(responseDone, task.Close(link.Writer))); err != nil {
		return fmt.Errorf("connection ends: %w", err)
	}
	return nil
}

func (o *redirectCustomOutbound) resolveDestination(requested xnet.Destination) (xnet.Destination, error) {
	network := requested.Network
	if override := strings.ToLower(strings.TrimSpace(o.config.Network)); override != "" {
		switch override {
		case "tcp":
			network = xnet.Network_TCP
		case "udp":
			network = xnet.Network_UDP
		}
	}
	if network != xnet.Network_TCP && network != xnet.Network_UDP {
		return xnet.Destination{}, fmt.Errorf("redirect outbound requires tcp or udp destination")
	}
	address := strings.TrimSpace(o.config.Address)
	if address == "" && requested.Address != nil {
		address = requested.Address.String()
		if requested.Address.Family().IsDomain() {
			address = requested.Address.Domain()
		}
	}
	if address == "" {
		return xnet.Destination{}, fmt.Errorf("redirect outbound requires address")
	}
	port := o.config.Port
	if port <= 0 {
		port = int(requested.Port)
	}
	if port <= 0 || port > 65535 {
		return xnet.Destination{}, fmt.Errorf("redirect outbound requires valid port")
	}
	var parsedAddress xnet.Address
	if ip := stdnet.ParseIP(address); ip != nil {
		parsedAddress = xnet.IPAddress(ip)
	} else {
		parsedAddress = xnet.DomainAddress(address)
	}
	return xnet.Destination{
		Network: network,
		Address: parsedAddress,
		Port:    xnet.Port(port),
	}, nil
}

func (o *redirectCustomOutbound) Start() error { return nil }

func (o *redirectCustomOutbound) Close() error { return nil }

func (o *redirectCustomOutbound) SenderSettings() *serial.TypedMessage { return nil }

func (o *redirectCustomOutbound) ProxySettings() *serial.TypedMessage { return nil }

func currentOutboundTarget(ctx context.Context) xnet.Destination {
	outs := session.OutboundsFromContext(ctx)
	if len(outs) == 0 || outs[len(outs)-1] == nil {
		return xnet.Destination{}
	}
	return outs[len(outs)-1].Target
}

func init() {
	RegisterXrayCustomOutbound("redirect", newRedirectCustomOutbound)
}
