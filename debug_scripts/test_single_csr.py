#!/usr/bin/env python3
"""Debug script to test a single CSR and see what the testbench returns."""

import argparse
import struct
import sys
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
ROOT_DIR = SCRIPT_DIR.parent

sys.path.insert(0, str(ROOT_DIR))
sys.path.insert(0, str(ROOT_DIR / "common"))
sys.path.insert(0, str(ROOT_DIR / "plotting"))

import analyzer


def generate_bin_for_csr(csr_num):
    """Generate a binary with a single CSR read instruction."""
    # Instruction: csrrs x0, csr, x0
    # RISC-V encoding: [csr[11:0]] [rs1[4:0]] [funct3[2:0]] [rd[4:0]] [opcode[6:0]]
    opcode = 0x73  # SYSTEM
    rd = 0         # x0
    funct3 = 2     # CSRRS
    rs1 = 0        # x0
    
    instr = (csr_num << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
    
    bin_data = struct.pack("<I", instr)
    return bin_data


def main():
    parser = argparse.ArgumentParser(description="Test a single CSR")
    parser.add_argument("--csr", type=lambda x: int(x, 0), required=True, help="CSR number (hex or decimal)")
    parser.add_argument("--testbench", required=True, help="Path to testbench")
    
    args = parser.parse_args()
    
    csr_num = args.csr
    testbench = args.testbench
    
    print(f"Testing CSR 0x{csr_num:03X}")
    print(f"Testbench: {testbench}")
    print()
    
    # Generate binary
    bin_data = generate_bin_for_csr(csr_num)
    
    with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as tmp:
        tmp.write(bin_data)
        bin_path = tmp.name
    
    print(f"Generated binary: {bin_path}")
    print(f"Instruction encoding: 0x{struct.unpack('<I', bin_data)[0]:08X}")
    print()
    
    try:
        # Run testbench
        print("=" * 60)
        print("Running testbench...")
        print("=" * 60)
        
        with tempfile.NamedTemporaryFile(delete=False, suffix=".fst") as wf:
            waveform_path = wf.name
        
        result = analyzer.check_commit_log(
            testbench,
            bin_path,
            waveform_output=waveform_path,
            check_invariant_satisfaction=False
        )
        
        if result is None:
            print("ERROR: Testbench returned None")
            return 1
        
        print()
        print("=" * 60)
        print("Result:")
        print("=" * 60)
        print(f"Kind: {result.Kind}")
        print(f"Difference Location: {result.difference_location}")
        print(f"Constants: {result.constants}")
        print()
        
        if result.Kind == analyzer.CheckerResultKind.CEX:
            print("✓ MISMATCH DETECTED (CEX)")
            print(f"  Mismatch at instruction #{result.difference_location.instruction_number}")
            print(f"  Ref core cycle: {result.difference_location.clock_cycle_refcore}")
            print(f"  DUT cycle: {result.difference_location.clock_cycle_dut}")
        elif result.Kind == analyzer.CheckerResultKind.BENIGN:
            print("✗ BENIGN (no mismatch)")
            print("  Both cores executed the same way")
        elif result.Kind == analyzer.CheckerResultKind.FULFILLS_INVARIANT:
            print("~ FULFILLS INVARIANT")
            
        print(f"\nWaveform saved to: {waveform_path}")
        
    except Exception as e:
        print(f"ERROR: {e}")
        import traceback
        traceback.print_exc()
        return 1
    
    return 0


if __name__ == "__main__":
    sys.exit(main())
