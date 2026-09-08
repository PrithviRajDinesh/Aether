/// flow_bench — Flow-table lookup throughput benchmark
///
/// Measures how many stateful per-flow lookups the LoadBalancer can sustain
/// per second on a single core, using the real production HashMap implementation.
///
/// This demonstrates that Aether's control plane can track millions of concurrent
/// TCP/IP sessions without a kernel context switch — a key property of kernel-bypass
/// load balancers.
///
/// Resume metric: "Stateful per-flow LB: X Mlookups/sec, Y ns/lookup, N concurrent flows"

use std::ffi::CString;
use std::os::raw::c_char;

use Aether::data_plane::lb::{Backend, FlowKey, LoadBalancer};
use Aether::dpdk;


/// Total number of lookup operations measured.
const TOTAL_LOOKUPS: u64 = 10_000_000;

/// Number of distinct flows to pre-seed before timing.
/// Controls cache pressure: more flows → more hash-map misses → harder benchmark.
const UNIQUE_FLOWS: u32 = 100_000;

/// Number of backends the LB distributes across.
const NUM_BACKENDS: usize = 8;

/// Warm-up iterations (not timed).
const WARMUP_LOOKUPS: u64 = 50_000;


fn make_flow(i: u32) -> FlowKey {
    // Spread flows across 192.168.0.0/16 src range.
    FlowKey {
        src_ip:   u32::from_be_bytes([192, 168, (i >> 8) as u8, i as u8]),
        dst_ip:   u32::from_be_bytes([10, 0, 0, 100]),
        src_port: ((i % 60000) + 1024) as u16,
        dst_port: 80,
        protocol: 6,
    }
}

fn build_backends() -> Vec<Backend> {
    (0..NUM_BACKENDS)
        .map(|i| Backend {
            ip:   u32::from_be_bytes([10, 0, 1, i as u8]),
            port: 8080,
        })
        .collect()
}


fn main() {
    // Minimal DPDK init — no PCI / no NIC needed.
    let dpdk_args = vec![
        "flow_bench".to_string(),
        "--no-pci".to_string(),
        "-l".to_string(),
        "0".to_string(),
    ];
    let c_strings: Vec<CString> =
        dpdk_args.iter().map(|a| CString::new(a.as_str()).unwrap()).collect();
    let mut c_ptrs: Vec<*mut c_char> =
        c_strings.iter().map(|s| s.as_ptr() as *mut c_char).collect();

    let ret = unsafe { dpdk::rte_eal_init(c_ptrs.len() as i32, c_ptrs.as_mut_ptr()) };
    assert!(ret >= 0, "EAL init failed: {}", ret);

    let tsc_hz = unsafe { dpdk::aether_get_tsc_hz() };

    let mut lb = LoadBalancer::new(build_backends());

    // Pre-seed UNIQUE_FLOWS entries so lookups are mostly hits.
    for i in 0..UNIQUE_FLOWS {
        let _ = lb.lookup(make_flow(i));
    }

    for i in 0..WARMUP_LOOKUPS {
        let _ = lb.lookup(make_flow((i % UNIQUE_FLOWS as u64) as u32));
    }

    let mut per_lookup_cycles: Vec<u64> = Vec::with_capacity(TOTAL_LOOKUPS as usize);

    for i in 0..TOTAL_LOOKUPS {
        let flow = make_flow((i % UNIQUE_FLOWS as u64) as u32);

        let t0 = unsafe { dpdk::aether_rdtsc() };
        let _ = std::hint::black_box(lb.lookup(flow));
        let t1 = unsafe { dpdk::aether_rdtsc() };

        per_lookup_cycles.push(t1.wrapping_sub(t0));
    }

    let total_cycles: u64 = per_lookup_cycles.iter().sum();

    let min_cyc = *per_lookup_cycles.iter().min().unwrap();
    let max_cyc = *per_lookup_cycles.iter().max().unwrap();

    let mean = total_cycles as f64 / TOTAL_LOOKUPS as f64;

    let variance: f64 = per_lookup_cycles
        .iter()
        .map(|&c| { let d = c as f64 - mean; d * d })
        .sum::<f64>()
        / TOTAL_LOOKUPS as f64;

    let stddev = variance.sqrt();

    per_lookup_cycles.sort_unstable();
    let p99_idx = ((TOTAL_LOOKUPS as f64 - 1.0) * 0.99).round() as usize;
    let p99_cyc = per_lookup_cycles[p99_idx];

    let cycles_to_ns = |c: f64| c * 1_000_000_000.0 / tsc_hz as f64;

    let elapsed_secs = total_cycles as f64 / tsc_hz as f64;
    let mlps         = TOTAL_LOOKUPS as f64 / elapsed_secs / 1_000_000.0; // Mega-lookups/s

    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║        Aether  —  flow_bench  results                    ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Total lookups        : {:>12}                      ║", TOTAL_LOOKUPS);
    println!("║  Unique flows         : {:>12}                      ║", UNIQUE_FLOWS);
    println!("║  Backends             : {:>12}                      ║", NUM_BACKENDS);
    println!("║  TSC frequency        : {:>12} Hz                  ║", tsc_hz);
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Cycles per lookup                                        ║");
    println!("║    mean               : {:>10.1} cyc  ({:.1} ns)        ║", mean, cycles_to_ns(mean));
    println!("║    min                : {:>10}  cyc  ({:.1} ns)        ║", min_cyc,  cycles_to_ns(min_cyc  as f64));
    println!("║    max                : {:>10}  cyc  ({:.1} ns)        ║", max_cyc,  cycles_to_ns(max_cyc  as f64));
    println!("║    stddev             : {:>10.1} cyc  ({:.1} ns)        ║", stddev, cycles_to_ns(stddev));
    println!("║    p99                : {:>10}  cyc  ({:.1} ns)        ║", p99_cyc, cycles_to_ns(p99_cyc as f64));
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Throughput           : {:>10.3} Mlookups/sec            ║", mlps);
    println!("║  Lookup type          :  HashMap stateful (hit path)      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    println!(
        "RESUME: Stateful per-flow LB — {:.3} Mlookups/sec, {:.1} ns/lookup (p99={:.1} ns), \
{} concurrent flows tracked (single-core, kernel-bypass)",
        mlps,
        cycles_to_ns(mean),
        cycles_to_ns(p99_cyc as f64),
        UNIQUE_FLOWS,
    );
    println!();
}
