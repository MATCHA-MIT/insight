class InstructionType:
    def __init__(self, instruction_mnemonics: str, extensions: list[str],mask: int, match: int, var_operands: list[str]):
        self._inst = instruction_mnemonics
        self._ext = extensions
        self._mask = mask
        self._match = match
        self._var_operands = var_operands

    def get_var_operands(self) -> list[str]:
        return self._var_operands
    
    def get_mnemonics(self) -> str:
        return self._inst
    
    def get_extensions(self) -> list[str]:
        return self._ext

    def get_match(self) -> int:
        return self._match
    
    def __str__(self) -> str:
        return f"Instruction: {self._inst}\nExtensions: {self._ext}\nMask: 0x{self._mask:02x}\nMatch: 0x{self._match:02x}\nVariable Operands: {self._var_operands}"
    
    def __repr__(self) -> str:
        return f"Instruction: {self._inst}\nExtensions: {self._ext}\nMask: 0x{self._mask:02x}\nMatch: 0x{self._match:02x}\nVariable Operands: {self._var_operands}"
    
    def match(self, instr: int) -> bool:
        """Check if this instruction matches the given instruction
        Args:
            instr (int): The instruction to match
        Returns:
            bool: True if the instruction matches, False otherwise
        """
        return (instr & self._mask) == self._match
