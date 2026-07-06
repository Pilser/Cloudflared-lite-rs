# cloudflared-lite

[![Crates.io][crates-badge]][crates-url]
[![MIT License][mit-badge]][mit-url]

[crates-badge]: https://img.shields.io/crates/v/cloudflared-lite.svg
[crates-url]: https://crates.io/crates/cloudflared-lite
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://opensource.org/licenses/MIT

**Minimal embeddable Cloudflare Tunnel client in Rust** — no `cloudflared` subprocess, no Go runtime.

- Named tunnels via `TUNNEL_TOKEN` or credentials JSON
- Quick tunnels via `api.trycloudflare.com`
- Pure Rust, single process, small binary
- Library API — call `TunnelManager::start()` from `fn main()`

## Status

| Feature | Status |
|---------|--------|
| HTTP/1.1 proxy (static assets, APIs) | ✅ Tested live |
| WebSocket (wss:// upgrade + bidi) | ✅ Tested live |
| TCP health checks (edge liveness) | ✅ Implemented |
| SSE (Server-Sent Events) | 🔧 Implemented, untested |
| gRPC-Web | 🔧 Implemented, untested |
| gRPC (native) | ❌ Requires HTTP/2 support |
| Quick tunnels | ✅ Implemented |
| HA connections (multiple QUIC) | ✅ Implemented, untested |
| Reconnect with backoff | ✅ Implemented |
| Graceful shutdown | ✅ Implemented |

## Installation

```toml
[dependencies]
cloudflared-lite = "0.1"
```

Requires Rust 1.86+. On Linux, `libwebkit2gtk-4.1` may be needed for the test server's HTTP client.

## Quick Start

```bash
# Named tunnel via token
TUNNEL_TOKEN="eyJh..." TUNNEL_PORT=8080 cargo run -p test-server
# → Tunnel ready: https://xxxx-xxxx.cfargotunnel.com

# Quick tunnel (anonymous)
TUNNEL_QUICK=true TUNNEL_PORT=8080 cargo run -p test-server
# → Tunnel ready: https://xxxx.trycloudflare.com
```

## Public API

```rust
use cloudflared_lite::TunnelManager;

// Named tunnel
let tunnel = TunnelManager::named_with_token(&token)
    .port(8080)
    .ha_connections(2)
    .start().await?;

// Quick tunnel
let tunnel = TunnelManager::quick()
    .port(8080)
    .start().await?;

tunnel.url();              // public hostname
tunnel.shutdown().await;   // graceful
```

## Architecture

```
App ──→ cloudflared-lite ──→ QUIC ──→ Cloudflare Edge ──→ Browser
               │
               └──→ localhost:PORT (your HTTP server)
```

The library has no CLI, no YAML, no service files — just a Rust `struct` you call.

## Deps

`tokio`, `quinn`, `capnp`, `httparse`, `reqwest` — total ~1.3K lines hand-written.

## License

MIT. See [LICENSE](LICENSE).
