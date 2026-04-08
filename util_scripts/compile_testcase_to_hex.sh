#!/bin/bash
set -e
#set -x
# Check if the correct number of arguments is provided
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <input_assembly_file> <output_hex_file>"
    exit 1
fi
# Assign input parameters to variables
input_assembly_file=$1
output_hex_file=$2
temp_file=$(mktemp /tmp/temp_assembly_XXXXXX.o)
#riscv64-unknown-elf-as  -march=rv32imzifencei_zicsr -mabi=ilp32 -o "$temp_file" $input_assembly_file
riscv64-unknown-elf-as  -march=rv64im_zicsr -mabi=lp64 -o "$temp_file" $input_assembly_file
#riscv64-unknown-elf-as  -march=rv32imzifencei_zicsr_zcb_d_c -mabi=ilp32 -o "$temp_file" $input_assembly_file
riscv64-unknown-elf-objcopy -O binary -S -j .text "$temp_file" $output_hex_file
rm "$temp_file"
