use trusttunnel_deeplink::{encode, DeepLinkConfig};

use crate::pb::proxyswarm::{Account, RegistryServiceConfig, RegistryTemplateLink};
use crate::services::link_builder::{build_vless_link, VlessLinkInput};
use crate::services::registry_api::RegistryApiService;
use crate::state::{
    effective_inbound_groups, format_link_remark, normalize_groups, AccountInfo, InboundEntryDraft,
    NodeConfigDraft, ProxyNode, RegistryInfo, State,
};
use crate::storage;

#[derive(Default, Clone)]
pub struct DeployAllSummary {
    pub registries_total: usize,
    pub registries_succeeded: usize,
    pub services_deployed: usize,
    pub services_deleted: usize,
    pub skipped_inbounds: usize,
    pub failures: Vec<String>,
}

#[derive(Default, Clone)]
pub struct AccountProxyLinksResult {
    pub links: Vec<AccountProxyLink>,
    pub skipped: Vec<String>,
}

#[derive(Clone, PartialEq)]
pub struct AccountProxyLink {
    pub link: String,
    pub node_country: String,
    pub node_name: String,
    pub inbound_name: String,
}

pub async fn deploy_all_registries(state: &State) -> DeployAllSummary {
    let enabled_registries: Vec<RegistryInfo> = state
        .registries
        .iter()
        .filter(|registry| registry.enabled)
        .cloned()
        .collect();
    let generated = build_registry_config(state);

    let mut summary = DeployAllSummary {
        registries_total: enabled_registries.len(),
        skipped_inbounds: generated.skipped_inbounds,
        failures: generated.failures,
        ..DeployAllSummary::default()
    };

    for registry in enabled_registries {
        match deploy_registry(&registry, generated.config.clone()).await {
            Ok(()) => {
                summary.registries_succeeded += 1;
                summary.services_deployed += 1;
            }
            Err(error) => summary
                .failures
                .push(format!("{}: {}", registry.name.trim(), error)),
        }
    }

    summary
}

pub async fn deploy_registry_by_id(
    state: &State,
    registry_id: &str,
) -> Result<DeployAllSummary, String> {
    let registry = state
        .registries
        .iter()
        .find(|registry| registry.id == registry_id)
        .cloned()
        .ok_or_else(|| "Registry not found".to_string())?;
    let generated = build_registry_config(state);

    let mut summary = DeployAllSummary {
        registries_total: 1,
        skipped_inbounds: generated.skipped_inbounds,
        failures: generated.failures,
        ..DeployAllSummary::default()
    };

    match deploy_registry(&registry, generated.config).await {
        Ok(()) => {
            summary.registries_succeeded = 1;
            summary.services_deployed = 1;
            Ok(summary)
        }
        Err(error) => {
            summary
                .failures
                .push(format!("{}: {}", registry.name.trim(), error));
            Err(summary.failures.join("; "))
        }
    }
}

pub fn collect_account_proxy_links(
    state: &State,
    account: &AccountInfo,
) -> AccountProxyLinksResult {
    let mut result = AccountProxyLinksResult::default();
    let mut seen = std::collections::HashSet::new();

    for node in &state.nodes {
        let node_config = effective_node_config(node);
        for inbound in &node_config.inbounds {
            if !inbound.enabled {
                continue;
            }
            let template_groups = effective_inbound_groups(&node.groups, &inbound.groups);
            if template_groups.is_empty() && !normalize_groups(&inbound.groups).is_empty() {
                continue;
            }
            if !groups_intersect(&account.groups, &template_groups) {
                continue;
            }

            let template = match build_template_link(node, &node_config, inbound) {
                Ok(template) => template,
                Err(error) => {
                    result.skipped.push(format!(
                        "Skipped {} / {}: {}",
                        node.name.trim(),
                        inbound_display_name(inbound),
                        error
                    ));
                    continue;
                }
            };
            let link = render_template_link(&template, account);
            let trimmed = link.trim();
            if trimmed.is_empty() {
                continue;
            }
            if seen.insert(trimmed.to_string()) {
                result.links.push(AccountProxyLink {
                    link: trimmed.to_string(),
                    node_country: node.country.trim().to_string(),
                    node_name: node.name.trim().to_string(),
                    inbound_name: inbound_display_name(inbound),
                });
            }
        }
    }

    result
}

#[derive(Default)]
struct BuildConfigResult {
    config: RegistryServiceConfig,
    skipped_inbounds: usize,
    failures: Vec<String>,
}

fn build_registry_config(state: &State) -> BuildConfigResult {
    let mut result = BuildConfigResult {
        config: RegistryServiceConfig {
            accounts: registry_accounts(&state.accounts),
            template_links: Vec::new(),
        },
        ..BuildConfigResult::default()
    };

    for node in &state.nodes {
        let node_config = effective_node_config(node);
        for inbound in &node_config.inbounds {
            if !inbound.enabled {
                continue;
            }
            let template_groups = effective_inbound_groups(&node.groups, &inbound.groups);
            if template_groups.is_empty() && !normalize_groups(&inbound.groups).is_empty() {
                result.skipped_inbounds += 1;
                result.failures.push(format!(
                    "Skipped {} / {}: inbound groups do not overlap node groups",
                    node.name.trim(),
                    inbound_display_name(inbound),
                ));
                continue;
            }

            match build_template_link(node, &node_config, inbound) {
                Ok(template) => {
                    result.config.template_links.push(RegistryTemplateLink {
                        node_id: node.id.clone(),
                        node_name: node.name.clone(),
                        inbound_id: inbound.id.clone(),
                        inbound_name: inbound.name.clone(),
                        protocol: inbound.protocol.trim().to_uppercase(),
                        template,
                        groups: template_groups,
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

fn effective_node_config(node: &ProxyNode) -> NodeConfigDraft {
    if let Some(draft) = storage::load_node_draft_local(&node.id) {
        return draft;
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
        return revision.config.clone();
    }
    NodeConfigDraft::default()
}

async fn deploy_registry(
    registry: &RegistryInfo,
    desired_config: RegistryServiceConfig,
) -> Result<(), String> {
    let api = RegistryApiService::new(
        registry.manage_endpoint.clone(),
        registry.master_key.clone(),
    );
    api.update_config(desired_config).await?;
    Ok(())
}

fn registry_accounts(accounts: &[AccountInfo]) -> Vec<Account> {
    accounts
        .iter()
        .map(|account| Account {
            id: account.id.clone(),
            token: account.token.clone(),
            expiry_time: account.expiry_date,
            allowed_ips: account.allowed_ips.clone(),
            groups: normalize_groups(&account.groups),
            name: account.name.clone(),
        })
        .collect()
}

fn inbound_display_name(inbound: &InboundEntryDraft) -> String {
    let name = inbound.name.trim();
    if name.is_empty() {
        inbound.protocol.trim().to_uppercase()
    } else {
        name.to_string()
    }
}

fn groups_intersect(account_groups: &[String], node_groups: &[String]) -> bool {
    let account_groups = normalize_groups(account_groups);
    let node_groups = normalize_groups(node_groups);
    account_groups
        .iter()
        .any(|group| node_groups.iter().any(|candidate| candidate == group))
}

fn render_template_link(template: &str, account: &AccountInfo) -> String {
    template
        .replace("{{token}}", &account.token)
        .replace("{{name}}", &account.name)
        .replace("{{id}}", &account.id)
        .replace("{token}", &account.token)
        .replace("{name}", &account.name)
        .replace("{id}", &account.id)
}

fn build_template_link(
    node: &ProxyNode,
    config: &crate::state::NodeConfigDraft,
    inbound: &InboundEntryDraft,
) -> Result<String, String> {
    match inbound.protocol.trim().to_uppercase().as_str() {
        "VLESS" => build_vless_template(node, config, inbound),
        "TRUSTTUNNEL" => build_trusttunnel_template(node, config, inbound),
        "HYSTERIA2" => build_hysteria2_template(node, config, inbound),
        "NAIVEPROXY" => build_naiveproxy_template(node, config, inbound),
        "SOCKS5" => build_socks5_template(node, config, inbound),
        protocol => Err(format!("unsupported protocol {}", protocol)),
    }
}

fn build_vless_template(
    node: &ProxyNode,
    config: &crate::state::NodeConfigDraft,
    inbound: &InboundEntryDraft,
) -> Result<String, String> {
    let host = normalized_public_ip_host(node)?;
    build_vless_link(VlessLinkInput {
        inbound,
        host: &host,
        user: "{{token}}",
        label_fragment: &template_label(node, config, inbound),
    })
}

fn build_trusttunnel_template(
    node: &ProxyNode,
    config: &crate::state::NodeConfigDraft,
    inbound: &InboundEntryDraft,
) -> Result<String, String> {
    let host = normalized_public_ip_host(node)?;
    let username = "{id}".to_string();
    let password = "{{token}}".to_string();
    let custom_sni = inbound.tls.server_name.trim().to_string();

    if inbound
        .trust_tunnel
        .link_type
        .trim()
        .eq_ignore_ascii_case("Simple")
    {
        let mut link_host = host.clone();
        if link_host.contains(':') && !link_host.starts_with('[') && !link_host.contains('.') {
            link_host = format!("[{}]", link_host);
        }
        let sni = if custom_sni.is_empty() {
            host.clone()
        } else {
            custom_sni
        };
        let query_string = vec![("security", "tls".to_string()), ("sni", sni)]
            .into_iter()
            .map(|(key, value)| format!("{}={}", key, js_sys::encode_uri_component(&value)))
            .collect::<Vec<_>>()
            .join("&");

        return Ok(format!(
            "tt://{}:{}@{}:{}?{}#{}",
            username,
            password,
            link_host,
            inbound.port,
            query_string,
            template_label(node, config, inbound)
        ));
    }

    let config = DeepLinkConfig::builder()
        .hostname(host.clone())
        .addresses(vec![format!("{}:{}", host, inbound.port)])
        .username(username)
        .password(password)
        .custom_sni((!custom_sni.is_empty() && custom_sni != host).then_some(custom_sni))
        .name(Some(template_label(node, config, inbound)))
        .build()
        .map_err(|error| format!("failed to build TrustTunnel template: {error}"))?;

    encode(&config).map_err(|error| format!("failed to encode TrustTunnel template: {error}"))
}

fn build_hysteria2_template(
    node: &ProxyNode,
    config: &crate::state::NodeConfigDraft,
    inbound: &InboundEntryDraft,
) -> Result<String, String> {
    let host = normalized_public_ip_host(node)?;
    let mut link_host = host.clone();
    if link_host.contains(':') && !link_host.starts_with('[') && !link_host.contains('.') {
        link_host = format!("[{}]", link_host);
    }
    let password = "{{token}}";

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
    if !inbound.hysteria2.obfs_type.trim().is_empty()
        && !inbound.hysteria2.obfs_password.trim().is_empty()
    {
        query.push((
            "obfs-password",
            inbound.hysteria2.obfs_password.trim().to_string(),
        ));
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
        password,
        link_host,
        inbound.port,
        suffix,
        template_label(node, config, inbound)
    ))
}

fn build_naiveproxy_template(
    node: &ProxyNode,
    config: &crate::state::NodeConfigDraft,
    inbound: &InboundEntryDraft,
) -> Result<String, String> {
    let host = normalized_public_ip_host(node)?;
    let username = "{id}";
    let password = "{{token}}";
    let sni = if inbound.tls.server_name.trim().is_empty() {
        host.trim_matches(&['[', ']'][..]).to_string()
    } else {
        inbound.tls.server_name.trim().to_string()
    };
    let query_string = vec![("security", "tls".to_string()), ("sni", sni)]
        .into_iter()
        .map(|(key, value)| format!("{}={}", key, js_sys::encode_uri_component(&value)))
        .collect::<Vec<_>>()
        .join("&");

    Ok(format!(
        "naive+https://{}:{}@{}:{}?{}#{}",
        js_sys::encode_uri_component(username),
        js_sys::encode_uri_component(password),
        host,
        inbound.port,
        query_string,
        template_label(node, config, inbound)
    ))
}

fn build_socks5_template(
    node: &ProxyNode,
    config: &crate::state::NodeConfigDraft,
    inbound: &InboundEntryDraft,
) -> Result<String, String> {
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
        template_label(node, config, inbound)
    ))
}

fn template_label(
    node: &ProxyNode,
    config: &crate::state::NodeConfigDraft,
    inbound: &InboundEntryDraft,
) -> String {
    let inbound_name = inbound_display_name(inbound);
    let encoded_node = js_sys::encode_uri_component(node.name.trim())
        .as_string()
        .unwrap_or_default();
    let encoded_inbound = js_sys::encode_uri_component(&inbound_name)
        .as_string()
        .unwrap_or_default();
    format_link_remark(
        &config.link_remark_template,
        &encoded_node,
        &encoded_inbound,
        "{{name}}",
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
    if let Some(stripped) = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
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
