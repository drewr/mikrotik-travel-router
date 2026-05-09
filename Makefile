build:
	cargo build --release

check:
	cargo clippy -- -D warnings

.PHONY: build check
