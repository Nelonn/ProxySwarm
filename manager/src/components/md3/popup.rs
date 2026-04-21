use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{window, KeyboardEvent};
use yew::create_portal;
use yew::prelude::*;

use crate::components::SvgIcon;

const POPUP_STYLE_ID: &str = "md3-popup-component-styles";
const POPUP_CSS: &str = r#"
.md3-popup-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    z-index: 1000;
    animation: md3-popup-fade-in 140ms ease-out;
}
.md3-popup-panel {
    width: 100%;
    max-height: 90vh;
    overflow: auto;
    background-color: var(--md-sys-color-surface);
    border-radius: 1.5rem;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
    transform-origin: center;
    animation: md3-popup-scale-in 160ms ease-out;
}
.md3-popup-panel-sm { max-width: 24rem; }
.md3-popup-panel-md { max-width: 42rem; }
.md3-popup-panel-lg { max-width: 56rem; }
.md3-popup-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1.25rem 1.25rem 0.75rem;
    gap: 1rem;
}
.md3-popup-body {
    padding: 0 1.25rem 1.25rem;
}
.md3-popup-close {
    appearance: none;
    border: none;
    background: transparent;
    color: var(--md-sys-color-on-surface);
    width: 2.25rem;
    height: 2.25rem;
    border-radius: 9999px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: background-color 0.15s;
}
.md3-popup-close:hover { background-color: var(--md-sys-color-surface-container); }
.md3-popup-actions {
    display: flex;
    gap: 1rem;
    padding-top: 1rem;
}
.md3-wizard-page {
    animation: md3-wizard-page-in 180ms ease-out;
    transform-origin: top center;
}
@media (max-width: 520px) {
    .md3-popup-actions { flex-direction: column; }
}
@keyframes md3-popup-fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
}
@keyframes md3-popup-scale-in {
    from { opacity: 0; transform: scale(0.98); }
    to { opacity: 1; transform: scale(1); }
}
@keyframes md3-wizard-page-in {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
}
@media (prefers-reduced-motion: reduce) {
    .md3-popup-overlay, .md3-popup-panel, .md3-wizard-page { animation: none; }
}
"#;

fn ensure_popup_styles() {
    let Some(window) = window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if document.get_element_by_id(POPUP_STYLE_ID).is_some() {
        return;
    }
    let Ok(style_element) = document.create_element("style") else {
        return;
    };
    let _ = style_element.set_attribute("id", POPUP_STYLE_ID);
    style_element.set_text_content(Some(POPUP_CSS));
    if let Some(body) = document.body() {
        let _ = body.append_child(&style_element);
    }
}

/// Reusable modal popup (backdrop + panel).
///
/// Typical usage:
/// - Render it conditionally when visible.
/// - Put your form/content inside as `children`.
/// - Use `on_close` for Escape/backdrop/close button.
///
/// ```ignore
/// html! {
///   { if *open {
///       html! { <Popup title={"Title".into()} on_close={on_close}>{"..."}</Popup> }
///   } else { html!{} } }
/// }
/// ```
#[derive(Clone, Copy, PartialEq)]
pub enum PopupSize {
    Sm,
    Md,
    Lg,
}

impl PopupSize {
    fn class(self) -> &'static str {
        match self {
            PopupSize::Sm => "md3-popup-panel-sm",
            PopupSize::Md => "md3-popup-panel-md",
            PopupSize::Lg => "md3-popup-panel-lg",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct PopupProps {
    pub title: AttrValue,
    pub on_close: Callback<()>,
    #[prop_or(PopupSize::Md)]
    pub size: PopupSize,
    #[prop_or(true)]
    pub close_on_backdrop: bool,
    #[prop_or(true)]
    pub close_on_escape: bool,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Popup)]
pub fn popup(props: &PopupProps) -> Html {
    use_effect_with((), move |_| {
        ensure_popup_styles();
        || ()
    });

    let on_close = props.on_close.clone();
    let close_on_escape = props.close_on_escape;

    // Popup is only mounted when it is visible, so this listener stays scoped to the lifetime
    // of the popup component.
    use_effect(move || -> Box<dyn FnOnce()> {
        if !close_on_escape {
            return Box::new(|| ());
        }

        let Some(window) = window() else {
            return Box::new(|| ());
        };

        let on_close = on_close.clone();
        let handler = Closure::<dyn FnMut(KeyboardEvent)>::new(move |e: KeyboardEvent| {
            if e.key() == "Escape" {
                on_close.emit(());
            }
        });

        // Best-effort: if listener registration fails, we still render the popup.
        let _ =
            window.add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());

        Box::new(move || {
            let _ = window
                .remove_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
            drop(handler);
        })
    });

    let backdrop_onclick = {
        let on_close = props.on_close.clone();
        let close_on_backdrop = props.close_on_backdrop;
        Callback::from(move |_| {
            if close_on_backdrop {
                on_close.emit(());
            }
        })
    };

    let panel_onclick = Callback::from(|e: MouseEvent| e.stop_propagation());

    let close_btn_onclick = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    let content = html! {
        <div class="md3-popup-overlay" onclick={backdrop_onclick}>
            <div
                class={classes!("md3-popup-panel", props.size.class())}
                role="dialog"
                aria-modal="true"
                onclick={panel_onclick}
            >
                <div class="md3-popup-header">
                    <h2 class="text-2xl font-bold">{ props.title.clone() }</h2>
                    <button class="md3-popup-close" onclick={close_btn_onclick} aria-label="Close popup">
                        <SvgIcon name="close_24dp" size={24} />
                    </button>
                </div>
                <div class="md3-popup-body">
                    { for props.children.iter() }
                </div>
            </div>
        </div>
    };

    if let Some(document) = window().and_then(|window| window.document()) {
        if let Some(body) = document.body() {
            return create_portal(content, body.into());
        }
    }

    content
}
