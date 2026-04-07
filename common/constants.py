import sys
from pathlib import Path

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))

import enum
INVARIANT_FINDER_LIBRARY_PATH = "formula_finder/target/release/libinvariant_finder_rust.so"
BINARY_SUBDIR = "binaries"
WAVEFORM_SUBDIR = "waveforms"
BENIGN_EXAMPLES_PATH = "benign_examples"
CEX_PATH ="cexs"
SEEDS_DIR = "seeds"
INVARIANT_PATH = "invariants"
SEED_INVARIANT_PATH = "seed_invariants"
JG_FOUND_CEXS = "jg_found_cexs"
JG_FOUND_BENIGN = "jg_found_benign"
CLK_SIGNAL = "TOP.correctness.clk"
CURRENT_CEX_LENGTH = 1
MAX_CEX_LENGTH = 5
IGNORE_DIFFERING_BUG_TYPE = True # Do not filter out generatex CEX that have different bug types
WAVEFORM_FILE_SUFFIX = ".fst" # Change to .vcd if you want to use VCD files instead of FST files
MAX_MUTATION_STEPS = 3
MUTATION_PER_STEP = 500
BEX_MULTIPLIER = 10 #250 # 0 means no BEX, 1000 means always BEX

WAVEFORM_PATH_KEY = "waveform_path"
