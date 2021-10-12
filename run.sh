#!/bin/bash

cargo build
qemu-system-i386 -S -s -kernel target/i686-unknown-linux-gnu/debug/ron &
sleep 1
gdb target/i686-unknown-linux-gnu/debug/ron -ex "target remote localhost:1234" -ex "break _start" -ex "c" -ex "break goto_kmain"
