# Simple KVM firmware

This repository contains a simple KVM firmware that is designed to be launched
from anything that supports loading ELF binaries and running them with the Linux
kernel loading standard.

The ultimate goal is to be able to use this "firmware" to be able to load a
bootloader from within a disk image.

## Building

To compile:

cargo xbuild --target target.json

The result will be in:

target/target/debug/kvm-fw

## Running

Works with Firecracker as a drop in replacement for the Linux kernel. It does
not work with crosvm as crosvm has a hardcoded kernel function start address.

## Features

* Outputs "hello world" on the serial port
* Reboots

## TODO

* virtio-{mmio/pci} support
* virtio-blk
* FAT filesystem
* PE32 loader
* EFI runtime services stub implmentations


