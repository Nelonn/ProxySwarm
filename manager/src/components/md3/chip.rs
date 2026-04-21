use yew::prelude::*;

use crate::components::{FixedHeightText, SvgIcon};

const CHIP_STYLE_ID: &str = "md3-chip-component-styles";
const CHIP_CSS: &str = r#"
.md3-chip {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    white-space: nowrap;
    border-radius: 8px;

    height: 32px;
    padding-top: 6px;
    padding-bottom: 6px;

    font-size: 14px;
    font-weight: 500;
    line-height: 20px;

    user-select: none;
}

.md3-chip--filled {
    background-color: var(--md-sys-color-secondary-container);
    color: var(--md-sys-color-on-secondary-container);
    box-shadow: none;
}

.md3-chip--outlined {
    background-color: transparent;
    color: var(--md-sys-color-on-surface);
    box-shadow: inset 0 0 0 1px var(--md-sys-color-outline);
}

.md3-chip__content {
    display: inline-flex;
    align-items: center;
    min-height: 20px;
}

.md3-chip__icon {
    display: inline-flex;
    width: 18px;
    height: 18px;
    align-items: center;
    justify-content: center;
    line-height: 0;
}

.md3-chip__icon-btn {
    appearance: none;
    border: 0;
    background: transparent;
    padding: 0;
    margin: 0;
    color: inherit;
    cursor: pointer;
    display: inline-flex;
    width: 18px;
    height: 18px;
    align-items: center;
    justify-content: center;
    line-height: 0;
}

.md3-chip__icon-gap {
    width: 8px;
}
"#;

fn ensure_chip_styles() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if document.get_element_by_id(CHIP_STYLE_ID).is_some() {
        return;
    }
    let Ok(style_element) = document.create_element("style") else {
        return;
    };
    let _ = style_element.set_attribute("id", CHIP_STYLE_ID);
    style_element.set_text_content(Some(CHIP_CSS));
    if let Some(body) = document.body() {
        let _ = body.append_child(&style_element);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChipMode {
    Filled,
    Outlined,
}

#[derive(Properties, PartialEq)]
pub struct ChipProps {
    pub label: AttrValue,

    #[prop_or(ChipMode::Filled)]
    pub mode: ChipMode,

    #[prop_or_default]
    pub color: Option<String>,
    #[prop_or_default]
    pub text_color: Option<String>,
    #[prop_or_default]
    pub border_color: Option<String>,

    #[prop_or_default]
    pub leading_icon: Option<String>,
    #[prop_or_default]
    pub trailing_icon: Option<String>,
    #[prop_or_default]
    pub on_trailing_click: Option<Callback<()>>,
}

#[function_component(Chip)]
pub fn chip(props: &ChipProps) -> Html {
    ensure_chip_styles();

    let has_leading = props
        .leading_icon
        .as_ref()
        .is_some_and(|name| !name.trim().is_empty());
    let has_trailing = props
        .trailing_icon
        .as_ref()
        .is_some_and(|name| !name.trim().is_empty());
    let left_pad = if has_leading { 8 } else { 12 };
    let right_pad = if has_trailing { 8 } else { 12 };

    let mode_class = match props.mode {
        ChipMode::Filled => "md3-chip--filled",
        ChipMode::Outlined => "md3-chip--outlined",
    };

    let mut style = format!(
        "padding-left: {left}px; padding-right: {right}px;",
        left = left_pad,
        right = right_pad
    );
    if let Some(color) = props
        .color
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        style.push_str(&format!(" background-color: {color};", color = color));
    }
    if let Some(text_color) = props
        .text_color
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        style.push_str(&format!(" color: {color};", color = text_color));
    }
    if let Some(border_color) = props
        .border_color
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        style.push_str(&format!(" border-color: {color};", color = border_color));
    }

    let trailing_click = props.on_trailing_click.clone();

    html! {
        <span class={classes!("md3-chip", mode_class)} style={style}>
            <span class="md3-chip__content">
                {
                    if let Some(name) = props.leading_icon.clone().filter(|name| !name.trim().is_empty()) {
                        html! {
                            <>
                                <span class="md3-chip__icon">
                                    <SvgIcon name={AttrValue::from(name)} size={18} />
                                </span>
                                <span class="md3-chip__icon-gap" />
                            </>
                        }
                    } else {
                        html! {}
                    }
                }
                <FixedHeightText text={props.label.clone()} />
                {
                    if let Some(name) = props.trailing_icon.clone().filter(|name| !name.trim().is_empty()) {
                        html! {
                            <>
                                <span class="md3-chip__icon-gap" />
                                {
                                    if let Some(onclick) = trailing_click {
                                        html! {
                                            <button type="button" class="md3-chip__icon-btn" onclick={Callback::from(move |_| onclick.emit(()))}>
                                                <SvgIcon name={AttrValue::from(name)} size={18} />
                                            </button>
                                        }
                                    } else {
                                        html! {
                                            <span class="md3-chip__icon">
                                                <SvgIcon name={AttrValue::from(name)} size={18} />
                                            </span>
                                        }
                                    }
                                }
                            </>
                        }
                    } else {
                        html! {}
                    }
                }
            </span>
        </span>
    }
}
