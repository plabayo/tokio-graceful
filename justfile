fmt:
	cargo fmt --all

sort:
	cargo sort --grouped

lint: fmt sort

check:
	cargo check --all-targets

clippy:
	cargo clippy --all-targets

clippy-fix:
	cargo clippy --fix

typos:
	typos -w

doc:
	RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc --no-deps

doc-open:
	RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc --no-deps --open

test:
	cargo test

test-loom:
    RUSTFLAGS="--cfg loom" cargo test test_loom_sender_trigger

qa: lint check clippy doc test test-loom
