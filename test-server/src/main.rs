use cloudflared_lite::TunnelManager;
use std::env;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("cloudflared_lite=debug".parse().unwrap())
            .add_directive("test_server=info".parse().unwrap()))
        .init();

    dotenvy::dotenv().ok();

    let port: u16 = env::var("TUNNEL_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid TUNNEL_PORT: {e}"))?;

    let ha: u32 = env::var("TUNNEL_HA_CONNECTIONS")
        .unwrap_or_else(|_| "1".into())
        .parse()
        .unwrap_or(1);

    let is_quick = env::var("TUNNEL_QUICK").is_ok();

    let mut tunnel = if is_quick {
        tracing::info!("Starting quick tunnel → localhost:{port}");
        TunnelManager::quick()
            .port(port)
            .ha_connections(ha)
            .start()
            .await?
    } else if let Ok(token) = env::var("TUNNEL_TOKEN") {
        tracing::info!("Starting named tunnel with token → localhost:{port}");
        TunnelManager::named_with_token(&token)
            .port(port)
            .ha_connections(ha)
            .start()
            .await?
    } else if let Ok(path) = env::var("TUNNEL_CREDENTIALS") {
        tracing::info!("Starting named tunnel with credentials file → localhost:{port}");
        TunnelManager::named_with_file(&path)
            .port(port)
            .ha_connections(ha)
            .start()
            .await?
    } else {
        anyhow::bail!(
            "Set TUNNEL_TOKEN, TUNNEL_CREDENTIALS, or TUNNEL_QUICK=true in .env"
        );
    };

    tracing::info!("Tunnel ready: {}", tunnel.url());
    tracing::info!("Press Ctrl+C to shutdown");

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");
    tunnel.shutdown().await;

    Ok(())
}
