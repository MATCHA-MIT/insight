import unittest
import os
import logging
from unittest.mock import patch, MagicMock
from riscv_instruction_mutator.mutator import Mutator
from riscv_instruction_mutator import program
from riscv_instruction_mutator.disassembler import Disassembler

# Set up logging configuration
logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler()  # Console output only
    ]
)

logger = logging.getLogger(__name__)

class TestMutator(unittest.TestCase):
    
    def setUp(self):
        logger.info("Setting up TestMutator")
        # Load test program from testfiles/triple_write.bin
        test_file_path = os.path.join(os.path.dirname(__file__), 'testfiles', 'triple_write.bin')
        logger.debug(f"Loading test file from: {test_file_path}")
        
        try:
            with open(test_file_path, 'rb') as f:
                self.test_bytecode = f.read()
            logger.info(f"Successfully loaded {len(self.test_bytecode)} bytes from test file")
        except FileNotFoundError:
            logger.error(f"Test file not found: {test_file_path}")
            raise
        except Exception as e:
            logger.error(f"Error loading test file: {e}")
            raise
        
        self.mutator = Mutator(self.test_bytecode, mutation_steps=1)
        self.disassembler = Disassembler()
        logger.debug("TestMutator setup completed")
        
    
    def disassemble_program(self, prog):
        """Helper method to disassemble all instructions in a program"""
        logger.debug(f"Disassembling program with {len(prog.instructions)} instructions")
        print("\n".join([str(x) for x in prog.instructions]))
        # disassembly = []
        # for instr in prog.instructions:
        #     disasm = self.disassembler.disassemble(instr)
        #     if disasm:
        #         disassembly.append(disasm)
        #     else:
        #         disassembly.append(f"Unknown instruction: 0x{instr:08x}")
        # return "\n".join(disassembly)
    
    def test_change_operand_and_last_write_mutation(self):
        """Test the change_operand_and_last_write mutation"""
        print("Hello?", flush=True)
        logger.info("Starting test_change_operand_and_last_write_mutation")
        
        # Create a program from the test bytecode
        original_program = program.Program(self.test_bytecode)
        logger.debug(f"Created original program with {len(original_program.instructions)} instructions")
        
        # Print original disassembly
        print("Original disassembly:")
        print(self.disassemble_program(original_program))
        
        # Apply the mutation
        logger.debug("Applying mutation: change_source_operand_and_last_write")
        try:
            mutated_program = self.mutator.do_one_mutation(
                original_program, 
                list(range(len(original_program.instructions))),
                choice_arg=(2, "change_source_operand_and_last_write"),
                mutate_field=None,
                arg_mutate_new_value=1
            )[0]
            logger.info("Mutation applied successfully")
        except Exception as e:
            logger.error(f"Error applying mutation: {e}")
            raise
        
        # Print mutated disassembly
        print("\nMutated disassembly:")
        print(self.disassemble_program(mutated_program))
        
        logger.info("test_change_operand_and_last_write_mutation completed")
        # Verify the mutation was applied
        #self.assertNotEqual(original_program.to_bytes(), mutated_program.to_bytes())
    
    def test_operand_mutation_with_specific_field(self):
        """Test operand mutation with a specific field"""
        return
        original_program = program.Program(self.test_bytecode)
        
        # Apply operand mutation to rs1 field
        mutated_program = self.mutator.do_one_mutation(
            original_program,
            list(range(len(original_program.instructions))),
            choice_arg=(0, "operands"),
            mutate_field="rs1",
            arg_mutate_new_value=15
        )[0]
        
        print("\nOriginal vs Operand Mutated:")
        print("Original:", self.disassemble_program(original_program))
        print("Mutated: ", self.disassemble_program(mutated_program))
        
        #self.assertNotEqual(original_program.to_bytes(), mutated_program.to_bytes())
    
    def test_opcode_mutation(self):
        """Test opcode mutation"""
        return
        original_program = program.Program(self.test_bytecode)
        
        # Apply opcode mutation
        mutated_program = self.mutator.do_one_mutation(
            original_program,
            list(range(len(original_program.instructions))),
            choice_arg=(0, "opcode"),
            mutate_field=None,
            arg_mutate_new_value=None
        )[0]
        
        print("\nOriginal vs Opcode Mutated:")
        print("Original:", self.disassemble_program(original_program))
        print("Mutated: ", self.disassemble_program(mutated_program))
        
        #self.assertNotEqual(original_program.to_bytes(), mutated_program.to_bytes())
    
    def test_list_of_mutations(self):
        """Test applying a list of mutations"""
        return
        mutations = [
            ((0, "change_source_operand_and_last_write"), None, 0x1000),
            ((1, "operands"), "rs2", 7),
        ]
        
        original_program = program.Program(self.test_bytecode)
        mutated_program = self.mutator.do_list_of_mutations(mutations)
        
        print("\nOriginal vs List Mutated:")
        print("Original:", self.disassemble_program(original_program))
        print("Mutated: ", self.disassemble_program(mutated_program))
        
        #self.assertNotEqual(original_program.to_bytes(), mutated_program.to_bytes())
    
    def test_random_mutation(self):
        """Test random mutation"""
        return
        original_program = program.Program(self.test_bytecode)
        
        # Set mutation steps to 3 for this test
        self.mutator._mutation_steps = 3
        mutated_program = self.mutator.mutate_random()
        
        print("\nOriginal vs Random Mutated:")
        print("Original:", self.disassemble_program(original_program))
        print("Mutated: ", self.disassemble_program(mutated_program))
        
        #self.assertNotEqual(original_program.to_bytes(), mutated_program.to_bytes())


if __name__ == '__main__':
    logger.info("Starting test execution")
    unittest.main()