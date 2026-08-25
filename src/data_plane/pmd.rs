use std::ptr;
use std::thread;

use core_affinity;

use crate::dpdk;

const PORT_ID: u16 = 0;
const RX_QUEUE_ID: u16 = 0;
const BURST_SIZE: usize = 32;

pub fn start_pmd() {
    let core_ids = core_affinity::get_core_ids()
        .expect("Failed to get CPU core IDs");

    if core_ids.is_empty() {
        panic!("No CPU cores available for PMD");
    }

    let core_id = core_ids[0];

    println!(
        "PMD                : Starting on {:?}",
        core_id
    );

    let handle = thread::spawn(move || {

        // Pin PMD thread to its CPU core
        let success = core_affinity::set_for_current(core_id);

        if !success {
            panic!("Failed to pin PMD thread to {:?}", core_id);
        }

        println!(
            "Core Affinity      : SUCCESS ({:?})",
            core_id
        );

        // RX packet pointer array
        let mut rx_mbufs: [*mut dpdk::rte_mbuf; BURST_SIZE] =
            [ptr::null_mut(); BURST_SIZE];

        println!("PMD Polling        : STARTED");

        // PMD polling loop
        loop {
            let nb_rx = unsafe {
                dpdk::aether_eth_rx_burst(
                    PORT_ID,
                    RX_QUEUE_ID,
                    rx_mbufs.as_mut_ptr(),
                    BURST_SIZE as u16,
                )
            };

            core::hint::black_box(nb_rx);
        }
    });

    handle
        .join()
        .expect("PMD thread panicked");
}
