use super::*;

pub(super) fn render_routing_tab(
    draft: &UseStateHandle<NodeConfigDraft>,
    routing_rules: &[RoutingRuleDraft],
    editing_routing_rule: &UseStateHandle<Option<(usize, RoutingRuleDraft, bool)>>,
    action_routing_rule: &UseStateHandle<Option<(usize, (f64, f64, f64))>>,
    _pending_routing_delete: &UseStateHandle<Option<usize>>,
    routing_move_anim: &UseStateHandle<Option<(usize, bool)>>,
) -> Html {
    html! {
        <div class="space-y-6">
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
                                                    "space-y-3",
                                                    "md3-routing-rule-card",
                                                    move_up_active.then_some("md3-routing-rule-card-move-up"),
                                                    move_down_active.then_some("md3-routing-rule-card-move-down")
                                                )} style="padding: 0.875rem 1rem;">
                                                    <div class="flex justify-between items-center" style="gap: 0.75rem;">
                                                        <div class="font-semibold">{ format!("Rule #{}", idx + 1) }</div>
                                                        <div class="md3-list-actions">
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
                                                                label="Action"
                                                                button_type={ButtonType::Outlined}
                                                                onclick={Callback::from({
                                                                    let action_routing_rule = action_routing_rule.clone();
                                                                    move |e: MouseEvent| {
                                                                        if let Some((left, top, width)) = menu_anchor_from_mouse_event(&e) {
                                                                            action_routing_rule.set(Some((idx_delete, (left, top, width))));
                                                                        }
                                                                    }
                                                                })}
                                                            />
                                                        </div>
                                                    </div>
                                                    <div class="grid gap-3 text-sm" style="line-height: 1.3; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));">
                                                        <div>
                                                            <div class="opacity-70">{ "Remark" }</div>
                                                            <div>{ if rule.remark.trim().is_empty() { "-" } else { rule.remark.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="opacity-70">{ "Outbound" }</div>
                                                            <div>{ if rule.outbound_tag.trim().is_empty() { "-" } else { rule.outbound_tag.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="opacity-70">{ "Domains" }</div>
                                                            <div>{ if rule.domain.trim().is_empty() { "-" } else { rule.domain.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="opacity-70">{ "IPs" }</div>
                                                            <div>{ if rule.ip.trim().is_empty() { "-" } else { rule.ip.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="opacity-70">{ "Ports" }</div>
                                                            <div>{ if rule.port.trim().is_empty() { "-" } else { rule.port.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="opacity-70">{ "Transport" }</div>
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
                                                            <div class="opacity-70">{ "Protocols" }</div>
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
                                                            <div class="opacity-70">{ "Inbound Tags" }</div>
                                                            <div>{ if rule.inbound_tag.trim().is_empty() { "-" } else { rule.inbound_tag.as_str() } }</div>
                                                        </div>
                                                        <div>
                                                            <div class="opacity-70">{ "Users" }</div>
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
    pub(super) user_options: Vec<DropdownOption>,
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
    let portal_user_options = std::iter::once(DropdownOption {
        value: String::new(),
        label: "Select user".to_string(),
    })
    .chain(props.user_options.iter().cloned())
    .collect::<Vec<_>>();
    let save_disabled = data.tag.trim().is_empty()
        || data.portal_inbound_tag.trim().is_empty()
        || data.portal_user_id.trim().is_empty();

    html! {
        <Popup
            title={if props.is_new { "Add VLESS Reverse" } else { "Edit VLESS Reverse" }}
            size={PopupSize::Md}
            on_close={props.on_close.clone()}
        >
            <div class="space-y-4">
                <SwitchField
                    label="Enabled"
                    checked={data.enabled}
                    onchange={on_bool_change(|draft, value| draft.enabled = value)}
                />
                <TextBox
                    label="Tag"
                    value={data.tag.clone()}
                    onchange={on_text_change(|draft, value| draft.tag = value)}
                    placeholder="r-outbound"
                />
                <Dropdown
                    label="Inbound Tag"
                    value={data.portal_inbound_tag.clone()}
                    options={portal_options}
                    onchange={on_text_change(|draft, value| draft.portal_inbound_tag = value)}
                />
                <Dropdown
                    label="User"
                    value={data.portal_user_id.clone()}
                    options={portal_user_options}
                    onchange={on_text_change(|draft, value| draft.portal_user_id = value)}
                />
                <div class="text-sm opacity-70">{ "This creates a VLESS reverse tunnel for the selected user and exposes it as an outbound tag for routing rules." }</div>
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    <Button label={if props.is_new { "Add VLESS Reverse" } else { "Apply Changes" }} button_type={ButtonType::Filled} disabled={save_disabled} onclick={Callback::from({
                        let on_save = props.on_save.clone();
                        let reverse_proxy = reverse_proxy.clone();
                        move |_| on_save.emit((*reverse_proxy).clone())
                    })} />
                </div>
            </div>
        </Popup>
    }
}
