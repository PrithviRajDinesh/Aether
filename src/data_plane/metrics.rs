use std::sync::atomic::{
    AtomicU64,
    Ordering,
};

pub struct Metrics {
    pub packets_received: AtomicU64,
    pub packets_transmitted: AtomicU64,

    pub bytes_received: AtomicU64,
    pub bytes_transmitted: AtomicU64,

    pub rx_dropped: AtomicU64,
    pub worker_dropped: AtomicU64,
    pub tx_dropped: AtomicU64,

    pub rx_pps: AtomicU64,
    pub tx_pps: AtomicU64,

    pub rx_queue_depth: AtomicU64,
    pub tx_queue_depth: AtomicU64,
}

impl Metrics {
    pub const fn new() -> Self {
        Self {
            packets_received: AtomicU64::new(0),
            packets_transmitted: AtomicU64::new(0),

            bytes_received: AtomicU64::new(0),
            bytes_transmitted: AtomicU64::new(0),

            rx_dropped: AtomicU64::new(0),
            worker_dropped: AtomicU64::new(0),
            tx_dropped: AtomicU64::new(0),

            rx_pps: AtomicU64::new(0),
            tx_pps: AtomicU64::new(0),

            rx_queue_depth: AtomicU64::new(0),
            tx_queue_depth: AtomicU64::new(0),
        }
    }

    pub fn total_dropped(&self) -> u64 {
        self.rx_dropped.load(Ordering::Relaxed)
            + self.worker_dropped.load(Ordering::Relaxed)
            + self.tx_dropped.load(Ordering::Relaxed)
    }
}

pub static METRICS: Metrics = Metrics::new();
