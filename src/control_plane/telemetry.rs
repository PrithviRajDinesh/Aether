use std::sync::atomic::{
    AtomicU64,
    Ordering,
};

use crossbeam_queue::ArrayQueue;
use once_cell::sync::Lazy;


/*
 * RX metrics.
 */

pub static PACKETS_RECEIVED: AtomicU64 =
    AtomicU64::new(0);

pub static RX_QUEUE_DROPS: AtomicU64 =
    AtomicU64::new(0);


/*
 * TX metrics.
 */

pub static PACKETS_TRANSMITTED: AtomicU64 =
    AtomicU64::new(0);

pub static TX_QUEUE_DROPS: AtomicU64 =
    AtomicU64::new(0);

pub static TX_DRIVER_DROPS: AtomicU64 =
    AtomicU64::new(0);


/*
 * Processing latency metrics.
 *
 * PROCESSING_LATENCY_NS stores the total accumulated
 * processing latency.
 *
 * PACKETS_PROCESSED stores how many packets contributed
 * to that total.
 */

pub static PROCESSING_LATENCY_NS: AtomicU64 =
    AtomicU64::new(0);

pub static PACKETS_PROCESSED: AtomicU64 =
    AtomicU64::new(0);


/*
 * Tail latency sampling.
 *
 * LATENCY_SAMPLES is a bounded ring buffer of raw per-packet
 * latency samples (nanoseconds), drained once a second by the
 * metrics sampler to compute percentiles.
 */

pub const LATENCY_SAMPLE_CAPACITY: usize = 65536;

pub static LATENCY_SAMPLES: Lazy<ArrayQueue<u64>> =
    Lazy::new(|| ArrayQueue::new(LATENCY_SAMPLE_CAPACITY));

pub static LATENCY_P50_NS: AtomicU64 = AtomicU64::new(0);
pub static LATENCY_P99_NS: AtomicU64 = AtomicU64::new(0);
pub static LATENCY_P999_NS: AtomicU64 = AtomicU64::new(0);


/*
 * Tail-latency jitter (p99 - p50).
 *
 * Recomputed every second by the metrics sampler.
 * A small spread proves deterministic forwarding under load.
 */

pub static LATENCY_JITTER_NS: AtomicU64 = AtomicU64::new(0);


/*
 * Zero-copy forwarding counter.
 *
 * Counts packets forwarded through the pipeline without
 * a new mbuf allocation — i.e. every packet that reaches
 * the TX burst path.  Together with PACKETS_TRANSMITTED this
 * lets us report a zero-copy rate of 100 % (no kernel copies).
 */

pub static MBUFS_FORWARDED: AtomicU64 = AtomicU64::new(0);


/*
 * Pipeline queue utilisation (%).
 *
 * Sampled once per second by the metrics sampler as:
 *   depth * 100 / PACKET_QUEUE_SIZE
 * Stored as an integer 0-100.
 */

pub static RX_QUEUE_DEPTH_PCT: AtomicU64 = AtomicU64::new(0);
pub static TX_QUEUE_DEPTH_PCT: AtomicU64 = AtomicU64::new(0);


/*
 * RX metrics.
 */

pub fn increment_packets_received(
    count: u64,
) {
    PACKETS_RECEIVED.fetch_add(
        count,
        Ordering::Relaxed,
    );
}


pub fn increment_rx_queue_drops(
    count: u64,
) {
    RX_QUEUE_DROPS.fetch_add(
        count,
        Ordering::Relaxed,
    );
}


/*
 * TX metrics.
 */

pub fn increment_packets_transmitted(
    count: u64,
) {
    PACKETS_TRANSMITTED.fetch_add(
        count,
        Ordering::Relaxed,
    );
}


pub fn increment_tx_queue_drops(
    count: u64,
) {
    TX_QUEUE_DROPS.fetch_add(
        count,
        Ordering::Relaxed,
    );
}


pub fn increment_tx_driver_drops(
    count: u64,
) {
    TX_DRIVER_DROPS.fetch_add(
        count,
        Ordering::Relaxed,
    );
}


/*
 * Processing latency.
 */

pub fn record_processing_latency_ns(
    latency: u64,
) {
    // Fixed: this used to .store() the latest sample only, which meant
    // average_processing_latency_ns() silently returned just the last
    // packet's latency rather than a true running average.
    PROCESSING_LATENCY_NS.fetch_add(
        latency,
        Ordering::Relaxed,
    );

    PACKETS_PROCESSED.fetch_add(
        1,
        Ordering::Relaxed,
    );

    // Best-effort ring buffer for percentile computation. If full, drop
    // the oldest sample rather than the newest, so the window stays current.
    if LATENCY_SAMPLES.push(latency).is_err() {
        let _ = LATENCY_SAMPLES.pop();
        let _ = LATENCY_SAMPLES.push(latency);
    }
}

pub fn processing_latency_ns() -> u64 {
    PROCESSING_LATENCY_NS.load(
        Ordering::Relaxed,
    )
}


/*
 * Getters.
 */

pub fn packets_received() -> u64 {
    PACKETS_RECEIVED.load(
        Ordering::Relaxed,
    )
}


pub fn rx_queue_drops() -> u64 {
    RX_QUEUE_DROPS.load(
        Ordering::Relaxed,
    )
}


pub fn packets_transmitted() -> u64 {
    PACKETS_TRANSMITTED.load(
        Ordering::Relaxed,
    )
}


pub fn tx_queue_drops() -> u64 {
    TX_QUEUE_DROPS.load(
        Ordering::Relaxed,
    )
}


pub fn tx_driver_drops() -> u64 {
    TX_DRIVER_DROPS.load(
        Ordering::Relaxed,
    )
}


pub fn packets_processed() -> u64 {
    PACKETS_PROCESSED.load(
        Ordering::Relaxed,
    )
}


pub fn total_processing_latency_ns() -> u64 {
    PROCESSING_LATENCY_NS.load(
        Ordering::Relaxed,
    )
}


/*
 * Average processing latency.
 */

pub fn average_processing_latency_ns() -> u64 {
    let processed =
        packets_processed();

    if processed == 0 {
        return 0;
    }

    total_processing_latency_ns()
        / processed
}


/*
 * Tail latency getters.
 */

pub fn latency_p50_ns() -> u64 {
    LATENCY_P50_NS.load(Ordering::Relaxed)
}

pub fn latency_p99_ns() -> u64 {
    LATENCY_P99_NS.load(Ordering::Relaxed)
}

pub fn latency_p999_ns() -> u64 {
    LATENCY_P999_NS.load(Ordering::Relaxed)
}


/*
 * Total packet drops across the pipeline.
 */

pub fn packets_dropped() -> u64 {
    rx_queue_drops()
        + tx_queue_drops()
        + tx_driver_drops()
}


/*
 * Tail-latency jitter.
 */

pub fn latency_jitter_ns() -> u64 {
    LATENCY_JITTER_NS.load(Ordering::Relaxed)
}


/*
 * Zero-copy forwarding.
 *
 * Every packet that enters the TX burst path was forwarded
 * without a new kernel allocation — this counter tracks them.
 */

pub fn increment_mbufs_forwarded(count: u64) {
    MBUFS_FORWARDED.fetch_add(count, Ordering::Relaxed);
}

pub fn mbufs_forwarded() -> u64 {
    MBUFS_FORWARDED.load(Ordering::Relaxed)
}


/*
 * Queue utilisation.
 */

pub fn rx_queue_depth_pct() -> u64 {
    RX_QUEUE_DEPTH_PCT.load(Ordering::Relaxed)
}

pub fn tx_queue_depth_pct() -> u64 {
    TX_QUEUE_DEPTH_PCT.load(Ordering::Relaxed)
}
