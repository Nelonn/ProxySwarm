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
    Button, ButtonType, Chip, ChipMode, Dropdown, DropdownOption, IconButton, Popup, PopupSize,
    RichTable, SnackbarBus, SvgIcon, Switch, SwitchField, TextBox, WideNavigationBar,
    WideNavigationBarItem,
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

use access_links::*;
use config_build::*;
use dns_editors::*;
use helpers::*;
use popups::*;
use status_widgets::*;

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

#[derive(Properties, PartialEq)]
struct CertificateEditorPopupProps {
    certificate: CertificateDraft,
    is_new: bool,
    on_close: Callback<()>,
    on_save: Callback<CertificateDraft>,
}

#[function_component(CertificateEditorPopup)]
fn certificate_editor_popup(props: &CertificateEditorPopupProps) -> Html {
    let certificate = use_state(|| props.certificate.clone());

    let update_text = |mutator: fn(&mut CertificateDraft, String)| {
        let certificate = certificate.clone();
        Callback::from(move |value: String| {
            let mut next = (*certificate).clone();
            mutator(&mut next, value);
            certificate.set(next);
        })
    };

    let data = (*certificate).clone();
    let popup_title = if props.is_new {
        "Add Certificate"
    } else {
        "Edit Certificate"
    };

    let read_clipboard_into = |mutator: fn(&mut CertificateDraft, String)| {
        let certificate = certificate.clone();
        Callback::from(move |_| {
            let certificate = certificate.clone();
            spawn_local(async move {
                let Some(window) = window() else {
                    return;
                };
                let navigator = window.navigator();
                let Ok(clipboard) =
                    js_sys::Reflect::get(&navigator, &JsValue::from_str("clipboard"))
                else {
                    return;
                };
                let Ok(read_text) =
                    js_sys::Reflect::get(&clipboard, &JsValue::from_str("readText"))
                else {
                    return;
                };
                let Ok(function) = read_text.dyn_into::<js_sys::Function>() else {
                    return;
                };
                let Ok(promise_value) = function.call0(&clipboard) else {
                    return;
                };
                let promise = js_sys::Promise::from(promise_value);
                let Ok(value) = JsFuture::from(promise).await else {
                    return;
                };
                if let Some(text) = value.as_string() {
                    let mut next = (*certificate).clone();
                    mutator(&mut next, text);
                    certificate.set(next);
                }
            });
        })
    };

    let import_file_into = |mutator: fn(&mut CertificateDraft, String)| {
        let certificate = certificate.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            let Some(files) = input.files() else {
                return;
            };
            let Some(file) = files.get(0) else {
                return;
            };
            let certificate = certificate.clone();
            spawn_local(async move {
                let promise = file.text();
                let Ok(value) = JsFuture::from(promise).await else {
                    return;
                };
                if let Some(text) = value.as_string() {
                    let mut next = (*certificate).clone();
                    mutator(&mut next, text);
                    certificate.set(next);
                }
            });
        })
    };

    html! {
        <Popup title={popup_title} size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <TextBox label="Name" value={data.name.clone()} onchange={update_text(|certificate, value| certificate.name = value)} />
                <Dropdown
                    label="Type"
                    value={data.cert_type.clone()}
                    options={vec![
                        DropdownOption { value: "CUSTOM".to_string(), label: "Custom".to_string() },
                        DropdownOption { value: "ACME".to_string(), label: "ACME".to_string() },
                    ]}
                    onchange={update_text(|certificate, value| certificate.cert_type = value)}
                />
                {
                    if data.cert_type == "ACME" {
                        html! {
                            <>
                                <Dropdown
                                    label="ACME Type"
                                    value={data.acme_type.clone()}
                                    options={vec![
                                        DropdownOption { value: "HTTP".to_string(), label: "HTTP".to_string() },
                                        DropdownOption { value: "TLS".to_string(), label: "TLS".to_string() },
                                        DropdownOption { value: "DNS".to_string(), label: "DNS".to_string() },
                                    ]}
                                    onchange={update_text(|certificate, value| certificate.acme_type = value)}
                                />
                                <Dropdown
                                    label="CA"
                                    value={data.acme_ca.clone()}
                                    options={vec![
                                        DropdownOption { value: "letsencrypt".to_string(), label: "Let's Encrypt".to_string() },
                                        DropdownOption { value: "zerossl".to_string(), label: "ZeroSSL".to_string() },
                                        DropdownOption { value: "google".to_string(), label: "Google Trust Services".to_string() },
                                        DropdownOption { value: "buypass".to_string(), label: "Buypass Go SSL".to_string() },
                                        DropdownOption { value: "sslcom".to_string(), label: "SSL.com".to_string() },
                                    ]}
                                    onchange={update_text(|certificate, value| certificate.acme_ca = value)}
                                />
                                <TextBox label="Email" value={data.acme_email.clone()} onchange={update_text(|certificate, value| certificate.acme_email = value)} />
                                <TextBox label="Domain" value={data.acme_domain.clone()} onchange={update_text(|certificate, value| certificate.acme_domain = value)} />
                                {
                                    match data.acme_type.as_str() {
                                        "HTTP" => html! {
                                            <TextBox
                                                label="Port"
                                                value={data.acme_http_port.to_string()}
                                                onchange={update_text(|certificate, value| certificate.acme_http_port = value.parse().unwrap_or(0))}
                                                input_type="number"
                                            />
                                        },
                                        "TLS" => html! {
                                            <TextBox
                                                label="Port"
                                                value={data.acme_port.to_string()}
                                                onchange={update_text(|certificate, value| certificate.acme_port = value.parse().unwrap_or(0))}
                                                input_type="number"
                                            />
                                        },
                                        _ => html! {},
                                    }
                                }
                            </>
                        }
                    } else {
                        html! {
                            <>
                                <div class="space-y-2">
                                    <TextBox label="Certificate PEM" value={data.certificate_pem.clone()} onchange={update_text(|certificate, value| certificate.certificate_pem = value)} is_textarea={true} />
                                    <div class="flex" style="gap: 0.75rem;">
                                        <Button label="Paste Certificate" button_type={ButtonType::Outlined} onclick={read_clipboard_into(|certificate, value| certificate.certificate_pem = value)} />
                                        <label class="md3-btn md3-btn--outlined" style="cursor: pointer;">
                                            { "Import Certificate File" }
                                            <input type="file" accept=".pem,.crt,.cer,.txt" style="display: none;" onchange={import_file_into(|certificate, value| certificate.certificate_pem = value)} />
                                        </label>
                                    </div>
                                </div>
                                <div class="space-y-2">
                                    <TextBox label="Key PEM" value={data.key_pem.clone()} onchange={update_text(|certificate, value| certificate.key_pem = value)} is_textarea={true} />
                                    <div class="flex" style="gap: 0.75rem;">
                                        <Button label="Paste Key" button_type={ButtonType::Outlined} onclick={read_clipboard_into(|certificate, value| certificate.key_pem = value)} />
                                        <label class="md3-btn md3-btn--outlined" style="cursor: pointer;">
                                            { "Import Key File" }
                                            <input type="file" accept=".pem,.key,.txt" style="display: none;" onchange={import_file_into(|certificate, value| certificate.key_pem = value)} />
                                        </label>
                                    </div>
                                </div>
                            </>
                        }
                    }
                }
            </div>
            <div class="md3-popup-actions" style="justify-content: flex-end;">
                <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                    let on_close = props.on_close.clone();
                    move |_| on_close.emit(())
                })} />
                <Button label={if props.is_new { "Create Certificate" } else { "Apply Changes" }} button_type={ButtonType::Filled} onclick={Callback::from({
                    let on_save = props.on_save.clone();
                    let certificate = certificate.clone();
                    move |_| on_save.emit((*certificate).clone())
                })} />
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct InboundEditorPopupProps {
    inbound: InboundEntryDraft,
    certificates: Vec<CertificateDraft>,
    is_new: bool,
    on_close: Callback<()>,
    on_save: Callback<InboundEntryDraft>,
}

fn inbound_creation_steps(inbound: &InboundEntryDraft) -> usize {
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

fn outbound_creation_steps(outbound: &OutboundEntryDraft) -> usize {
    match outbound.outbound_type.as_str() {
        "VLESS" => 3,
        "WIREGUARD" => 3,
        "SOCKS5" => 3,
        _ => 3,
    }
}

#[function_component(InboundEditorPopup)]
fn inbound_editor_popup(props: &InboundEditorPopupProps) -> Html {
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

#[derive(Properties, PartialEq)]
struct OutboundEditorPopupProps {
    outbound: OutboundEntryDraft,
    is_new: bool,
    node_address: String,
    master_key: String,
    on_close: Callback<()>,
    on_save: Callback<OutboundEntryDraft>,
}

#[derive(Properties, PartialEq)]
struct WarpCreatePopupProps {
    node_address: String,
    master_key: String,
    initial_registration: Option<crate::services::warp::WarpRegistration>,
    on_registration_change: Callback<Option<crate::services::warp::WarpRegistration>>,
    on_close: Callback<()>,
    on_create: Callback<OutboundEntryDraft>,
}

#[function_component(WarpCreatePopup)]
fn warp_create_popup(props: &WarpCreatePopupProps) -> Html {
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
fn outbound_editor_popup(props: &OutboundEditorPopupProps) -> Html {
    let outbound = use_state(|| props.outbound.clone());
    let step = use_state(|| 0usize);

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
                                                <div class="grid grid-cols-1 md-grid-cols-2 gap-6">
                                                    <TextBox label="Tag" value={data.vless.tag.clone()} onchange={update_text(|outbound, value| outbound.vless.tag = value)} />
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
                                </>
                            },
                            _ => html! {
                                <>
                                    {
                                        match data.outbound_type.trim().to_uppercase().as_str() {
                                            "VLESS" => html! {
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
                                                        label="Transmission"
                                                        value={data.vless.transmission.clone()}
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
                                            },
                                            _ => html! {
                                                <ConfigSection title="Review">
                                                    <div class="space-y-2 text-sm">
                                                        <div>{ format!("Name: {}", data.name) }</div>
                                                        <div>{ format!("Type: {}", data.outbound_type) }</div>
                                                    </div>
                                                </ConfigSection>
                                            }
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
                                <TextBox label="Tag" value={data.vless.tag.clone()} onchange={update_text(|outbound, value| outbound.vless.tag = value)} />
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

#[derive(Properties, PartialEq)]
struct AccessLinkPopupProps {
    node: ProxyNode,
    inbound: InboundEntryDraft,
    accounts: Vec<AccountInfo>,
    on_close: Callback<()>,
}

#[function_component(AccessLinkPopup)]
fn access_link_popup(props: &AccessLinkPopupProps) -> Html {
    let snackbar = use_context::<SnackbarBus>();
    let initial_account = props
        .accounts
        .first()
        .map(|account| account.id.clone())
        .unwrap_or_default();
    let selected_account_id = use_state(|| initial_account);
    let copy_status = use_state(|| Option::<String>::None);
    let generated_link = use_state(|| Option::<String>::None);

    let selected_account = props
        .accounts
        .iter()
        .find(|account| account.id == *selected_account_id)
        .cloned();
    let qr = generated_link.as_ref().and_then(|value| qr_svg(value));

    html! {
        <Popup title="Generate Access Link" size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                {
                    if let Some(link) = (*generated_link).clone() {
                        html! {
                            <div class="space-y-4">
                                <div class="md3-qr-card">
                                    {
                                        if let Some(qr) = qr {
                                            Html::from_html_unchecked(AttrValue::from(qr))
                                        } else {
                                            html! { <div>{ "QR unavailable" }</div> }
                                        }
                                    }
                                </div>
                                <div>
                                    <label class="block text-sm font-medium mb-1 text-on-surface">{ "Access Link" }</label>
                                    <div class="md3-access-link">{ link.clone() }</div>
                                    <div style="margin-top: 0.5rem; display: flex; justify-content: flex-start;">
                                        <Button label="Copy" button_type={ButtonType::Tonal} onclick={Callback::from({
                                            let link = link.clone();
                                            let copy_status = copy_status.clone();
                                            let snackbar = snackbar.clone();
                                            move |_| {
                                                let copy_status = copy_status.clone();
                                                let link = link.clone();
                                                let snackbar = snackbar.clone();
                                                spawn_local(async move {
                                                    match copy_to_clipboard(link).await {
                                                        Ok(_) => {
                                                            copy_status.set(None);
                                                            if let Some(bus) = snackbar {
                                                                bus.push("Copied access link");
                                                            }
                                                        }
                                                        Err(error) => {
                                                            copy_status.set(Some(error.clone()));
                                                            if let Some(bus) = snackbar {
                                                                bus.push(error);
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                        })} />
                                    </div>
                                </div>
                            </div>
                        }
                    } else {
                        html! {
                            <>
                                <Dropdown
                                    label="User"
                                    value={(*selected_account_id).clone()}
                                    options={props.accounts.iter().map(|account| DropdownOption {
                                        value: account.id.clone(),
                                        label: account.name.clone(),
                                    }).collect::<Vec<_>>()}
                                    onchange={Callback::from({
                                        let selected_account_id = selected_account_id.clone();
                                        let generated_link = generated_link.clone();
                                        let copy_status = copy_status.clone();
                                        move |value: String| {
                                            selected_account_id.set(value);
                                            generated_link.set(None);
                                            copy_status.set(None);
                                        }
                                    })}
                                />
                                <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">{ "Select user, then click Generate. Access links are available for VLESS and TrustTunnel inbounds with user credentials and node address." }</div>
                            </>
                        }
                    }
                }
                {
                    if let Some(status) = &*copy_status {
                        html! { <div class="text-sm opacity-70">{ status }</div> }
                    } else {
                        html! {}
                    }
                }
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    {
                        if generated_link.is_some() {
                            html! {
                                <Button label="Back" button_type={ButtonType::Text} onclick={Callback::from({
                                    let generated_link = generated_link.clone();
                                    let copy_status = copy_status.clone();
                                    move |_| {
                                        generated_link.set(None);
                                        copy_status.set(None);
                                    }
                                })} />
                            }
                        } else {
                            html! {}
                        }
                    }
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    {
                        if generated_link.is_none() {
                            html! {
                                <Button label="Generate" button_type={ButtonType::Filled} onclick={Callback::from({
                                    let selected_account = selected_account.clone();
                                    let generated_link = generated_link.clone();
                                    let copy_status = copy_status.clone();
                                    let node = props.node.clone();
                                    let inbound = props.inbound.clone();
                                    move |_| {
                                        copy_status.set(None);
                                        match selected_account
                                            .as_ref()
                                            .ok_or_else(|| "Select user first".to_string())
                                            .and_then(|account| build_access_link(&node.config, &node, &inbound, account))
                                        {
                                            Ok(link) => generated_link.set(Some(link)),
                                            Err(error) => {
                                                generated_link.set(None);
                                                copy_status.set(Some(error));
                                            }
                                        }
                                    }
                                })} />
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct RoutingRuleEditorPopupProps {
    rule: RoutingRuleDraft,
    is_new: bool,
    inbound_options: Vec<String>,
    user_options: Vec<String>,
    outbound_options: Vec<DropdownOption>,
    on_close: Callback<()>,
    on_save: Callback<RoutingRuleDraft>,
}

#[function_component(RoutingRuleEditorPopup)]
fn routing_rule_editor_popup(props: &RoutingRuleEditorPopupProps) -> Html {
    let rule = use_state(|| props.rule.clone());
    let inbound_tag_query = use_state(String::new);
    let protocol_query = use_state(String::new);
    let protocol_open = use_state(|| false);
    let inbound_open = use_state(|| false);
    let protocol_input_ref = use_node_ref();
    let inbound_input_ref = use_node_ref();
    let user_query = use_state(String::new);
    let user_open = use_state(|| false);
    let user_input_ref = use_node_ref();
    let transport_value = use_state(|| {
        let mut has_tcp = false;
        let mut has_udp = false;
        for value in split_lines_csv(&props.rule.transport)
            .into_iter()
            .map(|value| value.trim().to_lowercase())
        {
            match value.as_str() {
                "tcp" => has_tcp = true,
                "udp" => has_udp = true,
                _ => {}
            }
        }
        match (has_tcp, has_udp) {
            (true, true) => "tcp,udp".to_string(),
            (true, false) => "tcp".to_string(),
            (false, true) => "udp".to_string(),
            (false, false) => "tcp,udp".to_string(),
        }
    });
    {
        let rule = rule.clone();
        let incoming = props.rule.clone();
        use_effect_with(incoming, move |next_rule| {
            rule.set(next_rule.clone());
            || ()
        });
    }

    let on_text_change = |mutator: fn(&mut RoutingRuleDraft, String)| {
        let rule = rule.clone();
        Callback::from(move |value: String| {
            let mut next = (*rule).clone();
            mutator(&mut next, value);
            rule.set(next);
        })
    };

    let on_save = {
        let on_save = props.on_save.clone();
        let rule = rule.clone();
        let transport_value = transport_value.clone();
        let allowed_user_options = props.user_options.clone();
        Callback::from(move |_| {
            let mut next = (*rule).clone();

            let mut transports: Vec<String> = match transport_value.as_str() {
                "tcp" => vec!["tcp".to_string()],
                "udp" => vec!["udp".to_string()],
                _ => vec!["tcp".to_string(), "udp".to_string()],
            };
            next.transport = transports.join(",");

            let allowed_app = ["http", "tls", "bittorrent"];
            let mut app_protocols = split_lines_csv(&next.protocol)
                .into_iter()
                .map(|value| value.trim().to_lowercase())
                .filter(|value| {
                    if value.is_empty() {
                        return false;
                    }
                    if value == "tcp" || value == "udp" {
                        return false;
                    }
                    allowed_app.iter().any(|p| p == value)
                })
                .collect::<Vec<_>>();
            app_protocols.sort();
            app_protocols.dedup();
            next.protocol = app_protocols.join(",");

            let allowed_users = split_lines_csv(&next.user)
                .into_iter()
                .map(|value| value.trim().to_string())
                .filter(|value| {
                    !value.is_empty() && allowed_user_options.iter().any(|opt| opt == value)
                })
                .collect::<Vec<_>>();
            let mut users = allowed_users;
            users.sort();
            users.dedup();
            next.user = users.join(", ");

            on_save.emit(next)
        })
    };
    let on_close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    let selected_inbound_tags = split_lines_csv(&rule.inbound_tag)
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let selected_users = split_lines_csv(&rule.user)
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let selected_protocols = split_lines_csv(&rule.protocol)
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let inbound_suggestions = {
        let needle = inbound_tag_query.trim().to_lowercase();
        let mut options = props
            .inbound_options
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .filter(|value| {
                !selected_inbound_tags
                    .iter()
                    .any(|existing| existing == value)
            })
            .collect::<Vec<_>>();

        if !needle.is_empty() {
            options.retain(|value| value.to_lowercase().contains(&needle));
        }
        options.sort();
        options.dedup();
        options
    };

    let user_suggestions = {
        let needle = user_query.trim().to_lowercase();
        let mut options = props
            .user_options
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .filter(|value| !selected_users.iter().any(|existing| existing == value))
            .collect::<Vec<_>>();

        if !needle.is_empty() {
            options.retain(|value| value.to_lowercase().contains(&needle));
        }
        options.sort();
        options.dedup();
        options
    };

    let protocol_suggestions = {
        let allowed = ["http", "tls", "bittorrent"];
        let needle = protocol_query.trim().to_lowercase();
        let mut options = allowed
            .iter()
            .map(|value| value.to_string())
            .filter(|value| !selected_protocols.iter().any(|existing| existing == value))
            .collect::<Vec<_>>();
        if !needle.is_empty() {
            options.retain(|value| value.to_lowercase().contains(&needle));
        }
        options.sort();
        options.dedup();
        options
    };

    html! {
        <Popup
            title={if props.is_new { "Add Routing Rule" } else { "Edit Routing Rule" }}
            size={PopupSize::Lg}
            on_close={props.on_close.clone()}
        >
            <div class="space-y-4">
                <TextBox
                    label="Remark"
                    value={rule.remark.clone()}
                    onchange={on_text_change(|draft, value| draft.remark = value)}
                    placeholder="Human-readable note for this rule"
                />
                <TextBox
                    label="Domains"
                    value={rule.domain.clone()}
                    onchange={on_text_change(|draft, value| draft.domain = value)}
                    is_textarea={true}
                    placeholder="example.com, api.example.com"
                />
                <TextBox
                    label="IPs"
                    value={rule.ip.clone()}
                    onchange={on_text_change(|draft, value| draft.ip = value)}
                    is_textarea={true}
                    placeholder="1.1.1.1, 10.0.0.0/24"
                />
                <TextBox
                    label="Ports"
                    value={rule.port.clone()}
                    onchange={on_text_change(|draft, value| draft.port = value)}
                    is_textarea={true}
                    placeholder="80,443"
                />
                <Dropdown
                    label="Transport"
                    value={(*transport_value).clone()}
                    options={vec![
                        DropdownOption { label: "tcp".to_string(), value: "tcp".to_string() },
                        DropdownOption { label: "udp".to_string(), value: "udp".to_string() },
                        DropdownOption { label: "tcp,udp".to_string(), value: "tcp,udp".to_string() },
                    ]}
                    onchange={Callback::from({
                        let transport_value = transport_value.clone();
                        move |value: String| transport_value.set(value)
                    })}
                />
                <div class="w-full">
                    <label class="block text-sm font-medium mb-1 text-on-surface">{ "Protocols" }</label>
                    <div style="position: relative;">
                        <div
                            class="md3-input"
                            tabindex={0}
                            style={format!(
                                "display:flex; flex-wrap:nowrap; gap:8px; align-items:center; padding: 8px 12px; cursor:text; border-color: {}; height: 48px; overflow-x:auto; overflow-y:hidden; white-space:nowrap;",
                                if *protocol_open { "var(--md-sys-color-primary)" } else { "var(--md-sys-color-primary-outline)" }
                            )}
                            onclick={Callback::from({
                                let protocol_input_ref = protocol_input_ref.clone();
                                let protocol_open = protocol_open.clone();
                                move |_| {
                                    protocol_open.set(true);
                                    if let Some(input) = protocol_input_ref.cast::<web_sys::HtmlInputElement>() {
                                        let _ = input.focus();
                                    }
                                }
                            })}
                            onfocus={Callback::from({
                                let protocol_open = protocol_open.clone();
                                move |_| protocol_open.set(true)
                            })}
                        >
                            {
                                for selected_protocols.iter().cloned().map(|proto| {
                                    let rule = rule.clone();
                                    let proto_remove = proto.clone();
                                    html! {
                                        <Chip
                                            label={AttrValue::from(proto)}
                                            mode={ChipMode::Outlined}
                                            trailing_icon={Some("close_24dp".to_string())}
                                            on_trailing_click={Some(Callback::from(move |_| {
                                                let mut next = (*rule).clone();
                                                let mut remaining = split_lines_csv(&next.protocol)
                                                    .into_iter()
                                                    .map(|value| value.trim().to_lowercase())
                                                    .filter(|value| {
                                                        if value.is_empty() {
                                                            return false;
                                                        }
                                                        if value == "tcp" || value == "udp" {
                                                            return true;
                                                        }
                                                        value != &proto_remove
                                                    })
                                                    .collect::<Vec<_>>();
                                                remaining.sort();
                                                remaining.dedup();
                                                next.protocol = remaining.join(", ");
                                                rule.set(next);
                                            }))}
                                        />
                                    }
                                })
                            }
                            <input
                                ref={protocol_input_ref.clone()}
                                type="text"
                                class=""
                                placeholder="Type to search, then pick from suggestions"
                                value={(*protocol_query).clone()}
                                onclick={Callback::from({
                                    let protocol_open = protocol_open.clone();
                                    move |_| protocol_open.set(true)
                                })}
                                onfocus={Callback::from({
                                    let protocol_open = protocol_open.clone();
                                    move |_| protocol_open.set(true)
                                })}
                                oninput={Callback::from({
                                    let protocol_query = protocol_query.clone();
                                    let protocol_open = protocol_open.clone();
                                    move |e: InputEvent| {
                                        let target = e.target().unwrap();
                                        let value = js_sys::Reflect::get(&target, &"value".into())
                                            .unwrap()
                                            .as_string()
                                            .unwrap();
                                        protocol_query.set(value);
                                        protocol_open.set(true);
                                    }
                                })}
                                onblur={Callback::from({
                                    let protocol_open = protocol_open.clone();
                                    move |_| {
                                        let protocol_open = protocol_open.clone();
                                        Timeout::new(120, move || protocol_open.set(false)).forget();
                                    }
                                })}
                                onkeydown={Callback::from({
                                    let rule = rule.clone();
                                    let protocol_query = protocol_query.clone();
                                    let protocol_suggestions = protocol_suggestions.clone();
                                    let selected_protocols = selected_protocols.clone();
                                    let protocol_open = protocol_open.clone();
                                    move |e: KeyboardEvent| {
                                        if e.key() != "Enter" {
                                            return;
                                        }
                                        e.prevent_default();
                                        let Some(first) = protocol_suggestions.first().cloned() else {
                                            return;
                                        };
                                        let mut next = (*rule).clone();
                                        let mut combined = selected_protocols.clone();
                                        if !combined.iter().any(|value| value == &first) {
                                            combined.push(first);
                                        }
                                        combined.sort();
                                        combined.dedup();
                                        let mut keep_transport = split_lines_csv(&next.protocol)
                                            .into_iter()
                                            .map(|value| value.trim().to_lowercase())
                                            .filter(|value| value == "tcp" || value == "udp")
                                            .collect::<Vec<_>>();
                                        keep_transport.extend(combined);
                                        keep_transport.sort();
                                        keep_transport.dedup();
                                        next.protocol = keep_transport.join(", ");
                                        rule.set(next);
                                        protocol_query.set(String::new());
                                        protocol_open.set(false);
                                    }
                                })}
                                style="border:0; outline:none; background:transparent; padding:0; margin:0; min-width: 120px; flex: 1 0 120px; color: inherit; font-size: 1rem;"
                            />
                        </div>
                        {
                            if !*protocol_open || protocol_suggestions.is_empty() {
                                html! {}
                            } else {
                                html! {
                                    <div
                                        class="md3-card bg-surface-container"
                                        style="position:absolute; z-index: 50; left: 0; right: 0; top: calc(100% + 6px); padding: 8px; max-height: 180px; overflow:auto;"
                                    >
                                        <div class="flex flex-wrap" style="gap: 8px; align-items: center;">
                                            {
                                                for protocol_suggestions.iter().cloned().map(|proto| {
                                                    let chip_label = proto.clone();
                                                    let rule = rule.clone();
                                                    let protocol_query = protocol_query.clone();
                                                    let protocol_open = protocol_open.clone();
                                                    let selected_protocols = selected_protocols.clone();
                                                    html! {
                                                        <span onmousedown={Callback::from(move |e: MouseEvent| {
                                                            e.prevent_default();
                                                            let mut next = (*rule).clone();
                                                            let mut combined = selected_protocols.clone();
                                                            if !combined.iter().any(|value| value == &proto) {
                                                                combined.push(proto.clone());
                                                            }
                                                            combined.sort();
                                                            combined.dedup();
                                                            let mut keep_transport = split_lines_csv(&next.protocol)
                                                                .into_iter()
                                                                .map(|value| value.trim().to_lowercase())
                                                                .filter(|value| value == "tcp" || value == "udp")
                                                                .collect::<Vec<_>>();
                                                            keep_transport.extend(combined);
                                                            keep_transport.sort();
                                                            keep_transport.dedup();
                                                            next.protocol = keep_transport.join(", ");
                                                            rule.set(next);
                                                            protocol_query.set(String::new());
                                                            protocol_open.set(false);
                                                        })}>
                                                            <Chip label={AttrValue::from(chip_label)} mode={ChipMode::Outlined} />
                                                        </span>
                                                    }
                                                })
                                            }
                                        </div>
                                    </div>
                                }
                            }
                        }
                    </div>
                </div>
                <div class="w-full">
                    <label class="block text-sm font-medium mb-1 text-on-surface">{ "Inbound Tags" }</label>
                    <div style="position: relative;">
                        <div
                            class="md3-input"
                            tabindex={0}
                            style={format!(
                                "display:flex; flex-wrap:nowrap; gap:8px; align-items:center; padding: 8px 12px; cursor:text; border-color: {}; height: 48px; overflow-x:auto; overflow-y:hidden; white-space:nowrap;",
                                if *inbound_open { "var(--md-sys-color-primary)" } else { "var(--md-sys-color-primary-outline)" }
                            )}
                            onclick={Callback::from({
                                let inbound_input_ref = inbound_input_ref.clone();
                                let inbound_open = inbound_open.clone();
                                move |_| {
                                    inbound_open.set(true);
                                    if let Some(input) = inbound_input_ref.cast::<web_sys::HtmlInputElement>() {
                                        let _ = input.focus();
                                    }
                                }
                            })}
                            onfocus={Callback::from({
                                let inbound_open = inbound_open.clone();
                                move |_| inbound_open.set(true)
                            })}
                        >
                            {
                                for selected_inbound_tags.iter().cloned().map(|tag| {
                                    let rule = rule.clone();
                                    let tag_remove = tag.clone();
                                    html! {
                                        <Chip
                                            label={AttrValue::from(tag)}
                                            mode={ChipMode::Outlined}
                                            trailing_icon={Some("close_24dp".to_string())}
                                            on_trailing_click={Some(Callback::from(move |_| {
                                                let mut next = (*rule).clone();
                                                let remaining = split_lines_csv(&next.inbound_tag)
                                                    .into_iter()
                                                    .map(|value| value.trim().to_string())
                                                    .filter(|value| !value.is_empty() && value != &tag_remove)
                                                    .collect::<Vec<_>>();
                                                next.inbound_tag = remaining.join(", ");
                                                rule.set(next);
                                            }))}
                                        />
                                    }
                                })
                            }
                            <input
                                ref={inbound_input_ref.clone()}
                                type="text"
                                class=""
                                placeholder="Type to search, then pick from suggestions"
                                value={(*inbound_tag_query).clone()}
                                onclick={Callback::from({
                                    let inbound_open = inbound_open.clone();
                                    move |_| inbound_open.set(true)
                                })}
                                onfocus={Callback::from({
                                    let inbound_open = inbound_open.clone();
                                    move |_| inbound_open.set(true)
                                })}
                                oninput={Callback::from({
                                    let inbound_tag_query = inbound_tag_query.clone();
                                    let inbound_open = inbound_open.clone();
                                    move |e: InputEvent| {
                                        let target = e.target().unwrap();
                                        let value = js_sys::Reflect::get(&target, &"value".into())
                                            .unwrap()
                                            .as_string()
                                            .unwrap();
                                        inbound_tag_query.set(value);
                                        inbound_open.set(true);
                                    }
                                })}
                                onblur={Callback::from({
                                    let inbound_open = inbound_open.clone();
                                    move |_| {
                                        let inbound_open = inbound_open.clone();
                                        Timeout::new(120, move || inbound_open.set(false)).forget();
                                    }
                                })}
                                onkeydown={Callback::from({
                                    let rule = rule.clone();
                                    let inbound_tag_query = inbound_tag_query.clone();
                                    let inbound_suggestions = inbound_suggestions.clone();
                                    let selected_inbound_tags = selected_inbound_tags.clone();
                                    let inbound_open = inbound_open.clone();
                                    move |e: KeyboardEvent| {
                                        if e.key() != "Enter" {
                                            return;
                                        }
                                        e.prevent_default();
                                        let Some(first) = inbound_suggestions.first().cloned() else {
                                            return;
                                        };
                                        let mut next = (*rule).clone();
                                        let mut combined = selected_inbound_tags.clone();
                                        if !combined.iter().any(|value| value == &first) {
                                            combined.push(first);
                                        }
                                        combined.sort();
                                        combined.dedup();
                                        next.inbound_tag = combined.join(", ");
                                        rule.set(next);
                                        inbound_tag_query.set(String::new());
                                        inbound_open.set(false);
                                    }
                                })}
                                style="border:0; outline:none; background:transparent; padding:0; margin:0; min-width: 140px; flex: 1 0 140px; color: inherit; font-size: 1rem;"
                            />
                        </div>
                        {
                            if !*inbound_open || inbound_suggestions.is_empty() {
                                html! {}
                            } else {
                                html! {
                                    <div
                                        class="md3-card bg-surface-container"
                                        style="position:absolute; z-index: 50; left: 0; right: 0; top: calc(100% + 6px); padding: 8px; max-height: 180px; overflow:auto;"
                                    >
                                        <div class="flex flex-wrap" style="gap: 8px; align-items: center;">
                                            {
                                                for inbound_suggestions.iter().cloned().map(|tag| {
                                                    let chip_label = tag.clone();
                                                    let rule = rule.clone();
                                                    let inbound_tag_query = inbound_tag_query.clone();
                                                    let inbound_open = inbound_open.clone();
                                                    let selected_inbound_tags = selected_inbound_tags.clone();
                                                    html! {
                                                        <span onmousedown={Callback::from(move |e: MouseEvent| {
                                                            e.prevent_default();
                                                            let mut next = (*rule).clone();
                                                            let mut combined = selected_inbound_tags.clone();
                                                            if !combined.iter().any(|value| value == &tag) {
                                                                combined.push(tag.clone());
                                                            }
                                                            combined.sort();
                                                            combined.dedup();
                                                            next.inbound_tag = combined.join(", ");
                                                            rule.set(next);
                                                            inbound_tag_query.set(String::new());
                                                            inbound_open.set(false);
                                                        })}>
                                                            <Chip label={AttrValue::from(chip_label)} mode={ChipMode::Outlined} />
                                                        </span>
                                                    }
                                                })
                                            }
                                        </div>
                                    </div>
                                }
                            }
                        }
                    </div>
                </div>
                <div class="w-full">
                    <label class="block text-sm font-medium mb-1 text-on-surface">{ "Users" }</label>
                    <div style="position: relative;">
                        <div
                            class="md3-input"
                            tabindex={0}
                            style={format!(
                                "display:flex; flex-wrap:nowrap; gap:8px; align-items:center; padding: 8px 12px; cursor:text; border-color: {}; height: 48px; overflow-x:auto; overflow-y:hidden; white-space:nowrap;",
                                if *user_open { "var(--md-sys-color-primary)" } else { "var(--md-sys-color-primary-outline)" }
                            )}
                            onclick={Callback::from({
                                let user_input_ref = user_input_ref.clone();
                                let user_open = user_open.clone();
                                move |_| {
                                    user_open.set(true);
                                    if let Some(input) = user_input_ref.cast::<web_sys::HtmlInputElement>() {
                                        let _ = input.focus();
                                    }
                                }
                            })}
                            onfocus={Callback::from({
                                let user_open = user_open.clone();
                                move |_| user_open.set(true)
                            })}
                        >
                            {
                                for selected_users.iter().cloned().map(|name| {
                                    let rule = rule.clone();
                                    let remove_name = name.clone();
                                    html! {
                                        <Chip
                                            label={AttrValue::from(name)}
                                            mode={ChipMode::Outlined}
                                            trailing_icon={Some("close_24dp".to_string())}
                                            on_trailing_click={Some(Callback::from(move |_| {
                                                let mut next = (*rule).clone();
                                                let remaining = split_lines_csv(&next.user)
                                                    .into_iter()
                                                    .map(|value| value.trim().to_string())
                                                    .filter(|value| !value.is_empty() && value != &remove_name)
                                                    .collect::<Vec<_>>();
                                                next.user = remaining.join(", ");
                                                rule.set(next);
                                            }))}
                                        />
                                    }
                                })
                            }
                            <input
                                ref={user_input_ref.clone()}
                                type="text"
                                class=""
                                placeholder="Type to search, then pick from suggestions"
                                value={(*user_query).clone()}
                                onclick={Callback::from({
                                    let user_open = user_open.clone();
                                    move |_| user_open.set(true)
                                })}
                                onfocus={Callback::from({
                                    let user_open = user_open.clone();
                                    move |_| user_open.set(true)
                                })}
                                oninput={Callback::from({
                                    let user_query = user_query.clone();
                                    let user_open = user_open.clone();
                                    move |e: InputEvent| {
                                        let target = e.target().unwrap();
                                        let value = js_sys::Reflect::get(&target, &"value".into())
                                            .unwrap()
                                            .as_string()
                                            .unwrap();
                                        user_query.set(value);
                                        user_open.set(true);
                                    }
                                })}
                                onblur={Callback::from({
                                    let user_open = user_open.clone();
                                    move |_| {
                                        let user_open = user_open.clone();
                                        Timeout::new(120, move || user_open.set(false)).forget();
                                    }
                                })}
                                onkeydown={Callback::from({
                                    let rule = rule.clone();
                                    let user_query = user_query.clone();
                                    let user_suggestions = user_suggestions.clone();
                                    let selected_users = selected_users.clone();
                                    let user_open = user_open.clone();
                                    move |e: KeyboardEvent| {
                                        if e.key() != "Enter" {
                                            return;
                                        }
                                        e.prevent_default();
                                        let Some(first) = user_suggestions.first().cloned() else {
                                            return;
                                        };
                                        let mut next = (*rule).clone();
                                        let mut combined = selected_users.clone();
                                        if !combined.iter().any(|value| value == &first) {
                                            combined.push(first);
                                        }
                                        combined.sort();
                                        combined.dedup();
                                        next.user = combined.join(", ");
                                        rule.set(next);
                                        user_query.set(String::new());
                                        user_open.set(false);
                                    }
                                })}
                                style="border:0; outline:none; background:transparent; padding:0; margin:0; min-width: 140px; flex: 1 0 140px; color: inherit; font-size: 1rem;"
                            />
                        </div>
                        {
                            if !*user_open || user_suggestions.is_empty() {
                                html! {}
                            } else {
                                html! {
                                    <div
                                        class="md3-card bg-surface-container"
                                        style="position:absolute; z-index: 50; left: 0; right: 0; top: calc(100% + 6px); padding: 8px; max-height: 180px; overflow:auto;"
                                    >
                                        <div class="flex flex-wrap" style="gap: 8px; align-items: center;">
                                            {
                                                for user_suggestions.iter().cloned().map(|name| {
                                                    let chip_label = name.clone();
                                                    let rule = rule.clone();
                                                    let user_query = user_query.clone();
                                                    let user_open = user_open.clone();
                                                    let selected_users = selected_users.clone();
                                                    html! {
                                                        <span onmousedown={Callback::from(move |e: MouseEvent| {
                                                            e.prevent_default();
                                                            let mut next = (*rule).clone();
                                                            let mut combined = selected_users.clone();
                                                            if !combined.iter().any(|value| value == &name) {
                                                                combined.push(name.clone());
                                                            }
                                                            combined.sort();
                                                            combined.dedup();
                                                            next.user = combined.join(", ");
                                                            rule.set(next);
                                                            user_query.set(String::new());
                                                            user_open.set(false);
                                                        })}>
                                                            <Chip label={AttrValue::from(chip_label)} mode={ChipMode::Outlined} />
                                                        </span>
                                                    }
                                                })
                                            }
                                        </div>
                                    </div>
                                }
                            }
                        }
                    </div>
                </div>
                <Dropdown
                    label="Outbound"
                    value={rule.outbound_tag.clone()}
                    options={props.outbound_options.clone()}
                    onchange={on_text_change(|draft, value| draft.outbound_tag = value)}
                />
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={on_close} />
                    <Button label={if props.is_new { "Add Rule" } else { "Apply Changes" }} button_type={ButtonType::Filled} onclick={on_save} />
                </div>
            </div>
        </Popup>
    }
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
    let pending_inbound_delete = use_state(|| Option::<(String, String)>::None);
    let pending_outbound_delete = use_state(|| Option::<(String, String)>::None);
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
    let on_deploy_preview_click = {
        let deploy_preview_json = deploy_preview_json.clone();
        let draft = draft.clone();
        let state = state.clone();
        let node_id = node.id.clone();
        let node_for_deploy = node.clone();
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
    let mut routing_outbound_options: Vec<DropdownOption> = Vec::new();
    for outbound in &d.outbounds {
        let tag = outbound_tag_for_routing(outbound);
        if tag.trim().is_empty()
            || routing_outbound_options
                .iter()
                .any(|option| option.value == tag)
        {
            continue;
        }
        routing_outbound_options.push(DropdownOption {
            value: tag,
            label: outbound_label_for_routing(outbound),
        });
    }
    let routing_inbound_options = {
        let mut options = d
            .inbounds
            .iter()
            .map(|inbound| inbound.name.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        options.sort();
        options.dedup();
        options
    };
    let routing_user_options = {
        let mut options = state
            .accounts
            .iter()
            .map(|account| account.name.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        options.sort();
        options.dedup();
        options
    };
    let selected_access_inbound = (*access_link_inbound_id)
        .clone()
        .and_then(|id| inbounds.iter().find(|inbound| inbound.id == id).cloned());
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
                    ConfigTab::Inbounds => inbounds::render_inbounds_tab(&draft, &inbounds, &editing_inbound, &access_link_inbound_id, &pending_inbound_delete),
                    ConfigTab::Outbounds => outbounds::render_outbounds_tab(&draft, &d.outbounds, &editing_outbound, &warp_popup_open, &pending_outbound_delete),
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

            {
                if let Some((rule_index, rule, is_new)) = &*editing_routing_rule {
                    html! {
                        <RoutingRuleEditorPopup
                            rule={rule.clone()}
                            is_new={*is_new}
                            inbound_options={routing_inbound_options.clone()}
                            user_options={routing_user_options.clone()}
                            outbound_options={routing_outbound_options.clone()}
                            on_close={Callback::from({
                                let editing_routing_rule = editing_routing_rule.clone();
                                move |_| editing_routing_rule.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_routing_rule = editing_routing_rule.clone();
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

            {
                if *deploy_confirm_open {
                    html! {
                        <ConfirmPopup
                            title="Deploy Revision"
                            body="Deploy current draft to this node now? This overwrites active runtime configuration."
                            confirm_label="Deploy"
                            extra_label={Some(AttrValue::from("Preview"))}
                            align_actions_end={true}
                            on_close={Callback::from({
                                let deploy_confirm_open = deploy_confirm_open.clone();
                                move |_| deploy_confirm_open.set(false)
                            })}
                            on_extra={Some(Callback::from({
                                let on_deploy_preview_click = on_deploy_preview_click.clone();
                                move |_| on_deploy_preview_click.emit(())
                            }))}
                            on_confirm={Callback::from({
                                let deploy_confirm_open = deploy_confirm_open.clone();
                                let deploy_revision = deploy_revision.clone();
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

            {
                if let Some(preview_json) = (*deploy_preview_json).clone() {
                    html! {
                        <Popup
                            title="Deploy Preview"
                            size={PopupSize::Lg}
                            on_close={Callback::from({
                                let deploy_preview_json = deploy_preview_json.clone();
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
                                            let deploy_preview_json = deploy_preview_json.clone();
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

            {
                if let Some((inbound_id, inbound_name)) = (*pending_inbound_delete).clone() {
                    html! {
                        <ConfirmPopup
                            title="Delete Inbound"
                            body={format!("Are you sure you want to delete inbound \"{}\"?", inbound_name)}
                            confirm_label="Delete"
                            align_actions_end={true}
                            on_close={Callback::from({
                                let pending_inbound_delete = pending_inbound_delete.clone();
                                move |_| pending_inbound_delete.set(None)
                            })}
                            on_confirm={Callback::from({
                                let draft = draft.clone();
                                let pending_inbound_delete = pending_inbound_delete.clone();
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

            {
                if let Some((outbound_id, outbound_name)) = (*pending_outbound_delete).clone() {
                    html! {
                        <ConfirmPopup
                            title="Delete Outbound"
                            body={format!("Are you sure you want to delete outbound \"{}\"?", outbound_name)}
                            confirm_label="Delete"
                            align_actions_end={true}
                            on_close={Callback::from({
                                let pending_outbound_delete = pending_outbound_delete.clone();
                                move |_| pending_outbound_delete.set(None)
                            })}
                            on_confirm={Callback::from({
                                let draft = draft.clone();
                                let pending_outbound_delete = pending_outbound_delete.clone();
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

            {
                if let Some(rule_index) = *pending_routing_delete {
                    html! {
                        <ConfirmPopup
                            title="Delete Rule"
                            body="Are you sure you want to delete this routing rule?"
                            confirm_label="Delete"
                            align_actions_end={true}
                            on_close={Callback::from({
                                let pending_routing_delete = pending_routing_delete.clone();
                                move |_| pending_routing_delete.set(None)
                            })}
                            on_confirm={Callback::from({
                                let draft = draft.clone();
                                let pending_routing_delete = pending_routing_delete.clone();
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

            {
                if *warp_popup_open {
                    html! {
                        <WarpCreatePopup
                            node_address={node.address.clone()}
                            master_key={node.master_key.clone()}
                            initial_registration={initial_warp_registration(&d)}
                            on_registration_change={Callback::from({
                                let draft = draft.clone();
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
                                let warp_popup_open = warp_popup_open.clone();
                                move |_| warp_popup_open.set(false)
                            })}
                            on_create={Callback::from({
                                let draft = draft.clone();
                                let warp_popup_open = warp_popup_open.clone();
                                let snackbar = snackbar.clone();
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

            {
                if *acme_confirm_open {
                    html! {
                        <ConfirmPopup
                            title="Request Certificate"
                            body="This will trigger ACME issuance on node. Certificate authorities enforce rate limits, so repeated failed attempts can temporarily block new issuance. Continue only if domain, ports, and challenge routing are ready."
                            confirm_label="Request"
                            on_close={Callback::from({
                                let acme_confirm_open = acme_confirm_open.clone();
                                let pending_acme_certificate = pending_acme_certificate.clone();
                                move |_| {
                                    acme_confirm_open.set(false);
                                    pending_acme_certificate.set(None);
                                }
                            })}
                            on_confirm={Callback::from({
                                let acme_confirm_open = acme_confirm_open.clone();
                                let pending_acme_certificate = pending_acme_certificate.clone();
                                let acme_logs = acme_logs.clone();
                                let acme_logs_open = acme_logs_open.clone();
                                let acme_loading = acme_loading.clone();
                                let node = node.clone();
                                let draft = draft.clone();
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

            {
                if *acme_logs_open {
                    if let Some(result) = &*acme_logs {
                        html! {
                            <AcmeLogsPopup
                                title="ACME Logs"
                                logs={result.logs.clone()}
                                loading={*acme_loading}
                                success={result.success}
                                error={result.error.clone()}
                                on_close={Callback::from({
                                    let acme_logs_open = acme_logs_open.clone();
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

            {
                if let Some((server_index, server, is_new)) = &*editing_dns_server {
                    html! {
                        <DnsServerEditorPopup
                            server={server.clone()}
                            is_new={*is_new}
                            on_close={Callback::from({
                                let editing_dns_server = editing_dns_server.clone();
                                move |_| editing_dns_server.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_dns_server = editing_dns_server.clone();
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

            {
                if let Some((host_index, host, is_new)) = &*editing_dns_host {
                    html! {
                        <DnsHostEditorPopup
                            host={host.clone()}
                            is_new={*is_new}
                            on_close={Callback::from({
                                let editing_dns_host = editing_dns_host.clone();
                                move |_| editing_dns_host.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_dns_host = editing_dns_host.clone();
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

            {
                if let Some((certificate, is_new)) = &*editing_certificate {
                    html! {
                        <CertificateEditorPopup
                            certificate={certificate.clone()}
                            is_new={*is_new}
                            on_close={Callback::from({
                                let editing_certificate = editing_certificate.clone();
                                move |_| editing_certificate.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_certificate = editing_certificate.clone();
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

            {
                if let Some((inbound, is_new)) = &*editing_inbound {
                    html! {
                        <InboundEditorPopup
                            inbound={inbound.clone()}
                            certificates={d.certificates.clone()}
                            is_new={*is_new}
                            on_close={Callback::from({
                                let editing_inbound = editing_inbound.clone();
                                move |_| editing_inbound.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_inbound = editing_inbound.clone();
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

            {
                if let Some((outbound, is_new)) = &*editing_outbound {
                    html! {
                        <OutboundEditorPopup
                            outbound={outbound.clone()}
                            is_new={*is_new}
                            node_address={node.address.clone()}
                            master_key={node.master_key.clone()}
                            on_close={Callback::from({
                                let editing_outbound = editing_outbound.clone();
                                move |_| editing_outbound.set(None)
                            })}
                            on_save={Callback::from({
                                let draft = draft.clone();
                                let editing_outbound = editing_outbound.clone();
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

            {
                if let Some(inbound) = selected_access_inbound {
                    html! {
                        <AccessLinkPopup
                            node={node.clone()}
                            inbound={inbound}
                            accounts={state.accounts.clone()}
                            on_close={Callback::from({
                                let access_link_inbound_id = access_link_inbound_id.clone();
                                move |_| access_link_inbound_id.set(None)
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
