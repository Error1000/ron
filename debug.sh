#!/bin/bash

./make_iso.sh
qemu-system-x86_64 -S -s -bios /usr/share/ovmf/x64/OVMF.fd -cdrom out.iso -no-reboot -hda test-disk-1mb.img &
# qemu-system-x86_64 -S -s -boot d -cdrom out.iso -no-reboot -hda test-disk-1mb.img &
sleep 1
rust-gdb target/*-unknown-linux-gnu/debug/ron -ex "target remote localhost:1234" -ex "break _start" -ex "c" -ex "break goto_kmain" -ex "tui enable"
killall qemu-system-x86_64
