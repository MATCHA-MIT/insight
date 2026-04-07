#!/usr/bin/env python3

import os
import sys
from pathlib import Path

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))

import json
import tempfile
import struct
from pathlib import Path
from multiprocessing import Pool, cpu_count

# Add paths for analyzer and constants as seen in run_full_pipeline.py
REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.append(str(REPO_ROOT))
SUBDIR = REPO_ROOT / "formal-verif" / "invariant_generation" / "vincent_invariant_generator"
sys.path.append(str(SUBDIR))

import analyzer  # type: ignore


class CSRSeparatorGenerator:
    @classmethod
    def from_config_path(cls, config_path, output_dir="csr_separators"):
        with open(config_path, "r") as f:
            config = json.load(f)
        checker_path = config.get("verilator_script")
        if not checker_path:
            raise ValueError("Config must specify 'verilator_script' path (testbench).")
        return cls(checker_path, output_dir)
        
    
    def __init__(self, checker_path, output_dir="csr_separators"):
        # Take inspiration from run_full_pipeline.py for config loading
        #self.output_dir = Path(self.config.get("output_dir", "tmp_output"))
        # In run_full_pipeline.py, checker_path is often configs["verilator_script"]
        self.checker_path = checker_path

        if not self.checker_path:
            raise ValueError("Config must specify 'verilator_script' path (testbench).")

        #self.output_dir.mkdir(parents=True, exist_ok=True)
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

    def generate_bin_for_csr(self, csr_num, tmp_dir):
        """Generates a raw binary consisting of a single CSR read instruction."""
        # Instruction: csrrs x0, csr, x0
        # RISC-V encoding for CSRRS rd, csr, rs1:
        # [csr[11:0]] [rs1[4:0]] [funct3[2:0]] [rd[4:0]] [opcode[6:0]]
        # opcode: 1110011 (0x73), rd: 1 (x1), funct3: 010 (2), rs1: 0 (x0)

        opcode = 0x73
        rd = 0
        funct3 = 2
        rs1 = 0

        instr = (csr_num << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode

        bin_path = Path(tmp_dir) / f"csr_{csr_num:03X}.bin"
        # Write as little-endian 32-bit word
        with open(bin_path, "wb") as f:
            f.write(struct.pack("<I", instr))

        return bin_path

    def run_testbench(self, bin_path):
        """Invokes the testbench using the analyzer check_commit_log."""
        # Take inspiration from run_full_pipeline.py for calling the checker
        result = analyzer.check_commit_log(self.checker_path, str(bin_path), None)
        if result is None:
            return False

        # Return True if result is a Counter-Example (CEX)
        return getattr(result, "Kind", None) == analyzer.CheckerResultKind.CEX

    def generate_csr_separator_for_mismatches(self, mismatched_csrs):
        """Generate separators for all mismatched CSRs."""
        """Use ValueIn Predicate Like so
        {
            "base_formula": {
                "ValueIn": {
                "signal_name": "TOP.core.opcode",
                "signal_idx": 42,
                "signal_length": 7,
                "signal_types": {
                    "types": ["Control"]
                },
                "values": [51, 19, 35]
                }
            }
        }
        """
        #First, construct the assume string for JG as a big disjunction of all mismatched CSRs
        assume_str = "assume {{!(funct3 != 3'b000 && opcode == 7'h73 && imm_i inside {" + ", ".join(f"12'h{csr:03X}" for csr in mismatched_csrs) + "})}}"
        print(f"Generated assume invariant for JG:\n{assume_str}")
        separator = {
            "separator_formula": {
                "InvariantDisjunction": {
                    "disjunctions": [
                        {
                            "predicate_set": {
                                "predicates": [
                                    {
                                        "base_formula": {
                                            "SignalToConst": {
                                                "signal_name": "TOP.correctness.sodor_core.core.d.opcode",
                                                "signal_idx": 25,
                                                "operator": "Equal",
                                                "signal_length": 7,
                                                "signal_types": {
                                                    "types": [
                                                        "Control"
                                                    ]
                                                },
                                                "value": 115
                                            }
                                        }
                                    },
                                    {
                                        "base_formula": {
                                            "SignalToConst": {
                                                "signal_name": "TOP.correctness.funct3",
                                                "signal_idx": 28,
                                                "operator": "NotEqual",
                                                "signal_length": 3,
                                                "signal_types": {
                                                    "types": [
                                                        "Control"
                                                    ]
                                                },
                                                "value": 0
                                            }
                                        }
                                    },
                                    {
                                        "base_formula": {
                                            "ValueIn": {
                                                "signal_name": "TOP.correctness.imm_i",
                                                "signal_idx": 31,
                                                "signal_length": 12,
                                                "signal_types": {
                                                    "types": [
                                                        "Immediate",
                                                        "RegisterFileAddress"
                                                    ]
                                                },
                                                "values": mismatched_csrs
                                            }
                                        }
                                    }
                                ]
                            }
                        }
                    ]
                }
            },
            "assume_invariant": assume_str
        }
        # Save to output directory
        output_file = self.output_dir / f"ignore_invalid_csrs.json"
        with open(output_file, "w") as f:
            json.dump(separator, f, indent=2)
        print(f"  Saved separator to {output_file}")
        return output_file

    def generate_csr_separator(self, csr_num):
        """Function to generate an invariant for a mismatched CSR."""
        print(f"Generating csr separator for 0x{csr_num:03X}")
        
        # Create the separator formula constraining imm_i to this specific CSR
        # Following the structure from out.json
        separator = {
            "separator_formula": {
                "InvariantDisjunction": {
                    "disjunctions": [
                        {
                            "predicate_set": {
                                "predicates": [
                                    {
                                        "base_formula": {
                                            "SignalToConst": {
                                                "signal_name": "TOP.correctness.sodor_core.core.d.opcode",
                                                "signal_idx": 25,
                                                "operator": "Equal",
                                                "signal_length": 7,
                                                "signal_types": {
                                                    "types": [
                                                        "Control"
                                                    ]
                                                },
                                                "value": 115
                                            }
                                        }
                                    },
                                    {
                                        "base_formula": {
                                            "SignalToConst": {
                                                "signal_name": "TOP.correctness.imm_i",
                                                "signal_idx": 31,
                                                "operator": "Equal",
                                                "signal_length": 12,
                                                "signal_types": {
                                                    "types": [
                                                        "Immediate",
                                                        "RegisterFileAddress"
                                                    ]
                                                },
                                                "value": csr_num
                                            }
                                        }
                                    },
                                    {
                                        "base_formula": {
                                            "SignalToConst": {
                                                "signal_name": "TOP.correctness.sodor_core.core.d.regfile_ext.W0_addr",
                                                "signal_idx": 30,
                                                "operator": "NotEqual",
                                                "signal_length": 5,
                                                "signal_types": {
                                                    "types": [
                                                        "Register"
                                                    ]
                                                },
                                                "value": 0
                                            }
                                        }
                                    }
                                ]
                            }
                        }
                    ]
                }
            },
            "assume_invariant": f"assume {{!(TOP.correctness.sodor_core.core.d.opcode == 7'h73 && TOP.correctness.imm_i == 12'h{csr_num:03X} && TOP.correctness.sodor_core.core.d.regfile_ext.W0_addr != 5'h0)}}"
        }
        
        # Save to output directory
        output_file = self.output_dir / f"csr_separator_0x{csr_num:03X}.json"
        with open(output_file, "w") as f:
            json.dump(separator, f, indent=2)
        
        print(f"  Saved separator to {output_file}")

    def process_single_csr(self, csr_num):
        """Process a single CSR number and return result."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            try:
                bin_path = self.generate_bin_for_csr(csr_num, tmp_dir)

                if self.run_testbench(bin_path):
                    #print(f"Mismatch (CEX) detected for CSR 0x{csr_num:03X}")
                    #self.generate_csr_separator(csr_num)
                    return csr_num
            except Exception as e:
                print(f"Error processing CSR 0x{csr_num:03X}: {e}")
        return None

    def run(self, num_workers=None) -> str:
        """Main loop iterating through all possible 12-bit CSRs."""
        if num_workers is None:
            num_workers = max(1, cpu_count() - 1)
        
        print(f"Scanning CSRs [0x000-0xFFF] using testbench: {self.checker_path}")
        print(f"Using {num_workers} parallel workers")

        with Pool(num_workers) as pool:
            results = pool.map(self.process_single_csr, range(4096))
        
        mismatched = [csr for csr in results if csr is not None]
        print(f"\nCompleted. Found {len(mismatched)} CSRs with mismatches.")
        return self.generate_csr_separator_for_mismatches(mismatched)
        


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: ./generate_csr_separators.py <config_path>")
        sys.exit(1)

    generator = CSRSeparatorGenerator(sys.argv[1])
    generator.run()
