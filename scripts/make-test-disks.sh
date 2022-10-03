#!/bin/bash

make_test_disks() {
    WORKLOADS_DIR="$1"
    pushd "$WORKLOADS_DIR"

    rm -f fat12.img fat16.img fat32.img

    mkdosfs -F 12 -C fat12.img 8192
    file fat12.img
    mkdosfs -F 16 -C fat16.img 32768
    file fat16.img
    mkdosfs -F 32 -C fat32.img 1048576
    file fat32.img

    rm -rf test_data
    mkdir -p test_data/a/b/c
    for x in `seq 0 32767`; do
        echo -n "a" >> test_data/a/b/c/d
    done

    dd if=test_data/a/b/c/d of=test_data/a/b/c/0 count=0 bs=1
    for x in `seq 9 15`; do
        n=$[2**x]
        dd if=test_data/a/b/c/d of=test_data/a/b/c/$[$n - 1] count=$[$n - 1] bs=1
        dd if=test_data/a/b/c/d of=test_data/a/b/c/$n count=$n bs=1
    done

    mkdir -p test_data/largedir
    for x in `seq 0 100`; do
        touch test_data/largedir/$x
    done

    touch test_data/longfilenametest

    export MTOOLS_SKIP_CHECK=1
    mcopy -oi fat12.img  -s test_data/* ::
    mcopy -oi fat16.img  -s test_data/* ::
    mcopy -oi fat32.img  -s test_data/* ::

    rm -rf test_data

    popd
}
