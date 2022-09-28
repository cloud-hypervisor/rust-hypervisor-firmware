#!/bin/bash

# Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
# Copyright © 2020 Intel Corporation
# SPDX-License-Identifier: Apache-2.0

CLI_NAME="Rust Hypervisor Firmware"

CTR_IMAGE_TAG="rusthypervisorfirmware/dev"
CTR_IMAGE_VERSION="latest"
CTR_IMAGE="${CTR_IMAGE_TAG}:${CTR_IMAGE_VERSION}"

DOCKER_RUNTIME="docker"

# Host paths
RHF_SCRIPTS_DIR=$(cd "$(dirname "$0")" && pwd)
RHF_ROOT_DIR=$(cd "${RHF_SCRIPTS_DIR}/.." && pwd)
RHF_BUILD_DIR="${RHF_ROOT_DIR}/build"
RHF_CARGO_TARGET="${RHF_BUILD_DIR}/cargo_target"
RHF_DOCKERFILE="${RHF_ROOT_DIR}/resources/Dockerfile"
RHF_CTR_BUILD_DIR="/tmp/rust-hypervisor-firmware/ctr-build"
RHF_WORKLOADS="${HOME}/workloads"

# Container paths
CTR_RHF_ROOT_DIR="/rust-hypervisor-firmware"
CTR_RHF_CARGO_BUILD_DIR="${CTR_RHF_ROOT_DIR}/build"
CTR_RHF_CARGO_TARGET="${CTR_RHF_CARGO_BUILD_DIR}/cargo_target"
CTR_RHF_WORKLOADS="/root/workloads"

# Container networking option
CTR_RHF_NET="bridge"

# Cargo paths
# Full path to the cargo registry dir on the host. This appears on the host
# because we want to persist the cargo registry across container invocations.
# Otherwise, any rust crates from crates.io would be downloaded again each time
# we build or test.
CARGO_REGISTRY_DIR="${RHF_BUILD_DIR}/cargo_registry"

# Full path to the cargo git registry on the host. This serves the same purpose
# as CARGO_REGISTRY_DIR, for crates downloaded from GitHub repos instead of
# crates.io.
CARGO_GIT_REGISTRY_DIR="${RHF_BUILD_DIR}/cargo_git_registry"

# Full path to the cargo target dir on the host.
CARGO_TARGET_DIR="${RHF_BUILD_DIR}/cargo_target"

# Send a decorated message to stdout, followed by a new line
#
say() {
    [ -t 1 ] && [ -n "$TERM" ] \
        && echo "$(tput setaf 2)[$CLI_NAME]$(tput sgr0) $*" \
        || echo "[$CLI_NAME] $*"
}

# Send a decorated message to stdout, without a trailing new line
#
say_noln() {
    [ -t 1 ] && [ -n "$TERM" ] \
        && echo -n "$(tput setaf 2)[$CLI_NAME]$(tput sgr0) $*" \
        || echo "[$CLI_NAME] $*"
}

# Send a text message to stderr
#
say_err() {
    [ -t 2 ] && [ -n "$TERM" ] \
        && echo "$(tput setaf 1)[$CLI_NAME] $*$(tput sgr0)" 1>&2 \
        || echo "[$CLI_NAME] $*" 1>&2
}

# Send a warning-highlighted text to stdout
say_warn() {
    [ -t 1 ] && [ -n "$TERM" ] \
        && echo "$(tput setaf 3)[$CLI_NAME] $*$(tput sgr0)" \
        || echo "[$CLI_NAME] $*"
}

# Exit with an error message and (optional) code
# Usage: die [-c <error code>] <error message>
#
die() {
    code=1
    [[ "$1" = "-c" ]] && {
        code="$2"
        shift 2
    }
    say_err "$@"
    exit $code
}

# Exit with an error message if the last exit code is not 0
#
ok_or_die() {
    code=$?
    [[ $code -eq 0 ]] || die -c $code "$@"
}

# Fix main directory permissions after a container ran as root.
# Since the container ran as root, any files it creates will be owned by root.
# This fixes that by recursively changing the ownership of /cloud-hypervisor to the
# current user.
#
fix_dir_perms() {
    # Yes, running Docker to get elevated privileges, just to chown some files
    # is a dirty hack.
    $DOCKER_RUNTIME run \
	--workdir "$CTR_RHF_ROOT_DIR" \
	   --rm \
	   --volume /dev:/dev \
	   --volume "$RHF_ROOT_DIR:$CTR_RHF_ROOT_DIR" $exported_volumes \
	   "$CTR_IMAGE" \
           chown -R "$(id -u):$(id -g)" "$CTR_RHF_ROOT_DIR"

    return $1
}

# Process exported volumes argument, separate the volumes and make docker compatible
# Sample input: --volumes /a:/a#/b:/b
# Sample output: --volume /a:/a --volume /b:/b
#
process_volumes_args() {
    if [ -z "$arg_vols" ]; then
        return
    fi
    exported_volumes=""
    arr_vols=(${arg_vols//#/ })
    for var in "${arr_vols[@]}"
    do
        parts=(${var//:/ })
        if [[ ! -e "${parts[0]}" ]]; then
            echo "The volume ${parts[0]} does not exist."
            exit 1
        fi
        exported_volumes="$exported_volumes --volume $var"
    done
}

# Make sure the build/ dirs are available. Exit if we can't create them.
# Upon returning from this call, the caller can be certain the build/ dirs exist.
#
ensure_build_dir() {
    for dir in "$RHF_BUILD_DIR" \
		   "$RHF_WORKLOADS" \
		   "$CARGO_TARGET_DIR" \
		   "$CARGO_REGISTRY_DIR" \
		   "$CARGO_GIT_REGISTRY_DIR"; do
        mkdir -p "$dir" || die "Error: cannot create dir $dir"
        [ -x "$dir" ] && [ -w "$dir" ] || \
            {
                say "Wrong permissions for $dir. Attempting to fix them ..."
                chmod +x+w "$dir"
            } || \
            die "Error: wrong permissions for $dir. Should be +x+w"
    done
}

# Make sure we're using the latest dev container, by just pulling it.
ensure_latest_ctr() {
    $DOCKER_RUNTIME pull "$CTR_IMAGE"

    ok_or_die "Error pulling container image. Aborting."
}

cmd_help() {
    echo ""
    echo "Rust Hypervisor Firmware $(basename $0)"
    echo "Usage: $(basename $0) <command> [<command args>]"
    echo ""
    echo "Available commands:"
    echo ""
    echo "    build [--debug|--release] [-- [<cargo args>]]"
    echo "        Build the Rust Hypervisor Firmware binary."
    echo "        --debug               Build the debug binary. This is the default."
    echo "        --release             Build the release binary."
    echo "        --volumes             Hash separated volumes to be exported. Example --volumes /mnt:/mnt#/myvol:/myvol"
    echo ""
    echo "    build-container [--type]"
    echo "        Build the Rust Hypervisor Firmware container."
    echo "        --dev                Build dev container. This is the default."
    echo ""
    echo "    clean [<cargo args>]]"
    echo "        Remove the Rust Hypervisor Firmware artifacts."
    echo ""
    echo "    tests [--unit|--cargo|--all] [-- [<cargo test args>]]"
    echo "        Run the Rust Hypervisor Firmware tests."
    echo "        --unit                       Run the unit tests."
    echo "        --cargo                      Run the cargo tests."
    echo "        --integration                Run the integration tests."
    echo "        --integration-coreboot       Run the coreboot target integration tests."
    echo "        --integration-windows        Run the Windows guest integration tests."
    echo "        --volumes                    Hash separated volumes to be exported. Example --volumes /mnt:/mnt#/myvol:/myvol"
    echo "        --all                        Run all tests."
    echo ""
    echo "    shell"
    echo "        Run the development container into an interactive, privileged BASH shell."
    echo "        --volumes             Hash separated volumes to be exported. Example --volumes /mnt:/mnt#/myvol:/myvol"
    echo ""
    echo "    help"
    echo "        Display this help message."
    echo ""
}

cmd_build() {
    build="debug"
    features_build=""
    exported_device="dev/kvm"
    while [ $# -gt 0 ]; do
	case "$1" in
            "-h"|"--help")  { cmd_help; exit 1; } ;;
            "--debug")      { build="debug"; } ;;
            "--release")    { build="release"; } ;;
            "--volumes")
                shift
                arg_vols="$1"
                ;;
            "--")           { shift; break; } ;;
            *)
		die "Unknown build argument: $1. Please use --help for help."
		;;
	esac
	shift
    done

    ensure_build_dir
    ensure_latest_ctr

    process_volumes_args

    cargo_args=("$@")
    [ $build = "release" ] && cargo_args+=("--release")

    rustflags=""

    $DOCKER_RUNTIME run \
	   --user "$(id -u):$(id -g)" \
	   --workdir "$CTR_RHF_ROOT_DIR" \
	   --rm \
	   --volume $exported_device \
	   --volume "$RHF_ROOT_DIR:$CTR_RHF_ROOT_DIR" $exported_volumes \
	   --env RUSTFLAGS="$rustflags" \
	   "$CTR_IMAGE" \
	   cargo build --target "x86_64-unknown-none.json" \
	         -Zbuild-std=core,alloc \
	         -Zbuild-std-features=compiler-builtins-mem \
	         --target-dir "$CTR_RHF_CARGO_TARGET" \
	         "${cargo_args[@]}" && say "Binary placed under $RHF_CARGO_TARGET/target/$build"
}

cmd_clean() {
    cargo_args=("$@")

    ensure_build_dir
    ensure_latest_ctr

    $DOCKER_RUNTIME run \
	   --user "$(id -u):$(id -g)" \
	   --workdir "$CTR_RHF_ROOT_DIR" \
	   --rm \
	   --volume "$RHF_ROOT_DIR:$CTR_RHF_ROOT_DIR" $exported_volumes \
	   "$CTR_IMAGE" \
	   cargo clean \
	         --target-dir "$CTR_RHF_CARGO_TARGET" \
	         "${cargo_args[@]}"
}

cmd_tests() {
    unit=false
    cargo=false
    integration=false
    integration_coreboot=false
    integration_windows=false
    arg_vols=""
    exported_device="/dev/kvm"
    while [ $# -gt 0 ]; do
	case "$1" in
            "-h"|"--help")                  { cmd_help; exit 1; } ;;
            "--unit")                       { unit=true; } ;;
            "--cargo")                      { cargo=true; } ;;
            "--integration")                { integration=true; } ;;
            "--integration-coreboot")       { integration_coreboot=true; } ;;
            "--integration-windows")        { integration_windows=true; } ;;
            "--volumes")
                shift
                arg_vols="$1"
                ;;
            "--all")                 { cargo=true; unit=true; integration=true; } ;;
            "--")                    { shift; break; } ;;
            *)
		die "Unknown tests argument: $1. Please use --help for help."
		;;
	esac
	shift
    done

    ensure_build_dir
    ensure_latest_ctr

    process_volumes_args

    if [ "$unit" = true ] ;  then
	say "Running unit tests..."
	$DOCKER_RUNTIME run \
	       --workdir "$CTR_RHF_ROOT_DIR" \
	       --rm \
	       --volume "$RHF_ROOT_DIR:$CTR_RHF_ROOT_DIR" $exported_volumes \
	       --volume "$RHF_WORKLOADS:$CTR_RHF_WORKLOADS" \
	       "$CTR_IMAGE" \
	       ./scripts/run_unit_tests.sh "$@" || fix_dir_perms $? || exit $?
    fi

    if [ "$cargo" = true ] ;  then
	say "Running cargo tests..."
	$DOCKER_RUNTIME run \
	       --workdir "$CTR_RHF_ROOT_DIR" \
	       --rm \
	       --volume "$RHF_ROOT_DIR:$CTR_RHF_ROOT_DIR" $exported_volumes \
	       "$CTR_IMAGE" \
	       ./scripts/run_cargo_tests.sh "$@"  || fix_dir_perms $? || exit $?
    fi

    if [ "$integration" = true ] ;  then
	say "Running integration tests..."
	$DOCKER_RUNTIME run \
	       --workdir "$CTR_RHF_ROOT_DIR" \
	       --rm \
	       --privileged \
	       --security-opt seccomp=unconfined \
	       --ipc=host \
	       --net="$CTR_RHF_NET" \
	       --mount type=tmpfs,destination=/tmp \
	       --volume /dev:/dev \
	       --volume "$RHF_ROOT_DIR:$CTR_RHF_ROOT_DIR" $exported_volumes \
	       --volume "$RHF_WORKLOADS:$CTR_RHF_WORKLOADS" \
	       --env USER="root" \
	       "$CTR_IMAGE" \
	       ./scripts/run_integration_tests.sh "$@" || fix_dir_perms $? || exit $?
    fi

    if [ "$integration_coreboot" = true ] ;  then
	say "Running coreboot integration tests..."
	$DOCKER_RUNTIME run \
	       --workdir "$CTR_RHF_ROOT_DIR" \
	       --rm \
	       --privileged \
	       --security-opt seccomp=unconfined \
	       --ipc=host \
	       --net="$CTR_RHF_NET" \
	       --mount type=tmpfs,destination=/tmp \
	       --volume /dev:/dev \
	       --volume "$RHF_ROOT_DIR:$CTR_RHF_ROOT_DIR" $exported_volumes \
	       --volume "$RHF_WORKLOADS:$CTR_RHF_WORKLOADS" \
	       --env USER="root" \
	       "$CTR_IMAGE" \
	       ./scripts/run_coreboot_integration_tests.sh "$@" || fix_dir_perms $? || exit $?
    fi

    if [ "$integration_windows" = true ] ;  then
	say "Running Windows integration tests..."
	$DOCKER_RUNTIME run \
	       --workdir "$CTR_RHF_ROOT_DIR" \
	       --rm \
	       --privileged \
	       --security-opt seccomp=unconfined \
	       --ipc=host \
	       --net="$CTR_RHF_NET" \
	       --mount type=tmpfs,destination=/tmp \
	       --volume /dev:/dev \
	       --volume "$RHF_ROOT_DIR:$CTR_RHF_ROOT_DIR" $exported_volumes \
	       --volume "$RHF_WORKLOADS:$CTR_RHF_WORKLOADS" \
	       --env USER="root" \
	       "$CTR_IMAGE" \
	       ./scripts/run_integration_tests_windows.sh "$@" || fix_dir_perms $? || exit $?
    fi

    fix_dir_perms $?
}


cmd_build-container() {
    container_type="dev"

    while [ $# -gt 0 ]; do
	case "$1" in
            "-h"|"--help")  { cmd_help; exit 1; } ;;
            "--dev")        { container_type="dev"; } ;;
            "--")           { shift; break; } ;;
            *)
		die "Unknown build-container argument: $1. Please use --help for help."
		;;
	esac
	shift
    done

    ensure_build_dir

    BUILD_DIR="$RHF_CTR_BUILD_DIR"

    mkdir -p $BUILD_DIR
    cp $RHF_DOCKERFILE $BUILD_DIR

    [ $(uname -m) = "x86_64" ] && TARGETARCH="amd64"
    RUST_TOOLCHAIN="$(rustup show active-toolchain | cut -d ' ' -f1)"

    $DOCKER_RUNTIME build \
	   --target $container_type \
	   -t $CTR_IMAGE \
	   -f $BUILD_DIR/Dockerfile \
       --build-arg TARGETARCH=$TARGETARCH \
       --build-arg RUST_TOOLCHAIN=$RUST_TOOLCHAIN \
	   $BUILD_DIR
}

cmd_shell() {
    while [ $# -gt 0 ]; do
	case "$1" in
            "-h"|"--help")  { cmd_help; exit 1; } ;;
            "--volumes")
                shift
                arg_vols="$1"
                ;;
            "--")           { shift; break; } ;;
            *)
		;;
	esac
	shift
    done
    ensure_build_dir
    ensure_latest_ctr
    process_volumes_args
    say_warn "Starting a privileged shell prompt as root ..."
    say_warn "WARNING: Your $RHF_ROOT_DIR folder will be bind-mounted in the container under $CTR_RHF_ROOT_DIR"
    $DOCKER_RUNTIME run \
	   -ti \
	   --workdir "$CTR_RHF_ROOT_DIR" \
	   --rm \
	   --privileged \
	   --security-opt seccomp=unconfined \
	   --ipc=host \
	   --net="$CTR_RHF_NET" \
	   --tmpfs /tmp:exec \
	   --volume /dev:/dev \
	   --volume "$RHF_ROOT_DIR:$CTR_RHF_ROOT_DIR" $exported_volumes \
	   --volume "$RHF_WORKLOADS:$CTR_RHF_WORKLOADS" \
	   --env USER="root" \
	   --entrypoint bash \
	   "$CTR_IMAGE"

    fix_dir_perms $?
}


# Parse main command line args.
#
while [ $# -gt 0 ]; do
    case "$1" in
        -h|--help)              { cmd_help; exit 1; } ;;
        -y|--unattended)        { OPT_UNATTENDED=true; } ;;
        -*)
            die "Unknown arg: $1. Please use \`$0 help\` for help."
            ;;
        *)
            break
            ;;
    esac
    shift
done

# $1 is now a command name. Check if it is a valid command and, if so,
# run it.
#
declare -f "cmd_$1" > /dev/null
ok_or_die "Unknown command: $1. Please use \`$0 help\` for help."

cmd=cmd_$1
shift


$cmd "$@"
