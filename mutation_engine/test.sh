#!/bin/bash
source ../../chipyard-private/env.sh
#python -m unittest riscv_instruction_mutator.test_mutator
python -m unittest riscv_instruction_mutator.test_mutator.TestMutator.test_change_operand_and_last_write_mutation -v

