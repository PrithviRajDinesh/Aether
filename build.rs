// build.rs
use std::env;
use std::path::PathBuf;

fn main() {
    let libs = pkg_config::probe_library("libdpdk").unwrap();
    println!("cargo:rerun-if-changed=wrapper.h");

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .derive_debug(false)   // Fixes E0277: Stops bindgen from choking on complex C unions
        .layout_tests(false)  // Speeds up builds by ignoring C-to-Rust alignment assertion tests
        
        // Fixes E0588: Treats problematic protocol headers as raw byte chunks 
        .opaque_type("rte_arp_ipv4")
        .opaque_type("rte_arp_hdr")
        .opaque_type("rte_l2tpv2_combined_msg_hdr")
        .opaque_type("rte_l2tpv2_common_hdr");

    for path in libs.include_paths {
        builder = builder.clang_arg(format!("-I{}", path.to_str().unwrap()));
    }

    let bindings = builder
        .generate()
        .expect("Unable to generate DPDK bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
