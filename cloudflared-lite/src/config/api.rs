use crate::config::token::decode_quick;
use crate::config::TunnelConfig;
use crate::error::Result;

pub async fn register_quick_tunnel() -> Result<TunnelConfig> {
    let client = reqwest::Client::builder()
        .user_agent("cloudflared-lite")
        .build()?;

    let resp = client
        .post("https://api.trycloudflare.com/tunnel")
        .send()
        .await?;

    let body = resp.text().await?;
    decode_quick(&body)
}
