#!/bin/bash
set -x

fetch_ch() {
    CH_PATH="$1"
    CH_ARCH="$2"
    CH_VERSION="v36.0"
    CH_URL_BASE="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/$CH_VERSION"

    [ "$CH_ARCH" = "aarch64" ] && CH_NAME="cloud-hypervisor-static-aarch64"
    [ "$CH_ARCH" = "x86_64" ] && CH_NAME="cloud-hypervisor"
    CH_URL="$CH_URL_BASE/$CH_NAME"

    WGET_RETRY_MAX=10
    WGET_RETRY=0

    until [ "$WGET_RETRY" -ge "$WGET_RETRY_MAX" ]; do
        wget --quiet $CH_URL -O $CH_PATH && break
        WGET_RETRY=$[$WGET_RETRY+1]
    done

    if [ "$WGET_RETRY" -ge "$WGET_RETRY_MAX" ]; then
        echo "Failed to download $CH_URL"
        exit 1
    fi

    wget --quiet $CH_URL -O $CH_PATH
    chmod +x $CH_PATH
    sudo setcap cap_net_admin+ep $CH_PATH
}

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

fetch_raw_ubuntu_image() {
    OS_NAME="$1"
    OS_ARCH="$2"
    OS_DATE="$3"
    OS_IMAGE_NAME="$OS_NAME-server-cloudimg-$OS_ARCH.img"
    OS_RAW_IMAGE_NAME="$OS_NAME-server-cloudimg-$OS_ARCH-raw.img"
    OS_IMAGE_BASE="https://cloud-images.ubuntu.com"
    OS_IMAGE_URL="$OS_IMAGE_BASE/$OS_NAME/$OS_DATE/$OS_IMAGE_NAME"
    fetch_image "$OS_IMAGE_NAME" "$OS_IMAGE_URL"
    convert_image "$OS_IMAGE_NAME" "$OS_RAW_IMAGE_NAME"
}

fetch_clear_image() {
    OS_VERSION="$1"
    OS_IMAGE_NAME="clear-$OS_VERSION-kvm.img"
    OS_IMAGE_BASE="https://ch-images.azureedge.net"
    OS_IMAGE_URL="$OS_IMAGE_BASE/$OS_IMAGE_NAME.xz"
    fetch_image "$OS_IMAGE_NAME" "$OS_IMAGE_URL"
    xz -d "$OS_IMAGE_NAME.xz"
}

aarch64_fetch_disk_images() {
    fetch_raw_ubuntu_image "focal" "arm64" "current"
    fetch_raw_ubuntu_image "jammy" "arm64" "current"
}

x86_64_fetch_disk_images() {
    fetch_clear_image "28660"

    fetch_raw_ubuntu_image "focal" "amd64" "current"
    fetch_raw_ubuntu_image "jammy" "amd64" "current"
}

fetch_disk_images() {
    WORKLOADS_DIR="$1"
    ARCH="$2"

    pushd "$WORKLOADS_DIR"

    [ "$ARCH" = "aarch64" ] && aarch64_fetch_disk_images
    [ "$ARCH" = "x86_64" ] && x86_64_fetch_disk_images

    popd
}
