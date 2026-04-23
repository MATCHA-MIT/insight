# INSIGHT

This repository contains the source code for the paper:
INSIGHT: Automatic Generation of Explanations for Efficient Identification of Hardware Bugs and Underspecifications
by *Vincent Quentin Ulitzsch, Alessandro Bertani, Peter William Deutsch, David Langus Rodriguez, Kelly Xu, Aarti Gupta, Sharad Malik, and Mengjia Yan*.
Appearing in *2026 IEEE Symposium on Security and Privacy (S&P)*.

## Project Structure

This project is structured as follows.
- `mutation_engine/`: The Rust-based RISC-V mutation engine used to generate variants.
- `formula_finder/`: The Rust FFI engine for separator inference. (Formerly `invariant_finder_rust`)
- `tools/verilator/`: Custom Verilator used for fast simulation.
- `example_cores/`: JasperGold setups, Verilator testbenches, and configs for BOOM and Kronos.
- `orchestration/`: Pipeline loop, including model checking algorithms and deduplication.
- `common/`: Core utilities and the counterexample generation logic.
- `plotting/`: Scripts used to generate evaluation plots.

## How to use

### Build & Setup

Initialize the project, install dependencies and build the Rust modules:
```bash
./setup.sh
source .venv/bin/activate
```
*(Make sure to compile example cores testbenches as needed for your target).*

### Running the Pipeline

You can run the model checking loops via the orchestrator:

```bash
python orchestration/model_checking_algorithm.py --config example_cores/configs/<your_config>.json \
    --bex-weight 50 --predicate-cost 10
```

### Deduplication

To run the deduplication pipeline separately on discovered counterexamples:
```bash
python orchestration/deduplicate.py --config example_cores/configs/<your_config>.json
```

Read `AGENTS.md` for more technical details into the codebase components.

### Reproducing Paper Results

To reproduce the model checking results from our paper on the BOOM and Kronos configurations, you can run the provided scripts in the `reproduce/` directory.

- **BOOM (Buggy & Cascade):**
  ```bash
  ./reproduce/reproduce_boom_model_checking.sh
  ```
- **Kronos (Baseline & Cascade):**
  ```bash
  ./reproduce/reproduce_kronos_model_checking.sh
  ```

  These scripts demonstrate the pipeline behavior with INSIGHT enabled (testing testing differing parameters) and disabled (`--no-insight`).

## Porting to a New Core

To run INSIGHT on a new RISC-V core:

1. **Set up Verilator Testbench:** Create a new folder under `example_cores/compare_to_<core>/`. You must provide:
   - A TCL script for JasperGold configuration (`correctness_template.tcl`) finding CEX that point to a difference between the DUT and the reference model.
   - A Verilator testbench wrapper that compares the DUT committing state with the reference model's committing state.
   - You can copy the setup from an existing core (e.g., `compare_to_kronos`) and adapt the signal mappings in your Verilator C++ testbench and JasperGold TCL.
2. **Create a Pipeline Config:** Add a JSON file in `example_cores/configs/`. This configuration defines:
   - `verilator_testbench_path`: Path to the compiled Verilator binary for your setup.
   - `core_name`: Used to identify the core in output directories.
   - `jaspergold_server`: Remote server details (if your verification runs over SSH).
3. **Regex Mapping Config:** This JSON configuration is linked from the Pipeline Config. It defines your core's memory sizes, formatting strings, and regular expressions to parse Verilator output for signal extraction mappings.

Once your Verilator testbench builds successfully and JasperGold can prove simple properties given your TCL file, you can point `model_checking_algorithm.py` at your new pipeline config to start the INSIGHT loop!

## Citation

If you use INSIGHT in your research, please cite our paper:

```bibtex
@inproceedings{ulitzsch2026insight,
  title={{INSIGHT}: Automatic Generation of Explanations for Efficient Identification of Hardware Bugs and Underspecifications},
  author={Ulitzsch, Vincent Quentin and Bertani, Alessandro and Deutsch, Peter William and Rodriguez, David Langus and Xu, Kelly and Gupta, Aarti and Malik, Sharad and Yan, Mengjia},
  booktitle={2026 IEEE Symposium on Security and Privacy (S\&P)},
  year={2026},
  organization={IEEE}
}
```
