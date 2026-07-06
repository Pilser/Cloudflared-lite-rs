use crate::proxy::{proxy_http, Pool};
use quinn::Connection;
use std::sync::Arc;
use tokio::sync::Notify;

pub enum AcceptExit {
    Shutdown,
    ConnectionLost,
}

pub async fn accept_loop(
    conn: &Connection,
    local_port: u16,
    pool: Arc<Pool>,
    shutdown: Arc<Notify>,
) -> AcceptExit {
    loop {
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                return AcceptExit::Shutdown;
            }
            stream = conn.accept_bi() => {
                match stream {
                    Ok((send, recv)) => {
                        tokio::spawn(proxy_http(
                            local_port,
                            send,
                            recv,
                            pool.clone(),
                            shutdown.clone(),
                        ));
                    }
                    Err(e) => {
                        tracing::warn!("accept_bi error: {e}");
                        return AcceptExit::ConnectionLost;
                    }
                }
            }
        }
    }
}
