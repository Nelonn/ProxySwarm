use super::*;

fn format_account_status_id(account: &AccountStatus) -> String {
    account.id.trim().to_string()
}

pub(super) fn format_status_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes < 1024_u64.pow(4) {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!(
            "{:.2} TB",
            bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0)
        )
    }
}

pub(super) fn format_status_rate(bytes_per_second: f64) -> String {
    // Network speeds are conventionally shown in bits/sec (not bytes/sec).
    let bits_per_second = (bytes_per_second * 8.0).max(0.0);
    if bits_per_second < 1000.0 {
        format!("{:.0} b/s", bits_per_second.round())
    } else if bits_per_second < 1_000_000.0 {
        format!("{:.1} Kb/s", bits_per_second / 1000.0)
    } else if bits_per_second < 1_000_000_000.0 {
        format!("{:.1} Mb/s", bits_per_second / 1_000_000.0)
    } else if bits_per_second < 1_000_000_000_000.0 {
        format!("{:.2} Gb/s", bits_per_second / 1_000_000_000.0)
    } else {
        format!("{:.2} Tb/s", bits_per_second / 1_000_000_000_000.0)
    }
}

pub(super) fn format_optional_limit_bytes(bytes: Option<u64>) -> String {
    bytes
        .map(format_status_bytes)
        .unwrap_or_else(|| "Unlimited".to_string())
}

pub(super) fn format_optional_bandwidth(bandwidth_mbps: Option<u32>) -> String {
    bandwidth_mbps
        .map(|value| format!("{} Mbps", value))
        .unwrap_or_else(|| "Not set".to_string())
}

fn server_inbound_traffic(status: &NodeStatus) -> (u64, f64) {
    let from_clients = status
        .total_inbound_traffic
        .as_ref()
        .map(|traffic| (traffic.tx, traffic.tx_rate))
        .unwrap_or((0, 0.0));
    let from_servers = status
        .total_outbound_traffic
        .as_ref()
        .map(|traffic| (traffic.rx, traffic.rx_rate))
        .unwrap_or((0, 0.0));
    (
        from_clients.0.saturating_add(from_servers.0),
        from_clients.1 + from_servers.1,
    )
}

fn server_outbound_traffic(status: &NodeStatus) -> (u64, f64) {
    let to_clients = status
        .total_inbound_traffic
        .as_ref()
        .map(|traffic| (traffic.rx, traffic.rx_rate))
        .unwrap_or((0, 0.0));
    let to_servers = status
        .total_outbound_traffic
        .as_ref()
        .map(|traffic| (traffic.tx, traffic.tx_rate))
        .unwrap_or((0, 0.0));
    (
        to_clients.0.saturating_add(to_servers.0),
        to_clients.1 + to_servers.1,
    )
}

#[derive(Properties, PartialEq)]
pub(super) struct CircularProgressProps {
    value: f64,
    #[prop_or(false)]
    show_label_inside: bool,
}

#[function_component(CircularProgress)]
pub(super) fn circular_progress(props: &CircularProgressProps) -> Html {
    let value = props.value.clamp(0.0, 100.0);
    let normalized = value / 100.0;
    let radius = 18.0;
    let stroke_width = 4.0;
    let circumference = 2.0 * std::f64::consts::PI * radius;
    let gap_length = 7.0;
    let track_length = (circumference - gap_length * 2.0).max(0.0);
    let active_length = (track_length * normalized).clamp(0.0, track_length);
    let inactive_length = (track_length - active_length).max(0.0);
    let active_dasharray = format!("{:.3} {:.3}", active_length, circumference);
    let inactive_dasharray = format!("{:.3} {:.3}", inactive_length, circumference);
    let active_dashoffset = format!("{:.3}", -gap_length / 2.0);
    let inactive_dashoffset = format!("{:.3}", -(active_length + gap_length * 1.5));

    html! {
        <div
            style="position: relative; width: 72px; height: 72px; flex: 0 0 auto;"
        >
            <svg
                viewBox="0 0 48 48"
                width="72"
                height="72"
                aria-hidden="true"
                style="display: block;"
            >
                <g transform="rotate(-90 24 24)">
                    {
                        if inactive_length > 0.01 {
                            html! {
                                <circle
                                    cx="24"
                                    cy="24"
                                    r={radius.to_string()}
                                    fill="none"
                                    stroke="var(--md-sys-color-outline-variant)"
                                    stroke-width={stroke_width.to_string()}
                                    stroke-linecap="round"
                                    stroke-dasharray={inactive_dasharray}
                                    stroke-dashoffset={inactive_dashoffset}
                                    style="transition: stroke-dasharray 240ms ease, stroke-dashoffset 240ms ease;"
                                />
                            }
                        } else {
                            html! {}
                        }
                    }
                    {
                        if active_length > 0.01 {
                            html! {
                                <circle
                                    cx="24"
                                    cy="24"
                                    r={radius.to_string()}
                                    fill="none"
                                    stroke="var(--md-sys-color-primary)"
                                    stroke-width={stroke_width.to_string()}
                                    stroke-linecap="round"
                                    stroke-dasharray={active_dasharray}
                                    stroke-dashoffset={active_dashoffset}
                                    style="transition: stroke-dasharray 240ms ease, stroke-dashoffset 240ms ease;"
                                />
                            }
                        } else {
                            html! {}
                        }
                    }
                </g>
            </svg>
            <div
                style="position: absolute; inset: 0px; display: flex; align-items: center; justify-content: center; pointer-events: none;"
            >
                <div class="font-semibold" style="font-size: 13px; line-height: 16px;">
                    {
                        if props.show_label_inside {
                            html! { format!("{:.0}%", value) }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct UnifiedTrafficProps {
    traffic: Option<TrafficStats>,
    #[prop_or(false)]
    invert_icon: bool,
}

#[function_component(UnifiedTraffic)]
pub(super) fn unified_traffic(props: &UnifiedTrafficProps) -> Html {
    let traffic = props.traffic.clone().unwrap_or_default();
    let outbound_icon = "icon-straight";
    let inbound_icon = "icon-straight-inbound";
    html! {
        <div class="opacity-80 rounded-lg" style="font-size: 13px; font-weight: 500; line-height: 18px; border: 0px solid var(--md-sys-color-outline-variant); padding: 4px 10px 4px 4px;">
            <div class="flex items-center justify-end" style="gap: 2px; min-height: 18px;">
                <span style="height: 18px; width: 18px; display: inline-flex; align-items: center; justify-content: center; flex: 0 0 18px;">
                    <SvgIcon name={outbound_icon} size={14} class={classes!("opacity-70")} />
                </span>
                <span style="display: inline-flex; align-items: center; min-height: 18px;">
                    { format!("{} ({})", format_status_bytes(traffic.tx), format_status_rate(traffic.tx_rate)) }
                </span>
            </div>
            <div class="flex items-center justify-end" style="gap: 2px; min-height: 18px;">
                <span style="height: 18px; width: 18px; display: inline-flex; align-items: center; justify-content: center; flex: 0 0 18px;">
                    <SvgIcon name={inbound_icon} size={14} class={classes!("opacity-70")} />
                </span>
                <span style="display: inline-flex; align-items: center; min-height: 18px;">
                    { format!("{} ({})", format_status_bytes(traffic.rx), format_status_rate(traffic.rx_rate)) }
                </span>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct UserStatusDotProps {
    online: bool,
}

#[function_component(UserStatusDot)]
pub(super) fn user_status_dot(props: &UserStatusDotProps) -> Html {
    let dot_color = if props.online {
        "var(--md-sys-color-primary)"
    } else {
        "var(--md-sys-color-outline)"
    };
    let ripple_color = if props.online {
        "color-mix(in srgb, var(--md-sys-color-primary) 30%, transparent)"
    } else {
        "transparent"
    };

    html! {
        <div
            style={format!(
                "position: relative; width: 18px; height: 18px; flex: 0 0 18px; display: inline-flex; align-items: center; justify-content: center;",
            )}
            aria-label={if props.online { "Online" } else { "Offline" }}
            title={if props.online { "Online" } else { "Offline" }}
        >
            <span
                style={format!(
                    "width: 10px; height: 10px; border-radius: 999px; background: {}; display: block; flex: 0 0 10px;",
                dot_color
            )}
            />
            {
                if props.online {
                    html! {
                        <>
                            <span style={format!(
                                "position: absolute; left: 4px; top: 4px; width: 10px; height: 10px; border-radius: 999px; background: {}; animation: md3-user-status-ripple 1s ease-out infinite;",
                                ripple_color
                            )} />
                        </>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub(super) struct NodeStatusPanelProps {
    pub(super) status: NodeStatus,
    pub(super) accounts: Vec<AccountInfo>,
    pub(super) bandwidth_mbps: Option<u32>,
    pub(super) max_traffic_bytes: Option<u64>,
}

#[function_component(NodeStatusPanel)]
pub(super) fn node_status_panel(props: &NodeStatusPanelProps) -> Html {
    let (server_inbound_bytes, server_inbound_rate) = server_inbound_traffic(&props.status);
    let (server_outbound_bytes, server_outbound_rate) = server_outbound_traffic(&props.status);
    let total_traffic = server_inbound_bytes.saturating_add(server_outbound_bytes);
    let traffic_cap_progress = props.max_traffic_bytes.map(|limit| {
        if limit == 0 {
            0.0
        } else {
            ((total_traffic as f64 / limit as f64) * 100.0).clamp(0.0, 100.0)
        }
    });

    html! {
        <div class="space-y-6">
            <style>
                {r#"
                    @keyframes md3-user-status-ripple {
                        0% {
                            opacity: 0;
                            transform: scale(1);
                        }
                        12% {
                            opacity: 0.55;
                            transform: scale(1);
                        }
                        70% {
                            opacity: 0;
                            transform: scale(2.4);
                        }
                        100% {
                            opacity: 0;
                            transform: scale(2.4);
                        }
                    }
                "#}
            </style>
            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Server Bandwidth" }</div>
                    <div class="font-bold" style="font-size: 20px; line-height: 28px;">{ format_optional_bandwidth(props.bandwidth_mbps) }</div>
                </div>
                <div class="md3-card bg-surface-container">
                    <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                        <div style="min-width: 0px; flex: 1 1 auto;">
                            <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Max Traffic" }</div>
                            <div class="font-bold" style="font-size: 20px; line-height: 28px;">{ format_optional_limit_bytes(props.max_traffic_bytes) }</div>
                            {
                                if traffic_cap_progress.is_some() {
                                    html! {
                                        <div style="color: var(--md-sys-color-on-surface-variant); font-size: 13px; line-height: 18px;">
                                            { format!("Current: {}", format_status_bytes(total_traffic)) }
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                        </div>
                        <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                            {
                                if let Some(progress) = traffic_cap_progress {
                                    html! { <CircularProgress value={progress} show_label_inside={true} /> }
                                } else {
                                    html! {}
                                }
                            }
                        </div>
                    </div>
                </div>
            </div>

            {
                if let Some(hw) = &props.status.hardware {
                    let ram_progress = if hw.ram_total == 0 {
                        0.0
                    } else {
                        ((hw.ram_used as f64 / hw.ram_total as f64) * 100.0).clamp(0.0, 100.0)
                    };
                    html! {
                        <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                            <div class="md3-card bg-surface-container">
                                <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                                    <div style="min-width: 0px; flex: 1 1 auto;">
                                        <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "CPU Usage" }</div>
                                        <div class="font-bold" style="font-size: 20px; line-height: 28px; margin-top: 8px;">{ "Processor Load" }</div>
                                        <div style="color: var(--md-sys-color-on-surface-variant); font-size: 13px; line-height: 18px;">
                                            {
                                                if hw.cpu_cores > 0 {
                                                    format!("{} CPU cores", hw.cpu_cores)
                                                } else {
                                                    "CPU cores unavailable".to_string()
                                                }
                                            }
                                        </div>
                                    </div>
                                    <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                                        <CircularProgress value={hw.cpu_usage} show_label_inside={true} />
                                    </div>
                                </div>
                            </div>
                            <div class="md3-card bg-surface-container">
                                <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                                    <div style="min-width: 0px; flex: 1 1 auto;">
                                        <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "RAM Usage" }</div>
                                        <div class="font-bold" style="font-size: 20px; line-height: 28px; margin-top: 8px;">{ format!("{} / {}", format_status_bytes(hw.ram_used), format_status_bytes(hw.ram_total)) }</div>
                                        <div style="color: var(--md-sys-color-on-surface-variant); font-size: 13px; line-height: 18px;">
                                            { format!("Free {}", format_status_bytes(hw.ram_total.saturating_sub(hw.ram_used))) }
                                        </div>
                                    </div>
                                    <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                                        <CircularProgress value={ram_progress} show_label_inside={true} />
                                    </div>
                                </div>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Server Inbound Traffic" }</div>
                    <div class="font-bold" style="font-size: 20px; line-height: 28px;">
                        { format_status_bytes(server_inbound_bytes) }
                    </div>
                    <div class="mt-2" style="font-size: 13px; line-height: 18px;">
                        {
                            if props.status.total_inbound_traffic.is_some() || props.status.total_outbound_traffic.is_some() {
                                html! {
                                    <>
                                        <div style="color: var(--md-sys-color-on-surface-variant);">{ format!("Total: {}", format_status_rate(server_inbound_rate)) }</div>
                                        <div style="color: var(--md-sys-color-on-surface-variant);">{ "Traffic entering this node" }</div>
                                    </>
                                }
                            } else {
                                html! { <div style="color: var(--md-sys-color-on-surface-variant);">{ "No sample yet" }</div> }
                            }
                        }
                    </div>
                </div>
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Server Outbound Traffic" }</div>
                    <div class="font-bold" style="font-size: 20px; line-height: 28px;">
                        { format_status_bytes(server_outbound_bytes) }
                    </div>
                    <div class="mt-2" style="font-size: 13px; line-height: 18px;">
                        {
                            if props.status.total_inbound_traffic.is_some() || props.status.total_outbound_traffic.is_some() {
                                html! {
                                    <>
                                        <div style="color: var(--md-sys-color-on-surface-variant);">{ format!("Total: {}", format_status_rate(server_outbound_rate)) }</div>
                                        <div style="color: var(--md-sys-color-on-surface-variant);">{ "Traffic leaving this node" }</div>
                                    </>
                                }
                            } else {
                                html! { <div style="color: var(--md-sys-color-on-surface-variant);">{ "No sample yet" }</div> }
                            }
                        }
                    </div>
                </div>
            </div>

            <div class="md3-card bg-surface-container">
                <div class="flex justify-between" style="align-items: center;">
                    <div>
                        <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ "Connections" }</div>
                        <div class="font-bold" style="font-size: 24px; line-height: 32px;">
                            {
                                props.status
                                    .connections
                                    .clone()
                                    .map(|c| format!("TCP {} / UDP {}", c.tcp, c.udp))
                                    .unwrap_or_else(|| "TCP 0 / UDP 0".to_string())
                            }
                        </div>
                    </div>
                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">
                        { format!("Sample window: {}s", props.status.sample_window_seconds.max(1)) }
                    </div>
                </div>
            </div>

            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container space-y-3">
                    <div class="font-semibold uppercase opacity-70" style="font-size: 13px; line-height: 18px;">{ "Inbounds" }</div>
                    { for props.status.inbounds.iter().map(|inbound: &InboundStatus| html! {
                        <div class="bg-surface-container p-3 rounded-lg">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;">
                                    <div class="font-medium" style="font-size: 14px; line-height: 20px;">{ inbound.name.clone() }</div>
                                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">
                                        {
                                            inbound.connections.clone()
                                                .map(|c| format!("TCP {} / UDP {}", c.tcp, c.udp))
                                                .unwrap_or_else(|| "TCP 0 / UDP 0".to_string())
                                        }
                                    </div>
                                </div>
                                <UnifiedTraffic traffic={inbound.traffic.clone()} invert_icon={true} />
                            </div>
                        </div>
                    }) }
                </div>
                <div class="md3-card bg-surface-container space-y-3">
                    <div class="font-semibold uppercase opacity-70" style="font-size: 13px; line-height: 18px;">{ "Outbounds" }</div>
                    { for props.status.outbounds.iter().map(|outbound: &OutboundStatus| html! {
                        <div class="bg-surface-container p-3 rounded-lg">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;">
                                    <div class="font-medium" style="font-size: 14px; line-height: 20px;">{ outbound.name.clone() }</div>
                                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">
                                        {
                                            if outbound.excluded_from_totals {
                                                format!("{} • excluded from totals", outbound.r#type)
                                            } else {
                                                outbound.r#type.clone()
                                            }
                                        }
                                    </div>
                                </div>
                                <UnifiedTraffic traffic={outbound.traffic.clone()} />
                            </div>
                        </div>
                    }) }
                </div>
            </div>

            <div class="md3-card bg-surface-container space-y-3">
                <div class="font-semibold uppercase opacity-70" style="font-size: 13px; line-height: 18px;">{ "Users" }</div>
                { for props.status.accounts.iter().map(|account: &AccountStatus| {
                    let is_online = account.online > 0;
                    let account_label = format_account_status_id(account);
                    html! {
                        <div class="bg-surface-container p-3 rounded-lg space-y-2">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div class="space-y-1" style="min-width: 0px;">
                                    <div class="flex items-center" style="gap: 10px; min-height: 20px; align-items: center;">
                                        <UserStatusDot online={is_online} />
                                        <div class="font-semibold" style="font-size: 15px; line-height: 20px; display: flex; align-items: center; min-height: 20px;">{ account_label }</div>
                                    </div>
                                </div>
                                <UnifiedTraffic traffic={account.traffic.clone()} invert_icon={true} />
                            </div>
                        </div>
                    }
                }) }
            </div>
        </div>
    }
}

#[function_component(StatusSkeletonPanel)]
pub(super) fn status_skeleton_panel() -> Html {
    let bar = |width: &str, height: &str| {
        html! {
            <div
                class="rounded-full"
                style={format!(
                    "width: {}; height: {}; background-color: rgba(255, 255, 255, 0.10);",
                    width, height
                )}
            />
        }
    };
    let dot = |size: &str| {
        html! {
            <div
                class="rounded-full"
                style={format!(
                    "width: {}; height: {}; background-color: rgba(255, 255, 255, 0.10); flex: 0 0 {};",
                    size, size, size
                )}
            />
        }
    };
    let ring = || {
        html! {
            <div style="position: relative; width: 72px; height: 72px; flex: 0 0 auto;">
                <svg
                    viewBox="0 0 48 48"
                    width="72"
                    height="72"
                    aria-hidden="true"
                    style="display: block;"
                >
                    <circle
                        cx="24"
                        cy="24"
                        r="18"
                        fill="none"
                        stroke="rgba(255, 255, 255, 0.10)"
                        stroke-width="4"
                        stroke-linecap="round"
                    />
                </svg>
            </div>
        }
    };
    let traffic_line = |label_width: &str, value_width: &str| {
        html! {
            <div class="flex items-center" style="gap: 2px; min-height: 18px;">
                <span style="height: 18px; width: 18px; display: inline-flex; align-items: center; justify-content: center; flex: 0 0 18px;">
                    <div class="rounded-full" style="width: 10px; height: 10px; background-color: rgba(255, 255, 255, 0.10);" />
                </span>
                <div style="display: inline-flex; align-items: center; min-height: 18px; gap: 6px;">
                    { bar(label_width, "14px") }
                    { bar(value_width, "14px") }
                </div>
            </div>
        }
    };
    let traffic_stack = || {
        html! {
            <div class="opacity-80 rounded-lg" style="font-size: 13px; font-weight: 500; line-height: 18px; padding: 4px 10px 4px 4px;">
                { traffic_line("4.25rem", "5.5rem") }
                { traffic_line("4.25rem", "5.5rem") }
            </div>
        }
    };

    html! {
        <div class="space-y-6 animate-pulse">
            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container space-y-2">
                    { bar("9rem", "16px") }
                    { bar("6rem", "28px") }
                </div>
                <div class="md3-card bg-surface-container space-y-2">
                    { bar("8rem", "16px") }
                    { bar("6.5rem", "28px") }
                </div>
            </div>
            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container">
                    <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                        <div style="min-width: 0px; flex: 1 1 auto;" class="space-y-2">
                            { bar("6rem", "16px") }
                            { bar("8rem", "28px") }
                            { bar("7rem", "18px") }
                        </div>
                        <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                            { ring() }
                        </div>
                    </div>
                </div>
                <div class="md3-card bg-surface-container">
                    <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px;">
                        <div style="min-width: 0px; flex: 1 1 auto;" class="space-y-2">
                            { bar("6rem", "16px") }
                            { bar("8rem", "28px") }
                            { bar("7rem", "18px") }
                        </div>
                        <div style="display: flex; align-items: center; justify-content: center; flex: 0 0 72px;">
                            { ring() }
                        </div>
                    </div>
                </div>
            </div>

            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ bar("9rem", "16px") }</div>
                    <div class="font-bold" style="font-size: 24px; line-height: 32px;">{ bar("10rem", "32px") }</div>
                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">{ bar("7rem", "18px") }</div>
                </div>
                <div class="md3-card bg-surface-container">
                    <div class="opacity-70 uppercase" style="font-size: 12px; line-height: 16px;">{ bar("9rem", "16px") }</div>
                    <div class="font-bold" style="font-size: 24px; line-height: 32px;">{ bar("10rem", "32px") }</div>
                    <div class="opacity-70" style="font-size: 13px; line-height: 18px;">{ bar("7rem", "18px") }</div>
                </div>
            </div>

            <div class="md3-card bg-surface-container">
                <div class="flex justify-between" style="align-items: center; gap: 16px;">
                    <div class="space-y-2">
                        { bar("22%", "16px") }
                        { bar("14rem", "32px") }
                    </div>
                    <div style="width: 72px;"></div>
                </div>
            </div>

            <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                <div class="md3-card bg-surface-container space-y-3">
                    { bar("18%", "18px") }
                    { for (0..4).map(|_| html! {
                        <div class="bg-surface-container p-3 rounded-lg space-y-2">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;" class="space-y-2">
                                    { bar("8rem", "20px") }
                                    { bar("10rem", "18px") }
                                </div>
                                { traffic_stack() }
                            </div>
                        </div>
                    }) }
                </div>
                <div class="md3-card bg-surface-container space-y-3">
                    { bar("20%", "18px") }
                    { for (0..4).map(|_| html! {
                        <div class="bg-surface-container p-3 rounded-lg space-y-2">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;" class="space-y-2">
                                    { bar("8rem", "20px") }
                                    { bar("10rem", "18px") }
                                </div>
                                { traffic_stack() }
                            </div>
                        </div>
                    }) }
                </div>
            </div>

            <div class="md3-card bg-surface-container">
                { bar("18%", "18px") }
                <div class="space-y-3 mt-3">
                    { for (0..4).map(|_| html! {
                        <div class="bg-surface-container p-3 rounded-lg space-y-2">
                            <div class="flex justify-between" style="align-items: center; gap: 16px;">
                                <div style="min-width: 0px;" class="space-y-2">
                                    <div class="flex items-center" style="gap: 10px; min-height: 20px; align-items: center;">
                                        { dot("10px") }
                                        { bar("9rem", "20px") }
                                    </div>
                                </div>
                                { bar("5.5rem", "20px") }
                            </div>
                        </div>
                    }) }
                </div>
            </div>
        </div>
    }
}
