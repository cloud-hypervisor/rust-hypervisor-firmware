#!/bin/bash
set -xeuf

source "${CARGO_HOME:-$HOME/.cargo}/env"

rustup component add rust-src
cargo build --release --target target.json -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem --features "coreboot"

FW_BIN="$(pwd)/target/target/release/hypervisor-fw"

CB_DIR="./resources/coreboot"

CB_VERSION="4.13"
CB_URL="https://github.com/coreboot/coreboot.git"
CB_PATH="$CB_DIR/coreboot"
CB_CONFIG="$CB_DIR/qemu-q35-config.in"
if [ ! -d "$CB_PATH" ]; then
  git clone --quiet --branch $CB_VERSION --depth 1 $CB_URL $CB_PATH
fi

cat $CB_CONFIG | sed -e "s#@CONFIG_PAYLOAD_FILE@#$FW_BIN#g" > "$CB_PATH/.config"
make -C $CB_PATH crossgcc-i386 CPUS="$(nproc)"
make -C $CB_PATH olddefconfig
make -C $CB_PATH -j"$(nproc)"

bash ./fetch_disk_images.sh

# Add the user to the kvm group (if not already in it), so they can run VMs
id -nGz "$USER" | grep -qzxF kvm || sudo adduser "$USER" kvm

newgrp kvm << EOF
export RUST_BACKTRACE=1
cargo test --features "coreboot integration_tests"  -- --test-threads=1 test_boot
EOF
