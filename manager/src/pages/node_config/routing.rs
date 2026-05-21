use super::*;

pub(super) fn render_routing_tab(
    draft: &UseStateHandle<NodeConfigDraft>,
    routing_rules: &[RoutingRuleDraft],
    editing_routing_rule: &UseStateHandle<Option<(usize, RoutingRuleDraft, bool)>>,
    pending_routing_delete: &UseStateHandle<Option<usize>>,
    routing_move_anim: &UseStateHandle<Option<(usize, bool)>>,
) -> Html {
    html! {
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
    }
}

