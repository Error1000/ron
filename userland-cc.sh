#!/bin/sh
riscv64-unknown-elf-gcc -march=rv64imc -mabi=lp64 -static -nostartfiles -nostdlib $@
