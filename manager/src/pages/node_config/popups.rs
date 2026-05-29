use super::*;

#[derive(Properties, PartialEq)]
pub(super) struct SectionProps {
    pub(super) title: AttrValue,
    #[prop_or_default]
    pub(super) children: Children,
}

#[function_component(ConfigSection)]
pub(super) fn config_section(props: &SectionProps) -> Html {
    html! {
        <div class="md3-card bg-surface-container">
            <h2 class="text-xl font-semibold mb-4">{ props.title.clone() }</h2>
            <div class="space-y-4">
                { for props.children.iter() }
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct ConfirmPopupProps {
    pub(super) title: AttrValue,
    pub(super) body: AttrValue,
    pub(super) confirm_label: AttrValue,
    #[prop_or_default]
    pub(super) extra_label: Option<AttrValue>,
    #[prop_or(false)]
    pub(super) align_actions_end: bool,
    #[prop_or_default]
    pub(super) on_extra: Option<Callback<()>>,
    pub(super) on_confirm: Callback<()>,
    pub(super) on_close: Callback<()>,
}

#[function_component(ConfirmPopup)]
pub(super) fn confirm_popup(props: &ConfirmPopupProps) -> Html {
    let on_confirm = {
        let on_confirm = props.on_confirm.clone();
        Callback::from(move |_| on_confirm.emit(()))
    };
    let on_close_btn = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };
    let on_extra = props
        .on_extra
        .clone()
        .map(|on_extra| Callback::from(move |_| on_extra.emit(())));

    html! {
        <Popup title={props.title.clone()} size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-6">
                <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant); line-height: 1.5;">
                    { props.body.clone() }
                </div>
                <div class="md3-popup-actions" style={if props.align_actions_end { "justify-content: flex-end;" } else { "" }}>
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={on_close_btn} />
                    {
                        if let (Some(label), Some(on_extra)) = (props.extra_label.clone(), on_extra) {
                            html! { <Button label={label.to_string()} button_type={ButtonType::Outlined} onclick={on_extra} /> }
                        } else {
                            html! {}
                        }
                    }
                    <Button label={props.confirm_label.to_string()} button_type={ButtonType::Filled} onclick={on_confirm} />
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct AcmeLogsPopupProps {
    pub(super) title: AttrValue,
    pub(super) logs: Vec<String>,
    pub(super) loading: bool,
    pub(super) success: bool,
    pub(super) error: String,
    pub(super) on_close: Callback<()>,
}

#[function_component(AcmeLogsPopup)]
pub(super) fn acme_logs_popup(props: &AcmeLogsPopupProps) -> Html {
    let log_text = if props.logs.is_empty() {
        "Waiting for node response...".to_string()
    } else {
        props.logs.join("\n")
    };

    html! {
        <Popup title={props.title.clone()} size={PopupSize::Lg} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                    {
                        if props.loading {
                            "Request in progress..."
                        } else if props.success {
                            "Certificate request finished successfully."
                        } else {
                            "Certificate request failed."
                        }
                    }
                </div>
                {
                    if !props.error.is_empty() {
                        html! {
                            <div class="text-sm" style="color: #F2B8B5;">
                                { props.error.clone() }
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                <pre class="md3-code-block">{ log_text }</pre>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct NamePromptPopupProps {
    pub(super) title: AttrValue,
    pub(super) label: AttrValue,
    pub(super) confirm_label: AttrValue,
    pub(super) initial_value: String,
    pub(super) on_confirm: Callback<String>,
    pub(super) on_close: Callback<()>,
}

#[function_component(NamePromptPopup)]
pub(super) fn name_prompt_popup(props: &NamePromptPopupProps) -> Html {
    let value = use_state(|| props.initial_value.clone());
    let error = use_state(|| Option::<String>::None);

    let on_change = {
        let value = value.clone();
        let error = error.clone();
        Callback::from(move |next: String| {
            value.set(next);
            error.set(None);
        })
    };

    let on_confirm = {
        let value = value.clone();
        let error = error.clone();
        let on_confirm = props.on_confirm.clone();
        Callback::from(move |_| {
            let next = value.trim().to_string();
            if next.is_empty() {
                error.set(Some("Name is required.".to_string()));
                return;
            }
            on_confirm.emit(next);
        })
    };

    let on_close_btn = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    html! {
        <Popup title={props.title.clone()} size={PopupSize::Sm} on_close={props.on_close.clone()}>
            <div class="space-y-6">
                <TextBox
                    label={props.label.to_string()}
                    value={(*value).clone()}
                    onchange={on_change}
                    error={(*error).clone()}
                />
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={on_close_btn} />
                    <Button label={props.confirm_label.to_string()} button_type={ButtonType::Filled} onclick={on_confirm} />
                </div>
            </div>
        </Popup>
    }
}
