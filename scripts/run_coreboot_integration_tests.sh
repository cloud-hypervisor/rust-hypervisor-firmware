#!/bin/bash
set -x

RHF_ROOT_DIR=$(cd "$(dirname "$0")/../" && pwd)

source "${CARGO_HOME:-$HOME/.cargo}/env"
source "$(dirnam "$0")/fetch_images.sh"

WORKLOADS_DIR="$HOME/workloads"
mkdir -p "$WORKLOADS_DIR"

fetch_disk_images "$WORKLOADS_DIR"

rustup component add rust-src
cargo build --release --target x86_64-unknown-none.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem --features "coreboot"

RHF_BIN="$RHF_ROOT_DIR/target/x86_64-unknown-none/release/hypervisor-fw"
COREBOOT_CONFIG_IN="$RHF_ROOT_DIR/resources/coreboot/qemu-q35-config.in"

cat $COREBOOT_CONFIG_IN | sed -e "s#@CONFIG_PAYLOAD_FILE@#$RHF_BIN#g" > "$COREBOOT_DIR/.config"
make -C $COREBOOT_DIR olddefconfig
make -C $COREBOOT_DIR -j"$(nproc)"

export RUST_BACKTRACE=1
cargo test --features "coreboot integration_tests" "integration::tests::linux::x86_64"
