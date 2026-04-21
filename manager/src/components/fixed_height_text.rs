use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FixedHeightTextProps {
    pub text: AttrValue,
}

#[function_component(FixedHeightText)]
pub fn fixed_height_text(props: &FixedHeightTextProps) -> Html {
    html! {
        <span style="display:block; height:20px; line-height:20px; overflow:hidden;">{ props.text.clone() }</span>
    }
}
