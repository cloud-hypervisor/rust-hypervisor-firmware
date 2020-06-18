#!/bin/bash
set -x

source $HOME/.cargo/env

CH_VERSION="v0.8.0"
rm cloud-hypervisor
wget --quiet "https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/$CH_VERSION/cloud-hypervisor" || exit 1
chmod +x cloud-hypervisor

WORKLOADS_DIR="$HOME/workloads"
mkdir -p "$WORKLOADS_DIR"


CLEAR_OS_IMAGE_NAME="clear-31311-cloudguest.img"
CLEAR_OS_IMAGE_URL="https://cloudhypervisorstorage.blob.core.windows.net/images/$CLEAR_OS_IMAGE_NAME"
CLEAR_OS_IMAGE="$WORKLOADS_DIR/$CLEAR_OS_IMAGE_NAME"
if [ ! -f "$CLEAR_OS_IMAGE" ]; then
    pushd $WORKLOADS_DIR
    wget --quiet $CLEAR_OS_IMAGE_URL || exit 1
    popd
fi

BIONIC_OS_IMAGE_NAME="bionic-server-cloudimg-amd64.img"
BIONIC_OS_IMAGE_URL="https://cloudhypervisorstorage.blob.core.windows.net/images/$BIONIC_OS_IMAGE_NAME"
BIONIC_OS_IMAGE="$WORKLOADS_DIR/$BIONIC_OS_IMAGE_NAME"
if [ ! -f "$BIONIC_OS_IMAGE" ]; then
    pushd $WORKLOADS_DIR
    wget --quiet $BIONIC_OS_IMAGE_URL || exit 1
    popd
fi

BIONIC_OS_RAW_IMAGE_NAME="bionic-server-cloudimg-amd64-raw.img"
BIONIC_OS_RAW_IMAGE="$WORKLOADS_DIR/$BIONIC_OS_RAW_IMAGE_NAME"
if [ ! -f "$BIONIC_OS_RAW_IMAGE" ]; then
    pushd $WORKLOADS_DIR
    qemu-img convert -p -f qcow2 -O raw $BIONIC_OS_IMAGE_NAME $BIONIC_OS_RAW_IMAGE_NAME || exit 1
    popd
fi


FOCAL_OS_IMAGE_NAME="focal-server-cloudimg-amd64.img"
FOCAL_OS_IMAGE_URL="https://cloudhypervisorstorage.blob.core.windows.net/images/$FOCAL_OS_IMAGE_NAME"
FOCAL_OS_IMAGE="$WORKLOADS_DIR/$FOCAL_OS_IMAGE_NAME"
if [ ! -f "$FOCAL_OS_IMAGE" ]; then
    pushd $WORKLOADS_DIR
    wget --quiet $FOCAL_OS_IMAGE_URL || exit 1
    popd
fi

FOCAL_OS_RAW_IMAGE_NAME="focal-server-cloudimg-amd64-raw.img"
FOCAL_OS_RAW_IMAGE="$WORKLOADS_DIR/$FOCAL_OS_RAW_IMAGE_NAME"
if [ ! -f "$FOCAL_OS_RAW_IMAGE" ]; then
    pushd $WORKLOADS_DIR
    qemu-img convert -p -f qcow2 -O raw $FOCAL_OS_IMAGE_NAME $FOCAL_OS_RAW_IMAGE_NAME || exit 1
    popd
fi

cargo install cargo-xbuild
rustup component add rust-src
cargo xbuild --release --target target.json

sudo adduser $USER kvm
newgrp kvm << EOF
export RUST_BACKTRACE=1
cargo test --features "integration_tests"  -- --test-threads=1 test_boot
EOF
