use Aether::data_plane::queue;
use Aether::data_plane::{
    tx,
    worker,
};
use Aether::{
    control_plane,
    data_plane,
};


fn main() {

    println!(
        "Aether Core Infrastructure Booting"
    );

    println!(
        "========================================="
    );

    let core_ids =
        core_affinity::get_core_ids()
            .expect(
                "Failed to get CPU core IDs"
            );


    println!(
        "Available Rust CPU cores: {:?}",
        core_ids
    );


    println!(
        "Number of cores: {}",
        core_ids.len()
    );

    if core_ids.len() < 3 {

        panic!(
            "Aether requires at least 3 CPU cores"
        );
    }


    let pmd_core =
        core_ids[0];

    let worker_core =
        core_ids[1];

    let tx_core =
        core_ids[2];

    data_plane::eal::init_hardware_env();

    queue::init_packet_queues();

    println!(
        "========================================="
    );

    println!(
        "Core Assignment:"
    );

    println!(
        "RX/PMD Core       : {:?}",
        pmd_core
    );

    println!(
        "Worker Core       : {:?}",
        worker_core
    );

    println!(
        "TX Core           : {:?}",
        tx_core
    );

    println!(
        "========================================="
    );

    let _worker_handle =
        worker::start_worker(
            worker_core
        );

    let _tx_handle =
        tx::start_tx(
            tx_core
        );

    let _dashboard_handle =
        control_plane::ipc::start_dashboard();

    data_plane::pmd::start_pmd(
        pmd_core
    );
}
