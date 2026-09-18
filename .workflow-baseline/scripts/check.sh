#!/usr/bin/env bash
# Universal project check script — detects project type and runs all CI checks.
# Works for: Rust (Cargo.toml), Python (pyproject.toml), Node (package.json)
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

Runs all CI checks for the detected project type:
- Rust: fmt + clippy + check + test (unless --skip-tests)
- Python: ruff + mypy + pytest (unless --skip-tests)
- Node: prettier + eslint + tsc + test (unless --skip-tests)

Flags:
  --fix         Apply fixes (fmt, ruff --fix, prettier --write)
  --skip-tests  Skip test step
EOF
            exit 0
            ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

detect_project_type() {
    if [[ -f "Cargo.toml" ]]; then
        echo "rust"
    elif [[ -f "pyproject.toml" ]]; then
        echo "python"
    elif [[ -f "package.json" ]]; then
        echo "node"
    else
        echo "unknown"
    fi
}

run_rust() {
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
}

run_python() {
    if [[ "$fix_mode" == true ]]; then
        ruff check --fix .
        ruff format .
    else
        ruff check .
        ruff format --check .
    fi
    if command -v mypy >/dev/null 2>&1; then
        mypy .
    fi
    if [[ "$skip_tests" != true ]]; then
        pytest
    fi
}

run_node() {
    if [[ "$fix_mode" == true ]]; then
        npx prettier --write .
        npx eslint --fix .
    else
        npx prettier --check .
        npx eslint .
    fi
    if [[ -f "tsconfig.json" ]] && command -v tsc >/dev/null 2>&1; then
        npx tsc --noEmit
    fi
    if [[ "$skip_tests" != true ]]; then
        if [[ -f "package.json" ]] && grep -q '"test"' package.json; then
            npm test
        fi
    fi
}

project_type=$(detect_project_type)

case "$project_type" in
    rust)
        run_rust
        ;;
    python)
        run_python
        ;;
    node)
        run_node
        ;;
    *)
        echo "No recognized project manifest found (Cargo.toml, pyproject.toml, package.json)" >&2
        exit 1
        ;;
esac

echo "==> All checks passed!"