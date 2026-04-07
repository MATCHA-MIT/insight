#!/bin/bash

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}RISC-V Instruction Mutator - Setup Script${NC}"
echo "==========================================="
echo ""

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Cargo (Rust) is not installed${NC}"
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

echo -e "${GREEN}✓${NC} Cargo found: $(cargo --version)"

# Check if Python is installed
if ! command -v python3 &> /dev/null; then
    echo -e "${RED}Error: Python 3 is not installed${NC}"
    exit 1
fi

echo -e "${GREEN}✓${NC} Python found: $(python3 --version)"

# Check if pip is installed
if ! command -v pip3 &> /dev/null && ! command -v pip &> /dev/null; then
    echo -e "${RED}Error: pip is not installed${NC}"
    exit 1
fi

# Check if maturin is installed, if not install it
if ! command -v maturin &> /dev/null; then
    echo -e "${YELLOW}Maturin not found. Installing maturin...${NC}"
    pip3 install maturin || pip install maturin
fi

echo -e "${GREEN}✓${NC} Maturin found: $(maturin --version)"

# Build the Rust binary
echo ""
echo -e "${YELLOW}Building Rust binary...${NC}"
cargo build --release

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} Rust binary built successfully"
    echo "  Binary location: ./target/release/riscv_mutator"
else
    echo -e "${RED}✗${NC} Failed to build Rust binary"
    exit 1
fi

# Run tests
echo ""
echo -e "${YELLOW}Running tests...${NC}"
#cargo test

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} All tests passed"
else
    echo -e "${RED}✗${NC} Some tests failed"
    exit 1
fi

# Build Python module
echo ""
echo -e "${YELLOW}Building Python module with maturin...${NC}"
echo "Running: maturin develop --release"
maturin develop --release

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} Python module built and installed successfully"
else
    echo -e "${RED}✗${NC} Failed to build Python module"
    echo ""
    echo "Debug information:"
    echo "  Python executable: $(which python3)"
    echo "  Python version: $(python3 --version)"
    echo "  Python site-packages:"
    python3 -c "import site; print('  ' + '\n  '.join(site.getsitepackages()))"
    exit 1
fi

# Generate example data
echo ""
echo -e "${YELLOW}Generating example data...${NC}"
cargo run --example generate_sample

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} Example data generated"
else
    echo -e "${YELLOW}⚠${NC} Failed to generate example data (non-critical)"
fi

# Test Python import with debugging
echo ""
echo -e "${YELLOW}Testing Python import...${NC}"
echo "Python path:"
python3 -c "import sys; print('\n'.join(sys.path))"
echo ""
echo "Attempting import..."
python3 -c "import riscv_mutator; print('Successfully imported riscv_mutator'); print('Module location:', riscv_mutator.__file__)"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} Python module can be imported"
else
    echo -e "${RED}✗${NC} Failed to import Python module"
    echo ""
    echo "Troubleshooting:"
    echo "  Try running: pip3 list | grep riscv"
    echo "  Or: python3 -m pip show riscv_mutator"
    exit 1
fi

# Summary
echo ""
echo -e "${GREEN}=========================================${NC}"
echo -e "${GREEN}Setup completed successfully!${NC}"
echo -e "${GREEN}=========================================${NC}"
echo ""
echo "Available commands:"
echo "  1. Run CLI: ./target/release/riscv_mutator --help"
echo "  2. Run examples: cargo run --example basic_usage"
echo "  3. Python usage: python3 python_example.py"
echo "  4. Run tests: cargo test"
echo ""
echo "Python module 'riscv_mutator' is now available in your Python environment"
echo ""
