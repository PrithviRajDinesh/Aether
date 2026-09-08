use std::{
    sync::atomic::Ordering,
    thread,
    time::Duration,
};

use crate::control_plane::telemetry;
use crate::data_plane::metrics::METRICS;
use crate::data_plane::queue;

pub fn start_metrics_sampler() -> thread::JoinHandle<()> {
    thread::spawn(|| {
        let mut previous_rx = 0u64;
        let mut previous_tx = 0u64;

        loop {
            thread::sleep(Duration::from_secs(1));

            // ── PPS ───────────────────────────────────────────────────────
            let current_rx =
                METRICS
                    .packets_received
                    .load(Ordering::Relaxed);

            let current_tx =
                METRICS
                    .packets_transmitted
                    .load(Ordering::Relaxed);

            let rx_pps =
                current_rx.saturating_sub(previous_rx);

            let tx_pps =
                current_tx.saturating_sub(previous_tx);

            METRICS
                .rx_pps
                .store(rx_pps, Ordering::Relaxed);

            METRICS
                .tx_pps
                .store(tx_pps, Ordering::Relaxed);

            previous_rx = current_rx;
            previous_tx = current_tx;

            // ── Tail latency percentiles over last second's samples ───────
            let mut samples: Vec<u64> =
                Vec::with_capacity(telemetry::LATENCY_SAMPLE_CAPACITY);

            while let Some(sample) = telemetry::LATENCY_SAMPLES.pop() {
                samples.push(sample);
            }

            if !samples.is_empty() {
                samples.sort_unstable();

                let p50 = percentile(&samples, 0.50);
                let p99 = percentile(&samples, 0.99);
                let p999 = percentile(&samples, 0.999);

                telemetry::LATENCY_P50_NS
                    .store(p50, Ordering::Relaxed);

                telemetry::LATENCY_P99_NS
                    .store(p99, Ordering::Relaxed);

                telemetry::LATENCY_P999_NS
                    .store(p999, Ordering::Relaxed);

                /*
                 * Jitter = p99 − p50.
                 *
                 * A tight spread (< 500 ns) proves the data plane
                 * processes packets deterministically with minimal
                 * scheduling variance — a key kernel-bypass property.
                 */
                telemetry::LATENCY_JITTER_NS
                    .store(p99.saturating_sub(p50), Ordering::Relaxed);
            }

            // ── Queue utilisation % ───────────────────────────────────────
            //
            // Both queues have the same fixed capacity.
            // We express depth as an integer percentage (0-100) so the
            // dashboard can display a simple back-pressure indicator.

            let cap = queue::PACKET_QUEUE_SIZE as u64;

            let rx_depth =
                queue::rx_queue_len() as u64;

            let tx_depth =
                queue::tx_queue_len() as u64;

            telemetry::RX_QUEUE_DEPTH_PCT
                .store(rx_depth * 100 / cap, Ordering::Relaxed);

            telemetry::TX_QUEUE_DEPTH_PCT
                .store(tx_depth * 100 / cap, Ordering::Relaxed);
        }
    })
}

fn percentile(sorted_samples: &[u64], p: f64) -> u64 {
    let idx =
        ((sorted_samples.len() as f64 - 1.0) * p).round() as usize;

    sorted_samples[idx]
}
