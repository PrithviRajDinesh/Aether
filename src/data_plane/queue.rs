use std::sync::OnceLock;
use crossbeam_queue::ArrayQueue;
use crate::dpdk;
use std::sync::atomic::{fence, Ordering};

pub const PACKET_QUEUE_SIZE: usize = 4096;

#[derive(Copy, Clone)]
pub struct PacketPtr(pub *mut dpdk::rte_mbuf);

unsafe impl Send for PacketPtr {}

unsafe impl Sync for PacketPtr {}

static PACKET_QUEUE: OnceLock<ArrayQueue<PacketPtr>> =
    OnceLock::new();

pub fn init_packet_queue() {
    PACKET_QUEUE
        .set(ArrayQueue::new(PACKET_QUEUE_SIZE))
        .expect("Packet queue already initialized");

    println!(
        "Packet Queue       : SUCCESS (capacity {})",
        PACKET_QUEUE_SIZE
    );
}

fn packet_queue() -> &'static ArrayQueue<PacketPtr> {
    PACKET_QUEUE
        .get()
        .expect("Packet queue not initialized")
}

pub fn enqueue_packet(packet: PacketPtr) -> Result<(), PacketPtr> {
    //Ensure that all writes are performed before handing the packet to another core
    fence(Ordering::Release);
    packet_queue().push(packet)
}

pub fn dequeue_packet() -> Option<PacketPtr> {
    let packet = packet_queue().pop();

    if packet.is_some() {
        //Ensure writes are visible before processing
        fence(Ordering::Acquire);
    }
    packet
}
