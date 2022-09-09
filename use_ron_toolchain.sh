#!/bin/sh
export PATH="$(pwd)/toolchain/.build/riscv64-unknown-elf/buildtools/bin:${PATH}"
export CC="riscv64-unknown-elf-gcc -march=rv64imc -mabi=lp64 -static -nostartfiles"
export LDFLAGS="-I$(pwd)/rlibc/include"
export LDLIBS="$(pwd)/target/riscv64imac-unknown-none-elf/release/librlibc.a"
