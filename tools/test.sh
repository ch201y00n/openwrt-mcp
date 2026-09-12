#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo run --locked -p xtask -- architecture --base "${1:-HEAD}"
cargo test --locked -p xtask
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
