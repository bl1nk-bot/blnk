use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=proto");
    println!("cargo:rerun-if-changed=build.rs");

    // Try vendored protoc first; fall back to system protoc or PROTOC env var.
    // protoc-bin-vendored does not ship binaries for all targets (e.g. aarch64-android).
    let protoc_path = match protoc_bin_vendored::protoc_bin_path() {
        Ok(path) => path,
        Err(vendored_err) => resolve_system_protoc(vendored_err),
    };

    // Rust 2024 marks process environment mutation as unsafe because it is
    // process-global. The build script has no concurrent application code.
    unsafe {
        env::set_var("PROTOC", &protoc_path);
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set by Cargo"));
    let proto_dir = PathBuf::from("proto");

    // Dynamically discover all .proto files in the proto directory to easily support future features
    let mut proto_files = Vec::new();
    if proto_dir.exists()
        && proto_dir.is_dir()
        && let Ok(entries) = fs::read_dir(&proto_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("proto") {
                proto_files.push(path);
            }
        }
    }

    // Sort to ensure deterministic compilation order
    proto_files.sort();

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

/// Resolve a usable protoc binary when vendored is unavailable.
fn resolve_system_protoc(vendored_err: impl std::fmt::Display) -> PathBuf {
    // Respect PROTOC env var if already set to a non-empty value.
    if let Ok(existing) = env::var("PROTOC") {
        if !existing.is_empty() {
            println!(
                "cargo:warning=protoc-bin-vendored unavailable ({vendored_err}); \
                 using PROTOC={existing}"
            );
            return PathBuf::from(existing);
        }
    }

    // Try `which protoc` to find a system binary.
    if let Ok(output) = std::process::Command::new("which").arg("protoc").output()
        && output.status.success()
    {
        let candidate = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !candidate.is_empty() {
            println!(
                "cargo:warning=protoc-bin-vendored unavailable ({vendored_err}); \
                 using system protoc at {candidate}"
            );
            return PathBuf::from(candidate);
        }
    }

    panic!(
        "protoc-bin-vendored must provide a protoc executable: {vendored_err}. \
         Set the PROTOC environment variable to the path of a protoc binary, \
         or install protoc (e.g. `pkg install protobuf` on Termux)."
    );
}
