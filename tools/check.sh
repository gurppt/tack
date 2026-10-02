#!/usr/bin/env bash
set -euo pipefail
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo doc --workspace --no-deps --locked
python3 -m unittest discover -s tools -p 'test_*.py'
cargo deny check
