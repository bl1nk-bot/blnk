# Justfile for blnk (modern command runner)
# Install just: cargo install just or winget install Casey.Just

set shell := ["bash", "-c"]

# Default recipe: list available commands
default:
    @just --list

# Format check
fmt:
    cargo fmt --all -- --check

# Format and apply fixes
fmt-fix:
    cargo fmt --all

# Run Clippy linter
clippy:
    cargo clippy --all --all-targets -- -D warnings

# Run all tests
test:
    cargo test --all --verbose

# Run fast check on all targets
check:
    cargo check --all --all-targets

# Run all local CI verification checks
ci: fmt clippy check test
    @echo "==> All CI checks passed!"

# Build release binary
build-release:
    cargo build --release --locked

# Run benchmarks
bench:
    cargo bench

# Clean build artifacts
clean:
    cargo clean
