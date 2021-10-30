#!/bin/bash
set -x

fetch_image() {
    OS_IMAGE="$1"
    OS_IMAGE_URL="$2"
    if [ ! -f "$OS_IMAGE" ]; then
        pushd $WORKLOADS_DIR
        time wget --quiet $OS_IMAGE_URL
        popd
    fi
}

convert_image() {
    OS_IMAGE="$1"
    OS_RAW_IMAGE="$2"
    if [ ! -f "$OS_RAW_IMAGE" ]; then
        time qemu-img convert -p -f qcow2 -O raw $OS_IMAGE $OS_RAW_IMAGE
    fi
}

fetch_disk_images() {
    WORKLOADS_DIR="$1"
    pushd "$WORKLOADS_DIR"

    CLEAR_OS_IMAGE_NAME="clear-31311-cloudguest.img"
    CLEAR_OS_URL_BASE="https://cloud-hypervisor.azureedge.net/"
    CLEAR_OS_IMAGE_URL="$CLEAR_OS_URL_BASE/$CLEAR_OS_IMAGE_NAME"
    fetch_image "$CLEAR_OS_IMAGE_NAME" "$CLEAR_OS_IMAGE_URL"

    BIONIC_OS_IMAGE_NAME="bionic-server-cloudimg-amd64.img"
    BIONIC_OS_RAW_IMAGE_NAME="bionic-server-cloudimg-amd64-raw.img"
    BIONIC_OS_IMAGE_BASE="https://cloud-images.ubuntu.com/bionic/current"
    BIONIC_OS_IMAGE_URL="$BIONIC_OS_IMAGE_BASE/$BIONIC_OS_IMAGE_NAME"
    fetch_image "$BIONIC_OS_IMAGE_NAME" "$BIONIC_OS_IMAGE_URL"
    convert_image "$BIONIC_OS_IMAGE_NAME" "$BIONIC_OS_RAW_IMAGE_NAME"

    FOCAL_OS_IMAGE_NAME="focal-server-cloudimg-amd64.img"
    FOCAL_OS_RAW_IMAGE_NAME="focal-server-cloudimg-amd64-raw.img"
    FOCAL_OS_IMAGE_BASE="https://cloud-images.ubuntu.com/focal/current"
    FOCAL_OS_IMAGE_URL="$FOCAL_OS_IMAGE_BASE/$FOCAL_OS_IMAGE_NAME"
    fetch_image "$FOCAL_OS_IMAGE_NAME" "$FOCAL_OS_IMAGE_URL"
    convert_image "$FOCAL_OS_IMAGE_NAME" "$FOCAL_OS_RAW_IMAGE_NAME"

    HIRSUTE_OS_IMAGE_NAME="hirsute-server-cloudimg-amd64.img"
    HIRSUTE_OS_RAW_IMAGE_NAME="hirsute-server-cloudimg-amd64-raw.img"
    HIRSUTE_OS_IMAGE_BASE="https://cloud-images.ubuntu.com/hirsute/current"
    HIRSUTE_OS_IMAGE_URL="$HIRSUTE_OS_IMAGE_BASE/$HIRSUTE_OS_IMAGE_NAME"
    fetch_image "$HIRSUTE_OS_IMAGE_NAME" "$HIRSUTE_OS_IMAGE_URL"
    convert_image "$HIRSUTE_OS_IMAGE_NAME" "$HIRSUTE_OS_RAW_IMAGE_NAME"

    popd
}
