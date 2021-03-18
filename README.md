# Rust Hypervisor Firmware

This repository contains a simple firmware that is designed to be launched from
anything that supports loading ELF binaries and running them with the
PVH booting standard

The purpose is to be able to use this firmware to be able to load a
bootloader from within a disk image without requiring the use of a complex
firmware such as TianoCore/edk2 and without requiring the VMM to reuse
functionality used for booting the Linux kernel.

Currently it will directly load a kernel from a disk image that follows the
[Boot Loader Specification](https://systemd.io/BOOT_LOADER_SPECIFICATION)

There is also minimal EFI compatibility support allowing the boot of some
images that use EFI (shim + GRUB2 as used by Ubuntu).

The firmware is primarily developed against [Cloud
Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) but there is
also support for using QEMU's PVH loader.

This project was originally developed using
[Firecracker](https://github.com/firecracker-microvm) however as it does not
currently support resetting the virtio block device it is not possible to boot
all the way into the OS.

## Building

To compile:

cargo build --release --target target.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem

The result will be in:

target/target/release/hypervisor-fw

## Features

* virtio (PCI) block support
* GPT parsing (to find EFI system partition)
* FAT12/16/32 directory traversal and file reading
* bzImage loader
* "Boot Loader Specification" parser
* PE32+ loader
* Minimal EFI environment (sufficient to boot shim + GRUB2 as used by Ubuntu)

## Running

Works with Cloud Hypervisor and QEMU via their PVH loaders as an alternative to
the Linux kernel.

Cloud Hypervisor and QEMU are currently the primary development targets for the
firmware although support for other VMMs will be considered.

### Cloud Hypervisor

As per [getting
started](https://github.com/cloud-hypervisor/cloud-hypervisor/blob/master/README.md#2-getting-started)

However instead of using the binary firmware for the parameter to `--kernel`
instead use the binary you build above.

```
$ pushd $CLOUDH
$ sudo setcap cap_net_admin+ep ./cloud-hypervisor/target/release/cloud-hypervisor
$ ./cloud-hypervisor/target/release/cloud-hypervisor \
	--kernel ./target/target/release/hypervisor-fw \
	--disk path=focal-server-cloudimg-amd64.raw \
	--cpus boot=4 \
	--memory size=512M \
	--net "tap=,mac=,ip=,mask=" \
	--rng
$ popd
```

### QEMU

Use the QEMU `-kernel` parameter to specify the path to the firmware.

e.g.

```
$ qemu-system-x86_64 -machine q35,accel=kvm -cpu host,-vmx -m 1G\
    -kernel ./target/target/release/hypervisor-fw \
    -display none -nodefaults \
    -serial stdio \
    -drive id=os,file=focal-server-cloudimg-amd64.raw,if=none \
    -device virtio-blk-pci,drive=os,disable-legacy=on
```

## Testing

"cargo test" needs disk images from make-test-disks.sh

And clear-28660-kvm.img:

https://download.clearlinux.org/releases/28660/clear/clear-28660-kvm.img.xz

sha1sum: 5fc086643dea4b20c59a795a262e0d2400fab15f
