#!/bin/bash
set -xeuf

source "${CARGO_HOME:-$HOME/.cargo}/env"

XBUILD_VERSION="0.5.34"
cargo install cargo-xbuild --version $XBUILD_VERSION

rustup component add rust-src
cargo xbuild --release --target target.json

CH_VERSION="v0.8.0"
CH_URL="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/$CH_VERSION/cloud-hypervisor"
CH_PATH="./resources/cloud-hypervisor"
if [ ! -f "$CH_PATH" ]; then
    wget --quiet $CH_URL -O $CH_PATH
    chmod +x $CH_PATH
fi

IMAGES_DIR="./resources/images"
mkdir -p "$IMAGES_DIR"

CLEAR_OS_IMAGE_NAME="clear-31311-cloudguest.img"
CLEAR_OS_IMAGE_URL="https://cloudhypervisorstorage.blob.core.windows.net/images/$CLEAR_OS_IMAGE_NAME"
CLEAR_OS_IMAGE="$IMAGES_DIR/$CLEAR_OS_IMAGE_NAME"
if [ ! -f "$CLEAR_OS_IMAGE" ]; then
    pushd $IMAGES_DIR
    wget --quiet $CLEAR_OS_IMAGE_URL
    popd
fi

BIONIC_OS_IMAGE_NAME="bionic-server-cloudimg-amd64.img"
BIONIC_OS_IMAGE_URL="https://cloudhypervisorstorage.blob.core.windows.net/images/$BIONIC_OS_IMAGE_NAME"
BIONIC_OS_IMAGE="$IMAGES_DIR/$BIONIC_OS_IMAGE_NAME"
if [ ! -f "$BIONIC_OS_IMAGE" ]; then
    pushd $IMAGES_DIR
    wget --quiet $BIONIC_OS_IMAGE_URL
    popd
fi

BIONIC_OS_RAW_IMAGE_NAME="bionic-server-cloudimg-amd64-raw.img"
BIONIC_OS_RAW_IMAGE="$IMAGES_DIR/$BIONIC_OS_RAW_IMAGE_NAME"
if [ ! -f "$BIONIC_OS_RAW_IMAGE" ]; then
    pushd $IMAGES_DIR
    qemu-img convert -p -f qcow2 -O raw $BIONIC_OS_IMAGE_NAME $BIONIC_OS_RAW_IMAGE_NAME
    popd
fi


FOCAL_OS_IMAGE_NAME="focal-server-cloudimg-amd64.img"
FOCAL_OS_IMAGE_URL="https://cloudhypervisorstorage.blob.core.windows.net/images/$FOCAL_OS_IMAGE_NAME"
FOCAL_OS_IMAGE="$IMAGES_DIR/$FOCAL_OS_IMAGE_NAME"
if [ ! -f "$FOCAL_OS_IMAGE" ]; then
    pushd $IMAGES_DIR
    wget --quiet $FOCAL_OS_IMAGE_URL
    popd
fi

FOCAL_OS_RAW_IMAGE_NAME="focal-server-cloudimg-amd64-raw.img"
FOCAL_OS_RAW_IMAGE="$IMAGES_DIR/$FOCAL_OS_RAW_IMAGE_NAME"
if [ ! -f "$FOCAL_OS_RAW_IMAGE" ]; then
    pushd $IMAGES_DIR
    qemu-img convert -p -f qcow2 -O raw $FOCAL_OS_IMAGE_NAME $FOCAL_OS_RAW_IMAGE_NAME
    popd
fi

# Add the user to the kvm group (if not already in it), so they can run VMs
id -nGz "$USER" | grep -qzxF kvm || sudo adduser "$USER" kvm

newgrp kvm << EOF
export RUST_BACKTRACE=1
cargo test --features "integration_tests"  -- --test-threads=1 test_boot
EOF
