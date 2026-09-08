/// kernel_baseline — measures the per-packet cost of the Linux kernel networking path
/// for a comparable operation: loopback UDP send+recv on the same machine.
///
/// This establishes the "before kernel bypass" baseline so we can show the
/// speedup Aether achieves by eliminating syscall overhead, scheduler wakeups,
/// skb allocation, and kernel TCP/IP stack processing.

use std::net::UdpSocket;
use std::time::Instant;

const PAYLOAD_LEN: usize = 40; // ~TCP-payload-sized packet, no IP/Eth headers needed
const NUM_PACKETS: u64 = 1_000_000;
const WARMUP_PACKETS: u64 = 10_000;

fn main() {
    // ── Setup ─────────────────────────────────────────────────────────────────
    let sender   = UdpSocket::bind("127.0.0.1:0").expect("bind sender");
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("bind receiver");
    receiver.set_nonblocking(true).expect("set_nonblocking");

    let recv_addr = receiver.local_addr().expect("local_addr");
    sender.connect(recv_addr).expect("connect sender");

    let payload = [0xAB_u8; PAYLOAD_LEN];
    let mut recv_buf = [0u8; 65536];

    // ── Warm-up ───────────────────────────────────────────────────────────────
    for _ in 0..WARMUP_PACKETS {
        let _ = sender.send(&payload);
        let _ = receiver.recv(&mut recv_buf);
    }

    // ── Measure: per-packet send() syscall latency ────────────────────────────
    // This models what Aether replaces with rte_eth_tx_burst():
    // one syscall per packet, kernel allocates an sk_buff, walks the TCP/IP
    // stack, copies data, schedules the softirq.
    let mut send_samples: Vec<u64> = Vec::with_capacity(NUM_PACKETS as usize);

    for _ in 0..NUM_PACKETS {
        let t0 = Instant::now();
        let _ = sender.send(&payload);
        let elapsed_ns = t0.elapsed().as_nanos() as u64;
        send_samples.push(elapsed_ns);
        // drain receiver so the socket buffer doesn't fill
        let _ = receiver.recv(&mut recv_buf);
    }

    // ── Measure: round-trip latency (send + recv syscall pair) ────────────────
    // Full loopback RTT ≈ what a L4 proxy sees when using the kernel network stack
    let mut rtt_samples: Vec<u64> = Vec::with_capacity(100_000);
    let receiver_blocking = UdpSocket::bind("127.0.0.1:0").expect("bind receiver2");
    let sender2 = UdpSocket::bind("127.0.0.1:0").expect("bind sender2");
    sender2.connect(receiver_blocking.local_addr().unwrap()).unwrap();

    for _ in 0..100_000 {
        let t0 = Instant::now();
        let _ = sender2.send(&payload);
        let _ = receiver_blocking.recv(&mut recv_buf);
        rtt_samples.push(t0.elapsed().as_nanos() as u64);
    }

    // ── Statistics helper ─────────────────────────────────────────────────────
    fn stats(samples: &mut Vec<u64>) -> (f64, u64, u64, f64, u64) {
        let n = samples.len() as f64;
        let mean  = samples.iter().sum::<u64>() as f64 / n;
        let min   = *samples.iter().min().unwrap();
        let max   = *samples.iter().max().unwrap();
        let var   = samples.iter().map(|&x| { let d = x as f64 - mean; d*d }).sum::<f64>() / n;
        let stddev = var.sqrt();
        samples.sort_unstable();
        let p99_idx = ((n - 1.0) * 0.99).round() as usize;
        let p99 = samples[p99_idx];
        (mean, min, max, stddev, p99)
    }

    let (s_mean, s_min, s_max, s_stddev, s_p99) = stats(&mut send_samples);
    let (r_mean, _, _, _, r_p99) = stats(&mut rtt_samples);

    // ── Throughput (single-core send syscall path) ────────────────────────────
    let send_mpps = 1_000.0 / s_mean; // Mpps: packets/(us) = 1000/mean_ns * 1e-6 * 1e6

    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║      Linux Kernel Networking — Baseline Measurements     ║");
    println!("║      (loopback UDP, {} packets, {} B payload)    ║", NUM_PACKETS, PAYLOAD_LEN);
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  send() syscall latency per packet                        ║");
    println!("║    mean               : {:>10.1} ns                       ║", s_mean);
    println!("║    min                : {:>10} ns                       ║", s_min);
    println!("║    max                : {:>10} ns                       ║", s_max);
    println!("║    stddev (σ)         : {:>10.1} ns                       ║", s_stddev);
    println!("║    p99                : {:>10} ns                       ║", s_p99);
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Round-trip (send+recv) latency                           ║");
    println!("║    mean               : {:>10.1} ns                       ║", r_mean);
    println!("║    p99                : {:>10} ns                       ║", r_p99);
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Estimated single-core throughput                         ║");
    println!("║    ~{:.3} Mpps (1 syscall/pkt, non-batched)          ║", send_mpps);
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("NOTE: These numbers represent the Linux kernel socket API overhead.");
    println!("      Aether bypasses every part of this path using DPDK.");
    println!();
}
