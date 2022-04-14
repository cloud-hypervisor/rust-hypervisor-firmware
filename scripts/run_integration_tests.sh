#!/bin/bash
set -x

source "${CARGO_HOME:-$HOME/.cargo}/env"
source $(dirname "$0")/fetch_disk_images.sh

WORKLOADS_DIR="$HOME/workloads"
mkdir -p "$WORKLOADS_DIR"

CH_VERSION="v23.0"
CH_URL="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/$CH_VERSION/cloud-hypervisor"
CH_PATH="$WORKLOADS_DIR/cloud-hypervisor"
if [ ! -f "$CH_PATH" ]; then
    wget --quiet $CH_URL -O $CH_PATH
    chmod +x $CH_PATH
    sudo setcap cap_net_admin+ep $CH_PATH
fi

fetch_disk_images "$WORKLOADS_DIR"

rustup component add rust-src
cargo build --release --target target.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem

export RUST_BACKTRACE=1
time cargo test --features "integration_tests" "integration::tests::linux"
