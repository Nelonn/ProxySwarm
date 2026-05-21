use super::*;

#[derive(Properties, PartialEq)]
pub(super) struct AccessLinkPopupProps {
    pub(super) node: ProxyNode,
    pub(super) inbound: InboundEntryDraft,
    pub(super) accounts: Vec<AccountInfo>,
    pub(super) on_close: Callback<()>,
}

#[function_component(AccessLinkPopup)]
pub(super) fn access_link_popup(props: &AccessLinkPopupProps) -> Html {
    let snackbar = use_context::<SnackbarBus>();
    let allowed_groups = effective_inbound_groups(&props.node.groups, &props.inbound.groups);
    let filtered_accounts = props
        .accounts
        .iter()
        .filter(|account| {
            if allowed_groups.is_empty() {
                return normalize_groups(&props.inbound.groups).is_empty();
            }
            normalize_groups(&account.groups)
                .iter()
                .any(|group| allowed_groups.iter().any(|candidate| candidate == group))
        })
        .cloned()
        .collect::<Vec<_>>();
    let initial_account = filtered_accounts
        .first()
        .map(|account| account.id.clone())
        .unwrap_or_default();
    let selected_account_id = use_state(|| initial_account);
    let copy_status = use_state(|| Option::<String>::None);
    let generated_link = use_state(|| Option::<String>::None);

    let selected_account = filtered_accounts
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
                                    options={filtered_accounts.iter().map(|account| DropdownOption {
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
                                <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">{ "Select user, then click Generate. Empty inbound groups inherit node groups." }</div>
                                {
                                    if filtered_accounts.is_empty() {
                                        html! {
                                            <div class="text-sm" style="color: var(--md-sys-color-error-soft);">{ "No accounts match this inbound's effective groups." }</div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
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
                                })} disabled={filtered_accounts.is_empty()} />
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



