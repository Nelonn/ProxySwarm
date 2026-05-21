use super::*;

pub(super) fn render_routing_tab(
    draft: &UseStateHandle<NodeConfigDraft>,
    routing_rules: &[RoutingRuleDraft],
    editing_routing_rule: &UseStateHandle<Option<(usize, RoutingRuleDraft, bool)>>,
    pending_routing_delete: &UseStateHandle<Option<usize>>,
    routing_move_anim: &UseStateHandle<Option<(usize, bool)>>,
) -> Html {
    let inbound_options = draft
        .inbounds
        .iter()
        .map(|inbound| inbound.name.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

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
                            let draft = draft.clone();
                            move |_| {
                                let mut next = (*draft).clone();
                                sync_draft(&mut next);
                                next.reverse_proxies.push(default_reverse_proxy_entry());
                                sync_draft(&mut next);
                                draft.set(next);
                            }
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
                                        let portal_options = std::iter::once(DropdownOption {
                                            value: String::new(),
                                            label: "Select inbound".to_string(),
                                        })
                                        .chain(inbound_options.iter().map(|name| DropdownOption {
                                            value: name.clone(),
                                            label: name.clone(),
                                        }))
                                        .collect::<Vec<_>>();

                                        html! {
                                            <div class="md3-card space-y-4">
                                                <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                                    <div class="font-semibold">{ reverse_proxy_display_name(reverse_proxy, idx) }</div>
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

                                                <SwitchField
                                                    label="Enabled"
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

                                                <Dropdown
                                                    label="Mode"
                                                    value={reverse_proxy.mode.clone()}
                                                    options={vec![
                                                        DropdownOption { value: "portal".to_string(), label: "Portal".to_string() },
                                                        DropdownOption { value: "bridge".to_string(), label: "Bridge".to_string() },
                                                    ]}
                                                    onchange={Callback::from({
                                                        let draft = draft.clone();
                                                        move |value: String| {
                                                            let mut next = (*draft).clone();
                                                            sync_draft(&mut next);
                                                            if let Some(item) = next.reverse_proxies.get_mut(idx) {
                                                                item.mode = value;
                                                            }
                                                            sync_draft(&mut next);
                                                            draft.set(next);
                                                        }
                                                    })}
                                                />

                                                <TextBox
                                                    label="Reverse Tag"
                                                    value={reverse_proxy.tag.clone()}
                                                    onchange={Callback::from({
                                                        let draft = draft.clone();
                                                        move |value: String| {
                                                            let mut next = (*draft).clone();
                                                            sync_draft(&mut next);
                                                            if let Some(item) = next.reverse_proxies.get_mut(idx) {
                                                                item.tag = value;
                                                            }
                                                            sync_draft(&mut next);
                                                            draft.set(next);
                                                        }
                                                    })}
                                                    placeholder="portal"
                                                />

                                                <TextBox
                                                    label="Reverse Domain"
                                                    value={reverse_proxy.domain.clone()}
                                                    onchange={Callback::from({
                                                        let draft = draft.clone();
                                                        move |value: String| {
                                                            let mut next = (*draft).clone();
                                                            sync_draft(&mut next);
                                                            if let Some(item) = next.reverse_proxies.get_mut(idx) {
                                                                item.domain = value;
                                                            }
                                                            sync_draft(&mut next);
                                                            draft.set(next);
                                                        }
                                                    })}
                                                    placeholder="reverse.local"
                                                />

                                                {
                                                    if reverse_proxy.mode == "bridge" {
                                                        html! {
                                                            <>
                                                                <TextBox
                                                                    label="Bridge Outbound Tag"
                                                                    value={reverse_proxy.bridge_outbound_tag.clone()}
                                                                    onchange={Callback::from({
                                                                        let draft = draft.clone();
                                                                        move |value: String| {
                                                                            let mut next = (*draft).clone();
                                                                            sync_draft(&mut next);
                                                                            if let Some(item) = next.reverse_proxies.get_mut(idx) {
                                                                                item.bridge_outbound_tag = value;
                                                                            }
                                                                            sync_draft(&mut next);
                                                                            draft.set(next);
                                                                        }
                                                                    })}
                                                                    placeholder="interconn"
                                                                />
                                                                <TextBox
                                                                    label="Target Outbound Tag"
                                                                    value={reverse_proxy.target_outbound_tag.clone()}
                                                                    onchange={Callback::from({
                                                                        let draft = draft.clone();
                                                                        move |value: String| {
                                                                            let mut next = (*draft).clone();
                                                                            sync_draft(&mut next);
                                                                            if let Some(item) = next.reverse_proxies.get_mut(idx) {
                                                                                item.target_outbound_tag = value;
                                                                            }
                                                                            sync_draft(&mut next);
                                                                            draft.set(next);
                                                                        }
                                                                    })}
                                                                    placeholder="direct"
                                                                />
                                                            </>
                                                        }
                                                    } else {
                                                        html! {
                                                            <>
                                                                <Dropdown
                                                                    label="Portal Inbound Tag"
                                                                    value={reverse_proxy.portal_inbound_tag.clone()}
                                                                    options={portal_options}
                                                                    onchange={Callback::from({
                                                                        let draft = draft.clone();
                                                                        move |value: String| {
                                                                            let mut next = (*draft).clone();
                                                                            sync_draft(&mut next);
                                                                            if let Some(item) = next.reverse_proxies.get_mut(idx) {
                                                                                item.portal_inbound_tag = value;
                                                                            }
                                                                            sync_draft(&mut next);
                                                                            draft.set(next);
                                                                        }
                                                                    })}
                                                                />
                                                                <div class="text-sm opacity-70">{ "Portal inbound tag must match the name of a real inbound on this node." }</div>
                                                            </>
                                                        }
                                                    }
                                                }
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
