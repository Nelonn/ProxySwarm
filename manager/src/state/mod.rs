use serde::{de::Deserializer, Deserialize, Serialize};

use crate::storage;

fn normalize_account_token(value: &str) -> String {
    let token = value.trim();
    if token.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        token.to_string()
    }
}

fn generate_account_id() -> String {
    uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect()
}

fn now_unix_timestamp() -> i64 {
    (js_sys::Date::now() / 1000.0).floor() as i64
}

fn normalize_account_creation_date(value: i64) -> i64 {
    if value > 0 {
        value
    } else {
        now_unix_timestamp()
    }
}

fn normalize_account_id(value: &str) -> String {
    let id = value.trim().to_lowercase();
    if id.is_empty() {
        return generate_account_id();
    }
    if id.len() == 8 && id.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return id;
    }
    let filtered = id
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .take(8)
        .collect::<String>();
    if filtered.len() == 8 {
        filtered
    } else {
        generate_account_id()
    }
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub nodes: Vec<ProxyNode>,
    pub accounts: Vec<AccountInfo>,
    #[serde(default = "default_groups")]
    pub groups: Vec<String>,
    #[serde(default)]
    pub registries: Vec<RegistryInfo>,
}

impl State {
    pub fn load() -> Self {
        storage::load_state().normalized_on_load()
    }

    pub fn save(&self) {
        storage::save_state(self);
    }

    pub fn sanitized_for_storage(&self) -> Self {
        let mut cloned = self.clone();
        cloned.groups = normalize_groups(&cloned.groups);
        for node in &mut cloned.nodes {
            node.groups = normalize_groups(&node.groups);
            node.config = node.config.sanitized_for_storage();
            for revision in &mut node.revisions {
                revision.config = revision.config.sanitized_for_storage();
            }
        }
        for account in &mut cloned.accounts {
            account.id = normalize_account_id(&account.id);
            account.token = normalize_account_token(&account.token);
            account.groups = normalize_groups(&account.groups);
            account.creation_date = normalize_account_creation_date(account.creation_date);
        }
        cloned
    }

    pub fn normalized_on_load(&self) -> Self {
        self.sanitized_for_storage()
    }
}

fn default_groups() -> Vec<String> {
    vec!["default".to_string()]
}

pub fn default_link_remark_template() -> String {
    "{node}-{inbound}-{user}".to_string()
}

pub fn effective_link_remark_template(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        default_link_remark_template()
    } else {
        value.to_string()
    }
}

pub fn format_link_remark(
    template: &str,
    node_name: &str,
    inbound_name: &str,
    user_name: &str,
) -> String {
    let rendered = effective_link_remark_template(template)
        .replace("{node}", node_name.trim())
        .replace("{inbound}", inbound_name.trim())
        .replace("{user}", user_name.trim());
    rendered.trim().to_string()
}

pub fn normalize_groups(values: &[String]) -> Vec<String> {
    let mut groups = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    groups.sort();
    groups.dedup();
    groups
}

pub fn effective_inbound_groups(node_groups: &[String], inbound_groups: &[String]) -> Vec<String> {
    let node_groups = normalize_groups(node_groups);
    let inbound_groups = normalize_groups(inbound_groups);
    if inbound_groups.is_empty() {
        return node_groups;
    }
    inbound_groups
        .into_iter()
        .filter(|group| node_groups.iter().any(|candidate| candidate == group))
        .collect()
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
    #[serde(default = "default_groups")]
    pub groups: Vec<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reverse_proxies: Vec<ReverseProxyDraft>,
    #[serde(default)]
    pub outbounds: Vec<OutboundEntryDraft>,
    #[serde(default)]
    pub certificates: Vec<CertificateDraft>,
    pub master_key: String,
    #[serde(default)]
    pub routing_rules: Vec<RoutingRuleDraft>,
    #[serde(default)]
    pub dns: DnsDraft,
    #[serde(default = "default_link_remark_template")]
    pub link_remark_template: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub warp_registration: WarpRegistrationDraft,
}

impl NodeConfigDraft {
    pub fn sanitized_for_storage(&self) -> Self {
        let mut cloned = self.clone();
        for inbound in &mut cloned.inbounds {
            sanitize_inbound_for_storage(inbound);
        }
        for reverse_proxy in &mut cloned.reverse_proxies {
            sanitize_reverse_proxy_for_storage(reverse_proxy);
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
    #[serde(default)]
    pub expiry_time: i64,
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
    #[serde(default, skip_serializing_if = "is_default")]
    pub shadowsocks: ShadowsocksDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub trojan: TrojanDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub custom: CustomOutboundDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub usque_masque: UsqueMasqueOutboundDraft,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboundEntryDraft {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
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
    #[serde(default, skip_serializing_if = "is_default")]
    pub shadowsocks: ShadowsocksDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub reverse_proxy: ReverseProxyDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub tunnel: TunnelDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub tproxy: TProxyDraft,
    #[serde(default, skip_serializing_if = "is_default")]
    pub trojan: TrojanDraft,
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

fn default_routing_rule_enabled() -> bool {
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
    #[serde(default = "default_trusttunnel_link_type")]
    pub link_type: String,
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
            link_type: default_trusttunnel_link_type(),
            http1_upload_buffer_size: default_http1_upload_buffer_size(),
            http2_initial_connection_window_size: default_http2_initial_connection_window_size(),
            http2_initial_stream_window_size: default_http2_initial_stream_window_size(),
            http2_max_concurrent_streams: default_http2_max_concurrent_streams(),
            http2_max_frame_size: default_http2_max_frame_size(),
            http2_header_table_size: default_http2_header_table_size(),
        }
    }
}

fn default_trusttunnel_link_type() -> String {
    "DeepLink".to_string()
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
    pub network: String,
    pub quic_congestion_control: String,
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tls_server_name: String,
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

fn default_vless_transmission() -> String {
    "TCP".to_string()
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustTunnelOutboundDraft {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint_hostname: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint_addresses: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub username: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub certificate_pem: String,
    #[serde(default)]
    pub skip_verification: bool,
    #[serde(default = "default_trusttunnel_upstream_protocol")]
    pub upstream_protocol: String,
    #[serde(default)]
    pub anti_dpi: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub custom_sni: String,
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
            endpoint_hostname: String::new(),
            endpoint_addresses: String::new(),
            username: String::new(),
            password: String::new(),
            certificate_pem: String::new(),
            skip_verification: false,
            upstream_protocol: default_trusttunnel_upstream_protocol(),
            anti_dpi: false,
            custom_sni: String::new(),
            http1_upload_buffer_size: default_http1_upload_buffer_size(),
            http2_initial_connection_window_size: default_http2_initial_connection_window_size(),
            http2_initial_stream_window_size: default_http2_initial_stream_window_size(),
            http2_max_concurrent_streams: default_http2_max_concurrent_streams(),
            http2_max_frame_size: default_http2_max_frame_size(),
            http2_header_table_size: default_http2_header_table_size(),
        }
    }
}

fn default_trusttunnel_upstream_protocol() -> String {
    "http2".to_string()
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

fn deserialize_wireguard_peers<'de, D>(deserializer: D) -> Result<Vec<WireGuardPeerItem>, D::Error>
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
pub struct ShadowsocksDraft {
    pub tag: String,
    pub server: String,
    pub port: i32,
    pub method: String,
    pub password: String,
    pub plugin: String,
    pub plugin_opts: String,
    pub prefix: String,
    pub udp_enabled: bool,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrojanDraft {
    pub tag: String,
    pub server: String,
    pub port: i32,
    pub password: String,
    pub fallback: String,
    pub tls_enabled: bool,
    pub sni: String,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomOutboundDraft {
    pub tag: String,
    pub handler_name: String,
    pub config_json: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct UsqueMasqueOutboundDraft {
    pub tag: String,
    #[serde(default = "default_usque_masque_http_version")]
    pub http_version: String,
    #[serde(default = "default_usque_masque_sni")]
    pub sni: String,
    #[serde(default = "default_usque_masque_connect_uri")]
    pub connect_uri: String,
    pub endpoint: String,
    #[serde(default = "default_usque_masque_endpoint_port")]
    pub endpoint_v4_port: i32,
    pub endpoint_pub_key: String,
    pub private_key: String,
    pub ipv4: String,
    pub ipv6: String,
    #[serde(default = "default_usque_masque_mtu")]
    pub mtu: i32,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub license: String,
}

impl Default for UsqueMasqueOutboundDraft {
    fn default() -> Self {
        Self {
            tag: String::new(),
            http_version: default_usque_masque_http_version(),
            sni: default_usque_masque_sni(),
            connect_uri: default_usque_masque_connect_uri(),
            endpoint: String::new(),
            endpoint_v4_port: default_usque_masque_endpoint_port(),
            endpoint_pub_key: String::new(),
            private_key: String::new(),
            ipv4: String::new(),
            ipv6: String::new(),
            mtu: default_usque_masque_mtu(),
            insecure: false,
            access_token: String::new(),
            id: String::new(),
            license: String::new(),
        }
    }
}

fn default_usque_masque_http_version() -> String {
    "HTTP/3".to_string()
}

fn default_usque_masque_sni() -> String {
    "consumer-masque.cloudflareclient.com".to_string()
}

fn default_usque_masque_connect_uri() -> String {
    "https://cloudflareaccess.com".to_string()
}

fn default_usque_masque_endpoint_port() -> i32 {
    443
}

fn default_usque_masque_mtu() -> i32 {
    1280
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReverseProxyDraft {
    #[serde(default = "default_inbound_enabled")]
    pub enabled: bool,
    pub mode: String,
    pub tag: String,
    pub domain: String,
    pub bridge_outbound_tag: String,
    pub target_outbound_tag: String,
    pub portal_inbound_tag: String,
    pub portal_user_id: String,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct TunnelDraft {
    pub allowed_network: String,
}

fn sanitize_reverse_proxy_for_storage(reverse_proxy: &mut ReverseProxyDraft) {
    reverse_proxy.mode = reverse_proxy.mode.trim().to_lowercase();
    if reverse_proxy.mode != "bridge" && reverse_proxy.mode != "portal" {
        reverse_proxy.mode = "portal".to_string();
    }
    reverse_proxy.tag = reverse_proxy.tag.trim().to_string();
    reverse_proxy.domain = reverse_proxy.domain.trim().to_string();
    reverse_proxy.bridge_outbound_tag = reverse_proxy.bridge_outbound_tag.trim().to_string();
    reverse_proxy.target_outbound_tag = reverse_proxy.target_outbound_tag.trim().to_string();
    reverse_proxy.portal_inbound_tag = reverse_proxy.portal_inbound_tag.trim().to_string();
    reverse_proxy.portal_user_id = reverse_proxy.portal_user_id.trim().to_string();
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct TProxyDraft {
    pub network: String,
    pub sniffing_enabled: bool,
    pub sniffing_dest_override: String,
    pub sniffing_route_only: bool,
    pub socket_mark: i32,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingRuleDraft {
    #[serde(default = "default_routing_rule_enabled")]
    pub enabled: bool,
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
    #[serde(alias = "access_id")]
    pub token: String,
    pub allowed_ips: Vec<String>,
    #[serde(default = "default_groups")]
    pub groups: Vec<String>,
    #[serde(default)]
    pub creation_date: i64,
    pub expiry_date: i64,
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub public_endpoint: String,
    #[serde(default)]
    pub manage_endpoint: String,
    #[serde(default)]
    pub master_key: String,
    #[serde(default)]
    pub enabled: bool,
}

fn sanitize_inbound_for_storage(inbound: &mut InboundEntryDraft) {
    inbound.groups = normalize_groups(&inbound.groups);
    if inbound.protocol.trim().eq_ignore_ascii_case("TUNNEL") {
        let allowed_network = inbound.tunnel.allowed_network.trim();
        inbound.tunnel.allowed_network = match allowed_network {
            "udp" => "udp".to_string(),
            "tcp,udp" => "tcp,udp".to_string(),
            _ => "tcp".to_string(),
        };
    } else {
        inbound.tunnel = TunnelDraft::default();
    }
    match inbound.protocol.trim().to_uppercase().as_str() {
        "VLESS" => {
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.tproxy = TProxyDraft::default();
        }
        "HYSTERIA2" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.tproxy = TProxyDraft::default();
        }
        "TRUSTTUNNEL" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.tproxy = TProxyDraft::default();
        }
        "NAIVEPROXY" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.tproxy = TProxyDraft::default();
        }
        "WIREGUARD" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.tproxy = TProxyDraft::default();
        }
        "SOCKS5" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.tproxy = TProxyDraft::default();
        }
        "SHADOWSOCKS" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.tproxy = TProxyDraft::default();
        }
        "REVERSEPROXY" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.tproxy = TProxyDraft::default();
        }
        "TPROXY" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.trojan = TrojanDraft::default();
        }
        "TUNNEL" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.tproxy = TProxyDraft::default();
            inbound.trojan = TrojanDraft::default();
        }
        "TROJAN" => {
            inbound.vless = VlessInboundDraft::default();
            inbound.hysteria2 = Hysteria2Draft::default();
            inbound.trust_tunnel = TrustTunnelDraft::default();
            inbound.naive_proxy = NaiveProxyDraft::default();
            inbound.wireguard = WireGuardDraft::default();
            inbound.socks5 = Socks5Draft::default();
            inbound.shadowsocks = ShadowsocksDraft::default();
            inbound.reverse_proxy = ReverseProxyDraft::default();
            inbound.tproxy = TProxyDraft::default();
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
            outbound.shadowsocks = ShadowsocksDraft::default();
        }
        "TRUSTTUNNEL" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.wireguard = WireGuardDraft::default();
            outbound.socks5 = Socks5Draft::default();
            outbound.shadowsocks = ShadowsocksDraft::default();
        }
        "WIREGUARD" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.trust_tunnel = TrustTunnelOutboundDraft::default();
            outbound.socks5 = Socks5Draft::default();
            outbound.shadowsocks = ShadowsocksDraft::default();
        }
        "SOCKS5" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.trust_tunnel = TrustTunnelOutboundDraft::default();
            outbound.wireguard = WireGuardDraft::default();
            outbound.shadowsocks = ShadowsocksDraft::default();
        }
        "SHADOWSOCKS" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.trust_tunnel = TrustTunnelOutboundDraft::default();
            outbound.wireguard = WireGuardDraft::default();
            outbound.socks5 = Socks5Draft::default();
            outbound.trojan = TrojanDraft::default();
        }
        "TROJAN" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.trust_tunnel = TrustTunnelOutboundDraft::default();
            outbound.wireguard = WireGuardDraft::default();
            outbound.socks5 = Socks5Draft::default();
            outbound.shadowsocks = ShadowsocksDraft::default();
        }
        "DIRECT" | "BLOCK" => {
            outbound.vless = VlessOutboundDraft::default();
            outbound.trust_tunnel = TrustTunnelOutboundDraft::default();
            outbound.wireguard = WireGuardDraft::default();
            outbound.socks5 = Socks5Draft::default();
            outbound.shadowsocks = ShadowsocksDraft::default();
            outbound.trojan = TrojanDraft::default();
        }
        _ => {}
    }
}
