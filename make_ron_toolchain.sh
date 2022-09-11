#!/bin/sh
mkdir toolchain
cd toolchain
if test -f "build.log"; then
	echo "NOTE: Not rebuilding entire toolchain, to do that please clean the build directory"
else
	ct-ng riscv64-unknown-elf
	ct-ng build
fi
cd ..


cd rlibc/syslib
cargo build -Zbuild-std=core --lib --target=../../riscv64imc-unknown-ron-elf.json --release
