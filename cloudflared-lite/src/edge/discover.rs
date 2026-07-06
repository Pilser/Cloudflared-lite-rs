use crate::error::{Result, TunnelError};
use hickory_resolver::system_conf;
use hickory_resolver::TokioAsyncResolver;
use std::net::SocketAddr;

const SRV_NAME: &str = "_v2-origintunneld._tcp.argotunnel.com";
const EDGE_PORT: u16 = 7844;

#[derive(Debug, Clone)]
pub struct EdgeAddr {
    pub addr: SocketAddr,
}

pub struct EdgeRegistry;

impl EdgeRegistry {
    pub async fn discover() -> Result<Vec<EdgeAddr>> {
        discover_edges().await
    }
}

pub async fn discover_edges() -> Result<Vec<EdgeAddr>> {
    let (config, opts) = system_conf::read_system_conf()
        .map_err(|e| TunnelError::Discovery(format!("read system DNS config: {e}")))?;

    let resolver = TokioAsyncResolver::tokio(config, opts);

    let srv_lookup = resolver
        .srv_lookup(SRV_NAME)
        .await
        .map_err(|e| TunnelError::Discovery(format!("SRV lookup failed: {e}")))?;

    let mut addrs = Vec::new();
    for srv in srv_lookup.iter() {
        let target = srv.target().to_string();
        if let Ok(lookup) = resolver.lookup_ip(target).await {
            for ip in lookup.iter() {
                addrs.push(EdgeAddr {
                    addr: SocketAddr::new(ip, EDGE_PORT),
                });
            }
        }
    }

    if addrs.is_empty() {
        return Err(TunnelError::Discovery("no edge addresses resolved".into()));
    }

    Ok(addrs)
}
