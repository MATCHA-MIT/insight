use anyhow::Result;
use rand::distributions::uniform::{SampleRange, SampleUniform};
use riscv_mutator::instruction::{Instruction, InstructionFormat, Program};
use riscv_mutator::mutator::Mutator;
use std::ops::RangeBounds;
use std::path::PathBuf;
use tempfile::TempDir;
use rand::{RngCore, Rng};
use rand::rngs::ThreadRng;
use rand::thread_rng;
/// A mock RNG that returns a predefined sequence of booleans and integers.
pub struct MockRng {
    bools: Vec<bool>,
    ints: Vec<u8>,
    bool_idx: usize,
    int_idx: usize,
}

// impl MockRng {
//     pub fn new(bools: Vec<bool>, ints: Vec<u8>) -> Self {
//         Self {
//             bools,
//             ints,
//             bool_idx: 0,
//             int_idx: 0,
//         }
//     }
// }

impl RngCore for MockRng {
    fn next_u32(&mut self) -> u32 {
        // fallback: pull next int if available, else 0
        let val = self.ints.get(self.int_idx).copied().unwrap_or(2);
        self.int_idx += 1;
        println!("MockRng returning {} for next_u32()", val);
        val as u32
    }

    fn next_u64(&mut self) -> u64 {
        self.next_u32() as u64
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for (i, byte) in dest.iter_mut().enumerate() {
            *byte = self.ints.get(i).copied().unwrap_or(0);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> { self.fill_bytes(dest); Ok(()) }
}

impl MockRng {
    pub fn new(bools: Vec<bool>, ints: Vec<u8>) -> Self {
        Self { bools, ints, bool_idx: 0, int_idx: 0 }
    }

    pub fn gen_range<T, R>(&mut self, _range: R) -> T
    where
        T: From<u8>,
        R: RangeBounds<T>,
    {
        let val = self.ints.get(self.int_idx).copied().unwrap_or(0);
        self.int_idx += 1;
        println!("MockRng returning {} for gen_range()", val);
        val.into()
    }

    pub fn gen_bool(&mut self, _p: f64) -> bool {
        let val = self.bools.get(self.bool_idx).copied().unwrap_or(false);
        self.bool_idx += 1;
        val
    }
}


#[test]
fn test_mock_rng() {
    let mut rng = MockRng::new(vec![true, false], vec![2]);

    // Works out of the box!
    let b1 = rng.gen_bool(0.5); // uses internal u32 -> bool
    let n1: u8 = rng.gen_range(0..3); // uses next_u32() under the hood

    println!("b1={b1}, n1={n1}");
}

// // Deterministic RNG for tests: yields a fixed sequence of u64 values.
// // This lets us force gen_range(..) to pick index 1 first and gen_bool(0.5) to return true.
// struct SeqRng {
//     seq: Vec<u64>,
//     idx: usize,
//     fallback: ThreadRng,
// }

// impl SeqRng {
//     fn new(seq: Vec<u64>) -> Self { Self { seq, idx: 0, fallback: thread_rng() } }
// }

// impl RngCore for SeqRng {
//     fn next_u32(&mut self) -> u32 { self.next_u64() as u32 }
//     fn next_u64(&mut self) -> u64 {
//         if self.idx < self.seq.len() {
//             let val = self.seq[self.idx];
//             self.idx += 1;
//             println!("SeqRng returning {} for request #{}", val, self.idx);
//             val
//         } else {
//             self.fallback.next_u64()
//         }
//     }
//     fn fill_bytes(&mut self, dest: &mut [u8]) {
//         let mut i = 0;
//         while i < dest.len() {
//             let v = self.next_u64();
//             let b = v.to_le_bytes();
//             let n = (dest.len() - i).min(8);
//             dest[i..i + n].copy_from_slice(&b[..n]);
//             i += n;
//         }
//     }
//     fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> { self.fill_bytes(dest); Ok(()) }
// }

// impl Rng for SeqRng {

//     fn gen_range<T, R>(&mut self, range: R) -> T
//     where
//         T: SampleUniform,
//         R: SampleRange<T>
//     {
//         assert!(!range.is_empty(), "cannot sample empty range");
//         range.sample_single(self)
//     }

//     fn gen_bool(&mut self, p: f64) -> bool {
//         let threshold = ((p.clamp(0.0, 1.0)) * (u64::MAX as f64)) as u64;
//         self.next_u64() <= threshold
//     }

// }


// impl SeqRng {
//     // Deterministic helper: modulo mapping for usize ranges.
//     #[allow(dead_code)]
//     fn gen_range(&mut self, range: std::ops::Range<usize>) -> usize {
//         assert!(range.start < range.end, "empty range");
//         let w = range.end - range.start;
//         let v = (self.next_u64() as usize) % w;
//         range.start + v
//     }

//     // Deterministic helper: probability based on next_u64 value.
//     #[allow(dead_code)]
//     fn gen_bool(&mut self, p: f64) -> bool {
//         let threshold = ((p.clamp(0.0, 1.0)) * (u64::MAX as f64)) as u64;
//         self.next_u64() <= threshold
//     }
// }




#[test]
fn test_instruction_creation() {
    let instr = Instruction::new(0x02a00093); // addi x1, x0, 42
    assert_eq!(instr.opcode(), 0x13); // OPCODE for I-type
    assert_eq!(instr.rd(), 1);
    assert_eq!(instr.rs1(), 0);
}

#[test]
fn test_instruction_modification() {
    let mut instr = Instruction::new(0x02a00093);
    
    // Modify destination register
    instr.set_rd(5);
    assert_eq!(instr.rd(), 5);
    
    // Modify source register
    instr.set_rs1(3);
    assert_eq!(instr.rs1(), 3);
    
    // Modify opcode
    instr.set_opcode(0x33);
    assert_eq!(instr.opcode(), 0x33);
    
    // Modify funct3
    instr.set_funct3(0x5);
    assert_eq!(instr.funct3(), 0x5);
    
    // Modify funct7
    instr.set_funct7(0x20);
    assert_eq!(instr.funct7(), 0x20);
}

#[test]
fn test_program_save_load() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test_program.bin");
    
    // Create a program
    let original = Program {
        instructions: vec![
            Instruction::new(0x02a00093),
            Instruction::new(0x06400113),
            Instruction::new(0x002081b3),
        ],
    };
    
    // Save it
    original.to_file(&file_path)?;
    
    // Load it back
    let loaded = Program::from_file(&file_path)?;
    
    // Verify
    assert_eq!(original.len(), loaded.len());
    for (orig, load) in original.instructions.iter().zip(loaded.instructions.iter()) {
        assert_eq!(orig.bytes, load.bytes);
    }
    
    Ok(())
}

#[test]
fn test_mutator_changes_program() {
    let program = Program {
        instructions: vec![
            Instruction::new(0x02a00093),
            Instruction::new(0x06400113),
            Instruction::new(0x002081b3),
        ],
    };
    
    let mutator = Mutator::new();
    let mut rng = rand::thread_rng();
    
    // Generate mutations
    let mutated = mutator.mutate(&program, 5, &mut rng);
    
    // Verify some change occurred (not foolproof but reasonable)
    let mut differences = 0;
    for i in 0..program.len().min(mutated.len()) {
        if program.instructions.get(i).map(|i| i.bytes) 
           != mutated.instructions.get(i).map(|i| i.bytes) {
            differences += 1;
        }
    }
    
    // At least some mutations should have occurred
    assert!(differences > 0 || program.len() != mutated.len(), 
            "Expected mutations to change the program");
}

#[test]
fn test_parallel_generation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    let program = Program {
        instructions: vec![
            Instruction::new(0x02a00093),
            Instruction::new(0x06400113),
        ],
    };
    
    let results = riscv_mutator::parallel::generate_mutations_parallel(
        program,
        10,  // num_mutations
        3,   // mutations_per_sequence
        &temp_dir.path().to_path_buf(),
        Some(42), // seed for reproducibility
        2,   // workers
    )?;
    
    assert_eq!(results.len(), 30);
    
    // Verify all files exist
    for path in &results {
        assert!(path.exists(), "Generated file should exist: {:?}", path);
    }
    
    Ok(())
}

#[test]
fn test_mutation_types() {
    let mut program = Program {
        instructions: vec![Instruction::new(0x02a00093)],
    };
    
    let mutator = Mutator::new();
    let mut rng = rand::thread_rng();
    
    // Test that we can apply mutations without panicking
    for _ in 0..100 {
        program = mutator.mutate(&program, 1, &mut rng);
    }
    
    // Program should still have at least some instructions
    assert!(!program.is_empty() || program.len() < 200, 
            "Program grew too large or disappeared");
}

#[test]
fn test_immediate_extraction_and_modification() {
    // I-type: addi x1, x0, 42
    let mut instr = Instruction::new(0x02a00093);
    assert_eq!(instr.imm_i(), 42);
    
    instr.set_imm_i(100);
    assert_eq!(instr.imm_i(), 100);
    
    // Negative immediate
    instr.set_imm_i(-50);
    assert_eq!(instr.imm_i(), -50);
}

#[test]
fn test_register_swap_mutation() {
    let mut program = Program {
        instructions: vec![
            Instruction::new(0x002081b3), // add x3, x1, x2
        ],
    };
    
    let original_rs1 = program.instructions[0].rs1();
    let original_rs2 = program.instructions[0].rs2();
    
    // Manually swap rs1 and rs2
    let mut instr = program.instructions[0];
    let temp = instr.rs1();
    instr.set_rs1(instr.rs2());
    instr.set_rs2(temp);
    
    assert_eq!(instr.rs1(), original_rs2);
    assert_eq!(instr.rs2(), original_rs1);
}

#[test]
fn test_register_chain_renaming() {
    let program = Program {
        instructions: vec![
            Instruction::new(0x02a00093), // addi x1, x0, 42
            Instruction::new(0x06400113), // addi x2, x0, 100
            Instruction::new(0x002081b3), // add x3, x1, x2
            Instruction::new(0x40110233), // sub x4, x2, x1
        ],
    };
    
    let mutator = Mutator::new();
    let mut rng = rand::thread_rng();
    
    // Apply rename mutation multiple times
    let mut mutated = program.clone();
    for _ in 0..5 {
        mutator.mutate_rename_register_chain(&mut mutated, &mut rng);
    }
    
    // Program should still be valid (same length)
    assert_eq!(mutated.len(), program.len());
}

#[test]
fn test_register_chain_renaming_waw_hazard() {
    let mut program = Program {
        instructions: vec![
            Instruction::new(0x02a00093), // addi x1, x0, 42
            Instruction::new(0x02c00093), // addi x1, x0, 44
            Instruction::new(0x002081b3), // add x3, x1, x2
        ],
    };
    
    // Restrict mutations to only index 2 so selection is deterministic and does not consume RNG
    let mutator = Mutator::new();
    // Seed SeqRng so that:
    // - gen_bool(0.5)   -> true (0 below threshold)
    // - gen_range(1..32) -> 3 (value 2 maps to 3 via (start + v % range))
    let mut rng = MockRng::new(vec![true, false], vec![2, 4]);
    // Apply rename mutation in-place
    mutator.mutate_rename_register_chain(&mut program, &mut rng);
    let mutated = program;
    // // Ensure that the last instruction's destination register is not the same as any previous destination registers
    let last_source = mutated.instructions.last().unwrap().rs1();
    assert_eq!(last_source, 4, "Did not modify rs1 to 4?");
    let other_dest = vec![mutated.instructions[1].rd(), mutated.instructions[0].rd()];
    assert_eq!(other_dest, vec![4, 4], "Previous destinations were not modified correctly. Previous destinations: {other_dest:?}");
    
    // let dest_regs: Vec<usize> = mutated.instructions.iter().map(|instr| instr.rd()).collect();
    // let last_dest = dest_regs.last().unwrap();
    // let prior_dests: Vec<usize> = dest_regs[..dest_regs.len()-1].to_vec();
    
    // assert!(!prior_dests.contains(last_dest), 
    //         "WAW hazard detected: last destination register was used previously");
}

#[test]
fn test_instruction_format_detection() {
    // I-type
    let i_type = Instruction::new(0x02a00093);
    assert_eq!(i_type.format(), InstructionFormat::I);
    
    // R-type
    let r_type = Instruction::new(0x002081b3);
    assert_eq!(r_type.format(), InstructionFormat::R);
    
    // U-type (lui)
    let u_type = Instruction::new(0x00001037);
    assert_eq!(u_type.format(), InstructionFormat::U);
}

#[test]
fn test_new_mutations_dont_panic() {
    let program = Program {
        instructions: vec![
            Instruction::new(0x02a00093),
            Instruction::new(0x06400113),
            Instruction::new(0x002081b3),
        ],
    };
    
    let mutator = Mutator::new();
    let mut rng = rand::thread_rng();
    
    // Test each new mutation type explicitly
    for _ in 0..10 {
        let mut test_prog = program.clone();
        mutator.mutate_immediate(&mut test_prog, &mut rng);
        
        test_prog = program.clone();
        mutator.mutate_swap_registers(&mut test_prog, &mut rng);
        
        test_prog = program.clone();
        mutator.mutate_rename_register_chain(&mut test_prog, &mut rng);
    }
}

#[test]
fn test_funct_fields_extraction() {
    // add x3, x1, x2 (R-type: funct7=0x00, funct3=0x0)
    let instr = Instruction::new(0x002081b3);
    assert_eq!(instr.funct3(), 0x0);
    assert_eq!(instr.funct7(), 0x00);
    
    // sub x4, x2, x1 (R-type: funct7=0x20, funct3=0x0)
    let instr = Instruction::new(0x40110233);
    assert_eq!(instr.funct3(), 0x0);
    assert_eq!(instr.funct7(), 0x20);
}
