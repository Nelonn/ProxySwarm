use super::*;

pub(super) fn today_string() -> String {
    let now = JsDate::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        now.get_full_year(),
        now.get_month() + 1,
        now.get_date()
    )
}

pub(super) fn random_port() -> i32 {
    20000 + (js_sys::Math::random() * 40000.0).floor() as i32
}

pub(super) fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if getrandom::getrandom(&mut buf).is_err() {
        for b in &mut buf {
            *b = (js_sys::Math::random() * 256.0).floor() as u8;
        }
    }
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

pub(super) fn generate_reality_short_id() -> String {
    random_hex(8)
}

pub(super) fn generate_reality_short_ids_batch(count: usize) -> Vec<String> {
    (0..count).map(|_| generate_reality_short_id()).collect()
}

pub(super) fn generate_reality_keypair() -> (String, String) {
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

pub(super) fn default_certificate_draft() -> CertificateDraft {
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

pub(super) fn default_inbound_entry() -> InboundEntryDraft {
    InboundEntryDraft {
        id: uuid::Uuid::new_v4().to_string(),
        name: "main".to_string(),
        groups: Vec::new(),
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
            network: String::new(),
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
        reverse_proxy: ReverseProxyDraft {
            enabled: true,
            mode: "portal".to_string(),
            tag: "portal".to_string(),
            target_outbound_tag: "direct".to_string(),
            ..ReverseProxyDraft::default()
        },
        tunnel: TunnelDraft {
            allowed_network: "tcp".to_string(),
        },
        tproxy: TProxyDraft {
            network: "tcp,udp".to_string(),
            sniffing_enabled: true,
            sniffing_dest_override: "http, tls, quic".to_string(),
            ..TProxyDraft::default()
        },
        trojan: TrojanDraft::default(),
    }
}

pub(super) fn default_node_draft(node: &ProxyNode) -> NodeConfigDraft {
    NodeConfigDraft {
        reverse_proxies: vec![],
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

pub(super) fn default_reverse_proxy_entry() -> ReverseProxyDraft {
    ReverseProxyDraft {
        enabled: true,
        mode: "portal".to_string(),
        tag: "r-outbound".to_string(),
        ..ReverseProxyDraft::default()
    }
}

pub(super) fn reverse_proxy_display_name(
    reverse_proxy: &ReverseProxyDraft,
    index: usize,
) -> String {
    let tag = reverse_proxy.tag.trim();
    if tag.is_empty() {
        format!("VLESS Reverse #{}", index + 1)
    } else {
        tag.to_string()
    }
}

pub(super) fn inbound_display_name(inbound: &InboundEntryDraft) -> String {
    let name = inbound.name.trim();
    if name.is_empty() {
        inbound.protocol.trim().to_uppercase()
    } else {
        name.to_string()
    }
}

pub(super) fn account_display_name(account: &AccountInfo) -> String {
    let name = account.name.trim();
    if name.is_empty() {
        account.id.trim().to_string()
    } else {
        name.to_string()
    }
}

pub(super) fn rendered_link_remark(
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

pub(super) fn certificate_display_name(certificate: &CertificateDraft) -> String {
    if certificate.name.trim().is_empty() {
        "Unnamed certificate".to_string()
    } else {
        certificate.name.trim().to_string()
    }
}

pub(super) fn normalized_certificates(draft: &NodeConfigDraft) -> Vec<CertificateDraft> {
    draft.certificates.clone()
}

pub(super) fn certificate_name_from_reference(
    certificates: &[CertificateDraft],
    reference: &str,
) -> Option<String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }
    certificates
        .iter()
        .find(|certificate| {
            certificate.name.trim() == reference || certificate.id.trim() == reference
        })
        .map(|certificate| certificate.name.trim().to_string())
}

pub(super) fn certificate_by_name<'a>(
    certificates: &'a [CertificateDraft],
    name: &str,
) -> Option<&'a CertificateDraft> {
    certificates
        .iter()
        .find(|certificate| certificate.name.trim() == name.trim())
}

pub(super) fn inbound_tls_enabled(protocol: &str, inbound: &InboundEntryDraft) -> bool {
    match protocol.trim().to_uppercase().as_str() {
        "HYSTERIA2" | "TRUSTTUNNEL" | "NAIVEPROXY" | "TROJAN" => true,
        "VLESS" => inbound.vless.security.eq_ignore_ascii_case("TLS"),
        _ => inbound.tls.enabled,
    }
}

pub(super) fn acme_ca_directory_url(value: &str) -> &'static str {
    match value.trim().to_lowercase().as_str() {
        "zerossl" => "https://acme.zerossl.com/v2/DV90",
        "google" => "https://dv.acme-v02.api.pki.goog/directory",
        "buypass" => "https://api.buypass.com/acme/directory",
        "sslcom" => "https://acme.ssl.com/sslcom-dv-rsa",
        _ => "https://acme-v02.api.letsencrypt.org/directory",
    }
}

pub(super) fn certmagic_storage_component(value: &str) -> String {
    let mut value = value.trim().to_string();
    if let Some(stripped) = value.strip_prefix("http://") {
        value = stripped.to_string();
    } else if let Some(stripped) = value.strip_prefix("https://") {
        value = stripped.to_string();
    }
    value = value.replace('/', "-").replace('\\', "-");
    value.trim_matches('-').to_string()
}

pub(super) fn full_config_to_pretty_json(config: &FullConfig) -> String {
    serde_json::to_string_pretty(config).unwrap_or_else(|error| {
        format!(
            "{{\"error\":\"failed to serialize config\",\"details\":{}}}",
            serde_json::to_string(&error.to_string()).unwrap_or_else(|_| "\"unknown\"".to_string())
        )
    })
}

pub(super) fn certmagic_certificate_paths(ca: &str, domain: &str) -> (String, String) {
    let issuer = certmagic_storage_component(acme_ca_directory_url(ca));
    let safe_domain = certmagic_storage_component(domain);
    let base = format!("data/acme_storage/certificates/{}/{}", issuer, safe_domain);
    (
        format!("{}/{}.crt", base, safe_domain),
        format!("{}/{}.key", base, safe_domain),
    )
}

pub(super) fn default_routing_rule_entry() -> RoutingRuleDraft {
    RoutingRuleDraft {
        enabled: true,
        outbound_tag: "direct".to_string(),
        ..RoutingRuleDraft::default()
    }
}

pub(super) fn default_builtin_outbound(tag: &str, outbound_type: &str) -> OutboundEntryDraft {
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
        trojan: TrojanDraft {
            tag: tag.to_string(),
            ..TrojanDraft::default()
        },
        custom: CustomOutboundDraft {
            tag: tag.to_string(),
            ..CustomOutboundDraft::default()
        },
    }
}

pub(super) fn inbound_groups_label(inbound: &InboundEntryDraft) -> String {
    let groups = normalize_groups(&inbound.groups);
    if groups.is_empty() {
        "Inherits node groups".to_string()
    } else {
        groups.join(", ")
    }
}

pub(super) fn default_vless_outbound() -> OutboundEntryDraft {
    OutboundEntryDraft {
        id: uuid::Uuid::new_v4().to_string(),
        name: "VLESS".to_string(),
        outbound_type: "VLESS".to_string(),
        enabled: true,
        builtin: false,
        vless: VlessOutboundDraft {
            tag: "VLESS".to_string(),
            security: "NONE".to_string(),
            ..VlessOutboundDraft::default()
        },
        trust_tunnel: TrustTunnelOutboundDraft::default(),
        wireguard: WireGuardDraft::default(),
        socks5: Socks5Draft::default(),
        shadowsocks: ShadowsocksDraft::default(),
        trojan: TrojanDraft::default(),
        custom: CustomOutboundDraft::default(),
    }
}

pub(super) fn vless_outbound_tag_from_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "VLESS".to_string()
    } else {
        name.to_string()
    }
}

pub(super) fn decode_link_component(value: &str) -> String {
    js_sys::decode_uri_component(value)
        .ok()
        .and_then(|decoded| decoded.as_string())
        .unwrap_or_else(|| value.to_string())
}

pub(super) fn import_vless_outbound_link(
    link: &str,
    existing: &OutboundEntryDraft,
) -> Result<OutboundEntryDraft, String> {
    let link = link.trim();
    let Some(raw) = link.strip_prefix("vless://") else {
        return Err("Link must start with vless://".to_string());
    };

    let without_fragment = match raw.split_once('#') {
        Some((base, _)) => base,
        None => raw,
    };
    let (authority, query) = match without_fragment.split_once('?') {
        Some((authority, query)) => (authority, query),
        None => (without_fragment, ""),
    };
    let Some((user, host_port)) = authority.rsplit_once('@') else {
        return Err("Link must include UUID and server".to_string());
    };

    let uuid = decode_link_component(user).trim().to_string();
    if uuid.is_empty() {
        return Err("VLESS UUID is empty".to_string());
    }

    let (server, port) = if let Some(rest) = host_port.strip_prefix('[') {
        let Some((host, tail)) = rest.split_once(']') else {
            return Err("Invalid IPv6 server syntax".to_string());
        };
        let Some(port_str) = tail.strip_prefix(':') else {
            return Err("VLESS link is missing port".to_string());
        };
        (
            host.trim().to_string(),
            port_str
                .trim()
                .parse::<i32>()
                .map_err(|_| "Invalid VLESS port".to_string())?,
        )
    } else {
        let Some((host, port_str)) = host_port.rsplit_once(':') else {
            return Err("VLESS link is missing port".to_string());
        };
        (
            host.trim().to_string(),
            port_str
                .trim()
                .parse::<i32>()
                .map_err(|_| "Invalid VLESS port".to_string())?,
        )
    };

    if server.is_empty() {
        return Err("VLESS server is empty".to_string());
    }

    let mut imported = existing.clone();
    imported.outbound_type = "VLESS".to_string();
    imported.enabled = true;
    imported.builtin = false;
    imported.vless.server = server;
    imported.vless.port = port;
    imported.vless.uuid = uuid;

    for pair in query.split('&').filter(|pair| !pair.trim().is_empty()) {
        let (key, value) = match pair.split_once('=') {
            Some((key, value)) => (key.trim().to_lowercase(), decode_link_component(value)),
            None => (pair.trim().to_lowercase(), String::new()),
        };
        let value = value.trim().to_string();
        match key.as_str() {
            "type" => imported.vless.transmission = vless_transmission_from(&value),
            "flow" => imported.vless.flow = value,
            "security" => {
                imported.vless.security = match value.trim().to_lowercase().as_str() {
                    "tls" => "TLS".to_string(),
                    "reality" => "REALITY".to_string(),
                    _ => "NONE".to_string(),
                }
            }
            "sni" | "servername" => {
                imported.vless.reality_sni = value.clone();
                imported.vless.tls_server_name = value;
            }
            "pbk" | "publickey" => imported.vless.reality_public_key = value,
            "sid" | "shortid" => imported.vless.reality_short_ids = value,
            "fp" | "fingerprint" => imported.vless.reality_utls = value,
            "spx" => imported.vless.reality_spider_x = value,
            _ => {}
        }
    }

    if imported.vless.security.trim().is_empty() {
        imported.vless.security = "NONE".to_string();
    }
    if imported.vless.transmission.trim().is_empty() {
        imported.vless.transmission = "TCP".to_string();
    }

    if imported.vless.tag.trim().is_empty() {
        imported.vless.tag = vless_outbound_tag_from_name(&imported.name);
    }

    Ok(imported)
}

pub(super) fn default_warp_outbound() -> OutboundEntryDraft {
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
        trojan: TrojanDraft::default(),
        custom: CustomOutboundDraft::default(),
    }
}

pub(super) fn default_shadowsocks_outbound() -> OutboundEntryDraft {
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
        trojan: TrojanDraft::default(),
        custom: CustomOutboundDraft::default(),
    }
}

pub(super) fn warp_registration_from_outbounds(
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

pub(super) fn warp_registration_from_draft(
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

pub(super) fn warp_registration_to_draft(
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

pub(super) fn initial_warp_registration(
    draft: &NodeConfigDraft,
) -> Option<crate::services::warp::WarpRegistration> {
    warp_registration_from_draft(&draft.warp_registration)
        .or_else(|| warp_registration_from_outbounds(&draft.outbounds))
}

pub(super) fn revision_label(index: usize, revision: &NodeConfigRevision) -> String {
    let date = revision.created_at.trim();
    if date.is_empty() {
        format!("Revision {}", index + 1)
    } else {
        format!("Revision {} ({})", index + 1, date)
    }
}

pub(super) fn sync_draft(draft: &mut NodeConfigDraft) {
    let mut migrated_reverse_proxies = Vec::new();
    draft.inbounds.retain(|inbound| {
        if !inbound.protocol.trim().eq_ignore_ascii_case("REVERSEPROXY") {
            return true;
        }
        let mut reverse_proxy = inbound.reverse_proxy.clone();
        reverse_proxy.enabled = inbound.enabled;
        if reverse_proxy.tag.trim().is_empty() {
            reverse_proxy.tag = inbound.name.trim().to_string();
        }
        if reverse_proxy.mode.trim().is_empty() {
            reverse_proxy.mode = "portal".to_string();
        }
        if reverse_proxy.target_outbound_tag.trim().is_empty() {
            reverse_proxy.target_outbound_tag = "direct".to_string();
        }
        migrated_reverse_proxies.push(reverse_proxy);
        false
    });
    draft.reverse_proxies.extend(migrated_reverse_proxies);

    if draft.outbounds.is_empty() {
        draft
            .outbounds
            .push(default_builtin_outbound("direct", "DIRECT"));
        draft
            .outbounds
            .push(default_builtin_outbound("block", "BLOCK"));
    }

    let certificates = draft.certificates.clone();

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
        inbound.groups = normalize_groups(&inbound.groups);
        if inbound.wireguard.domain_strategy.trim().is_empty() {
            inbound.wireguard.domain_strategy = "ForceIP".to_string();
        }
        if let Some(certificate_name) =
            certificate_name_from_reference(&certificates, &inbound.tls.certificate_name)
        {
            inbound.tls.certificate_name = certificate_name;
        }
    }

    for reverse_proxy in draft.reverse_proxies.iter_mut() {
        reverse_proxy.mode = reverse_proxy.mode.trim().to_lowercase();
        if reverse_proxy.mode != "bridge" && reverse_proxy.mode != "portal" {
            reverse_proxy.mode = "portal".to_string();
        }
        reverse_proxy.tag = reverse_proxy.tag.trim().to_string();
        reverse_proxy.domain = reverse_proxy.domain.trim().to_string();
        reverse_proxy.bridge_outbound_tag = reverse_proxy.bridge_outbound_tag.trim().to_string();
        reverse_proxy.target_outbound_tag = if reverse_proxy.target_outbound_tag.trim().is_empty() {
            "direct".to_string()
        } else {
            reverse_proxy.target_outbound_tag.trim().to_string()
        };
        reverse_proxy.portal_inbound_tag = reverse_proxy.portal_inbound_tag.trim().to_string();
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
        } else if outbound.outbound_type.trim().eq_ignore_ascii_case("VLESS") {
            if outbound.name.trim().is_empty() {
                outbound.name = "VLESS".to_string();
            }
            outbound.vless.tag = vless_outbound_tag_from_name(&outbound.name);
        } else if outbound.outbound_type.trim().eq_ignore_ascii_case("CUSTOM")
            && outbound.custom.tag.trim().is_empty()
        {
            outbound.custom.tag = if outbound.name.trim().is_empty() {
                "custom".to_string()
            } else {
                outbound.name.trim().to_string()
            };
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

pub(super) fn normalized_inbounds(draft: &NodeConfigDraft) -> Vec<InboundEntryDraft> {
    let mut copy = draft.clone();
    sync_draft(&mut copy);
    copy.inbounds
}

pub(super) fn normalized_outbounds(draft: &NodeConfigDraft) -> Vec<OutboundEntryDraft> {
    let mut copy = draft.clone();
    sync_draft(&mut copy);
    copy.outbounds
}

pub(super) fn normalized_routing_rules(draft: &NodeConfigDraft) -> Vec<RoutingRuleDraft> {
    let mut copy = draft.clone();
    sync_draft(&mut copy);
    copy.routing_rules
}

pub(super) fn outbound_tag_for_routing(outbound: &OutboundEntryDraft) -> String {
    match outbound.outbound_type.trim().to_uppercase().as_str() {
        "DIRECT" | "BLOCK" => outbound.name.clone(),
        "TRUSTTUNNEL" => outbound.trust_tunnel.tag.clone(),
        "WIREGUARD" => outbound.name.clone(),
        "SOCKS5" => outbound.socks5.tag.clone(),
        "SHADOWSOCKS" => outbound.shadowsocks.tag.clone(),
        "CUSTOM" => outbound.custom.tag.clone(),
        _ => outbound.vless.tag.clone(),
    }
}

pub(super) fn outbound_label_for_routing(outbound: &OutboundEntryDraft) -> String {
    let tag = outbound_tag_for_routing(outbound);
    format!("{} ({})", tag, outbound.outbound_type)
}

pub(super) fn split_lines_csv(value: &str) -> Vec<String> {
    value
        .split(|c| c == ',' || c == '\n' || c == '\r')
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
        .collect()
}

pub(super) fn normalize_reality_short_ids(value: &str) -> Vec<String> {
    split_lines_csv(value)
        .into_iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .filter(|s| s.len() <= 16 && s.len() % 2 == 0)
        .filter(|s| s.chars().all(|c| c.is_ascii_hexdigit()))
        .collect()
}

pub(super) fn security_from(value: &str) -> i32 {
    match value.trim().to_uppercase().as_str() {
        "TLS" => SecurityMode::Tls as i32,
        "REALITY" => SecurityMode::Reality as i32,
        _ => SecurityMode::None as i32,
    }
}

pub(super) fn vless_transmission_from(value: &str) -> String {
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

pub(super) fn vless_link_type(value: &str) -> &'static str {
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

pub(super) fn shadowsocks_method_options() -> Vec<DropdownOption> {
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

pub(super) fn core_from(value: &str) -> i32 {
    match value.trim().to_uppercase().as_str() {
        "XRAY" => CoreType::Xray as i32,
        "SING_BOX" => CoreType::SingBox as i32,
        "TRUSTTUNNEL" => CoreType::SingBox as i32,
        _ => CoreType::SingBox as i32,
    }
}

pub(super) fn supported_protocol_values_for_core(core_type: &str) -> Vec<&'static str> {
    match core_type.trim().to_uppercase().as_str() {
        "XRAY" => vec![
            "VLESS",
            "HYSTERIA2",
            "WIREGUARD",
            "SOCKS5",
            "SHADOWSOCKS",
            "TPROXY",
            "TROJAN",
            "TUNNEL",
        ],
        "SING_BOX" => vec![
            "VLESS",
            "HYSTERIA2",
            "NAIVEPROXY",
            "SOCKS5",
            "SHADOWSOCKS",
            "TROJAN",
        ],
        "TRUSTTUNNEL" => vec!["TRUSTTUNNEL"],
        _ => vec![],
    }
}

pub(super) fn protocol_options_for_core(core_type: &str) -> Vec<DropdownOption> {
    supported_protocol_values_for_core(core_type)
        .into_iter()
        .map(|value| DropdownOption {
            value: value.to_string(),
            label: match value {
                "HYSTERIA2" => "Hysteria2".to_string(),
                "NAIVEPROXY" => "NaiveProxy".to_string(),
                "WIREGUARD" => "WireGuard".to_string(),
                "SHADOWSOCKS" => "Shadowsocks".to_string(),
                "TRUSTTUNNEL" => "TrustTunnel".to_string(),
                "TPROXY" => "TProxy".to_string(),
                "TROJAN" => "Trojan".to_string(),
                "TUNNEL" => "Tunnel".to_string(),
                _ => value.to_string(),
            },
        })
        .collect()
}

pub(super) fn normalize_protocol_for_core(core_type: &str, protocol: &str) -> String {
    let protocol = protocol.trim().to_uppercase();
    let core_type = core_type.trim().to_uppercase();

    if core_type.is_empty() {
        return "".to_string();
    }
    if core_type == "TRUSTTUNNEL" {
        return "TRUSTTUNNEL".to_string();
    }
    let supported = supported_protocol_values_for_core(&core_type);
    if supported.iter().any(|candidate| *candidate == protocol) {
        return protocol;
    }
    supported
        .first()
        .map(|value| (*value).to_string())
        .unwrap_or_default()
}

pub(super) fn inbound_traffic_label(inbound: &InboundEntryDraft) -> String {
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
        "NAIVEPROXY" => match inbound.naive_proxy.network.trim() {
            "" => "TCP+UDP".to_string(),
            "tcp" => "TCP".to_string(),
            "udp" => "UDP".to_string(),
            other => other.to_string(),
        },
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
        "TUNNEL" => match inbound.tunnel.allowed_network.trim() {
            "udp" => "UDP".to_string(),
            "tcp,udp" => "TCP+UDP".to_string(),
            _ => "TCP".to_string(),
        },
        "TPROXY" => "TCP+UDP".to_string(),
        _ => "TCP".to_string(),
    }
}
