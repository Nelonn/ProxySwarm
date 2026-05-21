use super::*;

pub(super) fn render_routing_tab(
    draft: &UseStateHandle<NodeConfigDraft>,
    routing_rules: &[RoutingRuleDraft],
    editing_routing_rule: &UseStateHandle<Option<(usize, RoutingRuleDraft, bool)>>,
    editing_reverse_proxy: &UseStateHandle<Option<(usize, ReverseProxyDraft, bool)>>,
    pending_routing_delete: &UseStateHandle<Option<usize>>,
    routing_move_anim: &UseStateHandle<Option<(usize, bool)>>,
) -> Html {
    html! {
        <div class="space-y-6">
            <div class="md3-card bg-surface-container space-y-4">
                <div class="flex justify-between" style="align-items: center; gap: 16px;">
                    <div>
                        <div class="font-semibold">{ "Xray Reverse Proxy" }</div>
                        <div class="text-sm opacity-70">{ "Reverse portal/bridge config lives here instead of Inbounds. Manager still serializes it in the legacy engine shape for node compatibility." }</div>
                    </div>
                    <Button
                        label="Add Reverse"
                        icon={Some("icon-add".to_string())}
                        button_type={ButtonType::Filled}
                        onclick={Callback::from({
                            let editing_reverse_proxy = editing_reverse_proxy.clone();
                            move |_| editing_reverse_proxy.set(Some((usize::MAX, default_reverse_proxy_entry(), true)))
                        })}
                    />
                </div>

                {
                    if draft.reverse_proxies.is_empty() {
                        html! { <div class="text-sm opacity-70">{ "No reverse proxies configured." }</div> }
                    } else {
                        html! {
                            <div class="space-y-4">
                                {
                                    for draft.reverse_proxies.iter().enumerate().map(|(idx, reverse_proxy)| {
                                        html! {
                                            <div class="md3-card space-y-4">
                                                <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                                    <div>
                                                        <div class="font-semibold">{ reverse_proxy_display_name(reverse_proxy, idx) }</div>
                                                        <div class="text-sm opacity-70">{ if reverse_proxy.enabled { "Enabled" } else { "Disabled" } }</div>
                                                    </div>
                                                    <div class="md3-list-actions">
                                                        <Button
                                                            label="Edit"
                                                            icon={Some("icon-edit".to_string())}
                                                            button_type={ButtonType::Outlined}
                                                            onclick={Callback::from({
                                                                let editing_reverse_proxy = editing_reverse_proxy.clone();
                                                                let reverse_proxy = reverse_proxy.clone();
                                                                move |_| editing_reverse_proxy.set(Some((idx, reverse_proxy.clone(), false)))
                                                            })}
                                                        />
                                                        <Button
                                                            label="Delete"
                                                            button_type={ButtonType::Text}
                                                            color={Some("#F2B8B5".to_string())}
                                                            onclick={Callback::from({
                                                                let draft = draft.clone();
                                                                move |_| {
                                                                    let mut next = (*draft).clone();
                                                                    sync_draft(&mut next);
                                                                    if idx < next.reverse_proxies.len() {
                                                                        next.reverse_proxies.remove(idx);
                                                                    }
                                                                    sync_draft(&mut next);
                                                                    draft.set(next);
                                                                }
                                                            })}
                                                        />
                                                    </div>
                                                </div>

                                                <div class="grid grid-cols-1 md-grid-cols-3 gap-6">
                                                    <div>
                                                        <div class="text-sm opacity-70">{ "Mode" }</div>
                                                        <div>{ optional_label(&reverse_proxy.mode) }</div>
                                                    </div>
                                                    <div>
                                                        <div class="text-sm opacity-70">{ "Reverse Domain" }</div>
                                                        <div>{ optional_label(&reverse_proxy.domain) }</div>
                                                    </div>
                                                    <div>
                                                        <div class="text-sm opacity-70">{ "Reverse Tag" }</div>
                                                        <div>{ optional_label(&reverse_proxy.tag) }</div>
                                                    </div>
                                                    {
                                                        if reverse_proxy.mode == "bridge" {
                                                            html! {
                                                                <>
                                                                    <div>
                                                                        <div class="text-sm opacity-70">{ "Bridge Outbound Tag" }</div>
                                                                        <div>{ optional_label(&reverse_proxy.bridge_outbound_tag) }</div>
                                                                    </div>
                                                                    <div>
                                                                        <div class="text-sm opacity-70">{ "Target Outbound Tag" }</div>
                                                                        <div>{ optional_label(&reverse_proxy.target_outbound_tag) }</div>
                                                                    </div>
                                                                </>
                                                            }
                                                        } else {
                                                            html! {
                                                                <div>
                                                                    <div class="text-sm opacity-70">{ "Portal Inbound Tag" }</div>
                                                                    <div>{ optional_label(&reverse_proxy.portal_inbound_tag) }</div>
                                                                </div>
                                                            }
                                                        }
                                                    }
                                                </div>
                                            </div>
                                        }
                                    })
                                }
                            </div>
                        }
                    }
                }
            </div>

            <ConfigSection title="Routing Rules">
                <div class="space-y-4">
                    <div class="flex justify-between" style="align-items: center; gap: 16px;">
                        <div class="text-sm opacity-70">
                            { "Rules are evaluated top-to-bottom. First matching rule is used." }
                        </div>
                        <Button
                            label="Add Rule"
                            icon={Some("icon-add".to_string())}
                            button_type={ButtonType::Filled}
                            onclick={Callback::from({
                                let editing_routing_rule = editing_routing_rule.clone();
                                let next_index = routing_rules.len();
                                move |_| {
                                    editing_routing_rule.set(Some((next_index, default_routing_rule_entry(), true)));
                                }
                            })}
                        />
                    </div>
                    {
                        if routing_rules.is_empty() {
                            html! {
                                <div class="md3-card">
                                    <div class="text-sm opacity-70">
                                        { "No routing rules yet. Add a rule to start traffic matching." }
                                    </div>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="space-y-4">
                                    {
                                        for routing_rules.iter().enumerate().map(|(idx, rule)| {
                                            let idx_delete = idx;
                                            let idx_up = idx;
                                            let idx_down = idx;
                                            let move_up_active = routing_move_anim.as_ref().map(|(i, up)| *i == idx && *up).unwrap_or(false);
                                            let move_down_active = routing_move_anim.as_ref().map(|(i, up)| *i == idx && !*up).unwrap_or(false);
                                            html! {
                                                <div class={classes!(
                                                    "md3-card",
                                                    "space-y-4",
                                                    "md3-routing-rule-card",
                                                    move_up_active.then_some("md3-routing-rule-card-move-up"),
                                                    move_down_active.then_some("md3-routing-rule-card-move-down")
                                                )}>
                                                    <div class="flex justify-between items-center" style="gap: 0.75rem;">
                                                        <div class="font-semibold">{ format!("Rule #{}", idx + 1) }</div>
                                                        <div class="md3-list-actions">
                                                            <Button
                                                                label="Edit"
                                                                icon={Some("icon-edit".to_string())}
                                                                button_type={ButtonType::Outlined}
                                                                onclick={Callback::from({
                                                                    let editing_routing_rule = editing_routing_rule.clone();
                                                                    let rule = rule.clone();
                                                                    move |_| editing_routing_rule.set(Some((idx, rule.clone(), false)))
                                                                })}
                                                            />
                                                            <button
                                                                type="button"
                                                                class="md3-btn md3-btn-outlined"
                                                                disabled={idx_up == 0}
                                                                onclick={Callback::from({
                                                                    let draft = draft.clone();
                                                                    let routing_move_anim = routing_move_anim.clone();
                                                                    move |_| {
                                                                        if idx_up == 0 {
                                                                            return;
                                                                        }
                                                                        let mut next = (*draft).clone();
                                                                        sync_draft(&mut next);
                                                                        next.routing_rules.swap(idx_up - 1, idx_up);
                                                                        sync_draft(&mut next);
                                                                        draft.set(next);
                                                                        routing_move_anim.set(Some((idx_up - 1, true)));
                                                                        let routing_move_anim_clear = routing_move_anim.clone();
                                                                        Timeout::new(280, move || routing_move_anim_clear.set(None)).forget();
                                                                    }
                                                                })}
                                                            >
                                                                <span class="mr-2" style="display: inline-flex; width: 20px; height: 20px; align-items: center; justify-content: center; line-height: 0;">
                                                                    <SvgIcon name="icon-arrow-upward" size={20} />
                                                                </span>
                                                                { "Move up" }
                                                            </button>
                                                            <button
                                                                type="button"
                                                                class="md3-btn md3-btn-outlined"
                                                                disabled={idx_down + 1 >= routing_rules.len()}
                                                                onclick={Callback::from({
                                                                    let draft = draft.clone();
                                                                    let routing_move_anim = routing_move_anim.clone();
                                                                    move |_| {
                                                                        let mut next = (*draft).clone();
                                                                        sync_draft(&mut next);
                                                                        if idx_down + 1 >= next.routing_rules.len() {
                                                                            return;
                                                                        }
                                                                        next.routing_rules.swap(idx_down, idx_down + 1);
                                                                        sync_draft(&mut next);
                                                                        draft.set(next);
                                                                        routing_move_anim.set(Some((idx_down + 1, false)));
                                                                        let routing_move_anim_clear = routing_move_anim.clone();
                                                                        Timeout::new(280, move || routing_move_anim_clear.set(None)).forget();
                                                                    }
                                                                })}
                                                            >
                                                                <span class="mr-2" style="display: inline-flex; width: 20px; height: 20px; align-items: center; justify-content: center; line-height: 0;">
                                                                    <SvgIcon name="icon-arrow-downward" size={20} />
                                                                </span>
                                                                { "Move down" }
                                                            </button>
                                                            <Button
                                                                label="Delete"
                                                                button_type={ButtonType::Text}
                                                                color={Some("#F2B8B5".to_string())}
                                                                onclick={Callback::from({
                                                                    let pending_routing_delete = pending_routing_delete.clone();
                                                                    move |_| pending_routing_delete.set(Some(idx_delete))
                                                                })}
                                                            />
                                                        </div>
                                                    </div>
                                                    <div class="grid grid-cols-1 md-grid-cols-3 gap-6">
                                                        <div>
                                                            <div class="text-sm opacity-70">{ "Remark" }</div>
                                                            <div>{ if rule.remark.trim().is_empty() { "-" } else { rule.remark.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="text-sm opacity-70">{ "Outbound" }</div>
                                                            <div>{ if rule.outbound_tag.trim().is_empty() { "-" } else { rule.outbound_tag.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="text-sm opacity-70">{ "Domains" }</div>
                                                            <div>{ if rule.domain.trim().is_empty() { "-" } else { rule.domain.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="text-sm opacity-70">{ "IPs" }</div>
                                                            <div>{ if rule.ip.trim().is_empty() { "-" } else { rule.ip.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="text-sm opacity-70">{ "Ports" }</div>
                                                            <div>{ if rule.port.trim().is_empty() { "-" } else { rule.port.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="text-sm opacity-70">{ "Transport" }</div>
                                                            <div>
                                                                {
                                                                    {
                                                                        let mut has_tcp = false;
                                                                        let mut has_udp = false;
                                                                        for value in split_lines_csv(&rule.transport)
                                                                            .into_iter()
                                                                            .map(|value| value.trim().to_lowercase())
                                                                        {
                                                                            match value.as_str() {
                                                                                "tcp" => has_tcp = true,
                                                                                "udp" => has_udp = true,
                                                                                _ => {}
                                                                            }
                                                                        }
                                                                        let label = match (has_tcp, has_udp) {
                                                                            (true, true) => "tcp,udp",
                                                                            (true, false) => "tcp",
                                                                            (false, true) => "udp",
                                                                            (false, false) => "-",
                                                                        };
                                                                        html! { label }
                                                                    }
                                                                }
                                                            </div>
                                                        </div>
                                                        <div>
                                                            <div class="text-sm opacity-70">{ "Protocols" }</div>
                                                            <div>
                                                                {
                                                                    {
                                                                        let mut values = split_lines_csv(&rule.protocol)
                                                                            .into_iter()
                                                                            .map(|value| value.trim().to_lowercase())
                                                                            .filter(|value| !value.is_empty())
                                                                            .collect::<Vec<_>>();
                                                                        values.sort();
                                                                        values.dedup();
                                                                        if values.is_empty() {
                                                                            html! { "-" }
                                                                        } else {
                                                                            html! { values.join(", ") }
                                                                        }
                                                                    }
                                                                }
                                                            </div>
                                                        </div>
                                                        <div>
                                                            <div class="text-sm opacity-70">{ "Inbound Tags" }</div>
                                                            <div>{ if rule.inbound_tag.trim().is_empty() { "-" } else { rule.inbound_tag.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="text-sm opacity-70">{ "Users" }</div>
                                                            <div>{ if rule.user.trim().is_empty() { "-" } else { rule.user.as_str() } }</div>
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        })
                                    }
                                </div>
                            }
                        }
                    }
                </div>
            </ConfigSection>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct ReverseProxyEditorPopupProps {
    pub(super) reverse_proxy: ReverseProxyDraft,
    pub(super) is_new: bool,
    pub(super) inbound_options: Vec<String>,
    pub(super) on_close: Callback<()>,
    pub(super) on_save: Callback<ReverseProxyDraft>,
}

#[function_component(ReverseProxyEditorPopup)]
pub(super) fn reverse_proxy_editor_popup(props: &ReverseProxyEditorPopupProps) -> Html {
    let reverse_proxy = use_state(|| props.reverse_proxy.clone());
    {
        let reverse_proxy = reverse_proxy.clone();
        let incoming = props.reverse_proxy.clone();
        use_effect_with(incoming, move |next_reverse_proxy| {
            reverse_proxy.set(next_reverse_proxy.clone());
            || ()
        });
    }

    let on_text_change = |mutator: fn(&mut ReverseProxyDraft, String)| {
        let reverse_proxy = reverse_proxy.clone();
        Callback::from(move |value: String| {
            let mut next = (*reverse_proxy).clone();
            mutator(&mut next, value);
            reverse_proxy.set(next);
        })
    };

    let on_bool_change = |mutator: fn(&mut ReverseProxyDraft, bool)| {
        let reverse_proxy = reverse_proxy.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            let mut next = (*reverse_proxy).clone();
            mutator(&mut next, input.checked());
            reverse_proxy.set(next);
        })
    };

    let data = (*reverse_proxy).clone();
    let portal_options = std::iter::once(DropdownOption {
        value: String::new(),
        label: "Select inbound".to_string(),
    })
    .chain(props.inbound_options.iter().map(|name| DropdownOption {
        value: name.clone(),
        label: name.clone(),
    }))
    .collect::<Vec<_>>();
    let save_disabled = data.mode.trim().is_empty()
        || data.tag.trim().is_empty()
        || data.domain.trim().is_empty()
        || (data.mode == "bridge" && data.bridge_outbound_tag.trim().is_empty())
        || (data.mode == "portal" && data.portal_inbound_tag.trim().is_empty());

    html! {
        <Popup
            title={if props.is_new { "Add Xray Reverse Proxy" } else { "Edit Xray Reverse Proxy" }}
            size={PopupSize::Md}
            on_close={props.on_close.clone()}
        >
            <div class="space-y-4">
                <SwitchField
                    label="Enabled"
                    checked={data.enabled}
                    onchange={on_bool_change(|draft, value| draft.enabled = value)}
                />
                <Dropdown
                    label="Mode"
                    value={data.mode.clone()}
                    options={vec![
                        DropdownOption { value: "portal".to_string(), label: "Portal".to_string() },
                        DropdownOption { value: "bridge".to_string(), label: "Bridge".to_string() },
                    ]}
                    onchange={on_text_change(|draft, value| draft.mode = value)}
                />
                <TextBox
                    label="Reverse Tag"
                    value={data.tag.clone()}
                    onchange={on_text_change(|draft, value| draft.tag = value)}
                    placeholder="portal"
                />
                <TextBox
                    label="Reverse Domain"
                    value={data.domain.clone()}
                    onchange={on_text_change(|draft, value| draft.domain = value)}
                    placeholder="reverse.local"
                />
                {
                    if data.mode == "bridge" {
                        html! {
                            <>
                                <TextBox
                                    label="Bridge Outbound Tag"
                                    value={data.bridge_outbound_tag.clone()}
                                    onchange={on_text_change(|draft, value| draft.bridge_outbound_tag = value)}
                                    placeholder="interconn"
                                />
                                <TextBox
                                    label="Target Outbound Tag"
                                    value={data.target_outbound_tag.clone()}
                                    onchange={on_text_change(|draft, value| draft.target_outbound_tag = value)}
                                    placeholder="direct"
                                />
                            </>
                        }
                    } else {
                        html! {
                            <>
                                <Dropdown
                                    label="Portal Inbound Tag"
                                    value={data.portal_inbound_tag.clone()}
                                    options={portal_options}
                                    onchange={on_text_change(|draft, value| draft.portal_inbound_tag = value)}
                                />
                                <div class="text-sm opacity-70">{ "Portal inbound tag must match the name of a real inbound on this node." }</div>
                            </>
                        }
                    }
                }
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    <Button label={if props.is_new { "Add Reverse" } else { "Apply Changes" }} button_type={ButtonType::Filled} disabled={save_disabled} onclick={Callback::from({
                        let on_save = props.on_save.clone();
                        let reverse_proxy = reverse_proxy.clone();
                        move |_| on_save.emit((*reverse_proxy).clone())
                    })} />
                </div>
            </div>
        </Popup>
    }
}
