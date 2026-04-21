use serde::{de::Deserializer, Deserialize, Serialize};

use crate::storage;

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub nodes: Vec<ProxyNode>,
    pub accounts: Vec<AccountInfo>,
}

impl State {
    pub fn load() -> Self {
        storage::load_state()
    }

    pub fn save(&self) {
        storage::save_state(self);
    }

    pub fn sanitized_for_storage(&self) -> Self {
        let mut cloned = self.clone();
        for node in &mut cloned.nodes {
            node.config = node.config.sanitized_for_storage();
            for revision in &mut node.revisions {
                revision.config = revision.config.sanitized_for_storage();
            }
        }
        cloned
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyNode {
    pub id: String,
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub public_ip: String,
    pub master_key: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub revisions: Vec<NodeConfigRevision>,
    #[serde(default)]
    pub active_revision_id: String,
    #[serde(default)]
    pub config: NodeConfigDraft,
    #[serde(default)]
    pub bandwidth_mbps: Option<u32>,
    #[serde(default)]
    pub max_traffic_bytes: Option<u64>,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeConfigRevision {
    pub id: String,
    pub created_at: String,
    pub config: NodeConfigDraft,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeConfigDraft {
    #[serde(default)]
    pub inbounds: Vec<InboundEntryDraft>,
    #[serde(default)]
    pub outbounds: Vec<OutboundEntryDraft>,
    #[serde(default)]
    pub certificates: Vec<CertificateDraft>,
    pub master_key: String,
    #[serde(default)]
    pub routing_rules: Vec<RoutingRuleDraft>,
    #[serde(default)]
    pub dns: DnsDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub warp_registration: WarpRegistrationDraft,
}

impl NodeConfigDraft {
    pub fn sanitized_for_storage(&self) -> Self {
        let mut cloned = self.clone();
        for inbound in &mut cloned.inbounds {
            sanitize_inbound_for_storage(inbound);
        }
        for outbound in &mut cloned.outbounds {
            sanitize_outbound_for_storage(outbound);
        }
        cloned
    }
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct CertificateDraft {
    pub id: String,
    pub name: String,
    #[serde(default = "default_certificate_type")]
    pub cert_type: String,
    #[serde(default = "default_certificate_source")]
    pub source: String,
    #[serde(default = "default_acme_type")]
    pub acme_type: String,
    #[serde(default = "default_acme_ca")]
    pub acme_ca: String,
    pub acme_email: String,
    pub acme_domain: String,
    pub certificate_path: String,
    pub key_path: String,
    pub certificate_pem: String,
    pub key_pem: String,
    pub acme_port: i32,
    pub acme_http_port: i32,
}

fn default_certificate_source() -> String {
    "PATH".to_string()
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutboundEntryDraft {
    pub id: String,
    pub name: String,
    pub outbound_type: String,
    #[serde(default = "default_outbound_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub vless: VlessOutboundDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub trust_tunnel: TrustTunnelOutboundDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub wireguard: WireGuardDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub socks5: Socks5Draft,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboundEntryDraft {
    pub id: String,
    pub name: String,
    pub listen: String,
    pub port: i32,
    #[serde(default = "default_inbound_enabled")]
    pub enabled: bool,
    pub core_type: String,
    pub protocol: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub tls: TlsDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub vless: VlessInboundDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub hysteria2: Hysteria2Draft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub trust_tunnel: TrustTunnelDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub naive_proxy: NaiveProxyDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub wireguard: WireGuardDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub socks5: Socks5Draft,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    v == &T::default()
}

fn default_inbound_enabled() -> bool {
    true
}

fn default_outbound_enabled() -> bool {
    true
}

fn default_certificate_type() -> String {
    "ACME".to_string()
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct TlsDraft {
    #[serde(default, skip_serializing_if = "is_false")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub server_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub certificate_name: String,
}

fn default_acme_type() -> String {
    "HTTP".to_string()
}

fn default_acme_ca() -> String {
    "letsencrypt".to_string()
}

fn default_reality_utls() -> String {
    "chrome".to_string()
}

fn default_reality_spider_x() -> String {
    "/".to_string()
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_default_reality_utls_value(value: &String) -> bool {
    *value == default_reality_utls()
}

fn is_default_reality_spider_x_value(value: &String) -> bool {
    *value == default_reality_spider_x()
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct VlessInboundDraft {
    pub uuid: String,
    pub flow: String,
    pub security: String,
    #[serde(default = "default_vless_transmission")]
    pub transmission: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reality_dest: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reality_private_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reality_public_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reality_sni: String,
    #[serde(
        default = "default_reality_utls",
        skip_serializing_if = "is_default_reality_utls_value"
    )]
    pub reality_utls: String,
    #[serde(
        default = "default_reality_spider_x",
        skip_serializing_if = "is_default_reality_spider_x_value"
    )]
    pub reality_spider_x: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reality_short_ids: String,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hysteria2Draft {
    pub password: String,
    pub obfs_type: String,
    pub obfs_password: String,
    pub up_mbps: u32,
    pub down_mbps: u32,
    pub ignore_client_bandwidth: bool,
    pub masquerade: String,
    pub bbr_profile: String,
    pub brutal_debug: bool,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustTunnelDraft {
    #[serde(default = "default_http1_upload_buffer_size")]
    pub http1_upload_buffer_size: u32,
    #[serde(default = "default_http2_initial_connection_window_size")]
    pub http2_initial_connection_window_size: u32,
    #[serde(default = "default_http2_initial_stream_window_size")]
    pub http2_initial_stream_window_size: u32,
    #[serde(default = "default_http2_max_concurrent_streams")]
    pub http2_max_concurrent_streams: u32,
    #[serde(default = "default_http2_max_frame_size")]
    pub http2_max_frame_size: u32,
    #[serde(default = "default_http2_header_table_size")]
    pub http2_header_table_size: u32,
}

impl Default for TrustTunnelDraft {
    fn default() -> Self {
        Self {
            http1_upload_buffer_size: default_http1_upload_buffer_size(),
            http2_initial_connection_window_size: default_http2_initial_connection_window_size(),
            http2_initial_stream_window_size: default_http2_initial_stream_window_size(),
            http2_max_concurrent_streams: default_http2_max_concurrent_streams(),
            http2_max_frame_size: default_http2_max_frame_size(),
            http2_header_table_size: default_http2_header_table_size(),
        }
    }
}

fn default_http1_upload_buffer_size() -> u32 {
    32768
}
fn default_http2_initial_connection_window_size() -> u32 {
    8_388_608
}
fn default_http2_initial_stream_window_size() -> u32 {
    131_072
}
fn default_http2_max_concurrent_streams() -> u32 {
    1000
}
fn default_http2_max_frame_size() -> u32 {
    16384
}
fn default_http2_header_table_size() -> u32 {
    65536
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct NaiveProxyDraft {
    pub username: String,
    pub password: String,
    pub protocol: String,
    pub target: String,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct VlessOutboundDraft {
    pub tag: String,
    pub server: String,
    pub port: i32,
    pub uuid: String,
    pub flow: String,
    pub security: String,
    #[serde(default = "default_vless_transmission")]
    pub transmission: String,
}

fn default_vless_transmission() -> String {
    "TCP".to_string()
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustTunnelOutboundDraft {
    pub tag: String,
    #[serde(default = "default_http1_upload_buffer_size")]
    pub http1_upload_buffer_size: u32,
    #[serde(default = "default_http2_initial_connection_window_size")]
    pub http2_initial_connection_window_size: u32,
    #[serde(default = "default_http2_initial_stream_window_size")]
    pub http2_initial_stream_window_size: u32,
    #[serde(default = "default_http2_max_concurrent_streams")]
    pub http2_max_concurrent_streams: u32,
    #[serde(default = "default_http2_max_frame_size")]
    pub http2_max_frame_size: u32,
    #[serde(default = "default_http2_header_table_size")]
    pub http2_header_table_size: u32,
}

impl Default for TrustTunnelOutboundDraft {
    fn default() -> Self {
        Self {
            tag: String::new(),
            http1_upload_buffer_size: default_http1_upload_buffer_size(),
            http2_initial_connection_window_size: default_http2_initial_connection_window_size(),
            http2_initial_stream_window_size: default_http2_initial_stream_window_size(),
            http2_max_concurrent_streams: default_http2_max_concurrent_streams(),
            http2_max_frame_size: default_http2_max_frame_size(),
            http2_header_table_size: default_http2_header_table_size(),
        }
    }
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireGuardDraft {
    pub tag: String,
    pub private_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub warp_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub warp_token: String,
    pub reserved: String,
    pub mtu: i32,
    #[serde(default = "default_wireguard_workers")]
    pub workers: i32,
    #[serde(default = "default_wireguard_domain_strategy")]
    pub domain_strategy: String,
    pub addresses: String,
    #[serde(
        default,
        deserialize_with = "deserialize_wireguard_peers",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub peers: Vec<WireGuardPeerItem>,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarpRegistrationDraft {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub private_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub public_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub peer_public_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reserved: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub addresses: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint: String,
}

fn default_wireguard_workers() -> i32 {
    2
}

fn default_wireguard_domain_strategy() -> String {
    "ForceIP".to_string()
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireGuardPeerItem {
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub allowed_ips: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WireGuardPeersCompat {
    Text(String),
    Items(Vec<WireGuardPeerItem>),
}

fn deserialize_wireguard_peers<'de, D>(
    deserializer: D,
) -> Result<Vec<WireGuardPeerItem>, D::Error>
where
    D: Deserializer<'de>,
{
    let parsed = Option::<WireGuardPeersCompat>::deserialize(deserializer)?;
    Ok(match parsed {
        None => Vec::new(),
        Some(WireGuardPeersCompat::Items(items)) => items,
        Some(WireGuardPeersCompat::Text(text)) => text
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let mut parts = line.split(';').map(|part| part.trim());
                let public_key = parts.next().unwrap_or_default().to_string();
                if public_key.is_empty() {
                    return None;
                }
                let endpoint = parts.next().unwrap_or_default().to_string();
                let allowed_ips = parts.next().unwrap_or_default().to_string();
                Some(WireGuardPeerItem {
                    public_key,
                    endpoint,
                    allowed_ips,
                })
            })
            .collect(),
    })
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Socks5Draft {
    pub tag: String,
    pub server: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub udp_enabled: bool,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingRuleDraft {
    #[serde(default)]
    pub remark: String,
    pub domain: String,
    pub ip: String,
    pub port: String,
    pub transport: String,
    pub protocol: String,
    pub outbound_tag: String,
    #[serde(default)]
    pub inbound_tag: String,
    #[serde(default)]
    pub user: String,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct DnsDraft {
    #[serde(default)]
    pub servers: Vec<DnsServerDraft>,
    #[serde(default)]
    pub hosts: Vec<DnsHostDraft>,
    #[serde(default)]
    pub client_ip: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub query_strategy: String,
    #[serde(default)]
    pub disable_cache: bool,
    #[serde(default)]
    pub serve_stale: bool,
    #[serde(default)]
    pub serve_expired_ttl: u32,
    #[serde(default)]
    pub disable_fallback: bool,
    #[serde(default)]
    pub disable_fallback_if_match: bool,
    #[serde(default)]
    pub enable_parallel_query: bool,
    #[serde(default)]
    pub use_system_hosts: bool,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct DnsServerDraft {
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub client_ip: String,
    #[serde(default)]
    pub port: u32,
    #[serde(default)]
    pub skip_fallback: bool,
    #[serde(default)]
    pub domains: String,
    #[serde(default)]
    pub expect_ips: String,
    #[serde(default)]
    pub query_strategy: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub timeout_ms: u64,
    #[serde(default)]
    pub disable_cache: Option<bool>,
    #[serde(default)]
    pub serve_stale: Option<bool>,
    #[serde(default)]
    pub serve_expired_ttl: Option<u32>,
    #[serde(default)]
    pub final_query: bool,
    #[serde(default)]
    pub unexpected_ips: String,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct DnsHostDraft {
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub values: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountInfo {
    pub id: String,
    pub name: String,
    pub access_id: String,
    pub allowed_ips: Vec<String>,
    pub expiry_date: i64,
}

fn sanitize_inbound_for_storage(inbound: &mut InboundEntryDraft) {
    match inbound.protocol.trim().to_uppercase().as_str() {
        "VLESS" => {
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
        }
        "HYSTERIA2" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
        }
        "TRUSTTUNNEL" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
        }
        "NAIVEPROXY" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
        }
        "WIREGUARD" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.socks5 = Socks5Draft::default();
        }
        "SOCKS5" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
        }
        _ => {}
    }
}

fn sanitize_outbound_for_storage(outbound: &mut OutboundEntryDraft) {
    match outbound.outbound_type.trim().to_uppercase().as_str() {
        "VLESS" => {
            outbound.trust_tunnel = TrustTunnelOutboundDraft::default();
            outbound.wireguard = WireGuardDraft::default();
            outbound.socks5 = Socks5Draft::default();
        }
        "TRUSTTUNNEL" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.wireguard = WireGuardDraft::default();
            outbound.socks5 = Socks5Draft::default();
        }
        "WIREGUARD" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.trust_tunnel = TrustTunnelOutboundDraft::default();
            outbound.socks5 = Socks5Draft::default();
        }
        "SOCKS5" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.trust_tunnel = TrustTunnelOutboundDraft::default();
            outbound.wireguard = WireGuardDraft::default();
        }
        "DIRECT" | "BLOCK" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.trust_tunnel = TrustTunnelOutboundDraft::default();
            outbound.wireguard = WireGuardDraft::default();
            outbound.socks5 = Socks5Draft::default();
        }
        _ => {}
    }
}
