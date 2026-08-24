# Contributing to blnk

Thank you for your interest in contributing to **blnk**!

## Development Workflow

1. Fork and clone the repository.
2. Ensure you have the stable Rust toolchain installed:
   ```bash
   cargo --version
   ```
3. Run tests locally before opening a pull request:
   ```bash
   cargo fmt --all -- --check
   cargo check --all-targets
   cargo test --all
   cargo clippy --all --all-targets -- -D warnings
   ```

## Pull Request Lifecycle

- Target the `main` branch.
- Each feature or fix should be associated with an issue (`Closes #<issue>`).
- Bump patch version in `Cargo.toml` and add an entry under `CHANGELOG.md`.
- Automated CI enforces zero clippy warnings and cross-platform compilation.
