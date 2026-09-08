use std::ptr;
use std::thread;
use std::time::{
    Duration,
    Instant,
};

use core_affinity;

use crate::control_plane::telemetry;
use crate::data_plane::queue;
use crate::dpdk;


const PORT_ID: u16 = 0;

const TX_QUEUE_ID: u16 = 0;

const TX_BURST_SIZE: usize = 32;

const TX_BATCH_MAX_WAIT:
    Duration =
    Duration::from_micros(100);


pub fn start_tx(
    core_id: core_affinity::CoreId,
) -> thread::JoinHandle<()> {

    println!(
        "TX Worker          : Starting on {:?}",
        core_id
    );


    thread::spawn(move || {

        let success =
            core_affinity::set_for_current(
                core_id
            );


        if !success {
            panic!(
                "Failed to pin TX worker to {:?}",
                core_id
            );
        }


        println!(
            "TX Affinity        : SUCCESS ({:?})",
            core_id
        );


        println!(
            "TX Pipeline        : STARTED"
        );


        let mut tx_mbufs:
            [*mut dpdk::rte_mbuf; TX_BURST_SIZE] =
            [ptr::null_mut(); TX_BURST_SIZE];


        loop {

            let mut nb_tx: usize = 0;


            let mut batch_started_at:
                Option<Instant> =
                None;

            while nb_tx < TX_BURST_SIZE {

                match queue::tx_packet_queue().pop() {

                    Some(packet) => {

                        tx_mbufs[nb_tx] =
                            packet.as_mbuf();


                        nb_tx += 1;


                        batch_started_at
                            .get_or_insert_with(
                                Instant::now
                            );
                    }


                    None => {

                        if nb_tx == 0 {

                            std::hint::spin_loop();

                            break;
                        }

                        let elapsed =
                            batch_started_at
                                .expect(
                                    "TX batch start missing"
                                )
                                .elapsed();


                        if elapsed >=
                            TX_BATCH_MAX_WAIT {

                            break;
                        }


                        std::hint::spin_loop();
                    }
                }
            }

            if nb_tx == 0 {
                continue;
            }

            let sent =
                unsafe {
                    dpdk::aether_eth_tx_burst(
                        PORT_ID,
                        TX_QUEUE_ID,
                        tx_mbufs.as_mut_ptr(),
                        nb_tx as u16,
                    )
                } as usize;

            if sent > 0 {

                telemetry::increment_packets_transmitted(
                    sent as u64
                );

                telemetry::increment_mbufs_forwarded(
                    sent as u64
                );
            }

            if sent < nb_tx {

                let dropped =
                    nb_tx - sent;


                telemetry::increment_tx_driver_drops(
                    dropped as u64
                );


                for i in sent..nb_tx {

                    unsafe {
                        dpdk::aether_pktmbuf_free(
                            tx_mbufs[i]
                        );
                    }
                }
            }
        }
    })
}
