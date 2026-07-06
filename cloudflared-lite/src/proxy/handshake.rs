use crate::error::{Result, TunnelError};
use crate::quic_metadata_protocol_capnp::{self, connect_request, connect_response};
use capnp::message::ReaderOptions;
use capnp_futures::serialize;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const DATA_STREAM_SIGNATURE: [u8; 6] = [0x0A, 0x36, 0xCD, 0x12, 0xA1, 0x3E];
pub const PROTOCOL_V1: [u8; 2] = [b'0', b'1'];

pub struct ConnectRequest {
    pub dest: String,
    pub conn_type: ConnectionType,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, PartialEq)]
pub enum ConnectionType {
    Http,
    WebSocket,
    Tcp,
}

pub async fn read_connect_request<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<ConnectRequest> {
    let mut sig = [0u8; 6];
    reader.read_exact(&mut sig).await
        .map_err(|e| TunnelError::Internal(format!("read signature: {e}")))?;
    if sig != DATA_STREAM_SIGNATURE {
        return Err(TunnelError::Internal(format!("bad signature: {sig:02x?}")));
    }

    let mut ver = [0u8; 2];
    reader.read_exact(&mut ver).await
        .map_err(|e| TunnelError::Internal(format!("read version: {e}")))?;

    let msg = serialize::read_message(reader, ReaderOptions::new())
        .await
        .map_err(|e| TunnelError::Internal(format!("read capnp: {e}")))?;

    let root: connect_request::Reader = msg
        .get_root()
        .map_err(|e| TunnelError::Internal(format!("root: {e}")))?;

    let dest = root
        .get_dest()
        .map_err(|e| TunnelError::Internal(format!("dest: {e}")))?
        .to_string()
        .map_err(|e| TunnelError::Internal(format!("dest utf8: {e}")))?;

    let conn_type = match root
        .get_type()
        .map_err(|e| TunnelError::Internal(format!("type: {e}")))?
    {
        quic_metadata_protocol_capnp::ConnectionType::Http => ConnectionType::Http,
        quic_metadata_protocol_capnp::ConnectionType::Websocket => ConnectionType::WebSocket,
        quic_metadata_protocol_capnp::ConnectionType::Tcp => ConnectionType::Tcp,
    };

    let mut headers = Vec::new();
    if let Ok(list) = root.get_metadata() {
        for i in 0..list.len() {
            let entry = list.get(i);
            let k = entry.get_key().ok()
                .and_then(|t| t.to_string().ok())
                .unwrap_or_default();
            let v = entry.get_val().ok()
                .and_then(|t| t.to_string().ok())
                .unwrap_or_default();
            headers.push((k, v));
        }
    }

    Ok(ConnectRequest { dest, conn_type, headers })
}

pub async fn write_connect_response<W: AsyncWrite + Unpin>(
    writer: &mut W,
    error: &str,
    metadata: &[(&str, &str)],
) -> Result<()> {
    writer.write_all(&DATA_STREAM_SIGNATURE).await
        .map_err(|e| TunnelError::Internal(format!("write sig: {e}")))?;
    writer.write_all(&PROTOCOL_V1).await
        .map_err(|e| TunnelError::Internal(format!("write version: {e}")))?;

    let mut message = capnp::message::Builder::new_default();
    {
        let mut root: connect_response::Builder = message.init_root();
        root.set_error(error);
        let mut meta = root.init_metadata(metadata.len() as u32);
        for (i, (k, v)) in metadata.iter().enumerate() {
            let mut entry = meta.reborrow().get(i as u32);
            entry.set_key(*k);
            entry.set_val(*v);
        }
    }

    serialize::write_message(&mut *writer, &message)
        .await
        .map_err(|e| TunnelError::Internal(format!("write capnp: {e}")))?;

    writer.flush().await
        .map_err(|e| TunnelError::Internal(format!("flush: {e}")))?;

    Ok(())
}
