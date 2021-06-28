#!/bin/bash
set -xeuf

TARGET="${1:-linux}"

source "${CARGO_HOME:-$HOME/.cargo}/env"

rustup component add rust-src
cargo build --release --target target.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem

CH_VERSION="v15.0"
CH_URL="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/$CH_VERSION/cloud-hypervisor"
CH_PATH="./resources/cloud-hypervisor"
if [ ! -f "$CH_PATH" ]; then
    wget --quiet $CH_URL -O $CH_PATH
    chmod +x $CH_PATH
    sudo setcap cap_net_admin+ep $CH_PATH
fi

if [ "$TARGET" == "linux" ]; then
  bash ./fetch_disk_images.sh
fi

WIN_IMAGE_FILE="./resources/images/windows-server-2019.raw"
if [ -e $WIN_IMAGE_FILE ]; then
  export img_blk_size=$(du -b -B 512 $WIN_IMAGE_FILE | awk '{print $1;}')
  export loop_device=$(sudo losetup --find --show --read-only $WIN_IMAGE_FILE)
  sudo dmsetup create windows-base --table "0 $img_blk_size linear $loop_device 0"
  sudo dmsetup mknodes
  sudo dmsetup create windows-snapshot-base --table "0 $img_blk_size snapshot-origin /dev/mapper/windows-base"
  sudo dmsetup mknodes
fi

# Add the user to the kvm group (if not already in it), so they can run VMs
id -nGz "$USER" | grep -qzxF kvm || sudo adduser "$USER" kvm

newgrp kvm << EOF
export RUST_BACKTRACE=1
cargo test --features "integration_tests" "integration::tests::${TARGET}"
EOF

if [ -e $WIN_IMAGE_FILE ]; then
  sudo dmsetup remove_all -f
  sudo losetup -D
fi
