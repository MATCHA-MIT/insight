#!/bin/bash
set -e

echo "Setting up Python virtual environment..."
python3 -m venv .venv
source .venv/bin/activate
unset CONDA_PREFIX


echo "Installing python dependencies..."
pip install --upgrade pip
pip install -r requirements.txt

echo "Building RISC-V opcodes..."
cd mutation_engine/riscv-opcodes
python parse.py rv_i rv32_i rv_s rv_zicsr rv_zifencei

echo "Building Rust RISC-V Mutator..."
cd ../rust_mutator
maturin develop --release
cd ../../

echo "Building Rust Formula Finder (Separator Inference)..."
cd formula_finder
cargo build --release
cd ..

echo "Setup completed successfully! Please run 'source .venv/bin/activate' before running INSIGHT."
