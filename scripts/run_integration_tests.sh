#!/bin/bash
set -x

source "${CARGO_HOME:-$HOME/.cargo}/env"
source "$(dirname "$0")/fetch_images.sh"

arch="$(uname -m)"

WORKLOADS_DIR="$HOME/workloads"
mkdir -p "$WORKLOADS_DIR"

CH_PATH="$WORKLOADS_DIR/cloud-hypervisor"
fetch_ch "$CH_PATH" "$arch"

fetch_disk_images "$WORKLOADS_DIR" "$arch"

[ "$arch" = "aarch64" ] && target="aarch64-unknown-none"
[ "$arch" = "x86_64" ] && target="x86_64-unknown-none"

rustup component add rust-src
cargo build --release --target "$target.json" -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem

export RUST_BACKTRACE=1
time cargo test --features "integration_tests" "integration::tests::linux::$arch" -- --test-threads=1
