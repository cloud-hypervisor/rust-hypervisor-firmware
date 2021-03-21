#!/bin/bash
set -xeuf

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
BIONIC_OS_IMAGE_URL="https://cloud-images.ubuntu.com/bionic/current/$BIONIC_OS_IMAGE_NAME"
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
FOCAL_OS_IMAGE_URL="https://cloud-images.ubuntu.com/focal/current/$FOCAL_OS_IMAGE_NAME"
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
