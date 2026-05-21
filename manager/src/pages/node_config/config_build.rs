use super::*;

pub(super) fn build_dns_server_config(server: &DnsServerDraft) -> Option<DnsServerConfig> {
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

pub(super) fn build_dns_host_mapping(host: &DnsHostDraft) -> Option<DnsHostMapping> {
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

pub(super) fn build_dns_config(draft: &NodeConfigDraft) -> Option<DnsConfig> {
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

pub(super) fn build_full_config(
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
            allowed_ips: account.allowed_ips.clone(),
            groups: normalize_groups(&account.groups),
            expiry_time: account.expiry_date,
            token: account.token.clone(),
        })
        .collect();
    let mut inbounds: Vec<InboundConfig> = normalized_inbounds(draft)
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
                enabled: inbound_tls_enabled(&normalized_protocol, &inbound),
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
                        network: inbound.naive_proxy.network.clone(),
                        quic_congestion_control: inbound
                            .naive_proxy
                            .quic_congestion_control
                            .clone(),
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
                "REVERSEPROXY" => Some(
                    crate::pb::proxyswarm::inbound_config::Protocol::Reverseproxy(
                        ReverseProxyConfig {
                            mode: inbound.reverse_proxy.mode.clone(),
                            tag: inbound.reverse_proxy.tag.clone(),
                            domain: inbound.reverse_proxy.domain.clone(),
                            bridge_outbound_tag: inbound.reverse_proxy.bridge_outbound_tag.clone(),
                            target_outbound_tag: inbound.reverse_proxy.target_outbound_tag.clone(),
                            portal_inbound_tag: inbound.reverse_proxy.portal_inbound_tag.clone(),
                        },
                    ),
                ),
                "TPROXY" => Some(crate::pb::proxyswarm::inbound_config::Protocol::Tproxy(
                    TProxyConfig {
                        network: inbound.tproxy.network.clone(),
                        sniffing_enabled: inbound.tproxy.sniffing_enabled,
                        sniffing_dest_override: split_lines_csv(&inbound.tproxy.sniffing_dest_override),
                        sniffing_route_only: inbound.tproxy.sniffing_route_only,
                        socket_mark: inbound.tproxy.socket_mark,
                    },
                )),
                "TROJAN" => Some(crate::pb::proxyswarm::inbound_config::Protocol::Trojan(
                    TrojanInboundConfig {
                        password: inbound.trojan.password.clone(),
                        tls: tls.clone(),
                        fallback: inbound.trojan.fallback.clone(),
                    },
                )),
                "TUNNEL" => Some(crate::pb::proxyswarm::inbound_config::Protocol::Tunnel(
                    TunnelConfig {},
                )),
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

    let normalized_reverse_proxies = {
        let mut copy = draft.clone();
        sync_draft(&mut copy);
        copy.reverse_proxies
    };
    for reverse_proxy in normalized_reverse_proxies {
        if !reverse_proxy.enabled {
            continue;
        }
        inbounds.push(InboundConfig {
            name: reverse_proxy.tag.clone(),
            listen: String::new(),
            port: 0,
            accounts: Vec::new(),
            enabled: true,
            core: CoreType::Xray as i32,
            protocol: Some(crate::pb::proxyswarm::inbound_config::Protocol::Reverseproxy(
                ReverseProxyConfig {
                    mode: reverse_proxy.mode,
                    tag: reverse_proxy.tag,
                    domain: reverse_proxy.domain,
                    bridge_outbound_tag: reverse_proxy.bridge_outbound_tag,
                    target_outbound_tag: reverse_proxy.target_outbound_tag,
                    portal_inbound_tag: reverse_proxy.portal_inbound_tag,
                },
            )),
        });
    }

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
            "TROJAN" if !outbound.trojan.tag.trim().is_empty() => {
                outbounds.push(OutboundConfig {
                    tag: outbound.trojan.tag.clone(),
                    r#type: OutboundType::Trojan as i32,
                    settings: Some(outbound_config::Settings::Trojan(
                        TrojanOutboundConfig {
                            server: outbound.trojan.server.clone(),
                            port: outbound.trojan.port,
                            password: outbound.trojan.password.clone(),
                            tls_enabled: outbound.trojan.tls_enabled,
                            sni: outbound.trojan.sni.clone(),
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
                name: certificate.name,
                kind: if certificate.cert_type.eq_ignore_ascii_case("ACME") {
                    Some(crate::pb::proxyswarm::certificate_config::Kind::Acme(
                        AcmeCertificateConfig {
                            acme_type: certificate.acme_type,
                            acme_ca: certificate.acme_ca,
                            acme_email: certificate.acme_email,
                            acme_domain: certificate.acme_domain,
                            acme_port: certificate.acme_port,
                            acme_http_port: certificate.acme_http_port,
                        },
                    ))
                } else {
                    Some(crate::pb::proxyswarm::certificate_config::Kind::Custom(
                        CustomCertificateConfig {
                            certificate_pem: certificate.certificate_pem,
                            key_pem: certificate.key_pem,
                        },
                    ))
                },
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


