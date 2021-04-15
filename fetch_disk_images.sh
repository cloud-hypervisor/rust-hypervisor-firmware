#!/bin/bash
set -xeuf

IMAGES_DIR="./resources/images"
mkdir -p "$IMAGES_DIR"

fetch_image() {
  OS_IMAGE_NAME="$1"
  OS_IMAGE_URL="$2"
  OS_IMAGE="$IMAGES_DIR/$OS_IMAGE_NAME"
  if [ ! -f "$OS_IMAGE" ]; then
      pushd $IMAGES_DIR
      wget --quiet $OS_IMAGE_URL
      popd
  fi
}

convert_image() {
  OS_IMAGE_NAME="$1"
  OS_RAW_IMAGE_NAME="$2"
  OS_IMAGE="$IMAGES_DIR/$OS_IMAGE_NAME"
  OS_RAW_IMAGE="$IMAGES_DIR/$OS_RAW_IMAGE_NAME"
  if [ ! -f "$OS_RAW_IMAGE" ]; then
      qemu-img convert -p -f qcow2 -O raw $OS_IMAGE $OS_RAW_IMAGE
  fi
}

CLEAR_OS_IMAGE_NAME="clear-31311-cloudguest.img"
CLEAR_OS_URL_BASE="https://cloudhypervisorstorage.blob.core.windows.net/images"
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

GROOVY_OS_IMAGE_NAME="groovy-server-cloudimg-amd64.img"
GROOVY_OS_RAW_IMAGE_NAME="groovy-server-cloudimg-amd64-raw.img"
GROOVY_OS_IMAGE_BASE="https://cloud-images.ubuntu.com/groovy/current"
GROOVY_OS_IMAGE_URL="$GROOVY_OS_IMAGE_BASE/$GROOVY_OS_IMAGE_NAME"
fetch_image "$GROOVY_OS_IMAGE_NAME" "$GROOVY_OS_IMAGE_URL"
convert_image "$GROOVY_OS_IMAGE_NAME" "$GROOVY_OS_RAW_IMAGE_NAME"
