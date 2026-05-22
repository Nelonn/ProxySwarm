use super::*;

#[derive(Properties, PartialEq)]
pub(super) struct RoutingRuleEditorPopupProps {
    pub(super) rule: RoutingRuleDraft,
    pub(super) is_new: bool,
    pub(super) inbound_options: Vec<String>,
    pub(super) user_options: Vec<DropdownOption>,
    pub(super) outbound_options: Vec<DropdownOption>,
    pub(super) on_close: Callback<()>,
    pub(super) on_save: Callback<RoutingRuleDraft>,
}

fn routing_user_label(user_id: &str, user_options: &[DropdownOption]) -> String {
    user_options
        .iter()
        .find(|option| option.value == user_id)
        .map(|option| option.label.clone())
        .unwrap_or_else(|| user_id.to_string())
}

#[function_component(RoutingRuleEditorPopup)]
pub(super) fn routing_rule_editor_popup(props: &RoutingRuleEditorPopupProps) -> Html {
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

            let transports: Vec<String> = match transport_value.as_str() {
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
                    !value.is_empty() && allowed_user_options.iter().any(|opt| &opt.value == value)
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
            .filter(|option| !option.value.trim().is_empty())
            .filter(|option| !selected_users.iter().any(|existing| existing == &option.value))
            .cloned()
            .collect::<Vec<_>>();

        if !needle.is_empty() {
            options.retain(|option| {
                option.value.to_lowercase().contains(&needle)
                    || option.label.to_lowercase().contains(&needle)
            });
        }
        options.sort_by(|a, b| a.label.cmp(&b.label).then(a.value.cmp(&b.value)));
        options.dedup_by(|a, b| a.value == b.value);
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
                <SwitchField
                    label="Enabled"
                    checked={rule.enabled}
                    onchange={Callback::from({
                        let rule = rule.clone();
                        move |e: Event| {
                            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
                            let mut next = (*rule).clone();
                            next.enabled = input.checked();
                            rule.set(next);
                        }
                    })}
                />
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
                                for selected_users.iter().cloned().map(|user_id| {
                                    let rule = rule.clone();
                                    let remove_user_id = user_id.clone();
                                    let chip_label = routing_user_label(&user_id, &props.user_options);
                                    html! {
                                        <Chip
                                            label={AttrValue::from(chip_label)}
                                            mode={ChipMode::Outlined}
                                            trailing_icon={Some("close_24dp".to_string())}
                                            on_trailing_click={Some(Callback::from(move |_| {
                                                let mut next = (*rule).clone();
                                                let remaining = split_lines_csv(&next.user)
                                                    .into_iter()
                                                    .map(|value| value.trim().to_string())
                                                    .filter(|value| !value.is_empty() && value != &remove_user_id)
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
                                        if !combined.iter().any(|value| value == &first.value) {
                                            combined.push(first.value);
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
                                                for user_suggestions.iter().cloned().map(|option| {
                                                    let chip_label = option.label.clone();
                                                    let rule = rule.clone();
                                                    let user_query = user_query.clone();
                                                    let user_open = user_open.clone();
                                                    let selected_users = selected_users.clone();
                                                    html! {
                                                        <span onmousedown={Callback::from(move |e: MouseEvent| {
                                                            e.prevent_default();
                                                            let mut next = (*rule).clone();
                                                            let mut combined = selected_users.clone();
                                                            if !combined.iter().any(|value| value == &option.value) {
                                                                combined.push(option.value.clone());
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


