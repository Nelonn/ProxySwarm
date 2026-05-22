use super::*;

pub(super) struct PopupHostContext<'a> {
    pub(super) state: &'a UseStateHandle<State>,
    pub(super) draft: &'a UseStateHandle<NodeConfigDraft>,
    pub(super) node: &'a ProxyNode,
    pub(super) draft_value: &'a NodeConfigDraft,
    pub(super) snackbar: &'a Option<SnackbarBus>,
    pub(super) editing_routing_rule: &'a UseStateHandle<Option<(usize, RoutingRuleDraft, bool)>>,
    pub(super) editing_reverse_proxy: &'a UseStateHandle<Option<(usize, ReverseProxyDraft, bool)>>,
    pub(super) action_routing_rule: &'a UseStateHandle<Option<(usize, (f64, f64, f64))>>,
    pub(super) pending_routing_delete: &'a UseStateHandle<Option<usize>>,
    pub(super) action_reverse_proxy: &'a UseStateHandle<Option<(usize, (f64, f64, f64))>>,
    pub(super) pending_reverse_proxy_delete: &'a UseStateHandle<Option<(usize, String)>>,
    pub(super) pending_duplicate_reverse_proxy: &'a UseStateHandle<Option<ReverseProxyDraft>>,
    pub(super) pending_inbound_delete: &'a UseStateHandle<Option<(String, String)>>,
    pub(super) pending_duplicate_inbound: &'a UseStateHandle<Option<InboundEntryDraft>>,
    pub(super) action_inbound: &'a UseStateHandle<Option<(String, (f64, f64, f64))>>,
    pub(super) pending_outbound_delete: &'a UseStateHandle<Option<(String, String)>>,
    pub(super) pending_duplicate_outbound: &'a UseStateHandle<Option<OutboundEntryDraft>>,
    pub(super) action_outbound: &'a UseStateHandle<Option<(String, (f64, f64, f64))>>,
    pub(super) warp_popup_open: &'a UseStateHandle<bool>,
    pub(super) access_link_inbound_id: &'a UseStateHandle<Option<String>>,
    pub(super) deploy_confirm_open: &'a UseStateHandle<bool>,
    pub(super) deploy_preview_json: &'a UseStateHandle<Option<String>>,
    pub(super) deploy_revision: &'a Callback<()>,
    pub(super) acme_confirm_open: &'a UseStateHandle<bool>,
    pub(super) pending_acme_certificate: &'a UseStateHandle<Option<CertificateDraft>>,
    pub(super) acme_logs: &'a UseStateHandle<Option<AcmeIssueResponse>>,
    pub(super) acme_logs_open: &'a UseStateHandle<bool>,
    pub(super) acme_loading: &'a UseStateHandle<bool>,
    pub(super) editing_dns_server: &'a UseStateHandle<Option<(usize, DnsServerDraft, bool)>>,
    pub(super) editing_dns_host: &'a UseStateHandle<Option<(usize, DnsHostDraft, bool)>>,
    pub(super) editing_certificate: &'a UseStateHandle<Option<(CertificateDraft, bool)>>,
    pub(super) editing_inbound: &'a UseStateHandle<Option<(InboundEntryDraft, bool)>>,
    pub(super) editing_outbound: &'a UseStateHandle<Option<(OutboundEntryDraft, bool)>>,
}

trait NodeConfigPopup {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html;
}

struct RoutingRuleEditorPopupSlot;
struct RoutingRuleActionMenuPopupSlot;
struct ReverseProxyEditorPopupSlot;
struct ReverseProxyActionMenuPopupSlot;
struct DeleteReverseProxyPopupSlot;
struct DuplicateReverseProxyPopupSlot;
struct DeployConfirmPopupSlot;
struct DeployPreviewPopupSlot;
struct DeleteInboundPopupSlot;
struct InboundActionMenuPopupSlot;
struct DuplicateInboundPopupSlot;
struct OutboundActionMenuPopupSlot;
struct DeleteOutboundPopupSlot;
struct DuplicateOutboundPopupSlot;
struct DeleteRoutingRulePopupSlot;
struct WarpCreatePopupSlot;
struct AcmeConfirmPopupSlot;
struct AcmeLogsPopupSlot;
struct DnsServerEditorPopupSlot;
struct DnsHostEditorPopupSlot;
struct CertificateEditorPopupSlot;
struct InboundEditorPopupSlot;
struct OutboundEditorPopupSlot;
struct AccessLinkPopupSlot;

impl NodeConfigPopup for RoutingRuleEditorPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_routing_rule_editor_popup(ctx) }
}

impl NodeConfigPopup for RoutingRuleActionMenuPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_routing_rule_action_menu_popup(ctx) }
}

impl NodeConfigPopup for ReverseProxyEditorPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_reverse_proxy_editor_popup(ctx) }
}

impl NodeConfigPopup for ReverseProxyActionMenuPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_reverse_proxy_action_menu_popup(ctx) }
}

impl NodeConfigPopup for DeleteReverseProxyPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_delete_reverse_proxy_popup(ctx) }
}

impl NodeConfigPopup for DuplicateReverseProxyPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_duplicate_reverse_proxy_popup(ctx) }
}

impl NodeConfigPopup for DeployConfirmPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_deploy_confirm_popup(ctx) }
}

impl NodeConfigPopup for DeployPreviewPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_deploy_preview_popup(ctx) }
}

impl NodeConfigPopup for DeleteInboundPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_delete_inbound_popup(ctx) }
}

impl NodeConfigPopup for InboundActionMenuPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_inbound_action_menu_popup(ctx) }
}

impl NodeConfigPopup for DuplicateInboundPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_duplicate_inbound_popup(ctx) }
}

impl NodeConfigPopup for OutboundActionMenuPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_outbound_action_menu_popup(ctx) }
}

impl NodeConfigPopup for DeleteOutboundPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_delete_outbound_popup(ctx) }
}

impl NodeConfigPopup for DuplicateOutboundPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_duplicate_outbound_popup(ctx) }
}

impl NodeConfigPopup for DeleteRoutingRulePopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_delete_routing_rule_popup(ctx) }
}

impl NodeConfigPopup for WarpCreatePopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_warp_create_popup(ctx) }
}

impl NodeConfigPopup for AcmeConfirmPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_acme_confirm_popup(ctx) }
}

impl NodeConfigPopup for AcmeLogsPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_acme_logs_popup(ctx) }
}

impl NodeConfigPopup for DnsServerEditorPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_dns_server_editor_popup(ctx) }
}

impl NodeConfigPopup for DnsHostEditorPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_dns_host_editor_popup(ctx) }
}

impl NodeConfigPopup for CertificateEditorPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_certificate_editor_popup(ctx) }
}

impl NodeConfigPopup for InboundEditorPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_inbound_editor_popup(ctx) }
}

impl NodeConfigPopup for OutboundEditorPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_outbound_editor_popup(ctx) }
}

impl NodeConfigPopup for AccessLinkPopupSlot {
    fn render(&self, ctx: &PopupHostContext<'_>) -> Html { render_access_link_popup(ctx) }
}

fn popup_slots() -> Vec<Box<dyn NodeConfigPopup>> {
    vec![
        Box::new(RoutingRuleEditorPopupSlot),
        Box::new(RoutingRuleActionMenuPopupSlot),
        Box::new(ReverseProxyEditorPopupSlot),
        Box::new(ReverseProxyActionMenuPopupSlot),
        Box::new(DeleteReverseProxyPopupSlot),
        Box::new(DuplicateReverseProxyPopupSlot),
        Box::new(DeployConfirmPopupSlot),
        Box::new(DeployPreviewPopupSlot),
        Box::new(InboundActionMenuPopupSlot),
        Box::new(DeleteInboundPopupSlot),
        Box::new(DuplicateInboundPopupSlot),
        Box::new(OutboundActionMenuPopupSlot),
        Box::new(DeleteOutboundPopupSlot),
        Box::new(DuplicateOutboundPopupSlot),
        Box::new(DeleteRoutingRulePopupSlot),
        Box::new(WarpCreatePopupSlot),
        Box::new(AcmeConfirmPopupSlot),
        Box::new(AcmeLogsPopupSlot),
        Box::new(DnsServerEditorPopupSlot),
        Box::new(DnsHostEditorPopupSlot),
        Box::new(CertificateEditorPopupSlot),
        Box::new(InboundEditorPopupSlot),
        Box::new(OutboundEditorPopupSlot),
        Box::new(AccessLinkPopupSlot),
    ]
}

pub(super) fn render_popup_host(ctx: &PopupHostContext<'_>) -> Html {
    let slots = popup_slots();
    html! { { for slots.into_iter().map(|slot| slot.render(ctx)) } }
}

fn routing_outbound_options(draft: &NodeConfigDraft) -> Vec<DropdownOption> {
    let mut options = Vec::new();
    for outbound in &draft.outbounds {
        let tag = outbound_tag_for_routing(outbound);
        if tag.trim().is_empty() || options.iter().any(|option: &DropdownOption| option.value == tag) {
            continue;
        }
        options.push(DropdownOption {
            value: tag,
            label: outbound_label_for_routing(outbound),
        });
    }
    for (index, reverse_proxy) in draft.reverse_proxies.iter().enumerate() {
        let tag = reverse_proxy.tag.trim().to_string();
        if tag.is_empty() || options.iter().any(|option: &DropdownOption| option.value == tag) {
            continue;
        }
        options.push(DropdownOption {
            value: tag.clone(),
            label: format!("{} (VLESS Reverse)", reverse_proxy_display_name(reverse_proxy, index)),
        });
    }
    options
}

fn routing_inbound_options(draft: &NodeConfigDraft) -> Vec<String> {
    let mut options = draft
        .inbounds
        .iter()
        .map(|inbound| inbound.name.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    options.sort();
    options.dedup();
    options
}

fn routing_user_options(state: &State) -> Vec<DropdownOption> {
    let mut options = state
        .accounts
        .iter()
        .map(|account| {
            let id = account.id.trim().to_string();
            let name = account.name.trim();
            let label = if name.is_empty() || name == id {
                id.clone()
            } else {
                format!("{} ({})", id, name)
            };
            DropdownOption { value: id, label }
        })
        .filter(|option| !option.value.is_empty())
        .collect::<Vec<_>>();
    options.sort_by(|a, b| a.label.cmp(&b.label).then(a.value.cmp(&b.value)));
    options.dedup_by(|a, b| a.value == b.value);
    options
}

fn render_routing_rule_editor_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((rule_index, rule, is_new)) = &**ctx.editing_routing_rule {
        html! {
            <RoutingRuleEditorPopup
                rule={rule.clone()}
                is_new={*is_new}
                inbound_options={routing_inbound_options(ctx.draft_value)}
                user_options={routing_user_options(ctx.state)}
                outbound_options={routing_outbound_options(ctx.draft_value)}
                on_close={Callback::from({
                    let editing_routing_rule = ctx.editing_routing_rule.clone();
                    move |_| editing_routing_rule.set(None)
                })}
                on_save={Callback::from({
                    let draft = ctx.draft.clone();
                    let editing_routing_rule = ctx.editing_routing_rule.clone();
                    let rule_index = *rule_index;
                    move |rule: RoutingRuleDraft| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        if let Some(existing) = next.routing_rules.get_mut(rule_index) {
                            *existing = rule;
                        } else {
                            next.routing_rules.push(rule);
                        }
                        sync_draft(&mut next);
                        draft.set(next);
                        editing_routing_rule.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_routing_rule_action_menu_popup(ctx: &PopupHostContext<'_>) -> Html {
    let Some((rule_index, (left, top, width))) = (**ctx.action_routing_rule).clone() else {
        return html! {};
    };
    let Some(rule) = ctx.draft_value.routing_rules.get(rule_index).cloned() else {
        return html! {};
    };

    html! {
        <ActionMenuPopup
            anchor_left={left}
            anchor_top={top}
            anchor_width={width}
            on_close={Callback::from({
                let action_routing_rule = ctx.action_routing_rule.clone();
                move |_| action_routing_rule.set(None)
            })}
            on_edit={Some(Callback::from({
                let action_routing_rule = ctx.action_routing_rule.clone();
                let editing_routing_rule = ctx.editing_routing_rule.clone();
                let rule = rule.clone();
                move |_| {
                    action_routing_rule.set(None);
                    editing_routing_rule.set(Some((rule_index, rule.clone(), false)));
                }
            }))}
            on_duplicate={Some(Callback::from({
                let action_routing_rule = ctx.action_routing_rule.clone();
                let draft = ctx.draft.clone();
                let rule = rule.clone();
                move |_| {
                    action_routing_rule.set(None);
                    let mut next = (*draft).clone();
                    sync_draft(&mut next);
                    let insert_at = (rule_index + 1).min(next.routing_rules.len());
                    next.routing_rules.insert(insert_at, rule.clone());
                    sync_draft(&mut next);
                    draft.set(next);
                }
            }))}
            on_delete={Some(Callback::from({
                let action_routing_rule = ctx.action_routing_rule.clone();
                let pending_routing_delete = ctx.pending_routing_delete.clone();
                move |_| {
                    action_routing_rule.set(None);
                    pending_routing_delete.set(Some(rule_index));
                }
            }))}
        />
    }
}

fn render_reverse_proxy_editor_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((reverse_proxy_index, reverse_proxy, is_new)) = &**ctx.editing_reverse_proxy {
        html! {
            <ReverseProxyEditorPopup
                reverse_proxy={reverse_proxy.clone()}
                is_new={*is_new}
                inbound_options={routing_inbound_options(ctx.draft_value)}
                user_options={routing_user_options(ctx.state)}
                on_close={Callback::from({
                    let editing_reverse_proxy = ctx.editing_reverse_proxy.clone();
                    move |_| editing_reverse_proxy.set(None)
                })}
                on_save={Callback::from({
                    let draft = ctx.draft.clone();
                    let editing_reverse_proxy = ctx.editing_reverse_proxy.clone();
                    let reverse_proxy_index = *reverse_proxy_index;
                    move |reverse_proxy: ReverseProxyDraft| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        let mut reverse_proxy = reverse_proxy;
                        reverse_proxy.mode = "portal".to_string();
                        reverse_proxy.domain.clear();
                        reverse_proxy.bridge_outbound_tag.clear();
                        reverse_proxy.target_outbound_tag.clear();
                        if let Some(existing) = next.reverse_proxies.get_mut(reverse_proxy_index) {
                            *existing = reverse_proxy;
                        } else {
                            next.reverse_proxies.push(reverse_proxy);
                        }
                        sync_draft(&mut next);
                        draft.set(next);
                        editing_reverse_proxy.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_reverse_proxy_action_menu_popup(ctx: &PopupHostContext<'_>) -> Html {
    let Some((reverse_proxy_index, (left, top, width))) = (**ctx.action_reverse_proxy).clone() else {
        return html! {};
    };
    let Some(reverse_proxy) = ctx.draft_value.reverse_proxies.get(reverse_proxy_index).cloned() else {
        return html! {};
    };

    html! {
        <ActionMenuPopup
            anchor_left={left}
            anchor_top={top}
            anchor_width={width}
            on_close={Callback::from({
                let action_reverse_proxy = ctx.action_reverse_proxy.clone();
                move |_| action_reverse_proxy.set(None)
            })}
            on_edit={Some(Callback::from({
                let action_reverse_proxy = ctx.action_reverse_proxy.clone();
                let editing_reverse_proxy = ctx.editing_reverse_proxy.clone();
                let reverse_proxy = reverse_proxy.clone();
                move |_| {
                    action_reverse_proxy.set(None);
                    editing_reverse_proxy.set(Some((reverse_proxy_index, reverse_proxy.clone(), false)));
                }
            }))}
            on_duplicate={Some(Callback::from({
                let action_reverse_proxy = ctx.action_reverse_proxy.clone();
                let pending_duplicate_reverse_proxy = ctx.pending_duplicate_reverse_proxy.clone();
                let reverse_proxy = reverse_proxy.clone();
                move |_| {
                    action_reverse_proxy.set(None);
                    pending_duplicate_reverse_proxy.set(Some(reverse_proxy.clone()));
                }
            }))}
            on_delete={Some(Callback::from({
                let action_reverse_proxy = ctx.action_reverse_proxy.clone();
                let pending_reverse_proxy_delete = ctx.pending_reverse_proxy_delete.clone();
                let reverse_proxy_name = reverse_proxy_display_name(&reverse_proxy, reverse_proxy_index);
                move |_| {
                    action_reverse_proxy.set(None);
                    pending_reverse_proxy_delete.set(Some((reverse_proxy_index, reverse_proxy_name.clone())));
                }
            }))}
        />
    }
}

fn render_delete_reverse_proxy_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((reverse_proxy_index, reverse_proxy_name)) = (**ctx.pending_reverse_proxy_delete).clone() {
        html! {
            <ConfirmPopup
                title="Delete VLESS Reverse"
                body={format!("Are you sure you want to delete VLESS reverse \"{}\"?", reverse_proxy_name)}
                confirm_label="Delete"
                align_actions_end={true}
                on_close={Callback::from({
                    let pending_reverse_proxy_delete = ctx.pending_reverse_proxy_delete.clone();
                    move |_| pending_reverse_proxy_delete.set(None)
                })}
                on_confirm={Callback::from({
                    let draft = ctx.draft.clone();
                    let pending_reverse_proxy_delete = ctx.pending_reverse_proxy_delete.clone();
                    move |_| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        if reverse_proxy_index < next.reverse_proxies.len() {
                            next.reverse_proxies.remove(reverse_proxy_index);
                        }
                        sync_draft(&mut next);
                        draft.set(next);
                        pending_reverse_proxy_delete.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_duplicate_reverse_proxy_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some(reverse_proxy) = (**ctx.pending_duplicate_reverse_proxy).clone() {
        let base_tag = reverse_proxy.tag.trim();
        let initial_tag = if base_tag.is_empty() {
            "reverse-copy".to_string()
        } else {
            format!("{}-copy", base_tag)
        };
        html! {
            <NamePromptPopup
                title="Duplicate VLESS Reverse"
                label="New reverse tag"
                confirm_label="Duplicate"
                initial_value={initial_tag}
                on_close={Callback::from({
                    let pending_duplicate_reverse_proxy = ctx.pending_duplicate_reverse_proxy.clone();
                    move |_| pending_duplicate_reverse_proxy.set(None)
                })}
                on_confirm={Callback::from({
                    let draft = ctx.draft.clone();
                    let pending_duplicate_reverse_proxy = ctx.pending_duplicate_reverse_proxy.clone();
                    move |name: String| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        let mut duplicated = reverse_proxy.clone();
                        duplicated.tag = name;
                        next.reverse_proxies.push(duplicated);
                        sync_draft(&mut next);
                        draft.set(next);
                        pending_duplicate_reverse_proxy.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_deploy_confirm_popup(ctx: &PopupHostContext<'_>) -> Html {
    if **ctx.deploy_confirm_open {
        let on_deploy_preview_click = {
            let deploy_preview_json = ctx.deploy_preview_json.clone();
            let draft = ctx.draft.clone();
            let state = ctx.state.clone();
            let node_id = ctx.node.id.clone();
            let node_for_deploy = ctx.node.clone();
            Callback::from(move |_| {
                let mut draft_value = (*draft).clone();
                sync_draft(&mut draft_value);
                if let Some(current_node) = state.nodes.iter().find(|node| node.id == node_id) {
                    draft_value.master_key = current_node.master_key.clone();
                }
                let config = build_full_config(&draft_value, &node_for_deploy, &(*state).accounts);
                deploy_preview_json.set(Some(full_config_to_pretty_json(&config)));
            })
        };

        html! {
            <ConfirmPopup
                title="Deploy Revision"
                body="Deploy current draft to this node now? This overwrites active runtime configuration."
                confirm_label="Deploy"
                extra_label={Some(AttrValue::from("Preview"))}
                align_actions_end={true}
                on_close={Callback::from({
                    let deploy_confirm_open = ctx.deploy_confirm_open.clone();
                    move |_| deploy_confirm_open.set(false)
                })}
                on_extra={Some(Callback::from(move |_| on_deploy_preview_click.emit(())))}
                on_confirm={Callback::from({
                    let deploy_confirm_open = ctx.deploy_confirm_open.clone();
                    let deploy_revision = ctx.deploy_revision.clone();
                    move |_| {
                        deploy_confirm_open.set(false);
                        deploy_revision.emit(());
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_deploy_preview_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some(preview_json) = (**ctx.deploy_preview_json).clone() {
        html! {
            <Popup
                title="Deploy Preview"
                size={PopupSize::Lg}
                on_close={Callback::from({
                    let deploy_preview_json = ctx.deploy_preview_json.clone();
                    move |_| deploy_preview_json.set(None)
                })}
            >
                <div class="space-y-4">
                    <TextBox
                        label="Proto Config JSON"
                        value={preview_json}
                        onchange={Callback::from(|_: String| {})}
                        is_textarea={true}
                    />
                    <div class="md3-popup-actions" style="justify-content: flex-end;">
                        <Button
                            label="Close"
                            button_type={ButtonType::Filled}
                            onclick={Callback::from({
                                let deploy_preview_json = ctx.deploy_preview_json.clone();
                                move |_| deploy_preview_json.set(None)
                            })}
                        />
                    </div>
                </div>
            </Popup>
        }
    } else {
        html! {}
    }
}

fn render_delete_inbound_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((inbound_id, inbound_name)) = (**ctx.pending_inbound_delete).clone() {
        html! {
            <ConfirmPopup
                title="Delete Inbound"
                body={format!("Are you sure you want to delete inbound \"{}\"?", inbound_name)}
                confirm_label="Delete"
                align_actions_end={true}
                on_close={Callback::from({
                    let pending_inbound_delete = ctx.pending_inbound_delete.clone();
                    move |_| pending_inbound_delete.set(None)
                })}
                on_confirm={Callback::from({
                    let draft = ctx.draft.clone();
                    let pending_inbound_delete = ctx.pending_inbound_delete.clone();
                    move |_| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        next.inbounds.retain(|item| item.id != inbound_id);
                        sync_draft(&mut next);
                        draft.set(next);
                        pending_inbound_delete.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_inbound_action_menu_popup(ctx: &PopupHostContext<'_>) -> Html {
    let Some((inbound_id, (left, top, width))) = (**ctx.action_inbound).clone() else {
        return html! {};
    };
    let Some(inbound) = ctx.draft_value.inbounds.iter().find(|item| item.id == inbound_id).cloned() else {
        return html! {};
    };

    html! {
        <ActionMenuPopup
            anchor_left={left}
            anchor_top={top}
            anchor_width={width}
            on_close={Callback::from({
                let action_inbound = ctx.action_inbound.clone();
                move |_| action_inbound.set(None)
            })}
            on_edit={Some(Callback::from({
                let action_inbound = ctx.action_inbound.clone();
                let editing_inbound = ctx.editing_inbound.clone();
                let inbound = inbound.clone();
                move |_| {
                    action_inbound.set(None);
                    editing_inbound.set(Some((inbound.clone(), false)));
                }
            }))}
            on_duplicate={Some(Callback::from({
                let action_inbound = ctx.action_inbound.clone();
                let pending_duplicate_inbound = ctx.pending_duplicate_inbound.clone();
                let inbound = inbound.clone();
                move |_| {
                    action_inbound.set(None);
                    pending_duplicate_inbound.set(Some(inbound.clone()));
                }
            }))}
            on_delete={Some(Callback::from({
                let action_inbound = ctx.action_inbound.clone();
                let pending_inbound_delete = ctx.pending_inbound_delete.clone();
                let inbound_id = inbound.id.clone();
                let inbound_name = inbound_display_name(&inbound);
                move |_| {
                    action_inbound.set(None);
                    pending_inbound_delete.set(Some((inbound_id.clone(), inbound_name.clone())));
                }
            }))}
        />
    }
}

fn render_delete_outbound_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((outbound_id, outbound_name)) = (**ctx.pending_outbound_delete).clone() {
        html! {
            <ConfirmPopup
                title="Delete Outbound"
                body={format!("Are you sure you want to delete outbound \"{}\"?", outbound_name)}
                confirm_label="Delete"
                align_actions_end={true}
                on_close={Callback::from({
                    let pending_outbound_delete = ctx.pending_outbound_delete.clone();
                    move |_| pending_outbound_delete.set(None)
                })}
                on_confirm={Callback::from({
                    let draft = ctx.draft.clone();
                    let pending_outbound_delete = ctx.pending_outbound_delete.clone();
                    move |_| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        next.outbounds.retain(|item| item.id != outbound_id);
                        sync_draft(&mut next);
                        draft.set(next);
                        pending_outbound_delete.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_outbound_action_menu_popup(ctx: &PopupHostContext<'_>) -> Html {
    let Some((outbound_id, (left, top, width))) = (**ctx.action_outbound).clone() else {
        return html! {};
    };
    let Some(outbound) = ctx.draft_value.outbounds.iter().find(|item| item.id == outbound_id).cloned() else {
        return html! {};
    };

    html! {
        <ActionMenuPopup
            anchor_left={left}
            anchor_top={top}
            anchor_width={width}
            on_close={Callback::from({
                let action_outbound = ctx.action_outbound.clone();
                move |_| action_outbound.set(None)
            })}
            on_edit={Some(Callback::from({
                let action_outbound = ctx.action_outbound.clone();
                let editing_outbound = ctx.editing_outbound.clone();
                let outbound = outbound.clone();
                move |_| {
                    action_outbound.set(None);
                    editing_outbound.set(Some((outbound.clone(), false)));
                }
            }))}
            on_duplicate={(!outbound.builtin).then_some(Callback::from({
                let action_outbound = ctx.action_outbound.clone();
                let pending_duplicate_outbound = ctx.pending_duplicate_outbound.clone();
                let outbound = outbound.clone();
                move |_| {
                    action_outbound.set(None);
                    pending_duplicate_outbound.set(Some(outbound.clone()));
                }
            }))}
            on_delete={(!outbound.builtin).then_some(Callback::from({
                let action_outbound = ctx.action_outbound.clone();
                let pending_outbound_delete = ctx.pending_outbound_delete.clone();
                let outbound_id = outbound.id.clone();
                let outbound_name = outbound.name.clone();
                move |_| {
                    action_outbound.set(None);
                    pending_outbound_delete.set(Some((outbound_id.clone(), outbound_name.clone())));
                }
            }))}
        />
    }
}

fn render_duplicate_inbound_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some(inbound) = (**ctx.pending_duplicate_inbound).clone() {
        let initial_name = format!("{} Copy", inbound_display_name(&inbound));
        html! {
            <NamePromptPopup
                title="Duplicate Inbound"
                label="New inbound name"
                confirm_label="Duplicate"
                initial_value={initial_name}
                on_close={Callback::from({
                    let pending_duplicate_inbound = ctx.pending_duplicate_inbound.clone();
                    move |_| pending_duplicate_inbound.set(None)
                })}
                on_confirm={Callback::from({
                    let draft = ctx.draft.clone();
                    let pending_duplicate_inbound = ctx.pending_duplicate_inbound.clone();
                    move |name: String| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        let mut duplicated = inbound.clone();
                        duplicated.id = uuid::Uuid::new_v4().to_string();
                        duplicated.name = name;
                        next.inbounds.push(duplicated);
                        sync_draft(&mut next);
                        draft.set(next);
                        pending_duplicate_inbound.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_duplicate_outbound_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some(outbound) = (**ctx.pending_duplicate_outbound).clone() {
        let initial_name = format!("{} Copy", outbound.name.trim());
        html! {
            <NamePromptPopup
                title="Duplicate Outbound"
                label="New outbound name"
                confirm_label="Duplicate"
                initial_value={initial_name}
                on_close={Callback::from({
                    let pending_duplicate_outbound = ctx.pending_duplicate_outbound.clone();
                    move |_| pending_duplicate_outbound.set(None)
                })}
                on_confirm={Callback::from({
                    let draft = ctx.draft.clone();
                    let pending_duplicate_outbound = ctx.pending_duplicate_outbound.clone();
                    move |name: String| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        let mut duplicated = outbound.clone();
                        duplicated.id = uuid::Uuid::new_v4().to_string();
                        duplicated.name = name.clone();
                        match duplicated.outbound_type.trim().to_uppercase().as_str() {
                            "VLESS" => duplicated.vless.tag = name,
                            "TRUSTTUNNEL" => duplicated.trust_tunnel.tag = name,
                            "WIREGUARD" => duplicated.wireguard.tag = name,
                            "CUSTOM" => duplicated.custom.tag = name,
                            "SOCKS5" => duplicated.socks5.tag = name,
                            "SHADOWSOCKS" => duplicated.shadowsocks.tag = name,
                            "TROJAN" => duplicated.trojan.tag = name,
                            _ => {}
                        }
                        next.outbounds.push(duplicated);
                        sync_draft(&mut next);
                        draft.set(next);
                        pending_duplicate_outbound.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_delete_routing_rule_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some(rule_index) = **ctx.pending_routing_delete {
        html! {
            <ConfirmPopup
                title="Delete Rule"
                body="Are you sure you want to delete this routing rule?"
                confirm_label="Delete"
                align_actions_end={true}
                on_close={Callback::from({
                    let pending_routing_delete = ctx.pending_routing_delete.clone();
                    move |_| pending_routing_delete.set(None)
                })}
                on_confirm={Callback::from({
                    let draft = ctx.draft.clone();
                    let pending_routing_delete = ctx.pending_routing_delete.clone();
                    move |_| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        if rule_index < next.routing_rules.len() {
                            next.routing_rules.remove(rule_index);
                        }
                        sync_draft(&mut next);
                        draft.set(next);
                        pending_routing_delete.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_warp_create_popup(ctx: &PopupHostContext<'_>) -> Html {
    if **ctx.warp_popup_open {
        html! {
            <WarpCreatePopup
                node_address={ctx.node.address.clone()}
                master_key={ctx.node.master_key.clone()}
                initial_registration={initial_warp_registration(ctx.draft_value)}
                on_registration_change={Callback::from({
                    let draft = ctx.draft.clone();
                    move |registration: Option<crate::services::warp::WarpRegistration>| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        let next_registration = registration
                            .as_ref()
                            .map(warp_registration_to_draft)
                            .unwrap_or_default();
                        if next.warp_registration != next_registration {
                            next.warp_registration = next_registration;
                            sync_draft(&mut next);
                            draft.set(next);
                        }
                    }
                })}
                on_close={Callback::from({
                    let warp_popup_open = ctx.warp_popup_open.clone();
                    move |_| warp_popup_open.set(false)
                })}
                on_create={Callback::from({
                    let draft = ctx.draft.clone();
                    let warp_popup_open = ctx.warp_popup_open.clone();
                    let snackbar = ctx.snackbar.clone();
                    move |outbound: OutboundEntryDraft| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        next.outbounds.push(outbound);
                        sync_draft(&mut next);
                        draft.set(next);
                        warp_popup_open.set(false);
                        if let Some(bus) = &snackbar {
                            bus.push("WireGuard outbound created from WARP account");
                        }
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_acme_confirm_popup(ctx: &PopupHostContext<'_>) -> Html {
    if **ctx.acme_confirm_open {
        html! {
            <ConfirmPopup
                title="Request Certificate"
                body="This will trigger ACME issuance on node. Certificate authorities enforce rate limits, so repeated failed attempts can temporarily block new issuance. Continue only if domain, ports, and challenge routing are ready."
                confirm_label="Request"
                on_close={Callback::from({
                    let acme_confirm_open = ctx.acme_confirm_open.clone();
                    let pending_acme_certificate = ctx.pending_acme_certificate.clone();
                    move |_| {
                        acme_confirm_open.set(false);
                        pending_acme_certificate.set(None);
                    }
                })}
                on_confirm={Callback::from({
                    let acme_confirm_open = ctx.acme_confirm_open.clone();
                    let pending_acme_certificate = ctx.pending_acme_certificate.clone();
                    let acme_logs = ctx.acme_logs.clone();
                    let acme_logs_open = ctx.acme_logs_open.clone();
                    let acme_loading = ctx.acme_loading.clone();
                    let node = ctx.node.clone();
                    let draft = ctx.draft.clone();
                    move |_| {
                        let Some(selected_certificate) = (*pending_acme_certificate).clone() else {
                            acme_confirm_open.set(false);
                            return;
                        };
                        acme_confirm_open.set(false);
                        pending_acme_certificate.set(None);
                        acme_logs_open.set(true);
                        acme_loading.set(true);
                        acme_logs.set(Some(AcmeIssueResponse {
                            success: false,
                            error: String::new(),
                            logs: vec!["Sending ACME request to node...".to_string()],
                            expiry_time: 0,
                        }));

                        let api = ApiService::new(node.address.clone());
                        let draft_value = (*draft).clone();
                        let acme_logs = acme_logs.clone();
                        let acme_loading = acme_loading.clone();
                        let draft = draft.clone();
                        spawn_local(async move {
                            let challenge_port = if selected_certificate.acme_type.eq_ignore_ascii_case("HTTP") {
                                selected_certificate.acme_http_port
                            } else {
                                selected_certificate.acme_port
                            };
                            let response = api.issue_acme_certificate(AcmeIssueRequest {
                                master_key: draft_value.master_key.clone(),
                                email: selected_certificate.acme_email.clone(),
                                domain: selected_certificate.acme_domain.clone(),
                                challenge_type: selected_certificate.acme_type.clone(),
                                ca: selected_certificate.acme_ca.clone(),
                                port: challenge_port,
                                certificate_path: selected_certificate.certificate_path.clone(),
                                key_path: selected_certificate.key_path.clone(),
                            }).await;

                            acme_loading.set(false);
                            match response {
                                Ok(result) => {
                                    let mut next = (*draft).clone();
                                    sync_draft(&mut next);
                                    if let Some(certificate) = next.certificates.iter_mut().find(|item| item.id == selected_certificate.id) {
                                        let (certificate_path, key_path) = certmagic_certificate_paths(
                                            &certificate.acme_ca,
                                            &certificate.acme_domain,
                                        );
                                        certificate.certificate_path = certificate_path;
                                        certificate.key_path = key_path;
                                        if result.expiry_time > 0 {
                                            certificate.expiry_time = result.expiry_time;
                                        }
                                    }
                                    draft.set(next);
                                    acme_logs.set(Some(result));
                                }
                                Err(error) => acme_logs.set(Some(AcmeIssueResponse {
                                    success: false,
                                    error: error.clone(),
                                    logs: vec![
                                        "Sending ACME request to node...".to_string(),
                                        format!("Request failed: {}", error),
                                    ],
                                    expiry_time: 0,
                                })),
                            }
                        });
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_acme_logs_popup(ctx: &PopupHostContext<'_>) -> Html {
    if **ctx.acme_logs_open {
        if let Some(result) = &**ctx.acme_logs {
            html! {
                <AcmeLogsPopup
                    title="ACME Logs"
                    logs={result.logs.clone()}
                    loading={**ctx.acme_loading}
                    success={result.success}
                    error={result.error.clone()}
                    on_close={Callback::from({
                        let acme_logs_open = ctx.acme_logs_open.clone();
                        move |_| acme_logs_open.set(false)
                    })}
                />
            }
        } else {
            html! {}
        }
    } else {
        html! {}
    }
}

fn render_dns_server_editor_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((server_index, server, is_new)) = &**ctx.editing_dns_server {
        html! {
            <DnsServerEditorPopup
                server={server.clone()}
                is_new={*is_new}
                on_close={Callback::from({
                    let editing_dns_server = ctx.editing_dns_server.clone();
                    move |_| editing_dns_server.set(None)
                })}
                on_save={Callback::from({
                    let draft = ctx.draft.clone();
                    let editing_dns_server = ctx.editing_dns_server.clone();
                    let server_index = *server_index;
                    move |server: DnsServerDraft| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        if server_index < next.dns.servers.len() {
                            next.dns.servers[server_index] = server.clone();
                        } else {
                            next.dns.servers.push(server);
                        }
                        draft.set(next);
                        editing_dns_server.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_dns_host_editor_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((host_index, host, is_new)) = &**ctx.editing_dns_host {
        html! {
            <DnsHostEditorPopup
                host={host.clone()}
                is_new={*is_new}
                on_close={Callback::from({
                    let editing_dns_host = ctx.editing_dns_host.clone();
                    move |_| editing_dns_host.set(None)
                })}
                on_save={Callback::from({
                    let draft = ctx.draft.clone();
                    let editing_dns_host = ctx.editing_dns_host.clone();
                    let host_index = *host_index;
                    move |host: DnsHostDraft| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        if host_index < next.dns.hosts.len() {
                            next.dns.hosts[host_index] = host.clone();
                        } else {
                            next.dns.hosts.push(host);
                        }
                        draft.set(next);
                        editing_dns_host.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_certificate_editor_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((certificate, is_new)) = &**ctx.editing_certificate {
        html! {
            <CertificateEditorPopup
                certificate={certificate.clone()}
                is_new={*is_new}
                on_close={Callback::from({
                    let editing_certificate = ctx.editing_certificate.clone();
                    move |_| editing_certificate.set(None)
                })}
                on_save={Callback::from({
                    let draft = ctx.draft.clone();
                    let editing_certificate = ctx.editing_certificate.clone();
                    move |certificate: CertificateDraft| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        if let Some(existing) = next.certificates.iter_mut().find(|item| item.id == certificate.id) {
                            let previous_name = existing.name.clone();
                            *existing = certificate.clone();
                            if previous_name != certificate.name {
                                for inbound in next.inbounds.iter_mut() {
                                    if inbound.tls.certificate_name == previous_name {
                                        inbound.tls.certificate_name = certificate.name.clone();
                                    }
                                }
                            }
                        } else {
                            next.certificates.push(certificate.clone());
                            for inbound in next.inbounds.iter_mut() {
                                if inbound.tls.certificate_name.trim().is_empty() {
                                    inbound.tls.certificate_name = certificate.name.clone();
                                }
                            }
                        }
                        draft.set(next);
                        editing_certificate.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_inbound_editor_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((inbound, is_new)) = &**ctx.editing_inbound {
        html! {
            <InboundEditorPopup
                inbound={inbound.clone()}
                certificates={ctx.draft_value.certificates.clone()}
                is_new={*is_new}
                on_close={Callback::from({
                    let editing_inbound = ctx.editing_inbound.clone();
                    move |_| editing_inbound.set(None)
                })}
                on_save={Callback::from({
                    let draft = ctx.draft.clone();
                    let editing_inbound = ctx.editing_inbound.clone();
                    move |inbound: InboundEntryDraft| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        if let Some(existing) = next.inbounds.iter_mut().find(|item| item.id == inbound.id) {
                            *existing = inbound;
                        } else {
                            next.inbounds.push(inbound);
                        }
                        sync_draft(&mut next);
                        draft.set(next);
                        editing_inbound.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_outbound_editor_popup(ctx: &PopupHostContext<'_>) -> Html {
    if let Some((outbound, is_new)) = &**ctx.editing_outbound {
        html! {
            <OutboundEditorPopup
                outbound={outbound.clone()}
                is_new={*is_new}
                node_address={ctx.node.address.clone()}
                master_key={ctx.node.master_key.clone()}
                on_close={Callback::from({
                    let editing_outbound = ctx.editing_outbound.clone();
                    move |_| editing_outbound.set(None)
                })}
                on_save={Callback::from({
                    let draft = ctx.draft.clone();
                    let editing_outbound = ctx.editing_outbound.clone();
                    move |outbound: OutboundEntryDraft| {
                        let mut next = (*draft).clone();
                        sync_draft(&mut next);
                        if let Some(existing) = next.outbounds.iter_mut().find(|item| item.id == outbound.id) {
                            *existing = outbound;
                        } else {
                            next.outbounds.push(outbound);
                        }
                        sync_draft(&mut next);
                        draft.set(next);
                        editing_outbound.set(None);
                    }
                })}
            />
        }
    } else {
        html! {}
    }
}

fn render_access_link_popup(ctx: &PopupHostContext<'_>) -> Html {
    let selected_access_inbound = (**ctx.access_link_inbound_id)
        .clone()
        .and_then(|id| ctx.draft_value.inbounds.iter().find(|inbound| inbound.id == id).cloned());

    if let Some(inbound) = selected_access_inbound {
        html! {
            <AccessLinkPopup
                node={ctx.node.clone()}
                inbound={inbound}
                accounts={ctx.state.accounts.clone()}
                on_close={Callback::from({
                    let access_link_inbound_id = ctx.access_link_inbound_id.clone();
                    move |_| access_link_inbound_id.set(None)
                })}
            />
        }
    } else {
        html! {}
    }
}
