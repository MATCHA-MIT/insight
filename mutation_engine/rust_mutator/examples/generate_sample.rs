use anyhow::Result;
use std::path::PathBuf;
use riscv_mutator::instruction::{Instruction, Program};

fn main() -> Result<()> {
    println!("Generating sample RISC-V program...\n");
    
    let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    std::fs::create_dir_all(&example_dir)?;
    
    // Create a simple RISC-V program with various instruction types
    let instructions = vec![
        // addi x1, x0, 42      # Load immediate 42 into x1
        Instruction::new(0x02a00093),
        // addi x2, x0, 100     # Load immediate 100 into x2
        Instruction::new(0x06400113),
        // add x3, x1, x2       # x3 = x1 + x2
        Instruction::new(0x002081b3),
        // sub x4, x2, x1       # x4 = x2 - x1
        Instruction::new(0x40110233),
        // and x5, x1, x2       # x5 = x1 & x2
        Instruction::new(0x0020f2b3),
        // or x6, x1, x2        # x6 = x1 | x2
        Instruction::new(0x0020e333),
        // xor x7, x1, x2       # x7 = x1 ^ x2
        Instruction::new(0x0020c3b3),
        // sll x8, x1, x2       # x8 = x1 << x2
        Instruction::new(0x00209433),
        // srl x9, x1, x2       # x9 = x1 >> x2 (logical)
        Instruction::new(0x0020d4b3),
        // sra x10, x1, x2      # x10 = x1 >> x2 (arithmetic)
        Instruction::new(0x4020d533),
    ];
    
    let program = Program { instructions };
    
    let output_path = example_dir.join("sample_program.bin");
    program.to_file(&output_path)?;
    
    println!("✓ Generated sample program with {} instructions", program.len());
    println!("  Saved to: {:?}\n", output_path);
    
    // Print instruction details
    println!("Instruction breakdown:");
    for (idx, instr) in program.instructions.iter().enumerate() {
        println!("  [{}] opcode=0x{:02x} rd={} rs1={} rs2={} (0x{:08x})",
                 idx,
                 instr.opcode(),
                 instr.rd(),
                 instr.rs1(),
                 instr.rs2(),
                 instr.bytes);
    }
    
    Ok(())
}
