import unittest
import sys
print(sys.path)
from riscv_instruction_mutator import utils
from riscv_instruction_mutator import mutator

class TestMutation(unittest.TestCase):
    # def test_opcode_mutation(self):
    #     instruction_dict = utils.get_instruction_dict_from_bytestring(b"\x83\x83\x07\x20") # lb      t2,512(a5)
    #     first_instruction = list(instruction_dict.keys())[0]
    #     print("Testing for instruction", hex(first_instruction))
    #     mut = mutator.Mutator(instruction_dict=instruction_dict, mutation_steps=1)
    #     print(utils.int_to_little_endian_hex(mut.mutate(choice_arg=(first_instruction, "opcode"))[0]))
        
    def test_change_source_operand_and_all_prior_writes(self):
        with open("waw_hazard.bin", "rb") as f:
            bytecode = f.read()
        #instruction_dict = utils.get_instruction_dict_from_bytestring(bytecode)
        #print("instruction dict", instruction_dict)
        mut = mutator.Mutator(bytecode=bytecode,
                              mutation_steps=1,
                                interesting_instructions=[0,1,2])
        
        print("Before mutation:")
        print(mut.program.code_as_hexstring())
        mut.program.change_source_operand_and_all_prior_writes(4, new_value=10)
        print("After mutation:")
        print(mut.program.code_as_hexstring())
        
        
        