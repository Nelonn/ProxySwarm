use yew::prelude::*;

/// RichTable — reusable table-like card used across Accounts, Nodes, Inbounds, Outbounds.
///
/// Props:
/// - columns: header labels for the table
/// - children: table rows (each child typically contains a divider + `.md3-list-row`)
/// - header_in_list: when true, header is rendered inside the `.md3-list` container (some pages expect this layout)
/// - card_class: optional extra class for the card (e.g., "bg-surface-container")
#[derive(Properties, PartialEq)]
pub struct RichTableProps {
    #[prop_or_default]
    pub columns: Vec<String>,

    #[prop_or_default]
    pub children: Children,

    #[prop_or_default]
    pub header_in_list: bool,

    #[prop_or_default]
    pub card_class: Option<String>,
}

#[function_component(RichTable)]
pub fn rich_table(props: &RichTableProps) -> Html {
    let card_classes = if let Some(class) = props.card_class.clone() {
        classes!("md3-card", class)
    } else {
        classes!("md3-card")
    };

    let column_count = props.columns.len().max(1);
    let table_style = format!("--rich-table-columns: repeat({}, minmax(0, 1fr));", column_count);

    let header = html! {
        <div class="md3-list-header">
            { for props.columns.iter().map(|column| html! {
                <div class="md3-list-col">{ column }</div>
            }) }
        </div>
    };

    if props.header_in_list {
        html! {
            <div class={card_classes} style={table_style}>
                <div class="md3-list">
                    { header }
                    { for props.children.iter() }
                </div>
            </div>
        }
    } else {
        html! {
            <div class={card_classes} style={table_style}>
                { header }
                <div class="md3-divider"></div>
                <div class="md3-list">
                    { for props.children.iter() }
                </div>
            </div>
        }
    }
}
