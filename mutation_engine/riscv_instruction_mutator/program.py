# from . import disassembler
import copy
import random
from . import instruction as instruction_module
import typing
from . import utils
import logging
import sys
import time


logger = logging.getLogger(__name__)

class Program:
    def __init__(self, code):
        instructions = []
        for i in range(0, len(code), 4):
        #    logger.debug(f"Code is {code}")
            instructions.append(instruction_module.Instruction(int.from_bytes(code[i:i+4], "little")))
        self.instructions: typing.List[instruction_module.Instruction] = instructions
        self.writes_map = self.compute_writes_map()
        #print(f"Initial writes_map: {self.writes_map}")

    def get_length(self):
        return len(self.instructions)
    
    def delete_instruction(self, index):
        self.instructions.pop(index)
        
    def delete_instructions(self, delete_idx_list: typing.List[int]):
        self.instructions = [instr for idx, instr in enumerate(self.instructions) if idx not in delete_idx_list]
        
    def compute_writes_map(self):
        """
        Compute a map from register number to the list of instruction indices that write to that register.
        """
        writes_map = {}
        for idx, instr in enumerate(self.instructions):
            rd = instr.get_field_if_exists_as_int("rd")
            if rd is not None:
                if rd not in writes_map:
                    writes_map[rd] = []
                writes_map[rd].append(idx)
        return writes_map

    def get_code(self):
        code = bytearray()
        for instr in self.instructions:
            code.extend(instr._instruction_bytes)
        return bytes(code)
    
    def code_as_hexstring(self, start_idx=None, end_idx=None):
        if start_idx is None:
            start_idx = 0
        if end_idx is None:
            end_idx = len(self.instructions)
        hex_chunks = [
            f"{instr._instruction_bytes[3]:02x}{instr._instruction_bytes[2]:02x}{instr._instruction_bytes[1]:02x}{instr._instruction_bytes[0]:02x}"
            for instr in self.instructions[start_idx:end_idx]
        ]
        return ''.join(hex_chunks)
    
    def print_code(self, start_idx=None, end_idx=None):
        logger.debug(self.code_as_hexstring(start_idx, end_idx))
        
    def substitute_with_nop_or_append(self, idx):
        """
        Substitute the instruction at index idx with a NOP instruction.
        If the instruction does not exists yet, append
        """
        if idx < len(self.instructions):
            # If the instruction exists, replace it with NOP
            self.instructions[idx] = instruction_module.Instruction(0x00000013)
        else:
            # If the instruction does not exist, append a NOP instruction
            while len(self.instructions) <= idx:
                self.instructions.append(instruction_module.Instruction(0x00000013))
    
    def set_instruction(self, idx, instruction):
        """
        Set the instruction at index idx to the given instruction.
        If the index is out of bounds, extend the list with NOPs until it reaches the index.
        """
        
        
    
    def substitute_with_addi(self, idx):
        self.instructions[idx] = instruction_module.Instruction(0x00100093)
        return self.instructions
        #new_sequence = copy.deepcopy(instr_dict)
        #new_sequence[0x00100093] = new_sequence.pop(instr)
        #new_sequence[0x00100093]["mnemonics"] = ["addi"]
        #return 
        #out_file = f"addi_substitution_{instr}"
        #with open(os.path.join(out_dir, out_file), "wb") as outf:
        #    for instr in sorted(new_sequence.keys(), key=lambda x: new_sequence[x]["index"]):
        #        outf.write(instr.to_bytes(4, "little"))

        #return out_file
        
    def get_code_slice(self, start_instruction, end_instruction):
        """
        Return the code (bytestring) for the instructions between start_instruction and end_instruction (inclusive)
        """
        code = b""
        for idx in range(start_instruction, end_instruction+1):
            code += self.instructions[idx]._instruction_bytes
        #for instr in self.instructions.keys():
        #    if self.instructions[instr]["index"] >= start_instruction and self.instructions[instr]["index"] <= end_instruction:
        #        code += instr.to_bytes(4, "little")
        return code
    
    def change_operand(self, idx, operand, new_value):
        instr = self.instructions[idx]
        new_instr = instr.create_copy_with_field_changed(operand, new_value)
        self.instructions[idx] = new_instr #TODO: Would we not need to change the write map as well?
        return self.instructions
    
    def change_source_operand_value(self, idx, new_value=None):
        """
        For the instruction at index idx, change the value of the source operand.
        If new_value is None, generate a random value.
        If the instruction does not have a source operand, do nothing.
        """
        # Check if the instruction has a source operand
        instr = self.instructions[idx]
        source_operand = instr.get_field_if_exists("rs1")
        if source_operand is None:
            return
        else:
            # Define the range for 32-bit integers in two's complement
            lower_bound = -2**31
            upper_bound = 2**31 - 1
            # Generate a random integer within the specified range
            random_integer = random.randint(lower_bound, upper_bound)
            new_value = random.randint(0, random_integer) if new_value is None else new_value
            bytecode = utils.generate_riscv_load_immediate_program_and_get_bytecode("./scripts/assembly_and_write_testcase.sh",source_operand, new_value)
            new_instructions = instruction_module.bytecode_to_instruction_list(bytecode)
            self.instructions = self.instructions[:idx] + new_instructions + self.instructions[idx+1:]
        return self.instructions
    
    def change_source_operand_and_last_write(self, idx, new_value=None):
        """
        For the instruction at index idx, change the source operand.
        Then, change the last write operand to the old source operand to the same value.
        """
        raise Exception("This function is deprecated, use change_source_operand_and_all_prior_writes instead")
        instr = self.instructions[idx]
        old_value = instr.get_field_if_exists("rs1")
        if old_value is not None:
            if new_value is None:
                new_value = random.randint(0, 31)
            new_instr = instr.create_copy_with_field_changed("rs1", new_value)
            self.instructions[idx] = new_instr
            # Find the first instruction before the current one
            for i in range(idx - 1, -1, -1):
                prev_instr = self.instructions[i]
                rd_value = prev_instr.get_field_if_exists("rd")
                if rd_value == old_value:
                    new_prev_instr = prev_instr.create_copy_with_field_changed("rd", new_value)
                    self.instructions[i] = new_prev_instr
                    break
        return self.instructions
    
    def change_source_operand_and_all_prior_writes(self, idx, new_value=None):
        """
        For the instruction at index idx, change a source operand (rs1 or rs2).
        Then, change all prior write operands to the old source operand to the same value.
        If both rs1 and rs2 are present, randomly select one.
        """
        print(f"Changing source operand for instruction at index {idx} to {new_value}")
        instr = self.instructions[idx]
        print(f"change_source_operand_and_all_prior_writes Instruction before change: {str(instr)}")
        
        # Check which source operands are available
        rs1_value = instr.get_field_if_exists_as_int("rs1")
        rs2_value = instr.get_field_if_exists_as_int("rs2")
        
        # Select which operand to change
        available_operands = []
        if rs1_value is not None:
            available_operands.append(("rs1", rs1_value))
        if rs2_value is not None:
            available_operands.append(("rs2", rs2_value))
        
        if not available_operands:
            logging.warning(f"No source operands available for instruction at index {idx}")
            return self.instructions
        
        # Randomly select one if multiple are available
        selected_operand, old_value = random.choice(available_operands)
        print(f"Selected operand {selected_operand} with old value {old_value}, available operands: {available_operands}")
        
        if new_value is None:
            new_value = random.randint(0, 31)
        
        # Change the selected source operand
        new_instr = instr.create_copy_with_field_changed(selected_operand, new_value)
        self.instructions[idx] = new_instr
        
        instructions_to_update = [i for i in self.writes_map.get(old_value, []) if i < idx]
        #print(f"Updating instructions {instructions_to_update} changing {old_value} to {new_value}")
        #print(f"writes_map before update: {self.writes_map}")
        # Update writes_map
        self.writes_map[new_value] = self.writes_map.get(new_value, []) + instructions_to_update
        if old_value in self.writes_map:
            self.writes_map[old_value] = [i for i in self.writes_map[old_value] if i >= idx]
            if not self.writes_map[old_value]:
                del self.writes_map[old_value]

        # Find all instructions before the current one that write to the old value
        for i in instructions_to_update:
            self.instructions[i] = self.instructions[i].create_copy_with_field_changed("rd", new_value)
        
        return self.instructions
    
    def has_operand(self, idx, operand):
        instr = self.instructions[idx]
        return instr.get_field_if_exists(operand) is not None
    
    def switch_instructions(self, idx1, idx2):
        self.instructions[idx1], self.instructions[idx2] = self.instructions[idx2], self.instructions[idx1]
        return self.instructions

    def __deepcopy__(self, memo):
        # Fast deepcopy: copy instructions using their own copy method if available
        # Light copy, as instructions are supposed to be immutable!!
        #print("Deepcopying program...")
        copied_instructions = [instr for instr in self.instructions]
        new_program = Program(b"")  # Empty code, will set instructions manually
        new_program.instructions = copied_instructions
        new_program.writes_map = copy.deepcopy(self.writes_map, memo)
        return new_program