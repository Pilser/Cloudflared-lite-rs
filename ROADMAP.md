# Roadmap

## Legend
- ✅ **Done & tested live** — works against real Cloudflare edge
- 🔧 **Done, untested** — code exists, needs live verification
- 📝 **Simple addition** — small code change, < 1 hour
- ❌ **Not implemented** — requires significant work

---

## Core Tunnel

| Feature | Status | Notes |
|---------|--------|-------|
| QUIC handshake (quinn + rustls, ALPN `argotunnel`) | ✅ | Tested live |
| DNS discovery (SRV → edge IPs) | ✅ | System resolver |
| `registerConnection` RPC | ✅ | Cap'n Proto on its own tokio runtime |
| Accept loop (`accept_bi`) | ✅ | Spawns per-stream tasks |
| Reconnect with exponential backoff + jitter | ✅ | Tested live |
| HA connections (multiple parallel QUIC) | ✅ | Default 1, configurable |
| Graceful shutdown | ✅ | `shutdown()` → Notify → `accept_loop` returns Shutdown → `session.shutdown_graceful()` → `unregisterConnection` RPC |
| `unregisterConnection` RPC on shutdown | ✅ | Sent by RPC driver thread on Graceful command |

## Config

| Feature | Status | Notes |
|---------|--------|-------|
| `TUNNEL_TOKEN` decode (base64 JSON) | ✅ | `{a, t, s}` → account/tunnel/secret |
| Credentials JSON file | 🔧 | Implemented, needs testing |
| Quick tunnel API (POST to trycloudflare.com) | 🔧 | Implemented via `reqwest`, needs live test |

## HTTP Proxy

| Feature | Status | Notes |
|---------|--------|-------|
| GET/HEAD requests (no body) | ✅ | Tested live |
| POST/PUT with Content-Length body | ✅ | Exact byte pumping |
| Response with Content-Length | ✅ | Exact byte pumping, `to_edge.close()` |
| Response without Content-Length (chunked / close) | ✅ | Pump to TCP EOF |
| Header reconstruction from edge metadata | ✅ | `HttpHeader:*` → raw HTTP |
| Host header from `HttpHost` metadata | ✅ | |
| Request path from `dest` URL | ✅ | Handles `http://host/path`, `/path`, bare |
| `Connection: close` for normal HTTP | ✅ | |
| `to_edge.close()` after response | ✅ | Signals QUIC stream completion |

## WebSocket

| Feature | Status | Notes |
|---------|--------|-------|
| `ConnectionType::WebSocket` detection | ✅ | Maps to `is_websocket=true` |
| `Sec-Websocket-Key` header detection | ✅ | |
| `Connection: Upgrade` + `Upgrade: websocket` reconstruction | ✅ | Edge strips these from metadata |
| 101 response handling | ✅ | Bidi pump with `tokio::join!` |
| Bidirectional frame forwarding | ✅ | `edge_to_local` + `local_to_edge` |
| WebSocket close/error handling | ✅ | Errors are benign (peer disconnect) |

## SSE (Server-Sent Events) — `text/event-stream`

| Feature | Status | Notes |
|---------|--------|-------|
| Detection via `content-type: text/event-stream` | 🔧 | Implemented — checked alongside status==101 |
| Bidi pump for persistent stream | 🔧 | Same bidi path as WebSocket |
| Keepalive / reconnect handling | ❌ | SSE reconnection is client-side, no proxy work needed |

## gRPC

| Feature | Status | Notes |
|---------|--------|-------|
| HTTP/2 cleartext (h2c) upgrade | ❌ | Our proxy speaks raw HTTP/1.1 |
| HTTP/2 TLS (h2) ALPN | ❌ | Origin connection uses TCP, not TLS |
| gRPC-web proxy | 🔧 | Streaming detection + bidi pump via `application/grpc-web*` Content-Type |

**gRPC-Web** works as HTTP/1.1 with gRPC-Web framing — our proxy forward bytes as-is. Status 200 + streaming body detected via `content-type: application/grpc-web*`. Streaming request body (client streaming / bidi streaming) forwarded from edge until EOF. Unary gRPC-Web works via existing Content-Length path.

Full native gRPC (HTTP/2) is not implemented — would require `h2` crate + ALPN negotiation on the origin connection side.

## TCP / Other Transports

| Feature | Status | Notes |
|---------|--------|-------|
| `ConnectionType::Tcp` (health checks) | 🔧 | Bidi pump with `join!`, successfully acknowledged edge |
| Long-lived raw TCP streams | 🔧 | Bidi pump should work, needs live test with real TCP service |
| QUIC connection migration | ❌ | Not supported by quinn |

## Performance & Reliability

| Feature | Status | Notes |
|---------|--------|-------|
| TCP connection pool (keep-alive reuse) | 🔧 | `Pool` struct exists, not wired into proxy |
| Stream counters / metrics | 🔧 | Behind `metrics` feature flag |
| Origin connection timeout | 📝 | Simple: add `tokio::time::timeout` on `TcpStream::connect` |
| Per-stream timeout (idle) | 📝 | Add timeout on `tcp_read.read()` |

---

## Summary of Simple Additions (< 1 hour each)

1. **Wire connection pool** — `Pool` struct exists, just needs integration in proxy dispatch
2. **Origin connect timeout** — wrap `TcpStream::connect` with `tokio::time::timeout`
3. **Idle per-stream timeout** — wrap `tcp_read.read()` with a timeout

Want me to implement any of these? Just say which ones.
