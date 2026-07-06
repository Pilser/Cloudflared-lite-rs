use crate::edge::EdgeAddr;
use crate::error::{Result, TunnelError};
use quinn::{ClientConfig, Endpoint, TransportConfig};
use rustls::pki_types::CertificateDer;
use rustls::ClientConfig as RustlsClientConfig;
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;

const ALPN: &[u8] = b"argotunnel";
const EDGE_SNI: &str = "quic.cftunnel.com";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

const CF_EDGE_ROOTS_PEM: &[u8] = include_bytes!("roots.pem");

pub struct DialOptions {
    pub handshake_timeout: Duration,
}

impl Default for DialOptions {
    fn default() -> Self {
        Self {
            handshake_timeout: HANDSHAKE_TIMEOUT,
        }
    }
}

pub fn build_endpoint() -> Result<Endpoint> {
    install_crypto_provider();
    let config = build_client_config()?;
    let local = "0.0.0.0:0".parse().unwrap();
    let mut endpoint = Endpoint::client(local)
        .map_err(|e| TunnelError::Internal(format!("Endpoint::client bind: {e}")))?;
    endpoint.set_default_client_config(config);
    Ok(endpoint)
}

pub async fn dial_any(
    endpoint: &Endpoint,
    edges: &[EdgeAddr],
    _opts: &DialOptions,
) -> Result<quinn::Connection> {
    if edges.is_empty() {
        return Err(TunnelError::QuicDial {
            attempts: 0,
            last: "no edges provided".into(),
        });
    }
    let mut last_err = String::new();
    for (i, edge) in edges.iter().enumerate() {
        match dial(endpoint, edge).await {
            Ok(conn) => {
                tracing::info!(attempt = i, addr = %edge.addr, "QUIC handshake OK");
                return Ok(conn);
            }
            Err(e) => {
                tracing::warn!(attempt = i, addr = %edge.addr, error = %e, "QUIC dial failed");
                last_err = e.to_string();
            }
        }
    }
    Err(TunnelError::QuicDial {
        attempts: edges.len() as u32,
        last: last_err,
    })
}

async fn dial(endpoint: &Endpoint, edge: &EdgeAddr) -> Result<quinn::Connection> {
    let connecting = endpoint
        .connect(edge.addr, EDGE_SNI)
        .map_err(|e| TunnelError::QuicDial {
            attempts: 1,
            last: format!("connect builder: {e}"),
        })?;

    tokio::time::timeout(HANDSHAKE_TIMEOUT, connecting)
        .await
        .map_err(|_| TunnelError::QuicDial {
            attempts: 1,
            last: "handshake timed out".into(),
        })?
        .map_err(|e| TunnelError::QuicDial {
            attempts: 1,
            last: format!("handshake: {e}"),
        })

}

fn install_crypto_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn build_client_config() -> Result<ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();

    for cert in rustls_native_certs::load_native_certs().unwrap_or_default() {
        let der = CertificateDer::from(cert.to_vec());
        let _ = roots.add(der);
    }

    let mut reader = std::io::BufReader::new(CF_EDGE_ROOTS_PEM);
    for cert in rustls_pemfile::certs(&mut reader) {
        if let Ok(cert) = cert {
            let _ = roots.add(cert);
        }
    }

    let mut tls = RustlsClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];

    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(tls)
        .map_err(|e| TunnelError::Internal(format!("rustls→quinn: {e}")))?;

    let mut cfg = ClientConfig::new(Arc::new(crypto));

    let mut transport = TransportConfig::default();
    transport.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));
    transport.keep_alive_interval(Some(Duration::from_secs(1)));
    cfg.transport_config(Arc::new(transport));

    Ok(cfg)
}
