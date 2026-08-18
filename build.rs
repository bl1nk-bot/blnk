use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=proto");
    println!("cargo:rerun-if-changed=build.rs");

    let protoc_path = protoc_bin_vendored::protoc_bin_path()
        .expect("protoc-bin-vendored must provide a protoc executable");
    // Rust 2024 marks process environment mutation as unsafe because it is
    // process-global. The build script has no concurrent application code.
    unsafe {
        env::set_var("PROTOC", protoc_path);
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set by Cargo"));
    let proto_files = [
        "proto/control.proto",
        "proto/identity.proto",
        "proto/pairing.proto",
        "proto/signaling.proto",
        "proto/stream.proto",
        "proto/swsp.proto",
    ];

    let mut config = prost_build::Config::new();
    config.out_dir(out_dir);
    config
        .compile_protos(&proto_files, &["proto"])
        .expect("all checked-in protobuf schemas must compile");
}
