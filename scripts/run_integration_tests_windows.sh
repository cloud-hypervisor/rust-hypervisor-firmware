#!/bin/bash
set -x

source "${CARGO_HOME:-$HOME/.cargo}/env"

WORKLOADS_DIR="$HOME/workloads"
mkdir -p "$WORKLOADS_DIR"

WIN_IMAGE_FILE="$WORKLOADS_DIR/windows-server-2019.raw"

CH_VERSION="v20.0"
CH_URL="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/$CH_VERSION/cloud-hypervisor"
CH_PATH="$WORKLOADS_DIR/cloud-hypervisor"
if [ ! -f "$CH_PATH" ]; then
    wget --quiet $CH_URL -O $CH_PATH
    chmod +x $CH_PATH
    sudo setcap cap_net_admin+ep $CH_PATH
fi

# Check if the image is present
if [ ! -f "$WIN_IMAGE_FILE" ]; then
    echo "Windows image not present in the host"
    exit 1
fi

# Use device mapper to create a snapshot of the Windows image
img_blk_size=$(du -b -B 512 ${WIN_IMAGE_FILE} | awk '{print $1;}')
loop_device=$(losetup --find --show --read-only ${WIN_IMAGE_FILE})
dmsetup create windows-base --table "0 $img_blk_size linear $loop_device 0"
dmsetup mknodes
dmsetup create windows-snapshot-base --table "0 $img_blk_size snapshot-origin /dev/mapper/windows-base"
dmsetup mknodes

rustup component add rust-src
cargo build --release --target target.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem

export RUST_BACKTRACE=1
time cargo test --features "integration_tests" "integration::tests::windows"
RES=$?

dmsetup remove_all -f
losetup -D

exit $RES
