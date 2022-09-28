#!/bin/bash

set -ex

source "${CARGO_HOME:-$HOME/.cargo}/env"

export RUSTFLAGS="-D warnings"

# Install cargo components
time rustup component add clippy
time rustup component add rustfmt
time rustup component add rust-src

# Run cargo builds and checks
time cargo build --target x86_64-unknown-none.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem
time cargo build --release --target x86_64-unknown-none.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem
time cargo build --target x86_64-unknown-none.json --features "coreboot" -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem
time cargo build --release --target x86_64-unknown-none.json --features "coreboot" -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem
time cargo clippy --target x86_64-unknown-none.json -Zbuild-std=core,alloc
time cargo clippy --target x86_64-unknown-none.json -Zbuild-std=core,alloc --features "coreboot"
time cargo clippy --all-targets --all-features
time cargo fmt --all -- --check
