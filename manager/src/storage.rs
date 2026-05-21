use gloo_storage::{LocalStorage, Storage};
use js_sys::{Function, Promise, Reflect};
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::window;
use yew::UseStateHandle;

use crate::state::{NodeConfigDraft, State};

const STATE_KEY: &str = "proxyswarm_state";
const LOAD_APP_STATE_CMD: &str = "load_app_state";
const SAVE_APP_STATE_CMD: &str = "save_app_state";
const LOAD_NODE_DRAFT_CMD: &str = "load_node_draft";
const SAVE_NODE_DRAFT_CMD: &str = "save_node_draft";

pub fn load_state() -> State {
    load_local_storage(STATE_KEY).unwrap_or_default()
}

pub fn save_state(state: &State) {
    let sanitized = state.sanitized_for_storage();
    let _ = LocalStorage::set(STATE_KEY, &sanitized);
    persist_desktop_state(&sanitized);
}

pub fn hydrate_desktop_state(state: UseStateHandle<State>) {
    spawn_local(async move {
        let Some(serialized) = invoke_desktop(LOAD_APP_STATE_CMD, JsValue::NULL).await else {
            return;
        };

        let Some(contents) = serialized.as_string() else {
            return;
        };

        let Ok(desktop_state) = serde_json::from_str::<State>(&contents) else {
            return;
        };
        let desktop_state = desktop_state.normalized_on_load();

        let _ = LocalStorage::set(STATE_KEY, &desktop_state);
        if *state != desktop_state {
            state.set(desktop_state);
        }
    });
}

pub fn load_node_draft_local(node_id: &str) -> Option<NodeConfigDraft> {
    load_local_storage(&node_draft_storage_key(node_id))
}

pub fn hydrate_desktop_node_draft(node_id: String, draft: UseStateHandle<NodeConfigDraft>) {
    spawn_local(async move {
        let payload = object_with_pairs(&[("nodeId", JsValue::from_str(&node_id))]);
        let Some(serialized) = invoke_desktop(LOAD_NODE_DRAFT_CMD, payload).await else {
            return;
        };

        let Some(contents) = serialized.as_string() else {
            return;
        };

        let Ok(next_draft) = serde_json::from_str::<NodeConfigDraft>(&contents) else {
            return;
        };

        let _ = LocalStorage::set(node_draft_storage_key(&node_id), &next_draft);
        if *draft != next_draft {
            draft.set(next_draft);
        }
    });
}

pub fn save_node_draft(node_id: &str, draft: &NodeConfigDraft) {
    let sanitized = draft.sanitized_for_storage();
    let storage_key = node_draft_storage_key(node_id);
    let _ = LocalStorage::set(&storage_key, &sanitized);

    let Ok(contents) = serde_json::to_string_pretty(&sanitized) else {
        return;
    };

    let payload = object_with_pairs(&[
        ("nodeId", JsValue::from_str(node_id)),
        ("contents", JsValue::from_str(&contents)),
    ]);

    spawn_local(async move {
        let _ = invoke_desktop(SAVE_NODE_DRAFT_CMD, payload).await;
    });
}

pub fn node_draft_storage_key(node_id: &str) -> String {
    format!("proxyswarm_node_draft_{}", node_id)
}

fn persist_desktop_state(state: &State) {
    let Ok(contents) = serde_json::to_string_pretty(state) else {
        return;
    };

    let payload = object_with_pairs(&[("contents", JsValue::from_str(&contents))]);
    spawn_local(async move {
        let _ = invoke_desktop(SAVE_APP_STATE_CMD, payload).await;
    });
}

fn load_local_storage<T>(key: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    LocalStorage::get(key).ok()
}

async fn invoke_desktop(cmd: &str, payload: JsValue) -> Option<JsValue> {
    let invoke = desktop_invoke_function()?;
    let promise = invoke
        .call2(&JsValue::NULL, &JsValue::from_str(cmd), &payload)
        .ok()?;
    let promise: Promise = promise.dyn_into().ok()?;
    JsFuture::from(promise).await.ok()
}

fn desktop_invoke_function() -> Option<Function> {
    let window = window()?;
    let tauri = Reflect::get(window.as_ref(), &JsValue::from_str("__TAURI__")).ok();
    if let Some(tauri) = tauri {
        let core = Reflect::get(&tauri, &JsValue::from_str("core")).ok()?;
        if let Ok(invoke) = Reflect::get(&core, &JsValue::from_str("invoke")) {
            if invoke.is_function() {
                return invoke.dyn_into().ok();
            }
        }
    }

    let internals =
        Reflect::get(window.as_ref(), &JsValue::from_str("__TAURI_INTERNALS__")).ok()?;
    let invoke = Reflect::get(&internals, &JsValue::from_str("invoke")).ok()?;
    if invoke.is_function() {
        invoke.dyn_into().ok()
    } else {
        None
    }
}

fn object_with_pairs(pairs: &[(&str, JsValue)]) -> JsValue {
    let object = js_sys::Object::new();
    for (key, value) in pairs {
        let _ = Reflect::set(&object, &JsValue::from_str(key), value);
    }
    object.into()
}
