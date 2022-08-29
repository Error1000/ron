#!/bin/bash
./make_iso.sh
qemu-system-x86_64 -bios /usr/share/ovmf/x64/OVMF.fd -cdrom out.iso -serial stdio -no-reboot -hda test-disk-1mb.img $@
# qemu-system-x86_64 -boot d -cdrom out.iso -serial stdio -no-reboot -hda test-disk-1mb.img $@
killall qemu-system-x86_64
