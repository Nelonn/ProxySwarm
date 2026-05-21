use crate::country::flag_emoji;
use crate::components::{
    Button, ButtonType, DatePicker, DatePickerType, Popup, PopupSize, RichTable, TextBox,
};
use crate::services::registry_deploy::collect_account_proxy_links;
use crate::state::{normalize_groups, AccountInfo, State};
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::window;
use yew::prelude::*;

#[function_component(Accounts)]
pub fn accounts() -> Html {
    let state = use_context::<UseStateHandle<State>>().expect("state context");
    let show_modal = use_state(|| false);
    let editing_account = use_state(|| Option::<AccountInfo>::None);
    let pending_delete = use_state(|| Option::<AccountInfo>::None);
    let show_groups_popup = use_state(|| false);
    let proxies_account = use_state(|| Option::<AccountInfo>::None);

    let on_add = {
        let show_modal = show_modal.clone();
        let editing_account = editing_account.clone();
        Callback::from(move |_| {
            editing_account.set(None);
            show_modal.set(true);
        })
    };

    html! {
        <div class="p-6 space-y-6">
            <div class="flex justify-between" style="align-items: baseline;">
                <h1 class="text-3xl font-bold">{ "Accounts" }</h1>
                <div class="flex items-center" style="gap: 0.5rem;">
                    <Button
                        label="Groups"
                        button_type={ButtonType::Outlined}
                        onclick={{
                            let show_groups_popup = show_groups_popup.clone();
                            move |_| show_groups_popup.set(true)
                        }}
                    />
                    <Button
                        label="Add Account"
                        icon={Some("icon-add".to_string())}
                        button_type={ButtonType::Filled}
                        onclick={move |_| on_add.emit(())}
                    />
                </div>
            </div>

            // Accounts List
            { if state.accounts.is_empty() {
                html! {
                    <div class="md3-card p-12 text-center">
                        <p class="text-xl opacity-70">{ "No accounts configured" }</p>
                        <p class="text-sm opacity-50 mt-2">{ "Add your first account to get started" }</p>
                    </div>
                }
            } else {
                html! {
                    <RichTable columns={vec![
                        "ID".to_string(),
                        "Account".to_string(),
                        "Token".to_string(),
                        "Groups".to_string(),
                        "Allowed IPs".to_string(),
                        "Expires".to_string(),
                        "Actions".to_string(),
                    ]}>
                        { for state.accounts.iter().map(|account| {
                            let state = state.clone();
                            let account_id = account.id.clone();
                            let pending_delete_handle = pending_delete.clone();

                            let on_delete = Callback::from(move |_: MouseEvent| {
                                if let Some(account) = state.accounts.iter().find(|a| a.id == account_id) {
                                    pending_delete_handle.set(Some(account.clone()));
                                }
                            });

                            html! {
                                <>
                                    <AccountRow
                                        account={account.clone()}
                                        on_proxies={{
                                            let proxies_account = proxies_account.clone();
                                            let account_for_proxies = account.clone();
                                            Callback::from(move |_| proxies_account.set(Some(account_for_proxies.clone())))
                                        }}
                                        on_edit={{
                                            let show_modal = show_modal.clone();
                                            let editing_account = editing_account.clone();
                                            let account_for_edit = account.clone();
                                            Callback::from(move |_| {
                                                editing_account.set(Some(account_for_edit.clone()));
                                                show_modal.set(true);
                                            })
                                        }}
                                        on_delete={on_delete}
                                    />
                                    <div class="md3-divider"></div>
                                </>
                            }
                        }) }
                    </RichTable>
                }
            } }

            { if *show_modal {
                html! {
                    <AccountModal
                        state={state.clone()}
                        initial_account={(*editing_account).clone()}
                        on_close={Callback::from(move |_| show_modal.set(false))}
                    />
                }
            } else {
                html! {}
            } }

            { if *show_groups_popup {
                html! {
                    <GroupsPopup
                        state={state.clone()}
                        on_close={Callback::from({
                            let show_groups_popup = show_groups_popup.clone();
                            move |_| show_groups_popup.set(false)
                        })}
                    />
                }
            } else {
                html! {}
            } }

            { if let Some(account) = &*proxies_account {
                html! {
                    <AccountProxiesPopup
                        state={state.clone()}
                        account={account.clone()}
                        on_close={Callback::from({
                            let proxies_account = proxies_account.clone();
                            move |_| proxies_account.set(None)
                        })}
                    />
                }
            } else {
                html! {}
            } }

            { if let Some(account) = &*pending_delete {
                let state = state.clone();
                let pending_delete_close = pending_delete.clone();
                let pending_delete_confirm = pending_delete.clone();
                let account_id = account.id.clone();
                html! {
                    <DeleteConfirmPopup
                        title={"Delete Account"}
                        message={format!("Delete account \"{}\"?", account.name)}
                        on_cancel={Callback::from(move |_| pending_delete_close.set(None))}
                        on_confirm={Callback::from(move |_| {
                            let mut new_state = (*state).clone();
                            new_state.accounts.retain(|a| a.id != account_id);
                            new_state.save();
                            state.set(new_state);
                            pending_delete_confirm.set(None);
                        })}
                    />
                }
            } else {
                html! {}
            } }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct AccountCardProps {
    account: AccountInfo,
    on_proxies: Callback<MouseEvent>,
    on_edit: Callback<MouseEvent>,
    on_delete: Callback<MouseEvent>,
}

#[function_component(AccountRow)]
fn account_row(props: &AccountCardProps) -> Html {
    let show_token = use_state(|| false);

    let expiry_date = if props.account.expiry_date > 0 {
        chrono::DateTime::from_timestamp(props.account.expiry_date, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "Never".to_string())
    } else {
        "Never".to_string()
    };

    let on_edit_click = props.on_edit.clone();
    let on_delete_click = props.on_delete.clone();
    let on_proxies_click = props.on_proxies.clone();
    let token_click = {
        let show_token = show_token.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            show_token.set(!*show_token);
        })
    };

    let token_hidden = "************";
    let token_html = if *show_token {
        html! { <code class="md3-secret md3-secret-revealed break-all">{ &props.account.token }</code> }
    } else {
        html! { <code class="md3-secret md3-secret-hidden">{ token_hidden }</code> }
    };

    html! {
        <div class="md3-list-row">
            <div class="md3-list-col">
                <code>{ &props.account.id }</code>
            </div>
            <div class="md3-list-col md3-list-col-main">
                <div class="text-lg font-bold">{ &props.account.name }</div>
            </div>
            <div class="md3-list-col md3-list-col-token">
                <button
                    class="md3-secret-btn"
                    onclick={token_click}
                    aria-pressed={show_token.to_string()}
                >
                    { token_html }
                </button>
            </div>
            <div class="md3-list-col">
                <div class="text-sm opacity-70">{ props.account.groups.join(", ") }</div>
            </div>
            <div class="md3-list-col">
                <div class="text-sm opacity-70">
                    {
                        if props.account.allowed_ips.is_empty() {
                            "Any".to_string()
                        } else {
                            props.account.allowed_ips.len().to_string()
                        }
                    }
                </div>
            </div>
            <div class="md3-list-col">
                <div class="text-sm opacity-70">{ expiry_date }</div>
            </div>
            <div class="md3-list-col md3-list-col-actions">
                <div class="md3-list-actions">
                    <Button
                        label="Proxies"
                        button_type={ButtonType::Outlined}
                        onclick={on_proxies_click}
                    />
                    <Button
                        label="Edit"
                        button_type={ButtonType::Outlined}
                        onclick={on_edit_click}
                    />
                    <Button
                        label="Delete"
                        button_type={ButtonType::Outlined}
                        color={Some("#F2B8B5".to_string())}
                        onclick={on_delete_click}
                    />
                </div>
            </div>
        </div>
    }
}

async fn copy_to_clipboard(text: String) -> Result<(), String> {
    let Some(window) = window() else {
        return Err("Clipboard unavailable".to_string());
    };

    let navigator = js_sys::Reflect::get(&window, &JsValue::from_str("navigator"))
        .map_err(|_| "Clipboard unavailable".to_string())?;
    let clipboard = js_sys::Reflect::get(&navigator, &JsValue::from_str("clipboard"))
        .map_err(|_| "Clipboard unavailable".to_string())?;
    let write_text = js_sys::Reflect::get(&clipboard, &JsValue::from_str("writeText"))
        .map_err(|_| "Clipboard unavailable".to_string())?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "Clipboard unavailable".to_string())?;

    let promise = write_text
        .call1(&clipboard, &JsValue::from_str(&text))
        .map_err(|_| "Clipboard unavailable".to_string())?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "Clipboard unavailable".to_string())?;

    JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|_| "Copy failed".to_string())
}

#[derive(Properties, PartialEq)]
struct AccountProxiesPopupProps {
    state: UseStateHandle<State>,
    account: AccountInfo,
    on_close: Callback<()>,
}

#[function_component(AccountProxiesPopup)]
fn account_proxies_popup(props: &AccountProxiesPopupProps) -> Html {
    let result = collect_account_proxy_links(&props.state, &props.account);
    let links = result.links;
    let skipped = result.skipped;
    let copy_error = use_state(|| Option::<String>::None);

    html! {
        <Popup
            title={format!("Proxies: {}", props.account.name)}
            size={PopupSize::Lg}
            on_close={props.on_close.clone()}
        >
            <div class="space-y-4">
                {
                    if links.is_empty() {
                        html! {
                            <div class="text-sm opacity-70">{ "No available access links for this account." }</div>
                        }
                    } else {
                        html! {
                            <RichTable columns={vec![
                                "Source".to_string(),
                                "Link".to_string(),
                                "Actions".to_string(),
                            ]} header_in_list={true} card_class={Some("bg-surface-container".to_string())}>
                                {
                                    for links.iter().cloned().map(|item| {
                                        let link_for_copy = item.link.clone();
                                        let flag = flag_emoji(&item.node_country);
                                        let source = {
                                            let node = item.node_name.trim();
                                            let inbound = item.inbound_name.trim();
                                            let mut parts = Vec::new();
                                            if !node.is_empty() {
                                                parts.push(node.to_string());
                                            }
                                            if !inbound.is_empty() {
                                                parts.push(inbound.to_string());
                                            }
                                            if parts.is_empty() {
                                                "-".to_string()
                                            } else if flag.is_empty() {
                                                parts.join(" | ")
                                            } else {
                                                format!("{} {}", flag, parts.join(" | "))
                                            }
                                        };
                                        html! {
                                            <>
                                                <div class="md3-divider"></div>
                                                <div class="md3-list-row">
                                                    <div class="md3-list-col">
                                                        <div class="text-sm opacity-70">{ source }</div>
                                                    </div>
                                                    <div class="md3-list-col md3-list-col-main">
                                                        <TextBox
                                                            label="Proxy"
                                                            value={item.link}
                                                            onchange={Callback::from(|_: String| {})}
                                                            disabled={true}
                                                            is_textarea={true}
                                                        />
                                                    </div>
                                                    <div class="md3-list-col md3-list-col-actions">
                                                        <div class="md3-list-actions">
                                                            <Button
                                                                label="Copy"
                                                                button_type={ButtonType::Outlined}
                                                                onclick={Callback::from({
                                                                    let copy_error = copy_error.clone();
                                                                    move |_| {
                                                                        copy_error.set(None);
                                                                        let copy_error = copy_error.clone();
                                                                        let link = link_for_copy.clone();
                                                                        spawn_local(async move {
                                                                            if let Err(error) = copy_to_clipboard(link).await {
                                                                                copy_error.set(Some(error));
                                                                            }
                                                                        });
                                                                    }
                                                                })}
                                                            />
                                                        </div>
                                                    </div>
                                                </div>
                                            </>
                                        }
                                    })
                                }
                            </RichTable>
                        }
                    }
                }
                {
                    if !skipped.is_empty() {
                        html! {
                            <div class="space-y-2">
                                <div class="text-sm font-medium opacity-80">{ "Skipped" }</div>
                                <div class="space-y-1">
                                    {
                                        for skipped.iter().map(|item| html! {
                                            <div class="text-sm opacity-70">{ item }</div>
                                        })
                                    }
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    if let Some(error) = &*copy_error {
                        html! { <div class="text-sm opacity-70">{ error }</div> }
                    } else {
                        html! {}
                    }
                }
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button
                        label="Close"
                        button_type={ButtonType::Text}
                        onclick={Callback::from({
                            let on_close = props.on_close.clone();
                            move |_| on_close.emit(())
                        })}
                    />
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct AccountModalProps {
    state: UseStateHandle<State>,
    #[prop_or_default]
    initial_account: Option<AccountInfo>,
    on_close: Callback<()>,
}

#[function_component(AccountModal)]
fn account_modal(props: &AccountModalProps) -> Html {
    let name = use_state(|| String::new());
    let token = use_state(|| String::new());
    let allowed_ips = use_state(|| String::new());
    let groups = use_state(|| "default".to_string());
    let expiry_date_str = use_state(|| String::new());

    {
        let name = name.clone();
        let token = token.clone();
        let allowed_ips = allowed_ips.clone();
        let groups = groups.clone();
        let expiry_date_str = expiry_date_str.clone();
        let initial_account = props.initial_account.clone();

        use_effect_with(initial_account, move |initial_account| {
            if let Some(account) = initial_account {
                name.set(account.name.clone());
                token.set(account.token.clone());
                allowed_ips.set(account.allowed_ips.join(", "));
                groups.set(normalize_groups(&account.groups).join(", "));
                let dt_str = chrono::DateTime::from_timestamp(account.expiry_date, 0)
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M").to_string())
                    .unwrap_or_default();
                expiry_date_str.set(if account.expiry_date > 0 {
                    dt_str
                } else {
                    String::new()
                });
            } else {
                name.set(String::new());
                token.set(String::new());
                allowed_ips.set(String::new());
                groups.set("default".to_string());
                expiry_date_str.set(String::new());
            }
            || ()
        });
    }

    let name_for_change = name.clone();
    let token_for_change = token.clone();
    let allowed_ips_for_change = allowed_ips.clone();
    let groups_for_change = groups.clone();
    let expiry_date_str_for_change = expiry_date_str.clone();

    let on_name_change = Callback::from(move |value: String| {
        name_for_change.set(value);
    });

    let on_token_change = Callback::from(move |value: String| {
        token_for_change.set(value);
    });

    let on_allowed_ips_change = Callback::from(move |value: String| {
        allowed_ips_for_change.set(value);
    });
    let on_groups_change = Callback::from(move |value: String| {
        groups_for_change.set(value);
    });

    let on_expiry_change = Callback::from(move |value: String| {
        expiry_date_str_for_change.set(value);
    });

    let name_for_submit = name.clone();
    let token_for_submit = token.clone();
    let allowed_ips_for_submit = allowed_ips.clone();
    let groups_for_submit = groups.clone();
    let expiry_date_str_for_submit = expiry_date_str.clone();
    let initial_account_for_submit = props.initial_account.clone();

    let on_submit = {
        let state = props.state.clone();
        let on_close = props.on_close.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            // Parse allowed IPs
            let allowed_ips_vec: Vec<String> = if allowed_ips_for_submit.is_empty() {
                Vec::new()
            } else {
                allowed_ips_for_submit
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };
            let groups_vec: Vec<String> = normalize_groups(
                &groups_for_submit
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>(),
            );

            // Parse expiry date
            let expiry_date = if expiry_date_str_for_submit.is_empty() {
                0
            } else {
                // `datetime-local` returns `YYYY-MM-DDTHH:MM` (no timezone).
                chrono::NaiveDateTime::parse_from_str(&expiry_date_str_for_submit, "%Y-%m-%dT%H:%M")
                    .ok()
                    .map(|dt| dt.and_utc().timestamp())
                    .unwrap_or(0)
            };

            let mut new_state = (*state).clone();
            if let Some(existing) = &initial_account_for_submit {
                let mut updated = false;
                for a in &mut new_state.accounts {
                    if a.id == existing.id {
                        a.name = (*name_for_submit).clone();
                        a.token = if token_for_submit.trim().is_empty() {
                            existing.token.clone()
                        } else {
                            token_for_submit.trim().to_string()
                        };
                        a.allowed_ips = allowed_ips_vec.clone();
                        a.groups = groups_vec.clone();
                        a.creation_date = if a.creation_date > 0 {
                            a.creation_date
                        } else {
                            chrono::Utc::now().timestamp()
                        };
                        a.expiry_date = expiry_date;
                        updated = true;
                        break;
                    }
                }
                if !updated {
                    new_state.accounts.push(AccountInfo {
                        id: existing.id.clone(),
                        name: (*name_for_submit).clone(),
                        token: if token_for_submit.trim().is_empty() {
                            existing.token.clone()
                        } else {
                            token_for_submit.trim().to_string()
                        },
                        allowed_ips: allowed_ips_vec,
                        groups: groups_vec,
                        creation_date: if existing.creation_date > 0 {
                            existing.creation_date
                        } else {
                            chrono::Utc::now().timestamp()
                        },
                        expiry_date,
                    });
                }
            } else {
                new_state.accounts.push(AccountInfo {
                    id: Uuid::new_v4().simple().to_string().chars().take(8).collect(),
                    name: (*name_for_submit).clone(),
                    token: if token_for_submit.trim().is_empty() {
                        Uuid::new_v4().to_string()
                    } else {
                        token_for_submit.trim().to_string()
                    },
                    allowed_ips: allowed_ips_vec,
                    groups: groups_vec,
                    creation_date: chrono::Utc::now().timestamp(),
                    expiry_date,
                });
            }
            new_state.save();
            state.set(new_state);
            on_close.emit(());
        })
    };

    let on_close_click = props.on_close.clone();
    let is_edit = props.initial_account.is_some();

    html! {
        <Popup
            title={if is_edit { "Edit Account" } else { "Add New Account" }}
            size={PopupSize::Lg}
            on_close={props.on_close.clone()}
        >
            <form onsubmit={on_submit} class="space-y-4">
                <TextBox
                    label="Account Name"
                    value={(*name).clone()}
                    onchange={on_name_change}
                    placeholder="My Account"
                />
                <TextBox
                    label="Token (Password)"
                    value={(*token).clone()}
                    onchange={on_token_change}
                    placeholder="Required for user auth"
                    action_icon={Some("icon-sync".to_string())}
                    action_label={Some("Randomize token".to_string())}
                    action_onclick={Some(Callback::from({
                        let token = token.clone();
                        move |_| token.set(Uuid::new_v4().to_string())
                    }))}
                />
                <TextBox
                    label="Groups (comma-separated)"
                    value={(*groups).clone()}
                    onchange={on_groups_change}
                    placeholder="default, premium"
                />
                <TextBox
                    label="Allowed IPs (comma-separated, optional)"
                    value={(*allowed_ips).clone()}
                    onchange={on_allowed_ips_change}
                    placeholder="192.168.1.100, 10.0.0.50"
                />
                <DatePicker
                    label="Expiry Date (optional)"
                    value={(*expiry_date_str).clone()}
                    onchange={on_expiry_change}
                    picker_type={DatePickerType::DateTimeLocal}
                    show_trigger_button={false}
                />

                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button
                        label={if is_edit { "Save Changes" } else { "Add Account" }}
                        html_type={"submit"}
                        button_type={ButtonType::Filled}
                        onclick={Callback::from(move |_| {})}
                    />
                    <Button
                        label="Cancel"
                        button_type={ButtonType::Text}
                        onclick={move |_| on_close_click.emit(())}
                    />
                </div>
            </form>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct GroupsPopupProps {
    state: UseStateHandle<State>,
    on_close: Callback<()>,
}

#[function_component(GroupsPopup)]
fn groups_popup(props: &GroupsPopupProps) -> Html {
    let value = use_state(|| normalize_groups(&props.state.groups).join(", "));
    let on_change = {
        let value = value.clone();
        Callback::from(move |next: String| value.set(next))
    };
    let on_save = {
        let state = props.state.clone();
        let on_close = props.on_close.clone();
        let value = value.clone();
        Callback::from(move |_| {
            let parsed = value
                .split(',')
                .map(|item| item.trim().to_string())
                .collect::<Vec<_>>();
            let normalized = normalize_groups(&parsed);
            let mut next = (*state).clone();
            next.groups = normalized.clone();
            for account in &mut next.accounts {
                account.groups = normalize_groups(&account.groups);
            }
            for node in &mut next.nodes {
                node.groups = normalize_groups(&node.groups);
            }
            next.save();
            state.set(next);
            on_close.emit(());
        })
    };

    html! {
        <Popup title={"Groups"} size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <TextBox
                    label="Groups (comma-separated)"
                    value={(*value).clone()}
                    onchange={on_change}
                    placeholder="default, premium, staff"
                />
                <div class="text-sm opacity-70">{ "Default group always exists and is auto-added." }</div>
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={{
                        let on_close = props.on_close.clone();
                        move |_| on_close.emit(())
                    }} />
                    <Button label="Save" button_type={ButtonType::Filled} onclick={on_save} />
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct DeleteConfirmPopupProps {
    title: AttrValue,
    message: String,
    on_cancel: Callback<()>,
    on_confirm: Callback<()>,
}

#[function_component(DeleteConfirmPopup)]
fn delete_confirm_popup(props: &DeleteConfirmPopupProps) -> Html {
    let on_cancel_click = props.on_cancel.clone();
    let on_confirm_click = props.on_confirm.clone();

    html! {
        <Popup
            title={props.title.clone()}
            size={PopupSize::Sm}
            on_close={props.on_cancel.clone()}
        >
            <div class="space-y-4">
                <p class="text-sm opacity-80">{ &props.message }</p>
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button
                        label="Cancel"
                        button_type={ButtonType::Text}
                        onclick={move |_| on_cancel_click.emit(())}
                    />
                    <Button
                        label="Delete"
                        button_type={ButtonType::Outlined}
                        color={Some("#F2B8B5".to_string())}
                        onclick={move |_| on_confirm_click.emit(())}
                    />
                </div>
            </div>
        </Popup>
    }
}
