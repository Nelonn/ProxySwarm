use base64::Engine;
use gloo_timers::callback::{Interval, Timeout};
use gloo_timers::future::TimeoutFuture;
use js_sys::Date as JsDate;
use qrcodegen::{QrCode, QrCodeEcc};
use trusttunnel_deeplink::{encode, DeepLinkConfig};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::window;
use x25519_dalek::{PublicKey, StaticSecret};
use yew::prelude::*;
use yew_router::prelude::use_navigator;

use crate::components::{
    Button, ButtonType, Chip, ChipMode, Dropdown, DropdownOption, IconButton, Popup, PopupSize,
    RichTable, SnackbarBus, SvgIcon, Switch, SwitchField, TextBox, WideNavigationBar,
    WideNavigationBarItem,
};
use crate::pb::proxyswarm::{
    outbound_config, Account, AccountStatus, CertificateConfig, CoreType, DnsConfig,
    DnsHostMapping, DnsServerConfig, FullConfig, Hysteria2Config, InboundConfig, InboundStatus,
    NaiveProxyConfig, NodeStatus, OutboundConfig, OutboundStatus, OutboundType, RoutingRule,
    SecurityMode, ShadowsocksInboundConfig, ShadowsocksOutboundConfig, Socks5InboundConfig,
    Socks5OutboundConfig, TlsConfig, TrafficStats, TrustTunnelConfig, VlessConfig,
    VlessOutboundConfig, VlessRealityConfig, WireGuardConfig, WireGuardPeer,
};
use crate::services::node_api::{AcmeIssueRequest, AcmeIssueResponse};
use crate::services::warp::{
    generate_wireguard_keypair, register_warp_with_keypair, update_warp_license,
};
use crate::services::ApiService;
use crate::state::{
    default_link_remark_template, format_link_remark, normalize_groups, AccountInfo,
    CertificateDraft, DnsDraft, DnsHostDraft, DnsServerDraft, Hysteria2Draft, InboundEntryDraft,
    NaiveProxyDraft, NodeConfigDraft, NodeConfigRevision, OutboundEntryDraft, ProxyNode,
    RoutingRuleDraft, ShadowsocksDraft, Socks5Draft, State, TlsDraft, TrustTunnelDraft,
    TrustTunnelOutboundDraft, VlessInboundDraft, VlessOutboundDraft, WarpRegistrationDraft,
    WireGuardDraft, WireGuardPeerItem,
};
use crate::storage;
use crate::Route;

#[derive(Properties, PartialEq)]
pub struct NodeConfigPageProps {
    pub id: String,
}

#[derive(Clone, PartialEq)]
enum ConfigTab {
    Inbounds,
    Outbounds,
    Routing,
    Settings,
    Status,
}

mod inbounds;
mod outbounds;
mod routing;
mod settings;
mod status;

fn today_string() -> String {
    let now = JsDate::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        now.get_full_year(),
        now.get_month() + 1,
        now.get_date()
    )
}

fn random_port() -> i32 {
    20000 + (js_sys::Math::random() * 40000.0).floor() as i32
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if getrandom::getrandom(&mut buf).is_err() {
        for b in &mut buf {
            *b = (js_sys::Math::random() * 256.0).floor() as u8;
        }
    }
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

fn generate_reality_short_id() -> String {
    random_hex(8)
}

fn generate_reality_short_ids_batch(count: usize) -> Vec<String> {
    (0..count).map(|_| generate_reality_short_id()).collect()
}

fn generate_reality_keypair() -> (String, String) {
    let mut secret_bytes = [0u8; 32];
    if getrandom::getrandom(&mut secret_bytes).is_err() {
        return (String::new(), String::new());
    }
    let secret = StaticSecret::from(secret_bytes);
    let public = PublicKey::from(&secret);
    (
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret.to_bytes()),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public.as_bytes()),
    )
}

fn default_certificate_draft() -> CertificateDraft {
    CertificateDraft {
        id: uuid::Uuid::new_v4().to_string(),
        name: "default".to_string(),
        cert_type: "ACME".to_string(),
        source: "PATH".to_string(),
        acme_type: "HTTP".to_string(),
        acme_ca: "letsencrypt".to_string(),
        acme_port: 80,
        acme_http_port: 80,
        ..CertificateDraft::default()
    }
}

fn default_inbound_entry() -> InboundEntryDraft {
    InboundEntryDraft {
        id: uuid::Uuid::new_v4().to_string(),
        name: "main".to_string(),
        listen: "0.0.0.0".to_string(),
        port: 443,
        enabled: true,
        core_type: String::new(),
        protocol: "VLESS".to_string(),
        tls: TlsDraft::default(),
        vless: VlessInboundDraft {
            security: "NONE".to_string(),
            reality_utls: "chrome".to_string(),
            reality_spider_x: "/".to_string(),
            ..VlessInboundDraft::default()
        },
        hysteria2: Hysteria2Draft {
            up_mbps: 100,
            down_mbps: 100,
            ..Hysteria2Draft::default()
        },
        trust_tunnel: TrustTunnelDraft {
            ..TrustTunnelDraft::default()
        },
        naive_proxy: NaiveProxyDraft {
            protocol: "h2".to_string(),
            ..NaiveProxyDraft::default()
        },
        wireguard: WireGuardDraft {
            mtu: 1420,
            addresses: "10.0.0.1/32, fd59:7153:2388:b5fd::1/128".to_string(),
            ..WireGuardDraft::default()
        },
        socks5: Socks5Draft {
            port: 1080,
            ..Socks5Draft::default()
        },
        shadowsocks: ShadowsocksDraft {
            port: 8388,
            method: "2022-blake3-aes-128-gcm".to_string(),
            udp_enabled: true,
            ..ShadowsocksDraft::default()
        },
    }
}

fn default_node_draft(node: &ProxyNode) -> NodeConfigDraft {
    NodeConfigDraft {
        outbounds: vec![
            default_builtin_outbound("direct", "DIRECT"),
            default_builtin_outbound("block", "BLOCK"),
        ],
        certificates: vec![],
        inbounds: vec![],
        master_key: node.master_key.clone(),
        routing_rules: vec![RoutingRuleDraft {
            outbound_tag: "direct".to_string(),
            ..RoutingRuleDraft::default()
        }],
        dns: DnsDraft::default(),
        link_remark_template: default_link_remark_template(),
        warp_registration: WarpRegistrationDraft::default(),
    }
}

fn inbound_display_name(inbound: &InboundEntryDraft) -> String {
    let name = inbound.name.trim();
    if name.is_empty() {
        inbound.protocol.trim().to_uppercase()
    } else {
        name.to_string()
    }
}

fn account_display_name(account: &AccountInfo) -> String {
    let name = account.name.trim();
    if name.is_empty() {
        account.id.trim().to_string()
    } else {
        name.to_string()
    }
}

fn rendered_link_remark(
    draft: &NodeConfigDraft,
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
    account: &AccountInfo,
) -> String {
    format_link_remark(
        &draft.link_remark_template,
        node.name.trim(),
        &inbound_display_name(inbound),
        &account_display_name(account),
    )
}

fn certificate_display_name(certificate: &CertificateDraft) -> String {
    if certificate.name.trim().is_empty() {
        "Unnamed certificate".to_string()
    } else {
        certificate.name.trim().to_string()
    }
}

fn normalized_certificates(draft: &NodeConfigDraft) -> Vec<CertificateDraft> {
    draft.certificates.clone()
}

fn certificate_by_name<'a>(
    certificates: &'a [CertificateDraft],
    name: &str,
) -> Option<&'a CertificateDraft> {
    certificates
        .iter()
        .find(|certificate| certificate.name.trim() == name.trim())
}

fn acme_ca_directory_url(value: &str) -> &'static str {
    match value.trim().to_lowercase().as_str() {
        "zerossl" => "https://acme.zerossl.com/v2/DV90",
        "google" => "https://dv.acme-v02.api.pki.goog/directory",
        "buypass" => "https://api.buypass.com/acme/directory",
        "sslcom" => "https://acme.ssl.com/sslcom-dv-rsa",
        _ => "https://acme-v02.api.letsencrypt.org/directory",
    }
}

fn certmagic_storage_component(value: &str) -> String {
    let mut value = value.trim().to_string();
    if let Some(stripped) = value.strip_prefix("http://") {
        value = stripped.to_string();
    } else if let Some(stripped) = value.strip_prefix("https://") {
        value = stripped.to_string();
    }
    value = value.replace('/', "-").replace('\\', "-");
    value.trim_matches('-').to_string()
}

fn certmagic_certificate_paths(ca: &str, domain: &str) -> (String, String) {
    let issuer = certmagic_storage_component(acme_ca_directory_url(ca));
    let safe_domain = certmagic_storage_component(domain);
    let base = format!(
        "data/acme_storage/certificates/{}/{}",
        issuer, safe_domain
    );
    (
        format!("{}/{}.crt", base, safe_domain),
        format!("{}/{}.key", base, safe_domain),
    )
}

fn default_routing_rule_entry() -> RoutingRuleDraft {
    RoutingRuleDraft {
        outbound_tag: "direct".to_string(),
        ..RoutingRuleDraft::default()
    }
}

fn default_builtin_outbound(tag: &str, outbound_type: &str) -> OutboundEntryDraft {
    OutboundEntryDraft {
        id: uuid::Uuid::new_v4().to_string(),
        name: tag.to_string(),
        outbound_type: outbound_type.to_string(),
        enabled: true,
        builtin: true,
        vless: VlessOutboundDraft {
            tag: tag.to_string(),
            ..VlessOutboundDraft::default()
        },
        trust_tunnel: TrustTunnelOutboundDraft {
            tag: tag.to_string(),
            ..TrustTunnelOutboundDraft::default()
        },
        wireguard: WireGuardDraft {
            tag: tag.to_string(),
            ..WireGuardDraft::default()
        },
        socks5: Socks5Draft {
            tag: tag.to_string(),
            ..Socks5Draft::default()
        },
        shadowsocks: ShadowsocksDraft {
            tag: tag.to_string(),
            ..ShadowsocksDraft::default()
        },
    }
}

fn default_vless_outbound() -> OutboundEntryDraft {
    OutboundEntryDraft {
        id: uuid::Uuid::new_v4().to_string(),
        name: "VLESS".to_string(),
        outbound_type: "VLESS".to_string(),
        enabled: true,
        builtin: false,
        vless: VlessOutboundDraft {
            tag: "proxy-vless".to_string(),
            security: "NONE".to_string(),
            ..VlessOutboundDraft::default()
        },
        trust_tunnel: TrustTunnelOutboundDraft::default(),
        wireguard: WireGuardDraft::default(),
        socks5: Socks5Draft::default(),
        shadowsocks: ShadowsocksDraft::default(),
    }
}

fn default_warp_outbound() -> OutboundEntryDraft {
    OutboundEntryDraft {
        id: uuid::Uuid::new_v4().to_string(),
        name: "WARP".to_string(),
        outbound_type: "WIREGUARD".to_string(),
        enabled: true,
        builtin: false,
        vless: VlessOutboundDraft::default(),
        trust_tunnel: TrustTunnelOutboundDraft::default(),
        wireguard: WireGuardDraft {
            tag: "warp".to_string(),
            mtu: 1420,
            ..WireGuardDraft::default()
        },
        socks5: Socks5Draft::default(),
        shadowsocks: ShadowsocksDraft::default(),
    }
}

fn default_shadowsocks_outbound() -> OutboundEntryDraft {
    OutboundEntryDraft {
        id: uuid::Uuid::new_v4().to_string(),
        name: "Shadowsocks".to_string(),
        outbound_type: "SHADOWSOCKS".to_string(),
        enabled: true,
        builtin: false,
        vless: VlessOutboundDraft::default(),
        trust_tunnel: TrustTunnelOutboundDraft::default(),
        wireguard: WireGuardDraft::default(),
        socks5: Socks5Draft::default(),
        shadowsocks: ShadowsocksDraft {
            tag: "proxy-ss".to_string(),
            port: 8388,
            method: "2022-blake3-aes-128-gcm".to_string(),
            udp_enabled: true,
            ..ShadowsocksDraft::default()
        },
    }
}

fn warp_registration_from_outbounds(
    outbounds: &[OutboundEntryDraft],
) -> Option<crate::services::warp::WarpRegistration> {
    outbounds.iter().find_map(|outbound| {
        if !outbound
            .outbound_type
            .trim()
            .eq_ignore_ascii_case("WIREGUARD")
        {
            return None;
        }
        if outbound.wireguard.warp_id.trim().is_empty()
            || outbound.wireguard.warp_token.trim().is_empty()
        {
            return None;
        }
        let (peer_public_key, endpoint) = outbound
            .wireguard
            .peers
            .first()
            .map(|peer| (peer.public_key.clone(), peer.endpoint.clone()))
            .unwrap_or_default();
        let reserved = split_lines_csv(&outbound.wireguard.reserved)
            .into_iter()
            .filter_map(|value| value.parse::<u8>().ok())
            .collect::<Vec<_>>();
        Some(crate::services::warp::WarpRegistration {
            id: outbound.wireguard.warp_id.clone(),
            token: outbound.wireguard.warp_token.clone(),
            private_key: outbound.wireguard.private_key.clone(),
            public_key: String::new(),
            peer_public_key,
            license: String::new(),
            reserved,
            addresses: split_lines_csv(&outbound.wireguard.addresses),
            endpoint,
        })
    })
}

fn warp_registration_from_draft(
    warp: &WarpRegistrationDraft,
) -> Option<crate::services::warp::WarpRegistration> {
    if warp.id.trim().is_empty()
        && warp.token.trim().is_empty()
        && warp.private_key.trim().is_empty()
        && warp.public_key.trim().is_empty()
        && warp.peer_public_key.trim().is_empty()
        && warp.license.trim().is_empty()
        && warp.reserved.trim().is_empty()
        && warp.addresses.trim().is_empty()
        && warp.endpoint.trim().is_empty()
    {
        return None;
    }
    Some(crate::services::warp::WarpRegistration {
        id: warp.id.clone(),
        token: warp.token.clone(),
        private_key: warp.private_key.clone(),
        public_key: warp.public_key.clone(),
        peer_public_key: warp.peer_public_key.clone(),
        license: warp.license.clone(),
        reserved: split_lines_csv(&warp.reserved)
            .into_iter()
            .filter_map(|value| value.parse::<u8>().ok())
            .collect(),
        addresses: split_lines_csv(&warp.addresses),
        endpoint: warp.endpoint.clone(),
    })
}

fn warp_registration_to_draft(
    registration: &crate::services::warp::WarpRegistration,
) -> WarpRegistrationDraft {
    WarpRegistrationDraft {
        id: registration.id.clone(),
        token: registration.token.clone(),
        private_key: registration.private_key.clone(),
        public_key: registration.public_key.clone(),
        peer_public_key: registration.peer_public_key.clone(),
        license: registration.license.clone(),
        reserved: registration
            .reserved
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        addresses: registration.addresses.join(", "),
        endpoint: registration.endpoint.clone(),
    }
}

fn initial_warp_registration(
    draft: &NodeConfigDraft,
) -> Option<crate::services::warp::WarpRegistration> {
    warp_registration_from_draft(&draft.warp_registration)
        .or_else(|| warp_registration_from_outbounds(&draft.outbounds))
}

fn revision_label(index: usize, revision: &NodeConfigRevision) -> String {
    let date = revision.created_at.trim();
    if date.is_empty() {
        format!("Revision {}", index + 1)
    } else {
        format!("Revision {} ({})", index + 1, date)
    }
}

fn sync_draft(draft: &mut NodeConfigDraft) {
    if draft.outbounds.is_empty() {
        draft
            .outbounds
            .push(default_builtin_outbound("direct", "DIRECT"));
        draft
            .outbounds
            .push(default_builtin_outbound("block", "BLOCK"));
    }

    for (index, inbound) in draft.inbounds.iter_mut().enumerate() {
        if inbound.id.trim().is_empty() {
            inbound.id = uuid::Uuid::new_v4().to_string();
        }
        if inbound.name.trim().is_empty() {
            inbound.name = format!("inbound-{}", index + 1);
        }
        if inbound.listen.trim().is_empty() {
            inbound.listen = "0.0.0.0".to_string();
        }
        if inbound.port <= 0 {
            inbound.port = if inbound.protocol.trim().eq_ignore_ascii_case("SOCKS5") {
                1080
            } else {
                443
            };
        }
        if inbound.core_type.trim().is_empty() {
            inbound.core_type = "SING_BOX".to_string();
        }
        if inbound.protocol.trim().is_empty() {
            inbound.protocol = "VLESS".to_string();
        }
        if inbound.wireguard.domain_strategy.trim().is_empty() {
            inbound.wireguard.domain_strategy = "ForceIP".to_string();
        }
    }

    for outbound in draft.outbounds.iter_mut() {
        if outbound.id.trim().is_empty() {
            outbound.id = uuid::Uuid::new_v4().to_string();
        }
        if outbound.outbound_type.trim().is_empty() {
            outbound.outbound_type = "VLESS".to_string();
        }
        if outbound.wireguard.domain_strategy.trim().is_empty() {
            outbound.wireguard.domain_strategy = "ForceIP".to_string();
        }
        if outbound.outbound_type.trim().eq_ignore_ascii_case("DIRECT")
            || outbound.outbound_type.trim().eq_ignore_ascii_case("BLOCK")
        {
            outbound.builtin = true;
            if outbound.name.trim().is_empty() {
                outbound.name = outbound.outbound_type.trim().to_lowercase();
            }
        }
    }

    if draft.routing_rules.is_empty() {
        draft.routing_rules.push(default_routing_rule_entry());
    }
    for rule in draft.routing_rules.iter_mut() {
        if rule.outbound_tag.trim().is_empty() {
            rule.outbound_tag = "direct".to_string();
        }
    }
}

fn normalized_inbounds(draft: &NodeConfigDraft) -> Vec<InboundEntryDraft> {
    let mut copy = draft.clone();
    sync_draft(&mut copy);
    copy.inbounds
}

fn normalized_outbounds(draft: &NodeConfigDraft) -> Vec<OutboundEntryDraft> {
    let mut copy = draft.clone();
    sync_draft(&mut copy);
    copy.outbounds
}

fn normalized_routing_rules(draft: &NodeConfigDraft) -> Vec<RoutingRuleDraft> {
    let mut copy = draft.clone();
    sync_draft(&mut copy);
    copy.routing_rules
}

fn outbound_tag_for_routing(outbound: &OutboundEntryDraft) -> String {
    match outbound.outbound_type.trim().to_uppercase().as_str() {
        "DIRECT" | "BLOCK" => outbound.name.clone(),
        "TRUSTTUNNEL" => outbound.trust_tunnel.tag.clone(),
        "WIREGUARD" => outbound.name.clone(),
        "SOCKS5" => outbound.socks5.tag.clone(),
        "SHADOWSOCKS" => outbound.shadowsocks.tag.clone(),
        _ => outbound.vless.tag.clone(),
    }
}

fn outbound_label_for_routing(outbound: &OutboundEntryDraft) -> String {
    let tag = outbound_tag_for_routing(outbound);
    format!("{} ({})", tag, outbound.outbound_type)
}

fn split_lines_csv(value: &str) -> Vec<String> {
    value
        .split(|c| c == ',' || c == '\n' || c == '\r')
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
        .collect()
}

fn normalize_reality_short_ids(value: &str) -> Vec<String> {
    split_lines_csv(value)
        .into_iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .filter(|s| s.len() <= 16 && s.len() % 2 == 0)
        .filter(|s| s.chars().all(|c| c.is_ascii_hexdigit()))
        .collect()
}

fn security_from(value: &str) -> i32 {
    match value.trim().to_uppercase().as_str() {
        "TLS" => SecurityMode::Tls as i32,
        "REALITY" => SecurityMode::Reality as i32,
        _ => SecurityMode::None as i32,
    }
}

fn vless_transmission_from(value: &str) -> String {
    match value.trim().to_uppercase().as_str() {
        "TCP" | "TCP (RAW)" => "TCP".to_string(),
        "HTTP" => "HTTP".to_string(),
        "GRPC" => "gRPC".to_string(),
        "WEBSOCKET" => "WebSocket".to_string(),
        "MKCP" => "mKCP".to_string(),
        "HTTPUPGRADE" => "HttpUpgrade".to_string(),
        "SPLITHTTP" => "SplitHTTP".to_string(),
        _ => "TCP".to_string(),
    }
}

fn vless_link_type(value: &str) -> &'static str {
    match vless_transmission_from(value).as_str() {
        "HTTP" => "http",
        "gRPC" => "grpc",
        "WebSocket" => "ws",
        "mKCP" => "kcp",
        "HttpUpgrade" => "httpupgrade",
        "SplitHTTP" => "splithttp",
        _ => "tcp",
    }
}

fn shadowsocks_method_options() -> Vec<DropdownOption> {
    vec![
        DropdownOption {
            value: "2022-blake3-aes-128-gcm".to_string(),
            label: "2022-blake3-aes-128-gcm".to_string(),
        },
        DropdownOption {
            value: "2022-blake3-aes-256-gcm".to_string(),
            label: "2022-blake3-aes-256-gcm".to_string(),
        },
        DropdownOption {
            value: "2022-blake3-chacha20-poly1305".to_string(),
            label: "2022-blake3-chacha20-poly1305".to_string(),
        },
        DropdownOption {
            value: "aes-256-gcm".to_string(),
            label: "aes-256-gcm".to_string(),
        },
        DropdownOption {
            value: "aes-128-gcm".to_string(),
            label: "aes-128-gcm".to_string(),
        },
        DropdownOption {
            value: "chacha20-poly1305".to_string(),
            label: "chacha20-poly1305 (or chacha20-ietf-poly1305)".to_string(),
        },
        DropdownOption {
            value: "xchacha20-poly1305".to_string(),
            label: "xchacha20-poly1305 (or xchacha20-ietf-poly1305)".to_string(),
        },
        DropdownOption {
            value: "none".to_string(),
            label: "none (or plain)".to_string(),
        },
    ]
}

fn core_from(value: &str) -> i32 {
    match value.trim().to_uppercase().as_str() {
        "XRAY" => CoreType::Xray as i32,
        "SING_BOX" => CoreType::SingBox as i32,
        "TRUSTTUNNEL" => CoreType::SingBox as i32,
        _ => CoreType::SingBox as i32,
    }
}

fn supports_hysteria2(core_type: &str) -> bool {
    // Hysteria2 is supported on both Xray and Sing-Box. TrustTunnel remains exclusive.
    !matches!(core_type.trim().to_uppercase().as_str(), "TRUSTTUNNEL")
}

fn protocol_options_for_core(core_type: &str) -> Vec<DropdownOption> {
    if core_type.trim().is_empty() {
        return vec![];
    }
    let mut options = vec![
        DropdownOption {
            value: "VLESS".to_string(),
            label: "VLESS".to_string(),
        },
        DropdownOption {
            value: "TRUSTTUNNEL".to_string(),
            label: "TrustTunnel".to_string(),
        },
        DropdownOption {
            value: "NAIVEPROXY".to_string(),
            label: "NaiveProxy".to_string(),
        },
        DropdownOption {
            value: "WIREGUARD".to_string(),
            label: "WireGuard".to_string(),
        },
        DropdownOption {
            value: "SOCKS5".to_string(),
            label: "SOCKS5".to_string(),
        },
        DropdownOption {
            value: "SHADOWSOCKS".to_string(),
            label: "Shadowsocks".to_string(),
        },
    ];
    if supports_hysteria2(core_type) {
        options.insert(
            1,
            DropdownOption {
                value: "HYSTERIA2".to_string(),
                label: "Hysteria2".to_string(),
            },
        );
    }
    options
}

fn normalize_protocol_for_core(core_type: &str, protocol: &str) -> String {
    let protocol = protocol.trim().to_uppercase();
    let core_type = core_type.trim().to_uppercase();

    if core_type.is_empty() {
        return "".to_string();
    }
    if core_type == "TRUSTTUNNEL" {
        return "TRUSTTUNNEL".to_string();
    }
    if protocol == "TRUSTTUNNEL" {
        return "VLESS".to_string();
    }
    if protocol == "HYSTERIA2" && !supports_hysteria2(&core_type) {
        return "VLESS".to_string();
    }
    protocol
}

fn inbound_traffic_label(inbound: &InboundEntryDraft) -> String {
    match inbound.protocol.trim().to_uppercase().as_str() {
        "VLESS" => {
            let transmission = vless_transmission_from(&inbound.vless.transmission).to_uppercase();
            let security = inbound.vless.security.trim().to_uppercase();

            if security.is_empty() || security == "NONE" {
                transmission
            } else {
                format!("{} {}", transmission, security)
            }
        }
        "HYSTERIA2" => "QUIC".to_string(),
        "TRUSTTUNNEL" => "http2".to_string(),
        "NAIVEPROXY" => inbound.naive_proxy.protocol.clone(),
        "WIREGUARD" => "UDP".to_string(),
        "SOCKS5" => {
            if inbound.socks5.udp_enabled {
                "TCP+UDP".to_string()
            } else {
                "TCP".to_string()
            }
        }
        "SHADOWSOCKS" => {
            if inbound.shadowsocks.udp_enabled {
                "TCP+UDP".to_string()
            } else {
                "TCP".to_string()
            }
        }
        _ => "TCP".to_string(),
    }
}

fn nav_key(tab: &ConfigTab) -> AttrValue {
    match tab {
        ConfigTab::Inbounds => "inbounds".into(),
        ConfigTab::Outbounds => "outbounds".into(),
        ConfigTab::Routing => "routing".into(),
        ConfigTab::Settings => "settings".into(),
        ConfigTab::Status => "status".into(),
    }
}

fn nav_items() -> Vec<WideNavigationBarItem> {
    vec![
        WideNavigationBarItem {
            value: "status".into(),
            label: "Status".into(),
            icon_name: "icon-bar-chart-4".into(),
        },
        WideNavigationBarItem {
            value: "inbounds".into(),
            label: "Inbounds".into(),
            icon_name: "icon-call-received".into(),
        },
        WideNavigationBarItem {
            value: "outbounds".into(),
            label: "Outbounds".into(),
            icon_name: "icon-call-made".into(),
        },
        WideNavigationBarItem {
            value: "routing".into(),
            label: "Routing".into(),
            icon_name: "icon-call-split".into(),
        },
        WideNavigationBarItem {
            value: "settings".into(),
            label: "Settings".into(),
            icon_name: "icon-settings".into(),
        },
    ]
}

fn format_status_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes < 1024_u64.pow(4) {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!(
            "{:.2} TB",
            bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0)
        )
    }
}

fn format_status_rate(bytes_per_second: f64) -> String {
    // Network speeds are conventionally shown in bits/sec (not bytes/sec).
    let bits_per_second = (bytes_per_second * 8.0).max(0.0);
    if bits_per_second < 1000.0 {
        format!("{:.0} b/s", bits_per_second.round())
    } else if bits_per_second < 1_000_000.0 {
        format!("{:.1} Kb/s", bits_per_second / 1000.0)
    } else if bits_per_second < 1_000_000_000.0 {
        format!("{:.1} Mb/s", bits_per_second / 1_000_000.0)
    } else if bits_per_second < 1_000_000_000_000.0 {
        format!("{:.2} Gb/s", bits_per_second / 1_000_000_000.0)
    } else {
        format!("{:.2} Tb/s", bits_per_second / 1_000_000_000_000.0)
    }
}

fn format_optional_limit_bytes(bytes: Option<u64>) -> String {
    bytes
        .map(format_status_bytes)
        .unwrap_or_else(|| "Unlimited".to_string())
}

fn format_optional_bandwidth(bandwidth_mbps: Option<u32>) -> String {
    bandwidth_mbps
        .map(|value| format!("{} Mbps", value))
        .unwrap_or_else(|| "Not set".to_string())
}

#[derive(Properties, PartialEq)]
struct CircularProgressProps {
    value: f64,
    #[prop_or(false)]
    show_label_inside: bool,
}

#[function_component(CircularProgress)]
fn circular_progress(props: &CircularProgressProps) -> Html {
    let value = props.value.clamp(0.0, 100.0);
    let normalized = value / 100.0;
    let radius = 18.0;
    let stroke_width = 4.0;
    let circumference = 2.0 * std::f64::consts::PI * radius;
    let gap_length = 7.0;
    let track_length = (circumference - gap_length * 2.0).max(0.0);
    let active_length = (track_length * normalized).clamp(0.0, track_length);
    let inactive_length = (track_length - active_length).max(0.0);
    let active_dasharray = format!("{:.3} {:.3}", active_length, circumference);
    let inactive_dasharray = format!("{:.3} {:.3}", inactive_length, circumference);
    let active_dashoffset = format!("{:.3}", -gap_length / 2.0);
    let inactive_dashoffset = format!("{:.3}", -(active_length + gap_length * 1.5));

    html! {
        <div
            style="position: relative; width: 72px; height: 72px; flex: 0 0 auto;"
        >
            <svg
                viewBox="0 0 48 48"
                width="72"
                height="72"
                aria-hidden="true"
                style="display: block;"
            >
                <g transform="rotate(-90 24 24)">
                    {
                        if inactive_length > 0.01 {
                            html! {
                                <circle
                                    cx="24"
                                    cy="24"
                                    r={radius.to_string()}
                                    fill="none"
                                    stroke="var(--md-sys-color-outline-variant)"
                                    stroke-width={stroke_width.to_string()}
                                    stroke-linecap="round"
                                    stroke-dasharray={inactive_dasharray}
                                    stroke-dashoffset={inactive_dashoffset}
                                    style="transition: stroke-dasharray 240ms ease, stroke-dashoffset 240ms ease;"
                                />
                            }
                        } else {
                            html! {}
                        }
                    }
                    {
                        if active_length > 0.01 {
                            html! {
                                <circle
                                    cx="24"
                                    cy="24"
                                    r={radius.to_string()}
                                    fill="none"
                                    stroke="var(--md-sys-color-primary)"
                                    stroke-width={stroke_width.to_string()}
                                    stroke-linecap="round"
                                    stroke-dasharray={active_dasharray}
                                    stroke-dashoffset={active_dashoffset}
                                    style="transition: stroke-dasharray 240ms ease, stroke-dashoffset 240ms ease;"
                                />
                            }
                        } else {
                            html! {}
                        }
                    }
                </g>
            </svg>
            <div
                style="position: absolute; inset: 0px; display: flex; align-items: center; justify-content: center; pointer-events: none;"
            >
                <div class="font-semibold" style="font-size: 13px; line-height: 16px;">
                    {
                        if props.show_label_inside {
                            html! { format!("{:.0}%", value) }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct UnifiedTrafficProps {
    traffic: Option<TrafficStats>,
    #[prop_or(false)]
    invert_icon: bool,
}

#[function_component(UnifiedTraffic)]
fn unified_traffic(props: &UnifiedTrafficProps) -> Html {
    let traffic = props.traffic.clone().unwrap_or_default();
    let outbound_icon = "icon-straight";
    let inbound_icon = "icon-straight-inbound";
    html! {
        <div class="opacity-80 rounded-lg" style="font-size: 13px; font-weight: 500; line-height: 18px; border: 0px solid var(--md-sys-color-outline-variant); padding: 4px 10px 4px 4px;">
            <div class="flex items-center justify-end" style="gap: 2px; min-height: 18px;">
                <span style="height: 18px; width: 18px; display: inline-flex; align-items: center; justify-content: center; flex: 0 0 18px;">
                    <SvgIcon name={outbound_icon} size={14} class={classes!("opacity-70")} />
                </span>
                <span style="display: inline-flex; align-items: center; min-height: 18px;">
                    { format!("{} ({})", format_status_bytes(traffic.tx), format_status_rate(traffic.tx_rate)) }
                </span>
            </div>
            <div class="flex items-center justify-end" style="gap: 2px; min-height: 18px;">
                <span style="height: 18px; width: 18px; display: inline-flex; align-items: center; justify-content: center; flex: 0 0 18px;">
                    <SvgIcon name={inbound_icon} size={14} class={classes!("opacity-70")} />
                </span>
                <span style="display: inline-flex; align-items: center; min-height: 18px;">
                    { format!("{} ({})", format_status_bytes(traffic.rx), format_status_rate(traffic.rx_rate)) }
                </span>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct UserStatusDotProps {
    online: bool,
}

#[function_component(UserStatusDot)]
fn user_status_dot(props: &UserStatusDotProps) -> Html {
    let dot_color = if props.online {
        "var(--md-sys-color-primary)"
    } else {
        "var(--md-sys-color-outline)"
    };
    let ripple_color = if props.online {
        "color-mix(in srgb, var(--md-sys-color-primary) 30%, transparent)"
    } else {
        "transparent"
    };

    html! {
        <div
            style={format!(
                "position: relative; width: 18px; height: 18px; flex: 0 0 18px; display: inline-flex; align-items: center; justify-content: center;",
            )}
            aria-label={if props.online { "Online" } else { "Offline" }}
            title={if props.online { "Online" } else { "Offline" }}
        >
            <span
                style={format!(
                    "width: 10px; height: 10px; border-radius: 999px; background: {}; display: block; flex: 0 0 10px;",
                dot_color
            )}
            />
            {
                if props.online {
                    html! {
                        <>
                            <span style={format!(
                                "position: absolute; left: 4px; top: 4px; width: 10px; height: 10px; border-radius: 999px; background: {}; animation: md3-user-status-ripple 1s ease-out infinite;",
                                ripple_color
                            )} />
                        </>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct NodeStatusPanelProps {
    status: NodeStatus,
    bandwidth_mbps: Option<u32>,
    max_traffic_bytes: Option<u64>,
}

#[function_component(NodeStatusPanel)]
fn node_status_panel(props: &NodeStatusPanelProps) -> Html {
    let total_traffic = props
        .status
        .total_inbound_traffic
        .clone()
        .map(|traffic| traffic.rx + traffic.tx)
        .unwrap_or(0);
    let traffic_cap_progress = props.max_traffic_bytes.map(|limit| {
        if limit == 0 {
            0.0
        } else {
            ((total_traffic as f64 / limit as f64) * 100.0).clamp(0.0, 100.0)
        }
    });

    html! {
        <div class="space-y-6">
            <style>
                {r#"
                    @keyframes md3-user-status-ripple {
                        0% {
                            opacity: 0;
                            transform: scale(1);
                        }
                        12% {
                            opacity: 0.55;
                            transform: scale(1);
                        }
                        70% {
                            opacity: 0;
                            transform: scale(2.4);
                        }
                        100% {
                            opacity: 0;
                            transform: scale(2.4);
                        }
                    }
                "#}
            </style>
            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Server Bandwidth" }</div>
                    <div class="font-bold" style="font-size: 20px; line-height: 28px;">{ format_optional_bandwidth(props.bandwidth_mbps) }</div>
                </div>
                <div class="md3-card bg-surface-container">
                    <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                        <div style="min-width: 0px; flex: 1 1 auto;">
                            <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Max Traffic" }</div>
                            <div class="font-bold" style="font-size: 20px; line-height: 28px;">{ format_optional_limit_bytes(props.max_traffic_bytes) }</div>
                            {
                                if traffic_cap_progress.is_some() {
                                    html! {
                                        <div style="color: var(--md-sys-color-on-surface-variant); font-size: 13px; line-height: 18px;">
                                            { format!("Current: {}", format_status_bytes(total_traffic)) }
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                        </div>
                        <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                            {
                                if let Some(progress) = traffic_cap_progress {
                                    html! { <CircularProgress value={progress} show_label_inside={true} /> }
                                } else {
                                    html! {}
                                }
                            }
                        </div>
                    </div>
                </div>
            </div>

            {
                if let Some(hw) = &props.status.hardware {
                    let ram_progress = if hw.ram_total == 0 {
                        0.0
                    } else {
                        ((hw.ram_used as f64 / hw.ram_total as f64) * 100.0).clamp(0.0, 100.0)
                    };
                    html! {
                        <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                            <div class="md3-card bg-surface-container">
                                <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                                    <div style="min-width: 0px; flex: 1 1 auto;">
                                        <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "CPU Usage" }</div>
                                        <div class="font-bold" style="font-size: 20px; line-height: 28px; margin-top: 8px;">{ "Processor Load" }</div>
                                        <div style="color: var(--md-sys-color-on-surface-variant); font-size: 13px; line-height: 18px;">
                                            {
                                                if hw.cpu_cores > 0 {
                                                    format!("{} CPU cores", hw.cpu_cores)
                                                } else {
                                                    "CPU cores unavailable".to_string()
                                                }
                                            }
                                        </div>
                                    </div>
                                    <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                                        <CircularProgress value={hw.cpu_usage} show_label_inside={true} />
                                    </div>
                                </div>
                            </div>
                            <div class="md3-card bg-surface-container">
                                <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                                    <div style="min-width: 0px; flex: 1 1 auto;">
                                        <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "RAM Usage" }</div>
                                        <div class="font-bold" style="font-size: 20px; line-height: 28px; margin-top: 8px;">{ format!("{} / {}", format_status_bytes(hw.ram_used), format_status_bytes(hw.ram_total)) }</div>
                                        <div style="color: var(--md-sys-color-on-surface-variant); font-size: 13px; line-height: 18px;">
                                            { format!("Free {}", format_status_bytes(hw.ram_total.saturating_sub(hw.ram_used))) }
                                        </div>
                                    </div>
                                    <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                                        <CircularProgress value={ram_progress} show_label_inside={true} />
                                    </div>
                                </div>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Total Inbound Traffic" }</div>
                    <div class="font-bold" style="font-size: 20px; line-height: 28px;">
                        {
                            props.status
                                .total_inbound_traffic
                                .clone()
                                .map(|traffic| format_status_bytes(traffic.rx.saturating_add(traffic.tx)))
                                .unwrap_or_else(|| "-".to_string())
                        }
                    </div>
                    <div class="mt-2" style="font-size: 13px; line-height: 18px;">
                        {
                            if let Some(traffic) = props.status.total_inbound_traffic.clone() {
                                html! {
                                    <>
                                        <div style="color: var(--md-sys-color-on-surface-variant);">{ format!("From Clients: {} ({})", format_status_bytes(traffic.tx), format_status_rate(traffic.tx_rate)) }</div>
                                        <div style="color: var(--md-sys-color-on-surface-variant);">{ format!("From Servers: {} ({})", format_status_bytes(traffic.rx), format_status_rate(traffic.rx_rate)) }</div>
                                    </>
                                }
                            } else {
                                html! { <div style="color: var(--md-sys-color-on-surface-variant);">{ "No sample yet" }</div> }
                            }
                        }
                    </div>
                </div>
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Total Outbound Traffic" }</div>
                    <div class="font-bold" style="font-size: 20px; line-height: 28px;">
                        {
                            props.status
                                .total_outbound_traffic
                                .clone()
                                .map(|traffic| format_status_bytes(traffic.rx.saturating_add(traffic.tx)))
                                .unwrap_or_else(|| "-".to_string())
                        }
                    </div>
                    <div class="mt-2" style="font-size: 13px; line-height: 18px;">
                        {
                            if let Some(traffic) = props.status.total_outbound_traffic.clone() {
                                html! {
                                    <>
                                        <div style="color: var(--md-sys-color-on-surface-variant);">{ format!("To Clients: {} ({})", format_status_bytes(traffic.rx), format_status_rate(traffic.rx_rate)) }</div>
                                        <div style="color: var(--md-sys-color-on-surface-variant);">{ format!("To Servers: {} ({})", format_status_bytes(traffic.tx), format_status_rate(traffic.tx_rate)) }</div>
                                    </>
                                }
                            } else {
                                html! { <div style="color: var(--md-sys-color-on-surface-variant);">{ "No sample yet" }</div> }
                            }
                        }
                    </div>
                </div>
            </div>

            <div class="md3-card bg-surface-container">
                <div class="flex justify-between" style="align-items: center;">
                    <div>
                        <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Connections" }</div>
                        <div class="font-bold" style="font-size: 24px; line-height: 32px;">
                            {
                                props.status
                                    .connections
                                    .clone()
                                    .map(|c| format!("TCP {} / UDP {}", c.tcp, c.udp))
                                    .unwrap_or_else(|| "TCP 0 / UDP 0".to_string())
                            }
                        </div>
                    </div>
                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">
                        { format!("Sample window: {}s", props.status.sample_window_seconds.max(1)) }
                    </div>
                </div>
            </div>

            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container space-y-3">
                    <div class="font-semibold uppercase opacity-70" style="font-size: 13px; line-height: 18px;">{ "Inbounds" }</div>
                    { for props.status.inbounds.iter().map(|inbound: &InboundStatus| html! {
                        <div class="bg-surface-container p-3 rounded-lg">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;">
                                    <div class="font-medium" style="font-size: 14px; line-height: 20px;">{ inbound.name.clone() }</div>
                                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">
                                        {
                                            inbound.connections.clone()
                                                .map(|c| format!("TCP {} / UDP {}", c.tcp, c.udp))
                                                .unwrap_or_else(|| "TCP 0 / UDP 0".to_string())
                                        }
                                    </div>
                                </div>
                                <UnifiedTraffic traffic={inbound.traffic.clone()} invert_icon={true} />
                            </div>
                        </div>
                    }) }
                </div>
                <div class="md3-card bg-surface-container space-y-3">
                    <div class="font-semibold uppercase opacity-70" style="font-size: 13px; line-height: 18px;">{ "Outbounds" }</div>
                    { for props.status.outbounds.iter().map(|outbound: &OutboundStatus| html! {
                        <div class="bg-surface-container p-3 rounded-lg">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;">
                                    <div class="font-medium" style="font-size: 14px; line-height: 20px;">{ outbound.name.clone() }</div>
                                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">
                                        {
                                            if outbound.excluded_from_totals {
                                                format!("{} • excluded from totals", outbound.r#type)
                                            } else {
                                                outbound.r#type.clone()
                                            }
                                        }
                                    </div>
                                </div>
                                <UnifiedTraffic traffic={outbound.traffic.clone()} />
                            </div>
                        </div>
                    }) }
                </div>
            </div>

            <div class="md3-card bg-surface-container space-y-3">
                <div class="font-semibold uppercase opacity-70" style="font-size: 13px; line-height: 18px;">{ "Users" }</div>
                { for props.status.accounts.iter().map(|account: &AccountStatus| {
                    let is_online = account.online > 0;
                    html! {
                        <div class="bg-surface-container p-3 rounded-lg space-y-2">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div class="space-y-1" style="min-width: 0px;">
                                    <div class="flex items-center" style="gap: 10px; min-height: 20px; align-items: center;">
                                        <UserStatusDot online={is_online} />
                                        <div class="font-semibold" style="font-size: 15px; line-height: 20px; display: flex; align-items: center; min-height: 20px;">{ account.name.clone() }</div>
                                    </div>
                                </div>
                                <UnifiedTraffic traffic={account.traffic.clone()} invert_icon={true} />
                            </div>
                        </div>
                    }
                }) }
            </div>
        </div>
    }
}

#[function_component(StatusSkeletonPanel)]
fn status_skeleton_panel() -> Html {
    let bar = |width: &str, height: &str| {
        html! {
            <div
                class="rounded-full"
                style={format!(
                    "width: {}; height: {}; background-color: rgba(255, 255, 255, 0.10);",
                    width, height
                )}
            />
        }
    };
    let dot = |size: &str| {
        html! {
            <div
                class="rounded-full"
                style={format!(
                    "width: {}; height: {}; background-color: rgba(255, 255, 255, 0.10); flex: 0 0 {};",
                    size, size, size
                )}
            />
        }
    };
    let ring = || {
        html! {
            <div style="position: relative; width: 72px; height: 72px; flex: 0 0 auto;">
                <svg
                    viewBox="0 0 48 48"
                    width="72"
                    height="72"
                    aria-hidden="true"
                    style="display: block;"
                >
                    <circle
                        cx="24"
                        cy="24"
                        r="18"
                        fill="none"
                        stroke="rgba(255, 255, 255, 0.10)"
                        stroke-width="4"
                        stroke-linecap="round"
                    />
                </svg>
            </div>
        }
    };
    let traffic_line = |label_width: &str, value_width: &str| {
        html! {
            <div class="flex items-center" style="gap: 2px; min-height: 18px;">
                <span style="height: 18px; width: 18px; display: inline-flex; align-items: center; justify-content: center; flex: 0 0 18px;">
                    <div class="rounded-full" style="width: 10px; height: 10px; background-color: rgba(255, 255, 255, 0.10);" />
                </span>
                <div style="display: inline-flex; align-items: center; min-height: 18px; gap: 6px;">
                    { bar(label_width, "14px") }
                    { bar(value_width, "14px") }
                </div>
            </div>
        }
    };
    let traffic_stack = || {
        html! {
            <div class="opacity-80 rounded-lg" style="font-size: 13px; font-weight: 500; line-height: 18px; padding: 4px 10px 4px 4px;">
                { traffic_line("4.25rem", "5.5rem") }
                { traffic_line("4.25rem", "5.5rem") }
            </div>
        }
    };

    html! {
        <div class="space-y-6 animate-pulse">
            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container space-y-2">
                    { bar("9rem", "16px") }
                    { bar("6rem", "28px") }
                </div>
                <div class="md3-card bg-surface-container space-y-2">
                    { bar("8rem", "16px") }
                    { bar("6.5rem", "28px") }
                </div>
            </div>
            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container">
                    <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                        <div style="min-width: 0px; flex: 1 1 auto;" class="space-y-2">
                            { bar("6rem", "16px") }
                            { bar("8rem", "28px") }
                            { bar("7rem", "18px") }
                        </div>
                        <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                            { ring() }
                        </div>
                    </div>
                </div>
                <div class="md3-card bg-surface-container">
                    <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                        <div style="min-width: 0px; flex: 1 1 auto;" class="space-y-2">
                            { bar("6rem", "16px") }
                            { bar("8rem", "28px") }
                            { bar("7rem", "18px") }
                        </div>
                        <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                            { ring() }
                        </div>
                    </div>
                </div>
            </div>

            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ bar("9rem", "16px") }</div>
                    <div class="font-bold" style="font-size: 24px; line-height: 32px;">{ bar("10rem", "32px") }</div>
                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">{ bar("7rem", "18px") }</div>
                </div>
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ bar("9rem", "16px") }</div>
                    <div class="font-bold" style="font-size: 24px; line-height: 32px;">{ bar("10rem", "32px") }</div>
                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">{ bar("7rem", "18px") }</div>
                </div>
            </div>

            <div class="md3-card bg-surface-container">
                <div class="flex justify-between" style="align-items: center; gap: 16px;">
                    <div class="space-y-2">
                        { bar("22%", "16px") }
                        { bar("14rem", "32px") }
                    </div>
                    <div style="width: 72px;"></div>
                </div>
            </div>

            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container space-y-3">
                    { bar("18%", "18px") }
                    { for (0..4).map(|_| html! {
                        <div class="bg-surface-container p-3 rounded-lg space-y-2">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;" class="space-y-2">
                                    { bar("8rem", "20px") }
                                    { bar("10rem", "18px") }
                                </div>
                                { traffic_stack() }
                            </div>
                        </div>
                    }) }
                </div>
                <div class="md3-card bg-surface-container space-y-3">
                    { bar("20%", "18px") }
                    { for (0..4).map(|_| html! {
                        <div class="bg-surface-container p-3 rounded-lg space-y-2">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;" class="space-y-2">
                                    { bar("8rem", "20px") }
                                    { bar("10rem", "18px") }
                                </div>
                                { traffic_stack() }
                            </div>
                        </div>
                    }) }
                </div>
            </div>

            <div class="md3-card bg-surface-container">
                { bar("18%", "18px") }
                <div class="space-y-3 mt-3">
                    { for (0..4).map(|_| html! {
                        <div class="bg-surface-container p-3 rounded-lg space-y-2">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;" class="space-y-2">
                                    <div class="flex items-center" style="gap: 10px; min-height: 20px; align-items: center;">
                                        { dot("10px") }
                                        { bar("9rem", "20px") }
                                    </div>
                                </div>
                                { bar("5.5rem", "20px") }
                            </div>
                        </div>
                    }) }
                </div>
            </div>
        </div>
    }
}

fn build_dns_server_config(server: &DnsServerDraft) -> Option<DnsServerConfig> {
    let address = server.address.trim();
    if address.is_empty() {
        return None;
    }
    Some(DnsServerConfig {
        address: address.to_string(),
        client_ip: server.client_ip.trim().to_string(),
        port: server.port,
        skip_fallback: server.skip_fallback,
        domains: split_lines_csv(&server.domains),
        expect_ips: split_lines_csv(&server.expect_ips),
        query_strategy: server.query_strategy.trim().to_string(),
        tag: server.tag.trim().to_string(),
        timeout_ms: server.timeout_ms,
        disable_cache: server.disable_cache,
        serve_stale: server.serve_stale,
        serve_expired_ttl: server.serve_expired_ttl,
        final_query: server.final_query,
        unexpected_ips: split_lines_csv(&server.unexpected_ips),
    })
}

fn build_dns_host_mapping(host: &DnsHostDraft) -> Option<DnsHostMapping> {
    let domain = host.domain.trim();
    if domain.is_empty() {
        return None;
    }
    let values = split_lines_csv(&host.values);
    if values.is_empty() {
        return None;
    }
    Some(DnsHostMapping {
        domain: domain.to_string(),
        values,
    })
}

fn build_dns_config(draft: &NodeConfigDraft) -> Option<DnsConfig> {
    let servers: Vec<DnsServerConfig> = draft
        .dns
        .servers
        .iter()
        .filter_map(build_dns_server_config)
        .collect();
    let hosts: Vec<DnsHostMapping> = draft
        .dns
        .hosts
        .iter()
        .filter_map(build_dns_host_mapping)
        .collect();

    let dns = DnsConfig {
        servers,
        hosts,
        client_ip: draft.dns.client_ip.trim().to_string(),
        tag: draft.dns.tag.trim().to_string(),
        query_strategy: draft.dns.query_strategy.trim().to_string(),
        disable_cache: draft.dns.disable_cache,
        serve_stale: draft.dns.serve_stale,
        serve_expired_ttl: draft.dns.serve_expired_ttl,
        disable_fallback: draft.dns.disable_fallback,
        disable_fallback_if_match: draft.dns.disable_fallback_if_match,
        enable_parallel_query: draft.dns.enable_parallel_query,
        use_system_hosts: draft.dns.use_system_hosts,
    };

    if dns.servers.is_empty()
        && dns.hosts.is_empty()
        && dns.client_ip.is_empty()
        && dns.tag.is_empty()
        && dns.query_strategy.is_empty()
        && !dns.disable_cache
        && !dns.serve_stale
        && dns.serve_expired_ttl == 0
        && !dns.disable_fallback
        && !dns.disable_fallback_if_match
        && !dns.enable_parallel_query
        && !dns.use_system_hosts
    {
        return None;
    }

    Some(dns)
}

fn build_full_config(
    draft: &NodeConfigDraft,
    node: &ProxyNode,
    accounts: &[AccountInfo],
) -> FullConfig {
    let certificates = normalized_certificates(draft);
    let node_accounts: Vec<Account> = accounts
        .iter()
        .filter(|account| {
            let account_groups = normalize_groups(&account.groups);
            let node_groups = normalize_groups(&node.groups);
            account_groups
                .iter()
                .any(|value| node_groups.iter().any(|candidate| candidate == value))
        })
        .map(|account| Account {
            id: account.id.clone(),
            name: account.name.clone(),
            allowed_ips: account.allowed_ips.clone(),
            groups: normalize_groups(&account.groups),
            expiry_time: account.expiry_date,
            token: account.token.clone(),
        })
        .collect();
    let inbounds = normalized_inbounds(draft)
        .into_iter()
        .map(|inbound| {
            let normalized_protocol =
                normalize_protocol_for_core(&inbound.core_type, &inbound.protocol);
            let selected_certificate =
                certificate_by_name(&certificates, &inbound.tls.certificate_name).cloned();
            let server_name = if inbound.tls.server_name.trim().is_empty() {
                selected_certificate
                    .as_ref()
                    .map(|certificate| certificate.acme_domain.clone())
                    .unwrap_or_default()
            } else {
                inbound.tls.server_name.clone()
            };
            let tls = Some(TlsConfig {
                enabled: inbound.tls.enabled,
                server_name,
                certificate_name: inbound.tls.certificate_name.clone(),
            });
            let reality = Some(VlessRealityConfig {
                dest: inbound.vless.reality_dest.clone(),
                private_key: inbound.vless.reality_private_key.clone(),
                short_id: normalize_reality_short_ids(&inbound.vless.reality_short_ids),
                public_key: inbound.vless.reality_public_key.clone(),
                sni: inbound.vless.reality_sni.clone(),
                utls: inbound.vless.reality_utls.clone(),
                spider_x: inbound.vless.reality_spider_x.clone(),
            });
            let protocol = match normalized_protocol.as_str() {
                "HYSTERIA2" => Some(crate::pb::proxyswarm::inbound_config::Protocol::Hysteria2(
                    Hysteria2Config {
                        password: inbound.hysteria2.password.clone(),
                        obfs_type: inbound.hysteria2.obfs_type.clone(),
                        obfs_password: inbound.hysteria2.obfs_password.clone(),
                        up_mbps: inbound.hysteria2.up_mbps,
                        down_mbps: inbound.hysteria2.down_mbps,
                        tls: tls.clone(),
                        ignore_client_bandwidth: inbound.hysteria2.ignore_client_bandwidth,
                        masquerade: inbound.hysteria2.masquerade.clone(),
                        bbr_profile: inbound.hysteria2.bbr_profile.clone(),
                        brutal_debug: inbound.hysteria2.brutal_debug,
                    },
                )),
                "TRUSTTUNNEL" => Some(
                    crate::pb::proxyswarm::inbound_config::Protocol::Trusttunnel(
                        TrustTunnelConfig {
                            http1_upload_buffer_size: inbound.trust_tunnel.http1_upload_buffer_size,
                            http2_initial_connection_window_size: inbound
                                .trust_tunnel
                                .http2_initial_connection_window_size,
                            http2_initial_stream_window_size: inbound
                                .trust_tunnel
                                .http2_initial_stream_window_size,
                            http2_max_concurrent_streams: inbound
                                .trust_tunnel
                                .http2_max_concurrent_streams,
                            http2_max_frame_size: inbound.trust_tunnel.http2_max_frame_size,
                            http2_header_table_size: inbound.trust_tunnel.http2_header_table_size,
                            tls: tls.clone(),
                        },
                    ),
                ),
                "NAIVEPROXY" => Some(crate::pb::proxyswarm::inbound_config::Protocol::Naiveproxy(
                    NaiveProxyConfig {
                        username: inbound.naive_proxy.username.clone(),
                        password: inbound.naive_proxy.password.clone(),
                        protocol: inbound.naive_proxy.protocol.clone(),
                        target: inbound.naive_proxy.target.clone(),
                        tls: tls.clone(),
                    },
                )),
                "WIREGUARD" => Some(crate::pb::proxyswarm::inbound_config::Protocol::Wireguard(
                    WireGuardConfig {
                        private_key: inbound.wireguard.private_key.clone(),
                        workers: inbound.wireguard.workers,
                        addresses: split_lines_csv(&inbound.wireguard.addresses),
                        peers: Vec::new(),
                        mtu: inbound.wireguard.mtu,
                        reserved: Vec::new(),
                        domain_strategy: inbound.wireguard.domain_strategy.clone(),
                    },
                )),
                "SOCKS5" => Some(crate::pb::proxyswarm::inbound_config::Protocol::Socks5(
                    Socks5InboundConfig {
                        username: inbound.socks5.username.clone(),
                        password: inbound.socks5.password.clone(),
                        udp_enabled: inbound.socks5.udp_enabled,
                    },
                )),
                "SHADOWSOCKS" => Some(
                    crate::pb::proxyswarm::inbound_config::Protocol::Shadowsocks(
                        ShadowsocksInboundConfig {
                            method: inbound.shadowsocks.method.clone(),
                            password: inbound.shadowsocks.password.clone(),
                            udp_enabled: inbound.shadowsocks.udp_enabled,
                        },
                    ),
                ),
                _ => Some(crate::pb::proxyswarm::inbound_config::Protocol::Vless(
                    VlessConfig {
                        uuid: String::new(),
                        flow: inbound.vless.flow.clone(),
                        security: security_from(&inbound.vless.security),
                        transmission: vless_transmission_from(&inbound.vless.transmission),
                        tls,
                        reality,
                    },
                )),
            };

            InboundConfig {
                name: inbound.name.clone(),
                listen: inbound.listen.clone(),
                port: inbound.port,
                accounts: node_accounts.clone(),
                enabled: inbound.enabled,
                core: core_from(&inbound.core_type),
                protocol,
            }
        })
        .collect();

    let mut outbounds = vec![];

    for outbound in normalized_outbounds(draft) {
        if !outbound.enabled && outbound.outbound_type.trim().to_uppercase() != "BLOCK" {
            continue;
        }
        match outbound.outbound_type.trim().to_uppercase().as_str() {
            "DIRECT" if !outbound.name.trim().is_empty() => outbounds.push(OutboundConfig {
                tag: outbound.name.clone(),
                r#type: OutboundType::Direct as i32,
                settings: None,
            }),
            "BLOCK" if !outbound.name.trim().is_empty() => outbounds.push(OutboundConfig {
                tag: outbound.name.clone(),
                r#type: OutboundType::Block as i32,
                settings: None,
            }),
            "VLESS" if !outbound.vless.tag.trim().is_empty() => outbounds.push(OutboundConfig {
                tag: outbound.vless.tag.clone(),
                r#type: OutboundType::Vless as i32,
                settings: Some(outbound_config::Settings::Vless(VlessOutboundConfig {
                    server: outbound.vless.server.clone(),
                    port: outbound.vless.port,
                    uuid: if outbound.vless.uuid.trim().is_empty() {
                        uuid::Uuid::new_v4().to_string()
                    } else {
                        outbound.vless.uuid.clone()
                    },
                    flow: outbound.vless.flow.clone(),
                    security: security_from(&outbound.vless.security),
                    transmission: vless_transmission_from(&outbound.vless.transmission),
                })),
            }),
            "TRUSTTUNNEL" if !outbound.trust_tunnel.tag.trim().is_empty() => {
                outbounds.push(OutboundConfig {
                    tag: outbound.trust_tunnel.tag.clone(),
                    r#type: OutboundType::Trusttunnel as i32,
                    settings: Some(outbound_config::Settings::Trusttunnel(TrustTunnelConfig {
                        http1_upload_buffer_size: outbound.trust_tunnel.http1_upload_buffer_size,
                        http2_initial_connection_window_size: outbound
                            .trust_tunnel
                            .http2_initial_connection_window_size,
                        http2_initial_stream_window_size: outbound
                            .trust_tunnel
                            .http2_initial_stream_window_size,
                        http2_max_concurrent_streams: outbound
                            .trust_tunnel
                            .http2_max_concurrent_streams,
                        http2_max_frame_size: outbound.trust_tunnel.http2_max_frame_size,
                        http2_header_table_size: outbound.trust_tunnel.http2_header_table_size,
                        tls: None,
                    })),
                })
            }
            "WIREGUARD" if !outbound.name.trim().is_empty() => {
                let peers: Vec<WireGuardPeer> = outbound
                    .wireguard
                    .peers
                    .iter()
                    .filter(|peer| !peer.public_key.trim().is_empty())
                    .map(|peer| WireGuardPeer {
                        public_key: peer.public_key.clone(),
                        endpoint: peer.endpoint.clone(),
                        allowed_ips: {
                            let parsed = split_lines_csv(&peer.allowed_ips);
                            if parsed.is_empty() {
                                vec!["0.0.0.0/0".to_string(), "::/0".to_string()]
                            } else {
                                parsed
                            }
                        },
                        reserved: Vec::new(),
                        keepalive: 0,
                        pre_shared_key: String::new(),
                    })
                    .collect();
                let reserved: Vec<u32> = split_lines_csv(&outbound.wireguard.reserved)
                    .into_iter()
                    .filter_map(|value| value.parse::<u32>().ok())
                    .collect();
                outbounds.push(OutboundConfig {
                    tag: outbound.name.clone(),
                    r#type: OutboundType::Wireguard as i32,
                    settings: Some(outbound_config::Settings::Wireguard(WireGuardConfig {
                        private_key: outbound.wireguard.private_key.clone(),
                        addresses: split_lines_csv(&outbound.wireguard.addresses),
                        peers,
                        mtu: outbound.wireguard.mtu,
                        workers: outbound.wireguard.workers,
                        reserved,
                        domain_strategy: outbound.wireguard.domain_strategy.clone(),
                    })),
                })
            }
            "SOCKS5" if !outbound.socks5.tag.trim().is_empty() => outbounds.push(OutboundConfig {
                tag: outbound.socks5.tag.clone(),
                r#type: OutboundType::Socks5 as i32,
                settings: Some(outbound_config::Settings::Socks5(Socks5OutboundConfig {
                    server: outbound.socks5.server.clone(),
                    port: outbound.socks5.port,
                    username: outbound.socks5.username.clone(),
                    password: outbound.socks5.password.clone(),
                })),
            }),
            "SHADOWSOCKS" if !outbound.shadowsocks.tag.trim().is_empty() => {
                outbounds.push(OutboundConfig {
                    tag: outbound.shadowsocks.tag.clone(),
                    r#type: OutboundType::Shadowsocks as i32,
                    settings: Some(outbound_config::Settings::Shadowsocks(
                        ShadowsocksOutboundConfig {
                            server: outbound.shadowsocks.server.clone(),
                            port: outbound.shadowsocks.port,
                            method: outbound.shadowsocks.method.clone(),
                            password: outbound.shadowsocks.password.clone(),
                            plugin: outbound.shadowsocks.plugin.clone(),
                            plugin_opts: outbound.shadowsocks.plugin_opts.clone(),
                            prefix: outbound.shadowsocks.prefix.clone(),
                            udp_enabled: outbound.shadowsocks.udp_enabled,
                        },
                    )),
                })
            }
            _ => {}
        }
    }

    let outbound_tags: std::collections::HashSet<String> = outbounds
        .iter()
        .map(|outbound| outbound.tag.clone())
        .collect();

    FullConfig {
        master_key: draft.master_key.clone(),
        inbounds,
        certificates: certificates
            .into_iter()
            .map(|certificate| CertificateConfig {
                id: certificate.id,
                name: certificate.name,
                cert_type: certificate.cert_type,
                acme_type: certificate.acme_type,
                acme_ca: certificate.acme_ca,
                acme_email: certificate.acme_email,
                acme_domain: certificate.acme_domain,
                certificate_path: certificate.certificate_path,
                key_path: certificate.key_path,
                acme_port: certificate.acme_port,
                acme_http_port: certificate.acme_http_port,
                certificate_pem: certificate.certificate_pem,
                key_pem: certificate.key_pem,
            })
            .collect(),
        accounts: node_accounts,
        outbounds,
        routing_rules: normalized_routing_rules(draft)
            .into_iter()
            .filter(|rule| !rule.outbound_tag.trim().is_empty())
            .map(|rule| RoutingRule {
                domain: split_lines_csv(&rule.domain),
                ip: split_lines_csv(&rule.ip),
                port: split_lines_csv(&rule.port),
                transport: split_lines_csv(&rule.transport),
                protocol: split_lines_csv(&rule.protocol),
                outbound_tag: if outbound_tags.contains(&rule.outbound_tag) {
                    rule.outbound_tag
                } else {
                    "block".to_string()
                },
                inbound_tag: split_lines_csv(&rule.inbound_tag),
                user: split_lines_csv(&rule.user),
            })
            .collect(),
        dns: build_dns_config(draft),
        link_remark_template: draft.link_remark_template.clone(),
    }
}

fn persist_revision(
    state: &UseStateHandle<State>,
    node_id: &str,
    draft: &NodeConfigDraft,
) -> Option<String> {
    let mut next_state = (**state).clone();
    let node = next_state
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id)?;
    let revision_id = uuid::Uuid::new_v4().to_string();
    let mut persisted_draft = draft.clone();
    persisted_draft.master_key = node.master_key.clone();
    sync_draft(&mut persisted_draft);
    let revision = NodeConfigRevision {
        id: revision_id.clone(),
        created_at: today_string(),
        config: persisted_draft.clone(),
    };
    node.revisions.push(revision);
    node.active_revision_id = revision_id.clone();
    node.config = persisted_draft;

    next_state.save();
    state.set(next_state);
    Some(revision_id)
}

fn build_vless_access_link(
    draft: &NodeConfigDraft,
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
    account: &AccountInfo,
) -> Result<String, String> {
    if inbound.protocol.trim().to_uppercase() != "VLESS" {
        return Err("Selected inbound is not VLESS".to_string());
    }

    let token = if !account.token.trim().is_empty() {
        account.token.trim().to_string()
    } else {
        account.id.trim().to_string()
    };
    if token.is_empty() {
        return Err("User token is empty".to_string());
    }

    let mut host = normalized_public_ip_host(node)?;
    if host.contains(':') && !host.starts_with('[') && !host.contains('.') {
        host = format!("[{}]", host);
    }

    let security = inbound.vless.security.trim().to_uppercase();
    let transmission = vless_link_type(&inbound.vless.transmission).to_string();

    let mut query: Vec<(String, String)> = vec![
        ("encryption".to_string(), "none".to_string()),
        ("type".to_string(), transmission.clone()),
    ];

    if !inbound.vless.flow.trim().is_empty() {
        query.push(("flow".to_string(), inbound.vless.flow.trim().to_string()));
    }

    let default_sni = if inbound.tls.server_name.trim().is_empty() {
        host.clone()
    } else {
        inbound.tls.server_name.trim().to_string()
    };

    match security.as_str() {
        "TLS" => {
            query.push(("security".to_string(), "tls".to_string()));
            query.push(("sni".to_string(), default_sni.clone()));
        }
        "REALITY" => {
            query.push(("security".to_string(), "reality".to_string()));
            let sni = if inbound.vless.reality_sni.trim().is_empty() {
                default_sni.clone()
            } else {
                inbound.vless.reality_sni.trim().to_string()
            };
            query.push(("sni".to_string(), sni));
            if !inbound.vless.reality_public_key.trim().is_empty() {
                query.push((
                    "pbk".to_string(),
                    inbound.vless.reality_public_key.trim().to_string(),
                ));
            }
            if !inbound.vless.reality_spider_x.trim().is_empty() {
                query.push((
                    "spx".to_string(),
                    inbound.vless.reality_spider_x.trim().to_string(),
                ));
            }
            if !inbound.vless.reality_utls.trim().is_empty() {
                query.push((
                    "fp".to_string(),
                    inbound.vless.reality_utls.trim().to_string(),
                ));
            }
        }
        _ => {
            query.push(("security".to_string(), "none".to_string()));
        }
    }

    match transmission.as_str() {
        "ws" | "httpupgrade" | "splithttp" | "http" => {
            query.push(("path".to_string(), "/".to_string()));
            query.push(("host".to_string(), default_sni.clone()));
        }
        "grpc" => {
            query.push(("serviceName".to_string(), "".to_string()));
        }
        _ => {}
    }

    let query_string = query
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, js_sys::encode_uri_component(&v)))
        .collect::<Vec<_>>()
        .join("&");

    Ok(format!(
        "vless://{}@{}:{}?{}#{}",
        token,
        host,
        inbound.port,
        query_string,
        js_sys::encode_uri_component(&rendered_link_remark(draft, node, inbound, account))
    ))
}

fn normalized_public_ip_host(node: &ProxyNode) -> Result<String, String> {
    let mut host = node.public_ip.trim().to_string();
    if let Some(stripped) = host.strip_prefix("http://") {
        host = stripped.to_string();
    } else if let Some(stripped) = host.strip_prefix("https://") {
        host = stripped.to_string();
    }
    if let Some((base, _)) = host.split_once('/') {
        host = base.to_string();
    }
    if let Some(stripped) = host.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        host = stripped.to_string();
    }
    if let Some((base, port)) = host.rsplit_once(':') {
        if !base.contains(':') || base.contains('.') {
            if port.parse::<u16>().is_ok() {
                host = base.to_string();
            }
        }
    }
    host = host.trim().to_string();
    if host.is_empty() {
        return Err("Node public IP address is empty".to_string());
    }
    Ok(host)
}

fn normalized_node_host(node: &ProxyNode) -> Result<String, String> {
    let mut host = if !node.address.trim().is_empty() {
        node.address.trim().to_string()
    } else {
        node.public_ip.trim().to_string()
    };
    if let Some(stripped) = host.strip_prefix("http://") {
        host = stripped.to_string();
    } else if let Some(stripped) = host.strip_prefix("https://") {
        host = stripped.to_string();
    }
    if let Some((base, _)) = host.split_once('/') {
        host = base.to_string();
    }
    if let Some(stripped) = host.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        host = stripped.to_string();
    }
    if let Some((base, port)) = host.rsplit_once(':') {
        if !base.contains(':') || base.contains('.') {
            if port.parse::<u16>().is_ok() {
                host = base.to_string();
            }
        }
    }
    host = host.trim().to_string();
    if host.is_empty() {
        return Err("Node access address/public IP is empty".to_string());
    }
    Ok(host)
}

fn build_trusttunnel_access_link(
    draft: &NodeConfigDraft,
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
    account: &AccountInfo,
) -> Result<String, String> {
    if inbound.protocol.trim().to_uppercase() != "TRUSTTUNNEL" {
        return Err("Selected inbound is not TrustTunnel".to_string());
    }

    let host = normalized_node_host(node)?;
    let username = if !account.name.trim().is_empty() {
        account.name.trim().to_string()
    } else {
        account.id.trim().to_string()
    };
    if username.is_empty() {
        return Err("User name is empty".to_string());
    }

    let password = if !account.token.trim().is_empty() {
        account.token.trim().to_string()
    } else {
        account.id.trim().to_string()
    };
    if password.is_empty() {
        return Err("User token is empty".to_string());
    }

    let custom_sni = inbound.tls.server_name.trim().to_string();
    let config_name = rendered_link_remark(draft, node, inbound, account);

    let config = DeepLinkConfig::builder()
        .hostname(host.clone())
        .addresses(vec![format!("{}:{}", host, inbound.port)])
        .username(username)
        .password(password)
        .custom_sni((!custom_sni.is_empty() && custom_sni != host).then_some(custom_sni))
        .name((!config_name.trim().is_empty()).then_some(config_name))
        .build()
        .map_err(|err| format!("Failed to build TrustTunnel deep-link config: {err}"))?;

    encode(&config).map_err(|err| format!("Failed to encode TrustTunnel deep-link: {err}"))
}

fn build_hysteria2_access_link(
    draft: &NodeConfigDraft,
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
    account: &AccountInfo,
) -> Result<String, String> {
    if inbound.protocol.trim().to_uppercase() != "HYSTERIA2" {
        return Err("Selected inbound is not Hysteria2".to_string());
    }

    let host = normalized_public_ip_host(node)?;
    let password = if !account.token.trim().is_empty() {
        account.token.trim().to_string()
    } else {
        account.id.trim().to_string()
    };
    if password.is_empty() {
        return Err("User token is empty".to_string());
    }

    let mut query = Vec::new();
    let sni = if inbound.tls.server_name.trim().is_empty() {
        host.clone()
    } else {
        inbound.tls.server_name.trim().to_string()
    };
    if !sni.is_empty() {
        query.push(("sni", sni));
    }
    if !inbound.hysteria2.obfs_type.trim().is_empty() {
        query.push(("obfs", inbound.hysteria2.obfs_type.trim().to_string()));
    }
    if !inbound.hysteria2.obfs_password.trim().is_empty() {
        query.push((
            "obfs-password",
            inbound.hysteria2.obfs_password.trim().to_string(),
        ));
    }
    if !inbound.hysteria2.masquerade.trim().is_empty() {
        query.push((
            "masquerade",
            inbound.hysteria2.masquerade.trim().to_string(),
        ));
    }

    let query_string = query
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, js_sys::encode_uri_component(&v)))
        .collect::<Vec<_>>()
        .join("&");
    let suffix = if query_string.is_empty() {
        String::new()
    } else {
        format!("?{}", query_string)
    };

    Ok(format!(
        "hysteria2://{}@{}:{}{}#{}",
        js_sys::encode_uri_component(&password),
        host,
        inbound.port,
        suffix,
        js_sys::encode_uri_component(&rendered_link_remark(draft, node, inbound, account))
    ))
}

fn build_access_link(
    draft: &NodeConfigDraft,
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
    account: &AccountInfo,
) -> Result<String, String> {
    match inbound.protocol.trim().to_uppercase().as_str() {
        "VLESS" => build_vless_access_link(draft, node, inbound, account),
        "TRUSTTUNNEL" => build_trusttunnel_access_link(draft, node, inbound, account),
        "HYSTERIA2" => build_hysteria2_access_link(draft, node, inbound, account),
        _ => Err("Access link is available only for VLESS, Hysteria2, and TrustTunnel inbounds".to_string()),
    }
}

async fn copy_to_clipboard(text: String) -> Result<(), String> {
    let Some(window) = window() else {
        return Err("Clipboard unavailable".to_string());
    };

    let navigator = js_sys::Reflect::get(&window, &JsValue::from_str("navigator"))
        .map_err(|_| "Clipboard unavailable".to_string())?;
    let clipboard = js_sys::Reflect::get(&navigator, &JsValue::from_str("clipboard"))
        .map_err(|_| "Clipboard unavailable".to_string())?;
    let write_text = js_sys::Reflect::get(&clipboard, &JsValue::from_str("writeText"))
        .map_err(|_| "Clipboard unavailable".to_string())?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "Clipboard unavailable".to_string())?;

    let promise = write_text
        .call1(&clipboard, &JsValue::from_str(&text))
        .map_err(|_| "Clipboard unavailable".to_string())?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "Clipboard unavailable".to_string())?;

    JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|_| "Copy failed".to_string())
}

fn qr_svg(value: &str) -> Option<String> {
    let qr = QrCode::encode_text(value, QrCodeEcc::Medium).ok()?;
    let border = 4;
    let size = qr.size();
    let total = size + border * 2;
    let mut path = String::new();

    for y in 0..size {
        for x in 0..size {
            if qr.get_module(x, y) {
                path.push_str(&format!("M{},{}h1v1h-1z", x + border, y + border));
            }
        }
    }

    Some(format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {0} {0}' shape-rendering='crispEdges'><rect width='100%' height='100%' fill='white'/><path d='{1}' fill='black'/></svg>",
        total, path
    ))
}

#[derive(Properties, PartialEq)]
struct SectionProps {
    title: AttrValue,
    #[prop_or_default]
    children: Children,
}

#[derive(Properties, PartialEq)]
struct ConfirmPopupProps {
    title: AttrValue,
    body: AttrValue,
    confirm_label: AttrValue,
    #[prop_or(false)]
    align_actions_end: bool,
    on_confirm: Callback<()>,
    on_close: Callback<()>,
}

#[function_component(ConfirmPopup)]
fn confirm_popup(props: &ConfirmPopupProps) -> Html {
    let on_confirm = {
        let on_confirm = props.on_confirm.clone();
        Callback::from(move |_| on_confirm.emit(()))
    };
    let on_close_btn = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    html! {
        <Popup title={props.title.clone()} size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-6">
                <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant); line-height: 1.5;">
                    { props.body.clone() }
                </div>
                <div class="md3-popup-actions" style={if props.align_actions_end { "justify-content: flex-end;" } else { "" }}>
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={on_close_btn} />
                    <Button label={props.confirm_label.to_string()} button_type={ButtonType::Filled} onclick={on_confirm} />
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct AcmeLogsPopupProps {
    title: AttrValue,
    logs: Vec<String>,
    loading: bool,
    success: bool,
    error: String,
    on_close: Callback<()>,
}

#[function_component(AcmeLogsPopup)]
fn acme_logs_popup(props: &AcmeLogsPopupProps) -> Html {
    let log_text = if props.logs.is_empty() {
        "Waiting for node response...".to_string()
    } else {
        props.logs.join("\n")
    };

    html! {
        <Popup title={props.title.clone()} size={PopupSize::Lg} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                    {
                        if props.loading {
                            "Request in progress..."
                        } else if props.success {
                            "Certificate request finished successfully."
                        } else {
                            "Certificate request failed."
                        }
                    }
                </div>
                {
                    if !props.error.is_empty() {
                        html! {
                            <div class="text-sm" style="color: #F2B8B5;">
                                { props.error.clone() }
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                <pre class="md3-code-block">{ log_text }</pre>
            </div>
        </Popup>
    }
}

#[function_component(ConfigSection)]
fn config_section(props: &SectionProps) -> Html {
    html! {
        <div class="md3-card bg-surface-container">
            <h2 class="text-xl font-semibold mb-4">{ props.title.clone() }</h2>
            <div class="space-y-4">
                { for props.children.iter() }
            </div>
        </div>
    }
}

fn option_bool_value(value: Option<bool>) -> String {
    match value {
        Some(true) => "true".to_string(),
        Some(false) => "false".to_string(),
        None => String::new(),
    }
}

fn option_bool_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Enabled",
        Some(false) => "Disabled",
        None => "Inherit",
    }
}

fn option_bool_from_value(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "enabled" | "on" => Some(true),
        "false" | "disabled" | "off" => Some(false),
        _ => None,
    }
}

fn default_dns_server_draft() -> DnsServerDraft {
    DnsServerDraft {
        port: 53,
        timeout_ms: 5000,
        ..DnsServerDraft::default()
    }
}

fn dns_server_summary(server: &DnsServerDraft) -> String {
    let mut parts = vec![format!("Port {}", server.port)];
    if !server.client_ip.trim().is_empty() {
        parts.push(format!("Client {}", server.client_ip.trim()));
    }
    if !server.query_strategy.trim().is_empty() {
        parts.push(server.query_strategy.trim().to_string());
    }
    parts.join(" · ")
}

fn dns_server_details(server: &DnsServerDraft) -> String {
    let mut details = Vec::new();
    if !server.domains.trim().is_empty() {
        details.push(format!("Domains: {}", server.domains.replace('\n', ", ")));
    }
    if !server.expect_ips.trim().is_empty() {
        details.push(format!("Expect: {}", server.expect_ips.replace('\n', ", ")));
    }
    if !server.unexpected_ips.trim().is_empty() {
        details.push(format!(
            "Unexpected: {}",
            server.unexpected_ips.replace('\n', ", ")
        ));
    }
    if details.is_empty() {
        "-".to_string()
    } else {
        details.join(" · ")
    }
}

fn dns_host_summary(host: &DnsHostDraft) -> String {
    let values = split_lines_csv(&host.values);
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

#[derive(Properties, PartialEq)]
struct DnsServerEditorPopupProps {
    server: DnsServerDraft,
    is_new: bool,
    on_close: Callback<()>,
    on_save: Callback<DnsServerDraft>,
}

#[function_component(DnsServerEditorPopup)]
fn dns_server_editor_popup(props: &DnsServerEditorPopupProps) -> Html {
    let server = use_state(|| props.server.clone());

    let update_text = |mutator: fn(&mut DnsServerDraft, String)| {
        let server = server.clone();
        Callback::from(move |value: String| {
            let mut next = (*server).clone();
            mutator(&mut next, value);
            server.set(next);
        })
    };
    let update_u32 = |mutator: fn(&mut DnsServerDraft, u32)| {
        let server = server.clone();
        Callback::from(move |value: String| {
            let mut next = (*server).clone();
            mutator(&mut next, value.parse::<u32>().unwrap_or(0));
            server.set(next);
        })
    };
    let update_u64 = |mutator: fn(&mut DnsServerDraft, u64)| {
        let server = server.clone();
        Callback::from(move |value: String| {
            let mut next = (*server).clone();
            mutator(&mut next, value.parse::<u64>().unwrap_or(0));
            server.set(next);
        })
    };
    let update_bool = |mutator: fn(&mut DnsServerDraft, bool)| {
        let server = server.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            let mut next = (*server).clone();
            mutator(&mut next, input.checked());
            server.set(next);
        })
    };
    let update_option_bool = |mutator: fn(&mut DnsServerDraft, Option<bool>)| {
        let server = server.clone();
        Callback::from(move |value: String| {
            let mut next = (*server).clone();
            mutator(&mut next, option_bool_from_value(&value));
            server.set(next);
        })
    };

    let data = (*server).clone();
    let popup_title = if props.is_new {
        "Add DNS Server"
    } else {
        "Edit DNS Server"
    };

    let on_save = {
        let on_save = props.on_save.clone();
        let server = server.clone();
        Callback::from(move |_| on_save.emit((*server).clone()))
    };

    html! {
        <Popup title={popup_title} size={PopupSize::Lg} on_close={props.on_close.clone()}>
            <div class="space-y-6">
                <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                    <TextBox label="Address" value={data.address.clone()} onchange={update_text(|server, value| server.address = value)} placeholder="https://1.1.1.1/dns-query" />
                    <TextBox label="Port" value={data.port.to_string()} onchange={update_u32(|server, value| server.port = value)} input_type="number" />
                    <TextBox label="Tag" value={data.tag.clone()} onchange={update_text(|server, value| server.tag = value)} placeholder="cloudflare" />
                    <TextBox label="Client IP" value={data.client_ip.clone()} onchange={update_text(|server, value| server.client_ip = value)} placeholder="Optional" />
                    <TextBox label="Query Strategy" value={data.query_strategy.clone()} onchange={update_text(|server, value| server.query_strategy = value)} placeholder="Optional" />
                    <TextBox label="Timeout ms" value={data.timeout_ms.to_string()} onchange={update_u64(|server, value| server.timeout_ms = value)} input_type="number" />
                </div>
                <TextBox label="Domains" value={data.domains.clone()} onchange={update_text(|server, value| server.domains = value)} is_textarea={true} placeholder="example.com, api.example.com" />
                <TextBox label="Expected IPs" value={data.expect_ips.clone()} onchange={update_text(|server, value| server.expect_ips = value)} is_textarea={true} placeholder="1.1.1.1, 1.0.0.1" />
                <TextBox label="Unexpected IPs" value={data.unexpected_ips.clone()} onchange={update_text(|server, value| server.unexpected_ips = value)} is_textarea={true} placeholder="0.0.0.0, 127.0.0.1" />
                <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                    <Dropdown
                        label="Disable Cache"
                        value={option_bool_value(data.disable_cache)}
                        options={vec![
                            DropdownOption { value: String::new(), label: "Inherit".to_string() },
                            DropdownOption { value: "true".to_string(), label: "Enabled".to_string() },
                            DropdownOption { value: "false".to_string(), label: "Disabled".to_string() },
                        ]}
                        onchange={update_option_bool(|server, value| server.disable_cache = value)}
                    />
                    <Dropdown
                        label="Serve Stale"
                        value={option_bool_value(data.serve_stale)}
                        options={vec![
                            DropdownOption { value: String::new(), label: "Inherit".to_string() },
                            DropdownOption { value: "true".to_string(), label: "Enabled".to_string() },
                            DropdownOption { value: "false".to_string(), label: "Disabled".to_string() },
                        ]}
                        onchange={update_option_bool(|server, value| server.serve_stale = value)}
                    />
                    <TextBox
                        label="Serve Expired TTL"
                        value={data.serve_expired_ttl.map(|value| value.to_string()).unwrap_or_default()}
                        onchange={Callback::from({
                            let server = server.clone();
                            move |value: String| {
                                let mut next = (*server).clone();
                                next.serve_expired_ttl = value.parse::<u32>().ok();
                                server.set(next);
                            }
                        })}
                        input_type="number"
                        placeholder="Optional"
                    />
                    <SwitchField
                        label="Skip Fallback"
                        checked={data.skip_fallback}
                        onchange={update_bool(|server, value| server.skip_fallback = value)}
                    />
                    <SwitchField
                        label="Final Query"
                        checked={data.final_query}
                        onchange={update_bool(|server, value| server.final_query = value)}
                    />
                </div>
                <div class="md3-popup-actions">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    <Button label="Save" button_type={ButtonType::Filled} onclick={on_save} />
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct DnsHostEditorPopupProps {
    host: DnsHostDraft,
    is_new: bool,
    on_close: Callback<()>,
    on_save: Callback<DnsHostDraft>,
}

#[function_component(DnsHostEditorPopup)]
fn dns_host_editor_popup(props: &DnsHostEditorPopupProps) -> Html {
    let host = use_state(|| props.host.clone());
    let update_text = |mutator: fn(&mut DnsHostDraft, String)| {
        let host = host.clone();
        Callback::from(move |value: String| {
            let mut next = (*host).clone();
            mutator(&mut next, value);
            host.set(next);
        })
    };

    let data = (*host).clone();
    let popup_title = if props.is_new {
        "Add DNS Host"
    } else {
        "Edit DNS Host"
    };
    let on_save = {
        let on_save = props.on_save.clone();
        let host = host.clone();
        Callback::from(move |_| on_save.emit((*host).clone()))
    };

    html! {
        <Popup title={popup_title} size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-6">
                <TextBox label="Domain" value={data.domain.clone()} onchange={update_text(|host, value| host.domain = value)} placeholder="example.com" />
                <TextBox label="Values" value={data.values.clone()} onchange={update_text(|host, value| host.values = value)} is_textarea={true} placeholder="1.1.1.1, 8.8.8.8" />
                <div class="md3-popup-actions">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    <Button label="Save" button_type={ButtonType::Filled} onclick={on_save} />
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct CertificateEditorPopupProps {
    certificate: CertificateDraft,
    is_new: bool,
    on_close: Callback<()>,
    on_save: Callback<CertificateDraft>,
}

#[function_component(CertificateEditorPopup)]
fn certificate_editor_popup(props: &CertificateEditorPopupProps) -> Html {
    let certificate = use_state(|| props.certificate.clone());

    let update_text = |mutator: fn(&mut CertificateDraft, String)| {
        let certificate = certificate.clone();
        Callback::from(move |value: String| {
            let mut next = (*certificate).clone();
            mutator(&mut next, value);
            certificate.set(next);
        })
    };

    let data = (*certificate).clone();
    let popup_title = if props.is_new {
        "Add Certificate"
    } else {
        "Edit Certificate"
    };

    let read_clipboard_into = |mutator: fn(&mut CertificateDraft, String)| {
        let certificate = certificate.clone();
        Callback::from(move |_| {
            let certificate = certificate.clone();
            spawn_local(async move {
                let Some(window) = window() else {
                    return;
                };
                let navigator = window.navigator();
                let Ok(clipboard) =
                    js_sys::Reflect::get(&navigator, &JsValue::from_str("clipboard"))
                else {
                    return;
                };
                let Ok(read_text) =
                    js_sys::Reflect::get(&clipboard, &JsValue::from_str("readText"))
                else {
                    return;
                };
                let Ok(function) = read_text.dyn_into::<js_sys::Function>() else {
                    return;
                };
                let Ok(promise_value) = function.call0(&clipboard) else {
                    return;
                };
                let promise = js_sys::Promise::from(promise_value);
                let Ok(value) = JsFuture::from(promise).await else {
                    return;
                };
                if let Some(text) = value.as_string() {
                    let mut next = (*certificate).clone();
                    mutator(&mut next, text);
                    certificate.set(next);
                }
            });
        })
    };

    let import_file_into = |mutator: fn(&mut CertificateDraft, String)| {
        let certificate = certificate.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            let Some(files) = input.files() else {
                return;
            };
            let Some(file) = files.get(0) else {
                return;
            };
            let certificate = certificate.clone();
            spawn_local(async move {
                let promise = file.text();
                let Ok(value) = JsFuture::from(promise).await else {
                    return;
                };
                if let Some(text) = value.as_string() {
                    let mut next = (*certificate).clone();
                    mutator(&mut next, text);
                    certificate.set(next);
                }
            });
        })
    };

    html! {
        <Popup title={popup_title} size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <TextBox label="Name" value={data.name.clone()} onchange={update_text(|certificate, value| certificate.name = value)} />
                <Dropdown
                    label="Type"
                    value={data.cert_type.clone()}
                    options={vec![
                        DropdownOption { value: "CUSTOM".to_string(), label: "Custom".to_string() },
                        DropdownOption { value: "ACME".to_string(), label: "ACME".to_string() },
                    ]}
                    onchange={update_text(|certificate, value| certificate.cert_type = value)}
                />
                {
                    if data.cert_type == "ACME" {
                        html! {
                            <>
                                <Dropdown
                                    label="ACME Type"
                                    value={data.acme_type.clone()}
                                    options={vec![
                                        DropdownOption { value: "HTTP".to_string(), label: "HTTP".to_string() },
                                        DropdownOption { value: "TLS".to_string(), label: "TLS".to_string() },
                                        DropdownOption { value: "DNS".to_string(), label: "DNS".to_string() },
                                    ]}
                                    onchange={update_text(|certificate, value| certificate.acme_type = value)}
                                />
                                <Dropdown
                                    label="CA"
                                    value={data.acme_ca.clone()}
                                    options={vec![
                                        DropdownOption { value: "letsencrypt".to_string(), label: "Let's Encrypt".to_string() },
                                        DropdownOption { value: "zerossl".to_string(), label: "ZeroSSL".to_string() },
                                        DropdownOption { value: "google".to_string(), label: "Google Trust Services".to_string() },
                                        DropdownOption { value: "buypass".to_string(), label: "Buypass Go SSL".to_string() },
                                        DropdownOption { value: "sslcom".to_string(), label: "SSL.com".to_string() },
                                    ]}
                                    onchange={update_text(|certificate, value| certificate.acme_ca = value)}
                                />
                                <TextBox label="Email" value={data.acme_email.clone()} onchange={update_text(|certificate, value| certificate.acme_email = value)} />
                                <TextBox label="Domain" value={data.acme_domain.clone()} onchange={update_text(|certificate, value| certificate.acme_domain = value)} />
                                {
                                    match data.acme_type.as_str() {
                                        "HTTP" => html! {
                                            <TextBox
                                                label="Port"
                                                value={data.acme_http_port.to_string()}
                                                onchange={update_text(|certificate, value| certificate.acme_http_port = value.parse().unwrap_or(0))}
                                                input_type="number"
                                            />
                                        },
                                        "TLS" => html! {
                                            <TextBox
                                                label="Port"
                                                value={data.acme_port.to_string()}
                                                onchange={update_text(|certificate, value| certificate.acme_port = value.parse().unwrap_or(0))}
                                                input_type="number"
                                            />
                                        },
                                        _ => html! {},
                                    }
                                }
                            </>
                        }
                    } else {
                        html! {
                            <>
                                <Dropdown
                                    label="Certificate Source"
                                    value={data.source.clone()}
                                    options={vec![
                                        DropdownOption { value: "PATH".to_string(), label: "On-node paths".to_string() },
                                        DropdownOption { value: "INLINE".to_string(), label: "Paste / import PEM".to_string() },
                                    ]}
                                    onchange={update_text(|certificate, value| certificate.source = value)}
                                />
                                {
                                    if data.source == "INLINE" {
                                        html! {
                                            <>
                                                <div class="space-y-2">
                                                    <TextBox label="Certificate PEM" value={data.certificate_pem.clone()} onchange={update_text(|certificate, value| certificate.certificate_pem = value)} is_textarea={true} />
                                                    <div class="flex" style="gap: 0.75rem;">
                                                        <Button label="Paste Certificate" button_type={ButtonType::Outlined} onclick={read_clipboard_into(|certificate, value| certificate.certificate_pem = value)} />
                                                        <label class="md3-btn md3-btn--outlined" style="cursor: pointer;">
                                                            { "Import Certificate File" }
                                                            <input type="file" accept=".pem,.crt,.cer,.txt" style="display: none;" onchange={import_file_into(|certificate, value| certificate.certificate_pem = value)} />
                                                        </label>
                                                    </div>
                                                </div>
                                                <div class="space-y-2">
                                                    <TextBox label="Key PEM" value={data.key_pem.clone()} onchange={update_text(|certificate, value| certificate.key_pem = value)} is_textarea={true} />
                                                    <div class="flex" style="gap: 0.75rem;">
                                                        <Button label="Paste Key" button_type={ButtonType::Outlined} onclick={read_clipboard_into(|certificate, value| certificate.key_pem = value)} />
                                                        <label class="md3-btn md3-btn--outlined" style="cursor: pointer;">
                                                            { "Import Key File" }
                                                            <input type="file" accept=".pem,.key,.txt" style="display: none;" onchange={import_file_into(|certificate, value| certificate.key_pem = value)} />
                                                        </label>
                                                    </div>
                                                </div>
                                            </>
                                        }
                                    } else {
                                        html! {
                                            <>
                                                <TextBox label="Certificate Path" value={data.certificate_path.clone()} onchange={update_text(|certificate, value| certificate.certificate_path = value)} placeholder="/etc/ssl/certs/node.crt" />
                                                <TextBox label="Key Path" value={data.key_path.clone()} onchange={update_text(|certificate, value| certificate.key_path = value)} placeholder="/etc/ssl/private/node.key" />
                                            </>
                                        }
                                    }
                                }
                            </>
                        }
                    }
                }
            </div>
            <div class="md3-popup-actions" style="justify-content: flex-end;">
                <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                    let on_close = props.on_close.clone();
                    move |_| on_close.emit(())
                })} />
                <Button label={if props.is_new { "Create Certificate" } else { "Apply Changes" }} button_type={ButtonType::Filled} onclick={Callback::from({
                    let on_save = props.on_save.clone();
                    let certificate = certificate.clone();
                    move |_| on_save.emit((*certificate).clone())
                })} />
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct InboundEditorPopupProps {
    inbound: InboundEntryDraft,
    certificates: Vec<CertificateDraft>,
    is_new: bool,
    on_close: Callback<()>,
    on_save: Callback<InboundEntryDraft>,
}

fn inbound_creation_steps(inbound: &InboundEntryDraft) -> usize {
    match inbound.protocol.as_str() {
        "VLESS" => {
            if inbound.vless.security == "REALITY" {
                4
            } else if inbound.vless.security == "TLS" {
                4
            } else {
                3
            }
        }
        "HYSTERIA2" => 4,
        "TRUSTTUNNEL" => 4,
        "WIREGUARD" => 3,
        _ => 3,
    }
}

fn outbound_creation_steps(outbound: &OutboundEntryDraft) -> usize {
    match outbound.outbound_type.as_str() {
        "VLESS" => 3,
        "WIREGUARD" => 3,
        "SOCKS5" => 3,
        _ => 3,
    }
}

#[function_component(InboundEditorPopup)]
fn inbound_editor_popup(props: &InboundEditorPopupProps) -> Html {
    let inbound = use_state(|| props.inbound.clone());
    let step = use_state(|| 0usize);
    let certificate_options = if props.certificates.is_empty() {
        vec![DropdownOption {
            value: "".to_string(),
            label: "No certificates yet".to_string(),
        }]
    } else {
        props
            .certificates
            .iter()
            .map(|certificate| DropdownOption {
                value: certificate.name.clone(),
                label: format!(
                    "{} ({})",
                    certificate_display_name(certificate),
                    certificate.cert_type
                ),
            })
            .collect::<Vec<_>>()
    };

    let update_text = |mutator: fn(&mut InboundEntryDraft, String)| {
        let inbound = inbound.clone();
        Callback::from(move |value: String| {
            let mut next = (*inbound).clone();
            mutator(&mut next, value);
            inbound.set(next);
        })
    };

    let update_bool = |mutator: fn(&mut InboundEntryDraft, bool)| {
        let inbound = inbound.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            let mut next = (*inbound).clone();
            mutator(&mut next, input.checked());
            inbound.set(next);
        })
    };

    let data = (*inbound).clone();
    let popup_title: AttrValue = if props.is_new {
        "Add Inbound"
    } else {
        "Edit Inbound"
    }
    .into();
    let total_steps = inbound_creation_steps(&data);

    if true {
        return html! {
            <Popup
                title={popup_title}
                size={PopupSize::Md}
                on_close={props.on_close.clone()}
            >
                <div class="space-y-6">
                    <div class="text-sm opacity-70">{ format!("Step {} of {}", *step + 1, total_steps) }</div>

                    <div key={format!("inbound-step-{}", *step)} class="md3-wizard-page">
                    {
                        match *step {
                            0 => html! {
                                <ConfigSection title="General">
                                    <TextBox label="Name" value={data.name.clone()} onchange={update_text(|inbound, value| inbound.name = value)} />
                                    <TextBox label="Listen Address" value={data.listen.clone()} onchange={update_text(|inbound, value| inbound.listen = value)} />
                                    <TextBox
                                        label="Port"
                                        value={data.port.to_string()}
                                        onchange={update_text(|inbound, value| inbound.port = value.parse().unwrap_or(0))}
                                        input_type="number"
                                        action_icon={Some("icon-sync".to_string())}
                                        action_label={Some("Randomize port".to_string())}
                                        action_onclick={Some(Callback::from({
                                            let inbound = inbound.clone();
                                            move |_| {
                                                let mut next = (*inbound).clone();
                                                next.port = random_port();
                                                inbound.set(next);
                                            }
                                        }))}
                                    />
                                    <SwitchField
                                        label="Inbound enabled"
                                        checked={data.enabled}
                                        onchange={update_bool(|inbound, value| inbound.enabled = value)}
                                    />
                                    <Dropdown
                                        label="Core"
                                        value={data.core_type.clone()}
                                        options={vec![
                                            DropdownOption { value: "".to_string(), label: "None".to_string() },
                                            DropdownOption { value: "XRAY".to_string(), label: "Xray".to_string() },
                                            DropdownOption { value: "SING_BOX".to_string(), label: "Sing-Box".to_string() },
                                            DropdownOption { value: "TRUSTTUNNEL".to_string(), label: "TrustTunnel".to_string() },
                                        ]}
                                        onchange={Callback::from({
                                            let inbound = inbound.clone();
                                            move |value: String| {
                                                let mut next = (*inbound).clone();
                                                next.core_type = value.clone();
                                                if value.trim().is_empty() {
                                                    next.protocol.clear();
                                                }
                                                next.protocol = normalize_protocol_for_core(&value, &next.protocol);
                                                inbound.set(next);
                                            }
                                        })}
                                    />
                                    <Dropdown
                                        label="Protocol"
                                        value={data.protocol.clone()}
                                        disabled={data.core_type == "TRUSTTUNNEL"}
                                        options={protocol_options_for_core(&data.core_type)}
                                        onchange={Callback::from({
                                            let inbound = inbound.clone();
                                            move |value: String| {
                                                let mut next = (*inbound).clone();
                                                next.protocol = normalize_protocol_for_core(&next.core_type, &value);
                                                if value == "WIREGUARD" {
                                                    next.core_type = "XRAY".to_string();
                                                }
                                                next.protocol = normalize_protocol_for_core(&next.core_type, &next.protocol);
                                                inbound.set(next);
                                            }
                                        })}
                                    />
                                </ConfigSection>
                            },
                            1 => html! {
                                <>
                                    {
                                        match data.protocol.as_str() {
                                            "HYSTERIA2" => html! {
                                                <ConfigSection title="Hysteria2">
                                                    <Dropdown
                                                        label="Obfuscation Type"
                                                        value={data.hysteria2.obfs_type.clone()}
                                                        options={vec![
                                                            DropdownOption { value: "".to_string(), label: "None".to_string() },
                                                            DropdownOption { value: "salamander".to_string(), label: "Salamander".to_string() },
                                                        ]}
                                                        onchange={update_text(|inbound, value| inbound.hysteria2.obfs_type = value)}
                                                    />
                                                    {
                                                        if data.hysteria2.obfs_type.is_empty() {
                                                            html! {}
                                                        } else {
                                                            html! {
                                                                <TextBox
                                                                    label="Obfuscation Password"
                                                                    value={data.hysteria2.obfs_password.clone()}
                                                                    onchange={update_text(|inbound, value| inbound.hysteria2.obfs_password = value)}
                                                                />
                                                            }
                                                        }
                                                    }
                                                    <TextBox label="Up Mbps" value={data.hysteria2.up_mbps.to_string()} onchange={update_text(|inbound, value| inbound.hysteria2.up_mbps = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="Down Mbps" value={data.hysteria2.down_mbps.to_string()} onchange={update_text(|inbound, value| inbound.hysteria2.down_mbps = value.parse().unwrap_or(0))} input_type="number" />
                                                    <SwitchField
                                                        label="Ignore client bandwidth"
                                                        checked={data.hysteria2.ignore_client_bandwidth}
                                                        onchange={update_bool(|inbound, value| inbound.hysteria2.ignore_client_bandwidth = value)}
                                                    />
                                                    <TextBox label="Masquerade" value={data.hysteria2.masquerade.clone()} onchange={update_text(|inbound, value| inbound.hysteria2.masquerade = value)} placeholder="Empty, URL, or raw JSON object" />
                                                    <TextBox label="BBR Profile" value={data.hysteria2.bbr_profile.clone()} onchange={update_text(|inbound, value| inbound.hysteria2.bbr_profile = value)} placeholder="Optional" />
                                                    <SwitchField
                                                        label="Brutal debug"
                                                        checked={data.hysteria2.brutal_debug}
                                                        onchange={update_bool(|inbound, value| inbound.hysteria2.brutal_debug = value)}
                                                    />
                                                </ConfigSection>
                                            },
                                            "TRUSTTUNNEL" => html! {
                                                <ConfigSection title="TrustTunnel">
                                                    <TextBox label="[listen_protocols.http1] upload_buffer_size" value={data.trust_tunnel.http1_upload_buffer_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http1_upload_buffer_size = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] initial_connection_window_size" value={data.trust_tunnel.http2_initial_connection_window_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_initial_connection_window_size = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] initial_stream_window_size" value={data.trust_tunnel.http2_initial_stream_window_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_initial_stream_window_size = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] max_concurrent_streams" value={data.trust_tunnel.http2_max_concurrent_streams.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_max_concurrent_streams = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] max_frame_size" value={data.trust_tunnel.http2_max_frame_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_max_frame_size = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] header_table_size" value={data.trust_tunnel.http2_header_table_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_header_table_size = value.parse().unwrap_or(0))} input_type="number" />
                                                </ConfigSection>
                                            },
                                            "NAIVEPROXY" => html! {
                                                <ConfigSection title="NaiveProxy">
                                                    <TextBox label="Username" value={data.naive_proxy.username.clone()} onchange={update_text(|inbound, value| inbound.naive_proxy.username = value)} />
                                                    <TextBox label="Password" value={data.naive_proxy.password.clone()} onchange={update_text(|inbound, value| inbound.naive_proxy.password = value)} />
                                                    <TextBox label="Protocol" value={data.naive_proxy.protocol.clone()} onchange={update_text(|inbound, value| inbound.naive_proxy.protocol = value)} placeholder="h2 / h3" />
                                                    <TextBox label="Target" value={data.naive_proxy.target.clone()} onchange={update_text(|inbound, value| inbound.naive_proxy.target = value)} />
                                                </ConfigSection>
                                            },
                                            "WIREGUARD" => html! {
                                                <ConfigSection title="WireGuard">
                                                    <div class="text-sm mb-4" style="color: var(--md-sys-color-on-surface-variant);">
                                                        { "WireGuard inbound uses Xray. Each account token must be peer public key, and Allowed IPs become peer routes." }
                                                    </div>
                                                    <TextBox label="Private Key" value={data.wireguard.private_key.clone()} onchange={update_text(|inbound, value| inbound.wireguard.private_key = value)} />
                                                    <TextBox label="MTU" value={data.wireguard.mtu.to_string()} onchange={update_text(|inbound, value| inbound.wireguard.mtu = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="Addresses" value={data.wireguard.addresses.clone()} onchange={update_text(|inbound, value| inbound.wireguard.addresses = value)} is_textarea={true} placeholder="10.0.0.1/32, fd59:7153:2388:b5fd::1/128" />
                                                </ConfigSection>
                                            },
                                            "SOCKS5" => html! {
                                                <ConfigSection title="SOCKS5">
                                                    <TextBox label="Username" value={data.socks5.username.clone()} onchange={update_text(|inbound, value| inbound.socks5.username = value)} />
                                                    <TextBox label="Password" value={data.socks5.password.clone()} onchange={update_text(|inbound, value| inbound.socks5.password = value)} />
                                                    <SwitchField
                                                        label="UDP enabled"
                                                        checked={data.socks5.udp_enabled}
                                                        onchange={update_bool(|inbound, value| inbound.socks5.udp_enabled = value)}
                                                    />
                                                </ConfigSection>
                                            },
                                            "SHADOWSOCKS" => html! {
                                                <ConfigSection title="Shadowsocks">
                                                    <Dropdown
                                                        label="Method"
                                                        value={data.shadowsocks.method.clone()}
                                                        options={shadowsocks_method_options()}
                                                        onchange={update_text(|inbound, value| inbound.shadowsocks.method = value)}
                                                    />
                                                    <TextBox label="Default Password" value={data.shadowsocks.password.clone()} onchange={update_text(|inbound, value| inbound.shadowsocks.password = value)} placeholder="Fallback if account token is empty" />
                                                    <SwitchField
                                                        label="UDP enabled"
                                                        checked={data.shadowsocks.udp_enabled}
                                                        onchange={update_bool(|inbound, value| inbound.shadowsocks.udp_enabled = value)}
                                                    />
                                                </ConfigSection>
                                            },
                                            _ => html! {
                                                <ConfigSection title="VLESS">
                                                    <Dropdown
                                                        label="Flow"
                                                        value={data.vless.flow.clone()}
                                                        options={vec![
                                                            DropdownOption { value: "".to_string(), label: "None".to_string() },
                                                            DropdownOption { value: "xtls-rprx-vision".to_string(), label: "xtls-rprx-vision".to_string() },
                                                            DropdownOption { value: "xtls-rprx-vision-udp443".to_string(), label: "xtls-rprx-vision-udp443".to_string() },
                                                        ]}
                                                        onchange={update_text(|inbound, value| inbound.vless.flow = value)}
                                                    />
                                                    <Dropdown
                                                        label="Security"
                                                        value={data.vless.security.clone()}
                                                        options={vec![
                                                            DropdownOption { value: "NONE".to_string(), label: "None".to_string() },
                                                            DropdownOption { value: "TLS".to_string(), label: "TLS".to_string() },
                                                            DropdownOption { value: "REALITY".to_string(), label: "Reality".to_string() },
                                                        ]}
                                                        onchange={update_text(|inbound, value| inbound.vless.security = value)}
                                                    />
                                                    <Dropdown
                                                        label="Transmission"
                                                        value={vless_transmission_from(&data.vless.transmission)}
                                                        options={vec![
                                                            DropdownOption { value: "TCP".to_string(), label: "TCP (RAW)".to_string() },
                                                            DropdownOption { value: "HTTP".to_string(), label: "HTTP".to_string() },
                                                            DropdownOption { value: "gRPC".to_string(), label: "gRPC".to_string() },
                                                            DropdownOption { value: "WebSocket".to_string(), label: "WebSocket".to_string() },
                                                            DropdownOption { value: "mKCP".to_string(), label: "mKCP".to_string() },
                                                            DropdownOption { value: "HttpUpgrade".to_string(), label: "HttpUpgrade".to_string() },
                                                            DropdownOption { value: "SplitHTTP".to_string(), label: "SplitHTTP".to_string() },
                                                        ]}
                                                        onchange={update_text(|inbound, value| inbound.vless.transmission = value)}
                                                    />
                                                </ConfigSection>
                                            }
                                        }
                                    }
                                </>
                            },
                            2 if data.protocol == "VLESS" && data.vless.security == "TLS" => html! {
                                <ConfigSection title="TLS">
                                    <TextBox label="Server Name" value={data.tls.server_name.clone()} onchange={update_text(|inbound, value| inbound.tls.server_name = value)} />
                                    <Dropdown
                                        label="Certificate"
                                        value={data.tls.certificate_name.clone()}
                                        options={certificate_options.clone()}
                                        onchange={update_text(|inbound, value| inbound.tls.certificate_name = value)}
                                    />
                                </ConfigSection>
                            },
                            2 if data.protocol == "HYSTERIA2" => html! {
                                <ConfigSection title="TLS">
                                    <TextBox label="Server Name" value={data.tls.server_name.clone()} onchange={update_text(|inbound, value| inbound.tls.server_name = value)} />
                                    <Dropdown
                                        label="Certificate"
                                        value={data.tls.certificate_name.clone()}
                                        options={certificate_options.clone()}
                                        onchange={update_text(|inbound, value| inbound.tls.certificate_name = value)}
                                    />
                                </ConfigSection>
                            },
                            2 if data.protocol == "TRUSTTUNNEL" => html! {
                                <ConfigSection title="TLS">
                                    <TextBox label="Server Name" value={data.tls.server_name.clone()} onchange={update_text(|inbound, value| inbound.tls.server_name = value)} />
                                    <Dropdown
                                        label="Certificate"
                                        value={data.tls.certificate_name.clone()}
                                        options={certificate_options.clone()}
                                        onchange={update_text(|inbound, value| inbound.tls.certificate_name = value)}
                                    />
                                </ConfigSection>
                            },
                            2 if data.protocol == "VLESS" && data.vless.security == "REALITY" => html! {
                                <ConfigSection title="Reality">
                                    <TextBox label="Dest" value={data.vless.reality_dest.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_dest = value)} />
                                    <TextBox label="SNI" value={data.vless.reality_sni.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_sni = value)} />
                                    <Dropdown
                                        label="uTLS"
                                        value={data.vless.reality_utls.clone()}
                                        options={vec![
                                            DropdownOption { value: "chrome".to_string(), label: "chrome".to_string() },
                                            DropdownOption { value: "firefox".to_string(), label: "firefox".to_string() },
                                            DropdownOption { value: "safari".to_string(), label: "safari".to_string() },
                                            DropdownOption { value: "edge".to_string(), label: "edge".to_string() },
                                            DropdownOption { value: "ios".to_string(), label: "ios".to_string() },
                                            DropdownOption { value: "android".to_string(), label: "android".to_string() },
                                            DropdownOption { value: "randomized".to_string(), label: "randomized".to_string() },
                                        ]}
                                        onchange={update_text(|inbound, value| inbound.vless.reality_utls = value)}
                                    />
                                    <TextBox label="SpiderX" value={data.vless.reality_spider_x.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_spider_x = value)} placeholder="/" />
                                    <TextBox label="Private Key" value={data.vless.reality_private_key.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_private_key = value)} />
                                    <TextBox label="Public Key" value={data.vless.reality_public_key.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_public_key = value)} />
                                    <TextBox label="Short IDs" value={data.vless.reality_short_ids.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_short_ids = value)} placeholder="id1, id2" />
                                    <div class="flex" style="gap: 0.75rem;">
                                        <Button label="Generate Short IDs" button_type={ButtonType::Outlined} onclick={Callback::from({
                                            let inbound = inbound.clone();
                                            move |_| {
                                                let mut next = (*inbound).clone();
                                                let mut ids = split_lines_csv(&next.vless.reality_short_ids);
                                                ids.extend(generate_reality_short_ids_batch(6));
                                                next.vless.reality_short_ids = ids.join(",");
                                                inbound.set(next);
                                            }
                                        })} />
                                        <Button label="Generate Keys" button_type={ButtonType::Outlined} onclick={Callback::from({
                                            let inbound = inbound.clone();
                                            move |_| {
                                                let (private_key, public_key) = generate_reality_keypair();
                                                let mut next = (*inbound).clone();
                                                next.vless.reality_private_key = private_key;
                                                next.vless.reality_public_key = public_key;
                                                inbound.set(next);
                                            }
                                        })} />
                                    </div>
                                </ConfigSection>
                            },
                            _ => html! {
                                <>
                                    <ConfigSection title="Review">
                                        <div class="space-y-2 text-sm">
                                            <div>{ format!("Name: {}", data.name) }</div>
                                            <div>{ format!("Protocol: {}", data.protocol) }</div>
                                            <div>{ format!("Listen: {}:{}", data.listen, data.port) }</div>
                                            <div>{ format!("Core: {}", data.core_type) }</div>
                                            {
                                                if data.protocol == "VLESS" {
                                                    html! { <div>{ format!("Security: {}", data.vless.security) }</div> }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>
                                    </ConfigSection>
                                </>
                            }
                        }
                    }
                    </div>

                    <div class="md3-popup-actions" style="justify-content: space-between; width: 100%;">
                        <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                            let on_close = props.on_close.clone();
                            move |_| on_close.emit(())
                        })} />
                        <div class="flex" style="gap: 0.75rem;">
                            {
                                if *step > 0 {
                                    html! {
                                        <Button label="Back" button_type={ButtonType::Outlined} onclick={Callback::from({
                                            let step = step.clone();
                                            move |_| step.set(step.saturating_sub(1))
                                        })} />
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            {
                                if *step + 1 < total_steps {
                                    html! {
                                        <Button label="Next" button_type={ButtonType::Filled} onclick={Callback::from({
                                            let step = step.clone();
                                            move |_| step.set(*step + 1)
                                        })} disabled={
                                            *step == 0
                                                && (data.core_type.trim().is_empty()
                                                    || data.protocol.trim().is_empty())
                                                || (*step == 2
                                                    && ((data.protocol == "VLESS"
                                                        && data.vless.security == "TLS")
                                                        || data.protocol == "HYSTERIA2"
                                                        || data.protocol == "TRUSTTUNNEL")
                                                    && data.tls.certificate_name.trim().is_empty())
                                        } />
                                    }
                                } else {
                                    html! {
                                    <Button label={if props.is_new { "Create Inbound" } else { "Apply Changes" }} button_type={ButtonType::Filled} onclick={Callback::from({
                                        let on_save = props.on_save.clone();
                                        let inbound = inbound.clone();
                                        move |_| on_save.emit((*inbound).clone())
                                    })} />
                                }
                                }
                            }
                        </div>
                    </div>
                </div>
            </Popup>
        };
    }

    html! {
        <Popup
            title={popup_title}
            size={PopupSize::Lg}
            on_close={props.on_close.clone()}
        >
            <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                <ConfigSection title="General">
                    <TextBox label="Name" value={data.name.clone()} onchange={update_text(|inbound, value| inbound.name = value)} />
                    <TextBox label="Listen Address" value={data.listen.clone()} onchange={update_text(|inbound, value| inbound.listen = value)} />
                    <TextBox
                        label="Port"
                        value={data.port.to_string()}
                        onchange={update_text(|inbound, value| inbound.port = value.parse().unwrap_or(0))}
                        input_type="number"
                        action_icon={Some("icon-sync".to_string())}
                        action_label={Some("Randomize port".to_string())}
                        action_onclick={Some(Callback::from({
                            let inbound = inbound.clone();
                            move |_| {
                                let mut next = (*inbound).clone();
                                next.port = random_port();
                                inbound.set(next);
                            }
                        }))}
                    />
                    <Dropdown
                        label="Core"
                        value={data.core_type.clone()}
                        options={vec![
                            DropdownOption { value: "".to_string(), label: "None".to_string() },
                            DropdownOption { value: "XRAY".to_string(), label: "Xray".to_string() },
                            DropdownOption { value: "SING_BOX".to_string(), label: "Sing-Box".to_string() },
                            DropdownOption { value: "TRUSTTUNNEL".to_string(), label: "TrustTunnel".to_string() },
                        ]}
                        onchange={Callback::from({
                            let inbound = inbound.clone();
                            move |value: String| {
                                let mut next = (*inbound).clone();
                                next.core_type = value.clone();
                                next.protocol = normalize_protocol_for_core(&value, &next.protocol);
                                inbound.set(next);
                            }
                        })}
                    />
                    <Dropdown
                        label="Protocol"
                        value={data.protocol.clone()}
                        disabled={data.core_type == "TRUSTTUNNEL"}
                        options={protocol_options_for_core(&data.core_type)}
                        onchange={Callback::from({
                            let inbound = inbound.clone();
                            move |value: String| {
                                let mut next = (*inbound).clone();
                                next.protocol = normalize_protocol_for_core(&next.core_type, &value);
                                if value == "WIREGUARD" {
                                    next.core_type = "XRAY".to_string();
                                }
                                next.protocol = normalize_protocol_for_core(&next.core_type, &next.protocol);
                                inbound.set(next);
                            }
                        })}
                    />
                </ConfigSection>

                <ConfigSection title="TLS / Reality">
                    <TextBox label="Server Name" value={data.tls.server_name.clone()} onchange={update_text(|inbound, value| inbound.tls.server_name = value)} />
                    <Dropdown
                        label="Certificate"
                        value={data.tls.certificate_name.clone()}
                        options={certificate_options.clone()}
                        onchange={update_text(|inbound, value| inbound.tls.certificate_name = value)}
                    />
                    <TextBox label="Dest" value={data.vless.reality_dest.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_dest = value)} />
                    <TextBox label="SNI" value={data.vless.reality_sni.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_sni = value)} />
                    <Dropdown
                        label="uTLS"
                        value={data.vless.reality_utls.clone()}
                        options={vec![
                            DropdownOption { value: "chrome".to_string(), label: "chrome".to_string() },
                            DropdownOption { value: "firefox".to_string(), label: "firefox".to_string() },
                            DropdownOption { value: "safari".to_string(), label: "safari".to_string() },
                            DropdownOption { value: "edge".to_string(), label: "edge".to_string() },
                            DropdownOption { value: "ios".to_string(), label: "ios".to_string() },
                            DropdownOption { value: "android".to_string(), label: "android".to_string() },
                            DropdownOption { value: "randomized".to_string(), label: "randomized".to_string() },
                        ]}
                        onchange={update_text(|inbound, value| inbound.vless.reality_utls = value)}
                    />
                    <TextBox label="SpiderX" value={data.vless.reality_spider_x.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_spider_x = value)} placeholder="/" />
                    <TextBox label="Private Key" value={data.vless.reality_private_key.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_private_key = value)} />
                    <TextBox label="Public Key" value={data.vless.reality_public_key.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_public_key = value)} />
                    <TextBox label="Short IDs" value={data.vless.reality_short_ids.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_short_ids = value)} placeholder="id1, id2" />
                    <div class="flex" style="gap: 0.75rem;">
                        <Button label="Generate Short IDs" button_type={ButtonType::Outlined} onclick={Callback::from({
                            let inbound = inbound.clone();
                            move |_| {
                                let mut next = (*inbound).clone();
                                let mut ids = split_lines_csv(&next.vless.reality_short_ids);
                                ids.extend(generate_reality_short_ids_batch(6));
                                next.vless.reality_short_ids = ids.join(",");
                                inbound.set(next);
                            }
                        })} />
                        <Button label="Generate Keys" button_type={ButtonType::Outlined} onclick={Callback::from({
                            let inbound = inbound.clone();
                            move |_| {
                                let (private_key, public_key) = generate_reality_keypair();
                                let mut next = (*inbound).clone();
                                next.vless.reality_private_key = private_key;
                                next.vless.reality_public_key = public_key;
                                inbound.set(next);
                            }
                        })} />
                    </div>
                </ConfigSection>

                {
                    match data.protocol.as_str() {
                        "HYSTERIA2" => html! {
                            <ConfigSection title="Hysteria2">
                                <Dropdown
                                    label="Obfuscation Type"
                                    value={data.hysteria2.obfs_type.clone()}
                                    options={vec![
                                        DropdownOption { value: "".to_string(), label: "None".to_string() },
                                        DropdownOption { value: "salamander".to_string(), label: "Salamander".to_string() },
                                    ]}
                                    onchange={update_text(|inbound, value| inbound.hysteria2.obfs_type = value)}
                                />
                                {
                                    if data.hysteria2.obfs_type.is_empty() {
                                        html! {}
                                    } else {
                                        html! {
                                            <TextBox
                                                label="Obfuscation Password"
                                                value={data.hysteria2.obfs_password.clone()}
                                                onchange={update_text(|inbound, value| inbound.hysteria2.obfs_password = value)}
                                            />
                                        }
                                    }
                                }
                                <TextBox label="Up Mbps" value={data.hysteria2.up_mbps.to_string()} onchange={update_text(|inbound, value| inbound.hysteria2.up_mbps = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="Down Mbps" value={data.hysteria2.down_mbps.to_string()} onchange={update_text(|inbound, value| inbound.hysteria2.down_mbps = value.parse().unwrap_or(0))} input_type="number" />
                                <SwitchField
                                    label="Ignore client bandwidth"
                                    checked={data.hysteria2.ignore_client_bandwidth}
                                    onchange={update_bool(|inbound, value| inbound.hysteria2.ignore_client_bandwidth = value)}
                                />
                                <TextBox label="Masquerade" value={data.hysteria2.masquerade.clone()} onchange={update_text(|inbound, value| inbound.hysteria2.masquerade = value)} placeholder="Empty, URL, or raw JSON object" />
                                <TextBox label="BBR Profile" value={data.hysteria2.bbr_profile.clone()} onchange={update_text(|inbound, value| inbound.hysteria2.bbr_profile = value)} placeholder="Optional" />
                                <SwitchField
                                    label="Brutal debug"
                                    checked={data.hysteria2.brutal_debug}
                                    onchange={update_bool(|inbound, value| inbound.hysteria2.brutal_debug = value)}
                                />
                            </ConfigSection>
                        },
                        "TRUSTTUNNEL" => html! {
                            <ConfigSection title="TrustTunnel">
                                <TextBox label="[listen_protocols.http1] upload_buffer_size" value={data.trust_tunnel.http1_upload_buffer_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http1_upload_buffer_size = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] initial_connection_window_size" value={data.trust_tunnel.http2_initial_connection_window_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_initial_connection_window_size = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] initial_stream_window_size" value={data.trust_tunnel.http2_initial_stream_window_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_initial_stream_window_size = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] max_concurrent_streams" value={data.trust_tunnel.http2_max_concurrent_streams.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_max_concurrent_streams = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] max_frame_size" value={data.trust_tunnel.http2_max_frame_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_max_frame_size = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] header_table_size" value={data.trust_tunnel.http2_header_table_size.to_string()} onchange={update_text(|inbound, value| inbound.trust_tunnel.http2_header_table_size = value.parse().unwrap_or(0))} input_type="number" />
                            </ConfigSection>
                        },
                        "NAIVEPROXY" => html! {
                            <ConfigSection title="NaiveProxy">
                                <TextBox label="Username" value={data.naive_proxy.username.clone()} onchange={update_text(|inbound, value| inbound.naive_proxy.username = value)} />
                                <TextBox label="Password" value={data.naive_proxy.password.clone()} onchange={update_text(|inbound, value| inbound.naive_proxy.password = value)} />
                                <TextBox label="Protocol" value={data.naive_proxy.protocol.clone()} onchange={update_text(|inbound, value| inbound.naive_proxy.protocol = value)} placeholder="h2 / h3" />
                                <TextBox label="Target" value={data.naive_proxy.target.clone()} onchange={update_text(|inbound, value| inbound.naive_proxy.target = value)} />
                            </ConfigSection>
                        },
                        "WIREGUARD" => html! {
                            <ConfigSection title="WireGuard">
                                <div class="text-sm mb-4" style="color: var(--md-sys-color-on-surface-variant);">
                                    { "WireGuard inbound uses Xray. Each account token must be peer public key, and Allowed IPs become peer routes." }
                                </div>
                                <TextBox label="Private Key" value={data.wireguard.private_key.clone()} onchange={update_text(|inbound, value| inbound.wireguard.private_key = value)} />
                                <TextBox label="MTU" value={data.wireguard.mtu.to_string()} onchange={update_text(|inbound, value| inbound.wireguard.mtu = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="Addresses" value={data.wireguard.addresses.clone()} onchange={update_text(|inbound, value| inbound.wireguard.addresses = value)} is_textarea={true} placeholder="10.0.0.1/32, fd59:7153:2388:b5fd::1/128" />
                            </ConfigSection>
                        },
                        "SOCKS5" => html! {
                            <ConfigSection title="SOCKS5">
                                <TextBox label="Username" value={data.socks5.username.clone()} onchange={update_text(|inbound, value| inbound.socks5.username = value)} />
                                <TextBox label="Password" value={data.socks5.password.clone()} onchange={update_text(|inbound, value| inbound.socks5.password = value)} />
                                <SwitchField
                                    label="UDP enabled"
                                    checked={data.socks5.udp_enabled}
                                    onchange={update_bool(|inbound, value| inbound.socks5.udp_enabled = value)}
                                />
                            </ConfigSection>
                        },
                        "SHADOWSOCKS" => html! {
                            <ConfigSection title="Shadowsocks">
                                <Dropdown
                                    label="Method"
                                    value={data.shadowsocks.method.clone()}
                                    options={shadowsocks_method_options()}
                                    onchange={update_text(|inbound, value| inbound.shadowsocks.method = value)}
                                />
                                <TextBox label="Default Password" value={data.shadowsocks.password.clone()} onchange={update_text(|inbound, value| inbound.shadowsocks.password = value)} placeholder="Fallback if account token is empty" />
                                <SwitchField
                                    label="UDP enabled"
                                    checked={data.shadowsocks.udp_enabled}
                                    onchange={update_bool(|inbound, value| inbound.shadowsocks.udp_enabled = value)}
                                />
                            </ConfigSection>
                        },
                        _ => html! {
                            <ConfigSection title="VLESS">
                                <Dropdown
                                    label="Flow"
                                    value={data.vless.flow.clone()}
                                    options={vec![
                                        DropdownOption { value: "".to_string(), label: "None".to_string() },
                                        DropdownOption { value: "xtls-rprx-vision".to_string(), label: "xtls-rprx-vision".to_string() },
                                        DropdownOption { value: "xtls-rprx-vision-udp443".to_string(), label: "xtls-rprx-vision-udp443".to_string() },
                                    ]}
                                    onchange={update_text(|inbound, value| inbound.vless.flow = value)}
                                />
                                <Dropdown
                                    label="Security"
                                    value={data.vless.security.clone()}
                                    options={vec![
                                        DropdownOption { value: "NONE".to_string(), label: "None".to_string() },
                                        DropdownOption { value: "TLS".to_string(), label: "TLS".to_string() },
                                        DropdownOption { value: "REALITY".to_string(), label: "Reality".to_string() },
                                    ]}
                                    onchange={update_text(|inbound, value| inbound.vless.security = value)}
                                />
                                <Dropdown
                                    label="Transmission"
                                    value={vless_transmission_from(&data.vless.transmission)}
                                    options={vec![
                                        DropdownOption { value: "TCP".to_string(), label: "TCP (RAW)".to_string() },
                                        DropdownOption { value: "HTTP".to_string(), label: "HTTP".to_string() },
                                        DropdownOption { value: "gRPC".to_string(), label: "gRPC".to_string() },
                                        DropdownOption { value: "WebSocket".to_string(), label: "WebSocket".to_string() },
                                        DropdownOption { value: "mKCP".to_string(), label: "mKCP".to_string() },
                                        DropdownOption { value: "HttpUpgrade".to_string(), label: "HttpUpgrade".to_string() },
                                        DropdownOption { value: "SplitHTTP".to_string(), label: "SplitHTTP".to_string() },
                                    ]}
                                    onchange={update_text(|inbound, value| inbound.vless.transmission = value)}
                                />
                            </ConfigSection>
                        },
                    }
                }
            </div>

            <div class="md3-popup-actions" style="justify-content: flex-end;">
                <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                    let on_close = props.on_close.clone();
                    move |_| on_close.emit(())
                })} />
                <Button label={if props.is_new { "Create Inbound" } else { "Apply Changes" }} button_type={ButtonType::Filled} onclick={Callback::from({
                    let on_save = props.on_save.clone();
                    let inbound = inbound.clone();
                    move |_| on_save.emit((*inbound).clone())
                })} />
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct OutboundEditorPopupProps {
    outbound: OutboundEntryDraft,
    is_new: bool,
    node_address: String,
    master_key: String,
    on_close: Callback<()>,
    on_save: Callback<OutboundEntryDraft>,
}

#[derive(Properties, PartialEq)]
struct WarpCreatePopupProps {
    node_address: String,
    master_key: String,
    initial_registration: Option<crate::services::warp::WarpRegistration>,
    on_registration_change: Callback<Option<crate::services::warp::WarpRegistration>>,
    on_close: Callback<()>,
    on_create: Callback<OutboundEntryDraft>,
}

#[function_component(WarpCreatePopup)]
fn warp_create_popup(props: &WarpCreatePopupProps) -> Html {
    let registration = use_state(|| props.initial_registration.clone());
    let status = use_state(|| Option::<String>::None);
    let loading = use_state(|| false);
    let warp_keypair = use_state(|| {
        if let Some(existing) = &props.initial_registration {
            (existing.private_key.clone(), existing.public_key.clone())
        } else {
            generate_wireguard_keypair().unwrap_or((String::new(), String::new()))
        }
    });

    let registration_value = (*registration).clone();
    {
        let registration_value = registration_value.clone();
        let on_registration_change = props.on_registration_change.clone();
        use_effect_with(registration_value, move |value| {
            on_registration_change.emit(value.clone());
            || ()
        });
    }

    html! {
        <Popup title="Create WARP Outbound" size={PopupSize::Lg} on_close={props.on_close.clone()}>
            <div class="space-y-6">
                <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                    { "Register a fresh WARP account on node, then create a WireGuard outbound from returned credentials." }
                </div>
                <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                    <TextBox
                        label="Private Key"
                        value={(*warp_keypair).0.clone()}
                        onchange={Callback::from({
                            let warp_keypair = warp_keypair.clone();
                            move |value: String| {
                                let (_, public_key) = (*warp_keypair).clone();
                                warp_keypair.set((value, public_key));
                            }
                        })}
                    />
                    <TextBox
                        label="Public Key"
                        value={(*warp_keypair).1.clone()}
                        onchange={Callback::from({
                            let warp_keypair = warp_keypair.clone();
                            move |value: String| {
                                let (private_key, _) = (*warp_keypair).clone();
                                warp_keypair.set((private_key, value));
                            }
                        })}
                    />
                    <TextBox
                        label="Account ID"
                        value={registration_value.as_ref().map(|data| data.id.clone()).unwrap_or_default()}
                        onchange={Callback::from(|_: String| {})}
                    />
                    <TextBox
                        label="Access Token"
                        value={registration_value.as_ref().map(|data| data.token.clone()).unwrap_or_default()}
                        onchange={Callback::from(|_: String| {})}
                    />
                </div>
                {
                    if let Some(message) = &*status {
                        html! {
                            <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                                { message.clone() }
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    if let Some(data) = &registration_value {
                        html! {
                            <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                <TextBox
                                    label="License"
                                    value={data.license.clone()}
                                    onchange={Callback::from({
                                        let registration = registration.clone();
                                        move |value: String| {
                                            if let Some(mut next) = (*registration).clone() {
                                                next.license = value;
                                                registration.set(Some(next));
                                            }
                                        }
                                    })}
                                />
                                <TextBox label="Reserved Bytes" value={data.reserved.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(", ")} onchange={Callback::from(|_: String| {})} />
                                <TextBox label="Endpoint" value={data.endpoint.clone()} onchange={Callback::from(|_: String| {})} />
                                <TextBox label="Addresses" value={data.addresses.join(", ")} onchange={Callback::from(|_: String| {})} is_textarea={true} />
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                <div class="md3-popup-actions" style="justify-content: space-between; width: 100%;">
                    <Button
                        label="Clear"
                        button_type={ButtonType::Text}
                        color={Some("#F2B8B5".to_string())}
                        disabled={*loading}
                        onclick={Callback::from({
                            let registration = registration.clone();
                            let warp_keypair = warp_keypair.clone();
                            let status = status.clone();
                            let loading = loading.clone();
                            move |_| {
                                registration.set(None);
                                warp_keypair
                                    .set(generate_wireguard_keypair().unwrap_or((String::new(), String::new())));
                                status.set(None);
                                loading.set(false);
                            }
                        })}
                    />
                    <div class="flex" style="gap: 0.75rem;">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    <Button
                        label={if *loading { "Registering..." } else { "Register Account" }}
                        button_type={ButtonType::Outlined}
                        disabled={*loading}
                        onclick={Callback::from({
                            let registration = registration.clone();
                            let status = status.clone();
                            let loading = loading.clone();
                            let node_address = props.node_address.clone();
                            let master_key = props.master_key.clone();
                            let warp_keypair = warp_keypair.clone();
                            move |_| {
                                loading.set(true);
                                status.set(Some("Registering WARP account on node...".to_string()));
                                let registration = registration.clone();
                                let status = status.clone();
                                let loading = loading.clone();
                                let node_address = node_address.clone();
                                let master_key = master_key.clone();
                                let (private_key_value, public_key_value) = (*warp_keypair).clone();
                                spawn_local(async move {
                                    match register_warp_with_keypair(
                                        node_address,
                                        master_key,
                                        private_key_value,
                                        public_key_value,
                                    )
                                    .await
                                    {
                                        Ok(data) => {
                                            registration.set(Some(data));
                                            status.set(Some("WARP account ready. Review credentials, then create outbound.".to_string()));
                                        }
                                        Err(error) => status.set(Some(format!("WARP registration failed: {}", error))),
                                    }
                                    loading.set(false);
                                });
                            }
                        })}
                    />
                    <Button
                        label={if *loading { "Updating..." } else { "Update License" }}
                        button_type={ButtonType::Outlined}
                        disabled={registration_value.is_none() || *loading}
                        onclick={Callback::from({
                            let registration = registration.clone();
                            let status = status.clone();
                            let loading = loading.clone();
                            let node_address = props.node_address.clone();
                            let master_key = props.master_key.clone();
                            move |_| {
                                let current = (*registration).clone();
                                if let Some(data) = current {
                                    loading.set(true);
                                    status.set(Some("Updating WARP license on node...".to_string()));
                                    let registration = registration.clone();
                                    let status = status.clone();
                                    let loading = loading.clone();
                                    let node_address = node_address.clone();
                                    let master_key = master_key.clone();
                                    spawn_local(async move {
                                        match update_warp_license(
                                            node_address,
                                            master_key,
                                            data.id.clone(),
                                            data.token.clone(),
                                            data.license.clone(),
                                        )
                                        .await
                                        {
                                            Ok(updated_license) => {
                                                let mut next = data;
                                                next.license = updated_license;
                                                registration.set(Some(next));
                                                status.set(Some("WARP license updated.".to_string()));
                                            }
                                            Err(error) => status.set(Some(format!("WARP license update failed: {}", error))),
                                        }
                                        loading.set(false);
                                    });
                                }
                            }
                        })}
                    />
                    <Button
                        label="Create Outbound"
                        button_type={ButtonType::Filled}
                        disabled={registration_value.is_none() || *loading}
                        onclick={Callback::from({
                            let on_create = props.on_create.clone();
                            let registration = registration.clone();
                            move |_| {
                                if let Some(data) = (*registration).clone() {
                                    let mut outbound = default_warp_outbound();
                                    outbound.wireguard.private_key = data.private_key;
                                    outbound.wireguard.warp_id = data.id;
                                    outbound.wireguard.warp_token = data.token;
                                    outbound.wireguard.reserved = data.reserved.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(", ");
                                    outbound.wireguard.addresses = data.addresses.join(", ");
                                    outbound.wireguard.peers = vec![WireGuardPeerItem {
                                        public_key: data.peer_public_key,
                                        endpoint: data.endpoint,
                                        allowed_ips: "0.0.0.0/0, ::/0".to_string(),
                                    }];
                                    on_create.emit(outbound);
                                }
                            }
                        })}
                    />
                    </div>
                </div>
            </div>
        </Popup>
    }
}

#[function_component(OutboundEditorPopup)]
fn outbound_editor_popup(props: &OutboundEditorPopupProps) -> Html {
    let outbound = use_state(|| props.outbound.clone());
    let step = use_state(|| 0usize);

    let update_text = |mutator: fn(&mut OutboundEntryDraft, String)| {
        let outbound = outbound.clone();
        Callback::from(move |value: String| {
            let mut next = (*outbound).clone();
            mutator(&mut next, value);
            outbound.set(next);
        })
    };

    let data = (*outbound).clone();
    let popup_title: AttrValue = if props.is_new {
        "Add Outbound"
    } else {
        "Edit Outbound"
    }
    .into();
    let total_steps = outbound_creation_steps(&data);

    if true {
        return html! {
            <Popup title={popup_title} size={PopupSize::Md} on_close={props.on_close.clone()}>
                <div class="space-y-6">
                    <div class="text-sm opacity-70">{ format!("Step {} of {}", *step + 1, total_steps) }</div>

                    <div key={format!("outbound-step-{}", *step)} class="md3-wizard-page">
                    {
                        match *step {
                            0 => html! {
                                <div class="space-y-4">
                                    <TextBox label="Name" value={data.name.clone()} onchange={update_text(|outbound, value| outbound.name = value)} />
                                    <Dropdown
                                        label="Type"
                                        value={data.outbound_type.clone()}
                                        options={vec![
                                            DropdownOption { value: "VLESS".to_string(), label: "VLESS".to_string() },
                                            DropdownOption { value: "TRUSTTUNNEL".to_string(), label: "TrustTunnel".to_string() },
                                            DropdownOption { value: "WIREGUARD".to_string(), label: "WireGuard".to_string() },
                                            DropdownOption { value: "SOCKS5".to_string(), label: "SOCKS5".to_string() },
                                            DropdownOption { value: "SHADOWSOCKS".to_string(), label: "Shadowsocks".to_string() },
                                        ]}
                                        onchange={Callback::from({
                                            let outbound = outbound.clone();
                                            move |value: String| {
                                                let mut next = (*outbound).clone();
                                                next.outbound_type = value.clone();
                                                if next.name.trim().is_empty() {
                                                    next.name = value.clone();
                                                }
                                                outbound.set(next);
                                            }
                                        })}
                                    />
                                </div>
                            },
                            1 => html! {
                                <>
                                    {
                                        match data.outbound_type.trim().to_uppercase().as_str() {
                                            "TRUSTTUNNEL" => html! {
                                                <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                    <TextBox label="Tag" value={data.trust_tunnel.tag.clone()} onchange={update_text(|outbound, value| outbound.trust_tunnel.tag = value)} />
                                                    <TextBox label="[listen_protocols.http1] upload_buffer_size" value={data.trust_tunnel.http1_upload_buffer_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http1_upload_buffer_size = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] initial_connection_window_size" value={data.trust_tunnel.http2_initial_connection_window_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_initial_connection_window_size = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] initial_stream_window_size" value={data.trust_tunnel.http2_initial_stream_window_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_initial_stream_window_size = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] max_concurrent_streams" value={data.trust_tunnel.http2_max_concurrent_streams.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_max_concurrent_streams = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] max_frame_size" value={data.trust_tunnel.http2_max_frame_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_max_frame_size = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="[listen_protocols.http2] header_table_size" value={data.trust_tunnel.http2_header_table_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_header_table_size = value.parse().unwrap_or(0))} input_type="number" />
                                                </div>
                                            },
                                            "WIREGUARD" => html! {
                                                <div class="space-y-4">
                                                    <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                        <Dropdown
                                                            label="Domain Strategy"
                                                            value={data.wireguard.domain_strategy.clone()}
                                                            options={vec![
                                                                DropdownOption { value: "ForceIP".to_string(), label: "ForceIP".to_string() },
                                                                DropdownOption { value: "ForceIPv4".to_string(), label: "ForceIPv4".to_string() },
                                                                DropdownOption { value: "ForceIPv4v6".to_string(), label: "ForceIPv4v6".to_string() },
                                                                DropdownOption { value: "ForceIPv6".to_string(), label: "ForceIPv6".to_string() },
                                                                DropdownOption { value: "ForceIPv6v4".to_string(), label: "ForceIPv6v4".to_string() },
                                                            ]}
                                                            onchange={update_text(|outbound, value| outbound.wireguard.domain_strategy = value)}
                                                        />
                                                        <TextBox label="MTU" value={data.wireguard.mtu.to_string()} onchange={update_text(|outbound, value| outbound.wireguard.mtu = value.parse().unwrap_or(0))} input_type="number" />
                                                        <TextBox label="Workers" value={data.wireguard.workers.to_string()} onchange={update_text(|outbound, value| outbound.wireguard.workers = value.parse().unwrap_or(0))} input_type="number" />
                                                        <TextBox label="Private Key" value={data.wireguard.private_key.clone()} onchange={update_text(|outbound, value| outbound.wireguard.private_key = value)} />
                                                        <TextBox label="Reserved Bytes" value={data.wireguard.reserved.clone()} onchange={update_text(|outbound, value| outbound.wireguard.reserved = value)} placeholder="1, 2, 3" />
                                                        <TextBox label="Addresses" value={data.wireguard.addresses.clone()} onchange={update_text(|outbound, value| outbound.wireguard.addresses = value)} placeholder="172.16.0.2/32, 2606:4700:110:.../128" />
                                                    </div>
                                                    <div class="space-y-4">
                                                        <div class="flex justify-between" style="align-items: center;">
                                                            <div class="text-sm font-semibold">{ "Peers" }</div>
                                                            <Button
                                                                label="Add Peer"
                                                                button_type={ButtonType::Outlined}
                                                                onclick={Callback::from({
                                                                    let outbound = outbound.clone();
                                                                    move |_| {
                                                                        let mut next = (*outbound).clone();
                                                                        next.wireguard.peers.push(WireGuardPeerItem::default());
                                                                        outbound.set(next);
                                                                    }
                                                                })}
                                                            />
                                                        </div>
                                                        {
                                                            for data.wireguard.peers.iter().enumerate().map(|(idx, peer)| {
                                                                html! {
                                                                    <div key={format!("wg-peer-step-{}-{}", *step, idx)} class="md3-card bg-surface-container space-y-3">
                                                                        <div class="flex justify-between" style="align-items: flex-start;">
                                                                            <div class="text-sm font-semibold opacity-80">{ format!("Peer {}", idx + 1) }</div>
                                                                            <IconButton
                                                                                label="Delete Peer"
                                                                                button_type={ButtonType::Text}
                                                                                color={Some("#F2B8B5".to_string())}
                                                                                onclick={Callback::from({
                                                                                    let outbound = outbound.clone();
                                                                                    move |_| {
                                                                                        let mut next = (*outbound).clone();
                                                                                        if idx < next.wireguard.peers.len() {
                                                                                            next.wireguard.peers.remove(idx);
                                                                                        }
                                                                                        outbound.set(next);
                                                                                    }
                                                                                })}
                                                                            >
                                                                                <SvgIcon name="delete_24dp" size={20} />
                                                                            </IconButton>
                                                                        </div>
                                                                        <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                                            <TextBox label="Public Key" value={peer.public_key.clone()} onchange={Callback::from({
                                                                                let outbound = outbound.clone();
                                                                                move |value: String| {
                                                                                    let mut next = (*outbound).clone();
                                                                                    if let Some(item) = next.wireguard.peers.get_mut(idx) {
                                                                                        item.public_key = value;
                                                                                    }
                                                                                    outbound.set(next);
                                                                                }
                                                                            })} />
                                                                            <TextBox label="Endpoint" value={peer.endpoint.clone()} onchange={Callback::from({
                                                                                let outbound = outbound.clone();
                                                                                move |value: String| {
                                                                                    let mut next = (*outbound).clone();
                                                                                    if let Some(item) = next.wireguard.peers.get_mut(idx) {
                                                                                        item.endpoint = value;
                                                                                    }
                                                                                    outbound.set(next);
                                                                                }
                                                                            })} />
                                                                        </div>
                                                                        <TextBox label="Allowed IPs" value={peer.allowed_ips.clone()} onchange={Callback::from({
                                                                            let outbound = outbound.clone();
                                                                            move |value: String| {
                                                                                let mut next = (*outbound).clone();
                                                                                if let Some(item) = next.wireguard.peers.get_mut(idx) {
                                                                                    item.allowed_ips = value;
                                                                                }
                                                                                outbound.set(next);
                                                                            }
                                                                        })} is_textarea={true} placeholder="0.0.0.0/0, ::/0" />
                                                                    </div>
                                                                }
                                                            })
                                                        }
                                                    </div>
                                                </div>
                                            },
                                            "SOCKS5" => html! {
                                                <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                    <TextBox label="Tag" value={data.socks5.tag.clone()} onchange={update_text(|outbound, value| outbound.socks5.tag = value)} />
                                                    <TextBox label="Server" value={data.socks5.server.clone()} onchange={update_text(|outbound, value| outbound.socks5.server = value)} />
                                                    <TextBox label="Port" value={data.socks5.port.to_string()} onchange={update_text(|outbound, value| outbound.socks5.port = value.parse().unwrap_or(0))} input_type="number" />
                                                    <TextBox label="Username" value={data.socks5.username.clone()} onchange={update_text(|outbound, value| outbound.socks5.username = value)} />
                                                    <TextBox label="Password" value={data.socks5.password.clone()} onchange={update_text(|outbound, value| outbound.socks5.password = value)} />
                                                </div>
                                            },
                                            "SHADOWSOCKS" => html! {
                                                <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                    <TextBox label="Tag" value={data.shadowsocks.tag.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.tag = value)} />
                                                    <TextBox label="Server" value={data.shadowsocks.server.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.server = value)} />
                                                    <TextBox label="Port" value={data.shadowsocks.port.to_string()} onchange={update_text(|outbound, value| outbound.shadowsocks.port = value.parse().unwrap_or(0))} input_type="number" />
                                                    <Dropdown
                                                        label="Method"
                                                        value={data.shadowsocks.method.clone()}
                                                        options={shadowsocks_method_options()}
                                                        onchange={update_text(|outbound, value| outbound.shadowsocks.method = value)}
                                                    />
                                                    <TextBox label="Password" value={data.shadowsocks.password.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.password = value)} />
                                                    <TextBox label="Plugin" value={data.shadowsocks.plugin.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.plugin = value)} placeholder="Optional" />
                                                    <TextBox label="Plugin Opts" value={data.shadowsocks.plugin_opts.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.plugin_opts = value)} placeholder="Optional" />
                                                    <TextBox label="Prefix (anti-DPI)" value={data.shadowsocks.prefix.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.prefix = value)} placeholder="Appended into plugin opts as prefix=..." />
                                                </div>
                                            },
                                            _ => html! {
                                                <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                    <TextBox label="Tag" value={data.vless.tag.clone()} onchange={update_text(|outbound, value| outbound.vless.tag = value)} />
                                                    <Dropdown
                                                        label="Security"
                                                        value={data.vless.security.clone()}
                                                        options={vec![
                                                            DropdownOption { value: "NONE".to_string(), label: "None".to_string() },
                                                            DropdownOption { value: "TLS".to_string(), label: "TLS".to_string() },
                                                            DropdownOption { value: "REALITY".to_string(), label: "Reality".to_string() },
                                                        ]}
                                                        onchange={update_text(|outbound, value| outbound.vless.security = value)}
                                                    />
                                                    <Dropdown
                                                        label="Transmission"
                                                        value={vless_transmission_from(&data.vless.transmission)}
                                                        options={vec![
                                                            DropdownOption { value: "TCP".to_string(), label: "TCP (RAW)".to_string() },
                                                            DropdownOption { value: "HTTP".to_string(), label: "HTTP".to_string() },
                                                            DropdownOption { value: "gRPC".to_string(), label: "gRPC".to_string() },
                                                            DropdownOption { value: "WebSocket".to_string(), label: "WebSocket".to_string() },
                                                            DropdownOption { value: "mKCP".to_string(), label: "mKCP".to_string() },
                                                            DropdownOption { value: "HttpUpgrade".to_string(), label: "HttpUpgrade".to_string() },
                                                            DropdownOption { value: "SplitHTTP".to_string(), label: "SplitHTTP".to_string() },
                                                        ]}
                                                        onchange={update_text(|outbound, value| outbound.vless.transmission = value)}
                                                    />
                                                </div>
                                            }
                                        }
                                    }
                                </>
                            },
                            _ => html! {
                                <>
                                    {
                                        match data.outbound_type.trim().to_uppercase().as_str() {
                                            "VLESS" => html! {
                                                <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                    <TextBox label="Server" value={data.vless.server.clone()} onchange={update_text(|outbound, value| outbound.vless.server = value)} />
                                                    <TextBox label="Port" value={data.vless.port.to_string()} onchange={update_text(|outbound, value| outbound.vless.port = value.parse().unwrap_or(0))} input_type="number" />
                                                    <Dropdown
                                                        label="Flow"
                                                        value={data.vless.flow.clone()}
                                                        options={vec![
                                                            DropdownOption { value: "".to_string(), label: "None".to_string() },
                                                            DropdownOption { value: "xtls-rprx-vision".to_string(), label: "xtls-rprx-vision".to_string() },
                                                            DropdownOption { value: "xtls-rprx-vision-udp443".to_string(), label: "xtls-rprx-vision-udp443".to_string() },
                                                        ]}
                                                        onchange={update_text(|outbound, value| outbound.vless.flow = value)}
                                                    />
                                                    <Dropdown
                                                        label="Transmission"
                                                        value={data.vless.transmission.clone()}
                                                        options={vec![
                                                            DropdownOption { value: "TCP".to_string(), label: "TCP (RAW)".to_string() },
                                                            DropdownOption { value: "HTTP".to_string(), label: "HTTP".to_string() },
                                                            DropdownOption { value: "gRPC".to_string(), label: "gRPC".to_string() },
                                                            DropdownOption { value: "WebSocket".to_string(), label: "WebSocket".to_string() },
                                                            DropdownOption { value: "mKCP".to_string(), label: "mKCP".to_string() },
                                                            DropdownOption { value: "HttpUpgrade".to_string(), label: "HttpUpgrade".to_string() },
                                                            DropdownOption { value: "SplitHTTP".to_string(), label: "SplitHTTP".to_string() },
                                                        ]}
                                                        onchange={update_text(|outbound, value| outbound.vless.transmission = value)}
                                                    />
                                                </div>
                                            },
                                            _ => html! {
                                                <ConfigSection title="Review">
                                                    <div class="space-y-2 text-sm">
                                                        <div>{ format!("Name: {}", data.name) }</div>
                                                        <div>{ format!("Type: {}", data.outbound_type) }</div>
                                                    </div>
                                                </ConfigSection>
                                            }
                                        }
                                    }
                                </>
                            }
                        }
                    }
                    </div>

                    <div class="md3-popup-actions" style="justify-content: space-between; width: 100%;">
                        <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                            let on_close = props.on_close.clone();
                            move |_| on_close.emit(())
                        })} />
                        <div class="flex" style="gap: 0.75rem;">
                            {
                                if *step > 0 {
                                    html! {
                                        <Button label="Back" button_type={ButtonType::Outlined} onclick={Callback::from({
                                            let step = step.clone();
                                            move |_| step.set(step.saturating_sub(1))
                                        })} />
                                    }
                                } else { html! {} }
                            }
                            {
                                if *step + 1 < total_steps {
                                    html! {
                                        <Button label="Next" button_type={ButtonType::Filled} onclick={Callback::from({
                                            let step = step.clone();
                                            move |_| step.set(*step + 1)
                                        })} disabled={
                                            *step == 0
                                                && (data.name.trim().is_empty()
                                                    || data.outbound_type.trim().is_empty())
                                                || (*step == 1
                                                    && data
                                                        .outbound_type
                                                        .trim()
                                                        .eq_ignore_ascii_case("WIREGUARD")
                                                    && data.wireguard.peers.is_empty())
                                        } />
                                    }
                                } else {
                                    html! {
                                        <Button
                                            label={if props.is_new { "Create Outbound" } else { "Apply Changes" }}
                                            button_type={ButtonType::Filled}
                                            onclick={Callback::from({
                                            let on_save = props.on_save.clone();
                                            let outbound = outbound.clone();
                                            move |_| on_save.emit((*outbound).clone())
                                        })} />
                                    }
                                }
                            }
                        </div>
                    </div>
                </div>
            </Popup>
        };
    }

    html! {
        <Popup title={popup_title} size={PopupSize::Lg} on_close={props.on_close.clone()}>
            <div class="space-y-6">
                <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                    <TextBox label="Name" value={data.name.clone()} onchange={update_text(|outbound, value| outbound.name = value)} />
                    <Dropdown
                        label="Type"
                        value={data.outbound_type.clone()}
                        options={vec![
                            DropdownOption { value: "VLESS".to_string(), label: "VLESS".to_string() },
                            DropdownOption { value: "TRUSTTUNNEL".to_string(), label: "TrustTunnel".to_string() },
                            DropdownOption { value: "WIREGUARD".to_string(), label: "WireGuard".to_string() },
                            DropdownOption { value: "SOCKS5".to_string(), label: "SOCKS5".to_string() },
                            DropdownOption { value: "SHADOWSOCKS".to_string(), label: "Shadowsocks".to_string() },
                            DropdownOption { value: "SHADOWSOCKS".to_string(), label: "Shadowsocks".to_string() },
                        ]}
                        onchange={Callback::from({
                            let outbound = outbound.clone();
                            move |value: String| {
                                let mut next = (*outbound).clone();
                                next.outbound_type = value.clone();
                                if next.name.trim().is_empty() {
                                    next.name = value.clone();
                                }
                                outbound.set(next);
                            }
                        })}
                    />
                </div>

                {
                    match data.outbound_type.trim().to_uppercase().as_str() {
                        "TRUSTTUNNEL" => html! {
                            <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                <TextBox label="Tag" value={data.trust_tunnel.tag.clone()} onchange={update_text(|outbound, value| outbound.trust_tunnel.tag = value)} />
                                <TextBox label="[listen_protocols.http1] upload_buffer_size" value={data.trust_tunnel.http1_upload_buffer_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http1_upload_buffer_size = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] initial_connection_window_size" value={data.trust_tunnel.http2_initial_connection_window_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_initial_connection_window_size = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] initial_stream_window_size" value={data.trust_tunnel.http2_initial_stream_window_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_initial_stream_window_size = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] max_concurrent_streams" value={data.trust_tunnel.http2_max_concurrent_streams.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_max_concurrent_streams = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] max_frame_size" value={data.trust_tunnel.http2_max_frame_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_max_frame_size = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="[listen_protocols.http2] header_table_size" value={data.trust_tunnel.http2_header_table_size.to_string()} onchange={update_text(|outbound, value| outbound.trust_tunnel.http2_header_table_size = value.parse().unwrap_or(0))} input_type="number" />
                            </div>
                        },
                        "WIREGUARD" => html! {
                            <div class="space-y-4">
                                <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                    <Dropdown
                                        label="Domain Strategy"
                                        value={data.wireguard.domain_strategy.clone()}
                                        options={vec![
                                            DropdownOption { value: "ForceIP".to_string(), label: "ForceIP".to_string() },
                                            DropdownOption { value: "ForceIPv4".to_string(), label: "ForceIPv4".to_string() },
                                            DropdownOption { value: "ForceIPv4v6".to_string(), label: "ForceIPv4v6".to_string() },
                                            DropdownOption { value: "ForceIPv6".to_string(), label: "ForceIPv6".to_string() },
                                            DropdownOption { value: "ForceIPv6v4".to_string(), label: "ForceIPv6v4".to_string() },
                                        ]}
                                        onchange={update_text(|outbound, value| outbound.wireguard.domain_strategy = value)}
                                    />
                                    <TextBox label="MTU" value={data.wireguard.mtu.to_string()} onchange={update_text(|outbound, value| outbound.wireguard.mtu = value.parse().unwrap_or(0))} input_type="number" />
                                    <TextBox label="Workers" value={data.wireguard.workers.to_string()} onchange={update_text(|outbound, value| outbound.wireguard.workers = value.parse().unwrap_or(0))} input_type="number" />
                                    <TextBox label="Private Key" value={data.wireguard.private_key.clone()} onchange={update_text(|outbound, value| outbound.wireguard.private_key = value)} />
                                    <TextBox label="Reserved Bytes" value={data.wireguard.reserved.clone()} onchange={update_text(|outbound, value| outbound.wireguard.reserved = value)} placeholder="1, 2, 3" />
                                    <TextBox label="Addresses" value={data.wireguard.addresses.clone()} onchange={update_text(|outbound, value| outbound.wireguard.addresses = value)} placeholder="172.16.0.2/32, 2606:4700:110:.../128" />
                                </div>
                                <div class="space-y-4">
                                    <div class="flex justify-between" style="align-items: center;">
                                        <div class="text-sm font-semibold">{ "Peers" }</div>
                                        <Button
                                            label="Add Peer"
                                            button_type={ButtonType::Outlined}
                                            onclick={Callback::from({
                                                let outbound = outbound.clone();
                                                move |_| {
                                                    let mut next = (*outbound).clone();
                                                    next.wireguard.peers.push(WireGuardPeerItem::default());
                                                    outbound.set(next);
                                                }
                                            })}
                                        />
                                    </div>
                                    {
                                        for data.wireguard.peers.iter().enumerate().map(|(idx, peer)| {
                                            html! {
                                                <div key={format!("wg-peer-inline-{}", idx)} class="md3-card bg-surface-container space-y-3">
                                                    <div class="flex justify-between" style="align-items: flex-start;">
                                                        <div class="text-sm font-semibold opacity-80">{ format!("Peer {}", idx + 1) }</div>
                                                        <IconButton
                                                            label="Delete Peer"
                                                            button_type={ButtonType::Text}
                                                            color={Some("#F2B8B5".to_string())}
                                                            onclick={Callback::from({
                                                                let outbound = outbound.clone();
                                                                move |_| {
                                                                    let mut next = (*outbound).clone();
                                                                    if idx < next.wireguard.peers.len() {
                                                                        next.wireguard.peers.remove(idx);
                                                                    }
                                                                    outbound.set(next);
                                                                }
                                                            })}
                                                        >
                                                            <SvgIcon name="delete_24dp" size={20} />
                                                        </IconButton>
                                                    </div>
                                                    <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                        <TextBox label="Public Key" value={peer.public_key.clone()} onchange={Callback::from({
                                                            let outbound = outbound.clone();
                                                            move |value: String| {
                                                                let mut next = (*outbound).clone();
                                                                if let Some(item) = next.wireguard.peers.get_mut(idx) {
                                                                    item.public_key = value;
                                                                }
                                                                outbound.set(next);
                                                            }
                                                        })} />
                                                        <TextBox label="Endpoint" value={peer.endpoint.clone()} onchange={Callback::from({
                                                            let outbound = outbound.clone();
                                                            move |value: String| {
                                                                let mut next = (*outbound).clone();
                                                                if let Some(item) = next.wireguard.peers.get_mut(idx) {
                                                                    item.endpoint = value;
                                                                }
                                                                outbound.set(next);
                                                            }
                                                        })} />
                                                    </div>
                                                    <TextBox label="Allowed IPs" value={peer.allowed_ips.clone()} onchange={Callback::from({
                                                        let outbound = outbound.clone();
                                                        move |value: String| {
                                                            let mut next = (*outbound).clone();
                                                            if let Some(item) = next.wireguard.peers.get_mut(idx) {
                                                                item.allowed_ips = value;
                                                            }
                                                            outbound.set(next);
                                                        }
                                                    })} is_textarea={true} placeholder="0.0.0.0/0, ::/0" />
                                                </div>
                                            }
                                        })
                                    }
                                </div>
                            </div>
                        },
                        "SOCKS5" => html! {
                            <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                <TextBox label="Tag" value={data.socks5.tag.clone()} onchange={update_text(|outbound, value| outbound.socks5.tag = value)} />
                                <TextBox label="Server" value={data.socks5.server.clone()} onchange={update_text(|outbound, value| outbound.socks5.server = value)} />
                                <TextBox label="Port" value={data.socks5.port.to_string()} onchange={update_text(|outbound, value| outbound.socks5.port = value.parse().unwrap_or(0))} input_type="number" />
                                <TextBox label="Username" value={data.socks5.username.clone()} onchange={update_text(|outbound, value| outbound.socks5.username = value)} />
                                <TextBox label="Password" value={data.socks5.password.clone()} onchange={update_text(|outbound, value| outbound.socks5.password = value)} />
                            </div>
                        },
                        "SHADOWSOCKS" => html! {
                            <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                <TextBox label="Tag" value={data.shadowsocks.tag.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.tag = value)} />
                                <TextBox label="Server" value={data.shadowsocks.server.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.server = value)} />
                                <TextBox label="Port" value={data.shadowsocks.port.to_string()} onchange={update_text(|outbound, value| outbound.shadowsocks.port = value.parse().unwrap_or(0))} input_type="number" />
                                <Dropdown
                                    label="Method"
                                    value={data.shadowsocks.method.clone()}
                                    options={shadowsocks_method_options()}
                                    onchange={update_text(|outbound, value| outbound.shadowsocks.method = value)}
                                />
                                <TextBox label="Password" value={data.shadowsocks.password.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.password = value)} />
                                <TextBox label="Plugin" value={data.shadowsocks.plugin.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.plugin = value)} placeholder="Optional" />
                                <TextBox label="Plugin Opts" value={data.shadowsocks.plugin_opts.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.plugin_opts = value)} placeholder="Optional" />
                                <TextBox label="Prefix (anti-DPI)" value={data.shadowsocks.prefix.clone()} onchange={update_text(|outbound, value| outbound.shadowsocks.prefix = value)} placeholder="Appended into plugin opts as prefix=..." />
                            </div>
                        },
                        _ => html! {
                            <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                <TextBox label="Tag" value={data.vless.tag.clone()} onchange={update_text(|outbound, value| outbound.vless.tag = value)} />
                                <TextBox label="Server" value={data.vless.server.clone()} onchange={update_text(|outbound, value| outbound.vless.server = value)} />
                                <TextBox label="Port" value={data.vless.port.to_string()} onchange={update_text(|outbound, value| outbound.vless.port = value.parse().unwrap_or(0))} input_type="number" />
                                <Dropdown
                                    label="Flow"
                                    value={data.vless.flow.clone()}
                                    options={vec![
                                        DropdownOption { value: "".to_string(), label: "None".to_string() },
                                        DropdownOption { value: "xtls-rprx-vision".to_string(), label: "xtls-rprx-vision".to_string() },
                                        DropdownOption { value: "xtls-rprx-vision-udp443".to_string(), label: "xtls-rprx-vision-udp443".to_string() },
                                    ]}
                                    onchange={update_text(|outbound, value| outbound.vless.flow = value)}
                                />
                                <Dropdown
                                    label="Security"
                                    value={data.vless.security.clone()}
                                    options={vec![
                                        DropdownOption { value: "NONE".to_string(), label: "None".to_string() },
                                        DropdownOption { value: "TLS".to_string(), label: "TLS".to_string() },
                                        DropdownOption { value: "REALITY".to_string(), label: "Reality".to_string() },
                                    ]}
                                    onchange={update_text(|outbound, value| outbound.vless.security = value)}
                                />
                                <Dropdown
                                    label="Transmission"
                                    value={vless_transmission_from(&data.vless.transmission)}
                                    options={vec![
                                        DropdownOption { value: "TCP".to_string(), label: "TCP (RAW)".to_string() },
                                        DropdownOption { value: "HTTP".to_string(), label: "HTTP".to_string() },
                                        DropdownOption { value: "gRPC".to_string(), label: "gRPC".to_string() },
                                        DropdownOption { value: "WebSocket".to_string(), label: "WebSocket".to_string() },
                                        DropdownOption { value: "mKCP".to_string(), label: "mKCP".to_string() },
                                        DropdownOption { value: "HttpUpgrade".to_string(), label: "HttpUpgrade".to_string() },
                                        DropdownOption { value: "SplitHTTP".to_string(), label: "SplitHTTP".to_string() },
                                    ]}
                                    onchange={update_text(|outbound, value| outbound.vless.transmission = value)}
                                />
                            </div>
                        }
                    }
                }

                <div class="md3-popup-actions">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    <Button label={if props.is_new { "Add Outbound" } else { "Apply Changes" }} button_type={ButtonType::Filled} onclick={Callback::from({
                        let on_save = props.on_save.clone();
                        let outbound = outbound.clone();
                        move |_| on_save.emit((*outbound).clone())
                    })} />
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct AccessLinkPopupProps {
    node: ProxyNode,
    inbound: InboundEntryDraft,
    accounts: Vec<AccountInfo>,
    on_close: Callback<()>,
}

#[function_component(AccessLinkPopup)]
fn access_link_popup(props: &AccessLinkPopupProps) -> Html {
    let snackbar = use_context::<SnackbarBus>();
    let initial_account = props
        .accounts
        .first()
        .map(|account| account.id.clone())
        .unwrap_or_default();
    let selected_account_id = use_state(|| initial_account);
    let copy_status = use_state(|| Option::<String>::None);
    let generated_link = use_state(|| Option::<String>::None);

    let selected_account = props
        .accounts
        .iter()
        .find(|account| account.id == *selected_account_id)
        .cloned();
    let qr = generated_link.as_ref().and_then(|value| qr_svg(value));

    html! {
        <Popup title="Generate Access Link" size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                {
                    if let Some(link) = (*generated_link).clone() {
                        html! {
                            <div class="space-y-4">
                                <div class="md3-qr-card">
                                    {
                                        if let Some(qr) = qr {
                                            Html::from_html_unchecked(AttrValue::from(qr))
                                        } else {
                                            html! { <div>{ "QR unavailable" }</div> }
                                        }
                                    }
                                </div>
                                <div>
                                    <label class="block text-sm font-medium mb-1 text-on-surface">{ "Access Link" }</label>
                                    <div class="md3-access-link">{ link.clone() }</div>
                                    <div style="margin-top: 0.5rem; display: flex; justify-content: flex-start;">
                                        <Button label="Copy" button_type={ButtonType::Tonal} onclick={Callback::from({
                                            let link = link.clone();
                                            let copy_status = copy_status.clone();
                                            let snackbar = snackbar.clone();
                                            move |_| {
                                                let copy_status = copy_status.clone();
                                                let link = link.clone();
                                                let snackbar = snackbar.clone();
                                                spawn_local(async move {
                                                    match copy_to_clipboard(link).await {
                                                        Ok(_) => {
                                                            copy_status.set(None);
                                                            if let Some(bus) = snackbar {
                                                                bus.push("Copied access link");
                                                            }
                                                        }
                                                        Err(error) => {
                                                            copy_status.set(Some(error.clone()));
                                                            if let Some(bus) = snackbar {
                                                                bus.push(error);
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                        })} />
                                    </div>
                                </div>
                            </div>
                        }
                    } else {
                        html! {
                            <>
                                <Dropdown
                                    label="User"
                                    value={(*selected_account_id).clone()}
                                    options={props.accounts.iter().map(|account| DropdownOption {
                                        value: account.id.clone(),
                                        label: account.name.clone(),
                                    }).collect::<Vec<_>>()}
                                    onchange={Callback::from({
                                        let selected_account_id = selected_account_id.clone();
                                        let generated_link = generated_link.clone();
                                        let copy_status = copy_status.clone();
                                        move |value: String| {
                                            selected_account_id.set(value);
                                            generated_link.set(None);
                                            copy_status.set(None);
                                        }
                                    })}
                                />
                                <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">{ "Select user, then click Generate. Access links are available for VLESS and TrustTunnel inbounds with user credentials and node address." }</div>
                            </>
                        }
                    }
                }
                {
                    if let Some(status) = &*copy_status {
                        html! { <div class="text-sm opacity-70">{ status }</div> }
                    } else {
                        html! {}
                    }
                }
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    {
                        if generated_link.is_some() {
                            html! {
                                <Button label="Back" button_type={ButtonType::Text} onclick={Callback::from({
                                    let generated_link = generated_link.clone();
                                    let copy_status = copy_status.clone();
                                    move |_| {
                                        generated_link.set(None);
                                        copy_status.set(None);
                                    }
                                })} />
                            }
                        } else {
                            html! {}
                        }
                    }
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    {
                        if generated_link.is_none() {
                            html! {
                                <Button label="Generate" button_type={ButtonType::Filled} onclick={Callback::from({
                                    let selected_account = selected_account.clone();
                                    let generated_link = generated_link.clone();
                                    let copy_status = copy_status.clone();
                                    let node = props.node.clone();
                                    let inbound = props.inbound.clone();
                                    move |_| {
                                        copy_status.set(None);
                                        match selected_account
                                            .as_ref()
                                            .ok_or_else(|| "Select user first".to_string())
                                            .and_then(|account| build_access_link(&node.config, &node, &inbound, account))
                                        {
                                            Ok(link) => generated_link.set(Some(link)),
                                            Err(error) => {
                                                generated_link.set(None);
                                                copy_status.set(Some(error));
                                            }
                                        }
                                    }
                                })} />
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct RoutingRuleEditorPopupProps {
    rule: RoutingRuleDraft,
    is_new: bool,
    inbound_options: Vec<String>,
    user_options: Vec<String>,
    outbound_options: Vec<DropdownOption>,
    on_close: Callback<()>,
    on_save: Callback<RoutingRuleDraft>,
}

#[function_component(RoutingRuleEditorPopup)]
fn routing_rule_editor_popup(props: &RoutingRuleEditorPopupProps) -> Html {
    let rule = use_state(|| props.rule.clone());
    let inbound_tag_query = use_state(String::new);
    let protocol_query = use_state(String::new);
    let protocol_open = use_state(|| false);
    let inbound_open = use_state(|| false);
    let protocol_input_ref = use_node_ref();
    let inbound_input_ref = use_node_ref();
    let user_query = use_state(String::new);
    let user_open = use_state(|| false);
    let user_input_ref = use_node_ref();
    let transport_value = use_state(|| {
        let mut has_tcp = false;
        let mut has_udp = false;
        for value in split_lines_csv(&props.rule.transport)
            .into_iter()
            .map(|value| value.trim().to_lowercase())
        {
            match value.as_str() {
                "tcp" => has_tcp = true,
                "udp" => has_udp = true,
                _ => {}
            }
        }
        match (has_tcp, has_udp) {
            (true, true) => "tcp,udp".to_string(),
            (true, false) => "tcp".to_string(),
            (false, true) => "udp".to_string(),
            (false, false) => "tcp,udp".to_string(),
        }
    });
    {
        let rule = rule.clone();
        let incoming = props.rule.clone();
        use_effect_with(incoming, move |next_rule| {
            rule.set(next_rule.clone());
            || ()
        });
    }

    let on_text_change = |mutator: fn(&mut RoutingRuleDraft, String)| {
        let rule = rule.clone();
        Callback::from(move |value: String| {
            let mut next = (*rule).clone();
            mutator(&mut next, value);
            rule.set(next);
        })
    };

    let on_save = {
        let on_save = props.on_save.clone();
        let rule = rule.clone();
        let transport_value = transport_value.clone();
        let allowed_user_options = props.user_options.clone();
        Callback::from(move |_| {
            let mut next = (*rule).clone();

            let mut transports: Vec<String> = match transport_value.as_str() {
                "tcp" => vec!["tcp".to_string()],
                "udp" => vec!["udp".to_string()],
                _ => vec!["tcp".to_string(), "udp".to_string()],
            };
            next.transport = transports.join(",");

            let allowed_app = ["http", "tls", "bittorrent"];
            let mut app_protocols = split_lines_csv(&next.protocol)
                .into_iter()
                .map(|value| value.trim().to_lowercase())
                .filter(|value| {
                    if value.is_empty() {
                        return false;
                    }
                    if value == "tcp" || value == "udp" {
                        return false;
                    }
                    allowed_app.iter().any(|p| p == value)
                })
                .collect::<Vec<_>>();
            app_protocols.sort();
            app_protocols.dedup();
            next.protocol = app_protocols.join(",");

            let allowed_users = split_lines_csv(&next.user)
                .into_iter()
                .map(|value| value.trim().to_string())
                .filter(|value| {
                    !value.is_empty() && allowed_user_options.iter().any(|opt| opt == value)
                })
                .collect::<Vec<_>>();
            let mut users = allowed_users;
            users.sort();
            users.dedup();
            next.user = users.join(", ");

            on_save.emit(next)
        })
    };
    let on_close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    let selected_inbound_tags = split_lines_csv(&rule.inbound_tag)
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let selected_users = split_lines_csv(&rule.user)
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let selected_protocols = split_lines_csv(&rule.protocol)
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let inbound_suggestions = {
        let needle = inbound_tag_query.trim().to_lowercase();
        let mut options = props
            .inbound_options
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .filter(|value| {
                !selected_inbound_tags
                    .iter()
                    .any(|existing| existing == value)
            })
            .collect::<Vec<_>>();

        if !needle.is_empty() {
            options.retain(|value| value.to_lowercase().contains(&needle));
        }
        options.sort();
        options.dedup();
        options
    };

    let user_suggestions = {
        let needle = user_query.trim().to_lowercase();
        let mut options = props
            .user_options
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .filter(|value| !selected_users.iter().any(|existing| existing == value))
            .collect::<Vec<_>>();

        if !needle.is_empty() {
            options.retain(|value| value.to_lowercase().contains(&needle));
        }
        options.sort();
        options.dedup();
        options
    };

    let protocol_suggestions = {
        let allowed = ["http", "tls", "bittorrent"];
        let needle = protocol_query.trim().to_lowercase();
        let mut options = allowed
            .iter()
            .map(|value| value.to_string())
            .filter(|value| !selected_protocols.iter().any(|existing| existing == value))
            .collect::<Vec<_>>();
        if !needle.is_empty() {
            options.retain(|value| value.to_lowercase().contains(&needle));
        }
        options.sort();
        options.dedup();
        options
    };

    html! {
        <Popup
            title={if props.is_new { "Add Routing Rule" } else { "Edit Routing Rule" }}
            size={PopupSize::Lg}
            on_close={props.on_close.clone()}
        >
            <div class="space-y-4">
                <TextBox
                    label="Remark"
                    value={rule.remark.clone()}
                    onchange={on_text_change(|draft, value| draft.remark = value)}
                    placeholder="Human-readable note for this rule"
                />
                <TextBox
                    label="Domains"
                    value={rule.domain.clone()}
                    onchange={on_text_change(|draft, value| draft.domain = value)}
                    is_textarea={true}
                    placeholder="example.com, api.example.com"
                />
                <TextBox
                    label="IPs"
                    value={rule.ip.clone()}
                    onchange={on_text_change(|draft, value| draft.ip = value)}
                    is_textarea={true}
                    placeholder="1.1.1.1, 10.0.0.0/24"
                />
                <TextBox
                    label="Ports"
                    value={rule.port.clone()}
                    onchange={on_text_change(|draft, value| draft.port = value)}
                    is_textarea={true}
                    placeholder="80,443"
                />
                <Dropdown
                    label="Transport"
                    value={(*transport_value).clone()}
                    options={vec![
                        DropdownOption { label: "tcp".to_string(), value: "tcp".to_string() },
                        DropdownOption { label: "udp".to_string(), value: "udp".to_string() },
                        DropdownOption { label: "tcp,udp".to_string(), value: "tcp,udp".to_string() },
                    ]}
                    onchange={Callback::from({
                        let transport_value = transport_value.clone();
                        move |value: String| transport_value.set(value)
                    })}
                />
                <div class="w-full">
                    <label class="block text-sm font-medium mb-1 text-on-surface">{ "Protocols" }</label>
                    <div style="position: relative;">
                        <div
                            class="md3-input"
                            tabindex={0}
                            style={format!(
                                "display:flex; flex-wrap:nowrap; gap:8px; align-items:center; padding: 8px 12px; cursor:text; border-color: {}; height: 48px; overflow-x:auto; overflow-y:hidden; white-space:nowrap;",
                                if *protocol_open { "var(--md-sys-color-primary)" } else { "var(--md-sys-color-primary-outline)" }
                            )}
                            onclick={Callback::from({
                                let protocol_input_ref = protocol_input_ref.clone();
                                let protocol_open = protocol_open.clone();
                                move |_| {
                                    protocol_open.set(true);
                                    if let Some(input) = protocol_input_ref.cast::<web_sys::HtmlInputElement>() {
                                        let _ = input.focus();
                                    }
                                }
                            })}
                            onfocus={Callback::from({
                                let protocol_open = protocol_open.clone();
                                move |_| protocol_open.set(true)
                            })}
                        >
                            {
                                for selected_protocols.iter().cloned().map(|proto| {
                                    let rule = rule.clone();
                                    let proto_remove = proto.clone();
                                    html! {
                                        <Chip
                                            label={AttrValue::from(proto)}
                                            mode={ChipMode::Outlined}
                                            trailing_icon={Some("close_24dp".to_string())}
                                            on_trailing_click={Some(Callback::from(move |_| {
                                                let mut next = (*rule).clone();
                                                let mut remaining = split_lines_csv(&next.protocol)
                                                    .into_iter()
                                                    .map(|value| value.trim().to_lowercase())
                                                    .filter(|value| {
                                                        if value.is_empty() {
                                                            return false;
                                                        }
                                                        if value == "tcp" || value == "udp" {
                                                            return true;
                                                        }
                                                        value != &proto_remove
                                                    })
                                                    .collect::<Vec<_>>();
                                                remaining.sort();
                                                remaining.dedup();
                                                next.protocol = remaining.join(", ");
                                                rule.set(next);
                                            }))}
                                        />
                                    }
                                })
                            }
                            <input
                                ref={protocol_input_ref.clone()}
                                type="text"
                                class=""
                                placeholder="Type to search, then pick from suggestions"
                                value={(*protocol_query).clone()}
                                onclick={Callback::from({
                                    let protocol_open = protocol_open.clone();
                                    move |_| protocol_open.set(true)
                                })}
                                onfocus={Callback::from({
                                    let protocol_open = protocol_open.clone();
                                    move |_| protocol_open.set(true)
                                })}
                                oninput={Callback::from({
                                    let protocol_query = protocol_query.clone();
                                    let protocol_open = protocol_open.clone();
                                    move |e: InputEvent| {
                                        let target = e.target().unwrap();
                                        let value = js_sys::Reflect::get(&target, &"value".into())
                                            .unwrap()
                                            .as_string()
                                            .unwrap();
                                        protocol_query.set(value);
                                        protocol_open.set(true);
                                    }
                                })}
                                onblur={Callback::from({
                                    let protocol_open = protocol_open.clone();
                                    move |_| {
                                        let protocol_open = protocol_open.clone();
                                        Timeout::new(120, move || protocol_open.set(false)).forget();
                                    }
                                })}
                                onkeydown={Callback::from({
                                    let rule = rule.clone();
                                    let protocol_query = protocol_query.clone();
                                    let protocol_suggestions = protocol_suggestions.clone();
                                    let selected_protocols = selected_protocols.clone();
                                    let protocol_open = protocol_open.clone();
                                    move |e: KeyboardEvent| {
                                        if e.key() != "Enter" {
                                            return;
                                        }
                                        e.prevent_default();
                                        let Some(first) = protocol_suggestions.first().cloned() else {
                                            return;
                                        };
                                        let mut next = (*rule).clone();
                                        let mut combined = selected_protocols.clone();
                                        if !combined.iter().any(|value| value == &first) {
                                            combined.push(first);
                                        }
                                        combined.sort();
                                        combined.dedup();
                                        let mut keep_transport = split_lines_csv(&next.protocol)
                                            .into_iter()
                                            .map(|value| value.trim().to_lowercase())
                                            .filter(|value| value == "tcp" || value == "udp")
                                            .collect::<Vec<_>>();
                                        keep_transport.extend(combined);
                                        keep_transport.sort();
                                        keep_transport.dedup();
                                        next.protocol = keep_transport.join(", ");
                                        rule.set(next);
                                        protocol_query.set(String::new());
                                        protocol_open.set(false);
                                    }
                                })}
                                style="border:0; outline:none; background:transparent; padding:0; margin:0; min-width: 120px; flex: 1 0 120px; color: inherit; font-size: 1rem;"
                            />
                        </div>
                        {
                            if !*protocol_open || protocol_suggestions.is_empty() {
                                html! {}
                            } else {
                                html! {
                                    <div
                                        class="md3-card bg-surface-container"
                                        style="position:absolute; z-index: 50; left: 0; right: 0; top: calc(100% + 6px); padding: 8px; max-height: 180px; overflow:auto;"
                                    >
                                        <div class="flex flex-wrap" style="gap: 8px; align-items: center;">
                                            {
                                                for protocol_suggestions.iter().cloned().map(|proto| {
                                                    let chip_label = proto.clone();
                                                    let rule = rule.clone();
                                                    let protocol_query = protocol_query.clone();
                                                    let protocol_open = protocol_open.clone();
                                                    let selected_protocols = selected_protocols.clone();
                                                    html! {
                                                        <span onmousedown={Callback::from(move |e: MouseEvent| {
                                                            e.prevent_default();
                                                            let mut next = (*rule).clone();
                                                            let mut combined = selected_protocols.clone();
                                                            if !combined.iter().any(|value| value == &proto) {
                                                                combined.push(proto.clone());
                                                            }
                                                            combined.sort();
                                                            combined.dedup();
                                                            let mut keep_transport = split_lines_csv(&next.protocol)
                                                                .into_iter()
                                                                .map(|value| value.trim().to_lowercase())
                                                                .filter(|value| value == "tcp" || value == "udp")
                                                                .collect::<Vec<_>>();
                                                            keep_transport.extend(combined);
                                                            keep_transport.sort();
                                                            keep_transport.dedup();
                                                            next.protocol = keep_transport.join(", ");
                                                            rule.set(next);
                                                            protocol_query.set(String::new());
                                                            protocol_open.set(false);
                                                        })}>
                                                            <Chip label={AttrValue::from(chip_label)} mode={ChipMode::Outlined} />
                                                        </span>
                                                    }
                                                })
                                            }
                                        </div>
                                    </div>
                                }
                            }
                        }
                    </div>
                </div>
                <div class="w-full">
                    <label class="block text-sm font-medium mb-1 text-on-surface">{ "Inbound Tags" }</label>
                    <div style="position: relative;">
                        <div
                            class="md3-input"
                            tabindex={0}
                            style={format!(
                                "display:flex; flex-wrap:nowrap; gap:8px; align-items:center; padding: 8px 12px; cursor:text; border-color: {}; height: 48px; overflow-x:auto; overflow-y:hidden; white-space:nowrap;",
                                if *inbound_open { "var(--md-sys-color-primary)" } else { "var(--md-sys-color-primary-outline)" }
                            )}
                            onclick={Callback::from({
                                let inbound_input_ref = inbound_input_ref.clone();
                                let inbound_open = inbound_open.clone();
                                move |_| {
                                    inbound_open.set(true);
                                    if let Some(input) = inbound_input_ref.cast::<web_sys::HtmlInputElement>() {
                                        let _ = input.focus();
                                    }
                                }
                            })}
                            onfocus={Callback::from({
                                let inbound_open = inbound_open.clone();
                                move |_| inbound_open.set(true)
                            })}
                        >
                            {
                                for selected_inbound_tags.iter().cloned().map(|tag| {
                                    let rule = rule.clone();
                                    let tag_remove = tag.clone();
                                    html! {
                                        <Chip
                                            label={AttrValue::from(tag)}
                                            mode={ChipMode::Outlined}
                                            trailing_icon={Some("close_24dp".to_string())}
                                            on_trailing_click={Some(Callback::from(move |_| {
                                                let mut next = (*rule).clone();
                                                let remaining = split_lines_csv(&next.inbound_tag)
                                                    .into_iter()
                                                    .map(|value| value.trim().to_string())
                                                    .filter(|value| !value.is_empty() && value != &tag_remove)
                                                    .collect::<Vec<_>>();
                                                next.inbound_tag = remaining.join(", ");
                                                rule.set(next);
                                            }))}
                                        />
                                    }
                                })
                            }
                            <input
                                ref={inbound_input_ref.clone()}
                                type="text"
                                class=""
                                placeholder="Type to search, then pick from suggestions"
                                value={(*inbound_tag_query).clone()}
                                onclick={Callback::from({
                                    let inbound_open = inbound_open.clone();
                                    move |_| inbound_open.set(true)
                                })}
                                onfocus={Callback::from({
                                    let inbound_open = inbound_open.clone();
                                    move |_| inbound_open.set(true)
                                })}
                                oninput={Callback::from({
                                    let inbound_tag_query = inbound_tag_query.clone();
                                    let inbound_open = inbound_open.clone();
                                    move |e: InputEvent| {
                                        let target = e.target().unwrap();
                                        let value = js_sys::Reflect::get(&target, &"value".into())
                                            .unwrap()
                                            .as_string()
                                            .unwrap();
                                        inbound_tag_query.set(value);
                                        inbound_open.set(true);
                                    }
                                })}
                                onblur={Callback::from({
                                    let inbound_open = inbound_open.clone();
                                    move |_| {
                                        let inbound_open = inbound_open.clone();
                                        Timeout::new(120, move || inbound_open.set(false)).forget();
                                    }
                                })}
                                onkeydown={Callback::from({
                                    let rule = rule.clone();
                                    let inbound_tag_query = inbound_tag_query.clone();
                                    let inbound_suggestions = inbound_suggestions.clone();
                                    let selected_inbound_tags = selected_inbound_tags.clone();
                                    let inbound_open = inbound_open.clone();
                                    move |e: KeyboardEvent| {
                                        if e.key() != "Enter" {
                                            return;
                                        }
                                        e.prevent_default();
                                        let Some(first) = inbound_suggestions.first().cloned() else {
                                            return;
                                        };
                                        let mut next = (*rule).clone();
                                        let mut combined = selected_inbound_tags.clone();
                                        if !combined.iter().any(|value| value == &first) {
                                            combined.push(first);
                                        }
                                        combined.sort();
                                        combined.dedup();
                                        next.inbound_tag = combined.join(", ");
                                        rule.set(next);
                                        inbound_tag_query.set(String::new());
                                        inbound_open.set(false);
                                    }
                                })}
                                style="border:0; outline:none; background:transparent; padding:0; margin:0; min-width: 140px; flex: 1 0 140px; color: inherit; font-size: 1rem;"
                            />
                        </div>
                        {
                            if !*inbound_open || inbound_suggestions.is_empty() {
                                html! {}
                            } else {
                                html! {
                                    <div
                                        class="md3-card bg-surface-container"
                                        style="position:absolute; z-index: 50; left: 0; right: 0; top: calc(100% + 6px); padding: 8px; max-height: 180px; overflow:auto;"
                                    >
                                        <div class="flex flex-wrap" style="gap: 8px; align-items: center;">
                                            {
                                                for inbound_suggestions.iter().cloned().map(|tag| {
                                                    let chip_label = tag.clone();
                                                    let rule = rule.clone();
                                                    let inbound_tag_query = inbound_tag_query.clone();
                                                    let inbound_open = inbound_open.clone();
                                                    let selected_inbound_tags = selected_inbound_tags.clone();
                                                    html! {
                                                        <span onmousedown={Callback::from(move |e: MouseEvent| {
                                                            e.prevent_default();
                                                            let mut next = (*rule).clone();
                                                            let mut combined = selected_inbound_tags.clone();
                                                            if !combined.iter().any(|value| value == &tag) {
                                                                combined.push(tag.clone());
                                                            }
                                                            combined.sort();
                                                            combined.dedup();
                                                            next.inbound_tag = combined.join(", ");
                                                            rule.set(next);
                                                            inbound_tag_query.set(String::new());
                                                            inbound_open.set(false);
                                                        })}>
                                                            <Chip label={AttrValue::from(chip_label)} mode={ChipMode::Outlined} />
                                                        </span>
                                                    }
                                                })
                                            }
                                        </div>
                                    </div>
                                }
                            }
                        }
                    </div>
                </div>
                <div class="w-full">
                    <label class="block text-sm font-medium mb-1 text-on-surface">{ "Users" }</label>
                    <div style="position: relative;">
                        <div
                            class="md3-input"
                            tabindex={0}
                            style={format!(
                                "display:flex; flex-wrap:nowrap; gap:8px; align-items:center; padding: 8px 12px; cursor:text; border-color: {}; height: 48px; overflow-x:auto; overflow-y:hidden; white-space:nowrap;",
                                if *user_open { "var(--md-sys-color-primary)" } else { "var(--md-sys-color-primary-outline)" }
                            )}
                            onclick={Callback::from({
                                let user_input_ref = user_input_ref.clone();
                                let user_open = user_open.clone();
                                move |_| {
                                    user_open.set(true);
                                    if let Some(input) = user_input_ref.cast::<web_sys::HtmlInputElement>() {
                                        let _ = input.focus();
                                    }
                                }
                            })}
                            onfocus={Callback::from({
                                let user_open = user_open.clone();
                                move |_| user_open.set(true)
                            })}
                        >
                            {
                                for selected_users.iter().cloned().map(|name| {
                                    let rule = rule.clone();
                                    let remove_name = name.clone();
                                    html! {
                                        <Chip
                                            label={AttrValue::from(name)}
                                            mode={ChipMode::Outlined}
                                            trailing_icon={Some("close_24dp".to_string())}
                                            on_trailing_click={Some(Callback::from(move |_| {
                                                let mut next = (*rule).clone();
                                                let remaining = split_lines_csv(&next.user)
                                                    .into_iter()
                                                    .map(|value| value.trim().to_string())
                                                    .filter(|value| !value.is_empty() && value != &remove_name)
                                                    .collect::<Vec<_>>();
                                                next.user = remaining.join(", ");
                                                rule.set(next);
                                            }))}
                                        />
                                    }
                                })
                            }
                            <input
                                ref={user_input_ref.clone()}
                                type="text"
                                class=""
                                placeholder="Type to search, then pick from suggestions"
                                value={(*user_query).clone()}
                                onclick={Callback::from({
                                    let user_open = user_open.clone();
                                    move |_| user_open.set(true)
                                })}
                                onfocus={Callback::from({
                                    let user_open = user_open.clone();
                                    move |_| user_open.set(true)
                                })}
                                oninput={Callback::from({
                                    let user_query = user_query.clone();
                                    let user_open = user_open.clone();
                                    move |e: InputEvent| {
                                        let target = e.target().unwrap();
                                        let value = js_sys::Reflect::get(&target, &"value".into())
                                            .unwrap()
                                            .as_string()
                                            .unwrap();
                                        user_query.set(value);
                                        user_open.set(true);
                                    }
                                })}
                                onblur={Callback::from({
                                    let user_open = user_open.clone();
                                    move |_| {
                                        let user_open = user_open.clone();
                                        Timeout::new(120, move || user_open.set(false)).forget();
                                    }
                                })}
                                onkeydown={Callback::from({
                                    let rule = rule.clone();
                                    let user_query = user_query.clone();
                                    let user_suggestions = user_suggestions.clone();
                                    let selected_users = selected_users.clone();
                                    let user_open = user_open.clone();
                                    move |e: KeyboardEvent| {
                                        if e.key() != "Enter" {
                                            return;
                                        }
                                        e.prevent_default();
                                        let Some(first) = user_suggestions.first().cloned() else {
                                            return;
                                        };
                                        let mut next = (*rule).clone();
                                        let mut combined = selected_users.clone();
                                        if !combined.iter().any(|value| value == &first) {
                                            combined.push(first);
                                        }
                                        combined.sort();
                                        combined.dedup();
                                        next.user = combined.join(", ");
                                        rule.set(next);
                                        user_query.set(String::new());
                                        user_open.set(false);
                                    }
                                })}
                                style="border:0; outline:none; background:transparent; padding:0; margin:0; min-width: 140px; flex: 1 0 140px; color: inherit; font-size: 1rem;"
                            />
                        </div>
                        {
                            if !*user_open || user_suggestions.is_empty() {
                                html! {}
                            } else {
                                html! {
                                    <div
                                        class="md3-card bg-surface-container"
                                        style="position:absolute; z-index: 50; left: 0; right: 0; top: calc(100% + 6px); padding: 8px; max-height: 180px; overflow:auto;"
                                    >
                                        <div class="flex flex-wrap" style="gap: 8px; align-items: center;">
                                            {
                                                for user_suggestions.iter().cloned().map(|name| {
                                                    let chip_label = name.clone();
                                                    let rule = rule.clone();
                                                    let user_query = user_query.clone();
                                                    let user_open = user_open.clone();
                                                    let selected_users = selected_users.clone();
                                                    html! {
                                                        <span onmousedown={Callback::from(move |e: MouseEvent| {
                                                            e.prevent_default();
                                                            let mut next = (*rule).clone();
                                                            let mut combined = selected_users.clone();
                                                            if !combined.iter().any(|value| value == &name) {
                                                                combined.push(name.clone());
                                                            }
                                                            combined.sort();
                                                            combined.dedup();
                                                            next.user = combined.join(", ");
                                                            rule.set(next);
                                                            user_query.set(String::new());
                                                            user_open.set(false);
                                                        })}>
                                                            <Chip label={AttrValue::from(chip_label)} mode={ChipMode::Outlined} />
                                                        </span>
                                                    }
                                                })
                                            }
                                        </div>
                                    </div>
                                }
                            }
                        }
                    </div>
                </div>
                <Dropdown
                    label="Outbound"
                    value={rule.outbound_tag.clone()}
                    options={props.outbound_options.clone()}
                    onchange={on_text_change(|draft, value| draft.outbound_tag = value)}
                />
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={on_close} />
                    <Button label={if props.is_new { "Add Rule" } else { "Apply Changes" }} button_type={ButtonType::Filled} onclick={on_save} />
                </div>
            </div>
        </Popup>
    }
}

#[function_component(NodeConfigPage)]
pub fn node_config_page(props: &NodeConfigPageProps) -> Html {
    let snackbar = use_context::<SnackbarBus>();
    let state = use_context::<UseStateHandle<State>>().expect("state context");
    let navigator = use_navigator();
    let node_id_for_rev = props.id.clone();
    let state_for_rev = state.clone();
    let selected_revision_id = use_state(move || {
        if let Some(node) = state_for_rev.nodes.iter().find(|n| n.id == node_id_for_rev) {
            return node.active_revision_id.clone();
        }
        String::new()
    });
    let node_id_for_init = props.id.clone();
    let state_for_init = state.clone();
    let draft = use_state(move || {
        if let Some(node) = state_for_init
            .nodes
            .iter()
            .find(|n| n.id == node_id_for_init)
        {
            if let Some(mut saved_draft) = storage::load_node_draft_local(&node.id) {
                saved_draft.master_key = node.master_key.clone();
                sync_draft(&mut saved_draft);
                return saved_draft;
            }
            if !node.config.inbounds.is_empty() || !node.config.outbounds.is_empty() {
                return node.config.clone();
            }
            if let Some(revision) = node
                .revisions
                .iter()
                .find(|revision| revision.id == node.active_revision_id)
                .or_else(|| node.revisions.last())
            {
                let mut next_draft = revision.config.clone();
                next_draft.master_key = node.master_key.clone();
                sync_draft(&mut next_draft);
                return next_draft;
            }
            let mut next_draft = default_node_draft(node);
            next_draft.master_key = node.master_key.clone();
            return next_draft;
        }
        NodeConfigDraft::default()
    });
    let active_tab = use_state(|| ConfigTab::Status);
    let editing_inbound = use_state(|| Option::<(InboundEntryDraft, bool)>::None);
    let editing_certificate = use_state(|| Option::<(CertificateDraft, bool)>::None);
    let editing_outbound = use_state(|| Option::<(OutboundEntryDraft, bool)>::None);
    let editing_dns_server = use_state(|| Option::<(usize, DnsServerDraft, bool)>::None);
    let editing_dns_host = use_state(|| Option::<(usize, DnsHostDraft, bool)>::None);
    let editing_routing_rule = use_state(|| Option::<(usize, RoutingRuleDraft, bool)>::None);
    let pending_routing_delete = use_state(|| Option::<usize>::None);
    let routing_move_anim = use_state(|| Option::<(usize, bool)>::None);
    let warp_popup_open = use_state(|| false);
    let access_link_inbound_id = use_state(|| Option::<String>::None);
    let deploy_confirm_open = use_state(|| false);
    let acme_confirm_open = use_state(|| false);
    let pending_acme_certificate = use_state(|| Option::<CertificateDraft>::None);
    let acme_logs = use_state(|| Option::<AcmeIssueResponse>::None);
    let acme_logs_open = use_state(|| false);
    let acme_loading = use_state(|| false);
    let live_status = use_state(|| Option::<NodeStatus>::None);
    let live_status_loading = use_state(|| false);
    let live_status_error = use_state(|| Option::<String>::None);
    let status_auto_refresh = use_state(|| true);
    let status_refresh_interval_ms = use_state(|| 2000u32);
    let status_refresh_menu_open = use_state(|| false);

    let node = state.nodes.iter().find(|node| node.id == props.id).cloned();

    {
        let draft = draft.clone();
        let node_id = props.id.clone();
        use_effect_with(
            (node_id.clone(), (*draft).clone()),
            move |(node_id, draft_value)| {
                storage::save_node_draft(node_id, draft_value);
                || ()
            },
        );
    }

    {
        let draft = draft.clone();
        let node_id = props.id.clone();
        use_effect_with(node_id.clone(), move |node_id| {
            storage::hydrate_desktop_node_draft(node_id.clone(), draft.clone());
            || ()
        });
    }

    let Some(node) = node else {
        return html! {
            <div class="p-6 space-y-6">
                <div class="flex justify-between" style="align-items: baseline;">
                    <Button
                        label="Back"
                        button_type={ButtonType::Text}
                        onclick={Callback::from(move |_| {
                            if let Some(navigator) = navigator.clone() {
                                navigator.push(&Route::Nodes);
                            }
                        })}
                    />
                </div>
                <div class="md3-card">
                    <h1 class="text-2xl font-bold">{ "Node not found" }</h1>
                </div>
            </div>
        };
    };

    let revision_options = if node.revisions.is_empty() {
        vec![("".to_string(), format!("Revision 1 ({})", today_string()))]
    } else {
        node.revisions
            .iter()
            .enumerate()
            .rev()
            .map(|(index, revision)| (revision.id.clone(), revision_label(index, revision)))
            .collect::<Vec<_>>()
    };

    let on_revision_change = {
        let selected_revision_id = selected_revision_id.clone();
        let draft = draft.clone();
        let node = node.clone();
        Callback::from(move |value: String| {
            selected_revision_id.set(value.clone());
            if let Some(revision) = node.revisions.iter().find(|revision| revision.id == value) {
                let mut next_draft = revision.config.clone();
                sync_draft(&mut next_draft);
                draft.set(next_draft);
            }
        })
    };

    let save_revision = {
        let state = state.clone();
        let draft = draft.clone();
        let selected_revision_id = selected_revision_id.clone();
        let snackbar = snackbar.clone();
        let node_id = node.id.clone();
        Callback::from(move |_| {
            if let Some(revision_id) = persist_revision(&state, &node_id, &draft) {
                selected_revision_id.set(revision_id);
                if let Some(bus) = &snackbar {
                    bus.push("Created revision");
                }
            }
        })
    };

    let deploy_revision = {
        let state = state.clone();
        let draft = draft.clone();
        let snackbar = snackbar.clone();
        let node_id = node.id.clone();
        let node_for_deploy = node.clone();
        let address = node.address.clone();
        Callback::from(move |_: ()| {
            let mut draft_value = (*draft).clone();
            sync_draft(&mut draft_value);
            if let Some(current_node) = state.nodes.iter().find(|node| node.id == node_id) {
                draft_value.master_key = current_node.master_key.clone();
            }
            let address = address.clone();
            let accounts = (*state).accounts.clone();
            let snackbar = snackbar.clone();
            let node_for_deploy = node_for_deploy.clone();
            spawn_local(async move {
                let applying_id = snackbar
                    .as_ref()
                    .map(|bus| bus.push("Deploying configuration..."));
                let api = ApiService::new(address.clone());
                let result = api
                    .update_config(build_full_config(&draft_value, &node_for_deploy, &accounts))
                    .await;
                if let Some(bus) = &snackbar {
                    if let Some(id) = applying_id {
                        bus.hide(id);
                    }
                }
                match result {
                    Ok(response) if response.success => {
                        if let Some(bus) = &snackbar {
                            bus.push("Deployed successfully");
                        }
                    }
                    Ok(response) => {
                        let msg = format!("Deploy failed: {}", response.error);
                        if let Some(bus) = &snackbar {
                            bus.push(msg);
                        }
                    }
                    Err(error) => {
                        let msg = format!("Deploy failed: {}", error);
                        if let Some(bus) = &snackbar {
                            bus.push(msg);
                        }
                    }
                }
            });
        })
    };
    let on_deploy_click = {
        let deploy_confirm_open = deploy_confirm_open.clone();
        Callback::from(move |_| deploy_confirm_open.set(true))
    };

    let on_back = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            if let Some(navigator) = navigator.clone() {
                navigator.push(&Route::Nodes);
            }
        })
    };

    let fetch_live_status = {
        let address = node.address.clone();
        let master_key = node.master_key.clone();
        let live_status = live_status.clone();
        let live_status_loading = live_status_loading.clone();
        let live_status_error = live_status_error.clone();
        Callback::from(move |_: ()| {
            let address = address.clone();
            let master_key = master_key.clone();
            let live_status = live_status.clone();
            let live_status_loading = live_status_loading.clone();
            let live_status_error = live_status_error.clone();
            live_status_loading.set(true);
            live_status_error.set(None);
            spawn_local(async move {
                let api = ApiService::new(address);
                match api.get_status(master_key).await {
                    Ok(status) => {
                        live_status.set(Some(status));
                    }
                    Err(error) => {
                        live_status_error.set(Some(error));
                    }
                }
                TimeoutFuture::new(500).await;
                live_status_loading.set(false);
            });
        })
    };

    let on_refresh_live_status = {
        let fetch_live_status = fetch_live_status.clone();
        Callback::from(move |_| fetch_live_status.emit(()))
    };

    {
        let active_tab = active_tab.clone();
        let fetch_live_status = fetch_live_status.clone();
        let status_auto_refresh = status_auto_refresh.clone();
        let status_refresh_interval_ms = status_refresh_interval_ms.clone();
        use_effect_with(
            (
                (*active_tab).clone(),
                *status_auto_refresh,
                *status_refresh_interval_ms,
            ),
            move |(tab, auto_refresh, refresh_ms)| {
                let interval = if *tab == ConfigTab::Status {
                    fetch_live_status.emit(());
                    if *auto_refresh {
                        Some(Interval::new(*refresh_ms, {
                            let fetch_live_status = fetch_live_status.clone();
                            move || fetch_live_status.emit(())
                        }))
                    } else {
                        None
                    }
                } else {
                    None
                };
                move || drop(interval)
            },
        );
    }

    let d = {
        let mut copy = (*draft).clone();
        sync_draft(&mut copy);
        copy
    };
    let inbounds = d.inbounds.clone();
    let routing_rules = d.routing_rules.clone();
    let mut routing_outbound_options: Vec<DropdownOption> = Vec::new();
    for outbound in &d.outbounds {
        let tag = outbound_tag_for_routing(outbound);
        if tag.trim().is_empty()
            || routing_outbound_options
                .iter()
                .any(|option| option.value == tag)
        {
            continue;
        }
        routing_outbound_options.push(DropdownOption {
            value: tag,
            label: outbound_label_for_routing(outbound),
        });
    }
    let routing_inbound_options = {
        let mut options = d
            .inbounds
            .iter()
            .map(|inbound| inbound.name.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        options.sort();
        options.dedup();
        options
    };
    let routing_user_options = {
        let mut options = state
            .accounts
            .iter()
            .map(|account| account.name.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        options.sort();
        options.dedup();
        options
    };
    let selected_access_inbound = (*access_link_inbound_id)
        .clone()
        .and_then(|id| inbounds.iter().find(|inbound| inbound.id == id).cloned());
    let wide_nav_items = nav_items();
    let wide_nav_active = nav_key(&*active_tab);
    let on_wide_nav_select = {
        let active_tab = active_tab.clone();
        Callback::from(move |value: AttrValue| {
            let tab = match value.as_str() {
                "outbounds" => ConfigTab::Outbounds,
                "routing" => ConfigTab::Routing,
                "settings" => ConfigTab::Settings,
                "status" => ConfigTab::Status,
                _ => ConfigTab::Inbounds,
            };
            active_tab.set(tab);
        })
    };

    html! {
        <div class="p-6 space-y-6" style="padding-bottom: 7.5rem;">
            <div class="flex justify-between" style="align-items: center;">
                <div class="flex items-center" style="gap: 1rem;">
                    <IconButton label="Back" button_type={ButtonType::Text} onclick={on_back}>
                        <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                            <path d="M14.71 6.71a1 1 0 0 1 0 1.41L10.83 12l3.88 3.88a1 1 0 1 1-1.41 1.41l-4.59-4.59a1 1 0 0 1 0-1.41l4.59-4.59a1 1 0 0 1 1.41 0Z" fill="currentColor" />
                        </svg>
                    </IconButton>
                    <h1 class="text-3xl font-bold">{ node.name.clone() }</h1>
                </div>
                <div class="flex items-center" style="gap: 0.75rem;">
                    <Dropdown
                        label=""
                        value={(*selected_revision_id).clone()}
                        options={revision_options.into_iter().map(|(value, label)| DropdownOption { value, label }).collect::<Vec<_>>()}
                        onchange={on_revision_change}
                        style={Some("min-width: 18rem;".to_string())}
                    />
                    <Button label="Create Revision" button_type={ButtonType::Outlined} onclick={save_revision} />
                    <Button label="Deploy" button_type={ButtonType::Filled} onclick={on_deploy_click} />
                </div>
            </div>

            {
                match &*active_tab {
                    ConfigTab::Inbounds => inbounds::render_inbounds_tab(&draft, &inbounds, &editing_inbound, &access_link_inbound_id),
                    ConfigTab::Outbounds => outbounds::render_outbounds_tab(&draft, &d.outbounds, &editing_outbound, &warp_popup_open),
                    ConfigTab::Routing => routing::render_routing_tab(
                        &draft,
                        &routing_rules,
                        &editing_routing_rule,
                        &pending_routing_delete,
                        &routing_move_anim,
                    ),
                    ConfigTab::Settings => settings::render_settings_tab(
                        &draft,
                        &d,
                        &editing_certificate,
                        &editing_dns_server,
                        &editing_dns_host,
                        &acme_confirm_open,
                        &pending_acme_certificate,
                    ),
                    ConfigTab::Status => status::render_status_tab(
                        &node,
                        &live_status,
                        &live_status_loading,
                        &live_status_error,
                        &status_auto_refresh,
                        &status_refresh_interval_ms,
                        &status_refresh_menu_open,
                        &on_refresh_live_status,
                    ),
                }
            }

            <div class="md3-config-nav">
                <WideNavigationBar
                    items={wide_nav_items}
                    active_value={wide_nav_active}
                    on_select={on_wide_nav_select}
                />
            </div>

            {
                if let Some((rule_index, rule, is_new)) = &*editing_routing_rule {
                    html! {
                        <RoutingRuleEditorPopup
                            rule={rule.clone()}
                            is_new={*is_new}
                            inbound_options={routing_inbound_options.clone()}
                            user_options={routing_user_options.clone()}
                            outbound_options={routing_outbound_options.clone()}
                            on_close={Callback::from({
                                let editing_routing_rule = editing_routing_rule.clone();
                                move |_| editing_routing_rule.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_routing_rule = editing_routing_rule.clone();
                                let rule_index = *rule_index;
                                move |rule: RoutingRuleDraft| {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    if let Some(existing) = next.routing_rules.get_mut(rule_index) {
                                        *existing = rule;
                                    } else {
                                        next.routing_rules.push(rule);
                                    }
                                    sync_draft(&mut next);
                                    draft.set(next);
                                    editing_routing_rule.set(None);
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if *deploy_confirm_open {
                    html! {
                        <ConfirmPopup
                            title="Deploy Revision"
                            body="Deploy current draft to this node now? This overwrites active runtime configuration."
                            confirm_label="Deploy"
                            align_actions_end={true}
                            on_close={Callback::from({
                                let deploy_confirm_open = deploy_confirm_open.clone();
                                move |_| deploy_confirm_open.set(false)
                            })}
                            on_confirm={Callback::from({
                                let deploy_confirm_open = deploy_confirm_open.clone();
                                let deploy_revision = deploy_revision.clone();
                                move |_| {
                                    deploy_confirm_open.set(false);
                                    deploy_revision.emit(());
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some(rule_index) = *pending_routing_delete {
                    html! {
                        <ConfirmPopup
                            title="Delete Rule"
                            body="Are you sure you want to delete this routing rule?"
                            confirm_label="Delete"
                            align_actions_end={true}
                            on_close={Callback::from({
                                let pending_routing_delete = pending_routing_delete.clone();
                                move |_| pending_routing_delete.set(None)
                            })}
                            on_confirm={Callback::from({
                                let draft = draft.clone();
                                let pending_routing_delete = pending_routing_delete.clone();
                                move |_| {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    if rule_index < next.routing_rules.len() {
                                        next.routing_rules.remove(rule_index);
                                    }
                                    sync_draft(&mut next);
                                    draft.set(next);
                                    pending_routing_delete.set(None);
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if *warp_popup_open {
                    html! {
                        <WarpCreatePopup
                            node_address={node.address.clone()}
                            master_key={node.master_key.clone()}
                            initial_registration={initial_warp_registration(&d)}
                            on_registration_change={Callback::from({
                                let draft = draft.clone();
                                move |registration: Option<crate::services::warp::WarpRegistration>| {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    let next_registration = registration
                                        .as_ref()
                                        .map(warp_registration_to_draft)
                                        .unwrap_or_default();
                                    if next.warp_registration != next_registration {
                                        next.warp_registration = next_registration;
                                        sync_draft(&mut next);
                                        draft.set(next);
                                    }
                                }
                            })}
                            on_close={Callback::from({
                                let warp_popup_open = warp_popup_open.clone();
                                move |_| warp_popup_open.set(false)
                            })}
                            on_create={Callback::from({
                                let draft = draft.clone();
                                let warp_popup_open = warp_popup_open.clone();
                                let snackbar = snackbar.clone();
                                move |outbound: OutboundEntryDraft| {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    next.outbounds.push(outbound);
                                    sync_draft(&mut next);
                                    draft.set(next);
                                    warp_popup_open.set(false);
                                    if let Some(bus) = &snackbar {
                                        bus.push("WireGuard outbound created from WARP account");
                                    }
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if *acme_confirm_open {
                    html! {
                        <ConfirmPopup
                            title="Request Certificate"
                            body="This will trigger ACME issuance on node. Certificate authorities enforce rate limits, so repeated failed attempts can temporarily block new issuance. Continue only if domain, ports, and challenge routing are ready."
                            confirm_label="Request"
                            on_close={Callback::from({
                                let acme_confirm_open = acme_confirm_open.clone();
                                let pending_acme_certificate = pending_acme_certificate.clone();
                                move |_| {
                                    acme_confirm_open.set(false);
                                    pending_acme_certificate.set(None);
                                }
                            })}
                            on_confirm={Callback::from({
                                let acme_confirm_open = acme_confirm_open.clone();
                                let pending_acme_certificate = pending_acme_certificate.clone();
                                let acme_logs = acme_logs.clone();
                                let acme_logs_open = acme_logs_open.clone();
                                let acme_loading = acme_loading.clone();
                                let node = node.clone();
                                let draft = draft.clone();
                                move |_| {
                                    let Some(selected_certificate) = (*pending_acme_certificate).clone() else {
                                        acme_confirm_open.set(false);
                                        return;
                                    };
                                    acme_confirm_open.set(false);
                                    pending_acme_certificate.set(None);
                                    acme_logs_open.set(true);
                                    acme_loading.set(true);
                                    acme_logs.set(Some(AcmeIssueResponse {
                                        success: false,
                                        error: String::new(),
                                        logs: vec!["Sending ACME request to node...".to_string()],
                                        expiry_time: 0,
                                    }));

                                    let api = ApiService::new(node.address.clone());
                                    let draft_value = (*draft).clone();
                                    let acme_logs = acme_logs.clone();
                                    let acme_loading = acme_loading.clone();
                                    let draft = draft.clone();
                                    spawn_local(async move {
                                        let challenge_port = if selected_certificate.acme_type.eq_ignore_ascii_case("HTTP") {
                                            selected_certificate.acme_http_port
                                        } else {
                                            selected_certificate.acme_port
                                        };
                                        let response = api.issue_acme_certificate(AcmeIssueRequest {
                                            master_key: draft_value.master_key.clone(),
                                            email: selected_certificate.acme_email.clone(),
                                            domain: selected_certificate.acme_domain.clone(),
                                            challenge_type: selected_certificate.acme_type.clone(),
                                            ca: selected_certificate.acme_ca.clone(),
                                            port: challenge_port,
                                            certificate_path: selected_certificate.certificate_path.clone(),
                                            key_path: selected_certificate.key_path.clone(),
                                        }).await;

                                        acme_loading.set(false);
                                        match response {
                                            Ok(result) => {
                                                let mut next = (*draft).clone();
                                                sync_draft(&mut next);
                                                if let Some(certificate) = next.certificates.iter_mut().find(|item| item.id == selected_certificate.id) {
                                                    let (certificate_path, key_path) = certmagic_certificate_paths(
                                                        &certificate.acme_ca,
                                                        &certificate.acme_domain,
                                                    );
                                                    certificate.certificate_path = certificate_path;
                                                    certificate.key_path = key_path;
                                                    if result.expiry_time > 0 {
                                                        certificate.expiry_time = result.expiry_time;
                                                    }
                                                }
                                                draft.set(next);
                                                acme_logs.set(Some(result));
                                            }
                                            Err(error) => acme_logs.set(Some(AcmeIssueResponse {
                                                success: false,
                                                error: error.clone(),
                                                logs: vec![
                                                    "Sending ACME request to node...".to_string(),
                                                    format!("Request failed: {}", error),
                                                ],
                                                expiry_time: 0,
                                            })),
                                        }
                                    });
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if *acme_logs_open {
                    if let Some(result) = &*acme_logs {
                        html! {
                            <AcmeLogsPopup
                                title="ACME Logs"
                                logs={result.logs.clone()}
                                loading={*acme_loading}
                                success={result.success}
                                error={result.error.clone()}
                                on_close={Callback::from({
                                    let acme_logs_open = acme_logs_open.clone();
                                    move |_| acme_logs_open.set(false)
                                })}
                            />
                        }
                    } else {
                        html! {}
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some((server_index, server, is_new)) = &*editing_dns_server {
                    html! {
                        <DnsServerEditorPopup
                            server={server.clone()}
                            is_new={*is_new}
                            on_close={Callback::from({
                                let editing_dns_server = editing_dns_server.clone();
                                move |_| editing_dns_server.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_dns_server = editing_dns_server.clone();
                                let server_index = *server_index;
                                move |server: DnsServerDraft| {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    if server_index < next.dns.servers.len() {
                                        next.dns.servers[server_index] = server.clone();
                                    } else {
                                        next.dns.servers.push(server);
                                    }
                                    draft.set(next);
                                    editing_dns_server.set(None);
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some((host_index, host, is_new)) = &*editing_dns_host {
                    html! {
                        <DnsHostEditorPopup
                            host={host.clone()}
                            is_new={*is_new}
                            on_close={Callback::from({
                                let editing_dns_host = editing_dns_host.clone();
                                move |_| editing_dns_host.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_dns_host = editing_dns_host.clone();
                                let host_index = *host_index;
                                move |host: DnsHostDraft| {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    if host_index < next.dns.hosts.len() {
                                        next.dns.hosts[host_index] = host.clone();
                                    } else {
                                        next.dns.hosts.push(host);
                                    }
                                    draft.set(next);
                                    editing_dns_host.set(None);
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some((certificate, is_new)) = &*editing_certificate {
                    html! {
                        <CertificateEditorPopup
                            certificate={certificate.clone()}
                            is_new={*is_new}
                            on_close={Callback::from({
                                let editing_certificate = editing_certificate.clone();
                                move |_| editing_certificate.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_certificate = editing_certificate.clone();
                                move |certificate: CertificateDraft| {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    if let Some(existing) = next.certificates.iter_mut().find(|item| item.id == certificate.id) {
                                        let previous_name = existing.name.clone();
                                        *existing = certificate.clone();
                                        if previous_name != certificate.name {
                                            for inbound in next.inbounds.iter_mut() {
                                                if inbound.tls.certificate_name == previous_name {
                                                    inbound.tls.certificate_name = certificate.name.clone();
                                                }
                                            }
                                        }
                                    } else {
                                        next.certificates.push(certificate.clone());
                                        for inbound in next.inbounds.iter_mut() {
                                            if inbound.tls.certificate_name.trim().is_empty() {
                                                inbound.tls.certificate_name = certificate.name.clone();
                                            }
                                        }
                                    }
                                    draft.set(next);
                                    editing_certificate.set(None);
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some((inbound, is_new)) = &*editing_inbound {
                    html! {
                        <InboundEditorPopup
                            inbound={inbound.clone()}
                            certificates={d.certificates.clone()}
                            is_new={*is_new}
                            on_close={Callback::from({
                                let editing_inbound = editing_inbound.clone();
                                move |_| editing_inbound.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_inbound = editing_inbound.clone();
                                move |inbound: InboundEntryDraft| {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    if let Some(existing) = next.inbounds.iter_mut().find(|item| item.id == inbound.id) {
                                        *existing = inbound;
                                    } else {
                                        next.inbounds.push(inbound);
                                    }
                                    sync_draft(&mut next);
                                    draft.set(next);
                                    editing_inbound.set(None);
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some((outbound, is_new)) = &*editing_outbound {
                    html! {
                        <OutboundEditorPopup
                            outbound={outbound.clone()}
                            is_new={*is_new}
                            node_address={node.address.clone()}
                            master_key={node.master_key.clone()}
                            on_close={Callback::from({
                                let editing_outbound = editing_outbound.clone();
                                move |_| editing_outbound.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_outbound = editing_outbound.clone();
                                move |outbound: OutboundEntryDraft| {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    if let Some(existing) = next.outbounds.iter_mut().find(|item| item.id == outbound.id) {
                                        *existing = outbound;
                                    } else {
                                        next.outbounds.push(outbound);
                                    }
                                    sync_draft(&mut next);
                                    draft.set(next);
                                    editing_outbound.set(None);
                                }
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some(inbound) = selected_access_inbound {
                    html! {
                        <AccessLinkPopup
                            node={node.clone()}
                            inbound={inbound}
                            accounts={state.accounts.clone()}
                            on_close={Callback::from({
                                let access_link_inbound_id = access_link_inbound_id.clone();
                                move |_| access_link_inbound_id.set(None)
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
