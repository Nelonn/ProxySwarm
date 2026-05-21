use super::*;

pub(super) fn render_outbounds_tab(
    draft: &UseStateHandle<NodeConfigDraft>,
    outbounds: &[OutboundEntryDraft],
    editing_outbound: &UseStateHandle<Option<(OutboundEntryDraft, bool)>>,
    warp_popup_open: &UseStateHandle<bool>,
    pending_outbound_delete: &UseStateHandle<Option<(String, String)>>,
) -> Html {
    html! {                        <div class="space-y-6">
                            <div class="flex justify-between" style="align-items: center;">
                                <div>
                                    <h2 class="text-2xl font-bold">{ "Outbounds" }</h2>
                                    <div class="text-sm opacity-70">{ "Builtin Direct and Block stay fixed. Add VLESS, TrustTunnel, WireGuard, SOCKS5, and Shadowsocks as reusable outbound entries." }</div>
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
                                    <Button label="WARP" button_type={ButtonType::Outlined} onclick={Callback::from({
                                        let warp_popup_open = warp_popup_open.clone();
                                        move |_| warp_popup_open.set(true)
                                    })} />
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
                                        let edit_id = outbound.id.clone();
                                        let delete_id = outbound.id.clone();
                                        let type_label = outbound.outbound_type.clone();
                                        let toggle_id = outbound.id.clone();
                                        let toggle_allowed = outbound.outbound_type.trim().to_uppercase() != "BLOCK";
                                        let target_label = match outbound.outbound_type.trim().to_uppercase().as_str() {
                                            "DIRECT" | "BLOCK" => outbound.name.clone(),
                                            "TRUSTTUNNEL" => outbound.trust_tunnel.tag.clone(),
                                            "WIREGUARD" => outbound
                                                .wireguard
                                                .peers
                                                .first()
                                                .map(|peer| peer.endpoint.clone())
                                                .unwrap_or_default(),
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
                                                            <Button label="Edit" button_type={ButtonType::Outlined} onclick={Callback::from({
                                                                let editing_outbound = editing_outbound.clone();
                                                                let draft = draft.clone();
                                                                move |_| {
                                                                    let mut data = (*draft).clone();
                                                                    sync_draft(&mut data);
                                                                    editing_outbound.set(data.outbounds.iter().find(|item| item.id == edit_id).cloned().map(|value| (value, false)));
                                                                }
                                                            })} />
                                                            {
                                                                if outbound.builtin {
                                                                    html! {}
                                                                } else {
                                                                    html! {
                                                                        <Button label="Delete" button_type={ButtonType::Text} color={Some("#F2B8B5".to_string())} onclick={Callback::from({
                                                                            let pending_outbound_delete = pending_outbound_delete.clone();
                                                                            let outbound_name = outbound.name.clone();
                                                                            move |_| pending_outbound_delete.set(Some((delete_id.clone(), outbound_name.clone())))
                                                                        })} />
                                                                    }
                                                                }
                                                            }
                                                        </div>
                                                    </div>
                                                </div>
                                            </>
                                        }
                                    })
                                }
                            </RichTable>
                        </div>
    }
}

