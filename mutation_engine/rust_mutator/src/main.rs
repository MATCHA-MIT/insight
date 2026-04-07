use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod instruction;
mod mutator;
mod parallel;
mod instruction_information;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input binary file containing RISC-V instructions
    #[arg(short, long)]
    input: PathBuf,

    /// Output directory for mutated programs
    #[arg(short, long)]
    output_dir: PathBuf,

    /// Number of mutation sequences to generate
    #[arg(short, long, default_value_t = 100)]
    num_mutations: usize,

    /// Number of mutations per sequence
    #[arg(short, long, default_value_t = 3)]
    mutations_per_sequence: usize,

    /// Random seed (optional)
    #[arg(short, long)]
    seed: Option<u64>,

    /// Number of parallel workers
    #[arg(short = 'j', long, default_value_t = 0)]
    workers: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Create output directory if it doesn't exist
    std::fs::create_dir_all(&args.output_dir)?;

    // Load program
    let program = instruction::Program::from_file(&args.input)?;
    
    println!("Loaded program with {} instructions", program.instructions.len());
    println!("Generating {} mutation sequences with {} mutations each", 
             args.num_mutations, args.mutations_per_sequence);

    // Run parallel mutation
    let results = parallel::generate_mutations_parallel(
        program,
        args.num_mutations,
        args.mutations_per_sequence,
        &args.output_dir,
        args.seed,
        args.workers,
    )?;

    println!("Successfully generated {} mutated programs", results.len());

    Ok(())
}
