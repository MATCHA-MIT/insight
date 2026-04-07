use crate::instruction::Program;
use crate::mutator::Mutator;
use anyhow::Result;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub fn generate_mutations_parallel(
    program: Program,
    num_mutations: usize,
    mutations_per_sequence: usize,
    output_dir: &PathBuf,
    seed: Option<u64>,
    num_workers: usize,
) -> Result<Vec<PathBuf>> {
    generate_mutations_parallel_with_interesting(
        program,
        num_mutations,
        mutations_per_sequence,
        output_dir,
        seed,
        num_workers,
        None,
        None,
    )
}

pub fn generate_mutations_parallel_with_interesting(
    program: Program,
    num_mutations_per_sequence: usize,
    num_sequences: usize,
    output_dir: &PathBuf,
    seed: Option<u64>,
    num_workers: usize,
    interesting_instructions: Option<Vec<usize>>,
    max_program_length: Option<usize>,
) -> Result<Vec<PathBuf>> {
    // Configure rayon thread pool (0 means auto-detect)
    if num_workers > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_workers)
            .build_global()
            .ok();
    }

    let counter = AtomicUsize::new(0);
    let base_seed = seed.unwrap_or_else(|| rand::random());

    // Generate a list of (sequence_length, task_id) tuples
    // For each sequence length from 1 to num_sequences, generate num_mutations_per_sequence mutations
    let tasks: Vec<(usize, usize)> = (1..=num_sequences)
        .flat_map(|sequence_length| {
            (0..(num_mutations_per_sequence / sequence_length)).map(move |task_id| (sequence_length, task_id))
        })
        .collect();
    
    println!("Mutating sequence with interesting instructions {:?} and max length {:?}", &interesting_instructions, max_program_length);
    // Generate mutations in parallel
    let results: Vec<Result<PathBuf>> = tasks
        .into_par_iter()
        .map(|(sequence_length, task_offset)| {
            // Create a unique RNG for each task
            let task_seed = base_seed
                .wrapping_add((sequence_length as u64) * 100000)
                .wrapping_add(task_offset as u64);
            let mut rng = StdRng::seed_from_u64(task_seed);
            
            let mutator = if let Some(ref interesting) = interesting_instructions {
                Mutator::with_interesting_instructions(interesting.clone())
            } else {
                Mutator::new()
            };
            let mutator = if let Some(max_len) = max_program_length {
                mutator.with_max_length(Some(max_len))
            } else {
                mutator
            };
            
            let (mutated_program, applied_mutations) = mutator.mutate_with_tracking(&program, sequence_length, &mut rng);

            let file_idx = counter.fetch_add(1, Ordering::SeqCst);

            // If we have a CI-type mutation but it's actually a CSR instruction, adjust the mutation string
            let adjusted_mutations: Vec<_> = applied_mutations
                .iter()
                .map(|(mutation_type, instr_idx)| {
                    // // Check if this is a CI mutation on a CSR instruction
                    // if let Some(instr_idx_inner) = instr_idx {
                    //     if mutation_type.short_name() == "ci" || mutation_type.short_name() == "cai" {
                    //         if let Some(instr) = mutated_program.instructions.get(*instr_idx_inner) {
                    //             let opcode = instr.opcode();
                    //             if opcode == 0b1110011 { // CSR opcode
                    //                 return (mutation_type.clone(), *instr_idx, "csr");
                    //             }
                    //         }
                    //     }
                    // }
                    (mutation_type.clone(), *instr_idx, mutation_type.short_name())
                })
                .collect();
            
            // Build mutation string from applied mutations
            let mutation_str = adjusted_mutations
                .iter()
                .map(|m| {
                    let formatted_idx = match m.1 {
                        Some(idx) => idx.to_string(),
                        None => "none".to_string(), // 'g' for global mutations
                    };
                    format!("{}_{}_", m.2, formatted_idx)
                })
                .collect::<Vec<_>>()
                .join("_");
            
            let output_path = if mutation_str.is_empty() {
                output_dir.join(format!("mutated_seq{}_idx{}.bin", sequence_length, file_idx))
            } else {
                output_dir.join(format!("mutated_{}_{}.bin", mutation_str, file_idx))
            };

            mutated_program.to_file(&output_path)?;
            Ok(output_path)
        })
        .collect();

    let nop_indices: Vec<usize> = match &interesting_instructions {
        Some(indices) => {
            let mut unique: Vec<_> = indices
                .iter()
                .copied()
                .filter(|&idx| idx < program.len())
                .collect();
            unique.sort_unstable();
            unique.dedup();
            unique
        }
        None => (0..program.len()).collect(),
    };

    let nop_results: Vec<Result<PathBuf>> = nop_indices
        .into_par_iter()
        .map(|i| {
            let mut mutated_program = program.clone();
            Mutator::new().mutate_nop_instruction(&mut mutated_program, i);

            let output_path = output_dir.join(format!("mutated_nop_{}.bin", i));
            mutated_program.to_file(&output_path)?;
            Ok(output_path)
        })
        .collect();

    let results = results.into_iter().chain(nop_results);
    // Collect successful results
    results.into_iter().collect()
}
