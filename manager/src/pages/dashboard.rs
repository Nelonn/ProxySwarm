use crate::pb::proxyswarm::{AccountStatus, InboundStatus, NodeStatus};
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
                            node_id={node.id.clone()}
                            node_name={node.name.clone()}
                            address={node.address.clone()}
                            public_ip={node.public_ip.clone()}
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
    node_id: String,
    node_name: String,
    address: String,
    public_ip: String,
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
                <div>
                    <h3 class="font-bold text-lg">{ &props.node_name }</h3>
                    <p class="text-sm opacity-70">{ format!("Access: {}", &props.address) }</p>
                    {
                        if props.public_ip.trim().is_empty() {
                            html! {}
                        } else {
                            html! { <p class="text-sm opacity-70">{ format!("Public IP: {}", &props.public_ip) }</p> }
                        }
                    }
                </div>
                <button
                    onclick={fetch_status}
                    disabled={*loading}
                    class="md3-btn md3-btn-filled text-sm hover:opacity-90 disabled:opacity-50 transition-opacity"
                >
                    { if *loading { "Loading..." } else { "Refresh" } }
                </button>
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
    html! {
        <div class="space-y-4">
            // Hardware Stats
            if let Some(hw) = &props.status.hardware {
                <div class="grid grid-cols-2 gap-4">
                    <div class="bg-surface-container p-3 rounded-lg">
                        <div class="text-xs opacity-70 uppercase">{"CPU Usage"}</div>
                        <div class="text-xl font-bold">{ format!("{:.1}%", hw.cpu_usage) }</div>
                    </div>
                    <div class="bg-surface-container p-3 rounded-lg">
                        <div class="text-xs opacity-70 uppercase">{"RAM Usage"}</div>
                        <div class="text-xl font-bold">{ format!("{} / {}", format_bytes(hw.ram_used), format_bytes(hw.ram_total)) }</div>
                    </div>
                    <div class="bg-surface-container p-3 rounded-lg">
                        <div class="text-xs opacity-70 uppercase">{"Uptime"}</div>
                        <div class="text-xl font-bold">{ format_uptime(hw.uptime) }</div>
                    </div>
                </div>
            }

            // Inbound Status
            if !props.status.inbounds.is_empty() {
                <div>
                    <h4 class="font-semibold text-sm mb-2 uppercase opacity-70">{"Inbounds"}</h4>
                    <div class="space-y-2">
                        { for props.status.inbounds.iter().map(|inbound| {
                            html! {
                                <InboundRow inbound={inbound.clone()} />
                            }
                        }) }
                    </div>
                </div>
            }

            // Account Status
            if !props.status.accounts.is_empty() {
                <div>
                    <h4 class="font-semibold text-sm mb-2 uppercase opacity-70">{"Accounts"}</h4>
                    <div class="space-y-2">
                        { for props.status.accounts.iter().map(|account| {
                            html! {
                                <AccountRow account={account.clone()} />
                            }
                        }) }
                    </div>
                </div>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct InboundRowProps {
    inbound: InboundStatus,
}

#[function_component(InboundRow)]
fn inbound_row(props: &InboundRowProps) -> Html {
    html! {
        <div class="flex justify-between items-center bg-surface-container p-3 rounded-lg">
            <span class="font-medium">{ &props.inbound.name }</span>
            if let Some(traffic) = &props.inbound.traffic {
                <span class="text-sm opacity-70">
                    { format!("↓ {} ↑ {}", format_bytes(traffic.rx), format_bytes(traffic.tx)) }
                </span>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct AccountRowProps {
    account: AccountStatus,
}

#[function_component(AccountRow)]
fn account_row(props: &AccountRowProps) -> Html {
    html! {
        <div class="flex justify-between items-center bg-surface-container p-3 rounded-lg">
            <span class="font-medium">{ &props.account.name }</span>
            if let Some(traffic) = &props.account.traffic {
                <span class="text-sm opacity-70">
                    { format!("↓ {} ↑ {}", format_bytes(traffic.rx), format_bytes(traffic.tx)) }
                </span>
            }
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
