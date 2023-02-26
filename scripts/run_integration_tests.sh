#!/bin/bash
set -x

source "${CARGO_HOME:-$HOME/.cargo}/env"
source "$(dirname "$0")/fetch_images.sh"

WORKLOADS_DIR="$HOME/workloads"
mkdir -p "$WORKLOADS_DIR"

CH_PATH="$WORKLOADS_DIR/cloud-hypervisor"
fetch_ch "$CH_PATH"

fetch_disk_images "$WORKLOADS_DIR"

rustup component add rust-src
cargo build --release --target x86_64-unknown-none.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem

export RUST_BACKTRACE=1
time cargo test --features "integration_tests" "integration::tests::linux"
