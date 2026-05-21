use super::*;

#[derive(Properties, PartialEq)]
pub(super) struct InboundEditorPopupProps {
    pub(super) inbound: InboundEntryDraft,
    pub(super) certificates: Vec<CertificateDraft>,
    pub(super) is_new: bool,
    pub(super) on_close: Callback<()>,
    pub(super) on_save: Callback<InboundEntryDraft>,
}

pub(super) fn inbound_creation_steps(inbound: &InboundEntryDraft) -> usize {
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
        "NAIVEPROXY" => 4,
        "TROJAN" => 4,
        "WIREGUARD" => 3,
        _ => 3,
    }
}

#[function_component(InboundEditorPopup)]
pub(super) fn inbound_editor_popup(props: &InboundEditorPopupProps) -> Html {
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
                                    <TextBox
                                        label="Groups"
                                        value={data.groups.join(", ")}
                                        onchange={update_text(|inbound, value| inbound.groups = split_lines_csv(&value))}
                                        placeholder="Empty = inherit node groups"
                                    />
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
                                                    <Dropdown
                                                        label="Network"
                                                        value={data.naive_proxy.network.clone()}
                                                        options={vec![
                                                            DropdownOption { value: "".to_string(), label: "Both (TCP + UDP)".to_string() },
                                                            DropdownOption { value: "tcp".to_string(), label: "TCP".to_string() },
                                                            DropdownOption { value: "udp".to_string(), label: "UDP".to_string() },
                                                        ]}
                                                        onchange={update_text(|inbound, value| inbound.naive_proxy.network = value)}
                                                    />
                                                    <TextBox
                                                        label="QUIC congestion control"
                                                        value={data.naive_proxy.quic_congestion_control.clone()}
                                                        onchange={update_text(|inbound, value| inbound.naive_proxy.quic_congestion_control = value)}
                                                        placeholder="bbr / bbr2 / cubic / reno"
                                                    />
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
                                            "TROJAN" => html! {
                                                <ConfigSection title="Trojan">
                                                    <TextBox label="Default Password" value={data.trojan.password.clone()} onchange={update_text(|inbound, value| inbound.trojan.password = value)} placeholder="Fallback if account token is empty" />
                                                    <TextBox label="Fallback Target Address" value={data.trojan.fallback.clone()} onchange={update_text(|inbound, value| inbound.trojan.fallback = value)} placeholder="e.g. 127.0.0.1:80" />
                                                </ConfigSection>
                                            },
                                            "TUNNEL" => html! {
                                                <ConfigSection title="Tunnel">
                                                    <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                                                        { "Xray tunnel inbound has no protocol-specific settings." }
                                                    </div>
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
                            2 if data.protocol == "NAIVEPROXY" => html! {
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
                            2 if data.protocol == "TROJAN" => html! {
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
                                                        || data.protocol == "TRUSTTUNNEL"
                                                        || data.protocol == "TROJAN")
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
                                <Dropdown
                                    label="Network"
                                    value={data.naive_proxy.network.clone()}
                                    options={vec![
                                        DropdownOption { value: "".to_string(), label: "Both (TCP + UDP)".to_string() },
                                        DropdownOption { value: "tcp".to_string(), label: "TCP".to_string() },
                                        DropdownOption { value: "udp".to_string(), label: "UDP".to_string() },
                                    ]}
                                    onchange={update_text(|inbound, value| inbound.naive_proxy.network = value)}
                                />
                                <TextBox
                                    label="QUIC congestion control"
                                    value={data.naive_proxy.quic_congestion_control.clone()}
                                    onchange={update_text(|inbound, value| inbound.naive_proxy.quic_congestion_control = value)}
                                    placeholder="bbr / bbr2 / cubic / reno"
                                />
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
                        "TROJAN" => html! {
                            <ConfigSection title="Trojan">
                                <TextBox label="Default Password" value={data.trojan.password.clone()} onchange={update_text(|inbound, value| inbound.trojan.password = value)} placeholder="Fallback if account token is empty" />
                                <TextBox label="Fallback Target Address" value={data.trojan.fallback.clone()} onchange={update_text(|inbound, value| inbound.trojan.fallback = value)} placeholder="e.g. 127.0.0.1:80" />
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
