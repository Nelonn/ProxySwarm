use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::{
    Button, ButtonType, Popup, PopupSize, RichTable, SnackbarBus, Switch, TextBox,
};
use crate::pb::proxyswarm::RegistryStatusResponse;
use crate::services::registry_api::RegistryApiService;
use crate::services::registry_deploy::deploy_all_registries;
use crate::state::{RegistryInfo, State};

#[function_component(Registries)]
pub fn registries() -> Html {
    let state = use_context::<UseStateHandle<State>>().expect("state context");
    let snackbar = use_context::<SnackbarBus>();
    let show_modal = use_state(|| false);
    let editing_registry = use_state(|| Option::<RegistryInfo>::None);
    let pending_delete = use_state(|| Option::<RegistryInfo>::None);
    let pending_deploy = use_state(|| false);
    let deploy_loading = use_state(|| false);
    let status_registry = use_state(|| Option::<RegistryInfo>::None);

    let on_open_modal = {
        let show_modal = show_modal.clone();
        let editing_registry = editing_registry.clone();
        Callback::from(move |_| {
            editing_registry.set(None);
            show_modal.set(true);
        })
    };

    let on_deploy_all = {
        let pending_deploy = pending_deploy.clone();
        Callback::from(move |_| pending_deploy.set(true))
    };

    let on_confirm_deploy_all = {
        let deploy_loading = deploy_loading.clone();
        let pending_deploy = pending_deploy.clone();
        let snackbar = snackbar.clone();
        let state = state.clone();
        Callback::from(move |_| {
            if *deploy_loading {
                return;
            }

            pending_deploy.set(false);
            let state_snapshot = (*state).clone();
            deploy_loading.set(true);
            let deploy_loading = deploy_loading.clone();
            let snackbar = snackbar.clone();

            spawn_local(async move {
                let loading_id = snackbar
                    .as_ref()
                    .map(|bus| bus.push("Deploying registry services..."));
                let summary = deploy_all_registries(&state_snapshot).await;

                if let (Some(bus), Some(id)) = (&snackbar, loading_id) {
                    bus.hide(id);
                }

                if let Some(bus) = &snackbar {
                    if summary.registries_total == 0 {
                        bus.push("No enabled registries to deploy.");
                    } else if !summary.failures.is_empty() {
                        bus.push(format!(
                            "Registry deploy finished with issues. {} of {} registries updated, {} configs pushed, {} inbounds skipped.",
                            summary.registries_succeeded,
                            summary.registries_total,
                            summary.services_deployed,
                            summary.skipped_inbounds
                        ));
                    } else {
                        bus.push(format!(
                            "Registry deploy complete. {} registries synced, {} configs pushed.",
                            summary.registries_succeeded, summary.services_deployed
                        ));
                    }
                }

                deploy_loading.set(false);
            });
        })
    };

    html! {
        <div class="p-6 space-y-6">
            <div class="flex justify-between" style="align-items: baseline;">
                <h1 class="text-3xl font-bold">{ "Registries" }</h1>
                <div class="flex items-center" style="gap: 0.5rem;">
                    <Button
                        label={if *deploy_loading { "Deploying..." } else { "Deploy All" }}
                        button_type={ButtonType::Outlined}
                        disabled={*deploy_loading}
                        onclick={on_deploy_all}
                    />
                    <Button
                        label="Add Registry"
                        icon={Some("icon-add".to_string())}
                        button_type={ButtonType::Filled}
                        onclick={move |_| on_open_modal.emit(())}
                    />
                </div>
            </div>

            {
                if state.registries.is_empty() {
                    html! {
                        <div class="md3-card p-12 text-center">
                            <p class="text-xl opacity-70">{ "No registries configured" }</p>
                            <p class="text-sm opacity-50 mt-2">{ "Add a registry to start tracking multiple endpoints" }</p>
                        </div>
                    }
                } else {
                    html! {
                        <RichTable columns={vec![
                            "Registry".to_string(),
                            "Public Endpoint".to_string(),
                            "Manage Endpoint".to_string(),
                            "Status".to_string(),
                            "Actions".to_string(),
                        ]}>
                            { for state.registries.iter().map(|registry| {
                                let registry_for_edit = registry.clone();
                                let registry_for_delete = registry.clone();
                                let registry_for_status = registry.clone();
                                html! {
                                    <>
                                        <div class="md3-list-row">
                                            <div class="md3-list-col md3-list-col-main">
                                                <div class="text-lg font-bold">{ &registry.name }</div>
                                            </div>
                                            <div class="md3-list-col md3-list-col-token">
                                                <div class="text-sm opacity-70 break-all">{ &registry.public_endpoint }</div>
                                            </div>
                                            <div class="md3-list-col md3-list-col-token">
                                                <div class="text-sm opacity-70 break-all">{ &registry.manage_endpoint }</div>
                                            </div>
                                            <div class="md3-list-col">
                                                <div class="text-sm opacity-70">
                                                    { if registry.enabled { "Enabled" } else { "Disabled" } }
                                                </div>
                                            </div>
                                            <div class="md3-list-col md3-list-col-actions">
                                                <div class="md3-list-actions">
                                                    <Button
                                                        label="Status"
                                                        button_type={ButtonType::Outlined}
                                                        onclick={{
                                                            let status_registry = status_registry.clone();
                                                            Callback::from(move |_| status_registry.set(Some(registry_for_status.clone())))
                                                        }}
                                                    />
                                                    <Button
                                                        label="Edit"
                                                        button_type={ButtonType::Outlined}
                                                        onclick={{
                                                            let show_modal = show_modal.clone();
                                                            let editing_registry = editing_registry.clone();
                                                            Callback::from(move |_| {
                                                                editing_registry.set(Some(registry_for_edit.clone()));
                                                                show_modal.set(true);
                                                            })
                                                        }}
                                                    />
                                                    <Button
                                                        label="Delete"
                                                        button_type={ButtonType::Outlined}
                                                        color={Some("#F2B8B5".to_string())}
                                                        onclick={{
                                                            let pending_delete = pending_delete.clone();
                                                            Callback::from(move |_| pending_delete.set(Some(registry_for_delete.clone())))
                                                        }}
                                                    />
                                                </div>
                                            </div>
                                        </div>
                                        <div class="md3-divider"></div>
                                    </>
                                }
                            }) }
                        </RichTable>
                    }
                }
            }

            {
                if *show_modal {
                    html! {
                        <RegistryModal
                            state={state.clone()}
                            initial_registry={(*editing_registry).clone()}
                            on_close={Callback::from({
                                let show_modal = show_modal.clone();
                                move |_| show_modal.set(false)
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if *pending_deploy {
                    let pending_deploy_cancel = pending_deploy.clone();
                    html! {
                        <DeleteConfirmPopup
                            title={"Deploy Registry Services"}
                            message={"Deploy all enabled node inbounds and accounts to every enabled registry?".to_string()}
                            confirm_label={"Deploy"}
                            on_cancel={Callback::from(move |_| pending_deploy_cancel.set(false))}
                            on_confirm={on_confirm_deploy_all.clone()}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some(registry) = &*pending_delete {
                    let state = state.clone();
                    let pending_delete_close = pending_delete.clone();
                    let pending_delete_confirm = pending_delete.clone();
                    let registry_id = registry.id.clone();
                    html! {
                        <DeleteConfirmPopup
                            title={"Delete Registry"}
                            message={format!("Delete registry \"{}\"?", registry.name)}
                            on_cancel={Callback::from(move |_| pending_delete_close.set(None))}
                            on_confirm={Callback::from(move |_| {
                                let mut new_state = (*state).clone();
                                new_state.registries.retain(|item| item.id != registry_id);
                                new_state.save();
                                state.set(new_state);
                                pending_delete_confirm.set(None);
                            })}
                        />
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some(registry) = &*status_registry {
                    html! {
                        <RegistryStatusPopup
                            registry={registry.clone()}
                            on_close={Callback::from({
                                let status_registry = status_registry.clone();
                                move |_| status_registry.set(None)
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

#[derive(Properties, PartialEq)]
struct RegistryStatusPopupProps {
    registry: RegistryInfo,
    on_close: Callback<()>,
}

#[function_component(RegistryStatusPopup)]
fn registry_status_popup(props: &RegistryStatusPopupProps) -> Html {
    let status = use_state(|| Option::<RegistryStatusResponse>::None);
    let loading = use_state(|| true);
    let error = use_state(|| Option::<String>::None);

    {
        let registry = props.registry.clone();
        let status = status.clone();
        let loading = loading.clone();
        let error = error.clone();
        use_effect_with(props.registry.id.clone(), move |_| {
            loading.set(true);
            status.set(None);
            error.set(None);

            spawn_local(async move {
                let api = RegistryApiService::new(
                    registry.manage_endpoint.clone(),
                    registry.master_key.clone(),
                );
                match api.status().await {
                    Ok(response) => status.set(Some(response)),
                    Err(err) => error.set(Some(err)),
                }
                loading.set(false);
            });

            || ()
        });
    }

    html! {
        <Popup
            title={format!("Registry Status: {}", props.registry.name)}
            size={PopupSize::Sm}
            on_close={props.on_close.clone()}
        >
            <div class="space-y-4">
                {
                    if *loading {
                        html! { <p class="text-sm opacity-80">{ "Loading status..." }</p> }
                    } else if let Some(error) = &*error {
                        html! { <p class="text-sm" style="color: #F2B8B5;">{ error.clone() }</p> }
                    } else if let Some(status) = &*status {
                        let updated_at = if status.updated_at_unix > 0 {
                            js_sys::Date::new(&(status.updated_at_unix as f64 * 1000.0).into())
                                .to_locale_string("en-US", &wasm_bindgen::JsValue::UNDEFINED)
                                .as_string()
                                .unwrap_or_else(|| status.updated_at_unix.to_string())
                        } else {
                            "Never".to_string()
                        };
                        html! {
                            <>
                                <div class="space-y-2">
                                    <div class="flex justify-between" style="gap: 1rem;">
                                        <span class="opacity-70">{ "Configured" }</span>
                                        <span>{ if status.configured { "Yes" } else { "No" } }</span>
                                    </div>
                                    <div class="flex justify-between" style="gap: 1rem;">
                                        <span class="opacity-70">{ "Accounts" }</span>
                                        <span>{ status.accounts }</span>
                                    </div>
                                    <div class="flex justify-between" style="gap: 1rem;">
                                        <span class="opacity-70">{ "Templates" }</span>
                                        <span>{ status.template_links }</span>
                                    </div>
                                    <div class="flex justify-between" style="gap: 1rem;">
                                        <span class="opacity-70">{ "Updated" }</span>
                                        <span class="text-right">{ updated_at }</span>
                                    </div>
                                </div>
                            </>
                        }
                    } else {
                        html! { <p class="text-sm opacity-80">{ "No status available." }</p> }
                    }
                }
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button
                        label="Close"
                        button_type={ButtonType::Text}
                        onclick={{
                            let on_close = props.on_close.clone();
                            Callback::from(move |_| on_close.emit(()))
                        }}
                    />
                </div>
            </div>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct RegistryModalProps {
    state: UseStateHandle<State>,
    #[prop_or_default]
    initial_registry: Option<RegistryInfo>,
    on_close: Callback<()>,
}

#[function_component(RegistryModal)]
fn registry_modal(props: &RegistryModalProps) -> Html {
    let name = use_state(String::new);
    let public_endpoint = use_state(String::new);
    let manage_endpoint = use_state(String::new);
    let master_key = use_state(String::new);
    let enabled = use_state(|| true);
    let name_error = use_state(|| Option::<String>::None);
    let public_endpoint_error = use_state(|| Option::<String>::None);
    let manage_endpoint_error = use_state(|| Option::<String>::None);
    let master_key_error = use_state(|| Option::<String>::None);

    {
        let name = name.clone();
        let public_endpoint = public_endpoint.clone();
        let manage_endpoint = manage_endpoint.clone();
        let master_key = master_key.clone();
        let enabled = enabled.clone();
        let name_error = name_error.clone();
        let public_endpoint_error = public_endpoint_error.clone();
        let manage_endpoint_error = manage_endpoint_error.clone();
        let master_key_error = master_key_error.clone();
        let initial_registry = props.initial_registry.clone();
        use_effect_with(initial_registry, move |initial_registry| {
            if let Some(registry) = initial_registry {
                name.set(registry.name.clone());
                public_endpoint.set(registry.public_endpoint.clone());
                manage_endpoint.set(registry.manage_endpoint.clone());
                master_key.set(registry.master_key.clone());
                enabled.set(registry.enabled);
            } else {
                name.set(String::new());
                public_endpoint.set(String::new());
                manage_endpoint.set(String::new());
                master_key.set(String::new());
                enabled.set(true);
            }
            name_error.set(None);
            public_endpoint_error.set(None);
            manage_endpoint_error.set(None);
            master_key_error.set(None);
            || ()
        });
    }

    let on_name_change = {
        let name = name.clone();
        let name_error = name_error.clone();
        Callback::from(move |value: String| {
            name.set(value);
            name_error.set(None);
        })
    };

    let on_public_endpoint_change = {
        let public_endpoint = public_endpoint.clone();
        let public_endpoint_error = public_endpoint_error.clone();
        Callback::from(move |value: String| {
            public_endpoint.set(value);
            public_endpoint_error.set(None);
        })
    };

    let on_manage_endpoint_change = {
        let manage_endpoint = manage_endpoint.clone();
        let manage_endpoint_error = manage_endpoint_error.clone();
        Callback::from(move |value: String| {
            manage_endpoint.set(value);
            manage_endpoint_error.set(None);
        })
    };

    let on_master_key_change = {
        let master_key = master_key.clone();
        let master_key_error = master_key_error.clone();
        Callback::from(move |value: String| {
            master_key.set(value);
            master_key_error.set(None);
        })
    };

    let on_enabled_change = {
        let enabled = enabled.clone();
        Callback::from(move |event: Event| {
            let checked = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
                .map(|input| input.checked())
                .unwrap_or(false);
            enabled.set(checked);
        })
    };

    let name_for_submit = name.clone();
    let public_endpoint_for_submit = public_endpoint.clone();
    let manage_endpoint_for_submit = manage_endpoint.clone();
    let master_key_for_submit = master_key.clone();
    let enabled_for_submit = enabled.clone();
    let name_error_for_submit = name_error.clone();
    let public_endpoint_error_for_submit = public_endpoint_error.clone();
    let manage_endpoint_error_for_submit = manage_endpoint_error.clone();
    let master_key_error_for_submit = master_key_error.clone();
    let initial_registry_for_submit = props.initial_registry.clone();
    let on_submit = {
        let state = props.state.clone();
        let on_close = props.on_close.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();

            name_error_for_submit.set(None);
            public_endpoint_error_for_submit.set(None);
            manage_endpoint_error_for_submit.set(None);
            master_key_error_for_submit.set(None);

            let name_value = name_for_submit.trim().to_string();
            let public_endpoint_value = public_endpoint_for_submit.trim().to_string();
            let manage_endpoint_value = manage_endpoint_for_submit.trim().to_string();
            let master_key_value = master_key_for_submit.trim().to_string();
            let mut has_error = false;

            if name_value.is_empty() {
                name_error_for_submit.set(Some("Registry name is required.".to_string()));
                has_error = true;
            }
            if public_endpoint_value.is_empty() {
                public_endpoint_error_for_submit
                    .set(Some("Public endpoint is required.".to_string()));
                has_error = true;
            }
            if manage_endpoint_value.is_empty() {
                manage_endpoint_error_for_submit
                    .set(Some("Manage endpoint is required.".to_string()));
                has_error = true;
            }
            if master_key_value.is_empty() {
                master_key_error_for_submit.set(Some("Master key is required.".to_string()));
                has_error = true;
            }

            if has_error {
                return;
            }

            let mut new_state = (*state).clone();
            if let Some(existing) = &initial_registry_for_submit {
                if let Some(registry) = new_state
                    .registries
                    .iter_mut()
                    .find(|item| item.id == existing.id)
                {
                    registry.name = name_value;
                    registry.public_endpoint = public_endpoint_value;
                    registry.manage_endpoint = manage_endpoint_value;
                    registry.master_key = master_key_value;
                    registry.enabled = *enabled_for_submit;
                }
            } else {
                new_state.registries.push(RegistryInfo {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: name_value,
                    public_endpoint: public_endpoint_value,
                    manage_endpoint: manage_endpoint_value,
                    master_key: master_key_value,
                    enabled: *enabled_for_submit,
                });
            }
            new_state.save();
            state.set(new_state);
            on_close.emit(());
        })
    };

    let is_edit = props.initial_registry.is_some();
    let on_close_click = props.on_close.clone();

    html! {
        <Popup
            title={if is_edit { "Edit Registry" } else { "Add Registry" }}
            size={PopupSize::Lg}
            on_close={props.on_close.clone()}
        >
            <form onsubmit={on_submit} class="space-y-4">
                <TextBox
                    label="Registry Name"
                    value={(*name).clone()}
                    onchange={on_name_change}
                    placeholder="Main Registry"
                    error={(*name_error).clone()}
                />
                <TextBox
                    label="Public Endpoint"
                    value={(*public_endpoint).clone()}
                    onchange={on_public_endpoint_change}
                    placeholder="http://127.0.0.1:9191"
                    error={(*public_endpoint_error).clone()}
                />
                <TextBox
                    label="Manage Endpoint"
                    value={(*manage_endpoint).clone()}
                    onchange={on_manage_endpoint_change}
                    placeholder="http://127.0.0.1:9291"
                    error={(*manage_endpoint_error).clone()}
                />
                <TextBox
                    label="Master Key"
                    value={(*master_key).clone()}
                    onchange={on_master_key_change}
                    placeholder="Enter registry master key"
                    error={(*master_key_error).clone()}
                />
                <div class="flex" style="align-items: center; gap: 0.75rem;">
                    <span>{ "Enabled" }</span>
                    <Switch checked={*enabled} onchange={on_enabled_change} />
                </div>

                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button
                        label="Cancel"
                        button_type={ButtonType::Text}
                        onclick={move |_| on_close_click.emit(())}
                    />
                    <Button
                        label={if is_edit { "Save Changes" } else { "Add Registry" }}
                        html_type={"submit"}
                        button_type={ButtonType::Filled}
                        onclick={Callback::from(move |_| {})}
                    />
                </div>
            </form>
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct DeleteConfirmPopupProps {
    title: AttrValue,
    message: String,
    #[prop_or(AttrValue::from("Delete"))]
    confirm_label: AttrValue,
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
                        label={props.confirm_label.to_string()}
                        button_type={ButtonType::Outlined}
                        color={Some("#F2B8B5".to_string())}
                        onclick={move |_| on_confirm_click.emit(())}
                    />
                </div>
            </div>
        </Popup>
    }
}
