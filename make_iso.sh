#!/bin/bash
cargo build
rm iso/boot/ron
# strip target/*/debug/ron # forgot this makes gdb debugging not work ¯\_(ツ)_/¯
cp target/*/debug/ron iso/boot
grub-mkrescue -o out.iso iso/
