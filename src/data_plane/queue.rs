use std::sync::OnceLock;

use crossbeam_queue::ArrayQueue;

use crate::dpdk;

pub const PACKET_QUEUE_SIZE: usize = 4096;

pub struct PacketPtr {
    addr: usize,
}

unsafe impl Send for PacketPtr {}
unsafe impl Sync for PacketPtr {}

impl PacketPtr {
    pub fn new(mbuf: *mut dpdk::rte_mbuf) -> Self {
        Self {
            addr: mbuf as usize,
        }
    }

    pub fn as_mbuf(&self) -> *mut dpdk::rte_mbuf {
        self.addr as *mut dpdk::rte_mbuf
    }
}

static RX_PACKET_QUEUE: OnceLock<ArrayQueue<PacketPtr>> =
    OnceLock::new();

static TX_PACKET_QUEUE: OnceLock<ArrayQueue<PacketPtr>> =
    OnceLock::new();

pub fn init_packet_queues() {
    RX_PACKET_QUEUE
        .set(ArrayQueue::new(PACKET_QUEUE_SIZE))
        .expect("RX packet queue already initialized");

    TX_PACKET_QUEUE
        .set(ArrayQueue::new(PACKET_QUEUE_SIZE))
        .expect("TX packet queue already initialized");

    println!(
        "RX Packet Queue    : SUCCESS (capacity {})",
        PACKET_QUEUE_SIZE
    );

    println!(
        "TX Packet Queue    : SUCCESS (capacity {})",
        PACKET_QUEUE_SIZE
    );
}

pub fn rx_packet_queue() -> &'static ArrayQueue<PacketPtr> {
    RX_PACKET_QUEUE
        .get()
        .expect("RX packet queue not initialized")
}

pub fn tx_packet_queue() -> &'static ArrayQueue<PacketPtr> {
    TX_PACKET_QUEUE
        .get()
        .expect("TX packet queue not initialized")
}
