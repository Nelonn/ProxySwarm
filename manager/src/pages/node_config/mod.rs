use base64::Engine;
use gloo_timers::callback::{Interval, Timeout};
use gloo_timers::future::TimeoutFuture;
use js_sys::Date as JsDate;
use qrcodegen::{QrCode, QrCodeEcc};
use trusttunnel_deeplink::{encode, DeepLinkConfig};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::window;
use x25519_dalek::{PublicKey, StaticSecret};
use yew::prelude::*;
use yew_router::prelude::use_navigator;

use crate::components::{
    ActionMenuPopup, Button, ButtonType, Chip, ChipMode, Dropdown, DropdownOption, IconButton,
    Popup, PopupSize, RichTable, SnackbarBus, SvgIcon, Switch, SwitchField, TextBox,
    WideNavigationBar, WideNavigationBarItem,
};
use crate::pb::proxyswarm::{
    outbound_config, Account, AccountStatus, AcmeCertificateConfig, CertificateConfig, CoreType,
    CustomCertificateConfig, DnsConfig, DnsHostMapping, DnsServerConfig, FullConfig,
    Hysteria2Config, InboundConfig, InboundStatus, NaiveProxyConfig, NodeStatus, OutboundConfig,
    OutboundStatus, OutboundType, RoutingRule, SecurityMode, ShadowsocksInboundConfig,
    ShadowsocksOutboundConfig, Socks5InboundConfig, Socks5OutboundConfig, TlsConfig, TrafficStats, TrustTunnelConfig, VlessConfig,
    VlessOutboundConfig, VlessRealityConfig, WireGuardConfig, WireGuardPeer,
    TrojanInboundConfig, TrojanOutboundConfig, ReverseProxyConfig, TProxyConfig,
};
use crate::services::node_api::{AcmeIssueRequest, AcmeIssueResponse};
use crate::services::warp::{
    generate_wireguard_keypair, register_warp_with_keypair, update_warp_license,
};
use crate::services::ApiService;
use crate::state::{
    default_link_remark_template, format_link_remark, normalize_groups, AccountInfo,
    CertificateDraft, DnsDraft, DnsHostDraft, DnsServerDraft, Hysteria2Draft, InboundEntryDraft,
    NaiveProxyDraft, NodeConfigDraft, NodeConfigRevision, OutboundEntryDraft, ProxyNode,
    RoutingRuleDraft, ShadowsocksDraft, Socks5Draft, State, TlsDraft, TrustTunnelDraft,
    TrustTunnelOutboundDraft, VlessInboundDraft, VlessOutboundDraft, WarpRegistrationDraft,
    WireGuardDraft, WireGuardPeerItem, TrojanDraft, ReverseProxyDraft, TProxyDraft,
};
use crate::storage;
use crate::Route;

#[derive(Properties, PartialEq)]
pub struct NodeConfigPageProps {
    pub id: String,
}

#[derive(Clone, PartialEq)]
enum ConfigTab {
    Inbounds,
    Outbounds,
    Routing,
    Settings,
    Status,
}

mod inbounds;
mod outbounds;
mod routing;
mod settings;
mod status;
mod helpers;
mod status_widgets;
mod config_build;
mod access_links;
mod dns_editors;
mod popups;
mod review;
mod certificate_editor;
mod inbound_editor;
mod outbound_editor;
mod access_link_popup;
mod routing_rule_editor;
mod popup_host;

use access_link_popup::*;
use access_links::*;
use certificate_editor::*;
use config_build::*;
use dns_editors::*;
use helpers::*;
use inbound_editor::*;
use outbound_editor::*;
use popup_host::*;
use popups::*;
use review::*;
use routing_rule_editor::*;
use status_widgets::*;

pub(super) use crate::components::menu_anchor_from_mouse_event;

fn nav_key(tab: &ConfigTab) -> AttrValue {
    match tab {
        ConfigTab::Inbounds => "inbounds".into(),
        ConfigTab::Outbounds => "outbounds".into(),
        ConfigTab::Routing => "routing".into(),
        ConfigTab::Settings => "settings".into(),
        ConfigTab::Status => "status".into(),
    }
}

fn nav_items() -> Vec<WideNavigationBarItem> {
    vec![
        WideNavigationBarItem {
            value: "status".into(),
            label: "Status".into(),
            icon_name: "icon-bar-chart-4".into(),
        },
        WideNavigationBarItem {
            value: "inbounds".into(),
            label: "Inbounds".into(),
            icon_name: "icon-call-received".into(),
        },
        WideNavigationBarItem {
            value: "outbounds".into(),
            label: "Outbounds".into(),
            icon_name: "icon-call-made".into(),
        },
        WideNavigationBarItem {
            value: "routing".into(),
            label: "Routing".into(),
            icon_name: "icon-call-split".into(),
        },
        WideNavigationBarItem {
            value: "settings".into(),
            label: "Settings".into(),
            icon_name: "icon-settings".into(),
        },
    ]
}


fn persist_revision(
    state: &UseStateHandle<State>,
    node_id: &str,
    draft: &NodeConfigDraft,
) -> Option<String> {
    let mut next_state = (**state).clone();
    let node = next_state
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id)?;
    let revision_id = uuid::Uuid::new_v4().to_string();
    let mut persisted_draft = draft.clone();
    persisted_draft.master_key = node.master_key.clone();
    sync_draft(&mut persisted_draft);
    let revision = NodeConfigRevision {
        id: revision_id.clone(),
        created_at: today_string(),
        config: persisted_draft.clone(),
    };
    node.revisions.push(revision);
    node.active_revision_id = revision_id.clone();
    node.config = persisted_draft;

    next_state.save();
    state.set(next_state);
    Some(revision_id)
}

#[function_component(NodeConfigPage)]
pub fn node_config_page(props: &NodeConfigPageProps) -> Html {
    let snackbar = use_context::<SnackbarBus>();
    let state = use_context::<UseStateHandle<State>>().expect("state context");
    let navigator = use_navigator();
    let node_id_for_rev = props.id.clone();
    let state_for_rev = state.clone();
    let selected_revision_id = use_state(move || {
        if let Some(node) = state_for_rev.nodes.iter().find(|n| n.id == node_id_for_rev) {
            return node.active_revision_id.clone();
        }
        String::new()
    });
    let node_id_for_init = props.id.clone();
    let state_for_init = state.clone();
    let draft = use_state(move || {
        if let Some(node) = state_for_init
            .nodes
            .iter()
            .find(|n| n.id == node_id_for_init)
        {
            if let Some(mut saved_draft) = storage::load_node_draft_local(&node.id) {
                saved_draft.master_key = node.master_key.clone();
                sync_draft(&mut saved_draft);
                return saved_draft;
            }
            if !node.config.inbounds.is_empty() || !node.config.outbounds.is_empty() {
                return node.config.clone();
            }
            if let Some(revision) = node
                .revisions
                .iter()
                .find(|revision| revision.id == node.active_revision_id)
                .or_else(|| node.revisions.last())
            {
                let mut next_draft = revision.config.clone();
                next_draft.master_key = node.master_key.clone();
                sync_draft(&mut next_draft);
                return next_draft;
            }
            let mut next_draft = default_node_draft(node);
            next_draft.master_key = node.master_key.clone();
            return next_draft;
        }
        NodeConfigDraft::default()
    });
    let active_tab = use_state(|| ConfigTab::Status);
    let editing_inbound = use_state(|| Option::<(InboundEntryDraft, bool)>::None);
    let editing_certificate = use_state(|| Option::<(CertificateDraft, bool)>::None);
    let editing_outbound = use_state(|| Option::<(OutboundEntryDraft, bool)>::None);
    let editing_dns_server = use_state(|| Option::<(usize, DnsServerDraft, bool)>::None);
    let editing_dns_host = use_state(|| Option::<(usize, DnsHostDraft, bool)>::None);
    let editing_routing_rule = use_state(|| Option::<(usize, RoutingRuleDraft, bool)>::None);
    let pending_routing_delete = use_state(|| Option::<usize>::None);
    let action_inbound = use_state(|| Option::<(String, (f64, f64, f64))>::None);
    let pending_inbound_delete = use_state(|| Option::<(String, String)>::None);
    let pending_duplicate_inbound = use_state(|| Option::<InboundEntryDraft>::None);
    let action_outbound = use_state(|| Option::<(String, (f64, f64, f64))>::None);
    let pending_outbound_delete = use_state(|| Option::<(String, String)>::None);
    let pending_duplicate_outbound = use_state(|| Option::<OutboundEntryDraft>::None);
    let routing_move_anim = use_state(|| Option::<(usize, bool)>::None);
    let warp_popup_open = use_state(|| false);
    let access_link_inbound_id = use_state(|| Option::<String>::None);
    let deploy_confirm_open = use_state(|| false);
    let deploy_preview_json = use_state(|| None::<String>);
    let acme_confirm_open = use_state(|| false);
    let pending_acme_certificate = use_state(|| Option::<CertificateDraft>::None);
    let acme_logs = use_state(|| Option::<AcmeIssueResponse>::None);
    let acme_logs_open = use_state(|| false);
    let acme_loading = use_state(|| false);
    let live_status = use_state(|| Option::<NodeStatus>::None);
    let live_status_loading = use_state(|| false);
    let live_status_error = use_state(|| Option::<String>::None);
    let status_auto_refresh = use_state(|| true);
    let status_refresh_interval_ms = use_state(|| 2000u32);
    let status_refresh_menu_open = use_state(|| false);

    let node = state.nodes.iter().find(|node| node.id == props.id).cloned();

    {
        let draft = draft.clone();
        let node_id = props.id.clone();
        use_effect_with(
            (node_id.clone(), (*draft).clone()),
            move |(node_id, draft_value)| {
                storage::save_node_draft(node_id, draft_value);
                || ()
            },
        );
    }

    {
        let draft = draft.clone();
        let node_id = props.id.clone();
        use_effect_with(node_id.clone(), move |node_id| {
            storage::hydrate_desktop_node_draft(node_id.clone(), draft.clone());
            || ()
        });
    }

    let Some(node) = node else {
        return html! {
            <div class="p-6 space-y-6">
                <div class="flex justify-between" style="align-items: baseline;">
                    <Button
                        label="Back"
                        button_type={ButtonType::Text}
                        onclick={Callback::from(move |_| {
                            if let Some(navigator) = navigator.clone() {
                                navigator.push(&Route::Nodes);
                            }
                        })}
                    />
                </div>
                <div class="md3-card">
                    <h1 class="text-2xl font-bold">{ "Node not found" }</h1>
                </div>
            </div>
        };
    };

    let revision_options = if node.revisions.is_empty() {
        vec![("".to_string(), format!("Revision 1 ({})", today_string()))]
    } else {
        node.revisions
            .iter()
            .enumerate()
            .rev()
            .map(|(index, revision)| (revision.id.clone(), revision_label(index, revision)))
            .collect::<Vec<_>>()
    };

    let on_revision_change = {
        let selected_revision_id = selected_revision_id.clone();
        let draft = draft.clone();
        let node = node.clone();
        Callback::from(move |value: String| {
            selected_revision_id.set(value.clone());
            if let Some(revision) = node.revisions.iter().find(|revision| revision.id == value) {
                let mut next_draft = revision.config.clone();
                sync_draft(&mut next_draft);
                draft.set(next_draft);
            }
        })
    };

    let save_revision = {
        let state = state.clone();
        let draft = draft.clone();
        let selected_revision_id = selected_revision_id.clone();
        let snackbar = snackbar.clone();
        let node_id = node.id.clone();
        Callback::from(move |_| {
            if let Some(revision_id) = persist_revision(&state, &node_id, &draft) {
                selected_revision_id.set(revision_id);
                if let Some(bus) = &snackbar {
                    bus.push("Created revision");
                }
            }
        })
    };

    let deploy_revision = {
        let state = state.clone();
        let draft = draft.clone();
        let snackbar = snackbar.clone();
        let node_id = node.id.clone();
        let node_for_deploy = node.clone();
        let address = node.address.clone();
        Callback::from(move |_: ()| {
            let mut draft_value = (*draft).clone();
            sync_draft(&mut draft_value);
            if let Some(current_node) = state.nodes.iter().find(|node| node.id == node_id) {
                draft_value.master_key = current_node.master_key.clone();
            }
            let address = address.clone();
            let accounts = (*state).accounts.clone();
            let snackbar = snackbar.clone();
            let node_for_deploy = node_for_deploy.clone();
            spawn_local(async move {
                let applying_id = snackbar
                    .as_ref()
                    .map(|bus| bus.push("Deploying configuration..."));
                let api = ApiService::new(address.clone());
                let result = api
                    .update_config(build_full_config(&draft_value, &node_for_deploy, &accounts))
                    .await;
                if let Some(bus) = &snackbar {
                    if let Some(id) = applying_id {
                        bus.hide(id);
                    }
                }
                match result {
                    Ok(response) if response.success => {
                        if let Some(bus) = &snackbar {
                            bus.push("Deployed successfully");
                        }
                    }
                    Ok(response) => {
                        let msg = format!("Deploy failed: {}", response.error);
                        if let Some(bus) = &snackbar {
                            bus.push(msg);
                        }
                    }
                    Err(error) => {
                        let msg = format!("Deploy failed: {}", error);
                        if let Some(bus) = &snackbar {
                            bus.push(msg);
                        }
                    }
                }
            });
        })
    };
    let on_deploy_click = {
        let deploy_confirm_open = deploy_confirm_open.clone();
        Callback::from(move |_| deploy_confirm_open.set(true))
    };

    let on_back = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            if let Some(navigator) = navigator.clone() {
                navigator.push(&Route::Nodes);
            }
        })
    };

    let fetch_live_status = {
        let address = node.address.clone();
        let master_key = node.master_key.clone();
        let live_status = live_status.clone();
        let live_status_loading = live_status_loading.clone();
        let live_status_error = live_status_error.clone();
        Callback::from(move |_: ()| {
            let address = address.clone();
            let master_key = master_key.clone();
            let live_status = live_status.clone();
            let live_status_loading = live_status_loading.clone();
            let live_status_error = live_status_error.clone();
            live_status_loading.set(true);
            live_status_error.set(None);
            spawn_local(async move {
                let api = ApiService::new(address);
                match api.get_status(master_key).await {
                    Ok(status) => {
                        live_status.set(Some(status));
                    }
                    Err(error) => {
                        live_status_error.set(Some(error));
                    }
                }
                TimeoutFuture::new(500).await;
                live_status_loading.set(false);
            });
        })
    };

    let on_refresh_live_status = {
        let fetch_live_status = fetch_live_status.clone();
        Callback::from(move |_| fetch_live_status.emit(()))
    };

    {
        let active_tab = active_tab.clone();
        let fetch_live_status = fetch_live_status.clone();
        let status_auto_refresh = status_auto_refresh.clone();
        let status_refresh_interval_ms = status_refresh_interval_ms.clone();
        use_effect_with(
            (
                (*active_tab).clone(),
                *status_auto_refresh,
                *status_refresh_interval_ms,
            ),
            move |(tab, auto_refresh, refresh_ms)| {
                let interval = if *tab == ConfigTab::Status {
                    fetch_live_status.emit(());
                    if *auto_refresh {
                        Some(Interval::new(*refresh_ms, {
                            let fetch_live_status = fetch_live_status.clone();
                            move || fetch_live_status.emit(())
                        }))
                    } else {
                        None
                    }
                } else {
                    None
                };
                move || drop(interval)
            },
        );
    }

    let d = {
        let mut copy = (*draft).clone();
        sync_draft(&mut copy);
        copy
    };
    let inbounds = d.inbounds.clone();
    let routing_rules = d.routing_rules.clone();
    let wide_nav_items = nav_items();
    let wide_nav_active = nav_key(&*active_tab);
    let on_wide_nav_select = {
        let active_tab = active_tab.clone();
        Callback::from(move |value: AttrValue| {
            let tab = match value.as_str() {
                "outbounds" => ConfigTab::Outbounds,
                "routing" => ConfigTab::Routing,
                "settings" => ConfigTab::Settings,
                "status" => ConfigTab::Status,
                _ => ConfigTab::Inbounds,
            };
            active_tab.set(tab);
        })
    };

    html! {
        <div class="p-6 space-y-6" style="padding-bottom: 7.5rem;">
            <div class="flex justify-between" style="align-items: center;">
                <div class="flex items-center" style="gap: 1rem;">
                    <IconButton label="Back" button_type={ButtonType::Text} onclick={on_back}>
                        <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                            <path d="M14.71 6.71a1 1 0 0 1 0 1.41L10.83 12l3.88 3.88a1 1 0 1 1-1.41 1.41l-4.59-4.59a1 1 0 0 1 0-1.41l4.59-4.59a1 1 0 0 1 1.41 0Z" fill="currentColor" />
                        </svg>
                    </IconButton>
                    <h1 class="text-3xl font-bold">{ node.name.clone() }</h1>
                </div>
                <div class="flex items-center" style="gap: 0.75rem;">
                    <Dropdown
                        label=""
                        value={(*selected_revision_id).clone()}
                        options={revision_options.into_iter().map(|(value, label)| DropdownOption { value, label }).collect::<Vec<_>>()}
                        onchange={on_revision_change}
                        style={Some("min-width: 18rem;".to_string())}
                    />
                    <Button label="Create Revision" button_type={ButtonType::Outlined} onclick={save_revision} />
                    <Button label="Deploy" button_type={ButtonType::Filled} onclick={on_deploy_click} />
                </div>
            </div>

            {
                match &*active_tab {
                    ConfigTab::Inbounds => inbounds::render_inbounds_tab(&draft, &inbounds, &editing_inbound, &access_link_inbound_id, &action_inbound),
                    ConfigTab::Outbounds => outbounds::render_outbounds_tab(&draft, &d.outbounds, &editing_outbound, &warp_popup_open, &action_outbound),
                    ConfigTab::Routing => routing::render_routing_tab(
                        &draft,
                        &routing_rules,
                        &editing_routing_rule,
                        &pending_routing_delete,
                        &routing_move_anim,
                    ),
                    ConfigTab::Settings => settings::render_settings_tab(
                        &draft,
                        &d,
                        &editing_certificate,
                        &editing_dns_server,
                        &editing_dns_host,
                        &acme_confirm_open,
                        &pending_acme_certificate,
                    ),
                    ConfigTab::Status => status::render_status_tab(
                        &node,
                        &state.accounts,
                        &live_status,
                        &live_status_loading,
                        &live_status_error,
                        &status_auto_refresh,
                        &status_refresh_interval_ms,
                        &status_refresh_menu_open,
                        &on_refresh_live_status,
                    ),
                }
            }

            <div class="md3-config-nav">
                <WideNavigationBar
                    items={wide_nav_items}
                    active_value={wide_nav_active}
                    on_select={on_wide_nav_select}
                />
            </div>

            { render_popup_host(&PopupHostContext {
                state: &state,
                draft: &draft,
                node: &node,
                draft_value: &d,
                snackbar: &snackbar,
                editing_routing_rule: &editing_routing_rule,
                pending_routing_delete: &pending_routing_delete,
                pending_inbound_delete: &pending_inbound_delete,
                pending_duplicate_inbound: &pending_duplicate_inbound,
                action_inbound: &action_inbound,
                pending_outbound_delete: &pending_outbound_delete,
                pending_duplicate_outbound: &pending_duplicate_outbound,
                action_outbound: &action_outbound,
                warp_popup_open: &warp_popup_open,
                access_link_inbound_id: &access_link_inbound_id,
                deploy_confirm_open: &deploy_confirm_open,
                deploy_preview_json: &deploy_preview_json,
                deploy_revision: &deploy_revision,
                acme_confirm_open: &acme_confirm_open,
                pending_acme_certificate: &pending_acme_certificate,
                acme_logs: &acme_logs,
                acme_logs_open: &acme_logs_open,
                acme_loading: &acme_loading,
                editing_dns_server: &editing_dns_server,
                editing_dns_host: &editing_dns_host,
                editing_certificate: &editing_certificate,
                editing_inbound: &editing_inbound,
                editing_outbound: &editing_outbound,
            }) }
        </div>
    }
}
