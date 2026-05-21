use super::*;

pub(super) fn option_bool_value(value: Option<bool>) -> String {
    match value {
        Some(true) => "true".to_string(),
        Some(false) => "false".to_string(),
        None => String::new(),
    }
}

pub(super) fn option_bool_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Enabled",
        Some(false) => "Disabled",
        None => "Inherit",
    }
}

pub(super) fn option_bool_from_value(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "enabled" | "on" => Some(true),
        "false" | "disabled" | "off" => Some(false),
        _ => None,
    }
}

pub(super) fn default_dns_server_draft() -> DnsServerDraft {
    DnsServerDraft {
        port: 53,
        timeout_ms: 5000,
        ..DnsServerDraft::default()
    }
}

pub(super) fn dns_server_summary(server: &DnsServerDraft) -> String {
    let mut parts = vec![format!("Port {}", server.port)];
    if !server.client_ip.trim().is_empty() {
        parts.push(format!("Client {}", server.client_ip.trim()));
    }
    if !server.query_strategy.trim().is_empty() {
        parts.push(server.query_strategy.trim().to_string());
    }
    parts.join(" · ")
}

pub(super) fn dns_server_details(server: &DnsServerDraft) -> String {
    let mut details = Vec::new();
    if !server.domains.trim().is_empty() {
        details.push(format!("Domains: {}", server.domains.replace('\n', ", ")));
    }
    if !server.expect_ips.trim().is_empty() {
        details.push(format!("Expect: {}", server.expect_ips.replace('\n', ", ")));
    }
    if !server.unexpected_ips.trim().is_empty() {
        details.push(format!(
            "Unexpected: {}",
            server.unexpected_ips.replace('\n', ", ")
        ));
    }
    if details.is_empty() {
        "-".to_string()
    } else {
        details.join(" · ")
    }
}

pub(super) fn dns_host_summary(host: &DnsHostDraft) -> String {
    let values = split_lines_csv(&host.values);
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct DnsServerEditorPopupProps {
    pub(super) server: DnsServerDraft,
    pub(super) is_new: bool,
    pub(super) on_close: Callback<()>,
    pub(super) on_save: Callback<DnsServerDraft>,
}

#[function_component(DnsServerEditorPopup)]
pub(super) fn dns_server_editor_popup(props: &DnsServerEditorPopupProps) -> Html {
    let server = use_state(|| props.server.clone());

    let update_text = |mutator: fn(&mut DnsServerDraft, String)| {
        let server = server.clone();
        Callback::from(move |value: String| {
            let mut next = (*server).clone();
            mutator(&mut next, value);
            server.set(next);
        })
    };
    let update_u32 = |mutator: fn(&mut DnsServerDraft, u32)| {
        let server = server.clone();
        Callback::from(move |value: String| {
            let mut next = (*server).clone();
            mutator(&mut next, value.parse::<u32>().unwrap_or(0));
            server.set(next);
        })
    };
    let update_u64 = |mutator: fn(&mut DnsServerDraft, u64)| {
        let server = server.clone();
        Callback::from(move |value: String| {
            let mut next = (*server).clone();
            mutator(&mut next, value.parse::<u64>().unwrap_or(0));
            server.set(next);
        })
    };
    let update_bool = |mutator: fn(&mut DnsServerDraft, bool)| {
        let server = server.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            let mut next = (*server).clone();
            mutator(&mut next, input.checked());
            server.set(next);
        })
    };
    let update_option_bool = |mutator: fn(&mut DnsServerDraft, Option<bool>)| {
        let server = server.clone();
        Callback::from(move |value: String| {
            let mut next = (*server).clone();
            mutator(&mut next, option_bool_from_value(&value));
            server.set(next);
        })
    };

    let data = (*server).clone();
    let popup_title = if props.is_new {
        "Add DNS Server"
    } else {
        "Edit DNS Server"
    };

    let on_save = {
        let on_save = props.on_save.clone();
        let server = server.clone();
        Callback::from(move |_| on_save.emit((*server).clone()))
    };

    html! {
        <Popup title={popup_title} size={PopupSize::Lg} on_close={props.on_close.clone()}>
            <div class="space-y-6">
                <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                    <TextBox label="Address" value={data.address.clone()} onchange={update_text(|server, value| server.address = value)} placeholder="https://1.1.1.1/dns-query" />
                    <TextBox label="Port" value={data.port.to_string()} onchange={update_u32(|server, value| server.port = value)} input_type="number" />
                    <TextBox label="Tag" value={data.tag.clone()} onchange={update_text(|server, value| server.tag = value)} placeholder="cloudflare" />
                    <TextBox label="Client IP" value={data.client_ip.clone()} onchange={update_text(|server, value| server.client_ip = value)} placeholder="Optional" />
                    <TextBox label="Query Strategy" value={data.query_strategy.clone()} onchange={update_text(|server, value| server.query_strategy = value)} placeholder="Optional" />
                    <TextBox label="Timeout ms" value={data.timeout_ms.to_string()} onchange={update_u64(|server, value| server.timeout_ms = value)} input_type="number" />
                </div>
                <TextBox label="Domains" value={data.domains.clone()} onchange={update_text(|server, value| server.domains = value)} is_textarea={true} placeholder="example.com, api.example.com" />
                <TextBox label="Expected IPs" value={data.expect_ips.clone()} onchange={update_text(|server, value| server.expect_ips = value)} is_textarea={true} placeholder="1.1.1.1, 1.0.0.1" />
                <TextBox label="Unexpected IPs" value={data.unexpected_ips.clone()} onchange={update_text(|server, value| server.unexpected_ips = value)} is_textarea={true} placeholder="0.0.0.0, 127.0.0.1" />
                <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                    <Dropdown
                        label="Disable Cache"
                        value={option_bool_value(data.disable_cache)}
                        options={vec![
                            DropdownOption { value: String::new(), label: "Inherit".to_string() },
                            DropdownOption { value: "true".to_string(), label: "Enabled".to_string() },
                            DropdownOption { value: "false".to_string(), label: "Disabled".to_string() },
                        ]}
                        onchange={update_option_bool(|server, value| server.disable_cache = value)}
                    />
                    <Dropdown
                        label="Serve Stale"
                        value={option_bool_value(data.serve_stale)}
                        options={vec![
                            DropdownOption { value: String::new(), label: "Inherit".to_string() },
                            DropdownOption { value: "true".to_string(), label: "Enabled".to_string() },
                            DropdownOption { value: "false".to_string(), label: "Disabled".to_string() },
                        ]}
                        onchange={update_option_bool(|server, value| server.serve_stale = value)}
                    />
                    <TextBox
                        label="Serve Expired TTL"
                        value={data.serve_expired_ttl.map(|value| value.to_string()).unwrap_or_default()}
                        onchange={Callback::from({
                            let server = server.clone();
                            move |value: String| {
                                let mut next = (*server).clone();
                                next.serve_expired_ttl = value.parse::<u32>().ok();
                                server.set(next);
                            }
                        })}
                        input_type="number"
                        placeholder="Optional"
                    />
                    <SwitchField
                        label="Skip Fallback"
                        checked={data.skip_fallback}
                        onchange={update_bool(|server, value| server.skip_fallback = value)}
                    />
                    <SwitchField
                        label="Final Query"
                        checked={data.final_query}
                        onchange={update_bool(|server, value| server.final_query = value)}
                    />
                </div>
                <div class="md3-popup-actions">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    <Button label="Save" button_type={ButtonType::Filled} onclick={on_save} />
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct DnsHostEditorPopupProps {
    pub(super) host: DnsHostDraft,
    pub(super) is_new: bool,
    pub(super) on_close: Callback<()>,
    pub(super) on_save: Callback<DnsHostDraft>,
}

#[function_component(DnsHostEditorPopup)]
pub(super) fn dns_host_editor_popup(props: &DnsHostEditorPopupProps) -> Html {
    let host = use_state(|| props.host.clone());
    let update_text = |mutator: fn(&mut DnsHostDraft, String)| {
        let host = host.clone();
        Callback::from(move |value: String| {
            let mut next = (*host).clone();
            mutator(&mut next, value);
            host.set(next);
        })
    };

    let data = (*host).clone();
    let popup_title = if props.is_new {
        "Add DNS Host"
    } else {
        "Edit DNS Host"
    };
    let on_save = {
        let on_save = props.on_save.clone();
        let host = host.clone();
        Callback::from(move |_| on_save.emit((*host).clone()))
    };

    html! {
        <Popup title={popup_title} size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-6">
                <TextBox label="Domain" value={data.domain.clone()} onchange={update_text(|host, value| host.domain = value)} placeholder="example.com" />
                <TextBox label="Values" value={data.values.clone()} onchange={update_text(|host, value| host.values = value)} is_textarea={true} placeholder="1.1.1.1, 8.8.8.8" />
                <div class="md3-popup-actions">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    })} />
                    <Button label="Save" button_type={ButtonType::Filled} onclick={on_save} />
                </div>
            </div>
        </Popup>
    }
}


