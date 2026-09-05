use std::ptr;
use std::thread;
use core_affinity;
use crate::data_plane::queue;
use crate::dpdk;

const PORT_ID: u16 = 0;
const RX_QUEUE_ID: u16 = 0;

const BURST_SIZE: usize = 32;

pub fn start_pmd(
    core_id: core_affinity::CoreId,
) {
    println!(
        "PMD                : Starting on {:?}",
        core_id
    );

    let handle = thread::spawn(move || {
        let success =
            core_affinity::set_for_current(core_id);

        if !success {
            panic!(
                "Failed to pin PMD thread to {:?}",
                core_id
            );
        }

        println!(
            "Core Affinity      : SUCCESS ({:?})",
            core_id
        );

        let mut rx_mbufs:
            [*mut dpdk::rte_mbuf; BURST_SIZE] =
            [ptr::null_mut(); BURST_SIZE];

        println!("PMD Polling        : STARTED");

        loop {
            let nb_rx = unsafe {
                dpdk::aether_eth_rx_burst(
                    PORT_ID,
                    RX_QUEUE_ID,
                    rx_mbufs.as_mut_ptr(),
                    BURST_SIZE as u16,
                )
            };

            if nb_rx > 0 {
                for i in 0..nb_rx as usize {
                    let mbuf = rx_mbufs[i];
                    let packet =
                        queue::PacketPtr::new(mbuf);

                    match queue::rx_packet_queue().push(packet) {
                        Ok(()) => {
                            // println!("PMD -> Worker Queue");
                        }

                        Err(packet) => {
                            println!(
                                "RX queue full - dropping packet"
                            );

                            unsafe {
                                dpdk::aether_pktmbuf_free(
                                    packet.as_mbuf()
                                );
                            }
                        }
                    }
                }
            }
            core::hint::black_box(nb_rx);
        }
    });
    handle
        .join()
        .expect("PMD thread panicked");
}
