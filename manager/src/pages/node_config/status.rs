use super::*;

pub(super) fn render_status_tab(
    node: &ProxyNode,
    live_status: &UseStateHandle<Option<NodeStatus>>,
    live_status_loading: &UseStateHandle<bool>,
    live_status_error: &UseStateHandle<Option<String>>,
    status_auto_refresh: &UseStateHandle<bool>,
    status_refresh_interval_ms: &UseStateHandle<u32>,
    status_refresh_menu_open: &UseStateHandle<bool>,
    on_refresh_live_status: &Callback<MouseEvent>,
) -> Html {
    html! {                        <div class="space-y-6">
                            <div class="flex justify-between" style="align-items: center; gap: 1rem;">
                                <div class="space-y-1" style="min-width: 0;">
                                    <h2 class="text-2xl font-bold">{ "Status" }</h2>
                                    {
                                        if let Some(error) = &**live_status_error {
                                            html! {
                                                <div class="text-sm" style="color: var(--md-sys-color-error-soft);">
                                                    { error.clone() }
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                                <div class="md3-btn-group">
                                    <Button
                                        label={"Refresh"}
                                        icon={Some("icon-sync".to_string())}
                                        button_type={ButtonType::Filled}
                                        loading={**live_status_loading}
                                        disabled={**live_status_loading}
                                        onclick={on_refresh_live_status.clone()}
                                    />
                                    <IconButton
                                        label="Status refresh settings"
                                        button_type={ButtonType::Filled}
                                        onclick={Callback::from({
                                            let status_refresh_menu_open = status_refresh_menu_open.clone();
                                            move |_| status_refresh_menu_open.set(true)
                                        })}
                                    >
                                        <SvgIcon name={"icon-chevron-down"} size={20} />
                                    </IconButton>
                                </div>
                            </div>
                            {
                                if **status_refresh_menu_open {
                                    let auto_refresh = **status_auto_refresh;
                                    let refresh_ms = **status_refresh_interval_ms;
                                    html! {
                                        <Popup
                                            title="Sampling & Refresh"
                                            size={PopupSize::Sm}
                                            on_close={Callback::from({
                                                let status_refresh_menu_open = status_refresh_menu_open.clone();
                                                move |_| status_refresh_menu_open.set(false)
                                            })}
                                        >
                                            <div class="space-y-4">
                                                <Dropdown
                                                    label="Sample rate"
                                                    value={refresh_ms.to_string()}
                                                    options={vec![
                                                        DropdownOption { value: "1000".to_string(), label: "1s".to_string() },
                                                        DropdownOption { value: "2000".to_string(), label: "2s".to_string() },
                                                        DropdownOption { value: "5000".to_string(), label: "5s".to_string() },
                                                        DropdownOption { value: "10000".to_string(), label: "10s".to_string() },
                                                    ]}
                                                    onchange={Callback::from({
                                                        let status_refresh_interval_ms = status_refresh_interval_ms.clone();
                                                        move |value: String| {
                                                            let next = value.parse::<u32>().unwrap_or(2000).max(250);
                                                            status_refresh_interval_ms.set(next);
                                                        }
                                                    })}
                                                />
                                                <SwitchField
                                                    label="Auto refresh"
                                                    checked={auto_refresh}
                                                    onchange={Callback::from({
                                                        let status_auto_refresh = status_auto_refresh.clone();
                                                        move |e: Event| {
                                                            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
                                                            status_auto_refresh.set(input.checked());
                                                        }
                                                    })}
                                                />
                                                <div class="md3-popup-actions" style="justify-content: flex-end;">
                                                    <Button
                                                        label="Close"
                                                        button_type={ButtonType::Text}
                                                        onclick={Callback::from({
                                                            let status_refresh_menu_open = status_refresh_menu_open.clone();
                                                            move |_| status_refresh_menu_open.set(false)
                                                        })}
                                                    />
                                                </div>
                                            </div>
                                        </Popup>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            {
                                if (**live_status_error).is_some() {
                                    html! { <StatusSkeletonPanel /> }
                                } else if let Some(status) = &**live_status {
                                    html! {
                                        <NodeStatusPanel
                                            status={status.clone()}
                                            bandwidth_mbps={node.bandwidth_mbps}
                                            max_traffic_bytes={node.max_traffic_bytes}
                                        />
                                    }
                                } else {
                                    html! { <StatusSkeletonPanel /> }
                                }
                            }
                        </div>
    }
}

