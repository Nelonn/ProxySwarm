use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{
    window, Blob, Event, FileReader, HtmlAnchorElement, HtmlInputElement, ProgressEvent, Url,
};
use yew::prelude::*;

use crate::components::{Button, ButtonType, Popup, PopupSize, SnackbarBus};
use crate::state::State;

#[derive(serde::Serialize, serde::Deserialize)]
struct BackupFile {
    version: u32,
    exported_at: String,
    state: State,
}

#[function_component(SaveData)]
pub fn save_data() -> Html {
    let snackbar = use_context::<SnackbarBus>();
    let state = use_context::<UseStateHandle<State>>().expect("state context");
    let file_input_ref = use_node_ref();
    let clear_confirm_open = use_state(|| false);

    let on_export = {
        let state = state.clone();
        let snackbar = snackbar.clone();
        Callback::from(move |_| {
            let mut exported_state = (*state).clone().sanitized_for_storage();
            for node in &mut exported_state.nodes {
                if let Some(draft) = crate::storage::load_node_draft_local(&node.id) {
                    node.config = draft.sanitized_for_storage();
                }
            }

            let backup = BackupFile {
                version: 1,
                exported_at: js_sys::Date::new_0().to_iso_string().into(),
                state: exported_state,
            };

            let json = match serde_json::to_string_pretty(&backup) {
                Ok(value) => value,
                Err(_) => {
                    if let Some(bus) = &snackbar {
                        bus.push("Failed to serialize backup data.");
                    }
                    return;
                }
            };

            let blob_parts = js_sys::Array::new();
            blob_parts.push(&json.into());
            let blob = match Blob::new_with_str_sequence(&blob_parts) {
                Ok(blob) => blob,
                Err(_) => {
                    if let Some(bus) = &snackbar {
                        bus.push("Failed to prepare backup file.");
                    }
                    return;
                }
            };

            let url = match Url::create_object_url_with_blob(&blob) {
                Ok(url) => url,
                Err(_) => {
                    if let Some(bus) = &snackbar {
                        bus.push("Failed to create download link.");
                    }
                    return;
                }
            };

            let now: String = js_sys::Date::new_0().to_iso_string().into();
            let filename = format!(
                "proxyswarm-backup-{}.json",
                now.replace(':', "-").replace('.', "-")
            );

            let Some(window) = window() else {
                if let Some(bus) = &snackbar {
                    bus.push("Browser window is not available.");
                }
                let _ = Url::revoke_object_url(&url);
                return;
            };

            let Some(document) = window.document() else {
                if let Some(bus) = &snackbar {
                    bus.push("Browser document is not available.");
                }
                let _ = Url::revoke_object_url(&url);
                return;
            };

            let element = match document.create_element("a") {
                Ok(value) => value,
                Err(_) => {
                    if let Some(bus) = &snackbar {
                        bus.push("Failed to create download element.");
                    }
                    let _ = Url::revoke_object_url(&url);
                    return;
                }
            };

            let anchor = match element.dyn_into::<HtmlAnchorElement>() {
                Ok(anchor) => anchor,
                Err(_) => {
                    if let Some(bus) = &snackbar {
                        bus.push("Failed to build download link.");
                    }
                    let _ = Url::revoke_object_url(&url);
                    return;
                }
            };

            anchor.set_href(&url);
            anchor.set_download(&filename);
            anchor.click();
            let _ = Url::revoke_object_url(&url);

            if let Some(bus) = &snackbar {
                bus.push("Full backup exported.");
            }
        })
    };

    let on_import_click = {
        let file_input_ref = file_input_ref.clone();
        Callback::from(move |_| {
            if let Some(input) = file_input_ref.cast::<HtmlInputElement>() {
                input.set_value("");
                input.click();
            }
        })
    };

    let on_file_change = {
        let snackbar = snackbar.clone();
        Callback::from(move |event: Event| {
            let Some(input) = event.target_dyn_into::<HtmlInputElement>() else {
                return;
            };
            let Some(files) = input.files() else {
                return;
            };
            let Some(file) = files.get(0) else {
                return;
            };

            let reader = match FileReader::new() {
                Ok(reader) => reader,
                Err(_) => {
                    if let Some(bus) = &snackbar {
                        bus.push("Failed to initialize file reader.");
                    }
                    return;
                }
            };

            let snackbar_for_load = snackbar.clone();
            let reader_for_load = reader.clone();
            let onload = Closure::<dyn FnMut(ProgressEvent)>::new(move |_| {
                let text = reader_for_load
                    .result()
                    .ok()
                    .and_then(|value| value.as_string());

                let Some(content) = text else {
                    if let Some(bus) = &snackbar_for_load {
                        bus.push("Selected file is not readable text.");
                    }
                    return;
                };

                if let Ok(backup) = serde_json::from_str::<BackupFile>(&content) {
                    backup.state.save();
                    for node in backup.state.nodes {
                        crate::storage::save_node_draft(&node.id, &node.config);
                    }
                    if let Some(bus) = &snackbar_for_load {
                        bus.push("Backup imported. Reloading...");
                    }
                } else if let Ok(state) = serde_json::from_str::<State>(&content) {
                    state.save();
                    for node in &state.nodes {
                        crate::storage::save_node_draft(&node.id, &node.config);
                    }
                    if let Some(bus) = &snackbar_for_load {
                        bus.push("State backup imported. Reloading...");
                    }
                } else {
                    if let Some(bus) = &snackbar_for_load {
                        bus.push("Invalid backup format.");
                    }
                    return;
                };

                if let Some(window) = window() {
                    let _ = window.location().reload();
                }
            });

            reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            onload.forget();

            if reader.read_as_text(&file).is_err() {
                if let Some(bus) = &snackbar {
                    bus.push("Failed to read selected file.");
                }
            }
        })
    };

    let on_clear_click = {
        let clear_confirm_open = clear_confirm_open.clone();
        Callback::from(move |_| clear_confirm_open.set(true))
    };

    let on_clear_cancel = {
        let clear_confirm_open = clear_confirm_open.clone();
        Callback::from(move |_| clear_confirm_open.set(false))
    };

    let on_clear_confirm = {
        let state = state.clone();
        let snackbar = snackbar.clone();
        let clear_confirm_open = clear_confirm_open.clone();
        Callback::from(move |_| {
            let next_state = State::default();
            next_state.save();
            state.set(next_state);
            clear_confirm_open.set(false);
            if let Some(bus) = &snackbar {
                bus.push("All local data cleared.");
            }
        })
    };

    html! {
        <div class="p-6 space-y-6">
            <h1 class="text-3xl font-bold">{ "Save Data" }</h1>
            <div class="md3-card space-y-4 max-w-2xl">
                <p class="opacity-80">
                    { "Export or load a full JSON backup of all local data: nodes, accounts, and node revisions." }
                </p>
                <div class="space-x-2">
                    <Button
                        label="Export Full Backup"
                        button_type={ButtonType::Filled}
                        onclick={on_export}
                    />
                    <Button
                        label="Load Backup File"
                        button_type={ButtonType::Outlined}
                        onclick={on_import_click}
                    />
                    <Button
                        label="Clear All Data"
                        button_type={ButtonType::Text}
                        color={Some("#F2B8B5".to_string())}
                        onclick={on_clear_click}
                    />
                </div>
                <input
                    ref={file_input_ref}
                    type="file"
                    accept=".json,application/json"
                    style="display: none;"
                    onchange={on_file_change}
                />
            </div>
            {
                if *clear_confirm_open {
                    html! {
                        <Popup
                            title="Clear All Data"
                            size={PopupSize::Sm}
                            on_close={Callback::from({
                                let on_clear_cancel = on_clear_cancel.clone();
                                move |_| on_clear_cancel.emit(())
                            })}
                        >
                            <div class="space-y-4">
                                <p class="opacity-80">
                                    { "This will permanently remove all locally stored nodes, accounts, revisions, and imported data from this browser." }
                                </p>
                                <div class="md3-popup-actions" style="justify-content: flex-end;">
                                    <Button
                                        label="Cancel"
                                        button_type={ButtonType::Text}
                                        onclick={Callback::from({
                                            let on_clear_cancel = on_clear_cancel.clone();
                                            move |_| on_clear_cancel.emit(())
                                        })}
                                    />
                                    <Button
                                        label="Clear Data"
                                        button_type={ButtonType::Filled}
                                        color={Some("var(--md-sys-color-error)".to_string())}
                                        onclick={on_clear_confirm}
                                    />
                                </div>
                            </div>
                        </Popup>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
