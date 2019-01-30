# Simple KVM firmware

This repository contains a simple KVM firmware that is designed to be launched
from anything that supports loading ELF binaries and running them with the Linux
kernel loading standard.

The ultimate goal is to be able to use this "firmware" to be able to load a
bootloader from within a disk image.

## Building

To compile:

cargo xbuild --release --target target.json

The result will be in:

target/target/release/kvm-fw

Debug builds do not currently function.

## Running

Works with Firecracker as a drop in replacement for the Linux kernel. It does
not work with crosvm as crosvm has a hardcoded kernel function start address.

## Features

* virtio (MMIO) block support
* GPT parsing (to find EFI system partition)
* FAT12/16/32 directory traversal and file reading

## TODO

* PE32 loader
* EFI runtime services stub implmentations

## Testing

"cargo test" needs disk images from make-test-disks.sh

super_grub2_disk_x86_64_efi_2.02s10.iso which you can download from:

http://download2.nust.na/pub4/sourceforge/s/su/supergrub2/2.02s10/super_grub2_disk_2.02s10/super_grub2_disk_x86_64_efi_2.02s10.iso

sha1sum: 2b6bec29fb696cce96c47895f2263d45e2dc822e
