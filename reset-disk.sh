#!/bin/sh
sudo losetup /dev/loop0 test-disk-1mb.img
sudo wipefs -a /dev/loop0p1
sudo dd if=/dev/zero of=/dev/loop0p1
sudo mkfs.ext2 /dev/loop0p1
sudo mount /dev/loop0p1 /mnt/loop
sudo cp /tmp/output /mnt/loop
sudo umount /mnt/loop
sudo losetup -d /dev/loop0
