#!/usr/bin/env bash
# Sync Rust version from rust-toolchain.toml to all dependent files.
# Usage: scripts/sync-rust-version.sh [--dry-run]

set -euo pipefail

dry_run=false
if [[ "${1:-}" == "--dry-run" ]]; then
    dry_run=true
fi

# Extract version from rust-toolchain.toml
version=$(sed -n 's/^channel = "\([^"]*\)"/\1/p' rust-toolchain.toml | head -n1)
if [[ -z "$version" ]]; then
    echo "Error: Could not extract version from rust-toolchain.toml" >&2
    exit 1
fi

echo "Syncing Rust version to: $version"

update_file() {
    local file="$1"
    local pattern="$2"
    local replacement="$3"
    
    if [[ ! -f "$file" ]]; then
        echo "Warning: $file not found, skipping" >&2
        return
    fi
    
    if $dry_run; then
        if grep -E -q -- "$pattern" "$file"; then
            echo "Would update $file"
        else
            echo "Pattern not found in $file, skipping"
        fi
    else
        if sed -i -E "s/$pattern/$replacement/" "$file"; then
            echo "Updated $file"
        else
            echo "Error updating $file" >&2
            exit 1
        fi
    fi
}

# flake.nix: pkgs.rust-bin.stable."1.97.0".complete -> pkgs.rust-bin.stable."$version".complete
update_file "flake.nix" \
    'pkgs\.rust-bin\.stable\."[^"]*"\.complete' \
    "pkgs.rust-bin.stable.\"${version}\".complete"

# Dockerfile: --default-toolchain 1.97.0 -> --default-toolchain $version
update_file "Dockerfile" \
    '--default-toolchain[[:space:]]+[0-9.]+' \
    "--default-toolchain ${version}"

# Dockerfile comment: # Install Rust 1.97 via rustup -> # Install Rust $version via rustup
update_file "Dockerfile" \
    '# Install Rust[[:space:]]+[0-9.]+ via rustup' \
    "# Install Rust ${version} via rustup"

echo "Done"