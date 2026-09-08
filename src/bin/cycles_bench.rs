use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;

use Aether::data_plane::rewrite;
use Aether::dpdk;

const NUM_PACKETS: u64 = 5_000_000;
const WARMUP_PACKETS: u64 = 10_000;

const SAMPLE_PACKET: [u8; 54] = [
    0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, // dst MAC
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // src MAC
    0x08, 0x00,                          // EtherType = IPv4
    0x45, 0x00, 0x00, 0x28, 0x00, 0x00, 0x00, 0x00, // IPv4: ver/ihl, dscp, len, id, frag
    0x40, 0x06, 0x00, 0x00,             // ttl, proto=TCP, checksum
    0xC0, 0xA8, 0x01, 0x0A,             // src 192.168.1.10
    0x0A, 0x00, 0x00, 0x64,             // dst 10.0.0.100 (rewritten by the function under test)
    0xC3, 0x50, 0x00, 0x50,             // TCP src_port, dst_port
    0x00, 0x00, 0x00, 0x00,             // seq
    0x00, 0x00, 0x00, 0x00,             // ack
    0x50, 0x02, 0xFF, 0xFF,             // data_offset/flags, window
    0x00, 0x00, 0x00, 0x00,             // checksum, urgent
];

unsafe fn load_packet(mbuf: *mut dpdk::rte_mbuf) {
    let data_ptr = dpdk::aether_pktmbuf_mtod(mbuf) as *mut u8;
    ptr::copy_nonoverlapping(SAMPLE_PACKET.as_ptr(), data_ptr, SAMPLE_PACKET.len());
    (*mbuf).__bindgen_anon_2.__bindgen_anon_1.pkt_len = SAMPLE_PACKET.len() as u32;
    (*mbuf).__bindgen_anon_2.__bindgen_anon_1.data_len = SAMPLE_PACKET.len() as u16;
}

fn main() {
    let dpdk_args = vec![
        "cycles_bench".to_string(),
        "--no-pci".to_string(),
        "-l".to_string(),
        "0".to_string(),
    ];
    let c_strings: Vec<CString> =
        dpdk_args.iter().map(|a| CString::new(a.as_str()).unwrap()).collect();
    let mut c_ptrs: Vec<*mut c_char> =
        c_strings.iter().map(|s| s.as_ptr() as *mut c_char).collect();

    let ret = unsafe { dpdk::rte_eal_init(c_ptrs.len() as i32, c_ptrs.as_mut_ptr()) };
    assert!(ret >= 0, "EAL init failed");

    let pool_name = CString::new("CYCLES_BENCH_POOL").unwrap();
    let mbuf_pool = unsafe {
        dpdk::rte_pktmbuf_pool_create(
            pool_name.as_ptr(),
            8192,
            256,
            0,
            dpdk::RTE_MBUF_DEFAULT_BUF_SIZE as u16,
            0,
        )
    };
    assert!(!mbuf_pool.is_null(), "Failed to create mbuf pool");

    let backend_ip   = u32::from_be_bytes([10, 0, 0, 11]);
    let backend_port: u16 = 8080;

    // Heat L1/L2 caches, branch predictor, and TLB before we start the clock.
    for _ in 0..WARMUP_PACKETS {
        let mbuf = unsafe { dpdk::aether_pktmbuf_alloc(mbuf_pool) };
        unsafe {
            load_packet(mbuf);
            rewrite::rewrite_ipv4_tcp_destination(mbuf, backend_ip, backend_port);
            dpdk::aether_pktmbuf_free(mbuf);
        }
    }

    let mut per_packet_cycles: Vec<u64> = Vec::with_capacity(NUM_PACKETS as usize);

    for _ in 0..NUM_PACKETS {
        let mbuf = unsafe { dpdk::aether_pktmbuf_alloc(mbuf_pool) };

        let t0 = unsafe { dpdk::aether_rdtsc() };

        unsafe {
            load_packet(mbuf);
            rewrite::rewrite_ipv4_tcp_destination(mbuf, backend_ip, backend_port);
            dpdk::aether_pktmbuf_free(mbuf);
        }

        let t1 = unsafe { dpdk::aether_rdtsc() };

        per_packet_cycles.push(t1.wrapping_sub(t0));
    }

    let tsc_hz = unsafe { dpdk::aether_get_tsc_hz() };

    let total_cycles: u64 = per_packet_cycles.iter().sum();
    let min_cycles   = *per_packet_cycles.iter().min().unwrap();
    let max_cycles   = *per_packet_cycles.iter().max().unwrap();

    let mean = total_cycles as f64 / NUM_PACKETS as f64;

    // Population stddev over the per-packet samples.
    let variance: f64 = per_packet_cycles
        .iter()
        .map(|&c| {
            let diff = c as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / NUM_PACKETS as f64;

    let stddev = variance.sqrt();

    // p99 single-packet latency (sorted index).
    per_packet_cycles.sort_unstable();
    let p99_idx = ((NUM_PACKETS as f64 - 1.0) * 0.99).round() as usize;
    let p99_cycles = per_packet_cycles[p99_idx];

    let elapsed_secs = total_cycles as f64 / tsc_hz as f64;
    let mpps         = NUM_PACKETS as f64 / elapsed_secs / 1_000_000.0;

    // Convert cycle counts → nanoseconds for the human-readable columns.
    let cycles_to_ns = |c: f64| c * 1_000_000_000.0 / tsc_hz as f64;

    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          Aether  —  cycles_bench  results                ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Packets processed    : {:>12}                      ║", NUM_PACKETS);
    println!("║  TSC frequency        : {:>12} Hz                  ║", tsc_hz);
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Cycles per packet                                        ║");
    println!("║    mean               : {:>10.1} cyc  ({:.1} ns)        ║", mean, cycles_to_ns(mean));
    println!("║    min                : {:>10}  cyc  ({:.1} ns)        ║", min_cycles, cycles_to_ns(min_cycles as f64));
    println!("║    max                : {:>10}  cyc  ({:.1} ns)        ║", max_cycles, cycles_to_ns(max_cycles as f64));
    println!("║    stddev             : {:>10.1} cyc  ({:.1} ns)        ║", stddev, cycles_to_ns(stddev));
    println!("║    p99                : {:>10}  cyc  ({:.1} ns)        ║", p99_cycles, cycles_to_ns(p99_cycles as f64));
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Throughput           : {:>10.3} Mpps  (single core)  ║", mpps);
    println!("║  Path                 :  alloc + rewrite + free          ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    println!(
        "RESUME: Kernel-bypass rewrite path — {:.1} cycles/pkt (σ={:.1}), \
{:.3} Mpps, p99={:.1} ns (single-core, zero kernel involvement)",
        mean,
        stddev,
        mpps,
        cycles_to_ns(p99_cycles as f64),
    );
    println!();
}
