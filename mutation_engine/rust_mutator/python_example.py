"""
Example of using the RISC-V mutator from Python
"""

import riscv_mutator

def main():
    # Example 1: Mutate all instructions
    results = riscv_mutator.generate_mutations(
        input_path="examples/sample_program.bin",
        output_dir="examples/python_output",
        num_mutations=50,
        mutations_per_sequence=3,
        seed=42,
        num_workers=4
    )
    
    print(f"Generated {len(results)} mutations (all instructions)")
    print(f"First few outputs: {results[:3]}")
    print("All output files:")
    for path in results:
        print(path)
    
    # Example 2: Only mutate specific instructions (e.g., indices 0, 2, 5)
    interesting = [0, 2, 5]
    results_restricted = riscv_mutator.generate_mutations(
        input_path="examples/sample_program.bin",
        output_dir="examples/python_output_restricted",
        num_mutations=50,
        mutations_per_sequence=3,
        interesting_instructions=interesting,
        seed=42,
        num_workers=4
    )
    
    print(f"\nGenerated {len(results_restricted)} mutations (only instructions {interesting})")
    print(f"First few outputs: {results_restricted[:3]}")
    print("All output files (restricted):")
    for path in results_restricted:
        print(path)

if __name__ == "__main__":
    main()
