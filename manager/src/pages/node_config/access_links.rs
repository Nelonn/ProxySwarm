use super::*;

pub(super) fn build_vless_access_link(
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
        "ws" | "httpupgrade" | "splithttp" | "xhttp" | "http" => {
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

pub(super) fn normalized_public_ip_host(node: &ProxyNode) -> Result<String, String> {
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

pub(super) fn normalized_node_host(node: &ProxyNode) -> Result<String, String> {
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

pub(super) fn build_trusttunnel_access_link(
    draft: &NodeConfigDraft,
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
    account: &AccountInfo,
) -> Result<String, String> {
    if inbound.protocol.trim().to_uppercase() != "TRUSTTUNNEL" {
        return Err("Selected inbound is not TrustTunnel".to_string());
    }

    let host = normalized_node_host(node)?;
    let username = if !account.id.trim().is_empty() {
        account.id.trim().to_string()
    } else {
        account.name.trim().to_string()
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
            js_sys::encode_uri_component(&username),
            js_sys::encode_uri_component(&password),
            link_host,
            inbound.port,
            query_string,
            js_sys::encode_uri_component(&config_name)
        ));
    }

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

pub(super) fn build_hysteria2_access_link(
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

pub(super) fn build_naiveproxy_access_link(
    draft: &NodeConfigDraft,
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
    account: &AccountInfo,
) -> Result<String, String> {
    if inbound.protocol.trim().to_uppercase() != "NAIVEPROXY" {
        return Err("Selected inbound is not NaiveProxy".to_string());
    }

    let mut host = normalized_node_host(node)?;
    if host.contains(':') && !host.starts_with('[') && !host.contains('.') {
        host = format!("[{}]", host);
    }

    let username = account.id.trim();
    if username.is_empty() {
        return Err("User id is empty".to_string());
    }

    let password = if !account.token.trim().is_empty() {
        account.token.trim()
    } else {
        account.id.trim()
    };
    if password.is_empty() {
        return Err("User token is empty".to_string());
    }

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
        js_sys::encode_uri_component(&rendered_link_remark(draft, node, inbound, account))
    ))
}

pub(super) fn build_access_link(
    draft: &NodeConfigDraft,
    node: &ProxyNode,
    inbound: &InboundEntryDraft,
    account: &AccountInfo,
) -> Result<String, String> {
    match inbound.protocol.trim().to_uppercase().as_str() {
        "VLESS" => build_vless_access_link(draft, node, inbound, account),
        "TRUSTTUNNEL" => build_trusttunnel_access_link(draft, node, inbound, account),
        "HYSTERIA2" => build_hysteria2_access_link(draft, node, inbound, account),
        "NAIVEPROXY" => build_naiveproxy_access_link(draft, node, inbound, account),
        _ => Err(
            "Access link is available only for VLESS, Hysteria2, TrustTunnel, and NaiveProxy inbounds"
                .to_string(),
        ),
    }
}

pub(super) async fn copy_to_clipboard(text: String) -> Result<(), String> {
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

pub(super) fn qr_svg(value: &str) -> Option<String> {
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
