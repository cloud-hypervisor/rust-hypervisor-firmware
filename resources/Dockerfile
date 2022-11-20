FROM ubuntu:20.04 as dev

ARG TARGETARCH
ARG RUST_TOOLCHAIN
ARG RHF_SRC_DIR="/rust-hypervisor-firmware"
ARG RHF_BUILD_DIR="$RHF_SRC_DIR/build"
ARG CARGO_REGISTRY_DIR="$RHF_BUILD_DIR/cargo_registry"
ARG CARGO_GIT_REGISTRY_DIR="$RHF_BUILD_DIR/cargo_git_registry"
ARG COREBOOT_VERSION="4.13"

ENV CARGO_HOME=/usr/local/rust
ENV RUSTUP_HOME=$CARGO_HOME
ENV PATH="$PATH:$CARGO_HOME/bin"
ENV COREBOOT_DIR=/opt/coreboot/src

# Install all CI dependencies
RUN if [ "$TARGETARCH" = "amd64" ]; then \
	apt-get update \
	&& apt-get -yq upgrade \
	&& DEBIAN_FRONTEND=noninteractive apt-get install -yq \
	build-essential \
	bc \
	docker.io \
	curl \
	wget \
	sudo \
	mtools \
	musl-tools \
	libssl-dev \
	pkg-config \
	flex \
	bison \
	libelf-dev \
	qemu-utils \
	qemu-system \
	libglib2.0-dev \
	libpixman-1-dev \
	libseccomp-dev \
	libcap-ng-dev \
	socat \
	dosfstools \
	cpio \
	python \
	python3 \
	python3-setuptools \
	ntfs-3g \
	python3-distutils \
	uuid-dev \
	m4 \
	zlib1g-dev \
	gnat-9 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* \
	; fi

RUN if [ "$TARGETARCH" = "arm64" ]; then \
	apt-get update \
	&& apt-get -yq upgrade \
	&& DEBIAN_FRONTEND=noninteractive apt-get install -yq \
	build-essential \
	docker.io \
	curl \
	wget \
	sudo \
	mtools \
	libssl-dev \
	pkg-config \
	qemu-utils \
	libseccomp-dev \
	libcap-ng-dev \
	libcap2-bin \
	dosfstools \
	&& apt-get clean \
	&& rm -rf /var/lib/apt/lists/* \
	; fi

# Fix the libssl-dev install
RUN export ARCH="$(uname -m)" \
    && cp /usr/include/$ARCH-linux-gnu/openssl/opensslconf.h /usr/include/openssl/
ENV X86_64_UNKNOWN_LINUX_GNU_OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu/
ENV X86_64_UNKNOWN_LINUX_MUSL_OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu/
ENV AARCH64_UNKNOWN_LINUX_GNU_OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu/
ENV AARCH64_UNKNOWN_LINUX_MUSL_OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu/
ENV OPENSSL_INCLUDE_DIR=/usr/include/

# Install the rust toolchain
RUN export ARCH="$(uname -m)" \
	&& nohup curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain "$RUST_TOOLCHAIN" \
	&& rustup component add rustfmt \
	&& rustup component add clippy \
	&& rustup component add rust-src \
	&& rustup target add aarch64-unknown-linux-gnu \
	&& rustup target add x86_64-unknown-linux-gnu \
	&& rm -rf "$CARGO_HOME/registry" \
	&& ln -s "$CARGO_REGISTRY_DIR" "$CARGO_HOME/registry" \
	&& rm -rf "$CARGO_HOME/git" \
	&& ln -s "$CARGO_GIT_REGISTRY_DIR" "$CARGO_HOME/git"

# Set the rust environment
RUN echo 'source $CARGO_HOME/env' >> $HOME/.bashrc \
	&& mkdir $HOME/.cargo \
	&& ln -s $CARGO_HOME/env $HOME/.cargo/env

# Checkout coreboot repository and setup cross toolchains
RUN if [ "$TARGETARCH" = "amd64" ]; then \
	git clone --quiet --branch "$COREBOOT_VERSION" --depth 1 https://github.com/coreboot/coreboot.git "$COREBOOT_DIR" \
	&& cd "$COREBOOT_DIR" \
	&& git submodule update --init --checkout \
	&& make crossgcc-i386 CPUS=`nproc`; \
	fi
