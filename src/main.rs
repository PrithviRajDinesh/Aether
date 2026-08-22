#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
pub mod dpdk {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub mod data_plane;

fn main() {
    println!("Aether Core Infrastructure Booting");
    println!("=========================================");

    data_plane::eal::init_hardware_env();
}
