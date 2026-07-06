use crate::config::{decode_token, load_credentials_file, TunnelConfig};
use crate::config::api::register_quick_tunnel;
use crate::error::Result;
use crate::metrics::{TunnelMetrics, TunnelMetricsSnapshot};
use crate::proxy::Pool;
use crate::tunnel::Reactor;
use std::sync::Arc;
use tokio::sync::Notify;

pub struct TunnelBuilder {
    config: Option<TunnelConfig>,
    local_port: u16,
    ha_connections: u32,
}

impl TunnelBuilder {
    fn new() -> Self {
        Self {
            config: None,
            local_port: 8080,
            ha_connections: 1,
        }
    }

    pub fn port(mut self, port: u16) -> Self {
        self.local_port = port;
        self
    }

    pub fn ha_connections(mut self, n: u32) -> Self {
        self.ha_connections = n.clamp(1, 4);
        self
    }

    pub async fn start(self) -> Result<TunnelManager> {
        let config = match self.config {
            Some(c) => c,
            None => register_quick_tunnel().await?,
        };

        let pool = Arc::new(Pool::new());
        let shutdown = Arc::new(Notify::new());
        let metrics = Arc::new(TunnelMetrics::default());

        let mut handles = Vec::with_capacity(self.ha_connections as usize);
        for i in 0..self.ha_connections {
            let reactor = Reactor::new(
                config.clone(),
                self.local_port,
                pool.clone(),
                shutdown.clone(),
                i as u8,
            );
            handles.push(tokio::spawn(async move { reactor.run().await }));
        }

        Ok(TunnelManager {
            config,
            shutdown,
            handles,
            metrics,
        })
    }
}

pub struct TunnelManager {
    config: TunnelConfig,
    shutdown: Arc<Notify>,
    handles: Vec<tokio::task::JoinHandle<Result<()>>>,
    metrics: Arc<TunnelMetrics>,
}

impl TunnelManager {
    pub fn quick() -> TunnelBuilder {
        TunnelBuilder::new()
    }

    pub fn named_with_token(token: &str) -> TunnelBuilder {
        let config = decode_token(token).expect("invalid TUNNEL_TOKEN");
        TunnelBuilder {
            config: Some(config),
            ..TunnelBuilder::new()
        }
    }

    pub fn named_with_file(path: &str) -> TunnelBuilder {
        let config = load_credentials_file(path).expect("invalid credentials file");
        TunnelBuilder {
            config: Some(config),
            ..TunnelBuilder::new()
        }
    }

    pub fn url(&self) -> &str {
        &self.config.hostname
    }

    pub fn metrics(&self) -> TunnelMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub async fn shutdown(&mut self) {
        self.shutdown.notify_waiters();
        for handle in self.handles.drain(..) {
            handle.await.ok();
        }
    }
}

impl Drop for TunnelManager {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
    }
}
