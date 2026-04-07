# AGENTS

## Workspace purpose
This workspace compares a Sodor reference core against BOOM variants for formal checking, Verilator builds, and counterexample generation.

## Current top-level layout
- `correctness.sv`: top-level comparison harness.
- `Makefile`: multi-variant Verilator build entrypoint.
- `boom_designs/`: BOOM RTL variants used by the build.
- `build/`: per-variant build outputs.
- `build_config/`: shared config and Verilator file list.
- `rtl/`: shared simulation and memory RTL.
- `scripts/`: generation and rebuild scripts.
- `sodor_verilog/`: Sodor and Vincent RTL.
- `templates/`: formal TCL templates.
- `verilog_original/`: original generated Verilog snapshot.
- `generates_files/`: generated helper artifacts.

## BOOM variant structure
`boom_designs/` now contains one folder per BOOM variant:
- `boom_baseline/`: baseline BOOM used as the default test target.
- `boom_all_fix/`: all bug fixes enabled.
- `boom_no_b1_fix/` through `boom_no_b5_fix/`: variants with one fix removed.

Each BOOM variant directory contains:
- generated BOOM `.sv` RTL files,
- the matching `*.mems.v` files,
- canonical `SmallBoomV3Config.*.mems.v` names when needed for shared filelist compatibility.

## Build output structure
`build/` mirrors the BOOM variant names:
- `build/boom_baseline/`
- `build/boom_all_fix/`
- `build/boom_no_b1_fix/` ... `build/boom_no_b5_fix/`

Each build directory holds Verilator objects and the produced `libcorrectness.so`.

## Shared configuration notes
- `build_config/filelist.f` is shared across variants.
- The build system selects a BOOM variant with `-y boom_designs/<variant>`.
- `templates/correctness_template.tcl` should point at `boom_designs/boom_baseline` when testing the baseline BOOM.

## Script notes
- `scripts/compile_scala_and_get_verilog.sh` regenerates Verilog, then rebuilds the comparison target.
- Variant generation scripts are expected to keep BOOM variant directories deterministic and compatible with the shared file list.
