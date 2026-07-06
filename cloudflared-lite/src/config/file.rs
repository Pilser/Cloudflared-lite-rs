use crate::config::TunnelConfig;
use crate::error::{Result, TunnelError};
use base64::Engine;

#[derive(serde::Deserialize)]
struct CredentialsFile {
    #[serde(rename = "AccountTag")]
    account_tag: String,
    #[serde(rename = "TunnelSecret")]
    tunnel_secret: String,
    #[serde(rename = "TunnelID")]
    tunnel_id: String,
}

pub fn load_credentials_file(path: &str) -> Result<TunnelConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| TunnelError::Config(format!("cannot read {path}: {e}")))?;

    let creds: CredentialsFile = serde_json::from_str(&content)
        .map_err(|e| TunnelError::Config(format!("invalid JSON: {e}")))?;

    let tunnel_secret = base64::engine::general_purpose::STANDARD
        .decode(&creds.tunnel_secret)
        .map_err(|e| TunnelError::Config(format!("invalid secret base64: {e}")))?;

    let tunnel_id = base64::engine::general_purpose::STANDARD
        .decode(&creds.tunnel_id)
        .map_err(|e| TunnelError::Config(format!("invalid id base64: {e}")))?;

    Ok(TunnelConfig {
        account_tag: creds.account_tag,
        tunnel_secret,
        tunnel_id,
        hostname: String::new(),
    })
}
