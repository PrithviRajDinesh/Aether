# Aether

> Kernel-bypass Layer 4 packet-forwarding engine written in Rust, built on DPDK.

Aether is a user-space network data plane that completely bypasses the Linux kernel
networking stack.  Packets are DMA-transferred directly from the NIC into huge-page
memory by DPDK's poll-mode driver, rewritten in-place by Rust workers pinned to
isolated CPU cores, and handed back to the NIC — all without a single system call,
memory allocation, or context switch in the forwarding path.

---

## Table of Contents

- [Why Kernel Bypass](#why-kernel-bypass)
- [Architecture](#architecture)
- [Project Layout](#project-layout)
- [How It Works](#how-it-works)
  - [Data Plane](#data-plane)
  - [Control Plane](#control-plane)
  - [DPDK C Shim](#dpdk-c-shim)
- [Performance](#performance)
- [Prerequisites](#prerequisites)
- [Building](#building)
- [Running](#running)
  - [One-Time System Setup](#one-time-system-setup)
  - [Full Pipeline](#full-pipeline)
  - [Benchmarks](#benchmarks)
  - [Live Dashboard](#live-dashboard)
- [Configuration](#configuration)
- [Telemetry API](#telemetry-api)
- [Binaries](#binaries)
- [Crate Dependencies](#crate-dependencies)

---

## Why Kernel Bypass

Every packet processed by the Linux kernel networking stack pays a fixed overhead:

| Step | Cost |
|------|------|
| `send()` / `recv()` syscall (user↔kernel switch) | ~150–200 ns |
| `sk_buff` allocation off slab | ~80–120 ns |
| Payload copy (user → kernel + DMA) | ~112 ns per 1500 B frame |
| TCP/IP stack walk + routing | ~150–250 ns |
| Softirq scheduling | ~50–100 ns |
| **Total (measured, i5-13450HX)** | **~944 ns mean · 1678 ns p99** |

Aether eliminates every step above.  The NIC DMA-writes packets straight into
pre-allocated mbuf rings; Rust workers read those buffers without ever entering kernel
mode.  The measured forwarding latency on the same hardware drops to **~40 ns mean,
~56 ns p99** — a ~24× improvement at the per-packet level.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Aether Data Plane                            │
│                                                                      │
│  ┌────────────┐    ┌─────────────────────┐    ┌───────────────────┐  │
│  │  PMD / RX  │    │   Worker / Rewrite  │    │    TX Pipeline    │  │
│  │  (core 0)  │───▶│     (core 1)        │───▶│     (core 2)     │  │
│  │            │    │                     │    │                   │  │
│  │ rx_burst() │    │ IPv4 dst IP rewrite │    │ tx_burst()        │  │
│  │  ↓ 32 pkt  │    │ TCP dst port rewrite│    │ 32-pkt burst      │  │
│  │  batches   │    │ HW csum offload     │    │ 100 µs timeout    │  │
│  └────────────┘    └─────────────────────┘    └───────────────────┘  │
│        │                    │ rdtsc timing            │              │
│        ▼                    ▼                         ▼              │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │          ArrayQueue<PacketPtr>   (lock-free, 4096 slots)     │    │
│  │          RX ring ──────────────────── TX ring                │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │                    Control Plane (unpinned)                  │    │
│  │  metrics_sampler (1 Hz) · HTTP dashboard · JSON /metrics     │    │
│  └──────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────┘
         ↑  rte_eth_rx_burst()                rte_eth_tx_burst()  ↑
         │        DPDK PMD  (kernel completely bypassed)          │
    ┌────┴──────────────────────────────────────────────────┐     │
    │                      NIC (DMA rings)                  ├─────┘
    └───────────────────────────────────────────────────────┘
```

### Core pinning

| Core | Role |
|------|------|
| 0 | RX poll-mode driver — calls `rte_eth_rx_burst()` in a tight busy-loop |
| 1 | Worker — pops packets, rewrites IPv4/TCP headers, measures per-packet latency |
| 2 | TX pipeline — batches packets up to 32, calls `rte_eth_tx_burst()` |
| unpinned | Control plane: metrics sampler + HTTP dashboard |

---

## Project Layout

```
Aether/
├── src/
│   ├── main.rs                        # Boot: EAL init → queue init → spawn threads
│   ├── lib.rs                         # DPDK FFI bindings + module exports
│   │
│   ├── data_plane/
│   │   ├── mod.rs
│   │   ├── eal.rs                     # DPDK EAL + port + queue init
│   │   ├── pmd.rs                     # RX poll-mode driver loop (core 0)
│   │   ├── worker.rs                  # Packet worker + rdtsc timing (core 1)
│   │   ├── tx.rs                      # TX burst pipeline (core 2)
│   │   ├── rewrite.rs                 # In-place IPv4 + TCP header rewrite
│   │   ├── parser.rs                  # #[repr(C,packed)] Ethernet/IPv4/TCP/UDP structs
│   │   ├── lb.rs                      # Stateful round-robin LoadBalancer (HashMap)
│   │   ├── queue.rs                   # Lock-free ArrayQueue rings (4096 slots each)
│   │   ├── metrics.rs                 # Per-dataplane AtomicU64 counters + PPS
│   │   ├── dpdk_shim.c                # C shims for DPDK static-inline hot-path functions
│   │   └── ring.rs
│   │
│   ├── control_plane/
│   │   ├── mod.rs
│   │   ├── telemetry.rs               # Global atomic counters + latency sample ring
│   │   ├── metrics_sampler.rs         # 1-Hz thread: percentiles, jitter, queue depth %
│   │   ├── dashboard.rs               # HTTP server: /metrics JSON + HTML dashboard
│   │   └── ipc.rs                     # (legacy) simple IPC dashboard
│   │
│   └── bin/
│       ├── cycles_bench.rs            # Cycles-per-packet microbenchmark (DPDK)
│       ├── flow_bench.rs              # Flow-table Mlookups/sec (DPDK + rdtsc)
│       ├── flow_bench_pure.rs         # Flow-table Mlookups/sec (no DPDK — runs anywhere)
│       ├── kernel_baseline.rs         # Linux kernel socket baseline (no DPDK — runs anywhere)
│       └── naive_forwarder.rs         # AF_PACKET raw-socket forwarder (kernel comparison)
│
├── wrapper.h                          # DPDK shim function declarations for bindgen
├── build.rs                           # Compiles dpdk_shim.c + generates bindings via bindgen
├── Cargo.toml
├── METRICS.md                         # Detailed benchmark guide + résumé bullets
└── scripts/
    └── setup_env.sh
```

---

## How It Works

### Data Plane

#### 1. EAL Initialisation (`eal.rs`)

`init_hardware_env()` calls `rte_eal_init()` with the PCI address from `AETHER_PCI_ID`,
creates the mbuf pool (8192 × 2 MB hugepage-backed buffers), configures one RX queue and
one TX queue with 1024 descriptors each, and starts the Ethernet port in promiscuous mode.

#### 2. RX Poll-Mode Driver (`pmd.rs`)

Runs on **core 0** in an infinite busy-loop.  Calls `aether_eth_rx_burst()` for up to 32
packets per iteration — DPDK batching amortises NIC doorbell writes and PCIe round-trips.
Each received mbuf is wrapped in a zero-cost `PacketPtr` (just stores the raw address) and
pushed into the lock-free `RX_PACKET_QUEUE`.  Packets that find a full queue are dropped
and counted as `rx_queue_drops`.

#### 3. Worker (`worker.rs`)

Runs on **core 1**.  Pops `PacketPtr`s from the RX ring, calls `rewrite_ipv4_tcp_destination()`
**in place** on the mbuf data, then pushes the same pointer into the TX ring.  No copies,
no allocations.  Per-packet `rdtsc` bracketing feeds the latency telemetry ring.

#### 4. Packet Rewrite (`rewrite.rs`)

`rewrite_ipv4_tcp_destination()` parses the on-wire Ethernet + IPv4 + TCP headers using
`read_unaligned` (safe with `#[repr(C, packed)]` structs), overwrites the destination IP
and port bytes in-place with `write_unaligned`, and calls `aether_mbuf_set_tx_checksum_offload()`
so the NIC recalculates the IP and TCP checksums in hardware — no software checksum path.

#### 5. TX Pipeline (`tx.rs`)

Runs on **core 2**.  Accumulates up to 32 packets into a burst array, flushing on a
100 µs timeout if the burst never fills.  Calls `aether_eth_tx_burst()`.  Packets the NIC
driver rejects are freed and counted as `tx_driver_drops`.

#### 6. Lock-Free Queues (`queue.rs`)

Both the RX→Worker and Worker→TX queues are `crossbeam_queue::ArrayQueue<PacketPtr>` —
fixed-capacity, `push`/`pop` are wait-free single-producer single-consumer operations with
no heap allocation.  Capacity: **4096 slots each**.

#### 7. Load Balancer (`lb.rs`)

`LoadBalancer` maintains a `HashMap<FlowKey, usize>` mapping 5-tuple flows to backend
indices and a round-robin `next_backend` cursor for new flows.  All lookups are O(1)
expected-time; there are no kernel locks in the lookup path.

```rust
pub struct FlowKey {
    src_ip: u32, dst_ip: u32,
    src_port: u16, dst_port: u16,
    protocol: u8,
}
```

#### 8. Packet Parser (`parser.rs`)

Defines `#[repr(C, packed)]` structs for Ethernet (14 B), IPv4 (20 B), TCP (20 B), and
UDP (8 B) headers with compile-time `size_of` assertions.  All reads use `read_unaligned`
to handle unaligned NIC DMA buffers safely.

---

### Control Plane

#### Telemetry (`telemetry.rs`)

All counters are `AtomicU64` with `Ordering::Relaxed` — visible across threads without
locking, with no memory-ordering overhead in the fast path.

| Counter | What it counts |
|---------|---------------|
| `PACKETS_RECEIVED` | mbufs returned by `rx_burst` |
| `PACKETS_TRANSMITTED` | mbufs accepted by `tx_burst` |
| `MBUFS_FORWARDED` | zero-copy forwarded packets |
| `RX_QUEUE_DROPS` | packets dropped at RX→Worker queue |
| `TX_QUEUE_DROPS` | packets dropped at Worker→TX queue |
| `TX_DRIVER_DROPS` | packets rejected by the NIC driver |
| `PROCESSING_LATENCY_NS` | accumulated worker latency |
| `PACKETS_PROCESSED` | packets that completed the rewrite path |
| `LATENCY_P50/P99/P999_NS` | percentiles (updated 1 Hz) |
| `LATENCY_JITTER_NS` | p99 − p50 (updated 1 Hz) |
| `RX/TX_QUEUE_DEPTH_PCT` | queue fill % (updated 1 Hz) |

The `LATENCY_SAMPLES` ring (`crossbeam_queue::ArrayQueue<u64>`, 65 536 slots) collects raw
per-packet nanosecond samples.  The metrics sampler drains and sorts it once per second to
compute percentiles.

#### Metrics Sampler (`metrics_sampler.rs`)

Unpinned background thread that wakes every second to:

1. Compute RX/TX PPS from counter deltas.
2. Drain `LATENCY_SAMPLES`, sort, write p50/p99/p999.
3. Compute jitter: `p99.saturating_sub(p50)` → `LATENCY_JITTER_NS`.
4. Read live queue depths, compute `depth * 100 / 4096` → `RX/TX_QUEUE_DEPTH_PCT`.

#### Dashboard (`dashboard.rs`)

Plain TCP HTTP/1.1 server on `127.0.0.1:8080` (no external dependency).

- `GET /` — HTML page with auto-refresh every 1 s, grouped metric table, inline progress
  bars for queue utilisation.
- `GET /metrics` — JSON object with 12 fields (see [Telemetry API](#telemetry-api)).

---

### DPDK C Shim

DPDK's hot-path functions (`rte_eth_rx_burst`, `rte_eth_tx_burst`, `rte_pktmbuf_mtod`, …)
are `static inline` in DPDK headers — they have no linkable symbol and cannot be called
through Rust FFI.  `src/data_plane/dpdk_shim.c` wraps each one in a named C function;
`build.rs` compiles it with `cc::Build` and generates Rust FFI bindings via `bindgen`.

```
wrapper.h          →  bindgen  →  OUT_DIR/bindings.rs   (Rust FFI types + fn signatures)
dpdk_shim.c        →  cc       →  libaether_dpdk_shim.a  (real symbols for the linker)
```

---

## Performance

Measured live on **Intel i5-13450HX** (Arch Linux, DPDK 25.11.1, Rust 1.98.1).

### Forwarding latency vs Linux kernel

| Path | Mean (ns) | p99 (ns) | Stddev (ns) | Single-core Mpps |
|------|:---------:|:--------:|:-----------:|:----------------:|
| Linux `send()` syscall (measured) | 944 | 1,678 | 454 | ~1.06 |
| **Aether DPDK fast path** | **~40** | **~56** | **~5** | **~8.3** |
| **Speedup** | **~24×** | **~30×** | **~87× tighter** | **~8×** |

### Flow-table lookup (measured, no DPDK required)

| | Mean (ns) | p99 (ns) | Throughput |
|-|:---------:|:--------:|:----------:|
| Linux `nf_conntrack` (published) | ~200–400 | — | ~2.5–5 Mlps |
| **Aether HashMap** (100k flows) | **67** | **234** | **14.97 Mlps** |

### Zero-copy forwarding

Every packet forwarded keeps a **zero-copy rate of 100 %** — the same mbuf received
from the NIC is rewritten and returned to the NIC with no additional allocation or copy.
At 10 Gbps this saves ~2.5 GB/s of memcpy bandwidth compared to a kernel proxy.

> See [METRICS.md](./METRICS.md) for full methodology, comparison tables, and
> résumé-ready bullet points.

---

## Prerequisites

| Dependency | Version | Notes |
|------------|---------|-------|
| Rust toolchain | 1.78+ | `rustup update stable` |
| DPDK | 23.x – 25.x | Must expose `libdpdk` via `pkg-config` |
| `libclang` | any | Required by `bindgen` |
| Linux kernel | 5.15+ | `vfio-pci` or `uio_pci_generic` driver |
| Hugepages | 1024 × 2 MB | See setup below |
| A DPDK-compatible NIC | — | Intel e1000/ixgbe/i40e, Mellanox mlx5, virtio, etc. |

Benchmark binaries (`kernel_baseline`, `flow_bench_pure`) have **no system requirements** —
they run on any Linux machine without DPDK or hugepages.

---

## Building

```bash
# Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install DPDK (Arch Linux example)
sudo pacman -S dpdk

# Ubuntu / Debian
# sudo apt install dpdk-dev libdpdk-dev

# Build all binaries (debug)
cargo build --bins

# Build optimised (use for benchmarks and production)
cargo build --release --bins

# Run unit tests (no root / no NIC required)
cargo test
```

The build script (`build.rs`) automatically:
1. Runs `pkg-config --libs libdpdk` to find DPDK.
2. Compiles `src/data_plane/dpdk_shim.c` with `-mssse3`.
3. Generates `bindings.rs` via `bindgen` from `wrapper.h`.

---

## Running

### One-Time System Setup

```bash
# 1. Load the VFIO driver (preferred, IOMMU-safe)
sudo modprobe vfio-pci

# 2. Allocate 1024 × 2 MB hugepages
echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages

# 3. Mount the hugetlbfs
sudo mkdir -p /mnt/huge
sudo mount -t hugetlbfs none /mnt/huge

# 4. Bind your NIC to the DPDK driver
#    Find the PCI address of your NIC first:
sudo dpdk-devbind.py --status-dev net

#    Then bind (replace 0000:02:00.0 with your NIC's address):
sudo dpdk-devbind.py --bind=vfio-pci 0000:02:00.0
```

### Full Pipeline

```bash
# Set the PCI address of the DPDK-bound NIC
export AETHER_PCI_ID=0000:02:00.0

# Run (requires root for hugepages + DPDK)
sudo -E cargo run --release

# Or run the pre-built binary directly
sudo -E ./target/release/Aether
```

Expected boot output:

```
Aether Core Infrastructure Booting
=========================================
DPDK Version Found : DPDK 25.11.1
EAL Boot Status    : SUCCESS (Parsed 6 arguments)
MBUF Pool Status   : SUCCESS (8192 mbufs)
Available Ports    : 1
Port Configuration : SUCCESS (port 0, RX queues 1, TX queues 1)
RX Queue Setup     : SUCCESS (queue 0, 1024 descriptors)
TX Queue Setup     : SUCCESS (queue 0, 1024 descriptors)
Port Start         : SUCCESS (port 0)
Promiscuous Mode   : Enabled  (port 0)
=========================================
Core Assignment:
RX/PMD Core        : CoreId { id: 0 }
Worker Core        : CoreId { id: 1 }
TX Core            : CoreId { id: 2 }
=========================================
Worker Affinity    : SUCCESS (CoreId { id: 1 })
TSC Frequency      : 3600000000 Hz
TX Affinity        : SUCCESS (CoreId { id: 2 })
PMD Polling        : STARTED
Web Dashboard      : http://localhost:8080
```

### Benchmarks

All benchmarks print a formatted table and a one-line **résumé summary** at the end.

```bash
# ── No DPDK / no sudo required ───────────────────────────────────────────────

# Kernel networking baseline (UDP socket send/recv latency)
./target/release/kernel_baseline

# Flow-table lookup throughput (Mlookups/sec, 100k flows)
./target/release/flow_bench_pure

# ── Requires hugepages + sudo ─────────────────────────────────────────────────

# Cycles-per-packet (alloc + IPv4/TCP rewrite + free, 5 M packets)
# Reports: mean/min/max/stddev/p99 cycles, Mpps
sudo ./target/release/cycles_bench

# Flow-table with rdtsc timing (more accurate cycle counts)
sudo ./target/release/flow_bench

# ── Comparison baseline using AF_PACKET raw sockets ──────────────────────────
# (kernel path, requires root + a network interface name)
sudo ./target/release/naive_forwarder eth0
# Metrics exposed on http://localhost:8081/
```

### Live Dashboard

While Aether is running:

```bash
# Open the HTML dashboard (auto-refreshes every 1 s)
xdg-open http://localhost:8080

# Poll the JSON API
watch -n1 "curl -s http://localhost:8080/metrics | python3 -m json.tool"

# Send test traffic
sudo tcpreplay --intf1=<iface> --pps=1000000 flood.pcap
```

---

## Configuration

| Environment variable | Default | Description |
|----------------------|---------|-------------|
| `AETHER_PCI_ID` | *(required)* | PCI address of the DPDK-bound NIC, e.g. `0000:02:00.0` |

Hard-coded constants (edit source to change):

| File | Constant | Default | Meaning |
|------|----------|---------|---------|
| `pmd.rs` | `BURST_SIZE` | 32 | Max packets per `rx_burst` call |
| `tx.rs` | `TX_BURST_SIZE` | 32 | Max packets per `tx_burst` call |
| `tx.rs` | `TX_BATCH_MAX_WAIT` | 100 µs | Partial-burst flush timeout |
| `queue.rs` | `PACKET_QUEUE_SIZE` | 4096 | Slots in each lock-free ring |
| `eal.rs` | `nmbuf` | 8192 | Number of pre-allocated mbufs |
| `eal.rs` | `nb_rx_desc` / `nb_tx_desc` | 1024 | NIC descriptor ring sizes |
| `worker.rs` | `backend_ip` | `10.0.0.11` | Rewrite destination IP |
| `worker.rs` | `backend_port` | `8080` | Rewrite destination TCP port |
| `telemetry.rs` | `LATENCY_SAMPLE_CAPACITY` | 65536 | Per-second latency ring size |

---

## Telemetry API

### `GET /metrics` — JSON

```bash
curl http://localhost:8080/metrics
```

```json
{
  "packets_received":      12400000,
  "packets_transmitted":   12400000,
  "packets_dropped":       0,
  "mbufs_forwarded":       12400000,
  "zero_copy_pct":         100,
  "processing_latency_ns": 41,
  "latency_p50_ns":        38,
  "latency_p99_ns":        97,
  "latency_p999_ns":       312,
  "latency_jitter_ns":     59,
  "rx_queue_depth_pct":    2,
  "tx_queue_depth_pct":    1
}
```

| Field | Description |
|-------|-------------|
| `packets_received` | Cumulative RX packet count |
| `packets_transmitted` | Cumulative TX packet count |
| `packets_dropped` | Sum of RX + TX queue + NIC driver drops |
| `mbufs_forwarded` | Zero-copy forwarded packet count |
| `zero_copy_pct` | `mbufs_forwarded * 100 / packets_received` |
| `processing_latency_ns` | Running average worker latency (ns) |
| `latency_p50_ns` | p50 over last 1 s window |
| `latency_p99_ns` | p99 over last 1 s window |
| `latency_p999_ns` | p999 over last 1 s window |
| `latency_jitter_ns` | p99 − p50 (determinism indicator) |
| `rx_queue_depth_pct` | RX ring fill % (back-pressure indicator) |
| `tx_queue_depth_pct` | TX ring fill % (back-pressure indicator) |

---

## Binaries

| Binary | Needs DPDK | Needs sudo | Purpose |
|--------|:----------:|:----------:|---------|
| `Aether` | ✅ | ✅ | Full kernel-bypass forwarding pipeline |
| `cycles_bench` | ✅ | ✅ | Cycles-per-packet rewrite microbenchmark |
| `flow_bench` | ✅ | ✅ | Flow-table Mlookups/sec with rdtsc |
| `flow_bench_pure` | ❌ | ❌ | Flow-table Mlookups/sec with `Instant` |
| `kernel_baseline` | ❌ | ❌ | Linux kernel socket latency baseline |
| `naive_forwarder` | ❌ | ✅ | AF_PACKET raw-socket forwarder |

---

## Crate Dependencies

| Crate | Version | Used for |
|-------|---------|---------|
| `crossbeam-queue` | 0.3 | `ArrayQueue` — wait-free lock-free ring buffers |
| `core_affinity` | 0.8 | CPU core pinning for data-plane threads |
| `once_cell` | 1 | `Lazy<ArrayQueue>` for the latency sample ring |
| `libc` | 0.2 | Raw syscall FFI (`AF_PACKET`, `recv`, `send`) |
| `anyhow` | 1.0 | Error handling |
| **Build-time** | | |
| `bindgen` | 0.72 | Auto-generates Rust FFI from `wrapper.h` + DPDK headers |
| `cc` | 1.2 | Compiles `dpdk_shim.c` |
| `pkg-config` | 0.3 | Locates DPDK at build time |
