import logging
import yaml
import os
from capstone import *
from .instruction_type  import InstructionType

def get_relative_path(relative_path):
    # Get the directory of the current script
    script_dir = os.path.dirname(os.path.abspath(__file__))

    # Construct the full path
    full_path = os.path.join(script_dir, relative_path)

    return full_path

logger = logging.getLogger('disassembler')

class Disassembler():
    def __init__(self):
        self.instructions = []
        self.load_instructions()

    def load_instructions(self):
        with open(get_relative_path('../riscv-opcodes/instr_dict.yaml'), 'r') as f:
            instr_dict = yaml.safe_load(f)

        for i in instr_dict:
            self.instructions.append(
                InstructionType(
                    i,
                    instr_dict[i]['extension'],
                    int(instr_dict[i]['mask'], 16),
                    int(instr_dict[i]['match'], 16),
                    instr_dict[i]['variable_fields']
                )
            )

        logger.debug(f"Loaded {len(self.instructions)} RISC-V instructions from specification")

    def disassemble(self, instr: int) -> str:
        """Disassemble the given instruction.
        Args:
            instr (int): The instruction to disassemble
        Returns:
            str: The disassembled instruction
        """
        instruction_match = list(filter(lambda x: x.match(instr), self.instructions))

        if len(instruction_match) == 0:
            #logger.error(f"Failed to disassemble instruction 0x{instr:02x}")
            return None
        else:
            #logger.info(f"Disassembled instruction 0x{instr:02x} as {instruction_match[0].get_mnemonics()}")
            return instruction_match[0].get_mnemonics()