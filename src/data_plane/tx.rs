use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use core_affinity;

use crate::data_plane::queue;
use crate::dpdk;

const PORT_ID: u16 = 0;
const TX_QUEUE_ID: u16 = 0;

/*
 * Maximum packets collected before calling rte_eth_tx_burst().
 *
 * We use 16 specifically for testing so we can clearly observe:
 *
 * TX: Sent 16/16 packets
 */
const TX_BURST_SIZE: usize = 16;

/*
 * Maximum amount of time to wait for a partial batch to fill.
 *
 * This is intentionally larger for testing purposes. It gives
 * packets enough time to accumulate in the TX queue.
 *
 * In a real high-performance dataplane this value would likely
 * need benchmarking and tuning.
 */
const TX_BATCH_MAX_WAIT: Duration = Duration::from_millis(10);

pub fn start_tx(
    core_id: core_affinity::CoreId,
) -> thread::JoinHandle<()> {
    println!(
        "TX Worker          : Starting on {:?}",
        core_id
    );

    thread::spawn(move || {
        /*
         * Pin the TX worker to its dedicated CPU core.
         */
        let success =
            core_affinity::set_for_current(core_id);

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

        println!("TX Pipeline        : STARTED");

        /*
         * Array used to hold packets before transmission.
         *
         * DPDK expects an array of rte_mbuf pointers.
         */
        let mut tx_mbufs:
            [*mut dpdk::rte_mbuf; TX_BURST_SIZE] =
            [ptr::null_mut(); TX_BURST_SIZE];

        loop {
            let mut nb_tx: usize = 0;

            /*
             * The time when we received the first packet
             * for this batch.
             */
            let mut batch_started_at: Option<Instant> = None;

            /*
             * Collect packets until:
             *
             * 1. The batch becomes full (16 packets), OR
             * 2. The batching timeout expires.
             */
            while nb_tx < TX_BURST_SIZE {
                match queue::tx_packet_queue().pop() {
                    Some(packet) => {
                        /*
                         * Store the DPDK mbuf pointer.
                         */
                        tx_mbufs[nb_tx] =
                            packet.as_mbuf();

                        nb_tx += 1;

                        /*
                         * Start the batching timer when
                         * the first packet arrives.
                         */
                        batch_started_at
                            .get_or_insert_with(Instant::now);
                    }

                    None => {
                        /*
                         * If we already have packets,
                         * check whether the batching
                         * timeout has expired.
                         */
                        if nb_tx > 0 {
                            let elapsed =
                                batch_started_at
                                    .expect(
                                        "Partial batch must have a start time"
                                    )
                                    .elapsed();

                            if elapsed >= TX_BATCH_MAX_WAIT {
                                println!(
                                    "TX Batch           : TIMEOUT ({}/{})",
                                    nb_tx,
                                    TX_BURST_SIZE
                                );

                                break;
                            }

                            /*
                             * Keep spinning while waiting
                             * for additional packets.
                             */
                            std::hint::spin_loop();
                        } else {
                            /*
                             * No packets available yet.
                             */
                            std::hint::spin_loop();
                        }
                    }
                }
            }

            /*
             * If the batch became completely full,
             * print the batching result.
             */
            if nb_tx == TX_BURST_SIZE {
                println!(
                    "TX Batch           : FULL ({}/{})",
                    nb_tx,
                    TX_BURST_SIZE
                );
            }

            /*
             * Nothing was collected.
             *
             * Go back to polling the TX queue.
             */
            if nb_tx == 0 {
                continue;
            }

            /*
             * Send the collected batch to the NIC.
             */
            let sent = unsafe {
                dpdk::aether_eth_tx_burst(
                    PORT_ID,
                    TX_QUEUE_ID,
                    tx_mbufs.as_mut_ptr(),
                    nb_tx as u16,
                )
            } as usize;

            println!(
                "TX: Sent {}/{} packets",
                sent,
                nb_tx
            );

            /*
             * DPDK takes ownership of packets that
             * were successfully transmitted.
             *
             * Packets that were not transmitted
             * must be freed by us.
             */
            if sent < nb_tx {
                let dropped = nb_tx - sent;

                println!(
                    "TX: {} packets not transmitted",
                    dropped
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
