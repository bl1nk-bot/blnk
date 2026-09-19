#!/usr/bin/env bash
# Universal release artifact builder — creates reproducible release artifacts.
# Currently supports: Rust projects with Cargo.toml
# Usage: scripts/release.sh [--target TRIPLE] [--output-dir DIR] [--skip-build]

set -euo pipefail

target=""
output_dir="dist"
skip_build=false
version=""

while (($# > 0)); do
    case "$1" in
        --target) target="$2"; shift 2 ;;
        --output-dir) output_dir="$2"; shift 2 ;;
        --skip-build) skip_build=true; shift ;;
        -h|--help)
            cat <<'EOF'
Usage: release.sh [--target TRIPLE] [--output-dir DIR] [--skip-build]

Builds and packages a release artifact with PROVENANCE.json.

Options:
  --target TRIPLE    Rust target triple (default: host target)
  --output-dir DIR   Output directory (default: dist)
  --skip-build       Skip build, only package existing binary

Environment:
  GITHUB_SHA         Source commit (default: git rev-parse HEAD)
EOF
            exit 0
            ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

# Detect project type and version
if [[ ! -f "Cargo.toml" ]]; then
    echo "Cargo.toml not found — only Rust projects supported" >&2
    exit 1
fi

version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)
if [[ -z "$version" ]]; then
    echo "could not determine version from Cargo.toml" >&2
    exit 1
fi

toolchain=$(sed -n 's/^channel = "\([^"]*\)"/\1/p' rust-toolchain.toml 2>/dev/null | head -n1 || echo "stable")

if [[ -z "$target" ]]; then
    target=$(rustc -vV | sed -n 's/^host: //p')
fi

if [[ -z "$target" ]]; then
    echo "could not determine target" >&2
    exit 1
fi

# Detect host target for smoke test eligibility
host_target=$(rustc -vV | sed -n 's/^host: //p')
is_cross_compile=false
if [[ "$target" != "$host_target" ]]; then
    is_cross_compile=true
fi

# Run checks unless skipped
if [[ "$skip_build" != true ]]; then
    cargo fmt --all -- --check
    cargo check --all-targets --locked
    cargo test --all --locked
    cargo clippy --all --all-targets --locked -- -D warnings
    cargo build --locked --release --target "$target"
fi

# Determine binary path (handle Windows .exe)
binary="target/${target}/release/blnk"
if [[ "$target" == *"windows"* ]]; then
    binary="${binary}.exe"
fi
if [[ ! -x "$binary" ]]; then
    # Try without target subdir for host builds
    binary="target/release/blnk"
    if [[ "$target" == *"windows"* ]]; then
        binary="${binary}.exe"
    fi
    if [[ ! -x "$binary" ]]; then
        echo "release binary not found: $binary" >&2
        exit 1
    fi
fi

manifest_sha=$(sha256sum Cargo.lock | awk '{print $1}')
source_commit="${GITHUB_SHA:-$(git rev-parse HEAD)}"
package_name="blnk-v${version}-${target}"
stage_root="${output_dir}/.stage/${package_name}"
archive="${output_dir}/${package_name}.tar.gz"
checksum="${archive}.sha256"
smoke_root=$(mktemp -d)

cleanup() {
    rm -rf "$smoke_root" "${output_dir}/.stage"
}
trap cleanup EXIT

# Only remove stage and archive, not the entire output_dir
rm -rf "${output_dir}/.stage" "${output_dir}/${package_name}.tar" "$archive" "$checksum" 2>/dev/null || true
mkdir -p "$stage_root"
install -m 0755 "$binary" "$stage_root/blnk"
install -m 0644 README.md "$stage_root/README.md" 2>/dev/null || true
printf '%s\n' "$version" > "$stage_root/VERSION"

cat > "$stage_root/PROVENANCE.json" <<EOF
{
  "name": "blnk",
  "version": "$version",
  "target": "$target",
  "source_commit": "$source_commit",
  "rust_toolchain": "$toolchain",
  "cargo_lock_sha256": "$manifest_sha",
  "build_command": "cargo build --locked --release --target $target",
  "smoke_command": "./blnk --version"
}
EOF

mkdir -p "$output_dir"
tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner \
    -cf "${output_dir}/${package_name}.tar" -C "${output_dir}/.stage" "$package_name"
gzip -n -9 -c "${output_dir}/${package_name}.tar" > "$archive"
rm -f "${output_dir}/${package_name}.tar"
sha256sum "$archive" > "$checksum"

# Smoke test (skip for cross-compiled targets)
if [[ "$is_cross_compile" == true ]]; then
    echo "skipping smoke test for cross-compiled target $target"
else
    mkdir -p "$smoke_root"
    tar -xzf "$archive" -C "$smoke_root"
    smoke_output=$("$smoke_root/$package_name/blnk" --version)
    expected="blnk $version"
    if [[ "$smoke_output" != "$expected" ]]; then
        echo "release smoke test failed: expected '$expected', got '$smoke_output'" >&2
        exit 1
    fi
fi

printf 'release_version=%s\n' "$version"
printf 'release_target=%s\n' "$target"
printf 'release_artifact=%s\n' "$archive"
printf 'release_checksum=%s\n' "$checksum"