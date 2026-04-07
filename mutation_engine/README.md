# RISC-V Instruction Mutator

## Usage

```bash
cd riscv-opcodes && ./parse.py rv_* rv32*
cd ../ & ./instr-mutator.py -if <input-file> -of <output-file> -s <max-mutation-steps> -n <number-of-generated-sequences>
```