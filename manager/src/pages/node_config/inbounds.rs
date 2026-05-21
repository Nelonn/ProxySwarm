use super::*;

pub(super) fn render_inbounds_tab(
    draft: &UseStateHandle<NodeConfigDraft>,
    inbounds: &[InboundEntryDraft],
    editing_inbound: &UseStateHandle<Option<(InboundEntryDraft, bool)>>,
    access_link_inbound_id: &UseStateHandle<Option<String>>,
    action_inbound: &UseStateHandle<Option<(String, (f64, f64, f64))>>,
) -> Html {
    html! {                        <div class="space-y-6">
                            <div class="flex justify-between" style="align-items: center;">
                                <div>
                                    <h2 class="text-2xl font-bold">{ "Inbounds" }</h2>
                                    <div class="text-sm opacity-70">{ "Ports, protocols, traffic mode, and access-link generation." }</div>
                                </div>
                                <Button
                                    label="Add Inbound"
                                    icon={Some("icon-add".to_string())}
                                    button_type={ButtonType::Filled}
                                    onclick={Callback::from({
                                    let editing_inbound = editing_inbound.clone();
                                    move |_| editing_inbound.set(Some((default_inbound_entry(), true)))
                                })}
                                />
                            </div>
                            <RichTable columns={vec![
                                "Name".to_string(),
                                "Port".to_string(),
                                "Protocol".to_string(),
                                "Enabled".to_string(),
                                "Traffic".to_string(),
                                "Actions".to_string(),
                            ]} card_class={Some("bg-surface-container".to_string())} header_in_list={true}>
                                {
                                    for inbounds.iter().map(|inbound| {
                                        let link_id = inbound.id.clone();
                                        let action_id = inbound.id.clone();
                                        html! {
                                            <>
                                                <div class="md3-divider"></div>
                                                <div class="md3-list-row">
                                                    <div class="md3-list-col-main">
                                                        <div class="font-semibold">{ inbound.name.clone() }</div>
                                                        <div class="text-sm opacity-70">{ inbound_groups_label(inbound) }</div>
                                                    </div>
                                                    <div class="md3-list-col">{ inbound.port }</div>
                                                    <div class="md3-list-col">{ inbound.protocol.clone() }</div>
                                                    <div class="md3-list-col">
                                                        <Switch
                                                            checked={inbound.enabled}
                                                            onchange={Callback::from({
                                                                let draft = draft.clone();
                                                                let toggle_id = inbound.id.clone();
                                                                move |e: Event| {
                                                                    let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
                                                                    let mut next = (*draft).clone();
                                                                    sync_draft(&mut next);
                                                                    if let Some(item) = next.inbounds.iter_mut().find(|item| item.id == toggle_id) {
                                                                        item.enabled = input.checked();
                                                                    }
                                                                    sync_draft(&mut next);
                                                                    draft.set(next);
                                                                }
                                                            })}
                                                        />
                                                    </div>
                                                    <div class="md3-list-col">{ inbound_traffic_label(inbound) }</div>
                                                    <div class="md3-list-col-actions">
                                                        <div class="md3-list-actions">
                                                            <Button label="Access Link" button_type={ButtonType::Tonal} onclick={Callback::from({
                                                                let access_link_inbound_id = access_link_inbound_id.clone();
                                                                move |_| access_link_inbound_id.set(Some(link_id.clone()))
                                                            })} />
                                                            <Button label="Action" button_type={ButtonType::Outlined} onclick={Callback::from({
                                                                let action_inbound = action_inbound.clone();
                                                                move |e: MouseEvent| {
                                                                    if let Some((left, top, width)) = menu_anchor_from_mouse_event(&e) {
                                                                        action_inbound.set(Some((
                                                                            action_id.clone(),
                                                                            (left, top, width),
                                                                        )));
                                                                    }
                                                                }
                                                            })} />
                                                        </div>
                                                    </div>
                                                </div>
                                            </>
                                        }
                                    })
                                }
                            </RichTable>
                        </div>
    }
}

