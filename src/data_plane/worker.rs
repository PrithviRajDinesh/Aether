use std::thread;

use core_affinity;

use crate::control_plane::telemetry;
use crate::data_plane::queue;
use crate::data_plane::rewrite;
use crate::dpdk;


pub fn start_worker(
    core_id: core_affinity::CoreId,
) -> thread::JoinHandle<()> {

    println!(
        "Worker             : Starting on {:?}",
        core_id
    );


    thread::spawn(move || {

        let success =
            core_affinity::set_for_current(
                core_id
            );


        if !success {
            panic!(
                "Failed to pin worker to {:?}",
                core_id
            );
        }


        println!(
            "Worker Affinity    : SUCCESS ({:?})",
            core_id
        );


        println!(
            "Worker Processing  : STARTED"
        );

        let backend_ip =
            u32::from_be_bytes(
                [10, 0, 0, 11]
            );

        let tsc_hz =
            unsafe {
                dpdk::aether_get_tsc_hz()
            };


        println!(
            "TSC Frequency      : {} Hz",
            tsc_hz
        );


        loop {

            match queue::rx_packet_queue().pop() {

                Some(packet) => {

                    let mbuf =
                        packet.as_mbuf();

                    let start_cycles =
                        unsafe {
                            dpdk::aether_rdtsc()
                        };

                    unsafe {
                        rewrite::rewrite_ipv4_tcp_destination(
                            mbuf,
                            backend_ip,
                            8080,
                        );
                    }

                    let end_cycles =
                        unsafe {
                            dpdk::aether_rdtsc()
                        };

                    let elapsed_cycles =
                        end_cycles
                            .wrapping_sub(
                                start_cycles
                            );


                    let latency_ns =
                        elapsed_cycles
                            .saturating_mul(
                                1_000_000_000
                            )
                            / tsc_hz;


                    telemetry::record_processing_latency_ns(
                        latency_ns
                    );

                    match queue::tx_packet_queue().push(
                        packet
                    ) {

                        Ok(()) => {}


                        Err(packet) => {

                            telemetry::increment_tx_queue_drops(
                                1
                            );


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
