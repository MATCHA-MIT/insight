# AGENTS.md

## Project Overview

This is the implementation of the **INSIGHT** paper. It implements an automated model checking pipeline for RISC-V processor verification using counterexample-guided separator inference. The system finds bugs in RISC-V CPU implementations by comparing a design-under-test (DUT) against a reference core using formal verification (JasperGold) and Verilator simulation.

## Architecture

The pipeline is orchestrated by `orchestration/model_checking_algorithm.py` and follows this loop:

1. **JasperGold Query** — A formal verification tool (JasperGold) runs to find counterexamples (CEXs) that demonstrate differences between the DUT and a reference core.
2. **CEX Mutation** — The `common/cex_generator.py` module mutates found counterexamples to produce variants, generating both counterexamples and benign examples (BEXs).
3. **Separator Inference** — A Rust FFI library (`formula_finder`) synthesizes invariants (separators) that distinguish counterexamples from benign behavior, using a grammar-based sampling approach.
4. **Invariant Feedback** — Discovered invariants are fed back as assumptions into the formal verification setup to prune the search space, and the loop repeats.

## Key Components

### Core Modules
- **`orchestration/model_checking_algorithm.py`** — Main pipeline orchestrator. Contains the central PipelineLoop.
- **`common/cex_generator.py`** — Handles counterexample mutation via RISC-V instruction mutation.
- **`plotting/analyzer.py`** — Waveform analysis and commit log checking. 
- **`common/common.py`** — Shared utilities including FFI calls for bulk invariant checking.
- **`common/constants.py`** — Global constants.
- **`common/generate_csr_separators.py`** — Generates invariants for invalid CSR accesses.

### Formula Finder Engine (Rust FFI Library)
- Located at `formula_finder/`
- Compiled to a shared library
- Provides: invariant generation and validation through CFFI.

### Mutation Engine
- Located at `mutation_engine/`
- Provides program mutations for RISC-V assembly via Rust modules.

### Example Cores & Configurations
- `example_cores/` — Contains comparison setups (e.g., `compare_to_boom_buggy`, `compare_to_kronos`, `compare_to_boom_cascade`, `compare_to_kronos_cascade`, `common`)
- `example_cores/configs/` — JSON configurations specifying JasperGold server settings, output directories, and regex config paths.

## Deduplication + Evaluation Workflow

### 1) Deduplicate counterexamples

```bash
python orchestration/deduplicate.py --config example_cores/configs/<pipeline_config>.json
```

### 2) Compute classes + generalization metrics

```bash
bash orchestration/compute_classes_and_metrics.sh [CEXS_DIR] [SWEEP_DIR] [CLASS_FILE]
```

### One-command automation

```bash
python orchestration/run_dedup_and_metrics.py --config example_cores/configs/<pipeline_config>.json
```

## Supported Cores
- **Kronos** — RISC-V 32-bit core
- **BOOM** — RISC-V 64-bit out-of-order core (64-bit memory)

## Setup
Before running the pipeline or reproducing results, ensure the environment is correctly set up:

```bash
./setup.sh
source .venv/bin/activate
```

This installs Python dependencies, builds the RISC-V rotation mutator, and compiles the Rust formula finder.

## Reproducing Results

To reproduce the deduplication results and generate evaluation plots using testcases from Zenodo (make sure you have run the setup steps above):

```bash
./reproduce_deduplication_results.sh
```

This script will:
1. Download testcases for BOOM and Kronos from Zenodo.
2. Extract them into the `results/` directory.
3. Build the Rust formula finder engine.
4. Run deduplication and compute generalization metrics for both cores.
5. Generate evaluation plots (found in `insight_output/<core>/deduplication/deduplication_*/plots`).

## Directory Conventions
- `insight_output/` / `output/` — Pipeline output directories containing CEXs, benign examples, invariants, and logs.
