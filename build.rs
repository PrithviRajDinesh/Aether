use std::env;
use std::path::PathBuf;

fn main() {
    let libs = pkg_config::probe_library("libdpdk").unwrap();
    
    println!("cargo:rerun-if-changed=wrapper.h");

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for path in libs.include_paths {
        builder = builder.clone_args(&["-I", path.to_str().unwrap()]);
    }

    let bindings = builder
        .generate()
        .expect("Unable to generate DPDK bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
