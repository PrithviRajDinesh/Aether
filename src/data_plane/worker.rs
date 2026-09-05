use std::thread;
use core_affinity;
use crate::data_plane::queue;
use crate::data_plane::rewrite;
use crate::dpdk;

pub fn start_worker(core_id: core_affinity::CoreId) -> thread::JoinHandle<()> {

    println!("Worker             : Starting on {:?}",core_id);

    thread::spawn(move || {
        let success =
            core_affinity::set_for_current(core_id);

        if !success {
            panic!("Failed to pin worker to {:?}", core_id);
        }

        println!("Worker Affinity    : SUCCESS ({:?})",core_id);

        println!("Worker Processing  : STARTED");

        let backend_ip =
            u32::from_be_bytes([10, 0, 0, 11]);

        loop {

            match queue::rx_packet_queue().pop() {
                Some(packet) => {

                    let mbuf =packet.as_mbuf();

                    // println!("Worker: Processing mbuf {:p}", mbuf);

                    unsafe {
                        rewrite::rewrite_ipv4_tcp_destination(
                            mbuf,
                            backend_ip,
                            8080,
                        )
                    };

                    match queue::tx_packet_queue().push(packet) {

                        Ok(()) => {
                            // println!("Worker -> TX Queue");
                        }

                        Err(packet) => {
                            println!("TX queue full - dropping packet");

                            unsafe {
                                dpdk::aether_pktmbuf_free(
                                    packet.as_mbuf()
                                );
                            }
                        }
                    }
                }

                None => {
                    std::hint::spin_loop();
                }
            }
        }
    })
}
