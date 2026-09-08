use std::io::Write;
use std::mem;
use std::net::TcpListener;
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Instant;

use libc::{c_void, sockaddr_ll, AF_PACKET, ETH_P_ALL, SOCK_RAW};

static PACKETS_RECEIVED: AtomicU64 = AtomicU64::new(0);
static PACKETS_DROPPED: AtomicU64 = AtomicU64::new(0);
static TOTAL_LATENCY_NS: AtomicU64 = AtomicU64::new(0);
static PACKETS_PROCESSED: AtomicU64 = AtomicU64::new(0);

const ETHERTYPE_IPV4: u16 = 0x0800;
const IP_PROTOCOL_TCP: u8 = 6;

fn htons(v: u16) -> u16 {
    v.to_be()
}

unsafe fn open_raw_socket(ifname: &str) -> RawFd {
    let fd = libc::socket(AF_PACKET, SOCK_RAW, htons(ETH_P_ALL as u16) as i32);
    assert!(fd >= 0, "Failed to open raw socket (are you root?)");

    let mut ifreq: libc::ifreq = mem::zeroed();
    for (i, b) in ifname.bytes().enumerate() {
        ifreq.ifr_name[i] = b as i8;
    }
    assert!(
        libc::ioctl(fd, libc::SIOCGIFINDEX, &mut ifreq) >= 0,
        "SIOCGIFINDEX failed for {}",
        ifname
    );
    let ifindex = ifreq.ifr_ifru.ifru_ifindex;

    let mut sll: sockaddr_ll = mem::zeroed();
    sll.sll_family = AF_PACKET as u16;
    sll.sll_protocol = htons(ETH_P_ALL as u16);
    sll.sll_ifindex = ifindex;

    assert!(
        libc::bind(
            fd,
            &sll as *const sockaddr_ll as *const libc::sockaddr,
            mem::size_of::<sockaddr_ll>() as u32,
        ) >= 0,
        "bind() failed on {}",
        ifname
    );

    fd
}

/// Identical rewrite semantics to data_plane::rewrite::rewrite_ipv4_tcp_destination,
/// operating on a raw frame buffer instead of an rte_mbuf. Kept as a faithful
/// byte-for-byte port so the comparison against the DPDK path is apples-to-apples.
fn rewrite_and_forward(buf: &mut [u8], dst_ip: u32, dst_port: u16) -> bool {
    const ETH_LEN: usize = 14;
    if buf.len() < ETH_LEN {
        return false;
    }

    let ether_type = u16::from_be_bytes([buf[12], buf[13]]);
    if ether_type != ETHERTYPE_IPV4 {
        return false;
    }

    if buf.len() < ETH_LEN + 20 {
        return false;
    }

    let version_ihl = buf[ETH_LEN];
    if version_ihl >> 4 != 4 {
        return false;
    }
    let ihl = (version_ihl & 0x0f) as usize;
    let ipv4_header_len = ihl * 4;
    if ipv4_header_len < 20 || buf.len() < ETH_LEN + ipv4_header_len {
        return false;
    }

    if buf[ETH_LEN + 9] != IP_PROTOCOL_TCP {
        return false;
    }

    let tcp_off = ETH_LEN + ipv4_header_len;
    if buf.len() < tcp_off + 20 {
        return false;
    }

    buf[ETH_LEN + 16..ETH_LEN + 20].copy_from_slice(&dst_ip.to_be_bytes());
    buf[tcp_off + 2..tcp_off + 4].copy_from_slice(&dst_port.to_be_bytes());

    true
}

fn start_metrics_server() {
    thread::spawn(|| {
        let listener =
            TcpListener::bind("127.0.0.1:8081").expect("Failed to bind metrics port 8081");

        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let received = PACKETS_RECEIVED.load(Ordering::Relaxed);
                let dropped = PACKETS_DROPPED.load(Ordering::Relaxed);
                let processed = PACKETS_PROCESSED.load(Ordering::Relaxed);
                let total_lat = TOTAL_LATENCY_NS.load(Ordering::Relaxed);
                let avg_lat = if processed > 0 { total_lat / processed } else { 0 };

                let body = format!(
                    "{{\"packets_received\":{},\"packets_dropped\":{},\"processing_latency_ns\":{}}}",
                    received, dropped, avg_lat
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        }
    });
}

fn main() {
    let ifname = std::env::args()
        .nth(1)
        .expect("Usage: naive_forwarder <ifname>");

    let backend_ip = u32::from_be_bytes([10, 0, 0, 11]);
    let backend_port: u16 = 8080;

    start_metrics_server();

    let fd = unsafe { open_raw_socket(&ifname) };
    let mut buf = [0u8; 2048];

    println!("Naive AF_PACKET forwarder listening on {} (metrics on :8081)", ifname);

    loop {
        let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut c_void, buf.len(), 0) };
        if n <= 0 {
            continue;
        }
        let n = n as usize;
        PACKETS_RECEIVED.fetch_add(1, Ordering::Relaxed);

        let start = Instant::now();
        let ok = rewrite_and_forward(&mut buf[..n], backend_ip, backend_port);
        let elapsed_ns = start.elapsed().as_nanos() as u64;

        if !ok {
            PACKETS_DROPPED.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        let sent = unsafe { libc::send(fd, buf.as_ptr() as *const c_void, n, 0) };
        if sent < 0 {
            PACKETS_DROPPED.fetch_add(1, Ordering::Relaxed);
        } else {
            TOTAL_LATENCY_NS.fetch_add(elapsed_ns, Ordering::Relaxed);
            PACKETS_PROCESSED.fetch_add(1, Ordering::Relaxed);
        }
    }
}
