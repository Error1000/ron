#!/bin/sh
sudo losetup /dev/loop0 test-disk-1mb.img
sudo wipefs -a /dev/loop0p1
sudo dd if=/dev/zero of=/dev/loop0p1
sudo sync
sudo mkfs.ext2 /dev/loop0p1
sudo losetup -d /dev/loop0
