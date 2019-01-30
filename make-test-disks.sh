#!/bin/bash

dd if=/dev/zero of=fat12.img count=4 bs=1M
mkdosfs fat12.img
file fat12.img
dd if=/dev/zero of=fat16.img count=16 bs=1M
mkdosfs fat16.img
file fat16.img
dd if=/dev/zero of=fat32.img count=512 bs=1M
mkdosfs fat32.img
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

export MTOOLS_SKIP_CHECK=1
mcopy -oi fat12.img  -s test_data/* ::
mcopy -oi fat16.img  -s test_data/* ::
mcopy -oi fat32.img  -s test_data/* ::

rm -rf test_data
