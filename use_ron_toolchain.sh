#!/bin/sh
export PATH="$(pwd)/toolchain/.build/riscv64-unknown-elf/buildtools/bin:${PATH}"
export CC="riscv64-unknown-elf-gcc -march=rv64imc -mabi=lp64 -static -nostdlib"
