use super::*;

pub(super) fn outbound_creation_steps(outbound: &OutboundEntryDraft) -> usize {
    match outbound.outbound_type.as_str() {
        "VLESS" => 3,
        "WIREGUARD" => 3,
        "SOCKS5" => 3,
        _ => 3,
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct OutboundEditorPopupProps {
    pub(super) outbound: OutboundEntryDraft,
    pub(super) is_new: bool,
    pub(super) node_address: String,
    pub(super) master_key: String,
    pub(super) on_close: Callback<()>,
    pub(super) on_save: Callback<OutboundEntryDraft>,
}

#[derive(Properties, PartialEq)]
pub(super) struct WarpCreatePopupProps {
    pub(super) node_address: String,
    pub(super) master_key: String,
    pub(super) initial_registration: Option<crate::services::warp::WarpRegistration>,
    pub(super) on_registration_change: Callback<Option<crate::services::warp::WarpRegistration>>,
    pub(super) on_close: Callback<()>,
    pub(super) on_create: Callback<OutboundEntryDraft>,
}

#[function_component(WarpCreatePopup)]
pub(super) fn warp_create_popup(props: &WarpCreatePopupProps) -> Html {
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
pub(super) fn outbound_editor_popup(props: &OutboundEditorPopupProps) -> Html {
    let outbound = use_state(|| props.outbound.clone());
    let step = use_state(|| 0usize);
    let import_link = use_state(String::new);
    let import_status = use_state(|| Option::<String>::None);

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
                                            DropdownOption { value: "CUSTOM".to_string(), label: "Custom Plugin".to_string() },
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
                                            "CUSTOM" => html! {
                                                <div class="grid grid-cols-1 gap-6">
                                                    <TextBox label="Tag" value={data.custom.tag.clone()} onchange={update_text(|outbound, value| outbound.custom.tag = value)} />
                                                    <TextBox label="Handler Name" value={data.custom.handler_name.clone()} onchange={update_text(|outbound, value| outbound.custom.handler_name = value)} placeholder="redirect" />
                                                    <TextBox label="Config JSON" value={data.custom.config_json.clone()} onchange={update_text(|outbound, value| outbound.custom.config_json = value)} is_textarea={true} placeholder={"{\n  \"address\": \"127.0.0.1\",\n  \"port\": 8080\n}"} />
                                                </div>
                                            },
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
                                                <div class="space-y-4">
                                                    <TextBox
                                                        label="Import VLESS Link"
                                                        value={(*import_link).clone()}
                                                        onchange={Callback::from({
                                                            let import_link = import_link.clone();
                                                            move |value: String| import_link.set(value)
                                                        })}
                                                        is_textarea={true}
                                                        placeholder="vless://uuid@host:443?security=reality&type=tcp..."
                                                    />
                                                    <div class="flex" style="gap: 0.75rem; align-items: center;">
                                                        <Button
                                                            label="Import Link"
                                                            button_type={ButtonType::Outlined}
                                                            onclick={Callback::from({
                                                                let outbound = outbound.clone();
                                                                let import_link = import_link.clone();
                                                                let import_status = import_status.clone();
                                                                move |_| match import_vless_outbound_link(&(*import_link), &(*outbound)) {
                                                                    Ok(next) => {
                                                                        outbound.set(next);
                                                                        import_status.set(Some("Imported VLESS link. Review and adjust fields below.".to_string()));
                                                                    }
                                                                    Err(error) => import_status.set(Some(error)),
                                                                }
                                                            })}
                                                        />
                                                        {
                                                            if let Some(message) = &*import_status {
                                                                html! { <div class="text-sm opacity-70">{ message.clone() }</div> }
                                                            } else {
                                                                html! {}
                                                            }
                                                        }
                                                    </div>
                                                    <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                        <TextBox label="UUID" value={data.vless.uuid.clone()} onchange={update_text(|outbound, value| outbound.vless.uuid = value)} />
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
                                                <div class="space-y-4">
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
                                                        <TextBox label="TLS Server Name" value={data.vless.tls_server_name.clone()} onchange={update_text(|outbound, value| outbound.vless.tls_server_name = value)} />
                                                    </div>
                                                    {
                                                        if data.vless.security.trim().eq_ignore_ascii_case("REALITY") {
                                                            html! {
                                                                <>
                                                                    <ConfigSection title="Reality">
                                                                        <TextBox label="SNI" value={data.vless.reality_sni.clone()} onchange={update_text(|outbound, value| outbound.vless.reality_sni = value)} />
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
                                                                            onchange={update_text(|outbound, value| outbound.vless.reality_utls = value)}
                                                                        />
                                                                        <TextBox label="SpiderX" value={data.vless.reality_spider_x.clone()} onchange={update_text(|outbound, value| outbound.vless.reality_spider_x = value)} placeholder="/" />
                                                                        <TextBox label="Public Key" value={data.vless.reality_public_key.clone()} onchange={update_text(|outbound, value| outbound.vless.reality_public_key = value)} />
                                                                        <TextBox label="Short IDs" value={data.vless.reality_short_ids.clone()} onchange={update_text(|outbound, value| outbound.vless.reality_short_ids = value)} placeholder="id1, id2" />
                                                                        <div class="flex" style="gap: 0.75rem;">
                                                                            <Button label="Generate Short IDs" button_type={ButtonType::Outlined} onclick={Callback::from({
                                                                                let outbound = outbound.clone();
                                                                                move |_| {
                                                                                    let mut next = (*outbound).clone();
                                                                                    let mut ids = split_lines_csv(&next.vless.reality_short_ids);
                                                                                    ids.extend(generate_reality_short_ids_batch(6));
                                                                                    next.vless.reality_short_ids = ids.join(",");
                                                                                    outbound.set(next);
                                                                                }
                                                                            })} />
                                                                        </div>
                                                                    </ConfigSection>
                                                                </>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </div>
                                            },
                                            _ => html! { { render_outbound_review(&data) } }
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
