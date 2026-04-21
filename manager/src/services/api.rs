use gloo_net::http::Request;
use js_sys::Uint8Array;
use prost::Message;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;

use crate::pb::proxyswarm::{
    FullConfig, NodeStatus, StatusRequest, UpdateResponse, WarpRegisterRequest,
    WarpLicenseUpdateRequest, WarpLicenseUpdateResponse, WarpRegisterResponse, WarpRegistration,
};
pub use crate::pb::proxyswarm::{AcmeIssueRequest, AcmeIssueResponse};

pub struct ApiService {
    base_url: String,
}

struct GrpcWebResponse {
    message: Vec<u8>,
    grpc_status: Option<u32>,
    grpc_error: Option<String>,
}

fn frame_grpc_web_message(payload: &[u8]) -> Vec<u8> {
    let mut framed = Vec::with_capacity(payload.len() + 5);
    framed.push(0);
    framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(payload);
    framed
}

fn decode_grpc_message(value: &str) -> String {
    js_sys::decode_uri_component(value)
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| value.to_string())
}

fn parse_grpc_web_response(bytes: &[u8]) -> Result<GrpcWebResponse, String> {
    let mut offset = 0usize;
    let mut message = Vec::new();
    let mut grpc_status = None;
    let mut grpc_error = None;

    while offset + 5 <= bytes.len() {
        let frame_type = bytes[offset];
        let frame_len = u32::from_be_bytes([
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
        ]) as usize;
        offset += 5;

        if offset + frame_len > bytes.len() {
            return Err("Malformed gRPC-Web response frame".to_string());
        }

        let frame = &bytes[offset..offset + frame_len];
        offset += frame_len;

        if frame_type & 0x80 != 0 {
            if let Ok(text) = std::str::from_utf8(frame) {
                for line in text.split("\r\n") {
                    if let Some(value) = line.strip_prefix("grpc-message:") {
                        grpc_error = Some(decode_grpc_message(value.trim()));
                    }
                    if let Some(value) = line.strip_prefix("grpc-status:") {
                        if let Ok(status) = value.trim().parse::<u32>() {
                            grpc_status = Some(status);
                            if status != 0 && grpc_error.is_none() {
                                grpc_error = Some(format!("gRPC status {}", status));
                            }
                        }
                    }
                }
            }
        } else {
            message.extend_from_slice(frame);
        }
    }

    Ok(GrpcWebResponse {
        message,
        grpc_status,
        grpc_error,
    })
}

impl ApiService {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn update_config(&self, mut config: FullConfig) -> Result<UpdateResponse, String> {
        config.master_key = hash_master_key(&config.master_key);
        let encoded = config.encode_to_vec();
        let framed = frame_grpc_web_message(&encoded);
        let url = format!("{}/proxyswarm.NodeService/UpdateConfig", self.base_url);
        let js_body = Uint8Array::from(framed.as_slice());

        let resp = Request::post(&url)
            .header("Content-Type", "application/grpc-web+proto")
            .header("Accept", "application/grpc-web+proto")
            .header("X-Grpc-Web", "1")
            .header("X-User-Agent", "grpc-web-javascript/0.1")
            .body(JsValue::from(js_body))
            .map_err(|e| format!("Failed to create request: {}", e))?;

        let resp = resp
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let bytes = resp
            .binary()
            .await
            .map_err(|e| format!("Failed to get response bytes: {}", e))?;
        let header_error = resp
            .headers()
            .get("grpc-message")
            .map(|value| decode_grpc_message(value.as_str()));
        let parsed = parse_grpc_web_response(&bytes)?;
        let grpc_status = parsed.grpc_status.unwrap_or(0);
        let grpc_error = parsed.grpc_error.or(header_error);

        if !resp.ok() {
            return Err(grpc_error
                .unwrap_or_else(|| format!("Request failed with status: {}", resp.status())));
        }

        if grpc_status != 0 {
            return Err(grpc_error.unwrap_or_else(|| format!("gRPC status {}", grpc_status)));
        }

        let response = UpdateResponse::decode(&parsed.message[..])
            .map_err(|e| format!("Failed to decode response: {}", e))?;

        if !response.success {
            return Err(if response.error.trim().is_empty() {
                grpc_error.unwrap_or_else(|| "Unknown apply error".to_string())
            } else {
                response.error.clone()
            });
        }

        Ok(response)
    }

    pub async fn get_status(&self, master_key: String) -> Result<NodeStatus, String> {
        let request = StatusRequest {
            master_key: hash_master_key(&master_key),
        };
        let encoded = request.encode_to_vec();
        let framed = frame_grpc_web_message(&encoded);
        let url = format!("{}/proxyswarm.NodeService/GetStatus", self.base_url);
        let js_body = Uint8Array::from(framed.as_slice());

        let resp = Request::post(&url)
            .header("Content-Type", "application/grpc-web+proto")
            .header("Accept", "application/grpc-web+proto")
            .header("X-Grpc-Web", "1")
            .header("X-User-Agent", "grpc-web-javascript/0.1")
            .body(JsValue::from(js_body))
            .map_err(|e| format!("Failed to create request: {}", e))?;

        let resp = resp
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        let bytes = resp
            .binary()
            .await
            .map_err(|e| format!("Failed to get response bytes: {}", e))?;
        let header_error = resp
            .headers()
            .get("grpc-message")
            .map(|value| decode_grpc_message(value.as_str()));
        let parsed = parse_grpc_web_response(&bytes)?;
        let grpc_status = parsed.grpc_status.unwrap_or(0);
        let grpc_error = parsed.grpc_error.or(header_error);

        if !resp.ok() {
            return Err(grpc_error
                .unwrap_or_else(|| format!("Request failed with status: {}", resp.status())));
        }

        if grpc_status != 0 {
            return Err(grpc_error.unwrap_or_else(|| format!("gRPC status {}", grpc_status)));
        }

        let status = NodeStatus::decode(&parsed.message[..])
            .map_err(|e| format!("Failed to decode response: {}", e))?;

        Ok(status)
    }

    pub async fn issue_acme_certificate(
        &self,
        mut request: AcmeIssueRequest,
    ) -> Result<AcmeIssueResponse, String> {
        request.master_key = hash_master_key(&request.master_key);
        let encoded = request.encode_to_vec();
        let framed = frame_grpc_web_message(&encoded);
        let url = format!("{}/proxyswarm.NodeService/IssueAcmeCertificate", self.base_url);
        let js_body = Uint8Array::from(framed.as_slice());

        let resp = Request::post(&url)
            .header("Content-Type", "application/grpc-web+proto")
            .header("Accept", "application/grpc-web+proto")
            .header("X-Grpc-Web", "1")
            .header("X-User-Agent", "grpc-web-javascript/0.1")
            .body(JsValue::from(js_body))
            .map_err(|e| format!("Failed to create request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let bytes = resp
            .binary()
            .await
            .map_err(|e| format!("Failed to get response bytes: {}", e))?;
        let header_error = resp
            .headers()
            .get("grpc-message")
            .map(|value| decode_grpc_message(value.as_str()));
        let parsed = parse_grpc_web_response(&bytes)?;
        let grpc_status = parsed.grpc_status.unwrap_or(0);
        let grpc_error = parsed.grpc_error.or(header_error);

        if !resp.ok() {
            return Err(grpc_error
                .unwrap_or_else(|| format!("Request failed with status: {}", resp.status())));
        }

        if grpc_status != 0 {
            return Err(grpc_error.unwrap_or_else(|| format!("gRPC status {}", grpc_status)));
        }

        let response = AcmeIssueResponse::decode(&parsed.message[..])
            .map_err(|e| format!("Failed to decode response: {}", e))?;

        if !response.success {
            return Err(if response.error.trim().is_empty() {
                grpc_error.unwrap_or_else(|| "Unknown ACME issuance error".to_string())
            } else {
                response.error.clone()
            });
        }

        Ok(response)
    }

    pub async fn register_warp(
        &self,
        master_key: String,
        public_key: String,
    ) -> Result<WarpRegistration, String> {
        let request = WarpRegisterRequest {
            master_key: hash_master_key(&master_key),
            public_key,
        };
        let encoded = request.encode_to_vec();
        let framed = frame_grpc_web_message(&encoded);
        let url = format!("{}/proxyswarm.NodeService/RegisterWarp", self.base_url);
        let js_body = Uint8Array::from(framed.as_slice());

        let resp = Request::post(&url)
            .header("Content-Type", "application/grpc-web+proto")
            .header("Accept", "application/grpc-web+proto")
            .header("X-Grpc-Web", "1")
            .header("X-User-Agent", "grpc-web-javascript/0.1")
            .body(JsValue::from(js_body))
            .map_err(|e| format!("Failed to create request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let bytes = resp
            .binary()
            .await
            .map_err(|e| format!("Failed to get response bytes: {}", e))?;
        let header_error = resp
            .headers()
            .get("grpc-message")
            .map(|value| decode_grpc_message(value.as_str()));
        let parsed = parse_grpc_web_response(&bytes)?;
        let grpc_status = parsed.grpc_status.unwrap_or(0);
        let grpc_error = parsed.grpc_error.or(header_error);

        if !resp.ok() {
            return Err(grpc_error
                .unwrap_or_else(|| format!("Request failed with status: {}", resp.status())));
        }

        if grpc_status != 0 {
            return Err(grpc_error.unwrap_or_else(|| format!("gRPC status {}", grpc_status)));
        }

        let response = WarpRegisterResponse::decode(&parsed.message[..])
            .map_err(|e| format!("Failed to decode response: {}", e))?;

        if !response.success {
            return Err(if response.error.trim().is_empty() {
                grpc_error.unwrap_or_else(|| "Unknown WARP registration error".to_string())
            } else {
                response.error.clone()
            });
        }

        response
            .registration
            .ok_or_else(|| "Missing WARP registration payload".to_string())
    }

    pub async fn update_warp_license(
        &self,
        master_key: String,
        device_id: String,
        access_token: String,
        license: String,
    ) -> Result<String, String> {
        let request = WarpLicenseUpdateRequest {
            master_key: hash_master_key(&master_key),
            device_id,
            access_token,
            license,
        };
        let encoded = request.encode_to_vec();
        let framed = frame_grpc_web_message(&encoded);
        let url = format!("{}/proxyswarm.NodeService/UpdateWarpLicense", self.base_url);
        let js_body = Uint8Array::from(framed.as_slice());

        let resp = Request::post(&url)
            .header("Content-Type", "application/grpc-web+proto")
            .header("Accept", "application/grpc-web+proto")
            .header("X-Grpc-Web", "1")
            .header("X-User-Agent", "grpc-web-javascript/0.1")
            .body(JsValue::from(js_body))
            .map_err(|e| format!("Failed to create request: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let bytes = resp
            .binary()
            .await
            .map_err(|e| format!("Failed to get response bytes: {}", e))?;
        let header_error = resp
            .headers()
            .get("grpc-message")
            .map(|value| decode_grpc_message(value.as_str()));
        let parsed = parse_grpc_web_response(&bytes)?;
        let grpc_status = parsed.grpc_status.unwrap_or(0);
        let grpc_error = parsed.grpc_error.or(header_error);

        if !resp.ok() {
            return Err(grpc_error
                .unwrap_or_else(|| format!("Request failed with status: {}", resp.status())));
        }

        if grpc_status != 0 {
            return Err(grpc_error.unwrap_or_else(|| format!("gRPC status {}", grpc_status)));
        }

        let response = WarpLicenseUpdateResponse::decode(&parsed.message[..])
            .map_err(|e| format!("Failed to decode response: {}", e))?;

        if !response.success {
            return Err(if response.error.trim().is_empty() {
                grpc_error.unwrap_or_else(|| "Unknown WARP license update error".to_string())
            } else {
                response.error.clone()
            });
        }

        Ok(response.license)
    }
}

pub fn hash_master_key(master_key: &str) -> String {
    let digest = Sha256::digest(master_key.as_bytes());
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{:02x}", byte));
    }
    output
}
