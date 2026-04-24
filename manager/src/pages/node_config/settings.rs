use super::*;

pub(super) fn render_settings_tab(
    draft: &UseStateHandle<NodeConfigDraft>,
    d: &NodeConfigDraft,
    editing_certificate: &UseStateHandle<Option<(CertificateDraft, bool)>>,
    editing_dns_server: &UseStateHandle<Option<(usize, DnsServerDraft, bool)>>,
    editing_dns_host: &UseStateHandle<Option<(usize, DnsHostDraft, bool)>>,
    acme_confirm_open: &UseStateHandle<bool>,
    pending_acme_certificate: &UseStateHandle<Option<CertificateDraft>>,
) -> Html {
    let update_dns_text = |mutator: fn(&mut DnsDraft, String)| {
        let draft = draft.clone();
        Callback::from(move |value: String| {
            let mut next = (*draft).clone();
            mutator(&mut next.dns, value);
            sync_draft(&mut next);
            draft.set(next);
        })
    };
    let update_dns_u32 = |mutator: fn(&mut DnsDraft, u32)| {
        let draft = draft.clone();
        Callback::from(move |value: String| {
            let mut next = (*draft).clone();
            mutator(&mut next.dns, value.parse::<u32>().unwrap_or(0));
            sync_draft(&mut next);
            draft.set(next);
        })
    };
    let update_dns_bool = |mutator: fn(&mut DnsDraft, bool)| {
        let draft = draft.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            let mut next = (*draft).clone();
            mutator(&mut next.dns, input.checked());
            sync_draft(&mut next);
            draft.set(next);
        })
    };
    let update_link_remark_template = {
        let draft = draft.clone();
        Callback::from(move |value: String| {
            let mut next = (*draft).clone();
            next.link_remark_template = value;
            sync_draft(&mut next);
            draft.set(next);
        })
    };

    html! {
        <div class="space-y-6">
            <ConfigSection title="Access Link Remark">
                <div class="space-y-3">
                    <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                        { "Controls generated access-link names in node UI and registry template links. Available placeholders: {node}, {inbound}, {user}." }
                    </div>
                    <TextBox
                        label="Remark Template"
                        value={d.link_remark_template.clone()}
                        onchange={update_link_remark_template}
                        placeholder="{node}-{inbound}-{user}"
                    />
                </div>
            </ConfigSection>
            <ConfigSection title="Certificates">
                <div class="flex justify-between" style="align-items: center; gap: 0.75rem;">
                    <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                        { "Create reusable ACME or custom certificates here, then choose a certificate name inside each inbound TLS config." }
                    </div>
                    <Button
                        label="Add Certificate"
                        icon={Some("icon-add".to_string())}
                        button_type={ButtonType::Filled}
                        onclick={Callback::from({
                            let editing_certificate = editing_certificate.clone();
                            move |_| editing_certificate.set(Some((default_certificate_draft(), true)))
                        })}
                    />
                </div>
                <div class="space-y-4">
                    {
                        if d.certificates.is_empty() {
                            html! {
                                <div class="md3-card bg-surface-container">
                                    <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                                        { "No certificates yet." }
                                    </div>
                                </div>
                            }
                        } else {
                            html! {
                                <>
                                    {
                                        for d.certificates.iter().map(|certificate| {
                                            let edit_id = certificate.id.clone();
                                            let delete_id = certificate.id.clone();
                                            let delete_name = certificate.name.clone();
                                            let acme_cert = certificate.clone();
                                            html! {
                                                <div class="md3-card bg-surface-container">
                                                    <div class="flex justify-between" style="align-items: flex-start; gap: 16px;">
                                                        <div class="space-y-1">
                                                            <div class="font-semibold">{ certificate_display_name(certificate) }</div>
                                                            <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                                                                { format!("Type: {}", certificate.cert_type) }
                                                            </div>
                                                            <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                                                                {
                                                                    if certificate.cert_type == "ACME" {
                                                                        format!("{} via {}", certificate.acme_domain, certificate.acme_ca)
                                                                    } else {
                                                                        format!("{} | {}", certificate.certificate_path, certificate.key_path)
                                                                    }
                                                                }
                                                            </div>
                                                        </div>
                                                        <div class="md3-list-actions">
                                                            {
                                                                if certificate.cert_type == "ACME" {
                                                                    html! {
                                                                        <Button
                                                                            label="Try to Get Certificate"
                                                                            button_type={ButtonType::Outlined}
                                                                            onclick={Callback::from({
                                                                                let acme_confirm_open = acme_confirm_open.clone();
                                                                                let pending_acme_certificate = pending_acme_certificate.clone();
                                                                                move |_| {
                                                                                    pending_acme_certificate.set(Some(acme_cert.clone()));
                                                                                    acme_confirm_open.set(true);
                                                                                }
                                                                            })}
                                                                        />
                                                                    }
                                                                } else {
                                                                    html! {}
                                                                }
                                                            }
                                                            <Button label="Edit" button_type={ButtonType::Outlined} onclick={Callback::from({
                                                                let editing_certificate = editing_certificate.clone();
                                                                let draft = draft.clone();
                                                                move |_| {
                                                                    let mut data = (*draft).clone();
                                                                    sync_draft(&mut data);
                                                                    editing_certificate.set(data.certificates.iter().find(|item| item.id == edit_id).cloned().map(|value| (value, false)));
                                                                }
                                                            })} />
                                                            <Button label="Delete" button_type={ButtonType::Text} color={Some("#F2B8B5".to_string())} onclick={Callback::from({
                                                                let draft = draft.clone();
                                                                move |_| {
                                                                    let mut next = (*draft).clone();
                                                                    sync_draft(&mut next);
                                                                    next.certificates.retain(|item| item.id != delete_id);
                                                                    let replacement = next.certificates.first().map(|item| item.name.clone()).unwrap_or_default();
                                                                    for inbound in next.inbounds.iter_mut() {
                                                                        if inbound.tls.certificate_name == delete_name {
                                                                            inbound.tls.certificate_name = replacement.clone();
                                                                        }
                                                                    }
                                                                    draft.set(next);
                                                                }
                                                            })} />
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        })
                                    }
                                </>
                            }
                        }
                    }
                </div>
            </ConfigSection>
            <ConfigSection title="DNS">
                <div class="grid grid-cols-1 md-grid-cols-2 gap-4">
                    <TextBox label="Client IP" value={d.dns.client_ip.clone()} onchange={update_dns_text(|dns, value| dns.client_ip = value)} placeholder="Optional" />
                    <TextBox label="Tag" value={d.dns.tag.clone()} onchange={update_dns_text(|dns, value| dns.tag = value)} placeholder="Optional" />
                    <TextBox label="Query Strategy" value={d.dns.query_strategy.clone()} onchange={update_dns_text(|dns, value| dns.query_strategy = value)} placeholder="Optional" />
                    <TextBox label="Serve Expired TTL" value={d.dns.serve_expired_ttl.to_string()} onchange={update_dns_u32(|dns, value| dns.serve_expired_ttl = value)} input_type="number" />
                    <SwitchField label="Disable Cache" checked={d.dns.disable_cache} onchange={update_dns_bool(|dns, value| dns.disable_cache = value)} />
                    <SwitchField label="Serve Stale" checked={d.dns.serve_stale} onchange={update_dns_bool(|dns, value| dns.serve_stale = value)} />
                    <SwitchField label="Disable Fallback" checked={d.dns.disable_fallback} onchange={update_dns_bool(|dns, value| dns.disable_fallback = value)} />
                    <SwitchField label="Disable Fallback If Match" checked={d.dns.disable_fallback_if_match} onchange={update_dns_bool(|dns, value| dns.disable_fallback_if_match = value)} />
                    <SwitchField label="Enable Parallel Query" checked={d.dns.enable_parallel_query} onchange={update_dns_bool(|dns, value| dns.enable_parallel_query = value)} />
                    <SwitchField label="Use System Hosts" checked={d.dns.use_system_hosts} onchange={update_dns_bool(|dns, value| dns.use_system_hosts = value)} />
                </div>

                <div class="space-y-4">
                    <div class="flex justify-between" style="align-items: center; gap: 0.75rem;">
                        <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                            { "DNS servers resolve outbound queries and can carry per-server fallback rules." }
                        </div>
                        <Button
                            label="Add Server"
                            icon={Some("icon-add".to_string())}
                            button_type={ButtonType::Filled}
                            onclick={Callback::from({
                                let editing_dns_server = editing_dns_server.clone();
                                let next_index = d.dns.servers.len();
                                move |_| editing_dns_server.set(Some((next_index, default_dns_server_draft(), true)))
                            })}
                        />
                    </div>
                    {
                        if d.dns.servers.is_empty() {
                            html! {
                                <div class="md3-card bg-surface-container">
                                    <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                                        { "No DNS servers yet." }
                                    </div>
                                </div>
                            }
                        } else {
                            html! {
                                <RichTable columns={vec![
                                    "Server".to_string(),
                                    "Port".to_string(),
                                    "Strategy".to_string(),
                                    "Fallback".to_string(),
                                    "Actions".to_string(),
                                ]} header_in_list={true} card_class={Some("bg-surface-container".to_string())}>
                                    {
                                        for d.dns.servers.iter().map(|server| {
                                            let edit_server = server.clone();
                                            let delete_index = d.dns.servers.iter().position(|item| item == server).unwrap_or(0);
                                            html! {
                                                <>
                                                    <div class="md3-divider"></div>
                                                    <div class="md3-list-row">
                                                        <div class="md3-list-col md3-list-col-main">
                                                            <div class="font-semibold">{ if server.address.trim().is_empty() { "-" } else { server.address.as_str() } }</div>
                                                            <div class="text-sm opacity-70">{ dns_server_summary(server) }</div>
                                                            <div class="text-xs opacity-50">{ dns_server_details(server) }</div>
                                                        </div>
                                                        <div class="md3-list-col">{ server.port }</div>
                                                        <div class="md3-list-col">
                                                            {
                                                                if server.query_strategy.trim().is_empty() {
                                                                    html! { <span class="opacity-60">{ "—" }</span> }
                                                                } else {
                                                                    html! { server.query_strategy.as_str() }
                                                                }
                                                            }
                                                        </div>
                                                        <div class="md3-list-col">
                                                            { option_bool_label(server.disable_cache) }
                                                            { " / " }
                                                            { option_bool_label(server.serve_stale) }
                                                        </div>
                                                        <div class="md3-list-col md3-list-col-actions">
                                                            <div class="md3-list-actions">
                                                                <Button
                                                                    label="Edit"
                                                                    button_type={ButtonType::Outlined}
                                                                    onclick={Callback::from({
                                                                        let editing_dns_server = editing_dns_server.clone();
                                                                        move |_| editing_dns_server.set(Some((delete_index, edit_server.clone(), false)))
                                                                    })}
                                                                />
                                                                <Button
                                                                    label="Delete"
                                                                    button_type={ButtonType::Text}
                                                                    color={Some("#F2B8B5".to_string())}
                                                                    onclick={Callback::from({
                                                                        let draft = draft.clone();
                                                                        move |_| {
                                                                            let mut next = (*draft).clone();
                                                                            sync_draft(&mut next);
                                                                            if delete_index < next.dns.servers.len() {
                                                                                next.dns.servers.remove(delete_index);
                                                                            }
                                                                            draft.set(next);
                                                                        }
                                                                    })}
                                                                />
                                                            </div>
                                                        </div>
                                                    </div>
                                                </>
                                            }
                                        })
                                    }
                                </RichTable>
                            }
                        }
                    }

                    <div class="flex justify-between" style="align-items: center; gap: 0.75rem;">
                        <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                            { "DNS hosts map domains to fixed answers before upstream lookup." }
                        </div>
                        <Button
                            label="Add Host"
                            icon={Some("icon-add".to_string())}
                            button_type={ButtonType::Filled}
                            onclick={Callback::from({
                                let editing_dns_host = editing_dns_host.clone();
                                let next_index = d.dns.hosts.len();
                                move |_| editing_dns_host.set(Some((next_index, DnsHostDraft::default(), true)))
                            })}
                        />
                    </div>
                    {
                        if d.dns.hosts.is_empty() {
                            html! {
                                <div class="md3-card bg-surface-container">
                                    <div class="text-sm" style="color: var(--md-sys-color-on-surface-variant);">
                                        { "No DNS hosts yet." }
                                    </div>
                                </div>
                            }
                        } else {
                            html! {
                                <RichTable columns={vec![
                                    "Domain".to_string(),
                                    "Values".to_string(),
                                    "Actions".to_string(),
                                ]} header_in_list={true} card_class={Some("bg-surface-container".to_string())}>
                                    {
                                        for d.dns.hosts.iter().map(|host| {
                                            let edit_host = host.clone();
                                            let delete_index = d.dns.hosts.iter().position(|item| item == host).unwrap_or(0);
                                            html! {
                                                <>
                                                    <div class="md3-divider"></div>
                                                    <div class="md3-list-row">
                                                        <div class="md3-list-col md3-list-col-main">
                                                            <div class="font-semibold">{ if host.domain.trim().is_empty() { "-" } else { host.domain.as_str() } }</div>
                                                            <div class="text-sm opacity-70">{ dns_host_summary(host) }</div>
                                                        </div>
                                                        <div class="md3-list-col">
                                                            { if host.values.trim().is_empty() { html! { <span class="opacity-60">{ "—" }</span> } } else { html! { host.values.as_str() } } }
                                                        </div>
                                                        <div class="md3-list-col md3-list-col-actions">
                                                            <div class="md3-list-actions">
                                                                <Button
                                                                    label="Edit"
                                                                    button_type={ButtonType::Outlined}
                                                                    onclick={Callback::from({
                                                                        let editing_dns_host = editing_dns_host.clone();
                                                                        move |_| editing_dns_host.set(Some((delete_index, edit_host.clone(), false)))
                                                                    })}
                                                                />
                                                                <Button
                                                                    label="Delete"
                                                                    button_type={ButtonType::Text}
                                                                    color={Some("#F2B8B5".to_string())}
                                                                    onclick={Callback::from({
                                                                        let draft = draft.clone();
                                                                        move |_| {
                                                                            let mut next = (*draft).clone();
                                                                            sync_draft(&mut next);
                                                                            if delete_index < next.dns.hosts.len() {
                                                                                next.dns.hosts.remove(delete_index);
                                                                            }
                                                                            draft.set(next);
                                                                        }
                                                                    })}
                                                                />
                                                            </div>
                                                        </div>
                                                    </div>
                                                </>
                                            }
                                        })
                                    }
                                </RichTable>
                            }
                        }
                    }
                </div>
            </ConfigSection>
        </div>
    }
}
