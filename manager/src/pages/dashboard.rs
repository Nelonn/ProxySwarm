use crate::components::{Button, ButtonType};
use crate::country::{country_name, flag_emoji, normalize_country_code};
use crate::pb::proxyswarm::{NodeStatus, TrafficStats};
use crate::services::ApiService;
use crate::state::State;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[function_component(Dashboard)]
pub fn dashboard() -> Html {
    let state = use_context::<UseStateHandle<State>>().expect("state context");
    let total_nodes = state.nodes.len();
    let total_accounts = state.accounts.len();

    html! {
        <div class="p-6 space-y-6">
            <h1 class="text-3xl font-bold">{ "Dashboard" }</h1>

            // Stats Cards
            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <StatCard title="Total Nodes" value={total_nodes.to_string()} />
                <StatCard title="Total Accounts" value={total_accounts.to_string()} />
            </div>

            // Node Status
            <div class="md3-card bg-surface-container">
                <h2 class="text-xl font-semibold mb-4">{ "Node Status" }</h2>
                if state.nodes.is_empty() {
                    <div class="text-center py-8 opacity-50">
                        <p>{ "No nodes configured yet" }</p>
                        <p class="text-sm mt-2">{ "Add a node to get started" }</p>
                    </div>
                }
                { for state.nodes.iter().map(|node| {
                    html! {
                        <NodeStatusCard
                            country={node.country.clone()}
                            node_name={node.name.clone()}
                            address={node.address.clone()}
                            master_key={node.master_key.clone()}
                        />
                    }
                }) }
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct StatCardProps {
    title: String,
    value: String,
}

#[function_component(StatCard)]
fn stat_card(props: &StatCardProps) -> Html {
    html! {
        <div class="md3-card bg-surface-container flex flex-col items-center justify-center">
            <div class="text-3xl font-bold">{ &props.value }</div>
            <div class="text-sm opacity-70 mt-1">{ &props.title }</div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct NodeStatusCardProps {
    country: String,
    node_name: String,
    address: String,
    master_key: String,
}

#[function_component(NodeStatusCard)]
fn node_status_card(props: &NodeStatusCardProps) -> Html {
    let status = use_state(|| Option::<NodeStatus>::None);
    let loading = use_state(|| false);
    let error = use_state(|| Option::<String>::None);

    let fetch_status = {
        let status = status.clone();
        let loading = loading.clone();
        let error = error.clone();
        let address = props.address.clone();
        let master_key = props.master_key.clone();

        Callback::from(move |_: MouseEvent| {
            let status = status.clone();
            let loading = loading.clone();
            let error = error.clone();
            let address = address.clone();
            let master_key = master_key.clone();

            loading.set(true);
            error.set(None);

            spawn_local(async move {
                let mut api = ApiService::new(address);
                match api.get_status(master_key).await {
                    Ok(s) => {
                        status.set(Some(s));
                        loading.set(false);
                    }
                    Err(e) => {
                        error.set(Some(e));
                        loading.set(false);
                    }
                }
            });
        })
    };

    html! {
        <div class="md3-card bg-surface-container">
            <div class="flex justify-between items-center mb-4">
                <div class="space-y-1">
                    <h3 class="font-bold text-lg">{ &props.node_name }</h3>
                    { render_country(&props.country) }
                </div>
                <Button
                    label={if *loading { "Loading...".to_string() } else { "Refresh".to_string() }}
                    button_type={ButtonType::Filled}
                    disabled={*loading}
                    onclick={fetch_status}
                />
            </div>

            if let Some(err) = &*error {
                <div class="text-error text-sm p-3 bg-error/10 rounded-lg">
                    { format!("Error: {}", err) }
                </div>
            } else if let Some(node_status) = &*status {
                <NodeStatusDisplay status={node_status.clone()} />
            } else {
                <div class="text-center py-8 opacity-50">
                    <p>{ "Click refresh to load node status" }</p>
                </div>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct NodeStatusDisplayProps {
    status: NodeStatus,
}

#[function_component(NodeStatusDisplay)]
fn node_status_display(props: &NodeStatusDisplayProps) -> Html {
    let current_traffic = current_traffic_usage(
        props.status.total_inbound_traffic.as_ref(),
        props.status.total_outbound_traffic.as_ref(),
    );
    let total_traffic = total_traffic_usage(
        props.status.total_inbound_traffic.as_ref(),
        props.status.total_outbound_traffic.as_ref(),
    );

    html! {
        <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
            <div class="bg-surface-container p-3 rounded-lg">
                <div class="text-xs opacity-70 uppercase">{ "Current Traffic Usage" }</div>
                <div class="text-xl font-bold">{ current_traffic }</div>
            </div>
            <div class="bg-surface-container p-3 rounded-lg">
                <div class="text-xs opacity-70 uppercase">{ "Total Traffic Usage" }</div>
                <div class="text-xl font-bold">{ total_traffic }</div>
            </div>
        </div>
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

fn render_country(country: &str) -> Html {
    let normalized = normalize_country_code(country);
    let flag = flag_emoji(country);

    if normalized.is_empty() || flag.is_empty() {
        html! {
            <div class="flex items-center gap-2" style="min-height: 1.5rem;">
                <span class="text-sm opacity-70">{ "Not set" }</span>
            </div>
        }
    } else {
        let label = country_name(country)
            .map(|name| format!("{} ({})", name, normalized))
            .unwrap_or_else(|| normalized.clone());
        html! {
            <div
                class="flex items-center gap-2 text-sm opacity-70"
                style="line-height: 1; min-height: 1.5rem; column-gap: 0.625rem;"
            >
                <span
                    class="text-2xl leading-none"
                    style="display: inline-flex; align-items: center; line-height: 1;"
                >
                    { flag }
                </span>
                <span style="display: inline-flex; align-items: center; line-height: 1;">
                    { label }
                </span>
            </div>
        }
    }
}

fn current_traffic_usage(inbound: Option<&TrafficStats>, outbound: Option<&TrafficStats>) -> String {
    let inbound_rate = inbound.map(|traffic| traffic.rx_rate + traffic.tx_rate).unwrap_or(0.0);
    let outbound_rate = outbound.map(|traffic| traffic.rx_rate + traffic.tx_rate).unwrap_or(0.0);
    format!("{}/s", format_bytes((inbound_rate + outbound_rate).round() as u64))
}

fn total_traffic_usage(inbound: Option<&TrafficStats>, outbound: Option<&TrafficStats>) -> String {
    let inbound_total = inbound.map(|traffic| traffic.rx.saturating_add(traffic.tx)).unwrap_or(0);
    let outbound_total = outbound.map(|traffic| traffic.rx.saturating_add(traffic.tx)).unwrap_or(0);
    format_bytes(inbound_total.saturating_add(outbound_total))
}
