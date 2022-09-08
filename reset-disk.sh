#!/bin/sh
sudo losetup /dev/loop0 disk.img
# Clear image
sudo wipefs -a /dev/loop0p1
sudo dd if=/dev/zero of=/dev/loop0p1

# Remake the image
sudo mkfs.ext2 /dev/loop0p1
sudo mkdir -p /tmp/ron-loop
sudo mount /dev/loop0p1 /tmp/ron-loop
sudo cp -r ./disk/. /tmp/ron-loop

# Clean up
sudo umount /tmp/ron-loop
sudo losetup -d /dev/loop0
