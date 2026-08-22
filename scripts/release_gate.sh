#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/release_gate.sh [--skip-build]

Runs the Linux release gate and writes a deterministic artifact under dist/.
The artifact is never published by this script; it is a local/CI evidence file.
EOF
}

skip_build=false
while (($# > 0)); do
    case "$1" in
        --skip-build)
            skip_build=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
if [[ -z "$version" ]]; then
    echo "could not determine package version from Cargo.toml" >&2
    exit 1
fi

toolchain="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' rust-toolchain.toml | head -n 1)"
if [[ -z "$toolchain" ]]; then
    echo "could not determine Rust toolchain from rust-toolchain.toml" >&2
    exit 1
fi

target="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -z "$target" ]]; then
    echo "could not determine host target" >&2
    exit 1
fi

if [[ "$target" != "x86_64-unknown-linux-gnu" ]]; then
    echo "release_gate.sh is Linux x86_64 only; detected $target" >&2
    exit 1
fi

cargo fmt --all -- --check
cargo check --all-targets --locked
cargo test --all --locked
cargo clippy --all --all-targets --locked -- -D warnings

if [[ "$skip_build" == false ]]; then
    cargo build --locked --release
fi

binary="target/release/blnk"
if [[ ! -x "$binary" ]]; then
    echo "release binary is missing or not executable: $binary" >&2
    exit 1
fi

manifest_sha="$(sha256sum Cargo.lock | awk '{print $1}')"
source_commit="${GITHUB_SHA:-$(git rev-parse HEAD)}"
artifact_root="dist"
package_name="blnk-v${version}-${target}"
stage_root="$artifact_root/.stage/$package_name"
archive="$artifact_root/${package_name}.tar.gz"
checksum="$archive.sha256"
smoke_root="$(mktemp -d)"
cleanup() {
    rm -rf "$smoke_root" "$artifact_root/.stage"
}
trap cleanup EXIT

rm -rf "$artifact_root"
mkdir -p "$stage_root"
install -m 0755 "$binary" "$stage_root/blnk"
install -m 0644 README.md "$stage_root/README.md"
printf '%s\n' "$version" > "$stage_root/VERSION"
cat > "$stage_root/PROVENANCE.json" <<EOF
{
  "name": "blnk",
  "version": "$version",
  "target": "$target",
  "source_commit": "$source_commit",
  "rust_toolchain": "$toolchain",
  "cargo_lock_sha256": "$manifest_sha",
  "build_command": "cargo build --locked --release",
  "smoke_command": "./blnk --version"
}
EOF

mkdir -p "$artifact_root"
tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner \
    -cf "$artifact_root/${package_name}.tar" -C "$artifact_root/.stage" "$package_name"
gzip -n -9 -c "$artifact_root/${package_name}.tar" > "$archive"
rm -f "$artifact_root/${package_name}.tar"
sha256sum "$archive" > "$checksum"

mkdir -p "$smoke_root"
tar -xzf "$archive" -C "$smoke_root"
smoke_output="$("$smoke_root/$package_name/blnk" --version)"
expected="blnk $version"
if [[ "$smoke_output" != "$expected" ]]; then
    echo "release smoke test failed: expected '$expected', got '$smoke_output'" >&2
    exit 1
fi

printf 'release_gate_status=passed\n'
printf 'release_version=%s\n' "$version"
printf 'release_target=%s\n' "$target"
printf 'release_artifact=%s\n' "$archive"
printf 'release_checksum=%s\n' "$checksum"
