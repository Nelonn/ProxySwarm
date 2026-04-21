use js_sys::Reflect;
use std::net::IpAddr;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::use_navigator;

use crate::components::{Button, ButtonType, Popup, PopupSize, TextBox, RichTable};
use crate::state::{ProxyNode, State};
use crate::Route;

#[derive(Clone, Copy, PartialEq)]
struct CountryOption {
    code: &'static str,
    name: &'static str,
    aliases: &'static [&'static str],
}

const COUNTRY_OPTIONS: &[CountryOption] = &[
    CountryOption {
        code: "AM",
        name: "Armenia",
        aliases: &[],
    },
    CountryOption {
        code: "AU",
        name: "Australia",
        aliases: &[],
    },
    CountryOption {
        code: "AT",
        name: "Austria",
        aliases: &[],
    },
    CountryOption {
        code: "BE",
        name: "Belgium",
        aliases: &[],
    },
    CountryOption {
        code: "BR",
        name: "Brazil",
        aliases: &[],
    },
    CountryOption {
        code: "BG",
        name: "Bulgaria",
        aliases: &[],
    },
    CountryOption {
        code: "CA",
        name: "Canada",
        aliases: &[],
    },
    CountryOption {
        code: "CN",
        name: "China",
        aliases: &[],
    },
    CountryOption {
        code: "HR",
        name: "Croatia",
        aliases: &[],
    },
    CountryOption {
        code: "CY",
        name: "Cyprus",
        aliases: &[],
    },
    CountryOption {
        code: "CZ",
        name: "Czech Republic",
        aliases: &["Czechia"],
    },
    CountryOption {
        code: "DK",
        name: "Denmark",
        aliases: &[],
    },
    CountryOption {
        code: "EE",
        name: "Estonia",
        aliases: &[],
    },
    CountryOption {
        code: "FI",
        name: "Finland",
        aliases: &[],
    },
    CountryOption {
        code: "FR",
        name: "France",
        aliases: &[],
    },
    CountryOption {
        code: "GE",
        name: "Georgia",
        aliases: &[],
    },
    CountryOption {
        code: "DE",
        name: "Germany",
        aliases: &[],
    },
    CountryOption {
        code: "GR",
        name: "Greece",
        aliases: &[],
    },
    CountryOption {
        code: "HK",
        name: "Hong Kong",
        aliases: &[],
    },
    CountryOption {
        code: "HU",
        name: "Hungary",
        aliases: &[],
    },
    CountryOption {
        code: "IS",
        name: "Iceland",
        aliases: &[],
    },
    CountryOption {
        code: "IN",
        name: "India",
        aliases: &[],
    },
    CountryOption {
        code: "ID",
        name: "Indonesia",
        aliases: &[],
    },
    CountryOption {
        code: "IE",
        name: "Ireland",
        aliases: &[],
    },
    CountryOption {
        code: "IL",
        name: "Israel",
        aliases: &[],
    },
    CountryOption {
        code: "IT",
        name: "Italy",
        aliases: &[],
    },
    CountryOption {
        code: "JP",
        name: "Japan",
        aliases: &[],
    },
    CountryOption {
        code: "KZ",
        name: "Kazakhstan",
        aliases: &[],
    },
    CountryOption {
        code: "LV",
        name: "Latvia",
        aliases: &[],
    },
    CountryOption {
        code: "LT",
        name: "Lithuania",
        aliases: &[],
    },
    CountryOption {
        code: "LU",
        name: "Luxembourg",
        aliases: &[],
    },
    CountryOption {
        code: "MY",
        name: "Malaysia",
        aliases: &[],
    },
    CountryOption {
        code: "MD",
        name: "Moldova",
        aliases: &[],
    },
    CountryOption {
        code: "NL",
        name: "Netherlands",
        aliases: &["Holland"],
    },
    CountryOption {
        code: "NZ",
        name: "New Zealand",
        aliases: &[],
    },
    CountryOption {
        code: "NO",
        name: "Norway",
        aliases: &[],
    },
    CountryOption {
        code: "PL",
        name: "Poland",
        aliases: &[],
    },
    CountryOption {
        code: "PT",
        name: "Portugal",
        aliases: &[],
    },
    CountryOption {
        code: "RO",
        name: "Romania",
        aliases: &[],
    },
    CountryOption {
        code: "RU",
        name: "Russia",
        aliases: &[],
    },
    CountryOption {
        code: "RS",
        name: "Serbia",
        aliases: &[],
    },
    CountryOption {
        code: "SG",
        name: "Singapore",
        aliases: &[],
    },
    CountryOption {
        code: "SK",
        name: "Slovakia",
        aliases: &[],
    },
    CountryOption {
        code: "SI",
        name: "Slovenia",
        aliases: &[],
    },
    CountryOption {
        code: "KR",
        name: "South Korea",
        aliases: &["Korea", "Republic of Korea"],
    },
    CountryOption {
        code: "ES",
        name: "Spain",
        aliases: &[],
    },
    CountryOption {
        code: "SE",
        name: "Sweden",
        aliases: &[],
    },
    CountryOption {
        code: "CH",
        name: "Switzerland",
        aliases: &[],
    },
    CountryOption {
        code: "TH",
        name: "Thailand",
        aliases: &[],
    },
    CountryOption {
        code: "TR",
        name: "Turkey",
        aliases: &["Turkiye"],
    },
    CountryOption {
        code: "UA",
        name: "Ukraine",
        aliases: &[],
    },
    CountryOption {
        code: "AE",
        name: "United Arab Emirates",
        aliases: &["UAE"],
    },
    CountryOption {
        code: "GB",
        name: "United Kingdom",
        aliases: &["UK", "Britain", "Great Britain"],
    },
    CountryOption {
        code: "US",
        name: "United States",
        aliases: &["USA", "United States of America", "America"],
    },
    CountryOption {
        code: "VN",
        name: "Vietnam",
        aliases: &[],
    },
];

fn normalize_country_code(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

fn flag_emoji(country: &str) -> String {
    let normalized = normalize_country_code(country);
    if normalized.len() != 2 {
        return String::new();
    }

    normalized
        .chars()
        .map(|ch| char::from_u32(0x1F1E6 + (ch as u32 - 'A' as u32)).unwrap_or(ch))
        .collect()
}

fn find_country_by_query(query: &str) -> Option<CountryOption> {
    let normalized = query.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }

    COUNTRY_OPTIONS.iter().copied().find(|country| {
        country.code.eq_ignore_ascii_case(query.trim())
            || country.name.eq_ignore_ascii_case(query.trim())
            || country
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(query.trim()))
    })
}

fn search_countries(query: &str) -> Vec<CountryOption> {
    let normalized = query.trim().to_lowercase();
    let mut results: Vec<CountryOption> = COUNTRY_OPTIONS
        .iter()
        .copied()
        .filter(|country| {
            if normalized.is_empty() {
                return true;
            }

            let code = country.code.to_lowercase();
            let name = country.name.to_lowercase();
            code.starts_with(&normalized)
                || name.starts_with(&normalized)
                || name.contains(&normalized)
                || country.aliases.iter().any(|alias| {
                    let alias = alias.to_lowercase();
                    alias.starts_with(&normalized) || alias.contains(&normalized)
                })
        })
        .collect();
    results.truncate(8);
    results
}

fn country_display(country: CountryOption) -> String {
    let flag = flag_emoji(country.code);
    format!("{} {} ({})", flag, country.name, country.code)
}

fn format_bandwidth_mbps(value: Option<u32>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

fn format_max_traffic_gb(value: Option<u64>) -> String {
    value
        .map(|bytes| format!("{:.2}", bytes as f64 / (1024.0 * 1024.0 * 1024.0)))
        .unwrap_or_default()
}

#[function_component(Nodes)]
pub fn nodes() -> Html {
    let state = use_context::<UseStateHandle<State>>().expect("state context");
    let show_modal = use_state(|| false);
    let editing_node = use_state(|| Option::<ProxyNode>::None);
    let pending_delete = use_state(|| Option::<ProxyNode>::None);
    let navigator = use_navigator();

    let on_open_modal = {
        let show_modal = show_modal.clone();
        let editing_node = editing_node.clone();
        Callback::from(move |_| {
            editing_node.set(None);
            show_modal.set(true);
        })
    };

    html! {
        <div class="p-6 space-y-6">
            <div class="flex justify-between" style="align-items: baseline;">
                <h1 class="text-3xl font-bold">{ "Nodes" }</h1>
                <Button
                    label="Add Node"
                    icon={Some("icon-add".to_string())}
                    button_type={ButtonType::Filled}
                    onclick={move |_| on_open_modal.emit(())}
                />
            </div>

            { if state.nodes.is_empty() {
                html! {
                    <div class="md3-card p-12 text-center">
                        <p class="text-xl opacity-70">{ "No nodes configured" }</p>
                        <p class="text-sm opacity-50 mt-2">{ "Add a node to start managing your cluster" }</p>
                    </div>
                }
            } else {
                html! {
                    <RichTable columns={vec![
                        "Node".to_string(),
                        "Access Address".to_string(),
                        "Public IP".to_string(),
                        "Country".to_string(),
                        "Actions".to_string(),
                    ]}>
                        { for state.nodes.iter().map(|node| {
                            let flag = flag_emoji(&node.country);
                            let node_for_edit = node.clone();
                            let node_for_delete = node.clone();
                            let node_id = node.id.clone();
                            html! {
                                <>
                                    <div class="md3-list-row">
                                        <div class="md3-list-col md3-list-col-main">
                                            <div class="text-lg font-bold">{ &node.name }</div>
                                        </div>
                                        <div class="md3-list-col md3-list-col-access-id">
                                            <div class="text-sm opacity-70 break-all">{ &node.address }</div>
                                        </div>
                                        <div class="md3-list-col md3-list-col-access-id">
                                            <div class="text-sm opacity-70 break-all">
                                                {
                                                    if node.public_ip.trim().is_empty() {
                                                        "-".to_string()
                                                    } else {
                                                        node.public_ip.clone()
                                                    }
                                                }
                                            </div>
                                        </div>
                                        <div class="md3-list-col">
                                            {
                                                if flag.is_empty() {
                                                    html! { <div class="text-sm opacity-50">{ "Not set" }</div> }
                                                } else {
                                                    html! {
                                                        <div class="flex items-center gap-2" style="line-height: 1; column-gap: 0.625rem;">
                                                            <span class="text-2xl leading-none">{ flag }</span>
                                                            <span class="text-sm opacity-70" style="display: inline-flex; align-items: center; line-height: 1;">{ normalize_country_code(&node.country) }</span>
                                                        </div>
                                                    }
                                                }
                                            }
                                        </div>
                                        <div class="md3-list-col md3-list-col-actions">
                                            <div class="md3-list-actions">
                                                <Button
                                                    label="Manage"
                                                    button_type={ButtonType::Tonal}
                                                    onclick={{
                                                        let navigator = navigator.clone();
                                                        Callback::from(move |_| {
                                                            if let Some(navigator) = navigator.clone() {
                                                                navigator.push(&Route::NodeConfig { id: node_id.clone() });
                                                            }
                                                        })
                                                    }}
                                                />
                                                <Button
                                                    label="Edit"
                                                    button_type={ButtonType::Outlined}
                                                    onclick={{
                                                        let show_modal = show_modal.clone();
                                                        let editing_node = editing_node.clone();
                                                        Callback::from(move |_| {
                                                            editing_node.set(Some(node_for_edit.clone()));
                                                            show_modal.set(true);
                                                        })
                                                    }}
                                                />
                                                <Button
                                                    label="Delete"
                                                    button_type={ButtonType::Outlined}
                                                    color={Some("#F2B8B5".to_string())}
                                                    onclick={{
                                                        let pending_delete = pending_delete.clone();
                                                        Callback::from(move |_| pending_delete.set(Some(node_for_delete.clone())))
                                                    }}
                                                />
                                            </div>
                                        </div>
                                    </div>
                                    <div class="md3-divider"></div>
                                </>
                            }
                        }) }
                    </RichTable>
                }
            } }

            { if *show_modal {
                html! {
                    <NodeModal
                        state={state.clone()}
                        initial_node={(*editing_node).clone()}
                        on_close={Callback::from(move |_| show_modal.set(false))}
                    />
                }
            } else {
                html! {}
            }}

            { if let Some(node) = &*pending_delete {
                let state = state.clone();
                let pending_delete_close = pending_delete.clone();
                let pending_delete_confirm = pending_delete.clone();
                let node_id = node.id.clone();
                html! {
                    <DeleteConfirmPopup
                        title={"Delete Node"}
                        message={format!("Delete node \"{}\"?", node.name)}
                        on_cancel={Callback::from(move |_| pending_delete_close.set(None))}
                        on_confirm={Callback::from(move |_| {
                            let mut new_state = (*state).clone();
                            new_state.nodes.retain(|n| n.id != node_id);
                            new_state.save();
                            state.set(new_state);
                            pending_delete_confirm.set(None);
                        })}
                    />
                }
            } else {
                html! {}
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct NodeModalProps {
    state: UseStateHandle<State>,
    #[prop_or_default]
    initial_node: Option<ProxyNode>,
    on_close: Callback<()>,
}

#[function_component(NodeModal)]
fn node_modal(props: &NodeModalProps) -> Html {
    let name = use_state(String::new);
    let address = use_state(String::new);
    let public_ip = use_state(String::new);
    let master_key = use_state(String::new);
    let bandwidth_mbps = use_state(String::new);
    let max_traffic_gb = use_state(String::new);
    let name_error = use_state(|| Option::<String>::None);
    let public_ip_error = use_state(|| Option::<String>::None);
    let master_key_error = use_state(|| Option::<String>::None);
    let bandwidth_error = use_state(|| Option::<String>::None);
    let max_traffic_error = use_state(|| Option::<String>::None);
    let country_error = use_state(|| Option::<String>::None);
    let country = use_state(String::new);
    let country_query = use_state(String::new);
    let show_country_results = use_state(|| false);
    let country_input_ref = use_node_ref();
    let country_menu_rect = use_state(|| (0.0_f64, 0.0_f64, 0.0_f64));

    {
        let name = name.clone();
        let address = address.clone();
        let public_ip = public_ip.clone();
        let master_key = master_key.clone();
        let bandwidth_mbps = bandwidth_mbps.clone();
        let max_traffic_gb = max_traffic_gb.clone();
        let name_error = name_error.clone();
        let public_ip_error = public_ip_error.clone();
        let master_key_error = master_key_error.clone();
        let bandwidth_error = bandwidth_error.clone();
        let max_traffic_error = max_traffic_error.clone();
        let country_error = country_error.clone();
        let country = country.clone();
        let country_query = country_query.clone();
        let initial_node = props.initial_node.clone();

        use_effect_with(initial_node, move |initial_node| {
            if let Some(node) = initial_node {
                name.set(node.name.clone());
                address.set(node.address.clone());
                public_ip.set(node.public_ip.clone());
                master_key.set(node.master_key.clone());
                bandwidth_mbps.set(format_bandwidth_mbps(node.bandwidth_mbps));
                max_traffic_gb.set(format_max_traffic_gb(node.max_traffic_bytes));
                name_error.set(None);
                public_ip_error.set(None);
                master_key_error.set(None);
                bandwidth_error.set(None);
                max_traffic_error.set(None);
                country_error.set(None);
                country.set(node.country.clone());
                country_query.set(
                    find_country_by_query(&node.country)
                        .map(country_display)
                        .unwrap_or_else(|| node.country.clone()),
                );
            } else {
                name.set(String::new());
                address.set(String::new());
                public_ip.set(String::new());
                master_key.set(String::new());
                bandwidth_mbps.set(String::new());
                max_traffic_gb.set(String::new());
                name_error.set(None);
                public_ip_error.set(None);
                master_key_error.set(None);
                bandwidth_error.set(None);
                max_traffic_error.set(None);
                country_error.set(None);
                country.set(String::new());
                country_query.set(String::new());
            }
            || ()
        });
    }

    let update_country_menu_rect = {
        let country_input_ref = country_input_ref.clone();
        let country_menu_rect = country_menu_rect.clone();
        Callback::from(move |_| {
            if let Some(input) = country_input_ref.cast::<HtmlInputElement>() {
                if let Ok(rect_fn) = Reflect::get(input.as_ref(), &"getBoundingClientRect".into()) {
                    if let Some(rect_fn) = rect_fn.dyn_ref::<js_sys::Function>() {
                        if let Ok(rect) = rect_fn.call0(input.as_ref()) {
                            let left = Reflect::get(&rect, &"left".into())
                                .ok()
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let bottom = Reflect::get(&rect, &"bottom".into())
                                .ok()
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let width = Reflect::get(&rect, &"width".into())
                                .ok()
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            country_menu_rect.set((left, bottom + 8.0, width));
                        }
                    }
                }
            }
        })
    };

    let on_name_change = {
        let name = name.clone();
        let name_error = name_error.clone();
        Callback::from(move |value: String| {
            name.set(value);
            name_error.set(None);
        })
    };

    let on_address_change = {
        let address = address.clone();
        Callback::from(move |value: String| address.set(value))
    };

    let on_master_key_change = {
        let master_key = master_key.clone();
        let master_key_error = master_key_error.clone();
        Callback::from(move |value: String| {
            master_key.set(value);
            master_key_error.set(None);
        })
    };

    let on_bandwidth_change = {
        let bandwidth_mbps = bandwidth_mbps.clone();
        let bandwidth_error = bandwidth_error.clone();
        Callback::from(move |value: String| {
            bandwidth_mbps.set(value);
            bandwidth_error.set(None);
        })
    };

    let on_max_traffic_change = {
        let max_traffic_gb = max_traffic_gb.clone();
        let max_traffic_error = max_traffic_error.clone();
        Callback::from(move |value: String| {
            max_traffic_gb.set(value);
            max_traffic_error.set(None);
        })
    };

    let on_public_ip_change = {
        let public_ip = public_ip.clone();
        let public_ip_error = public_ip_error.clone();
        Callback::from(move |value: String| {
            public_ip.set(value);
            public_ip_error.set(None);
        })
    };

    let on_country_change = {
        let country = country.clone();
        let country_query = country_query.clone();
        let country_error = country_error.clone();
        let show_country_results = show_country_results.clone();
        let update_country_menu_rect = update_country_menu_rect.clone();
        Callback::from(move |e: InputEvent| {
            let input = e.target_unchecked_into::<HtmlInputElement>();
            let value = input.value();
            country_query.set(value.clone());
            country.set(
                find_country_by_query(&value)
                    .map(|selected| selected.code.to_string())
                    .unwrap_or_default(),
            );
            country_error.set(None);
            show_country_results.set(true);
            update_country_menu_rect.emit(());
        })
    };

    let on_country_focus = {
        let show_country_results = show_country_results.clone();
        let update_country_menu_rect = update_country_menu_rect.clone();
        Callback::from(move |_| {
            show_country_results.set(true);
            update_country_menu_rect.emit(());
        })
    };

    let on_country_blur = {
        let show_country_results = show_country_results.clone();
        Callback::from(move |_| show_country_results.set(false))
    };

    let name_for_submit = name.clone();
    let address_for_submit = address.clone();
    let public_ip_for_submit = public_ip.clone();
    let master_key_for_submit = master_key.clone();
    let bandwidth_mbps_for_submit = bandwidth_mbps.clone();
    let max_traffic_gb_for_submit = max_traffic_gb.clone();
    let name_error_for_submit = name_error.clone();
    let public_ip_error_for_submit = public_ip_error.clone();
    let master_key_error_for_submit = master_key_error.clone();
    let bandwidth_error_for_submit = bandwidth_error.clone();
    let max_traffic_error_for_submit = max_traffic_error.clone();
    let country_error_for_submit = country_error.clone();
    let country_for_submit = country.clone();
    let country_query_for_submit = country_query.clone();
    let initial_node_for_submit = props.initial_node.clone();
    let on_submit = {
        let state = props.state.clone();
        let on_close = props.on_close.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            name_error_for_submit.set(None);
            public_ip_error_for_submit.set(None);
            master_key_error_for_submit.set(None);
            bandwidth_error_for_submit.set(None);
            max_traffic_error_for_submit.set(None);
            country_error_for_submit.set(None);

            let mut has_error = false;
            if name_for_submit.trim().is_empty() {
                name_error_for_submit.set(Some("Node name is required.".to_string()));
                has_error = true;
            }
            if master_key_for_submit.trim().is_empty() {
                master_key_error_for_submit.set(Some("Master key is required.".to_string()));
                has_error = true;
            }

            let public_ip_value = public_ip_for_submit.trim().to_string();
            if !public_ip_value.is_empty() && public_ip_value.parse::<IpAddr>().is_err() {
                public_ip_error_for_submit.set(Some(
                    "Public IP must be valid IPv4 or IPv6 address.".to_string(),
                ));
                has_error = true;
            }

            let bandwidth_value = if bandwidth_mbps_for_submit.trim().is_empty() {
                None
            } else {
                match bandwidth_mbps_for_submit.trim().parse::<u32>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        bandwidth_error_for_submit.set(Some(
                            "Bandwidth must be a whole number in Mbps.".to_string(),
                        ));
                        has_error = true;
                        None
                    }
                }
            };

            let max_traffic_value = if max_traffic_gb_for_submit.trim().is_empty() {
                None
            } else {
                match max_traffic_gb_for_submit.trim().parse::<f64>() {
                    Ok(value) if value >= 0.0 => Some((value * 1024.0 * 1024.0 * 1024.0) as u64),
                    _ => {
                        max_traffic_error_for_submit.set(Some(
                            "Max traffic must be a non-negative number in GB.".to_string(),
                        ));
                        has_error = true;
                        None
                    }
                }
            };

            let selected_country = if country_for_submit.is_empty() {
                find_country_by_query(&country_query_for_submit)
                    .map(|country| country.code.to_string())
                    .unwrap_or_default()
            } else {
                (*country_for_submit).clone()
            };
            if selected_country.trim().is_empty() {
                country_error_for_submit.set(Some("Country is required.".to_string()));
                has_error = true;
            }
            if has_error {
                return;
            }

            let mut new_state = (*state).clone();
            if let Some(existing) = &initial_node_for_submit {
                if let Some(node) = new_state
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == existing.id)
                {
                    node.name = name_for_submit.trim().to_string();
                    node.address = address_for_submit.trim().to_string();
                    node.public_ip = public_ip_value.clone();
                    node.master_key = (*master_key_for_submit).clone();
                    node.country = selected_country;
                    node.bandwidth_mbps = bandwidth_value;
                    node.max_traffic_bytes = max_traffic_value;
                }
            } else {
                new_state.nodes.push(ProxyNode {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: name_for_submit.trim().to_string(),
                    address: address_for_submit.trim().to_string(),
                    public_ip: public_ip_value,
                    master_key: (*master_key_for_submit).clone(),
                    country: selected_country,
                    revisions: Vec::new(),
                    active_revision_id: String::new(),
                    config: crate::state::NodeConfigDraft {
                        master_key: (*master_key_for_submit).clone(),
                        ..Default::default()
                    },
                    bandwidth_mbps: bandwidth_value,
                    max_traffic_bytes: max_traffic_value,
                });
            }
            new_state.save();
            state.set(new_state);
            on_close.emit(());
        })
    };

    let on_close_click = props.on_close.clone();
    let country_results = search_countries(&country_query);
    let is_edit = props.initial_node.is_some();

    html! {
        <Popup
            title={if is_edit { "Edit Node" } else { "Add Node" }}
            size={PopupSize::Lg}
            on_close={props.on_close.clone()}
        >
            <form onsubmit={on_submit} class="space-y-4">
                <TextBox
                    label="Node Name"
                    value={(*name).clone()}
                    onchange={on_name_change}
                    placeholder="My Server"
                    error={(*name_error).clone()}
                />
                <TextBox
                    label="Access Address"
                    value={(*address).clone()}
                    onchange={on_address_change}
                    placeholder="http://1.2.3.4:9090"
                />
                <TextBox
                    label="Public IP Address"
                    value={(*public_ip).clone()}
                    onchange={on_public_ip_change}
                    placeholder="1.2.3.4 or 2001:db8::1"
                    error={(*public_ip_error).clone()}
                />
                <TextBox
                    label="Master Key"
                    value={(*master_key).clone()}
                    onchange={on_master_key_change}
                    placeholder="Enter master key"
                    input_type="password"
                    error={(*master_key_error).clone()}
                />
                <TextBox
                    label="Server Bandwidth (Mbps)"
                    value={(*bandwidth_mbps).clone()}
                    onchange={on_bandwidth_change}
                    placeholder="Optional"
                    input_type="number"
                    error={(*bandwidth_error).clone()}
                />
                <TextBox
                    label="Max Traffic (GB)"
                    value={(*max_traffic_gb).clone()}
                    onchange={on_max_traffic_change}
                    placeholder="Optional"
                    input_type="number"
                    error={(*max_traffic_error).clone()}
                />
                <div class="w-full">
                    <label class="block text-sm font-medium mb-1 text-on-surface">
                        { "Country" }
                    </label>
                    <input
                        ref={country_input_ref}
                        class={classes!("md3-input", (!(*country_error).is_none()).then_some("md3-input-error"))}
                        type="text"
                        value={(*country_query).clone()}
                        oninput={on_country_change}
                        onfocus={on_country_focus}
                        onblur={on_country_blur}
                        placeholder="Type PL or Poland"
                        autocomplete="off"
                    />
                    {
                        if let Some(message) = &*country_error {
                            html! {
                                <div class="text-sm mt-2" style="color: var(--md-sys-color-error-soft);">
                                    { message.clone() }
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button
                        label="Cancel"
                        button_type={ButtonType::Text}
                        onclick={move |_| on_close_click.emit(())}
                    />
                    <Button
                        label={if is_edit { "Save Changes" } else { "Add Node" }}
                        html_type={"submit"}
                        button_type={ButtonType::Filled}
                        onclick={Callback::from(move |_| {})}
                    />
                </div>
            </form>
            {
                if *show_country_results && !country_results.is_empty() {
                    let (left, top, width) = *country_menu_rect;
                    html! {
                        <div
                            class="md3-country-picker-menu"
                            style={format!("position: fixed; left: {left}px; top: {top}px; width: {width}px;")}
                        >
                            {
                                for country_results.into_iter().map(|option| {
                                    let option_flag = flag_emoji(option.code);
                                    let option_label = country_display(option);
                                    let country = country.clone();
                                    let country_query = country_query.clone();
                                    let show_country_results = show_country_results.clone();
                                    html! {
                                        <button
                                            type="button"
                                            class="md3-country-picker-option"
                                            onmousedown={Callback::from(move |_| {
                                                country.set(option.code.to_string());
                                                country_query.set(option_label.clone());
                                                show_country_results.set(false);
                                            })}
                                        >
                                            <span class="mr-2 text-xl leading-none">{ option_flag }</span>
                                            <span>{ format!("{} ({})", option.name, option.code) }</span>
                                        </button>
                                    }
                                })
                            }
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </Popup>
    }
}

#[derive(Properties, PartialEq)]
struct DeleteConfirmPopupProps {
    title: AttrValue,
    message: String,
    on_cancel: Callback<()>,
    on_confirm: Callback<()>,
}

#[function_component(DeleteConfirmPopup)]
fn delete_confirm_popup(props: &DeleteConfirmPopupProps) -> Html {
    let on_cancel_click = props.on_cancel.clone();
    let on_confirm_click = props.on_confirm.clone();

    html! {
        <Popup
            title={props.title.clone()}
            size={PopupSize::Sm}
            on_close={props.on_cancel.clone()}
        >
            <div class="space-y-4">
                <p class="text-sm opacity-80">{ &props.message }</p>
                <div class="md3-popup-actions" style="justify-content: flex-end;">
                    <Button
                        label="Cancel"
                        button_type={ButtonType::Text}
                        onclick={move |_| on_cancel_click.emit(())}
                    />
                    <Button
                        label="Delete"
                        button_type={ButtonType::Outlined}
                        color={Some("#F2B8B5".to_string())}
                        onclick={move |_| on_confirm_click.emit(())}
                    />
                </div>
            </div>
        </Popup>
    }
}
