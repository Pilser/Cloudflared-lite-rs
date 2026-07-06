use std::time::Duration;
use tokio::sync::oneshot;

pub enum ShutdownCommand {
    Immediate,
    Graceful(Duration),
}

pub struct ControlSession {
    shutdown_tx: Option<oneshot::Sender<ShutdownCommand>>,
    driver_done_rx: Option<oneshot::Receiver<()>>,
    _join: std::thread::JoinHandle<()>,
    pub location_name: String,
    pub uuid: Vec<u8>,
}

impl ControlSession {
    pub fn new(
        location_name: String,
        uuid: Vec<u8>,
        shutdown_tx: oneshot::Sender<ShutdownCommand>,
        driver_done_rx: oneshot::Receiver<()>,
        join: std::thread::JoinHandle<()>,
    ) -> Self {
        Self {
            shutdown_tx: Some(shutdown_tx),
            driver_done_rx: Some(driver_done_rx),
            _join: join,
            location_name,
            uuid,
        }
    }

    pub async fn shutdown_graceful(mut self, grace: Duration) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(ShutdownCommand::Graceful(grace));
        }
        if let Some(rx) = self.driver_done_rx.take() {
            let budget = grace + Duration::from_secs(2);
            let _ = tokio::time::timeout(budget, rx).await;
        }
    }
}

impl Drop for ControlSession {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(ShutdownCommand::Immediate);
        }
    }
}
