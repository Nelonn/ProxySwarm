use super::*;

pub(super) fn review_row(label: &str, value: impl Into<String>) -> Html {
    html! {
        <div class="flex justify-between" style="gap: 1rem; align-items: flex-start;">
            <span class="opacity-70">{ label }</span>
            <span class="font-medium" style="text-align: right; overflow-wrap: anywhere;">{ value.into() }</span>
        </div>
    }
}

pub(super) fn bool_label(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

pub(super) fn optional_label(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() { "-".to_string() } else { value.to_string() }
}

pub(super) fn review_section(title: &str, rows: Vec<Html>) -> Html {
    html! {
        <div class="md3-card bg-surface-container space-y-2">
            <div class="font-semibold">{ title }</div>
            <div class="space-y-1 text-sm">{ for rows }</div>
        </div>
    }
}

pub(super) fn render_inbound_review(data: &InboundEntryDraft) -> Html {
    let protocol = data.protocol.trim().to_uppercase();
    html! {
        <ConfigSection title="Review">
            <div class="space-y-3">
                { review_section("General", vec![
                    review_row("Name", data.name.clone()),
                    review_row("Groups", inbound_groups_label(data)),
                    review_row("Enabled", bool_label(data.enabled)),
                    review_row("Core", optional_label(&data.core_type)),
                    review_row("Protocol", optional_label(&data.protocol)),
                    review_row("Listen", format!("{}:{}", data.listen, data.port)),
                ]) }
                {
                    match protocol.as_str() {
                        "VLESS" => review_section("VLESS", vec![
                            review_row("Flow", optional_label(&data.vless.flow)),
                            review_row("Security", optional_label(&data.vless.security)),
                            review_row("Transmission", vless_transmission_from(&data.vless.transmission)),
                            review_row("TLS server name", optional_label(&data.tls.server_name)),
                            review_row("TLS certificate", optional_label(&data.tls.certificate_name)),
                            review_row("Reality dest", optional_label(&data.vless.reality_dest)),
                            review_row("Reality SNI", optional_label(&data.vless.reality_sni)),
                            review_row("Reality uTLS", optional_label(&data.vless.reality_utls)),
                            review_row("Reality SpiderX", optional_label(&data.vless.reality_spider_x)),
                            review_row("Reality private key", optional_label(&data.vless.reality_private_key)),
                            review_row("Reality public key", optional_label(&data.vless.reality_public_key)),
                            review_row("Reality short IDs", optional_label(&data.vless.reality_short_ids)),
                        ]),
                        "HYSTERIA2" => review_section("Hysteria2", vec![
                            review_row("Password", optional_label(&data.hysteria2.password)),
                            review_row("Obfs type", optional_label(&data.hysteria2.obfs_type)),
                            review_row("Obfs password", optional_label(&data.hysteria2.obfs_password)),
                            review_row("Up Mbps", data.hysteria2.up_mbps.to_string()),
                            review_row("Down Mbps", data.hysteria2.down_mbps.to_string()),
                            review_row("Ignore client bandwidth", bool_label(data.hysteria2.ignore_client_bandwidth)),
                            review_row("Masquerade", optional_label(&data.hysteria2.masquerade)),
                            review_row("BBR profile", optional_label(&data.hysteria2.bbr_profile)),
                            review_row("Brutal debug", bool_label(data.hysteria2.brutal_debug)),
                            review_row("TLS server name", optional_label(&data.tls.server_name)),
                            review_row("TLS certificate", optional_label(&data.tls.certificate_name)),
                        ]),
                        "TRUSTTUNNEL" => review_section("TrustTunnel", vec![
                            review_row("HTTP/1 upload buffer", data.trust_tunnel.http1_upload_buffer_size.to_string()),
                            review_row("HTTP/2 initial connection window", data.trust_tunnel.http2_initial_connection_window_size.to_string()),
                            review_row("HTTP/2 initial stream window", data.trust_tunnel.http2_initial_stream_window_size.to_string()),
                            review_row("HTTP/2 max concurrent streams", data.trust_tunnel.http2_max_concurrent_streams.to_string()),
                            review_row("HTTP/2 max frame size", data.trust_tunnel.http2_max_frame_size.to_string()),
                            review_row("HTTP/2 header table size", data.trust_tunnel.http2_header_table_size.to_string()),
                            review_row("TLS server name", optional_label(&data.tls.server_name)),
                            review_row("TLS certificate", optional_label(&data.tls.certificate_name)),
                        ]),
                        "NAIVEPROXY" => review_section("NaiveProxy", vec![
                            review_row("Network", optional_label(&data.naive_proxy.network)),
                            review_row("QUIC congestion control", optional_label(&data.naive_proxy.quic_congestion_control)),
                            review_row("TLS server name", optional_label(&data.tls.server_name)),
                            review_row("TLS certificate", optional_label(&data.tls.certificate_name)),
                        ]),
                        "WIREGUARD" => review_section("WireGuard", vec![
                            review_row("Private key", optional_label(&data.wireguard.private_key)),
                            review_row("MTU", data.wireguard.mtu.to_string()),
                            review_row("Workers", data.wireguard.workers.to_string()),
                            review_row("Domain strategy", optional_label(&data.wireguard.domain_strategy)),
                            review_row("Reserved", optional_label(&data.wireguard.reserved)),
                            review_row("Addresses", optional_label(&data.wireguard.addresses)),
                        ]),
                        "SOCKS5" => review_section("SOCKS5", vec![
                            review_row("Username", optional_label(&data.socks5.username)),
                            review_row("Password", optional_label(&data.socks5.password)),
                            review_row("UDP enabled", bool_label(data.socks5.udp_enabled)),
                        ]),
                        "SHADOWSOCKS" => review_section("Shadowsocks", vec![
                            review_row("Method", optional_label(&data.shadowsocks.method)),
                            review_row("Default password", optional_label(&data.shadowsocks.password)),
                            review_row("UDP enabled", bool_label(data.shadowsocks.udp_enabled)),
                        ]),
                        "REVERSEPROXY" => review_section("VLESS Reverse", vec![
                            review_row("Mode", optional_label(&data.reverse_proxy.mode)),
                            review_row("Reverse tag", optional_label(&data.reverse_proxy.tag)),
                            review_row("Reverse domain", optional_label(&data.reverse_proxy.domain)),
                            review_row("Bridge outbound tag", optional_label(&data.reverse_proxy.bridge_outbound_tag)),
                            review_row("Bridge target outbound tag", optional_label(&data.reverse_proxy.target_outbound_tag)),
                            review_row("Portal client inbound tag", optional_label(&data.reverse_proxy.portal_inbound_tag)),
                        ]),
                        "TPROXY" => review_section("TProxy", vec![
                            review_row("Network", optional_label(&data.tproxy.network)),
                            review_row("Sniffing enabled", bool_label(data.tproxy.sniffing_enabled)),
                            review_row("Sniffing dest override", optional_label(&data.tproxy.sniffing_dest_override)),
                            review_row("Sniffing route only", bool_label(data.tproxy.sniffing_route_only)),
                            review_row("Socket mark", data.tproxy.socket_mark.to_string()),
                        ]),
                        "TUNNEL" => review_section("Tunnel", vec![
                            review_row("Allowed network", optional_label(&data.tunnel.allowed_network)),
                        ]),
                        _ => html! {},
                    }
                }
            </div>
        </ConfigSection>
    }
}

pub(super) fn render_outbound_review(data: &OutboundEntryDraft) -> Html {
    let outbound_type = data.outbound_type.trim().to_uppercase();
    html! {
        <ConfigSection title="Review">
            <div class="space-y-3">
                { review_section("General", vec![
                    review_row("Name", data.name.clone()),
                    review_row("Type", optional_label(&data.outbound_type)),
                    review_row("Enabled", bool_label(data.enabled)),
                    review_row("Built-in", bool_label(data.builtin)),
                ]) }
                {
                    match outbound_type.as_str() {
                        "DIRECT" | "BLOCK" => review_section("Built-in", vec![
                            review_row("Tag", data.name.clone()),
                        ]),
                        "VLESS" => review_section("VLESS", vec![
                            review_row("Tag", optional_label(&data.vless.tag)),
                            review_row("Server", optional_label(&data.vless.server)),
                            review_row("Port", data.vless.port.to_string()),
                            review_row("UUID", optional_label(&data.vless.uuid)),
                            review_row("Flow", optional_label(&data.vless.flow)),
                            review_row("Security", optional_label(&data.vless.security)),
                            review_row("Transmission", vless_transmission_from(&data.vless.transmission)),
                        ]),
                        "TRUSTTUNNEL" => review_section("TrustTunnel", vec![
                            review_row("Tag", optional_label(&data.trust_tunnel.tag)),
                            review_row("HTTP/1 upload buffer", data.trust_tunnel.http1_upload_buffer_size.to_string()),
                            review_row("HTTP/2 initial connection window", data.trust_tunnel.http2_initial_connection_window_size.to_string()),
                            review_row("HTTP/2 initial stream window", data.trust_tunnel.http2_initial_stream_window_size.to_string()),
                            review_row("HTTP/2 max concurrent streams", data.trust_tunnel.http2_max_concurrent_streams.to_string()),
                            review_row("HTTP/2 max frame size", data.trust_tunnel.http2_max_frame_size.to_string()),
                            review_row("HTTP/2 header table size", data.trust_tunnel.http2_header_table_size.to_string()),
                        ]),
                        "WIREGUARD" => review_section("WireGuard", vec![
                            review_row("Private key", optional_label(&data.wireguard.private_key)),
                            review_row("MTU", data.wireguard.mtu.to_string()),
                            review_row("Workers", data.wireguard.workers.to_string()),
                            review_row("Domain strategy", optional_label(&data.wireguard.domain_strategy)),
                            review_row("Reserved", optional_label(&data.wireguard.reserved)),
                            review_row("Addresses", optional_label(&data.wireguard.addresses)),
                            review_row("Peers", data.wireguard.peers.len().to_string()),
                        ]),
                        "SOCKS5" => review_section("SOCKS5", vec![
                            review_row("Tag", optional_label(&data.socks5.tag)),
                            review_row("Server", optional_label(&data.socks5.server)),
                            review_row("Port", data.socks5.port.to_string()),
                            review_row("Username", optional_label(&data.socks5.username)),
                            review_row("Password", optional_label(&data.socks5.password)),
                        ]),
                        "SHADOWSOCKS" => review_section("Shadowsocks", vec![
                            review_row("Tag", optional_label(&data.shadowsocks.tag)),
                            review_row("Server", optional_label(&data.shadowsocks.server)),
                            review_row("Port", data.shadowsocks.port.to_string()),
                            review_row("Method", optional_label(&data.shadowsocks.method)),
                            review_row("Password", optional_label(&data.shadowsocks.password)),
                            review_row("Plugin", optional_label(&data.shadowsocks.plugin)),
                            review_row("Plugin opts", optional_label(&data.shadowsocks.plugin_opts)),
                            review_row("Prefix", optional_label(&data.shadowsocks.prefix)),
                            review_row("UDP enabled", bool_label(data.shadowsocks.udp_enabled)),
                        ]),
                        _ => html! {},
                    }
                }
            </div>
        </ConfigSection>
    }
}


