use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    // This example demonstrates basic usage of the mutator library
    
    let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let input_file = example_dir.join("sample_program.bin");
    let output_dir = example_dir.join("output");
    
    println!("Running RISC-V Instruction Mutator Example");
    println!("===========================================\n");
    
    // Check if sample input exists
    if !input_file.exists() {
        eprintln!("Error: Sample program not found at {:?}", input_file);
        eprintln!("Please run: cargo run --example generate_sample first");
        std::process::exit(1);
    }
    
    // Create output directory
    std::fs::create_dir_all(&output_dir)?;
    
    // Load the program
    let program = riscv_mutator::instruction::Program::from_file(&input_file)?;
    println!("Loaded program with {} instructions", program.instructions.len());
    
    // Create mutator with default settings
    let mutator = riscv_mutator::mutator::Mutator::new();
    
    // Generate some mutations
    let num_sequences = 10;
    let mutations_per_sequence = 3;
    
    println!("Generating {} mutation sequences with {} mutations each\n", 
             num_sequences, mutations_per_sequence);
    
    let mut rng = rand::thread_rng();
    
    for i in 0..num_sequences {
        let mutated = mutator.mutate(&program, mutations_per_sequence, &mut rng);
        let output_path = output_dir.join(format!("mutated_{}.bin", i));
        mutated.to_file(&output_path)?;
        println!("  Generated: {:?}", output_path);
    }
    
    println!("\n✓ Successfully generated {} mutated programs", num_sequences);
    println!("  Output directory: {:?}", output_dir);
    
    Ok(())
}
