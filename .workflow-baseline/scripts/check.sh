#!/usr/bin/env bash
# Rust project CI check script — runs all local CI checks.
# Usage: scripts/check.sh [--fix] [--skip-tests]

set -euo pipefail

fix_mode=false
skip_tests=false

while (($# > 0)); do
    case "$1" in
        --fix) fix_mode=true; shift ;;
        --skip-tests) skip_tests=true; shift ;;
        -h|--help)
            cat <<'EOF'
Usage: check.sh [--fix] [--skip-tests]

Runs all CI checks for Rust project:
- fmt + clippy + check + test (unless --skip-tests)

Flags:
  --fix         Apply fixes (cargo fmt)
  --skip-tests  Skip test step
EOF
            exit 0
            ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [[ "$fix_mode" == true ]]; then
    cargo fmt --all
else
    cargo fmt --all -- --check
fi

cargo clippy --all --all-targets -- -D warnings
cargo check --all --all-targets

if [[ "$skip_tests" != true ]]; then
    cargo test --all
fi

echo "==> All checks passed!"