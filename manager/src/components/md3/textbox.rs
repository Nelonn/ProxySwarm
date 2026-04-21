use yew::prelude::*;

use crate::components::SvgIcon;

#[derive(Properties, PartialEq)]
pub struct TextBoxProps {
    pub label: String,
    pub value: String,
    pub onchange: Callback<String>,
    #[prop_or(None)]
    pub placeholder: Option<String>,
    #[prop_or(false)]
    pub disabled: bool,
    #[prop_or(false)]
    pub is_textarea: bool,
    #[prop_or(None)]
    pub error: Option<String>,
    #[prop_or("text".to_string())]
    pub input_type: String,
    #[prop_or(None)]
    pub action_icon: Option<String>,
    #[prop_or(None)]
    pub action_onclick: Option<Callback<MouseEvent>>,
    #[prop_or(None)]
    pub action_label: Option<String>,
}

#[function_component(TextBox)]
pub fn text_box(props: &TextBoxProps) -> Html {
    let oninput = {
        let onchange = props.onchange.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target().unwrap();
            let value = js_sys::Reflect::get(&target, &"value".into())
                .unwrap()
                .as_string()
                .unwrap();
            onchange.emit(value);
        })
    };

    let label_id = format!("label-{}", props.label.replace(' ', "-").to_lowercase());
    let input_class = if props.error.is_some() {
        "md3-input md3-input-error"
    } else {
        "md3-input"
    };
    let input_class = if props.action_onclick.is_some() && !props.is_textarea {
        format!("{} md3-input-with-action", input_class)
    } else {
        input_class.to_string()
    };

    let error_html = if let Some(error_msg) = &props.error {
        html! { <div class="text-sm mt-2" style="color: var(--md-sys-color-error-soft);">{ error_msg }</div> }
    } else {
        html! {}
    };

    if props.is_textarea {
        html! {
            <div class="w-full">
                <label id={label_id.clone()} class="block text-sm font-medium mb-1 text-on-surface">
                    { &props.label }
                </label>
                <textarea
                    class={input_class}
                    placeholder={props.placeholder.clone()}
                    disabled={props.disabled}
                    oninput={oninput}
                    value={props.value.clone()}
                    rows={3}
                />
                { error_html }
            </div>
        }
    } else {
        html! {
            <div class="w-full">
                <label id={label_id.clone()} class="block text-sm font-medium mb-1 text-on-surface">
                    { &props.label }
                </label>
                <div class="md3-input-shell">
                    <input
                        type={props.input_type.clone()}
                        class={input_class}
                        placeholder={props.placeholder.clone()}
                        disabled={props.disabled}
                        oninput={oninput}
                        value={props.value.clone()}
                    />
                    {
                        if let (Some(icon), Some(action_onclick)) = (&props.action_icon, &props.action_onclick) {
                            html! {
                                <button
                                    type="button"
                                    class="md3-input-action"
                                    onclick={action_onclick.clone()}
                                    aria-label={props.action_label.clone().unwrap_or_else(|| "Input action".to_string())}
                                >
                                    <span class="md3-input-action-icon">
                                        <SvgIcon name={AttrValue::from(icon.clone())} size={20} />
                                    </span>
                                </button>
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
}
