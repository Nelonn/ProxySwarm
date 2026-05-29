use super::*;

pub(super) fn render_outbounds_tab(
    draft: &UseStateHandle<NodeConfigDraft>,
    outbounds: &[OutboundEntryDraft],
    reverse_proxies: &[ReverseProxyDraft],
    editing_outbound: &UseStateHandle<Option<(OutboundEntryDraft, bool)>>,
    editing_reverse_proxy: &UseStateHandle<Option<(usize, ReverseProxyDraft, bool)>>,
    warp_popup_open: &UseStateHandle<bool>,
    action_outbound: &UseStateHandle<Option<(String, (f64, f64, f64))>>,
    action_reverse_proxy: &UseStateHandle<Option<(usize, (f64, f64, f64))>>,
) -> Html {
    html! {                        <div class="space-y-6">
                            <div class="flex justify-between" style="align-items: center;">
                                <div>
                                    <h2 class="text-2xl font-bold">{ "Outbounds" }</h2>
                                    <div class="text-sm opacity-70">{ "Builtin Direct and Block stay fixed. Add VLESS, custom plugin, VLESS Reverse, TrustTunnel, WireGuard, SOCKS5, and Shadowsocks as reusable outbound entries." }</div>
                                </div>
                                <div class="flex" style="gap: 0.75rem;">
                                     <Button
                                         label="Add Outbound"
                                         icon={Some("icon-add".to_string())}
                                         button_type={ButtonType::Filled}
                                         onclick={Callback::from({
                                        let editing_outbound = editing_outbound.clone();
                                        move |_| editing_outbound.set(Some((default_vless_outbound(), true)))
                                     })}
                                     />
                                     <Button label="MASQUE" button_type={ButtonType::Outlined} onclick={Callback::from({
                                         let editing_outbound = editing_outbound.clone();
                                         move |_| editing_outbound.set(Some((default_usque_masque_outbound(), true)))
                                     })} />
                                     <Button label="WARP" button_type={ButtonType::Outlined} onclick={Callback::from({
                                         let warp_popup_open = warp_popup_open.clone();
                                         move |_| warp_popup_open.set(true)
                                     })} />
                                    <Button
                                        label="Add VLESS Reverse"
                                        button_type={ButtonType::Outlined}
                                        onclick={Callback::from({
                                            let editing_reverse_proxy = editing_reverse_proxy.clone();
                                            move |_| editing_reverse_proxy.set(Some((usize::MAX, default_reverse_proxy_entry(), true)))
                                        })}
                                    />
                                </div>
                            </div>
                            <RichTable columns={vec![
                                "Name".to_string(),
                                "Type".to_string(),
                                "Enabled".to_string(),
                                "Tag / Target".to_string(),
                                "Actions".to_string(),
                            ]} card_class={Some("bg-surface-container".to_string())} header_in_list={true}>
                                {
                                    for outbounds.iter().map(|outbound| {
                                        let action_id = outbound.id.clone();
                                        let type_label = outbound.outbound_type.clone();
                                        let toggle_id = outbound.id.clone();
                                        let toggle_allowed = outbound.outbound_type.trim().to_uppercase() != "BLOCK";
                                        let target_label = match outbound.outbound_type.trim().to_uppercase().as_str() {
                                            "DIRECT" | "BLOCK" => outbound.name.clone(),
                                            "TRUSTTUNNEL" => outbound.name.clone(),
                                            "WIREGUARD" => outbound
                                                .wireguard
                                                .peers
                                                .first()
                                                .map(|peer| peer.endpoint.clone())
                                                .unwrap_or_default(),
                                            "VLESS" => {
                                                let server = outbound.vless.server.trim();
                                                if server.is_empty() || outbound.vless.port <= 0 {
                                                    outbound.vless.tag.clone()
                                                } else {
                                                    format!("{}:{}", server, outbound.vless.port)
                                                }
                                             }
                                             "CUSTOM" => outbound.name.clone(),
                                             "USQUE_MASQUE" => {
                                                 let endpoint = outbound.usque_masque.endpoint.trim();
                                                 if endpoint.is_empty() {
                                                     outbound.name.clone()
                                                 } else {
                                                     format!("{} via {}", outbound.name, endpoint)
                                                 }
                                             }
                                             "SOCKS5" => format!("{}:{}", outbound.socks5.server, outbound.socks5.port),
                                            "SHADOWSOCKS" => format!("{}:{}", outbound.shadowsocks.server, outbound.shadowsocks.port),
                                            _ => outbound.vless.tag.clone(),
                                        };
                                        html! {
                                            <>
                                                <div class="md3-divider"></div>
                                                <div class="md3-list-row">
                                                    <div class="md3-list-col-main">
                                                        <div class="font-semibold">{ outbound.name.clone() }</div>
                                                        <div class="text-sm opacity-70">
                                                            {
                                                                if outbound.builtin {
                                                                    "Built-in outbound"
                                                                } else {
                                                                    "Custom outbound"
                                                                }
                                                            }
                                                        </div>
                                                    </div>
                                                    <div class="md3-list-col">{ type_label }</div>
                                                    <div class="md3-list-col">
                                                        {
                                                            if toggle_allowed {
                                                                html! {
                                                                    <Switch
                                                                        checked={outbound.enabled}
                                                                        onchange={Callback::from({
                                                                            let draft = draft.clone();
                                                                            move |e: Event| {
                                                                                let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
                                                                                let mut next = (*draft).clone();
                                                                                sync_draft(&mut next);
                                                                                if let Some(item) = next.outbounds.iter_mut().find(|item| item.id == toggle_id) {
                                                                                    item.enabled = input.checked();
                                                                                }
                                                                                sync_draft(&mut next);
                                                                                draft.set(next);
                                                                            }
                                                                        })}
                                                                    />
                                                                }
                                                            } else {
                                                                html! { <span class="opacity-60">{ "—" }</span> }
                                                            }
                                                        }
                                                    </div>
                                                    <div class="md3-list-col">{ target_label }</div>
                                                    <div class="md3-list-col-actions">
                                                        <div class="md3-list-actions">
                                                            <Button label="Action" button_type={ButtonType::Outlined} onclick={Callback::from({
                                                                let action_outbound = action_outbound.clone();
                                                                move |e: MouseEvent| {
                                                                    if let Some((left, top, width)) = menu_anchor_from_mouse_event(&e) {
                                                                        action_outbound.set(Some((
                                                                            action_id.clone(),
                                                                            (left, top, width),
                                                                        )));
                                                                    }
                                                                }
                                                            })} />
                                                        </div>
                                                    </div>
                                                </div>
                                            </>
                                        }
                                    })
                                }
                            </RichTable>

                            <div class="space-y-3">
                                <div class="text-sm font-semibold opacity-80">{ "VLESS Reverse" }</div>
                                {
                                    if reverse_proxies.is_empty() {
                                        html! {
                                            <div class="md3-card bg-surface-container">
                                                <div class="text-sm opacity-70">{ "No VLESS Reverse entries configured." }</div>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <RichTable columns={vec![
                                                "Name".to_string(),
                                                "Type".to_string(),
                                                "Enabled".to_string(),
                                                "Tag / Target".to_string(),
                                                "Actions".to_string(),
                                            ]} card_class={Some("bg-surface-container".to_string())} header_in_list={true}>
                                                {
                                                    for reverse_proxies.iter().enumerate().map(|(idx, reverse_proxy)| {
                                                        html! {
                                                            <>
                                                                <div class="md3-divider"></div>
                                                                <div class="md3-list-row">
                                                                    <div class="md3-list-col-main">
                                                                        <div class="font-semibold">{ reverse_proxy_display_name(reverse_proxy, idx) }</div>
                                                                        <div class="text-sm opacity-70">{ "VLESS reverse outbound" }</div>
                                                                    </div>
                                                                    <div class="md3-list-col">{ "VLESS Reverse" }</div>
                                                                    <div class="md3-list-col">
                                                                        <Switch
                                                                            checked={reverse_proxy.enabled}
                                                                            onchange={Callback::from({
                                                                                let draft = draft.clone();
                                                                                move |e: Event| {
                                                                                    let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
                                                                                    let mut next = (*draft).clone();
                                                                                    sync_draft(&mut next);
                                                                                    if let Some(item) = next.reverse_proxies.get_mut(idx) {
                                                                                        item.enabled = input.checked();
                                                                                    }
                                                                                    sync_draft(&mut next);
                                                                                    draft.set(next);
                                                                                }
                                                                            })}
                                                                        />
                                                                    </div>
                                                                    <div class="md3-list-col">{ format!("{} / {}", optional_label(&reverse_proxy.portal_inbound_tag), optional_label(&reverse_proxy.portal_user_id)) }</div>
                                                                    <div class="md3-list-col-actions">
                                                                        <div class="md3-list-actions">
                                                                            <Button label="Action" button_type={ButtonType::Outlined} onclick={Callback::from({
                                                                                let action_reverse_proxy = action_reverse_proxy.clone();
                                                                                move |e: MouseEvent| {
                                                                                    if let Some((left, top, width)) = menu_anchor_from_mouse_event(&e) {
                                                                                        action_reverse_proxy.set(Some((idx, (left, top, width))));
                                                                                    }
                                                                                }
                                                                            })} />
                                                                        </div>
                                                                    </div>
                                                                </div>
                                                            </>
                                                        }
                                                    })
                                                }
                                            </RichTable>
                                        }
                                    }
                                }
                            </div>
                        </div>
    }
}
