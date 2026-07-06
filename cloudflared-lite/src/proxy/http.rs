use crate::error::{Result, TunnelError};
use crate::proxy::handshake::{read_connect_request, write_connect_response, ConnectionType};
use crate::proxy::Pool;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use quinn::{RecvStream, SendStream};
use std::sync::Arc;
use tokio::io::{AsyncReadExt as TokioRead, AsyncWriteExt as TokioWrite};
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

pub async fn proxy_http(
    local_port: u16,
    mut send: SendStream,
    mut recv: RecvStream,
    pool: Arc<Pool>,
    shutdown: Arc<Notify>,
) {
    tokio::select! {
        biased;
        _ = shutdown.notified() => {}
        result = proxy_dispatch(local_port, &mut send, &mut recv, pool) => {
            let _ = result;
        }
    }
}

async fn proxy_dispatch(
    local_port: u16,
    send: &mut SendStream,
    recv: &mut RecvStream,
    _pool: Arc<Pool>,
) -> Result<()> {
    let mut compat_recv = recv.compat();
    let req = read_connect_request(&mut compat_recv).await?;
    drop(compat_recv);

    match req.conn_type {
        ConnectionType::Http | ConnectionType::WebSocket => {
            let is_ws = req.conn_type == ConnectionType::WebSocket;
            let reader = recv.compat();
            let writer = send.compat_write();
            proxy_http_inner(local_port, req, is_ws, reader, writer).await
        }
        ConnectionType::Tcp => {
            let reader = recv.compat();
            let writer = send.compat_write();
            proxy_tcp(local_port, req, reader, writer).await
        }
    }
}

async fn proxy_tcp<R, W>(
    local_port: u16,
    _req: crate::proxy::handshake::ConnectRequest,
    mut from_edge: R,
    mut to_edge: W,
) -> Result<()>
where
    R: futures::io::AsyncRead + Unpin,
    W: futures::io::AsyncWrite + Unpin,
{
    let tcp = TcpStream::connect(("127.0.0.1", local_port))
        .await
        .map_err(|e| TunnelError::Internal(format!("tcp connect: {e}")))?;
    let (mut tcp_read, mut tcp_write) = tcp.into_split();

    write_connect_response(&mut to_edge, "", &[]).await?;

    let edge_to_local = async {
        let mut buf = [0u8; 16384];
        loop {
            let n = from_edge.read(&mut buf).await
                .map_err(|e| TunnelError::Internal(format!("edge read: {e}")))?;
            if n == 0 { break; }
            tcp_write.write_all(&buf[..n]).await
                .map_err(|e| TunnelError::Internal(format!("tcp write: {e}")))?;
        }
        Ok::<(), TunnelError>(())
    };

    let local_to_edge = async {
        let mut buf = [0u8; 16384];
        loop {
            let n = tcp_read.read(&mut buf).await
                .map_err(|e| TunnelError::Internal(format!("tcp read: {e}")))?;
            if n == 0 { break; }
            to_edge.write_all(&buf[..n]).await
                .map_err(|e| TunnelError::Internal(format!("edge write: {e}")))?;
        }
        Ok::<(), TunnelError>(())
    };

    let _ = tokio::join!(edge_to_local, local_to_edge);
    let _ = to_edge.close().await;
    Ok(())
}

async fn proxy_http_inner<R, W>(
    local_port: u16,
    req: crate::proxy::handshake::ConnectRequest,
    is_websocket_type: bool,
    mut from_edge: R,
    mut to_edge: W,
) -> Result<()>
where
    R: futures::io::AsyncRead + Unpin,
    W: futures::io::AsyncWrite + Unpin,
{
    let method = req
        .headers
        .iter()
        .find(|(k, _)| k == "HttpMethod")
        .map(|(_, v)| v.as_str())
        .unwrap_or("GET");

    let host = req
        .headers
        .iter()
        .find(|(k, _)| k == "HttpHost")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");

    let path = extract_path(&req.dest);
    let is_websocket = is_websocket_type
        || req.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("HttpHeader:Sec-Websocket-Key"));

    let mut head = format!("{method} {path} HTTP/1.1\r\n");
    if !host.is_empty() {
        head.push_str(&format!("Host: {host}\r\n"));
    }

    let mut saw_connection = false;
    for (k, v) in &req.headers {
        if let Some(name) = k.strip_prefix("HttpHeader:") {
            if name.eq_ignore_ascii_case("host") { continue; }
            if name.eq_ignore_ascii_case("connection") { saw_connection = true; }
            head.push_str(&format!("{name}: {v}\r\n"));
        }
    }
    if !saw_connection {
        if is_websocket {
            head.push_str("Connection: Upgrade\r\n");
            head.push_str("Upgrade: websocket\r\n");
        } else {
            head.push_str("Connection: close\r\n");
        }
    }
    head.push_str("\r\n");

    let tcp = TcpStream::connect(("127.0.0.1", local_port))
        .await
        .map_err(|e| TunnelError::Internal(format!("tcp connect: {e}")))?;
    let (mut tcp_read, mut tcp_write) = tcp.into_split();

    tcp_write.write_all(head.as_bytes()).await
        .map_err(|e| TunnelError::Internal(format!("tcp write head: {e}")))?;

    let is_grpc_web = req.headers.iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("HttpHeader:Content-Type")
            && (v.starts_with("application/grpc-web")
                || v.starts_with("application/grpc")));
    let req_body_len: u64 = req.headers.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("HttpHeader:Content-Length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);

    if req_body_len > 0 {
        let mut remaining = req_body_len;
        let mut buf = [0u8; 16384];
        while remaining > 0 {
            let n = from_edge.read(&mut buf).await
                .map_err(|e| TunnelError::Internal(format!("edge read body: {e}")))?;
            if n == 0 { break; }
            let to_write = (n as u64).min(remaining);
            tcp_write.write_all(&buf[..to_write as usize]).await
                .map_err(|e| TunnelError::Internal(format!("tcp write body: {e}")))?;
            remaining -= to_write;
        }
    } else if is_grpc_web {
        let mut buf = [0u8; 16384];
        loop {
            let n = from_edge.read(&mut buf).await
                .map_err(|e| TunnelError::Internal(format!("edge read body: {e}")))?;
            if n == 0 { break; }
            tcp_write.write_all(&buf[..n]).await
                .map_err(|e| TunnelError::Internal(format!("tcp write body: {e}")))?;
        }
        let _ = tcp_write.shutdown().await;
    }

    let (status, headers, leftover) = read_http_response_head(&mut tcp_read).await?;

    let resp_body_len: Option<u64> = headers.iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok());

    let mut meta: Vec<(String, String)> = Vec::with_capacity(headers.len() + 1);
    meta.push(("HttpStatus".to_string(), status.to_string()));
    for (name, value) in &headers {
        meta.push((format!("HttpHeader:{name}"), value.clone()));
    }
    let meta_refs: Vec<(&str, &str)> = meta.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    write_connect_response(&mut to_edge, "", &meta_refs).await?;

    if !leftover.is_empty() {
        to_edge.write_all(&leftover).await
            .map_err(|e| TunnelError::Internal(format!("write leftover: {e}")))?;
    }

    let content_type = headers.iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str());
    let is_streaming = status == 101
        || content_type.map_or(false, |ct| ct.starts_with("text/event-stream"))
        || content_type.map_or(false, |ct| ct.starts_with("application/grpc-web")
            || ct.starts_with("application/grpc"));

    if is_streaming {
        let edge_to_local = async {
            let mut buf = [0u8; 16384];
            loop {
                let n = from_edge.read(&mut buf).await
                    .map_err(|e| TunnelError::Internal(format!("edge read: {e}")))?;
                if n == 0 { break; }
                tcp_write.write_all(&buf[..n]).await
                    .map_err(|e| TunnelError::Internal(format!("tcp write: {e}")))?;
            }
            let _ = tcp_write.shutdown().await;
            Ok::<(), TunnelError>(())
        };
        let local_to_edge = async {
            let mut buf = [0u8; 16384];
            loop {
                let n = tcp_read.read(&mut buf).await
                    .map_err(|e| TunnelError::Internal(format!("tcp read: {e}")))?;
                if n == 0 { break; }
                to_edge.write_all(&buf[..n]).await
                    .map_err(|e| TunnelError::Internal(format!("edge write: {e}")))?;
            }
            Ok::<(), TunnelError>(())
        };
        let _ = tokio::join!(edge_to_local, local_to_edge);
    } else if let Some(total) = resp_body_len {
        // Normal HTTP with Content-Length — pump exact bytes, then close
        let mut remaining = total.saturating_sub(leftover.len() as u64);
        let mut buf = [0u8; 16384];
        while remaining > 0 {
            let n = tcp_read.read(&mut buf).await
                .map_err(|e| TunnelError::Internal(format!("tcp read: {e}")))?;
            if n == 0 { break; }
            let to_write = (n as u64).min(remaining);
            to_edge.write_all(&buf[..to_write as usize]).await
                .map_err(|e| TunnelError::Internal(format!("edge write: {e}")))?;
            remaining -= to_write;
        }
    } else {
        // No Content-Length (chunked / close) — signal done sending, pump to EOF
        let _ = tcp_write.shutdown().await;
        let mut buf = [0u8; 16384];
        loop {
            let n = tcp_read.read(&mut buf).await
                .map_err(|e| TunnelError::Internal(format!("tcp read: {e}")))?;
            if n == 0 { break; }
            to_edge.write_all(&buf[..n]).await
                .map_err(|e| TunnelError::Internal(format!("edge write: {e}")))?;
        }
    }

    let _ = to_edge.close().await;
    Ok(())
}

fn extract_path(dest: &str) -> String {
    if let Some(after_scheme) = dest.find("://") {
        let rest = &dest[after_scheme + 3..];
        if let Some(slash) = rest.find('/') {
            return rest[slash..].to_string();
        }
        return "/".into();
    }
    if dest.starts_with('/') {
        return dest.to_string();
    }
    "/".into()
}

const MAX_HEADER_BYTES: usize = 32 * 1024;

async fn read_http_response_head(
    tcp: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<(u16, Vec<(String, String)>, Vec<u8>)> {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 2048];
    loop {
        let n = tcp
            .read(&mut tmp)
            .await
            .map_err(|e| TunnelError::Internal(format!("tcp read head: {e}")))?;
        if n == 0 {
            return Err(TunnelError::Internal("origin closed before response head".into()));
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > MAX_HEADER_BYTES {
            return Err(TunnelError::Internal(format!("header exceeds {MAX_HEADER_BYTES}")));
        }
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut resp = httparse::Response::new(&mut headers);
        match resp.parse(&buf).map_err(|e| TunnelError::Internal(format!("httparse: {e}")))? {
            httparse::Status::Complete(consumed) => {
                let status = resp.code.ok_or_else(|| TunnelError::Internal("no status code".into()))?;
                let pairs = resp.headers.iter().map(|h| {
                    let v = String::from_utf8_lossy(h.value).into_owned();
                    (h.name.to_string(), v)
                }).collect::<Vec<_>>();
                let leftover = buf.split_off(consumed);
                return Ok((status, pairs, leftover));
            }
            httparse::Status::Partial => {}
        }
    }
}
