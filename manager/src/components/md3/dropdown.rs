use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlElement;
use yew::prelude::*;

use crate::components::FixedHeightText;

const DROPDOWN_STYLE_ID: &str = "md3-dropdown-component-styles";
const DROPDOWN_CSS: &str = r#"
/* MD3 Select/Dropdown */
.md3-select {
    width: 100%;
    background-color: var(--md-sys-color-surface-variant);
    border: 2px solid var(--md-sys-color-primary-outline);
    border-radius: 0.875rem;
    padding: 0.75rem 1rem;
    color: var(--md-sys-color-on-surface);
    font-size: 1rem;
    transition: border-color 0.2s ease, box-shadow 0.2s ease;
    cursor: pointer;
}
.md3-select:focus {
    outline: none;
    border-color: var(--md-sys-color-primary);
}
.md3-dropdown { position: relative; }

.md3-dropdown-trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    text-align: left;
}
.md3-dropdown-chevron {
    width: 1rem;
    height: 1rem;
    flex: 0 0 auto;
    transition: transform 0.18s ease;
}
.md3-dropdown-chevron-open {
    transform: rotate(180deg);
}
.md3-dropdown-backdrop {
    position: fixed;
    inset: 0;
    border: none;
    background: transparent;
    padding: 0;
    z-index: 39;
}
.md3-dropdown-menu {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(100% + 0.25rem);
    z-index: 40;
    background-color: var(--md-sys-color-surface);
    border: 2px solid var(--md-sys-color-primary-outline);
    border-radius: 0.75rem;
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.35);
    padding: 0.25rem;
    max-height: min(20rem, calc(100vh - 2rem));
    overflow-y: auto;
    animation: md3-popup-scale-in 140ms ease-out;
}
.md3-dropdown-option {
    width: 100%;
    display: flex;
    align-items: center;
    border: none;
    background: transparent;
    color: var(--md-sys-color-on-surface);
    border-radius: 0.5rem;
    padding: 0.75rem 0.8rem;
    text-align: left;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s ease, color 0.15s ease;
}
.md3-dropdown-option:hover {
    background-color: rgba(208, 188, 255, 0.08);
}
.md3-dropdown-option-selected {
    background-color: rgba(208, 188, 255, 0.12);
    color: var(--md-sys-color-primary);
}
"#;

fn ensure_dropdown_styles() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if document.get_element_by_id(DROPDOWN_STYLE_ID).is_some() {
        return;
    }
    let Ok(style_element) = document.create_element("style") else {
        return;
    };
    let _ = style_element.set_attribute("id", DROPDOWN_STYLE_ID);
    style_element.set_text_content(Some(DROPDOWN_CSS));

    if let Some(body) = document.body() {
        let _ = body.append_child(&style_element);
    }
}

#[derive(Clone, PartialEq)]
pub struct DropdownOption {
    pub value: String,
    pub label: String,
}

#[derive(Properties, PartialEq)]
pub struct DropdownProps {
    pub label: String,
    pub value: String,
    pub options: Vec<DropdownOption>,
    pub onchange: Callback<String>,
    #[prop_or(None)]
    pub placeholder: Option<String>,
    #[prop_or(false)]
    pub disabled: bool,
    #[prop_or(None)]
    pub error: Option<String>,
    #[prop_or(None)]
    pub style: Option<String>,
}

#[function_component(Dropdown)]
pub fn dropdown(props: &DropdownProps) -> Html {
    use_effect_with((), move |_| {
        ensure_dropdown_styles();
        || ()
    });

    let open = use_state(|| false);
    let menu_style = use_state(String::new);
    let trigger_ref = use_node_ref();

    let error_html = if let Some(error_msg) = &props.error {
        html! { <div class="text-sm mt-2" style="color: #F2B8B5;">{ error_msg }</div> }
    } else {
        html! {}
    };

    let selected_label = props
        .options
        .iter()
        .find(|opt| opt.value == props.value)
        .map(|opt| opt.label.clone())
        .or_else(|| props.placeholder.clone())
        .unwrap_or_default();

    let toggle = {
        let open = open.clone();
        let disabled = props.disabled;
        Callback::from(move |_| {
            if !disabled {
                open.set(!*open);
            }
        })
    };

    let close = {
        let open = open.clone();
        Callback::from(move |_: ()| open.set(false))
    };

    {
        let open = open.clone();
        let menu_style = menu_style.clone();
        let trigger_ref = trigger_ref.clone();
        use_effect_with(*open, move |is_open| {
            if *is_open {
                if let Some(trigger) = trigger_ref.cast::<HtmlElement>() {
                    if let Ok(rect) = js_sys::Reflect::apply(
                        &js_sys::Reflect::get(
                            &trigger,
                            &JsValue::from_str("getBoundingClientRect"),
                        )
                        .ok()
                        .and_then(|value| value.dyn_into::<js_sys::Function>().ok())
                        .unwrap(),
                        &trigger,
                        &js_sys::Array::new(),
                    ) {
                        let left = js_sys::Reflect::get(&rect, &JsValue::from_str("left"))
                            .ok()
                            .and_then(|value| value.as_f64())
                            .unwrap_or(0.0);
                        let bottom = js_sys::Reflect::get(&rect, &JsValue::from_str("bottom"))
                            .ok()
                            .and_then(|value| value.as_f64())
                            .unwrap_or(0.0);
                        let width = js_sys::Reflect::get(&rect, &JsValue::from_str("width"))
                            .ok()
                            .and_then(|value| value.as_f64())
                            .unwrap_or(0.0);
                        menu_style.set(format!(
                            "position: fixed; left: {}px; top: {}px; width: {}px;",
                            left,
                            bottom + 4.0,
                            width
                        ));
                    }
                }
            }
            || ()
        });
    }

    html! {
        <div class="w-full" style={props.style.clone()}>
            {
                if props.label.is_empty() {
                    html! {}
                } else {
                    html! {
                        <label class="block text-sm font-medium mb-1 text-on-surface">
                            { &props.label }
                        </label>
                    }
                }
            }
            <div class="md3-dropdown">
                <button
                    type="button"
                    class="md3-select md3-dropdown-trigger"
                    disabled={props.disabled}
                    onclick={toggle}
                    aria-expanded={open.to_string()}
                    ref={trigger_ref}
                >
                    <span class={classes!(if props.value.is_empty() { "opacity-70" } else { "" })}>
                        <FixedHeightText text={AttrValue::from(selected_label)} />
                    </span>
                    <svg class={classes!("md3-dropdown-chevron", if *open { "md3-dropdown-chevron-open" } else { "" })} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                        <path d="M7 10l5 5 5-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                </button>
                {
                    if *open {
                        html! {
                            <>
                                <button
                                    type="button"
                                    class="md3-dropdown-backdrop"
                                    onclick={Callback::from({
                                        let close = close.clone();
                                        move |_| close.emit(())
                                    })}
                                    aria-label="Close dropdown"
                                />
                                <div class="md3-dropdown-menu" style={(*menu_style).clone()}>
                                    {
                                        for props.options.iter().map(|opt| {
                                            let onchange = props.onchange.clone();
                                            let close = close.clone();
                                            let value = opt.value.clone();
                                            let selected = props.value == value;
                                            html! {
                                                <button
                                                    type="button"
                                                    class={classes!("md3-dropdown-option", if selected { "md3-dropdown-option-selected" } else { "" })}
                                                    onmousedown={Callback::from(move |_| {
                                                        onchange.emit(value.clone());
                                                        close.emit(());
                                                    })}
                                                >
                                                    <FixedHeightText text={AttrValue::from(opt.label.clone())} />
                                                </button>
                                            }
                                        })
                                    }
                                </div>
                            </>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
            { error_html }
        </div>
    }
}
