use crate::config::TunnelConfig;
use crate::edge::{build_endpoint, dial_any, discover_edges, DialOptions};
use crate::error::{Result, TunnelError};
use crate::proxy::Pool;
use crate::rpc::{register_connection, ControlSession, Registration};
use crate::tunnel::{accept_loop, AcceptExit};
use quinn::{Connection, Endpoint};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

const MAX_RETRIES: u32 = 10;
const BASE_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

struct ConnectedRegistration {
    conn: Connection,
    registration: Registration,
}

pub struct Reactor {
    config: TunnelConfig,
    local_port: u16,
    pool: Arc<Pool>,
    shutdown: Arc<Notify>,
}

impl Reactor {
    pub fn new(
        config: TunnelConfig,
        local_port: u16,
        pool: Arc<Pool>,
        shutdown: Arc<Notify>,
    ) -> Self {
        Self {
            config,
            local_port,
            pool,
            shutdown,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let endpoint = build_endpoint()?;
        let mut attempt: u32 = 0;

        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.notified() => {
                    tracing::info!("reactor shutting down");
                    return Ok(());
                }
                result = self.connect_cycle(&endpoint, attempt as u8) => {
                    match result {
                        Ok(cr) => {
                            attempt = 0;
                            let session = make_session(cr.registration);

                            let exit = accept_loop(
                                &cr.conn,
                                self.local_port,
                                self.pool.clone(),
                                self.shutdown.clone(),
                            ).await;

                            match exit {
                                AcceptExit::Shutdown => {
                                    session.shutdown_graceful(Duration::from_secs(30)).await;
                                    return Ok(());
                                }
                                AcceptExit::ConnectionLost => {
                                    tracing::warn!("connection lost, reconnecting...");
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("connect cycle failed: {e}");
                            attempt += 1;
                            if attempt >= MAX_RETRIES {
                                return Err(TunnelError::PermanentFailure(attempt));
                            }
                            let delay = backoff(attempt);
                            tracing::info!("reconnecting in {delay:?} (attempt {attempt})");
                            tokio::select! {
                                biased;
                                _ = self.shutdown.notified() => return Ok(()),
                                _ = tokio::time::sleep(delay) => {}
                            }
                        }
                    }
                }
            }
        }
    }

    async fn connect_cycle(
        &self,
        endpoint: &Endpoint,
        prev_attempts: u8,
    ) -> Result<ConnectedRegistration> {
        let edges = discover_edges().await?;
        let opts = DialOptions::default();
        let conn = dial_any(endpoint, &edges, &opts).await?;

        let registration = register_connection(
            &conn,
            &self.config,
            0,
            prev_attempts,
        )
        .await?;

        tracing::info!(
            "registered at colo {}, uuid={:02x?}, managed={}",
            registration.details.location,
            registration.details.uuid,
            registration.details.tunnel_is_remotely_managed,
        );

        Ok(ConnectedRegistration {
            conn,
            registration,
        })
    }
}

fn make_session(reg: Registration) -> ControlSession {
    ControlSession::new(
        reg.details.location,
        reg.details.uuid,
        reg.shutdown_tx,
        reg.driver_done_rx,
        reg._join,
    )
}

fn backoff(attempt: u32) -> Duration {
    let delay = BASE_BACKOFF * 2u32.saturating_pow(attempt.saturating_sub(1));
    std::cmp::min(delay, MAX_BACKOFF)
}
