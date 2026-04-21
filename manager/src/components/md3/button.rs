use yew::prelude::*;

use crate::components::{FixedHeightText, SvgIcon};

const BUTTON_STYLE_ID: &str = "md3-button-component-styles";
const BUTTON_CSS: &str = r#"
.md3-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    align-self: center;
    flex: 0 0 auto;
    padding: 10px 16px;
    border-radius: 100px;
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
    letter-spacing: 0.025em;
    cursor: pointer;
    border: none;
    transition: all 0.2s;
    text-decoration: none;
    white-space: nowrap;
}
.md3-btn:disabled {
    opacity: 0.38;
    cursor: not-allowed;
}
.md3-btn svg {
    width: 20px;
    height: 20px;
}
.md3-btn-filled {
    background-color: var(--md3-btn-fill-color, var(--md-sys-color-primary));
    color: var(--md3-btn-fill-text-color, var(--md-sys-color-on-primary));
}
.md3-btn-filled:hover:not(:disabled) {
    filter: brightness(1.1);
}
.md3-btn-tonal {
    background-color: var(--md3-btn-tonal-color, var(--md-sys-color-primary-container));
    color: var(--md3-btn-tonal-text-color, var(--md-sys-color-on-primary-container));
}
.md3-btn-tonal:hover:not(:disabled) {
    filter: brightness(1.08);
}
.md3-btn-outlined {
    background-color: transparent;
    color: var(--md3-btn-accent, var(--md-sys-color-primary));
    border: 2px solid var(--md3-btn-border-color, color-mix(in srgb, var(--md-sys-color-primary) 28%, var(--md-sys-color-surface) 72%));
}
.md3-btn-outlined:hover:not(:disabled) {
    background-color: var(--md3-btn-hover-color, rgba(208, 188, 255, 0.08));
}
.md3-btn-text {
    background-color: transparent;
    color: var(--md3-btn-accent, var(--md-sys-color-primary));
    padding: 10px 16px;
}
.md3-btn-text:hover:not(:disabled) {
    background-color: var(--md3-btn-hover-color, rgba(208, 188, 255, 0.08));
}
.md3-btn-xsmall {
    padding: 6px 12px;
}
.md3-btn-medium {
    padding: 16px 24px;
}
.md3-icon-btn {
    width: 2.75rem;
    aspect-ratio: 1 / 1;
    padding: 0;
    flex: 0 0 auto;
}
.md3-icon-btn svg {
    width: 1.35rem;
    height: 1.35rem;
    display: block;
}
.md3-icon-btn.md3-btn-xsmall {
    width: auto;
    aspect-ratio: auto;
    padding: 6px 12px;
}
.md3-icon-btn.md3-btn-xsmall svg {
    width: 20px;
    height: 20px;
}

/* MD3 Button groups (split buttons) */
.md3-btn-group {
    display: inline-flex;
    align-items: center;
    gap: 2px;
}
/* Button component injects its own border-radius; inner corners need an explicit override. */
.md3-btn-group > .md3-btn:first-child {
    border-radius: 20px 4px 4px 20px;
}
.md3-btn-group > .md3-btn:last-child {
    border-radius: 4px 20px 20px 4px;
}
.md3-btn-group > .md3-btn:only-child {
    border-radius: 20px;
}
/* Trailing icon button in group: custom padding, no aspect-ratio coupling. */
.md3-btn-group > .md3-icon-btn {
    width: auto;
    padding: 10px 14px 10px 10px;
    aspect-ratio: auto;
}
.md3-btn-group > .md3-icon-btn svg {
    width: 20px;
    height: 20px;
}

@keyframes md3-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(-360deg); }
}
.animate-spin {
    transform-origin: 50% 50%;
    animation: md3-spin 0.6s linear infinite;
}
"#;

fn ensure_button_styles() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if document.get_element_by_id(BUTTON_STYLE_ID).is_some() {
        return;
    }
    let Ok(style_element) = document.create_element("style") else {
        return;
    };
    let _ = style_element.set_attribute("id", BUTTON_STYLE_ID);
    style_element.set_text_content(Some(BUTTON_CSS));

    if let Some(body) = document.body() {
        let _ = body.append_child(&style_element);
    }
}

#[derive(Clone, PartialEq)]
pub enum ButtonType {
    Filled,
    Tonal,
    Outlined,
    Text,
}

#[derive(Clone, PartialEq)]
pub enum ButtonSize {
    Default,
    XSmall,
    Medium,
}

#[derive(Properties, PartialEq)]
pub struct ButtonProps {
    #[prop_or_default]
    pub label: String,
    #[prop_or(AttrValue::from("button"))]
    pub html_type: AttrValue,
    #[prop_or(ButtonType::Filled)]
    pub button_type: ButtonType,
    #[prop_or(false)]
    pub disabled: bool,
    #[prop_or(None)]
    pub icon: Option<String>,
    #[prop_or_default]
    pub icon_class: Classes,
    #[prop_or(false)]
    pub loading: bool,
    #[prop_or(None)]
    pub color: Option<String>,
    #[prop_or(ButtonSize::Default)]
    pub size: ButtonSize,
    pub onclick: Callback<MouseEvent>,
    #[prop_or_default]
    pub full_width: bool,
}

#[derive(Properties, PartialEq)]
pub struct IconButtonProps {
    #[prop_or_default]
    pub label: String,
    #[prop_or(AttrValue::from("button"))]
    pub html_type: AttrValue,
    #[prop_or(ButtonType::Filled)]
    pub button_type: ButtonType,
    #[prop_or(false)]
    pub disabled: bool,
    #[prop_or(None)]
    pub color: Option<String>,
    #[prop_or(ButtonSize::Default)]
    pub size: ButtonSize,
    pub onclick: Callback<MouseEvent>,
    #[prop_or_default]
    pub children: Children,
}

fn button_style(color: &Option<String>) -> Option<String> {
    color.as_ref().map(|color| {
        format!(
            "--md3-btn-accent: {0}; \
             --md3-btn-fill-color: {0}; \
             --md3-btn-fill-text-color: var(--md-sys-color-on-error, #ffffff); \
             --md3-btn-border-color: color-mix(in srgb, {0} 30%, var(--md-sys-color-surface) 70%); \
             --md3-btn-hover-color: color-mix(in srgb, {0} 10%, transparent); \
             --md3-btn-tonal-color: color-mix(in srgb, {0} 22%, var(--md-sys-color-surface-container) 78%);",
            color
        )
    })
}

fn button_class(button_type: &ButtonType) -> &'static str {
    match button_type {
        ButtonType::Filled => "md3-btn md3-btn-filled",
        ButtonType::Tonal => "md3-btn md3-btn-tonal",
        ButtonType::Outlined => "md3-btn md3-btn-outlined",
        ButtonType::Text => "md3-btn md3-btn-text",
    }
}

fn button_size_class(size: &ButtonSize) -> Option<&'static str> {
    match size {
        ButtonSize::Default => None,
        ButtonSize::XSmall => Some("md3-btn-xsmall"),
        ButtonSize::Medium => Some("md3-btn-medium"),
    }
}

#[function_component(Button)]
pub fn button(props: &ButtonProps) -> Html {
    use_effect_with((), move |_| {
        ensure_button_styles();
        || ()
    });

    let class = button_class(&props.button_type);

    let size_class = button_size_class(&props.size).unwrap_or("");
    let class = if props.full_width {
        format!("{} {} w-full", class, size_class)
    } else {
        format!("{} {}", class, size_class)
    };

    let style = button_style(&props.color);
    let icon_class = if props.loading {
        classes!(props.icon_class.clone(), "animate-spin")
    } else {
        props.icon_class.clone()
    };

    html! {
        <button
            class={class}
            type={props.html_type.clone()}
            style={style}
            disabled={props.disabled}
            aria-busy={props.loading.to_string()}
            onclick={props.onclick.clone()}
        >
            if let Some(icon) = &props.icon {
                <span class="mr-2" style="display: inline-flex; width: 20px; height: 20px; align-items: center; justify-content: center; line-height: 0;">
                    <SvgIcon name={AttrValue::from(icon.clone())} size={20} class={icon_class.clone()} />
                </span>
            }
            <FixedHeightText text={AttrValue::from(props.label.clone())} />
        </button>
    }
}

#[function_component(IconButton)]
pub fn icon_button(props: &IconButtonProps) -> Html {
    use_effect_with((), move |_| {
        ensure_button_styles();
        || ()
    });

    let size_class = button_size_class(&props.size).unwrap_or("");
    let class = format!(
        "{} md3-icon-btn {}",
        button_class(&props.button_type),
        size_class
    );
    let style = button_style(&props.color);
    let aria_label = if props.label.is_empty() {
        None
    } else {
        Some(props.label.clone())
    };

    html! {
        <button
            class={class}
            type={props.html_type.clone()}
            style={style}
            disabled={props.disabled}
            onclick={props.onclick.clone()}
            aria-label={aria_label}
        >
            { for props.children.iter() }
        </button>
    }
}
