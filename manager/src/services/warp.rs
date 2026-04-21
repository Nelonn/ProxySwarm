use base64::Engine;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::services::ApiService;

#[derive(Clone, PartialEq)]
pub struct WarpRegistration {
    pub id: String,
    pub token: String,
    pub private_key: String,
    pub public_key: String,
    pub peer_public_key: String,
    pub license: String,
    pub reserved: Vec<u8>,
    pub addresses: Vec<String>,
    pub endpoint: String,
}

pub fn generate_wireguard_keypair() -> Result<(String, String), String> {
    let mut secret_bytes = [0u8; 32];
    getrandom::getrandom(&mut secret_bytes)
        .map_err(|error| format!("failed to generate keypair: {error}"))?;
    let secret = StaticSecret::from(secret_bytes);
    let public = PublicKey::from(&secret);
    Ok((
        base64::engine::general_purpose::STANDARD.encode(secret.to_bytes()),
        base64::engine::general_purpose::STANDARD.encode(public.as_bytes()),
    ))
}

pub async fn register_warp_with_keypair(
    node_base_url: String,
    master_key: String,
    private_key: String,
    public_key: String,
) -> Result<WarpRegistration, String> {
    let registration = ApiService::new(node_base_url)
        .register_warp(master_key, public_key.clone())
        .await?;
    Ok(WarpRegistration {
        id: registration.id,
        token: registration.token,
        private_key,
        public_key,
        peer_public_key: registration.peer_public_key,
        license: registration.license,
        reserved: registration.reserved,
        addresses: registration.addresses,
        endpoint: registration.endpoint,
    })
}

pub async fn update_warp_license(
    node_base_url: String,
    master_key: String,
    device_id: String,
    access_token: String,
    license: String,
) -> Result<String, String> {
    ApiService::new(node_base_url)
        .update_warp_license(master_key, device_id, access_token, license)
        .await
}
