use yew::prelude::*;

fn strip_svg_wrapper(svg: &str) -> &str {
    let Some(start) = svg.find('>') else {
        return svg;
    };
    let Some(end) = svg.rfind("</svg>") else {
        return svg;
    };
    svg[start + 1..end].trim()
}

fn extract_view_box(svg: &str) -> &str {
    let needle = r#"viewBox=""#;
    let Some(start) = svg.find(needle) else {
        return "0 0 24 24";
    };
    let value_start = start + needle.len();
    let Some(end_rel) = svg[value_start..].find('"') else {
        return "0 0 24 24";
    };
    &svg[value_start..value_start + end_rel]
}

fn symbol(id: &str, svg: &str) -> String {
    format!(
        r#"<symbol id="{id}" viewBox="{view_box}">{body}</symbol>"#,
        id = id,
        view_box = extract_view_box(svg),
        body = strip_svg_wrapper(svg),
    )
}

fn symbol_with_body(id: &str, view_box: &str, body: &str) -> String {
    format!(
        r#"<symbol id="{id}" viewBox="{view_box}">{body}</symbol>"#,
        id = id,
        view_box = view_box,
        body = body,
    )
}

fn build_sprite_defs() -> String {
    [
        symbol(
            "icon-add",
            include_str!("../../assets/icons/add_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "icon-arrow-downward",
            include_str!(
                "../../assets/icons/arrow_downward_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"
            ),
        ),
        symbol(
            "icon-arrow-upward",
            include_str!(
                "../../assets/icons/arrow_upward_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"
            ),
        ),
        symbol(
            "icon-assignment",
            include_str!(
                "../../assets/icons/assignment_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"
            ),
        ),
        symbol(
            "icon-call-made",
            include_str!("../../assets/icons/call_made_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "icon-call-received",
            include_str!(
                "../../assets/icons/call_received_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"
            ),
        ),
        symbol(
            "icon-call-split",
            include_str!(
                "../../assets/icons/call_split_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"
            ),
        ),
        symbol(
            "icon-close",
            include_str!("../../assets/icons/close_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "close_24dp",
            include_str!("../../assets/icons/close_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "icon-dashboard",
            include_str!("../../assets/icons/dashboard_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "icon-bar-chart-4",
            include_str!(
                "../../assets/icons/bar_chart_4_bars_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"
            ),
        ),
        symbol(
            "icon-edit",
            include_str!("../../assets/icons/edit_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "delete_24dp",
            include_str!("../../assets/icons/delete_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "icon-exit-to-app",
            include_str!(
                "../../assets/icons/exit_to_app_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"
            ),
        ),
        symbol(
            "icon-groups",
            include_str!("../../assets/icons/groups_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "icon-lock",
            include_str!("../../assets/icons/lock_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "icon-network-node",
            include_str!(
                "../../assets/icons/network_node_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"
            ),
        ),
        symbol(
            "icon-settings",
            include_str!("../../assets/icons/settings_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol(
            "icon-straight",
            include_str!("../../assets/icons/straight_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol_with_body(
            "icon-straight-inbound",
            "0 -960 960 960",
            r#"<g transform="rotate(180 480 -480)"><path d="m440-687-36 36q-11 11-27.5 11T348-652q-11-11-11-28t11-28l104-104q12-12 28-12t28 12l104 104q11 11 11.5 27.5T612-652q-11 11-28 11t-28-11l-36-35v527q0 17-11.5 28.5T480-120q-17 0-28.5-11.5T440-160v-527Z"/></g>"#,
        ),
        symbol(
            "icon-sync",
            include_str!("../../assets/icons/sync_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"),
        ),
        symbol_with_body(
            "icon-chevron-down",
            "0 0 24 24",
            r#"<path d="M7 10l5 5 5-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />"#,
        ),
    ]
    .join("")
}

#[derive(Properties, PartialEq)]
pub struct SvgSpriteDefsProps;

#[function_component(SvgSpriteDefs)]
pub fn svg_sprite_defs(_: &SvgSpriteDefsProps) -> Html {
    let defs = build_sprite_defs();
    html! {
        <svg
            aria-hidden="true"
            focusable="false"
            width="0"
            height="0"
            style="position:absolute;width:0;height:0;overflow:hidden;"
        >
            <defs>
                { Html::from_html_unchecked(AttrValue::from(defs)) }
            </defs>
        </svg>
    }
}

#[derive(Properties, PartialEq)]
pub struct SvgIconProps {
    pub name: AttrValue,
    #[prop_or(24)]
    pub size: u32,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(SvgIcon)]
pub fn svg_icon(props: &SvgIconProps) -> Html {
    let href = format!("#{}", props.name);
    let size = props.size.to_string();
    html! {
        <svg
            class={props.class.clone()}
            width={size.clone()}
            height={size}
            fill="currentColor"
            aria-hidden="true"
            focusable="false"
        >
            <use href={href} />
        </svg>
    }
}
