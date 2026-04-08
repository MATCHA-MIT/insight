#!/usr/bin/env python3
import argparse
import subprocess
import os
import tempfile
import re
import binascii
import sys

def parse_assembly(assembly_text, arch="rv64im_zicsr", abi="lp64"):
    """Assembles the given text and returns raw bytes."""
    with tempfile.NamedTemporaryFile(suffix=".s", mode="w", delete=False) as f:
        f.write(".section .text\n")
        f.write(".globl _start\n")
        f.write("_start:\n")
        f.write(assembly_text + "\n")
        # Add a trailing jump-to-self to prevent hanging in simulation
        f.write("1: j 1b\n")
        temp_s = f.name
    
    temp_o = temp_s + ".o"
    temp_bin = temp_s + ".bin"
    
    try:
        # Assemble
        subprocess.check_call([
            "riscv64-unknown-elf-as",
            "-march=" + arch,
            "-mabi=" + abi,
            "-o", temp_o,
            temp_s
        ])
        # Objcopy to binary
        subprocess.check_call([
            "riscv64-unknown-elf-objcopy",
            "-O", "binary",
            "-S", "-j", ".text",
            temp_o, temp_bin
        ])
        with open(temp_bin, "rb") as f:
            data = f.read()
            return data
    finally:
        for p in [temp_s, temp_o, temp_bin]:
            if os.path.exists(p):
                os.remove(p)

def parse_hex_string(hex_text):
    """Parses a string of hex opcodes (4 bytes each) and returns raw bytes."""
    clean_text = re.sub(r'#.*', '', hex_text)
    clean_text = re.sub(r'\s+', '', clean_text)
    
    hex_list = re.findall(r'[0-9a-fA-F]{8}', clean_text)
    data = bytearray()
    for h in hex_list:
        word = int(h, 16)
        data.extend(word.to_bytes(4, 'little'))
    return bytes(data)

def parse_disassembly(dis_text):
    """Parses objdump-style disassembly and returns raw bytes."""
    hex_opcodes = []
    lines = dis_text.splitlines()
    for line in lines:
        match = re.search(r'^\s*[0-9a-f]+:\s+([0-9a-fA-F]{8})', line)
        if match:
            hex_opcodes.append(match.group(1))
    
    if not hex_opcodes:
        hex_opcodes = re.findall(r'([0-9a-fA-F]{8})(?=\s+[a-z])', dis_text)
        
    data = bytearray()
    for h in hex_opcodes:
        word = int(h, 16)
        data.extend(word.to_bytes(4, 'little'))
    return bytes(data)

def main():
    parser = argparse.ArgumentParser(description="Test RISC-V instruction sequences in BOOM buggy.")
    parser.add_argument("input", help="Instruction sequence (assembly, hex string, or disassembly snippet)")
    parser.add_argument("--type", choices=["asm", "hex", "dis", "auto"], default="auto", help="Input type (default: auto)")
    parser.add_argument("--last-only", action="store_true", help="Only test the last instruction in the sequence")
    parser.add_argument("--debug", action="store_true", help="Run with waveform debugging (sets DEBUG=1)")
    parser.add_argument("--out-bin", help="Save the generated binary to this path")
    parser.add_argument("--nops", type=int, default=5, help="Number of padding NOPs to add at the end (default: 5)")

    if len(sys.argv) == 1:
        parser.print_help()
        sys.exit(1)

    args = parser.parse_args()

    input_data = args.input
    input_type = args.type
    
    if input_type == "auto":
        if ":" in input_data and re.search(r'[0-9a-f]+:', input_data):
            input_type = "dis"
        elif re.search(r'[a-zA-Z]{2,}', input_data) and not all(c in "abcdefABCDEF \n\t" for c in input_data):
            input_type = "asm"
        else:
            input_type = "hex"
    
    print(f"Detected input type: {input_type}")

    if input_type == "asm":
        data = parse_assembly(input_data)
    elif input_type == "hex":
        data = parse_hex_string(input_data)
    else:
        data = parse_disassembly(input_data)

    if not data:
        print("Error: No instructions found in input.")
        sys.exit(1)

    if args.last_only:
        if len(data) >= 4:
            data = data[-4:]
            print(f"Isolating last instruction: {binascii.hexlify(data[::-1]).decode()}")

    if input_type != "asm":
        nop = (0x00000013).to_bytes(4, 'little')
        for _ in range(args.nops):
            data += nop
        loop = (0x0000006f).to_bytes(4, 'little')
        data += loop

    with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as f:
        f.write(data)
        temp_bin = f.name
    
    if args.out_bin:
        with open(args.out_bin, "wb") as f:
            f.write(data)
        print(f"Binary saved to {args.out_bin}")

    print(f"Running simulation with {len(data)//4} instructions...")
    
    env = os.environ.copy()
    if args.debug:
        env["DEBUG"] = "1"
    
    cmd = ["./util_scripts/run_boom_buggy.sh", temp_bin]
    
    try:
        result = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=60)
        print(result.stdout)
        print(result.stderr)
        
        if "Correct 1" in result.stdout:
            print("\x1b[32mRESULT: BENIGN (No Mismatch detected)\x1b[0m")
        elif "Correct 0" in result.stdout:
            print("\x1b[31mRESULT: MISMATCH DETECTED!\x1b[0m")
            match = re.search(r"Mismatch at index\s+(\d+)", result.stdout)
            if match:
                idx = int(match.group(1))
                print(f"Mismatch occurred at instruction index: {idx}")
        else:
            print("RESULT: Unknown (neither 'Correct 1' nor 'Correct 0' found in output)")

    except subprocess.TimeoutExpired:
        print("\x1b[33mRESULT: TIMEOUT (Simulation took too long)\x1b[0m")
    except Exception as e:
        print(f"Error running simulation: {e}")
    finally:
        if os.path.exists(temp_bin):
            os.remove(temp_bin)

if __name__ == "__main__":
    main()
