use super::*;

#[derive(Properties, PartialEq)]
pub(super) struct CertificateEditorPopupProps {
    pub(super) certificate: CertificateDraft,
    pub(super) is_new: bool,
    pub(super) on_close: Callback<()>,
    pub(super) on_save: Callback<CertificateDraft>,
}

#[function_component(CertificateEditorPopup)]
pub(super) fn certificate_editor_popup(props: &CertificateEditorPopupProps) -> Html {
    let certificate = use_state(|| props.certificate.clone());

    let update_text = |mutator: fn(&mut CertificateDraft, String)| {
        let certificate = certificate.clone();
        Callback::from(move |value: String| {
            let mut next = (*certificate).clone();
            mutator(&mut next, value);
            certificate.set(next);
        })
    };

    let data = (*certificate).clone();
    let popup_title = if props.is_new {
        "Add Certificate"
    } else {
        "Edit Certificate"
    };

    let read_clipboard_into = |mutator: fn(&mut CertificateDraft, String)| {
        let certificate = certificate.clone();
        Callback::from(move |_| {
            let certificate = certificate.clone();
            spawn_local(async move {
                let Some(window) = window() else {
                    return;
                };
                let navigator = window.navigator();
                let Ok(clipboard) =
                    js_sys::Reflect::get(&navigator, &JsValue::from_str("clipboard"))
                else {
                    return;
                };
                let Ok(read_text) =
                    js_sys::Reflect::get(&clipboard, &JsValue::from_str("readText"))
                else {
                    return;
                };
                let Ok(function) = read_text.dyn_into::<js_sys::Function>() else {
                    return;
                };
                let Ok(promise_value) = function.call0(&clipboard) else {
                    return;
                };
                let promise = js_sys::Promise::from(promise_value);
                let Ok(value) = JsFuture::from(promise).await else {
                    return;
                };
                if let Some(text) = value.as_string() {
                    let mut next = (*certificate).clone();
                    mutator(&mut next, text);
                    certificate.set(next);
                }
            });
        })
    };

    let import_file_into = |mutator: fn(&mut CertificateDraft, String)| {
        let certificate = certificate.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            let Some(files) = input.files() else {
                return;
            };
            let Some(file) = files.get(0) else {
                return;
            };
            let certificate = certificate.clone();
            spawn_local(async move {
                let promise = file.text();
                let Ok(value) = JsFuture::from(promise).await else {
                    return;
                };
                if let Some(text) = value.as_string() {
                    let mut next = (*certificate).clone();
                    mutator(&mut next, text);
                    certificate.set(next);
                }
            });
        })
    };

    html! {
        <Popup title={popup_title} size={PopupSize::Md} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <TextBox label="Name" value={data.name.clone()} onchange={update_text(|certificate, value| certificate.name = value)} />
                <Dropdown
                    label="Type"
                    value={data.cert_type.clone()}
                    options={vec![
                        DropdownOption { value: "CUSTOM".to_string(), label: "Custom".to_string() },
                        DropdownOption { value: "ACME".to_string(), label: "ACME".to_string() },
                    ]}
                    onchange={update_text(|certificate, value| certificate.cert_type = value)}
                />
                {
                    if data.cert_type == "ACME" {
                        html! {
                            <>
                                <Dropdown
                                    label="ACME Type"
                                    value={data.acme_type.clone()}
                                    options={vec![
                                        DropdownOption { value: "HTTP".to_string(), label: "HTTP".to_string() },
                                        DropdownOption { value: "TLS".to_string(), label: "TLS".to_string() },
                                        DropdownOption { value: "DNS".to_string(), label: "DNS".to_string() },
                                    ]}
                                    onchange={update_text(|certificate, value| certificate.acme_type = value)}
                                />
                                <Dropdown
                                    label="CA"
                                    value={data.acme_ca.clone()}
                                    options={vec![
                                        DropdownOption { value: "letsencrypt".to_string(), label: "Let's Encrypt".to_string() },
                                        DropdownOption { value: "zerossl".to_string(), label: "ZeroSSL".to_string() },
                                        DropdownOption { value: "google".to_string(), label: "Google Trust Services".to_string() },
                                        DropdownOption { value: "buypass".to_string(), label: "Buypass Go SSL".to_string() },
                                        DropdownOption { value: "sslcom".to_string(), label: "SSL.com".to_string() },
                                    ]}
                                    onchange={update_text(|certificate, value| certificate.acme_ca = value)}
                                />
                                <TextBox label="Email" value={data.acme_email.clone()} onchange={update_text(|certificate, value| certificate.acme_email = value)} />
                                <TextBox label="Domain" value={data.acme_domain.clone()} onchange={update_text(|certificate, value| certificate.acme_domain = value)} />
                                {
                                    match data.acme_type.as_str() {
                                        "HTTP" => html! {
                                            <TextBox
                                                label="Port"
                                                value={data.acme_http_port.to_string()}
                                                onchange={update_text(|certificate, value| certificate.acme_http_port = value.parse().unwrap_or(0))}
                                                input_type="number"
                                            />
                                        },
                                        "TLS" => html! {
                                            <TextBox
                                                label="Port"
                                                value={data.acme_port.to_string()}
                                                onchange={update_text(|certificate, value| certificate.acme_port = value.parse().unwrap_or(0))}
                                                input_type="number"
                                            />
                                        },
                                        _ => html! {},
                                    }
                                }
                            </>
                        }
                    } else {
                        html! {
                            <>
                                <div class="space-y-2">
                                    <TextBox label="Certificate PEM" value={data.certificate_pem.clone()} onchange={update_text(|certificate, value| certificate.certificate_pem = value)} is_textarea={true} />
                                    <div class="flex" style="gap: 0.75rem;">
                                        <Button label="Paste Certificate" button_type={ButtonType::Outlined} onclick={read_clipboard_into(|certificate, value| certificate.certificate_pem = value)} />
                                        <label class="md3-btn md3-btn--outlined" style="cursor: pointer;">
                                            { "Import Certificate File" }
                                            <input type="file" accept=".pem,.crt,.cer,.txt" style="display: none;" onchange={import_file_into(|certificate, value| certificate.certificate_pem = value)} />
                                        </label>
                                    </div>
                                </div>
                                <div class="space-y-2">
                                    <TextBox label="Key PEM" value={data.key_pem.clone()} onchange={update_text(|certificate, value| certificate.key_pem = value)} is_textarea={true} />
                                    <div class="flex" style="gap: 0.75rem;">
                                        <Button label="Paste Key" button_type={ButtonType::Outlined} onclick={read_clipboard_into(|certificate, value| certificate.key_pem = value)} />
                                        <label class="md3-btn md3-btn--outlined" style="cursor: pointer;">
                                            { "Import Key File" }
                                            <input type="file" accept=".pem,.key,.txt" style="display: none;" onchange={import_file_into(|certificate, value| certificate.key_pem = value)} />
                                        </label>
                                    </div>
                                </div>
                            </>
                        }
                    }
                }
            </div>
            <div class="md3-popup-actions" style="justify-content: flex-end;">
                <Button label="Cancel" button_type={ButtonType::Text} onclick={Callback::from({
                    let on_close = props.on_close.clone();
                    move |_| on_close.emit(())
                })} />
                <Button label={if props.is_new { "Create Certificate" } else { "Apply Changes" }} button_type={ButtonType::Filled} onclick={Callback::from({
                    let on_save = props.on_save.clone();
                    let certificate = certificate.clone();
                    move |_| on_save.emit((*certificate).clone())
                })} />
            </div>
        </Popup>
    }
}
