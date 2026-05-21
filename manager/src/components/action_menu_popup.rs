use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{window, Element, MouseEvent};
use yew::prelude::*;

use super::svg_icon::SvgIcon;

fn clamp_value(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

fn viewport_size() -> (f64, f64) {
    let Some(window) = window() else {
        return (0.0, 0.0);
    };
    let width = window
        .inner_width()
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let height = window
        .inner_height()
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    (width, height)
}

pub fn menu_anchor_from_mouse_event(event: &MouseEvent) -> Option<(f64, f64, f64)> {
    let target = event
        .target()
        .and_then(|value| value.dyn_into::<Element>().ok())
        .and_then(|element| element.closest("button").ok().flatten())
        .or_else(|| event.current_target()?.dyn_into::<Element>().ok())?;
    let rect_fn = js_sys::Reflect::get(&target, &"getBoundingClientRect".into()).ok()?;
    let rect_fn = rect_fn.dyn_into::<js_sys::Function>().ok()?;
    let rect = rect_fn.call0(&target).ok()?;
    let left = js_sys::Reflect::get(&rect, &"left".into()).ok()?.as_f64()?;
    let bottom = js_sys::Reflect::get(&rect, &"bottom".into()).ok()?.as_f64()?;
    let width = js_sys::Reflect::get(&rect, &"width".into()).ok()?.as_f64()?;
    Some((left, bottom + 4.0, width))
}

#[derive(Properties, PartialEq)]
pub struct ActionMenuPopupProps {
    pub anchor_left: f64,
    pub anchor_top: f64,
    pub anchor_width: f64,
    #[prop_or_default]
    pub on_edit: Option<Callback<()>>,
    #[prop_or_default]
    pub on_duplicate: Option<Callback<()>>,
    #[prop_or_default]
    pub on_delete: Option<Callback<()>>,
    pub on_close: Callback<()>,
}

fn menu_item(label: &str, icon: &str, onclick: Callback<()>) -> Html {
    html! {
        <button
            type="button"
            class="md3-action-menu-item"
            onclick={Callback::from(move |_| onclick.emit(()))}
        >
            <SvgIcon name={AttrValue::from(icon)} size={18} />
            <span>{ label }</span>
        </button>
    }
}

#[function_component(ActionMenuPopup)]
pub fn action_menu_popup(props: &ActionMenuPopupProps) -> Html {
    let menu_ref = use_node_ref();
    let menu_size = use_state(|| (0.0_f64, 0.0_f64));

    {
        let menu_ref = menu_ref.clone();
        let menu_size = menu_size.clone();
        use_effect(move || {
            if let Some(element) = menu_ref.cast::<Element>() {
                let rect_fn =
                    js_sys::Reflect::get(element.as_ref(), &JsValue::from_str("getBoundingClientRect"))
                        .ok()
                        .and_then(|value| value.dyn_into::<js_sys::Function>().ok());
                if let Some(rect_fn) = rect_fn {
                    if let Ok(rect) = rect_fn.call0(element.as_ref()) {
                        let width = js_sys::Reflect::get(&rect, &JsValue::from_str("width"))
                            .ok()
                            .and_then(|value| value.as_f64())
                            .unwrap_or(0.0);
                        let height = js_sys::Reflect::get(&rect, &JsValue::from_str("height"))
                            .ok()
                            .and_then(|value| value.as_f64())
                            .unwrap_or(0.0);
                        if width > 0.0 && height > 0.0 {
                            let next = (width, height);
                            if *menu_size != next {
                                menu_size.set(next);
                            }
                        }
                    }
                }
            }
            || ()
        });
    }

    let padding = 8.0;
    let (viewport_width, viewport_height) = viewport_size();
    let (menu_width, menu_height) = *menu_size;

    let bottom = props.anchor_top - padding;
    let mut top = props.anchor_top;
    if menu_height > 0.0 && viewport_height > 0.0 {
        if top + menu_height + padding > viewport_height {
            let above_top = bottom - menu_height - padding;
            if above_top >= padding {
                top = above_top;
            }
        }
        let max_top = if viewport_height > menu_height + padding {
            viewport_height - menu_height - padding
        } else {
            padding
        };
        top = clamp_value(top, padding, max_top);
    }

    let mut left = props.anchor_left;
    if menu_width > 0.0 && viewport_width > 0.0 {
        let max_left = if viewport_width > menu_width + padding {
            viewport_width - menu_width - padding
        } else {
            padding
        };
        left = clamp_value(left, padding, max_left);
    }

    let menu_style = format!(
        "position: fixed; left: {}px; top: {}px; min-width: 180px; max-width: 260px; width: max-content; z-index: 1400; padding: 8px;",
        left,
        top,
    );

    html! {
        <>
            <div
                style="position: fixed; inset: 0; z-index: 1399;"
                onclick={Callback::from({
                    let on_close = props.on_close.clone();
                    move |_| on_close.emit(())
                })}
            />
            <div ref={menu_ref} class="md3-card bg-surface-container md3-action-menu-popup" style={menu_style}>
                <div class="md3-action-menu">
                    {
                        if let Some(on_edit) = props.on_edit.clone() {
                            menu_item("Edit", "icon-edit", on_edit)
                        } else {
                            html! {}
                        }
                    }
                    {
                        if let Some(on_duplicate) = props.on_duplicate.clone() {
                            menu_item("Duplicate", "icon-duplicate", on_duplicate)
                        } else {
                            html! {}
                        }
                    }
                    {
                        if let Some(on_delete) = props.on_delete.clone() {
                            html! {
                                <button
                                    type="button"
                                    class="md3-action-menu-item md3-action-menu-item-danger"
                                    onclick={Callback::from(move |_| on_delete.emit(()))}
                                >
                                    <SvgIcon name={AttrValue::from("delete_24dp")} size={18} />
                                    <span>{ "Delete" }</span>
                                </button>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
        </>
    }
}
