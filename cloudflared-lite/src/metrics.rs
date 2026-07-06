use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct TunnelMetrics {
    pub streams_total: AtomicU64,
    pub bytes_in: AtomicU64,
    pub bytes_out: AtomicU64,
    pub reconnects: AtomicU64,
}

impl TunnelMetrics {
    pub fn increment_streams(&self) {
        self.streams_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_bytes_in(&self, n: u64) {
        self.bytes_in.fetch_add(n, Ordering::Relaxed);
    }

    pub fn add_bytes_out(&self, n: u64) {
        self.bytes_out.fetch_add(n, Ordering::Relaxed);
    }

    pub fn increment_reconnects(&self) {
        self.reconnects.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> TunnelMetricsSnapshot {
        TunnelMetricsSnapshot {
            streams_total: self.streams_total.load(Ordering::Relaxed),
            bytes_in: self.bytes_in.load(Ordering::Relaxed),
            bytes_out: self.bytes_out.load(Ordering::Relaxed),
            reconnects: self.reconnects.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TunnelMetricsSnapshot {
    pub streams_total: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub reconnects: u64,
}
