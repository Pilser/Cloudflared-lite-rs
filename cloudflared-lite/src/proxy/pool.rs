use crate::error::{Result, TunnelError};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

const MAX_IDLE: usize = 16;
const IDLE_TTL: Duration = Duration::from_secs(30);

struct IdleConnection {
    stream: TcpStream,
    created: Instant,
}

pub struct Pool {
    inner: tokio::sync::Mutex<PoolInner>,
}

struct PoolInner {
    connections: VecDeque<IdleConnection>,
}

impl Pool {
    pub fn new() -> Self {
        Self {
            inner: tokio::sync::Mutex::new(PoolInner {
                connections: VecDeque::new(),
            }),
        }
    }

    pub async fn acquire(&self, port: u16) -> Result<TcpStream> {
        let mut inner = self.inner.lock().await;
        let now = Instant::now();

        while let Some(idle) = inner.connections.pop_front() {
            if now.duration_since(idle.created) < IDLE_TTL {
                return Ok(idle.stream);
            }
        }

        TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .map_err(|e| TunnelError::Io(e))
    }

    pub async fn release(&self, stream: TcpStream) {
        let mut inner = self.inner.lock().await;
        if inner.connections.len() < MAX_IDLE {
            inner.connections.push_back(IdleConnection {
                stream,
                created: Instant::now(),
            });
        }
    }
}
