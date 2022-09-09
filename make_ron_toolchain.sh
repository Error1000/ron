#!/bin/sh
mkdir toolchain
cd toolchain
ct-ng riscv64-unknown-elf
ct-ng build
