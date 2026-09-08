/// flow_bench_pure — Flow-table lookup throughput using std::time (no DPDK needed)
/// Runs on any machine without hugepages.
/// Measures the same LoadBalancer but uses Instant for timing instead of rdtsc.

use std::collections::HashMap;
use std::time::Instant;

// Inline the FlowKey/Backend/LoadBalancer types here since we can't call into
// the DPDK-linked library from a no-DPDK binary.

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
struct FlowKey {
    src_ip:   u32,
    dst_ip:   u32,
    src_port: u16,
    dst_port: u16,
    protocol: u8,
}

#[derive(Copy, Clone)]
struct Backend {
    ip:   u32,
    port: u16,
}

struct LoadBalancer {
    backends:     Vec<Backend>,
    flows:        HashMap<FlowKey, usize>,
    next_backend: usize,
}

impl LoadBalancer {
    fn new(backends: Vec<Backend>) -> Self {
        Self { backends, flows: HashMap::new(), next_backend: 0 }
    }

    fn lookup(&mut self, flow: FlowKey) -> Backend {
        if let Some(&idx) = self.flows.get(&flow) {
            return self.backends[idx];
        }
        let idx = self.next_backend;
        self.next_backend = (self.next_backend + 1) % self.backends.len();
        self.flows.insert(flow, idx);
        self.backends[idx]
    }
}

const TOTAL_LOOKUPS: u64  = 10_000_000;
const UNIQUE_FLOWS: u32   = 100_000;
const NUM_BACKENDS: usize = 8;
const WARMUP_LOOKUPS: u64 = 50_000;

fn make_flow(i: u32) -> FlowKey {
    FlowKey {
        src_ip:   u32::from_be_bytes([192, 168, (i >> 8) as u8, i as u8]),
        dst_ip:   u32::from_be_bytes([10, 0, 0, 100]),
        src_port: ((i % 60000) + 1024) as u16,
        dst_port: 80,
        protocol: 6,
    }
}

fn main() {
    let backends: Vec<Backend> = (0..NUM_BACKENDS)
        .map(|i| Backend { ip: u32::from_be_bytes([10, 0, 1, i as u8]), port: 8080 })
        .collect();

    let mut lb = LoadBalancer::new(backends);

    // Pre-seed flows
    for i in 0..UNIQUE_FLOWS {
        let _ = lb.lookup(make_flow(i));
    }

    // Warm-up
    for i in 0..WARMUP_LOOKUPS {
        let _ = lb.lookup(make_flow((i % UNIQUE_FLOWS as u64) as u32));
    }

    // Timed run — per-lookup Instant timing
    let mut per_lookup_ns: Vec<u64> = Vec::with_capacity(TOTAL_LOOKUPS as usize);

    for i in 0..TOTAL_LOOKUPS {
        let flow = make_flow((i % UNIQUE_FLOWS as u64) as u32);
        let t0 = Instant::now();
        let _ = std::hint::black_box(lb.lookup(flow));
        per_lookup_ns.push(t0.elapsed().as_nanos() as u64);
    }

    // Stats
    let total_ns: u64 = per_lookup_ns.iter().sum();
    let min_ns    = *per_lookup_ns.iter().min().unwrap();
    let max_ns    = *per_lookup_ns.iter().max().unwrap();
    let mean      = total_ns as f64 / TOTAL_LOOKUPS as f64;
    let var: f64  = per_lookup_ns.iter()
        .map(|&x| { let d = x as f64 - mean; d * d })
        .sum::<f64>() / TOTAL_LOOKUPS as f64;
    let stddev = var.sqrt();

    per_lookup_ns.sort_unstable();
    let p50_idx  = ((TOTAL_LOOKUPS as f64 - 1.0) * 0.50).round() as usize;
    let p99_idx  = ((TOTAL_LOOKUPS as f64 - 1.0) * 0.99).round() as usize;
    let p50 = per_lookup_ns[p50_idx];
    let p99 = per_lookup_ns[p99_idx];

    let elapsed_secs = total_ns as f64 / 1_000_000_000.0;
    let mlps = TOTAL_LOOKUPS as f64 / elapsed_secs / 1_000_000.0;

    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║      Aether flow_bench (no DPDK timing overhead)         ║");
    println!("║      HashMap stateful LB — {} lookups, {} flows     ║", TOTAL_LOOKUPS, UNIQUE_FLOWS);
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ns per lookup                                            ║");
    println!("║    mean               : {:>10.1} ns                       ║", mean);
    println!("║    min                : {:>10} ns                       ║", min_ns);
    println!("║    max                : {:>10} ns                       ║", max_ns);
    println!("║    stddev (σ)         : {:>10.1} ns                       ║", stddev);
    println!("║    p50                : {:>10} ns                       ║", p50);
    println!("║    p99                : {:>10} ns                       ║", p99);
    println!("║    jitter (p99−p50)   : {:>10} ns                       ║", p99.saturating_sub(p50));
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Throughput           : {:>10.3} Mlookups/sec             ║", mlps);
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("RESUME: Stateful LB — {:.3} Mlookups/sec, {:.1} ns/lookup (p99={} ns), {} concurrent flows",
        mlps, mean, p99, UNIQUE_FLOWS);
    println!();
}
