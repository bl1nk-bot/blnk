{
  description = "blnk — P2P remote access multitool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Rust toolchain from rust-toolchain.toml
        rustToolchain = pkgs.rust-bin.stable."1.97.0".complete;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            # Rust toolchain
            rustToolchain

            # System dependencies
            pkgs.protobuf           # protoc for prost-build
            pkgs.pkg-config         # for native deps
            pkgs.openssl            # for TLS (reqwest)
            pkgs.curl               # for install scripts
            pkgs.git

            # Android cross-compile (optional, via ANDROID_NDK)
            # pkgs.android-tools

            # Windows cross-compile (optional, via mingw)
            # pkgs.mingw64Packages.gcc
          ];

          shellHook = ''
            echo "blnk dev shell — Rust $(rustc --version)"
            echo "  cargo check --workspace"
            echo "  cargo test --workspace"
            echo "  just ci"
          '';
        };
      }
    );
}
