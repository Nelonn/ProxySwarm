package engine

import (
	"context"
	"crypto/ecdsa"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"encoding/base64"
	"encoding/json"
	"encoding/pem"
	"errors"
	"fmt"
	"io"
	"math/big"
	"net"
	"net/netip"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	"proxyswarm/node/internal/logging"

	connectip "github.com/inipew/connect-ip-go"
	"github.com/inipew/usque/masque"
	"github.com/sagernet/gvisor/pkg/buffer"
	"github.com/sagernet/gvisor/pkg/tcpip"
	"github.com/sagernet/gvisor/pkg/tcpip/adapters/gonet"
	"github.com/sagernet/gvisor/pkg/tcpip/header"
	"github.com/sagernet/gvisor/pkg/tcpip/link/channel"
	"github.com/sagernet/gvisor/pkg/tcpip/network/ipv4"
	"github.com/sagernet/gvisor/pkg/tcpip/network/ipv6"
	"github.com/sagernet/gvisor/pkg/tcpip/stack"
	"github.com/sagernet/gvisor/pkg/tcpip/transport/icmp"
	"github.com/sagernet/gvisor/pkg/tcpip/transport/tcp"
	"github.com/sagernet/gvisor/pkg/tcpip/transport/udp"
	"github.com/sagernet/quic-go"
	"github.com/xtls/xray-core/common"
	"github.com/xtls/xray-core/common/buf"
	xnet "github.com/xtls/xray-core/common/net"
	"github.com/xtls/xray-core/common/serial"
	"github.com/xtls/xray-core/common/task"
	featureoutbound "github.com/xtls/xray-core/features/outbound"
	"github.com/xtls/xray-core/transport"
	"golang.zx2c4.com/wireguard/tun"
)

const (
	usqueMasqueHandlerName = "usque-masque"
	usqueMasqueDefaultSNI  = "consumer-masque.cloudflareclient.com"
	usqueMasqueDefaultURI  = "https://cloudflareaccess.com"
	usqueMasqueDefaultMTU  = 1280
)

type usqueMasqueConfig struct {
	PrivateKey     string `json:"private_key"`
	EndpointPubKey string `json:"endpoint_pub_key"`
	Endpoint       string `json:"endpoint,omitempty"`
	EndpointV4     string `json:"endpoint_v4,omitempty"`
	EndpointV4Port int    `json:"endpoint_v4_port,omitempty"`
	EndpointV6     string `json:"endpoint_v6,omitempty"`
	EndpointH2V4   string `json:"endpoint_h2_v4,omitempty"`
	EndpointH2V6   string `json:"endpoint_h2_v6,omitempty"`
	HTTPVersion    string `json:"http_version,omitempty"`
	UseHTTP2       bool   `json:"use_http2,omitempty"`
	SNI            string `json:"sni,omitempty"`
	ConnectURI     string `json:"connect_uri,omitempty"`
	IPv4           string `json:"ipv4,omitempty"`
	IPv6           string `json:"ipv6,omitempty"`
	MTU            int    `json:"mtu,omitempty"`
	Insecure       bool   `json:"insecure,omitempty"`
	AccessToken    string `json:"access_token,omitempty"`
	ID             string `json:"id,omitempty"`
	License        string `json:"license,omitempty"`
}

type usqueMasqueOutbound struct {
	tag       string
	config    usqueMasqueConfig
	endpoint  net.Addr
	tlsConfig *tls.Config
	netstack  *usqueMasqueNet

	ctx    context.Context
	cancel context.CancelFunc
	once   sync.Once
}

func newUsqueMasqueOutbound(spec XrayCustomOutboundSpec) (featureoutbound.Handler, error) {
	var cfg usqueMasqueConfig
	raw := strings.TrimSpace(spec.Config.GetConfigJson())
	if raw == "" {
		return nil, fmt.Errorf("missing config_json")
	}
	if err := json.Unmarshal([]byte(raw), &cfg); err != nil {
		return nil, fmt.Errorf("invalid usque masque config_json: %w", err)
	}
	if cfg.MTU <= 0 {
		cfg.MTU = usqueMasqueDefaultMTU
	}
	if cfg.ConnectURI == "" {
		cfg.ConnectURI = usqueMasqueDefaultURI
	}
	if cfg.SNI == "" {
		cfg.SNI = usqueMasqueDefaultSNI
	}

	privKey, err := parseUsqueECPrivateKey(cfg.PrivateKey)
	if err != nil {
		return nil, err
	}
	peerPubKey, err := parseUsqueECPublicKey(cfg.EndpointPubKey)
	if err != nil {
		return nil, err
	}
	cert, err := generateUsqueSelfSignedCert(privKey)
	if err != nil {
		return nil, fmt.Errorf("generate client certificate: %w", err)
	}
	tlsConfig, err := masque.PrepareTlsConfig(privKey, peerPubKey, cert, cfg.SNI, cfg.Insecure)
	if err != nil {
		return nil, fmt.Errorf("prepare tls config: %w", err)
	}

	endpoint, err := resolveUsqueMasqueEndpoint(cfg)
	if err != nil {
		return nil, err
	}
	localAddresses, err := parseUsqueMasqueLocalAddresses(cfg)
	if err != nil {
		return nil, err
	}
	netstack, err := newUsqueMasqueNet(localAddresses, cfg.MTU)
	if err != nil {
		return nil, err
	}

	ctx, cancel := context.WithCancel(context.Background())
	return &usqueMasqueOutbound{
		tag:       spec.Tag,
		config:    cfg,
		endpoint:  endpoint,
		tlsConfig: tlsConfig,
		netstack:  netstack,
		ctx:       ctx,
		cancel:    cancel,
	}, nil
}

func (o *usqueMasqueOutbound) Tag() string { return o.tag }

func (o *usqueMasqueOutbound) Start() error {
	o.once.Do(func() {
		go o.maintainTunnel()
	})
	return nil
}

func (o *usqueMasqueOutbound) Close() error {
	o.cancel()
	return o.netstack.Close()
}

func (o *usqueMasqueOutbound) SenderSettings() *serial.TypedMessage { return nil }

func (o *usqueMasqueOutbound) ProxySettings() *serial.TypedMessage { return nil }

func (o *usqueMasqueOutbound) Dispatch(ctx context.Context, link *transport.Link) {
	if err := o.dispatch(ctx, link); err != nil {
		logging.Warnf("[xray custom outbound:%s] %v", o.tag, err)
		common.Interrupt(link.Writer)
		common.Interrupt(link.Reader)
	}
}

func (o *usqueMasqueOutbound) dispatch(ctx context.Context, link *transport.Link) error {
	destination := currentOutboundTarget(ctx)
	if destination.Network != xnet.Network_TCP && destination.Network != xnet.Network_UDP {
		return fmt.Errorf("usque masque requires tcp or udp destination")
	}
	if destination.Address == nil || destination.Address.Family().IsDomain() {
		return fmt.Errorf("usque masque requires IP destination")
	}
	addr := netip.AddrFrom4([4]byte{})
	ip := net.ParseIP(destination.Address.String())
	if ip == nil {
		return fmt.Errorf("invalid destination IP %q", destination.Address.String())
	}
	if v4 := ip.To4(); v4 != nil {
		addr = netip.AddrFrom4([4]byte(v4))
	} else {
		v16 := ip.To16()
		if v16 == nil {
			return fmt.Errorf("invalid destination IP %q", destination.Address.String())
		}
		addr = netip.AddrFrom16([16]byte(v16))
	}
	addrPort := netip.AddrPortFrom(addr, uint16(destination.Port))

	var conn net.Conn
	var err error
	if destination.Network == xnet.Network_TCP {
		conn, err = o.netstack.DialContextTCPAddrPort(ctx, addrPort)
	} else {
		conn, err = o.netstack.DialUDPAddrPort(netip.AddrPort{}, addrPort)
	}
	if err != nil {
		return fmt.Errorf("dial %s through masque: %w", destination.String(), err)
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

func (o *usqueMasqueOutbound) maintainTunnel() {
	for o.ctx.Err() == nil {
		packetConn, tr, ipConn, rsp, err := masque.ConnectTunnel(o.ctx, masque.Config{
			TLSConfig:  o.tlsConfig,
			QuicConfig: &quic.Config{EnableDatagrams: true, KeepAlivePeriod: 30 * time.Second},
			ConnectURI: o.config.ConnectURI,
			Endpoint:   o.endpoint,
			UseHTTP2:   o.config.useHTTP2(),
		})
		if err != nil {
			logging.Warnf("[xray custom outbound:%s] connect masque tunnel: %v", o.tag, err)
			sleepContext(o.ctx, 3*time.Second)
			continue
		}
		if rsp == nil || rsp.StatusCode != 200 {
			status := "missing response"
			if rsp != nil {
				status = rsp.Status
			}
			logging.Warnf("[xray custom outbound:%s] masque tunnel rejected: %s", o.tag, status)
			closeUsqueTunnel(packetConn, tr, ipConn)
			sleepContext(o.ctx, 3*time.Second)
			continue
		}
		logging.Infof("[xray custom outbound:%s] masque tunnel connected to %s", o.tag, o.endpoint.String())
		o.pumpTunnel(packetConn, tr, ipConn)
		sleepContext(o.ctx, 1*time.Second)
	}
}

func (o *usqueMasqueOutbound) pumpTunnel(packetConn net.PacketConn, tr interface{ Close() error }, ipConn *connectip.Conn) {
	tunnelCtx, tunnelCancel := context.WithCancel(o.ctx)
	defer tunnelCancel()
	var wg sync.WaitGroup
	errCh := make(chan error, 2)
	reportErr := func(err error) {
		select {
		case errCh <- err:
		default:
		}
	}

	wg.Add(1)
	go func() {
		defer wg.Done()
		buf := make([]byte, o.config.MTU)
		for tunnelCtx.Err() == nil {
			n, err := o.netstack.ReadPacket(tunnelCtx, buf)
			if err != nil {
				reportErr(err)
				return
			}
			icmpPacket, err := ipConn.WritePacket(buf[:n])
			if err != nil {
				reportErr(err)
				return
			}
			if len(icmpPacket) > 0 {
				_ = o.netstack.WritePacket(icmpPacket)
			}
		}
	}()
	wg.Add(1)
	go func() {
		defer wg.Done()
		buf := make([]byte, o.config.MTU)
		for tunnelCtx.Err() == nil {
			n, err := ipConn.ReadPacket(buf, true)
			if err != nil {
				reportErr(err)
				return
			}
			if err := o.netstack.WritePacket(buf[:n]); err != nil {
				reportErr(err)
				return
			}
		}
	}()

	select {
	case <-o.ctx.Done():
	case err := <-errCh:
		if err != nil && !errors.Is(err, net.ErrClosed) && !errors.Is(err, io.ErrClosedPipe) {
			logging.Warnf("[xray custom outbound:%s] masque tunnel closed: %v", o.tag, err)
		}
	}
	tunnelCancel()
	closeUsqueTunnel(packetConn, tr, ipConn)
	wg.Wait()
}

func (cfg usqueMasqueConfig) useHTTP2() bool {
	switch strings.ToUpper(strings.TrimSpace(cfg.HTTPVersion)) {
	case "HTTP/2", "H2":
		return true
	case "HTTP/3", "H3":
		return false
	default:
		return cfg.UseHTTP2
	}
}

func resolveUsqueMasqueEndpoint(cfg usqueMasqueConfig) (net.Addr, error) {
	port := cfg.EndpointV4Port
	if port <= 0 {
		port = 443
	}
	candidates := []string{cfg.Endpoint}
	if cfg.useHTTP2() {
		candidates = append(candidates, cfg.EndpointH2V4, cfg.EndpointH2V6)
	}
	candidates = append(candidates, cfg.EndpointV4, cfg.EndpointV6)
	host := ""
	for _, candidate := range candidates {
		if trimmed := strings.TrimSpace(candidate); trimmed != "" {
			host = trimmed
			break
		}
	}
	if host == "" {
		return nil, fmt.Errorf("usque masque endpoint is required")
	}
	if parsedHost, parsedPort, err := net.SplitHostPort(host); err == nil {
		host = parsedHost
		if parsedPort != "" {
			if p, parseErr := strconv.Atoi(parsedPort); parseErr == nil {
				port = p
			}
		}
	}
	ip := net.ParseIP(strings.Trim(host, "[]"))
	if ip == nil {
		return nil, fmt.Errorf("usque masque endpoint must be an IP address")
	}
	if cfg.useHTTP2() {
		return &net.TCPAddr{IP: ip, Port: port}, nil
	}
	return &net.UDPAddr{IP: ip, Port: port}, nil
}

func parseUsqueMasqueLocalAddresses(cfg usqueMasqueConfig) ([]netip.Addr, error) {
	var addresses []netip.Addr
	for _, raw := range []string{cfg.IPv4, cfg.IPv6} {
		raw = strings.TrimSpace(raw)
		if raw == "" {
			continue
		}
		addr, err := netip.ParseAddr(raw)
		if err != nil {
			if prefix, prefixErr := netip.ParsePrefix(raw); prefixErr == nil {
				addr = prefix.Addr()
			} else {
				return nil, fmt.Errorf("invalid local address %q: %w", raw, err)
			}
		}
		addresses = append(addresses, addr)
	}
	if len(addresses) == 0 {
		return nil, fmt.Errorf("usque masque requires ipv4 or ipv6 local address")
	}
	return addresses, nil
}

func parseUsqueECPrivateKey(raw string) (*ecdsa.PrivateKey, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return nil, fmt.Errorf("private_key is required")
	}
	var der []byte
	if block, _ := pem.Decode([]byte(raw)); block != nil {
		der = block.Bytes
	} else {
		decoded, err := base64.StdEncoding.DecodeString(raw)
		if err != nil {
			return nil, fmt.Errorf("decode private_key: %w", err)
		}
		der = decoded
	}
	key, err := x509.ParseECPrivateKey(der)
	if err != nil {
		return nil, fmt.Errorf("parse private_key: %w", err)
	}
	return key, nil
}

func parseUsqueECPublicKey(raw string) (*ecdsa.PublicKey, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return nil, fmt.Errorf("endpoint_pub_key is required")
	}
	block, _ := pem.Decode([]byte(raw))
	if block == nil {
		return nil, fmt.Errorf("decode endpoint_pub_key PEM")
	}
	pub, err := x509.ParsePKIXPublicKey(block.Bytes)
	if err != nil {
		return nil, fmt.Errorf("parse endpoint_pub_key: %w", err)
	}
	ecPub, ok := pub.(*ecdsa.PublicKey)
	if !ok {
		return nil, fmt.Errorf("endpoint_pub_key must be ECDSA")
	}
	return ecPub, nil
}

func generateUsqueSelfSignedCert(privKey *ecdsa.PrivateKey) ([][]byte, error) {
	cert, err := x509.CreateCertificate(rand.Reader, &x509.Certificate{
		SerialNumber: big.NewInt(0),
		NotBefore:    time.Now(),
		NotAfter:     time.Now().Add(24 * time.Hour),
	}, &x509.Certificate{}, &privKey.PublicKey, privKey)
	if err != nil {
		return nil, err
	}
	return [][]byte{cert}, nil
}

func closeUsqueTunnel(packetConn net.PacketConn, tr interface{ Close() error }, ipConn *connectip.Conn) {
	if ipConn != nil {
		_ = ipConn.Close()
	}
	if tr != nil {
		_ = tr.Close()
	}
	if packetConn != nil {
		_ = packetConn.Close()
	}
}

func sleepContext(ctx context.Context, delay time.Duration) {
	timer := time.NewTimer(delay)
	defer timer.Stop()
	select {
	case <-ctx.Done():
	case <-timer.C:
	}
}

type usqueMasqueNet struct {
	ep             *channel.Endpoint
	stack          *stack.Stack
	events         chan tun.Event
	notifyHandle   *channel.NotificationHandle
	incomingPacket chan *buffer.View
	mtu            int
	done           chan struct{}
	closeOnce      sync.Once
}

func newUsqueMasqueNet(localAddresses []netip.Addr, mtu int) (*usqueMasqueNet, error) {
	dev := &usqueMasqueNet{
		ep: channel.New(1024, uint32(mtu), ""),
		stack: stack.New(stack.Options{
			NetworkProtocols:   []stack.NetworkProtocolFactory{ipv4.NewProtocol, ipv6.NewProtocol},
			TransportProtocols: []stack.TransportProtocolFactory{tcp.NewProtocol, udp.NewProtocol, icmp.NewProtocol6, icmp.NewProtocol4},
			HandleLocal:        true,
		}),
		events:         make(chan tun.Event, 10),
		incomingPacket: make(chan *buffer.View),
		mtu:            mtu,
		done:           make(chan struct{}),
	}
	dev.notifyHandle = dev.ep.AddNotify(dev)
	if tcpipErr := dev.stack.CreateNIC(1, dev.ep); tcpipErr != nil {
		return nil, fmt.Errorf("CreateNIC: %v", tcpipErr)
	}
	sackEnabledOpt := tcpip.TCPSACKEnabled(true)
	if tcpipErr := dev.stack.SetTransportProtocolOption(tcp.ProtocolNumber, &sackEnabledOpt); tcpipErr != nil {
		return nil, fmt.Errorf("enable TCP SACK: %v", tcpipErr)
	}
	var hasV4, hasV6 bool
	for _, ip := range localAddresses {
		protoNumber := ipv6.ProtocolNumber
		if ip.Is4() {
			protoNumber = ipv4.ProtocolNumber
			hasV4 = true
		} else {
			hasV6 = true
		}
		if tcpipErr := dev.stack.AddProtocolAddress(1, tcpip.ProtocolAddress{
			Protocol:          protoNumber,
			AddressWithPrefix: tcpip.AddrFromSlice(ip.AsSlice()).WithPrefix(),
		}, stack.AddressProperties{}); tcpipErr != nil {
			return nil, fmt.Errorf("AddProtocolAddress(%v): %v", ip, tcpipErr)
		}
	}
	if hasV4 {
		dev.stack.AddRoute(tcpip.Route{Destination: header.IPv4EmptySubnet, NIC: 1})
	}
	if hasV6 {
		dev.stack.AddRoute(tcpip.Route{Destination: header.IPv6EmptySubnet, NIC: 1})
	}
	dev.events <- tun.EventUp
	return dev, nil
}

func (n *usqueMasqueNet) ReadPacket(ctx context.Context, buf []byte) (int, error) {
	select {
	case view := <-n.incomingPacket:
		return view.Read(buf)
	case <-n.done:
		return 0, net.ErrClosed
	case <-ctx.Done():
		return 0, ctx.Err()
	}
}

func (n *usqueMasqueNet) WritePacket(pkt []byte) error {
	if len(pkt) == 0 {
		return nil
	}
	select {
	case <-n.done:
		return net.ErrClosed
	default:
	}
	pkb := stack.NewPacketBuffer(stack.PacketBufferOptions{Payload: buffer.MakeWithData(pkt)})
	switch pkt[0] >> 4 {
	case 4:
		n.ep.InjectInbound(header.IPv4ProtocolNumber, pkb)
	case 6:
		n.ep.InjectInbound(header.IPv6ProtocolNumber, pkb)
	default:
		return syscall.EAFNOSUPPORT
	}
	return nil
}

func (n *usqueMasqueNet) WriteNotify() {
	pkt := n.ep.Read()
	if pkt == nil {
		return
	}
	view := pkt.ToView()
	pkt.DecRef()
	select {
	case n.incomingPacket <- view:
	case <-n.done:
	}
}

func (n *usqueMasqueNet) Close() error {
	n.closeOnce.Do(func() {
		close(n.done)
		n.stack.RemoveNIC(1)
		n.stack.Close()
		n.ep.RemoveNotify(n.notifyHandle)
		n.ep.Close()
		close(n.events)
	})
	return nil
}

func (n *usqueMasqueNet) DialContextTCPAddrPort(ctx context.Context, addr netip.AddrPort) (*gonet.TCPConn, error) {
	fa, pn := usqueMasqueFullAddress(addr)
	return gonet.DialContextTCP(ctx, n.stack, fa, pn)
}

func (n *usqueMasqueNet) DialUDPAddrPort(laddr, raddr netip.AddrPort) (*gonet.UDPConn, error) {
	var lfa, rfa *tcpip.FullAddress
	var pn tcpip.NetworkProtocolNumber
	if laddr.IsValid() || laddr.Port() > 0 {
		addr, protoNumber := usqueMasqueFullAddress(laddr)
		lfa = &addr
		pn = protoNumber
	}
	if raddr.IsValid() || raddr.Port() > 0 {
		addr, protoNumber := usqueMasqueFullAddress(raddr)
		rfa = &addr
		pn = protoNumber
	}
	return gonet.DialUDP(n.stack, lfa, rfa, pn)
}

func usqueMasqueFullAddress(endpoint netip.AddrPort) (tcpip.FullAddress, tcpip.NetworkProtocolNumber) {
	protoNumber := ipv6.ProtocolNumber
	if endpoint.Addr().Is4() {
		protoNumber = ipv4.ProtocolNumber
	}
	return tcpip.FullAddress{
		NIC:  1,
		Addr: tcpip.AddrFromSlice(endpoint.Addr().AsSlice()),
		Port: endpoint.Port(),
	}, protoNumber
}

func init() {
	RegisterXrayCustomOutbound(usqueMasqueHandlerName, newUsqueMasqueOutbound)
}
