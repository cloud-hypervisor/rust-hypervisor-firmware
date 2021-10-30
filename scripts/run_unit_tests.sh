#!/bin/bash

source $HOME/.cargo/env
source $(dirname "$0")/make-test-disks.sh

WORKLOADS_DIR="$HOME/workloads"
mkdir -p "$WORKLOADS_DIR"

make_test_disks "$WORKLOADS_DIR"

CLEAR_OS_VERSION="28660"
CLEAR_OS_IMAGE_XZ_NAME="clear-$CLEAR_OS_VERSION-kvm.img.xz"
CLEAR_OS_IMAGE_XZ_URL="https://download.clearlinux.org/releases/$CLEAR_OS_VERSION/clear/$CLEAR_OS_IMAGE_XZ_NAME"
CLEAR_OS_IMAGE_XZ="$WORKLOADS_DIR/$CLEAR_OS_IMAGE_XZ_NAME"
if [ ! -f "$CLEAR_OS_IMAGE_XZ" ]; then
    pushd $WORKLOADS_DIR
    time wget --quiet $CLEAR_OS_IMAGE_XZ_URL || exit 1
    popd
fi

CLEAR_OS_IMAGE_NAME="clear-$CLEAR_OS_VERSION-kvm.img"
CLEAR_OS_IMAGE="$WORKLOADS_DIR/$CLEAR_OS_IMAGE_NAME"
if [ ! -f "$CLEAR_OS_IMAGE" ]; then
    pushd $WORKLOADS_DIR
    time unxz $CLEAR_OS_IMAGE_XZ
    popd
fi

export RUST_BACKTRACE=1
cargo test || exit 1;
