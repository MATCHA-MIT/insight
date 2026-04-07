pub mod instruction;
pub mod mutator;
pub mod parallel;
pub mod instruction_information;

use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use std::path::PathBuf;

/// Generate RISC-V instruction mutations from Python
/// 
/// # Arguments
/// * `input_path` - Path to input binary file containing RISC-V instructions
/// * `output_dir` - Directory where mutated programs will be saved
/// * `num_mutations` - Number of mutation sequences to generate
/// * `mutations_per_sequence` - Number of mutations to apply per sequence
/// * `interesting_instructions` - Optional list of instruction indices to mutate (None = mutate all)
/// * `seed` - Optional random seed for reproducibility
/// * `num_workers` - Number of parallel workers (0 = auto-detect)
/// * `max_program_length` - Optional maximum program length in instructions (None = no limit)
///
/// # Returns
/// List of paths to generated mutated programs
#[pyfunction]
#[pyo3(signature = (input_path, output_dir, num_mutations, mutations_per_sequence, interesting_instructions=None, seed=None, num_workers=0, max_program_length=None))]
fn generate_mutations(
    input_path: String,
    output_dir: String,
    num_mutations: usize,
    mutations_per_sequence: usize,
    interesting_instructions: Option<Vec<usize>>,
    seed: Option<u64>,
    num_workers: usize,
    max_program_length: Option<usize>,
) -> PyResult<Vec<String>> {
    // Load program
    println!("Will do {} mutations with at most {} mutations per sequence to file {} max length {:?}", num_mutations, mutations_per_sequence, input_path, max_program_length);
    let program = instruction::Program::from_file(&PathBuf::from(&input_path))
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to load program: {}", e)))?;

    // Create output directory
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create output directory: {}", e)))?;

    // Generate mutations
    let results = parallel::generate_mutations_parallel_with_interesting(
        program,
        num_mutations,
        mutations_per_sequence,
        &PathBuf::from(&output_dir),
        seed,
        num_workers,
        interesting_instructions,
        max_program_length,
    )
    .map_err(|e| PyRuntimeError::new_err(format!("Mutation failed: {}", e)))?;

    // Convert PathBuf to String
    Ok(results
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// Python module for RISC-V instruction mutation
#[pymodule]
fn riscv_mutator(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(generate_mutations, m)?)?;
    Ok(())
}
