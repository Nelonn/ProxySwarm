use std::collections::{HashMap, HashSet};

use trusttunnel_deeplink::{encode, DeepLinkConfig};

use crate::pb::proxyswarm::{Account, RegistryService, RegistryTemplateLink};
use crate::services::registry_api::RegistryApiService;
use crate::state::{AccountInfo, InboundEntryDraft, ProxyNode, RegistryInfo, State};

const MANAGED_SERVICE_ID_PREFIX: &str = "ps-managed:";
const DEFAULT_REFRESH_INTERVAL_SECONDS: i32 = 3600;

#[derive(Default, Clone)]
pub struct DeployAllSummary {
    pub registries_total: usize,
    pub registries_succeeded: usize,
    pub services_deployed: usize,
    pub services_deleted: usize,
    pub skipped_inbounds: usize,
    pub failures: Vec<String>,
}

#[derive(Clone)]
struct DesiredService {
    id: String,
    name: String,
    accounts: Vec<Account>,
    template_links: Vec<RegistryTemplateLink>,
}

pub async fn deploy_all_registries(state: &State) -> DeployAllSummary {
    let enabled_registries: Vec<RegistryInfo> = state
        .registries
        .iter()
        .filter(|registry| registry.enabled)
        .cloned()
        .collect();
    let generated = build_desired_services(state);

    let mut summary = DeployAllSummary {
        registries_total: enabled_registries.len(),
        skipped_inbounds: generated.skipped_inbounds,
        failures: generated.failures,
        ..DeployAllSummary::default()
    };

    for registry in enabled_registries {
        match deploy_registry(&registry, &generated.services).await {
            Ok(result) => {
                summary.registries_succeeded += 1;
                summary.services_deployed += result.services_deployed;
                summary.services_deleted += result.services_deleted;
            }
            Err(error) => summary
                .failures
                .push(format!("{}: {}", registry.name.trim(), error)),
        }
    }

    summary
}

#[derive(Default)]
struct BuildServicesResult {
    services: Vec<DesiredService>,
    skipped_inbounds: usize,
    failures: Vec<String>,
}

fn build_desired_services(state: &State) -> BuildServicesResult {
    let mut result = BuildServicesResult::default();
    let accounts = registry_accounts(&state.accounts);

    for node in &state.nodes {
        for inbound in &node.config.inbounds {
            if !inbound.enabled {
                continue;
            }

            match build_template_link(node, inbound) {
                Ok(template) => {
                    result.services.push(DesiredService {
                        id: managed_service_id(node, inbound),
                        name: display_service_name(node, inbound),
                        accounts: accounts.clone(),
                        template_links: vec![RegistryTemplateLink {
                            node_id: node.id.clone(),
                            node_name: node.name.clone(),
                            inbound_id: inbound.id.clone(),
                            inbound_name: inbound.name.clone(),
                            protocol: inbound.protocol.trim().to_uppercase(),
                            template,
                        }],
                    });
                }
                Err(error) => {
                    result.skipped_inbounds += 1;
                    result.failures.push(format!(
                        "Skipped {} / {}: {}",
                        node.name.trim(),
                        inbound_display_name(inbound),
                        error
                    ));
                }
            }
        }
    }

    result
}

#[derive(Default)]
struct RegistryDeployResult {
    services_deployed: usize,
    services_deleted: usize,
}

async fn deploy_registry(
    registry: &RegistryInfo,
    desired_services: &[DesiredService],
) -> Result<RegistryDeployResult, String> {
    let api = RegistryApiService::new(
        registry.manage_endpoint.clone(),
        registry.master_key.clone(),
    );
    let existing_services = api.list_services().await?;
    let existing_by_id: HashMap<String, RegistryService> = existing_services
        .into_iter()
        .map(|service| (service.id.clone(), service))
        .collect();
    let desired_ids: HashSet<String> = desired_services.iter().map(|service| service.id.clone()).collect();
    let subscription_url = build_subscription_url(registry);

    let mut result = RegistryDeployResult::default();

    for desired in desired_services {
        let refresh_interval_seconds = existing_by_id
            .get(&desired.id)
            .map(|service| service.refresh_interval_seconds)
            .filter(|interval| *interval > 0)
            .unwrap_or(DEFAULT_REFRESH_INTERVAL_SECONDS);

        api.upsert_service(RegistryService {
            id: desired.id.clone(),
            name: desired.name.clone(),
            subscription_url: subscription_url.clone(),
            enabled: true,
            refresh_interval_seconds,
            updated_at_unix: 0,
            accounts: desired.accounts.clone(),
            template_links: desired.template_links.clone(),
        })
        .await?;
        result.services_deployed += 1;
    }

    for existing in existing_by_id.values() {
        if !existing.id.starts_with(MANAGED_SERVICE_ID_PREFIX) {
            continue;
        }
        if desired_ids.contains(&existing.id) {
            continue;
        }
        api.delete_service(existing.id.clone()).await?;
        result.services_deleted += 1;
    }

    Ok(result)
}

fn build_subscription_url(registry: &RegistryInfo) -> String {
    format!(
        "{}/v1/subscription",
        registry.public_endpoint.trim_end_matches('/')
    )
}

fn registry_accounts(accounts: &[AccountInfo]) -> Vec<Account> {
    accounts
        .iter()
        .map(|account| Account {
            id: account.id.clone(),
            name: account.name.clone(),
            token: account.token.clone(),
            expiry_time: account.expiry_date,
            allowed_ips: account.allowed_ips.clone(),
        })
        .collect()
}

fn managed_service_id(node: &ProxyNode, inbound: &InboundEntryDraft) -> String {
    format!("{}{}:{}", MANAGED_SERVICE_ID_PREFIX, node.id, inbound.id)
}

fn display_service_name(node: &ProxyNode, inbound: &InboundEntryDraft) -> String {
    format!("{} / {}", node.name.trim(), inbound_display_name(inbound))
}

fn inbound_display_name(inbound: &InboundEntryDraft) -> String {
    let name = inbound.name.trim();
    if name.is_empty() {
        inbound.protocol.trim().to_uppercase()
    } else {
        name.to_string()
    }
}

fn build_template_link(node: &ProxyNode, inbound: &InboundEntryDraft) -> Result<String, String> {
    match inbound.protocol.trim().to_uppercase().as_str() {
        "VLESS" => build_vless_template(node, inbound),
        "TRUSTTUNNEL" => build_trusttunnel_template(node, inbound),
        "HYSTERIA2" => build_hysteria2_template(node, inbound),
        "NAIVEPROXY" => build_naiveproxy_template(node, inbound),
        "SOCKS5" => build_socks5_template(node, inbound),
        protocol => Err(format!("unsupported protocol {}", protocol)),
    }
}

fn build_vless_template(node: &ProxyNode, inbound: &InboundEntryDraft) -> Result<String, String> {
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
            if let Some(first_sid) = normalize_reality_short_ids(&inbound.vless.reality_short_ids)
                .first()
                .cloned()
            {
                query.push(("sid".to_string(), first_sid));
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
        _ => query.push(("security".to_string(), "none".to_string())),
    }

    match transmission.as_str() {
        "ws" | "httpupgrade" | "splithttp" | "http" => {
            query.push(("path".to_string(), "/".to_string()));
            query.push(("host".to_string(), default_sni));
        }
        "grpc" => query.push(("serviceName".to_string(), "".to_string())),
        _ => {}
    }

    let query_string = query
        .into_iter()
        .map(|(key, value)| format!("{}={}", key, js_sys::encode_uri_component(&value)))
        .collect::<Vec<_>>()
        .join("&");

    Ok(format!(
        "vless://{{{{token}}}}@{}:{}?{}#{}",
        host,
        inbound.port,
        query_string,
        template_label(node)
    ))
}

fn build_trusttunnel_template(
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
) -> Result<String, String> {
    let host = normalized_node_host(node)?;
    let username = "{id}".to_string();
    let password = "{{token}}".to_string();
    let custom_sni = inbound.tls.server_name.trim().to_string();

    let config = DeepLinkConfig::builder()
        .hostname(host.clone())
        .addresses(vec![format!("{}:{}", host, inbound.port)])
        .username(username)
        .password(password)
        .custom_sni((!custom_sni.is_empty() && custom_sni != host).then_some(custom_sni))
        .name(Some(template_label(node)))
        .build()
        .map_err(|error| format!("failed to build TrustTunnel template: {error}"))?;

    encode(&config).map_err(|error| format!("failed to encode TrustTunnel template: {error}"))
}

fn build_hysteria2_template(node: &ProxyNode, inbound: &InboundEntryDraft) -> Result<String, String> {
    let host = normalized_public_ip_host(node)?;
    let password = inbound.hysteria2.password.trim();
    if password.is_empty() {
        return Err("Hysteria2 password is empty".to_string());
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
        query.push(("masquerade", inbound.hysteria2.masquerade.trim().to_string()));
    }

    let query_string = query
        .into_iter()
        .map(|(key, value)| format!("{}={}", key, js_sys::encode_uri_component(&value)))
        .collect::<Vec<_>>()
        .join("&");
    let suffix = if query_string.is_empty() {
        String::new()
    } else {
        format!("?{}", query_string)
    };

    Ok(format!(
        "hysteria2://{}@{}:{}{}#{}",
        js_sys::encode_uri_component(password),
        host,
        inbound.port,
        suffix,
        template_label(node)
    ))
}

fn build_naiveproxy_template(
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
) -> Result<String, String> {
    let host = normalized_node_host(node)?;
    let username = inbound.naive_proxy.username.trim();
    let password = inbound.naive_proxy.password.trim();
    if username.is_empty() || password.is_empty() {
        return Err("NaiveProxy username/password is empty".to_string());
    }

    Ok(format!(
        "naive+https://{}:{}@{}:{}#{}",
        js_sys::encode_uri_component(username),
        js_sys::encode_uri_component(password),
        host,
        inbound.port,
        template_label(node)
    ))
}

fn build_socks5_template(node: &ProxyNode, inbound: &InboundEntryDraft) -> Result<String, String> {
    let host = normalized_node_host(node)?;
    let username = inbound.socks5.username.trim();
    let password = inbound.socks5.password.trim();
    let auth = if username.is_empty() && password.is_empty() {
        String::new()
    } else {
        format!(
            "{}:{}@",
            js_sys::encode_uri_component(username),
            js_sys::encode_uri_component(password)
        )
    };

    Ok(format!(
        "socks://{}{}:{}#{}",
        auth,
        host,
        inbound.port,
        template_label(node)
    ))
}

fn template_label(node: &ProxyNode) -> String {
    format!(
        "{}-{{id}}",
        js_sys::encode_uri_component(node.name.trim())
    )
}

fn normalized_public_ip_host(node: &ProxyNode) -> Result<String, String> {
    normalize_host(&node.public_ip).ok_or_else(|| "Node public IP address is empty".to_string())
}

fn normalized_node_host(node: &ProxyNode) -> Result<String, String> {
    normalize_host(if !node.address.trim().is_empty() {
        &node.address
    } else {
        &node.public_ip
    })
    .ok_or_else(|| "Node access address/public IP is empty".to_string())
}

fn normalize_host(value: &str) -> Option<String> {
    let mut host = value.trim().to_string();
    if let Some(stripped) = host.strip_prefix("http://") {
        host = stripped.to_string();
    } else if let Some(stripped) = host.strip_prefix("https://") {
        host = stripped.to_string();
    }
    if let Some((base, _)) = host.split_once('/') {
        host = base.to_string();
    }
    if let Some(stripped) = host.strip_prefix('[').and_then(|value| value.strip_suffix(']')) {
        host = stripped.to_string();
    }
    if let Some((base, port)) = host.rsplit_once(':') {
        if (!base.contains(':') || base.contains('.')) && port.parse::<u16>().is_ok() {
            host = base.to_string();
        }
    }
    let host = host.trim().to_string();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn split_lines_csv(value: &str) -> Vec<String> {
    value
        .split(|char| char == ',' || char == '\n' || char == '\r')
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
        .collect()
}

fn normalize_reality_short_ids(value: &str) -> Vec<String> {
    split_lines_csv(value)
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .filter(|value| value.len() <= 16 && value.len() % 2 == 0)
        .filter(|value| value.chars().all(|char| char.is_ascii_hexdigit()))
        .collect()
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
