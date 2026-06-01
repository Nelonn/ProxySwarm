use crate::state::InboundEntryDraft;

pub struct VlessLinkInput<'a> {
    pub inbound: &'a InboundEntryDraft,
    pub host: &'a str,
    pub user: &'a str,
    pub label_fragment: &'a str,
}

pub fn build_vless_link(input: VlessLinkInput<'_>) -> Result<String, String> {
    if input.inbound.protocol.trim().to_uppercase() != "VLESS" {
        return Err("Selected inbound is not VLESS".to_string());
    }

    let user = input.user.trim();
    if user.is_empty() {
        return Err("User token is empty".to_string());
    }

    let mut host = input.host.trim().to_string();
    if host.is_empty() {
        return Err("Node public IP address is empty".to_string());
    }
    if host.contains(':') && !host.starts_with('[') && !host.contains('.') {
        host = format!("[{}]", host);
    }

    let security = input.inbound.vless.security.trim().to_uppercase();
    let transmission = vless_link_type(&input.inbound.vless.transmission).to_string();
    let mut query: Vec<(String, String)> = vec![
        ("encryption".to_string(), "none".to_string()),
        ("type".to_string(), transmission.clone()),
    ];

    if !input.inbound.vless.flow.trim().is_empty() {
        query.push((
            "flow".to_string(),
            input.inbound.vless.flow.trim().to_string(),
        ));
    }

    let default_sni = if input.inbound.tls.server_name.trim().is_empty() {
        host.clone()
    } else {
        input.inbound.tls.server_name.trim().to_string()
    };

    match security.as_str() {
        "TLS" => {
            query.push(("security".to_string(), "tls".to_string()));
            query.push(("sni".to_string(), default_sni.clone()));
        }
        "REALITY" => {
            query.push(("security".to_string(), "reality".to_string()));
            let sni = if input.inbound.vless.reality_sni.trim().is_empty() {
                default_sni.clone()
            } else {
                input.inbound.vless.reality_sni.trim().to_string()
            };
            query.push(("sni".to_string(), sni));
            if !input.inbound.vless.reality_utls.trim().is_empty() {
                query.push((
                    "fp".to_string(),
                    input.inbound.vless.reality_utls.trim().to_string(),
                ));
            }
            if !input.inbound.vless.reality_public_key.trim().is_empty() {
                query.push((
                    "pbk".to_string(),
                    input.inbound.vless.reality_public_key.trim().to_string(),
                ));
            }
            if transmission != "xhttp" && !input.inbound.vless.reality_spider_x.trim().is_empty() {
                query.push((
                    "spx".to_string(),
                    input.inbound.vless.reality_spider_x.trim().to_string(),
                ));
            }
            if transmission == "xhttp" {
                if let Some(short_id) =
                    normalize_reality_short_ids(&input.inbound.vless.reality_short_ids).first()
                {
                    query.push(("sid".to_string(), short_id.clone()));
                }
            }
        }
        _ => {
            query.push(("security".to_string(), "none".to_string()));
        }
    }

    match transmission.as_str() {
        "xhttp" => {
            query.push(("mode".to_string(), "auto".to_string()));
            query.push(("extra".to_string(), vless_xhttp_extra().to_string()));
        }
        "ws" | "httpupgrade" | "splithttp" | "http" => {
            query.push(("path".to_string(), "/".to_string()));
            query.push(("host".to_string(), default_sni));
        }
        "grpc" => {
            query.push(("serviceName".to_string(), "".to_string()));
        }
        _ => {}
    }

    let query_string = query
        .into_iter()
        .map(|(key, value)| format!("{}={}", key, encode_uri_component(&value)))
        .collect::<Vec<_>>()
        .join("&");

    Ok(format!(
        "vless://{}@{}:{}?{}#{}",
        user, host, input.inbound.port, query_string, input.label_fragment
    ))
}

fn encode_uri_component(value: &str) -> String {
    js_sys::encode_uri_component(value)
        .as_string()
        .unwrap_or_default()
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
        "XHTTP" => "XHTTP".to_string(),
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
        "XHTTP" => "xhttp",
        _ => "tcp",
    }
}

fn vless_xhttp_extra() -> &'static str {
    r#"{"scMaxEachPostBytes":"1000000","scMinPostsIntervalMs":"30","xPaddingBytes":"100-1000","xmux":{"cMaxReuseTimes":"0","hMaxRequestTimes":"600-900","hMaxReusableSecs":"1800-3000","maxConcurrency":"16-32","maxConnections":"0"}}"#
}
