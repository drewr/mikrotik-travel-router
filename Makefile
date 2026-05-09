build:
	cargo build --release

check:
	cargo clippy -- -D warnings

test:
	cargo test

.PHONY: build check test
