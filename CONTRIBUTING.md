# Contributing to Rust Hypervisor Firmware

Rust Hypervisor Firmware is an open source project licensed under the [Apache v2 License](https://opensource.org/licenses/Apache-2.0).

## Coding Style

We follow the [Rust Style](https://github.com/rust-dev-tools/fmt-rfcs/blob/master/guide/guide.md)
convention and enforce it through the Continuous Integration (CI) process calling into `rustfmt`
for each submitted Pull Request (PR).

## Certificate of Origin

In order to get a clear contribution chain of trust we use the [signed-off-by language] (https://01.org/community/signed-process)
used by the Linux kernel project.

## Patch format

Beside the signed-off-by footer, we expect each patch to comply with the following format:

```
<component>: Change summary

More detailed explanation of your changes: Why and how.
Wrap it to 72 characters.
See http://chris.beams.io/posts/git-commit/
for some more good pieces of advice.

Signed-off-by: <contributor@foo.com>
```

For example:

```
commit 6e9477ba25ac09cc6d918e6512b9eb4e0fb5a2a5
Author: Rob Bradford <robert.bradford@intel.com>
Date:   Wed May 1 17:30:51 2019 +0100

    block: Partially split VirtioMMIOBlockDevice
    
    Create a new trait called VirtioTransport and create a
    VirtioMMIOTransport that implements that trait moving all the virtio
    MMIO register updates into that. This means the block code is somewhat
    independent of MMIO.
    
    Signed-off-by: Rob Bradford <robert.bradford@intel.com>

```

## Pull requests

Rust Hypervisor Firmware uses the “fork-and-pull” development model. Follow these steps if
you want to merge your changes to the project`:

1. Fork the [rust-hypervisor-firmware](https://github.com/intel/rust-hypervisor-firmware) project
   into your github organization.
2. Within your fork, create a branch for your contribution.
3. [Create a pull request](https://help.github.com/articles/creating-a-pull-request-from-a-fork/)
   against the master branch of the repository.
4. Once the pull request is approved, one of the maintainers will merge it.

## Issue tracking

If you have a problem, please let us know. We recommend using
[github issues](https://github.com/intel/rust-hypervisor-firmware/issues/new) for formally
reporting and documenting them.

