#!/usr/bin/env python3

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# Setup paths
SCRIPT_DIR = Path(__file__).parent
ROOT_DIR = SCRIPT_DIR.parent

# Add to path
sys.path.insert(0, str(ROOT_DIR))
sys.path.insert(0, str(ROOT_DIR / "common"))

import generate_csr_separators


def setup_gurobi_env():
    """Setup Gurobi environment variables."""
    gurobi_version = "gurobi1201"
    gurobi_home = Path.home() / "gurobi" / gurobi_version / "linux64"
    
    os.environ["GUROBI_VERSION"] = gurobi_version
    os.environ["GUROBI_HOME"] = str(gurobi_home)
    
    gurobi_bin = gurobi_home / "bin"
    gurobi_lib = gurobi_home / "lib"
    
    if gurobi_bin.exists():
        path_parts = os.environ.get("PATH", "").split(os.pathsep)
        if str(gurobi_bin) not in path_parts:
            path_parts.insert(0, str(gurobi_bin))
        os.environ["PATH"] = os.pathsep.join(path_parts)
    
    if gurobi_lib.exists():
        ld_library_path_parts = os.environ.get("LD_LIBRARY_PATH", "").split(os.pathsep)
        if str(gurobi_lib) not in ld_library_path_parts:
            ld_library_path_parts.insert(0, str(gurobi_lib))
        os.environ["LD_LIBRARY_PATH"] = os.pathsep.join(ld_library_path_parts)


def generate_csr_invariant(config_path, output_invariants_dir):
    """Generate the ignore_invalid_csr invariant."""
    try:
        with open(config_path, "r") as f:
            config = json.load(f)
        
        checker_path = config.get("verilator_script")
        if not checker_path:
            print(
                f"Warning: 'verilator_script' not found in config, skipping CSR invariant generation",
                file=sys.stderr
            )
            return None
        
        print(f"Generating CSR separator invariant from testbench: {checker_path}")
        csr_generator = generate_csr_separators.CSRSeparatorGenerator(
            checker_path,
            output_dir=str(output_invariants_dir)
        )
        invalid_csrs_file = csr_generator.run()
        print(f"Generated ignore_invalid_csr invariant: {invalid_csrs_file}")
        return invalid_csrs_file
    except Exception as e:
        print(f"Error generating CSR invariant: {e}", file=sys.stderr)
        return None


def copy_seed_invariants(config_path, output_invariants_dir):
    """Copy seed invariants from config to output directory and return copied paths."""
    copied_invariants = []
    try:
        with open(config_path, "r") as f:
            config = json.load(f)
        
        seed_invariants = config.get("seed_invariants", [])
        for invariant_path in seed_invariants:
            invariant_full_path = Path(invariant_path)
            if not invariant_full_path.is_absolute():
                invariant_full_path = ROOT_DIR / invariant_path
            if invariant_full_path.exists():
                dest = output_invariants_dir / invariant_full_path.name
                shutil.copy(str(invariant_full_path), str(dest))
                print(f"Copied seed invariant: {invariant_full_path.name}")
                copied_invariants.append(str(dest.resolve()))
            else:
                print(f"Warning: Seed invariant not found: {invariant_full_path}", file=sys.stderr)
    except Exception as e:
        print(f"Error copying seed invariants: {e}", file=sys.stderr)
    return copied_invariants


def build_cex_generator_config(config_path, output_dir, seed_invariants):
    """Create a temporary config for cex_generator with explicit seed invariants."""
    with open(config_path, "r") as f:
        config = json.load(f)

    # Preserve order while removing duplicates.
    config["seed_invariants"] = list(dict.fromkeys(seed_invariants))

    cex_config_path = output_dir / "cex_generator_config.json"
    with open(cex_config_path, "w") as f:
        json.dump(config, f, indent=4)

    return cex_config_path


def run_cex_generator(testcase_path, output_dir, config_path, jg_signal_list=None):
    """Run the CEX generator."""
    cmd = [
        sys.executable,
        "-u",
        str(ROOT_DIR / "common" / "cex_generator.py"),
        str(testcase_path),
        str(output_dir),
        "--log-level", "DEBUG",
        "--config-path", str(config_path)
    ]
    
    if jg_signal_list:
        cmd.extend(["--test_allowed_signals", jg_signal_list])
    
    print(f"Running CEX generator: {' '.join(cmd)}")
    result = subprocess.run(cmd, check=True)
    return result.returncode == 0


def build_formula_finder():
    """Build the formula_finder binary if needed."""
    formula_finder_bin = ROOT_DIR / "formula_finder" / "target" / "release" / "invariant_finder_rust"
    
    if formula_finder_bin.exists() and os.access(str(formula_finder_bin), os.X_OK):
        print(f"formula_finder binary already exists: {formula_finder_bin}")
        return formula_finder_bin
    
    print("Building formula_finder...")
    formula_finder_dir = ROOT_DIR / "formula_finder"
    result = subprocess.run(
        ["cargo", "build", "--release"],
        cwd=str(formula_finder_dir),
        check=True
    )
    
    if not formula_finder_bin.exists():
        raise RuntimeError("Failed to build formula_finder binary")
    
    return formula_finder_bin


def run_formula_finder(output_dir, core_config_path, bex_multiplier, predicate_cost):
    """Run the formula_finder binary."""
    output_sets_path = output_dir / "output_sets.json"
    invariant_out_path = output_dir / "invariant.json"
    
    formula_finder_bin = build_formula_finder()
    
    cmd = [
        str(formula_finder_bin),
        "--output-sets", str(output_sets_path),
        "--regex-config", str(core_config_path),
        "--invariant-out-path", str(invariant_out_path),
        "--bex-multiplier", str(bex_multiplier),
        "--predicate-base-cost", str(predicate_cost)
    ]
    
    print(f"Running formula_finder: {' '.join(cmd)}")
    result = subprocess.run(cmd, check=True)
    return result.returncode == 0


def main():
    """Main function."""
    parser = argparse.ArgumentParser(
        description="Run INSIGHT testcase with CEX generation and invariant synthesis"
    )
    
    parser.add_argument(
        "--testcase",
        required=True,
        help="Path to the testcase file"
    )
    parser.add_argument(
        "--output",
        default=None,
        help="Output directory (default: temporary directory)"
    )
    parser.add_argument(
        "--config",
        default=str(ROOT_DIR / "example_cores" / "configs" / "vincent_kronos_pipeline_config.json"),
        help="Config file path"
    )
    parser.add_argument(
        "--bex-weight",
        type=int,
        default=50,
        help="BEX multiplier for formula finder"
    )
    parser.add_argument(
        "--predicate-cost",
        type=int,
        default=50,
        help="Predicate cost for formula finder"
    )
    parser.add_argument(
        "--jg-signal-list",
        default=None,
        help="JasperGold signal list"
    )
    
    args = parser.parse_args()
    
    # Setup Gurobi
    setup_gurobi_env()
    
    # Create output directory if not specified
    if args.output is None:
        output_dir = Path(tempfile.mkdtemp(prefix="output-"))
    else:
        output_dir = Path(args.output).resolve()
    
    # Resolve paths
    config_path = Path(args.config).resolve()
    testcase_path = Path(args.testcase).resolve()
    
    # Ensure testcase exists
    if not testcase_path.exists():
        print(f"Error: testcase file not found: {testcase_path}", file=sys.stderr)
        sys.exit(1)
    
    if not config_path.exists():
        print(f"Error: config file not found: {config_path}", file=sys.stderr)
        sys.exit(1)
    
    # Create output directories
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "benign_examples").mkdir(parents=True, exist_ok=True)
    (output_dir / "cexs").mkdir(parents=True, exist_ok=True)
    (output_dir / "invariants").mkdir(parents=True, exist_ok=True)
    
    # Create seeds directory
    (ROOT_DIR / "seeds").mkdir(parents=True, exist_ok=True)
    
    # Print configuration
    print(f"testcase: {testcase_path}")
    print(f"output: {output_dir}")
    print(f"config: {config_path}")
    print(f"Running with target={testcase_path}, output={output_dir}, config={config_path}")
    print()
    
    # Generate CSR invariant
    print("=" * 80)
    print("Step 1: Generating CSR separator invariant")
    print("=" * 80)
    csr_invariant = generate_csr_invariant(config_path, output_dir / "invariants")
    print()
    
    # Copy seed invariants
    print("=" * 80)
    print("Step 2: Copying seed invariants")
    print("=" * 80)
    seed_invariants = copy_seed_invariants(config_path, output_dir / "invariants")
    if csr_invariant:
        csr_invariant_path = Path(csr_invariant)
        if not csr_invariant_path.is_absolute():
            invariant_candidates = [
                output_dir / "invariants" / csr_invariant_path,
                ROOT_DIR / csr_invariant_path,
            ]
            for candidate in invariant_candidates:
                if candidate.exists():
                    csr_invariant_path = candidate
                    break
            else:
                csr_invariant_path = output_dir / "invariants" / csr_invariant_path.name
        seed_invariants.append(str(csr_invariant_path.resolve()))
    print()
    
    # Run CEX generator
    print("=" * 80)
    print("Step 3: Running CEX generator")
    print("=" * 80)
    cex_config_path = build_cex_generator_config(config_path, output_dir, seed_invariants)
    print(f"Using {len(seed_invariants)} seed invariants for CEX generation")
    run_cex_generator(testcase_path, output_dir, cex_config_path, args.jg_signal_list)
    print()
    
    # Get core config path from config
    try:
        with open(config_path, "r") as f:
            config = json.load(f)
        core_config_path = config.get("regex_config_path")
        if not core_config_path:
            print("Error: 'regex_config_path' not found in config", file=sys.stderr)
            sys.exit(1)
        core_config_path = (ROOT_DIR / core_config_path).resolve()
    except Exception as e:
        print(f"Error reading config: {e}", file=sys.stderr)
        sys.exit(1)
    
    # Run formula finder
    print("=" * 80)
    print("Step 4: Running formula finder")
    print("=" * 80)
    run_formula_finder(output_dir, core_config_path, args.bex_weight, args.predicate_cost)
    print()
    
    print("=" * 80)
    print(f"Output saved to: {output_dir}")
    print(f"Invariant: {output_dir / 'invariant.json'}")
    print("=" * 80)


if __name__ == "__main__":
    main()
