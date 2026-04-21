use yew::prelude::*;

use crate::components::SvgIcon;

#[derive(Clone, PartialEq)]
pub struct WideNavigationBarItem {
    pub value: AttrValue,
    pub label: AttrValue,
    pub icon_name: AttrValue,
}

#[derive(Properties, PartialEq)]
pub struct WideNavigationBarProps {
    pub items: Vec<WideNavigationBarItem>,
    pub active_value: AttrValue,
    pub on_select: Callback<AttrValue>,
}

#[function_component(WideNavigationBar)]
pub fn wide_navigation_bar(props: &WideNavigationBarProps) -> Html {
    html! {
        <>
            {
                for props.items.iter().map(|item| {
                    let is_active = props.active_value == item.value;
                    let on_select = props.on_select.clone();
                    let value = item.value.clone();

                    html! {
                        <button
                            type="button"
                            class={classes!("md3-config-nav-item", if is_active { "md3-config-nav-item-active" } else { "" })}
                            onclick={Callback::from(move |_| on_select.emit(value.clone()))}
                        >
                            <span class="md3-config-nav-icon">
                                <SvgIcon name={item.icon_name.clone()} size={20} />
                            </span>
                            <span class="md3-config-nav-label">{ item.label.clone() }</span>
                        </button>
                    }
                })
            }
        </>
    }
}
