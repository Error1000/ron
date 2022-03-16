#!/bin/bash
./make_iso.sh
qemu-system-x86_64 -bios /usr/share/ovmf/x64/OVMF.fd -cdrom out.iso -serial stdio -no-reboot test-disk-512b.img
# qemu-system-x86_64 -cdrom out.iso -serial stdio -no-reboot test-disk-512b.img
killall qemu-system-x86_64
