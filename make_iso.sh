#!/bin/bash
cargo build
rm iso/boot/ron
cp target/*/debug/ron iso/boot
grub-mkrescue -o out.iso iso/
