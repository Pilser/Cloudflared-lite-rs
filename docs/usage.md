# Usage Guide

## Add to Your Project

```toml
[dependencies]
cloudflared-lite = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Named Tunnel (recommended)

Create a Cloudflare Tunnel in the dashboard, then:

```rust
use cloudflared_lite::TunnelManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("TUNNEL_TOKEN")
        .expect("TUNNEL_TOKEN env var");

    let tunnel = TunnelManager::named_with_token(&token)
        .port(3000)               // your local app
        .ha_connections(2)        // parallel QUIC connections
        .start()
        .await?;

    println!("Tunnel ready at: {}", tunnel.url());
    // e.g. https://xxxx-xxxx.cfargotunnel.com

    tokio::signal::ctrl_c().await?;
    tunnel.shutdown().await?;
    Ok(())
}
```

## Quick Tunnel (anonymous — try.cloudflare.com)

No Cloudflare dashboard needed:

```rust
let tunnel = TunnelManager::quick()
    .port(3000)
    .start()
    .await?;

println!("Tunnel ready at: {}", tunnel.url());
// e.g. https://xxxx.trycloudflare.com
```

## Without the test server

The `TunnelManager` is a library — integrate it into any Rust binary.
No CLI, no config files, no subprocess.

## What gets proxied

| Request type | Works? | Mechanism |
|---|---|---|
| HTTP GET/HEAD | ✅ | Content-Length pump |
| HTTP POST/PUT with body | ✅ | Content-Length pump |
| WebSocket | ✅ | Bidi pump (101 response) |
| SSE (text/event-stream) | ✅ | Bidi pump (streaming response) |
| gRPC-Web | ✅ | Bidi pump (streaming request/response) |
| Raw TCP (health checks) | ✅ | Bidi pump |
| HTTP/2 (native gRPC) | ❌ | Not yet supported |

## Environment reference (test-server)

| Variable | Example | Description |
|---|---|---|
| `TUNNEL_TOKEN` | `eyJhI...` | Base64 JSON token from Cloudflare |
| `TUNNEL_PORT` | `8080` | Local origin port |
| `TUNNEL_QUICK` | `true` | Use quick tunnel instead of token |
| `TUNNEL_HA_CONNECTIONS` | `2` | QUIC connections (default 1) |
