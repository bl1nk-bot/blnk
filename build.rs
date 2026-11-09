use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-changed=proto");
    println!("cargo:rerun-if-changed=build.rs");

    // Vendor protoc compiler path for deterministic builds
    let protoc_path = protoc_bin_vendored::protoc_bin_path()
        .expect("protoc-bin-vendored must provide a protoc executable");

    // Rust 2024 marks process environment mutation as unsafe because it is
    // process-global. The build script has no concurrent application code.
    unsafe {
        env::set_var("PROTOC", protoc_path);
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set by Cargo"));
    let proto_dir = PathBuf::from("proto");

    // Dynamically discover all .proto files in the proto directory to easily support future features
    let proto_files = discover_proto_files(&proto_dir);

    if !proto_files.is_empty() {
        let mut config = prost_build::Config::new();
        config.out_dir(&out_dir);

        // Derive serde traits for easier debugging and JSON serialization if needed
        config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");

        config
            .compile_protos(&proto_files, &[proto_dir])
            .expect("all protobuf schemas in proto/ must compile successfully");
    }
}

fn is_proto_file(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|ext| ext == "proto")
}

fn discover_proto_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_proto_file(path))
        .collect();

    // Sort to ensure deterministic compilation order
    files.sort();
    files
}
