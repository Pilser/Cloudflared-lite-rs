use crate::config::TunnelConfig;
use crate::error::{Result, TunnelError};
use base64::Engine as _;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct TokenPayload {
    #[serde(rename = "a")]
    account_tag: String,
    #[serde(rename = "s")]
    tunnel_secret: String,
    #[serde(rename = "t")]
    tunnel_id: String,
}

fn tunnel_id_to_hex(bytes: &[u8]) -> String {
    let uuid = Uuid::from_slice(bytes).unwrap_or_else(|_| {
        let mut buf = [0u8; 16];
        let copy_len = bytes.len().min(16);
        buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
        Uuid::from_bytes(buf)
    });
    uuid.to_string()
}

pub fn decode_token(token: &str) -> Result<TunnelConfig> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(token)
        .map_err(|e| TunnelError::TokenDecode(format!("invalid base64: {e}")))?;

    let payload: TokenPayload = serde_json::from_slice(&decoded)
        .map_err(|e| TunnelError::TokenDecode(format!("invalid JSON: {e}")))?;

    let tunnel_secret = base64::engine::general_purpose::STANDARD
        .decode(&payload.tunnel_secret)
        .map_err(|e| TunnelError::TokenDecode(format!("invalid secret base64: {e}")))?;

    let tunnel_id = Uuid::parse_str(&payload.tunnel_id)
        .map_err(|e| TunnelError::TokenDecode(format!("invalid tunnel id UUID: {e}")))?;
    let tunnel_id_bytes = tunnel_id.as_bytes().to_vec();

    let hostname = format!("{}.cfargotunnel.com", tunnel_id_to_hex(&tunnel_id_bytes));

    Ok(TunnelConfig {
        account_tag: payload.account_tag,
        tunnel_secret,
        tunnel_id: tunnel_id_bytes,
        hostname,
    })
}

pub(super) fn decode_quick(body: &str) -> Result<TunnelConfig> {
    #[derive(serde::Deserialize)]
    struct QuickResponse {
        id: String,
        hostname: String,
        account_tag: String,
        secret: String,
    }

    let resp: QuickResponse = serde_json::from_str(body)
        .map_err(|e| TunnelError::Internal(format!("invalid quick tunnel response: {e}")))?;

    let secret = base64::engine::general_purpose::STANDARD
        .decode(&resp.secret)
        .map_err(|e| TunnelError::Internal(format!("invalid secret base64: {e}")))?;

    let tunnel_id = Uuid::parse_str(&resp.id)
        .map_err(|e| TunnelError::Internal(format!("invalid tunnel id: {e}")))?;

    Ok(TunnelConfig {
        account_tag: resp.account_tag,
        tunnel_secret: secret,
        tunnel_id: tunnel_id.as_bytes().to_vec(),
        hostname: resp.hostname,
    })
}
