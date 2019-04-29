# Simple KVM firmware

**This project is an experiment and should not be used production workloads.**

This repository contains a simple KVM firmware that is designed to be launched
from anything that supports loading ELF binaries and running them with the Linux
kernel loading standard.

The ultimate goal is to be able to use this "firmware" to be able to load a
bootloader from within a disk image.

Currently it will directly load a kernel from a disk image that follows the
[Boot Loader Specification](https://systemd.io/BOOT_LOADER_SPECIFICATION)

Although this project has been developed using
[Firecracker](https://github.com/firecracker-microvm) as it does not currently
support resetting the virtio block device it is not possible to boot all the
way into the OS.

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
* bzImage loader
* "Boot Loader Specification" parser

## TODO

* PCI support
* PE32 loader
* EFI runtime services stub implmentations

## Testing

"cargo test" needs disk images from make-test-disks.sh

It also requires super_grub2_disk_x86_64_efi_2.02s10.iso which you can download from:

http://download2.nust.na/pub4/sourceforge/s/su/supergrub2/2.02s10/super_grub2_disk_2.02s10/super_grub2_disk_x86_64_efi_2.02s10.iso

sha1sum: 2b6bec29fb696cce96c47895f2263d45e2dc822e

And clear-28660-kvm.img:

https://download.clearlinux.org/releases/28660/clear/clear-28660-kvm.img.xz

sha1sum: 5fc086643dea4b20c59a795a262e0d2400fab15f

## Security

**Reporting a Potential Security Vulnerability**: If you have discovered
potential security vulnerability in this project, please send an e-mail to
secure@intel.com. For issues related to Intel Products, please visit
https://security-center.intel.com.

It is important to include the following details:
  - The projects and versions affected
  - Detailed description of the vulnerability
  - Information on known exploits

Vulnerability information is extremely sensitive. Please encrypt all security
vulnerability reports using our *PGP key*

A member of the Intel Product Security Team will review your e-mail and
contact you to to collaborate on resolving the issue. For more information on
how Intel works to resolve security issues, see: *Vulnerability Handling
Guidelines*

PGP Key: https://www.intel.com/content/www/us/en/security-center/pgp-public-key.html

Vulnerability Handling Guidelines: https://www.intel.com/content/www/us/en/security-center/vulnerability-handling-guidelines.html

