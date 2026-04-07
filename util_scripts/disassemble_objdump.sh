#!/bin/bash
echo "Disassembling $1"
#riscv64-unknown-elf-objdump -m riscv -M numeric -b binary --no-show-raw-insn --no-addresses -D $1
riscv64-unknown-elf-objdump -m riscv -M numeric -b binary  -D $1
