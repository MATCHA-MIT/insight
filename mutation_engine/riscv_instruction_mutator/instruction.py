from .import utils
import csv
from . import disassembler

class Instruction:
    
    _arg_lut = None
    _instruction_types = None
    disassembler = None
    
    @classmethod
    def bytecode_to_instruction_list(cls, bytecode):
        """
        Converts a bytecode string to a list of instructions.
        """
        cls.init_cls()
        instructions = []
        for i in range(0, len(bytecode), 4):
            instructions.append(cls(int.from_bytes(bytecode[i:i+4], "little")))
        return instructions
    
    @classmethod
    def _load_arg_lut(cls):
        if cls._arg_lut is None:
            with open(utils.get_relative_path('../riscv-opcodes/arg_lut.csv'), 'r') as f:
                reader = csv.reader(f)
                
                arg_lut = {}
                for row in reader:
                    arg_lut[row[0]] = row[1:]

                cls._arg_lut = arg_lut
                
    @classmethod
    def load_instruction_types(cls):
        cls._instruction_types = utils._load_instruction_types()
        
    @classmethod
    def init_disassembler(cls):
        cls.disassembler = disassembler.Disassembler()
        
    @classmethod
    def init_cls(cls):
        if cls.disassembler is None:
            cls.init_disassembler()
            cls.load_instruction_types()
            cls._load_arg_lut()
        
    
    def __init__(self, arg_instruction, instr_details=None):
        self.init_cls()
        if type(arg_instruction) is int:
            self._instruction_integer = arg_instruction
            self._instruction_bytes = arg_instruction.to_bytes(4, "little")
        else:
            self._instruction_bytes = arg_instruction
            self._instruction_integer = int.from_bytes(arg_instruction, "little")
        
        if instr_details is None:
            self._mnemonics = self.disassembler.disassemble(self._instruction_integer)
            if self._mnemonics is None:
                self._instr_details = None
            else:
                self._instr_details = next(filter(lambda i: i.get_mnemonics()==self._mnemonics, self._instruction_types))
        else:
            self._instr_details = instr_details
            self._mnemonics = instr_details.get_mnemonics()


    def get_field_if_exists_as_int(self, field) -> int | None:
        """
        """
        bits = self.get_field_if_exists_as_bitstring(field)
        if bits is None:
            return None
        else:
            return utils.bits_to_int(bits)
    
    def get_field_if_exists_as_bitstring(self, field) -> str | None:
        """
        """
        #print("Looking for field", field, "in ",self.instr_details._var_operands )
        if self._instr_details is None:
            return None
        if field in self._instr_details._var_operands:
            beg, end = self._arg_lut[field]
            ret_bytes = utils.int_to_bits(self._instruction_integer, 32)[-(int(beg)+1):-int(end)]
            #print("got match for", field, "bytes", str(ret_bytes))
            return ret_bytes
        else:
            None
    
    def set_field_if_exists(self, field, value, allow_overwrite=False):
        """
        """
        if allow_overwrite is False:
            raise Exception("We assume instruction to be read only")
        if field in self._instr_details._var_operands:
            beg, end = self._arg_lut[field]
            #print("Setting field", field, "to", value)
            self._instruction_integer = utils.set_operand(self._instruction_integer, value, beg, end)
            self._instruction_bytes = self._instruction_integer.to_bytes(4, "little")
        else:
            pass

    def create_copy_with_field_changed(self, field, value):
        """
        Create a copy of the instruction with the specified field changed to the new value.
        If the field does not exist, return a copy of the original instruction.
        """
        if self._instr_details is None:
            return Instruction(self._instruction_bytes, self._instr_details)
        if field in self._instr_details._var_operands:
            beg, end = self._arg_lut[field]
            new_instruction_integer = utils.set_operand(self._instruction_integer, value, beg, end)
            new_instruction_bytes = new_instruction_integer.to_bytes(4, "little")
            return Instruction(new_instruction_bytes, self._instr_details)
        else:
            return Instruction(self._instruction_bytes, self._instr_details)

    def __str__(self):
        """
        Returns a string representation of the instruction.
        """
        if self._mnemonics is None:
            return f"Unknown instruction: 0x{self._instruction_integer:08x}"
        # Add field values to the mnemonics string
        if self._instr_details is not None:
            field_info = []
            for field in self._instr_details._var_operands:
                value = self.get_field_if_exists_as_int(field)
                if value is not None:
                    field_info.append(f"{field}={value}")
            if field_info:
                return f"{self._mnemonics} ({', '.join(field_info)})"
        return self._mnemonics



