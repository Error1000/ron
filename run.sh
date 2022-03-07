#!/bin/bash

cargo build
./make_iso.sh
qemu-system-i386 -S -s -cdrom out.iso -bios /usr/share/ovmf/ia32/OVMF.fd -no-reboot &
sleep 1
gdb target/*-unknown-linux-gnu/debug/ron -ex "target remote localhost:1234" -ex "break _start" -ex "c" -ex "break goto_kmain" -ex "tui enable"
killall qemu-system-i386
