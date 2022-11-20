#!/bin/bash

set -ex

source "${CARGO_HOME:-$HOME/.cargo}/env"

export RUSTFLAGS="-D warnings"

arch="$(uname -m)"

do_cargo_tests() {
    local cargo_args=("-Zbuild-std=core,alloc" "-Zbuild-std-features=compiler-builtins-mem")
    local cmd="$1"
    local target="$2"
    local features="$3"
    [ -n "$features" ] && cargo_args+=("--features" "$features")
    time cargo "$cmd" --target "$target" "${cargo_args[@]}"
    time cargo "$cmd" --target "$target" --release "${cargo_args[@]}"
}

cargo_tests() {
    local features="$1"

    [ "$arch" = "aarch64" ] && target="aarch64-unknown-none.json"
    [ "$arch" = "x86_64" ] && target="x86_64-unknown-none.json"

    do_cargo_tests "build" "$target" "$features"
    do_cargo_tests "clippy" "$target" "$features"
}

# Install cargo components
time rustup component add clippy
time rustup component add rustfmt
time rustup component add rust-src

# Run cargo builds and checks
cargo_tests ""
if [ "$arch" = "x86_64" ] ; then
    cargo_tests "coreboot"
fi
time cargo clippy --all-targets --all-features
time cargo fmt --all -- --check
