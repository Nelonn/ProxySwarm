use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FixedHeightTextProps {
    pub text: AttrValue,
}

#[function_component(FixedHeightText)]
pub fn fixed_height_text(props: &FixedHeightTextProps) -> Html {
    html! {
        <span style="height:20px;">
            { props.text.clone() }
        </span>
    }
}
