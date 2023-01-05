#!/bin/sh

# Clear image
sudo losetup -d /dev/loop0
sudo sync

sudo rm disk.img
dd if=/dev/zero of=disk.img bs=128M count=1

sudo sync
sudo losetup /dev/loop0 disk.img
sudo sync

# Remake the image
sudo parted /dev/loop0 mklabel msdos
sudo parted -a minimal /dev/loop0 mkpart primary 0% 128MB
sudo sync

sudo partprobe
sudo sync

sudo mkfs.ext2 /dev/loop0p1
sudo mkdir -p /tmp/ron-loop
sudo mount /dev/loop0p1 /tmp/ron-loop
sudo cp -r ./disk/. /tmp/ron-loop

sudo sync
# Clean up
sudo umount /tmp/ron-loop
sudo losetup -d /dev/loop0
sudo sync
