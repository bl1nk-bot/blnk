.PHONY: fmt clippy test check ci push

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all --all-targets -- -D warnings

test:
	cargo test --all --verbose

check:
	cargo check --all --all-targets

ci: fmt clippy test check

push:
	@echo "ponytail: run \`make ci\` before push."
