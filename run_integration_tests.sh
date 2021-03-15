#!/bin/bash
set -xeuf

source "${CARGO_HOME:-$HOME/.cargo}/env"

rustup component add rust-src
cargo build --release --target target.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem

CH_VERSION="v0.13.0"
CH_URL="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/$CH_VERSION/cloud-hypervisor"
CH_PATH="./resources/cloud-hypervisor"
if [ ! -f "$CH_PATH" ]; then
    wget --quiet $CH_URL -O $CH_PATH
    chmod +x $CH_PATH
    sudo setcap cap_net_admin+ep $CH_PATH
fi

bash ./fetch_disk_images.sh

# Add the user to the kvm group (if not already in it), so they can run VMs
id -nGz "$USER" | grep -qzxF kvm || sudo adduser "$USER" kvm

newgrp kvm << EOF
export RUST_BACKTRACE=1
cargo test --features "integration_tests"  -- --test-threads=1 test_boot
EOF
