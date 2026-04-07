# RISC-V Instruction Mutator (Rust)

A high-performance, parallel RISC-V instruction mutator written in Rust with Python bindings.

## Features

- **Multiple Mutation Types**: Change opcodes, operands (rd, rs1, rs2), funct3, funct7, immediates, swap/delete/insert instructions
- **Parallel Processing**: Uses Rayon for efficient parallel mutation generation
- **Clean Architecture**: Modular design with separate concerns for instructions, mutations, and parallel processing
- **Configurable**: Adjust mutation probabilities, number of sequences, and parallel workers
- **Python FFI**: Call from Python via PyO3 bindings
- **Targeted Mutations**: Optionally restrict mutations to specific instruction indices

## Quick Start

### Automated Setup

```bash
# Make the setup script executable
chmod +x setup.sh

# Run the setup script (builds Rust binary, Python module, and runs tests)
./setup.sh
```

The setup script will:
- ✓ Check for required dependencies (Rust, Python)
- ✓ Install maturin if needed
- ✓ Build the Rust binary
- ✓ Run all tests
- ✓ Build and install the Python module
- ✓ Generate example data
- ✓ Verify Python module installation

### Manual Setup

#### Rust Binary
```bash
cargo build --release
```

#### Python Module
```bash
# Install maturin (build tool for PyO3)
pip install maturin

# Build and install the Python module
maturin develop --release

# Or build a wheel
maturin build --release
```

## Usage

### Command Line (Rust)
```bash
./target/release/riscv_mutator \
    --input input.bin \
    --output-dir ./output \
    --num-mutations 1000 \
    --mutations-per-sequence 5 \
    --workers 8
```

### Python API

```python
import riscv_mutator

# Mutate all instructions
results = riscv_mutator.generate_mutations(
    input_path="input.bin",
    output_dir="./output",
    num_mutations=1000,
    mutations_per_sequence=5,
    seed=42,
    num_workers=8
)

# Mutate only specific instructions (e.g., bug-related instructions)
interesting_indices = [0, 5, 10, 15]
results = riscv_mutator.generate_mutations(
    input_path="input.bin",
    output_dir="./output",
    num_mutations=1000,
    mutations_per_sequence=5,
    interesting_instructions=interesting_indices,
    seed=42,
    num_workers=8
)

print(f"Generated {len(results)} mutated programs")
```

### As a Rust Library
```rust
use riscv_mutator::{instruction::Program, mutator::Mutator};

let program = Program::from_file("input.bin")?;

// Mutate all instructions
let mutator = Mutator::new();
let mut rng = rand::thread_rng();
let mutated = mutator.mutate(&program, 5, &mut rng);
mutated.to_file("output.bin")?;

// Mutate only specific instructions
let mutator = Mutator::with_interesting_instructions(vec![0, 5, 10]);
let mutated = mutator.mutate(&program, 5, &mut rng);
```

## Running Examples

### Generate a sample RISC-V program
```bash
cargo run --example generate_sample
```

### Run basic mutation example
```bash
cargo run --example basic_usage
```

### Run Python example
```bash
python python_example.py
```

## Running Tests

```bash
cargo test
```

## Architecture

- `instruction.rs`: Core instruction and program types
- `mutator.rs`: Mutation strategies and application logic
- `parallel.rs`: Parallel generation and file I/O
- `main.rs`: CLI interface and orchestration
- `lib.rs`: Python FFI interface

## Mutation Types

1. **ChangeOpcode**: Randomly modify instruction opcode
2. **ChangeRd/Rs1/Rs2**: Modify destination or source registers
3. **ChangeFunct3/Funct7**: Modify function fields
4. **ChangeImmediate**: Modify immediate values
5. **SwapInstructions**: Swap two instructions in the sequence
6. **SwapRegistersInInstruction**: Swap registers within an instruction
7. **DeleteInstruction**: Remove an instruction
8. **InsertInstruction**: Insert a random instruction
9. **DuplicateInstruction**: Duplicate an existing instruction
10. **RenameRegisterChain**: Rename a register throughout its def-use chain

Weights can be adjusted in `Mutator::new()`.

## Python Integration

The mutator can be called from Python, which is useful for integration with existing Python-based testing frameworks. The `interesting_instructions` parameter allows you to focus mutations on specific instructions that are relevant to a bug or test case.
