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


cd rlibc/staticlib
cargo build --lib --target=riscv64imac-unknown-none-elf --release
