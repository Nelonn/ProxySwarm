 use super::*;
 
 #[derive(Properties, PartialEq)]
 pub(super) struct InboundEditorPopupProps {
     pub(super) inbound: InboundEntryDraft,
     pub(super) certificates: Vec<CertificateDraft>,
     pub(super) is_new: bool,
     pub(super) on_close: Callback<()>,
     pub(super) on_save: Callback<InboundEntryDraft>,
 }
 
 pub(super) fn inbound_creation_steps(inbound: &InboundEntryDraft) -> usize {
     match inbound.protocol.as_str() {
         "VLESS" => {
             if inbound.vless.security == "REALITY" {
                 4
             } else if inbound.vless.security == "TLS" {
                 4
             } else {
                 3
             }
         }
         "HYSTERIA2" => 4,
         "TRUSTTUNNEL" => 4,
         "NAIVEPROXY" => 4,
         "REVERSEPROXY" => 3,
         "TPROXY" => 3,
         "WIREGUARD" => 3,
         _ => 3,
     }
 }
 
 pub(super) fn outbound_creation_steps(outbound: &OutboundEntryDraft) -> usize {
     match outbound.outbound_type.as_str() {
         "VLESS" => 3,
         "WIREGUARD" => 3,
         "SOCKS5" => 3,
         _ => 3,
     }
 }
 
 #[function_component(InboundEditorPopup)]
 pub(super) fn inbound_editor_popup(props: &InboundEditorPopupProps) -> Html {
     let inbound = use_state(|| props.inbound.clone());
     let step = use_state(|| 0usize);
     let certificate_options = if props.certificates.is_empty() {
         vec![DropdownOp
                             }
                         })}
                     />
                 </ConfigSection>
 
                 <ConfigSection title="TLS / Reality">
                     <TextBox label="Server Name" value={data.tls.server_name.clone()} onchange={update_text(|inbound, value| inbound.tls.server_name = value)} />
                     <Dropdown
                         label="Certificate"
                         value={data.tls.certificate_name.clone()}
                         options={certificate_options.clone()}
                         onchange={update_text(|inbound, value| inbound.tls.certificate_name = value)}
                     />
                     <TextBox label="Dest" value={data.vless.reality_dest.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_dest = value)} />
                     <TextBox label="SNI" value={data.vless.reality_sni.clone()} onchange={update_text(|inbound, value| inbound.vless.reality_sni = value)} />
                     <Dropdown
                         label="uTLS"
                         value={data.vless.reality_utls.clone()}
                         options={vec![
                             DropdownOption { value: "chrome".to_string(), label: "chrome".to_string() },
                             DropdownOption { value: "firefox".to_string(), label: "firefox".to_string() },
                             DropdownOption { value: "safari".to_string(), label: "safari".to_string() },
                             DropdownOption { value: "edge".to_string(), label: "edge".to_string() },
                             DropdownOption { value: "ios".to_string(), label: "ios".to_string() },
                             DropdownO