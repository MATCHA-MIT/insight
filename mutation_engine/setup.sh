#!/bin/bash
cd riscv-opcodes
#python parse.py rv_i rv32_i rv_m rv_q rv_s rv_c rv_zicsr rv_zifencei
python parse.py rv_i rv32_i rv_s rv_zicsr rv_zifencei
cd ../
