#!/bin/bash
cargo build
rm iso/boot/ron
strip target/*/debug/ron
cp target/*/debug/ron iso/boot
grub-mkrescue -o out.iso iso/
