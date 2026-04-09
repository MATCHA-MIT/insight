#!/usr/bin/env python3

import os
import sys
import random
import subprocess
from pathlib import Path

# Add project root to sys.path to import constants if needed
project_root = Path(__file__).parent.parent
if str(project_root) not in sys.path:
    sys.path.append(str(project_root))

try:
    import common.constants as constants
except ImportError:
    # Fallback if constants cannot be imported directly
    constants = None

def generate_nop():
    return f"   addi x0, x0, 0"

def generate_random_arithmetic_instruction():
    # Define possible arithmetic instructions
    arithmetic_instructions = ['add', 'sub', 'mul', 'div']
    registers = [f'x{i}' for i in range(32)]

    # Randomly select an instruction and registers
    instruction = random.choice(arithmetic_instructions)
    rd = random.choice(registers)
    rs1 = random.choice(registers)
    rs2 = random.choice(registers)

    return f"   {instruction} {rd}, {rs1}, {rs2}"

def generate_random_jump_instruction():
    # Define possible jump instructions
    jump_instructions = ['beq', 'bne', 'blt', 'bge', 'jal']
    registers = [f'x{i}' for i in range(32)]

    # Randomly select an instruction and registers
    instruction = random.choice(jump_instructions)
    rs1 = random.choice(registers)
    rs2 = random.choice(registers)

    # Higher likelihood for addresses around 80000000
    address = random.choice([random.randint(0x7ff00000, 0x80100000) for _ in range(8)] +
                            [random.randint(0, 0xffffffff) for _ in range(2)])

    if instruction == 'jal':
        return f"   {instruction} {rs1}, {hex(address)}"
    else:
        return f"   {instruction} {rs1}, {rs2}, {hex(address)}"

def generate_random_csr_instruction():
    # Define possible CSR instructions
    csr_operations = ['csrrw', 'csrrs', 'csrrc', 'csrrwi', 'csrrsi', 'csrrci']
    registers = [f'x{i}' for i in range(32)]
    csrs = [f'0x{i:03X}' for i in range(4096)]  # Example CSR addresses

    # Randomly select an operation, register, and CSR
    operation = random.choice(csr_operations)
    rd = random.choice(registers)
    csr = random.choice(csrs)

    if operation.endswith('i'):
        return f"   {operation} {rd}, {csr}, 1"
    else:
        rs1 = random.choice(registers)
        return f"   {operation} {rd}, {csr}, {rs1}"

def generate_random_load_store_instruction():
    # Define possible load/store instructions (RV32I only)
    load_instructions = ['lb', 'lh', 'lw', 'lbu', 'lhu']
    store_instructions = ['sb', 'sh', 'sw']
    registers = [f'x{i}' for i in range(32)]

    # Randomly select an instruction type
    if random.choice([True, False]):
        instruction = random.choice(load_instructions)
        rd = random.choice(registers)
        rs1 = random.choice(registers)
        offset = random.randint(0, 0x3F)  # 6-bit length range
        return f"   {instruction} {rd}, {offset}({rs1})"
    else:
        instruction = random.choice(store_instructions)
        rs1 = random.choice(registers)
        rs2 = random.choice(registers)

        # Restrict offset for store instructions to 6-bit range
        offset = random.randint(0, 0x3F)  # 6-bit length range

        # Generate a load immediate instruction before the store instruction
        immediate_value = random.randint(0, 0xFFF)  # 12-bit length range
        li_instruction = f"   li {rs2}, {immediate_value}"
        store_instruction = f"   {instruction} {rs2}, {offset}({rs1})"

        return f"{li_instruction}\n{store_instruction}"

def generate_random_other_instruction():
    # Define possible other instructions
    other_instructions = ['nop', 'fence', 'ecall', 'ebreak']

    # Randomly select an instruction
    instruction = random.choice(other_instructions)

    return f"   {instruction}"

def generate_random_invalid_instruction():
    """Generate intentionally invalid RISC-V instructions"""
    # Only generate .word instructions with random opcodes
    return f"   .word 0x{random.randint(0, 0xFFFFFFFF):08X}"

def generate_random_instruction_file(out_dir, file_index, valid=True):
    # Generate three random instructions
    num_inst_random = random.randint(1, 3)
    if valid:
        instructions = [
            random.choice([
                generate_random_arithmetic_instruction,
                generate_random_jump_instruction,
                generate_random_csr_instruction,
                generate_random_load_store_instruction,
                generate_random_other_instruction
            ])() for _ in range(num_inst_random)
        ]
        prefix = "valid_instructions"
    else:
        # Mix of invalid and potentially valid instructions, with at least one invalid
        instructions = [generate_random_invalid_instruction()]  # Ensure at least one invalid
        if num_inst_random > 1:
            instructions += [
                random.choice([
                generate_random_invalid_instruction,
                generate_random_arithmetic_instruction,
                generate_random_jump_instruction,
                ])() for _ in range(num_inst_random - 1)
            ]
        prefix = "invalid_instructions"

    # Define the file content
    content = """.section .text
.global _start

_start:
""" + "\n".join(instructions) + "\n"

    # Define the file name
    file_name = f"{prefix}_{file_index}.s"
    file_path = os.path.join(out_dir, file_name)

    # Write the content to the file
    with open(file_path, 'w') as file:
        file.write(content)
    
    return file_name

def generate_csr_read_file(out_dir, csr_address):
    """Generate a file that reads from a specific CSR into x1"""
    # Define the file content
    content = f""".section .text
.global _start

_start:
   csrrs x1, 0x{csr_address:03X}, x0
"""

    # Define the file name
    file_name = f"csr_read_{csr_address:03X}.s"
    file_path = os.path.join(out_dir, file_name)

    # Write the content to the file
    with open(file_path, 'w') as file:
        file.write(content)
        
    return file_name

def compile_to_bin(source_file, bin_file):
    """Compile a .s file to a .bin file using the provided shell script."""
    try:
        # Use relative path from project root
        subprocess.run(['./util_scripts/compile_testcase_to_hex.sh', source_file, bin_file], check=True)
    except subprocess.CalledProcessError as e:
        print(f"Error compiling {source_file}: {e}")

def generate_assembly_files(out_dir, num_files=1000):
    # Ensure the output directory exists
    os.makedirs(out_dir, exist_ok=True)
    
    source_dir = os.path.join(out_dir, "source")
    bin_dir = os.path.join(out_dir, "binaries")
    
    os.makedirs(source_dir, exist_ok=True)
    os.makedirs(bin_dir, exist_ok=True)

    # Generate valid instruction files
    for i in range(num_files):
        s_file = generate_random_instruction_file(source_dir, i, valid=True)
        bin_file = s_file.replace(".s", ".bin")
        compile_to_bin(os.path.join(source_dir, s_file), os.path.join(bin_dir, bin_file))
    
    #Generate invalid instruction files
    for i in range(num_files):
        s_file = generate_random_instruction_file(source_dir, i, valid=False)
        bin_file = s_file.replace(".s", ".bin")
        compile_to_bin(os.path.join(source_dir, s_file), os.path.join(bin_dir, bin_file))

    # Generate CSR read files for all possible CSRs (0x000 to 0xFFF)
    for csr_addr in range(4096):
        s_file = generate_csr_read_file(source_dir, csr_addr)
        bin_file = s_file.replace(".s", ".bin")
        compile_to_bin(os.path.join(source_dir, s_file), os.path.join(bin_dir, bin_file))

    print(f"Files generated and compiled successfully: {num_files} valid, {num_files} invalid, and 4096 CSR read files")

def ensure_seeds_exist(seeds_dir=None):
    """
    Check if seeds exist in the specified directory.
    If not, generate them.
    """
    if seeds_dir is None:
        if constants:
            # constants.SEEDS_DIR is typically "insight_output/seeds/binaries"
            seeds_dir = os.path.dirname(constants.SEEDS_DIR)
        else:
            seeds_dir = "insight_output/seeds"
    
    binaries_dir = os.path.join(seeds_dir, "binaries")
    if os.path.exists(binaries_dir) and os.listdir(binaries_dir):
        # Seeds already exist
        return
    
    print(f"Seeds not found in {seeds_dir}. Generating...")
    generate_assembly_files(seeds_dir)

if __name__ == "__main__":
    if len(sys.argv) > 2:
        print("Usage: ./script.py [<out_dir>]")
        sys.exit(1)

    if len(sys.argv) == 2:
        out_dir = sys.argv[1]
    else:
        out_dir = "insight_output/seeds"
        
    generate_assembly_files(out_dir)
