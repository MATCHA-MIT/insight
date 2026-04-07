use crate::instruction::{Instruction, InstructionFormat, Program};
use rand::prelude::*;
use rand::distributions::WeightedIndex;
use std::{collections::HashSet, f32::consts::E};
use std::cell::RefCell;
use crate::instruction_information::{get_opcodes_for_format, get_valid_funct7_for_opcode};

#[derive(Debug, Clone, Copy)]
pub enum MutationType {
    ChangeOpcode,
    ChangeRd,
    ChangeRs1,
    ChangeRs2,
    ChangeFunct3,
    ChangeFunct7,
    SwapInstructions,
    DeleteInstruction,
    InsertInstruction,
    DuplicateInstruction,
    ChangeImmediate,
    SwapRegistersInInstruction,
    RenameRegisterChain,
    ReplaceInstruction,
    ChangeAllImmediates,
    SetRegisterToObservedImmediate,
    ChangeCSR,
}

impl MutationType {
    /// Get a short string identifier for filename use
    pub fn short_name(&self) -> &'static str {
        match self {
            MutationType::ChangeOpcode => "co",
            MutationType::ChangeRd => "crd",
            MutationType::ChangeRs1 => "crs1",
            MutationType::ChangeRs2 => "crs2",
            MutationType::ChangeFunct3 => "cf3",
            MutationType::ChangeFunct7 => "cf7",
            MutationType::SwapInstructions => "si",
            MutationType::DeleteInstruction => "di",
            MutationType::InsertInstruction => "ii",
            MutationType::DuplicateInstruction => "dupi",
            MutationType::ChangeImmediate => "ci",
            MutationType::SwapRegistersInInstruction => "sri",
            MutationType::RenameRegisterChain => "rrc",
            MutationType::ReplaceInstruction => "ri",
            MutationType::ChangeAllImmediates => "cai",
            MutationType::SetRegisterToObservedImmediate => "sroi",
            MutationType::ChangeCSR => "ccsr",
        }
    }
}

pub struct Mutator {
    mutation_weights: Vec<f64>,
    mutation_types: Vec<MutationType>,
    interesting_instructions: Option<RefCell<HashSet<usize>>>,
    max_program_length: Option<usize>,
}

impl Mutator {
    pub fn new() -> Self {
        let mutation_types = vec![
            MutationType::ChangeOpcode,
            MutationType::ChangeRd,
            MutationType::ChangeRs1,
            MutationType::ChangeRs2,
            MutationType::ChangeFunct3,
            MutationType::ChangeFunct7,
            MutationType::SwapInstructions,
            MutationType::DeleteInstruction,
            MutationType::InsertInstruction,
            MutationType::DuplicateInstruction,
            MutationType::ChangeImmediate,
            MutationType::SwapRegistersInInstruction,
            MutationType::RenameRegisterChain,
            MutationType::ReplaceInstruction,
            MutationType::ChangeAllImmediates,
            MutationType::SetRegisterToObservedImmediate,
            MutationType::ChangeCSR,
        ];

        // Default weights for each mutation type
        let mutation_weights = vec![
            0.07,  // ChangeOpcode
            0.11,  // ChangeRd
            0.11,  // ChangeRs1
            0.11,  // ChangeRs2
            0.09,  // ChangeFunct3
            0.07,  // ChangeFunct7
            0.06,  // SwapInstructions
            0.03,  // DeleteInstruction
            0.03,  // InsertInstruction
            0.03,  // DuplicateInstruction
            0.07,  // ChangeImmediate (reduced from 0.08)
            0.06,  // SwapRegistersInInstruction
            0.04,  // RenameRegisterChain
            0.08,  // ReplaceInstruction (reduced from 0.09)
            0.02,  // ChangeAllImmediates
            0.02,  // SetRegisterToObservedImmediate (new)
            0.02,  // ChangeCSR (new)
        ];

        Self {
            mutation_weights,
            mutation_types,
            interesting_instructions: None,
            max_program_length: None,
        }
    }

    /// Create a mutator that only mutates specific instruction indices
    pub fn with_interesting_instructions(interesting: Vec<usize>) -> Self {
        let mut mutator = Self::new();
        if !interesting.is_empty() {
            mutator.interesting_instructions = Some(RefCell::new(interesting.into_iter().collect()));
        }
        mutator
    }

    /// Set the maximum program length
    pub fn with_max_length(mut self, max_length: Option<usize>) -> Self {
        self.max_program_length = max_length;
        self
    }

    /// Check if we can add an instruction without exceeding max length
    fn can_insert(&self, current_length: usize) -> bool {
        match self.max_program_length {
            Some(max) => current_length < max,
            None => true,
        }
    }

    /// Get all instruction indices where a specific mutation type can be applied
    pub fn get_applicable_indices(&self, program: &Program, mutation: MutationType) -> Vec<usize> {
        if program.is_empty() {
            return Vec::new();
        }

        match mutation {
            MutationType::ChangeOpcode => {
                if self.program_is_csr_only(program) {
                    return Vec::new(); // Can't change opcode if all instructions are CSR (to avoid invalid non-CSR mutations)
                }
                self.get_indices_with_filter(program, |_| true)
            }
            MutationType::ChangeRd => {
                self.get_indices_with_filter(program, |instr| instr.has_rd())
            }
            MutationType::ChangeRs1 => {
                self.get_indices_with_filter(program, |instr| instr.has_rs1())
            }
            MutationType::ChangeRs2 => {
                self.get_indices_with_filter(program, |instr| instr.has_rs2())
            }
            MutationType::ChangeFunct3 => {
                self.get_indices_with_filter(program, |instr| instr.has_funct3())
            }
            MutationType::ChangeFunct7 => {
                self.get_indices_with_filter(program, |instr| instr.has_funct7())
            }
            MutationType::SwapInstructions => {
                if program.len() >= 2 {
                    self.get_indices_with_filter(program, |_| true)
                } else {
                    Vec::new()
                }
            }
            MutationType::DeleteInstruction => {
                if program.len() > 1 {
                    self.get_indices_with_filter(program, |_| true)
                } else {
                    Vec::new()
                }
            }
            MutationType::InsertInstruction => {
                // Can insert at any position if not at max length
                if self.can_insert(program.len()) {
                    (0..=program.len()).collect()
                } else {
                    Vec::new()
                }
            }
            MutationType::DuplicateInstruction => {
                if self.can_insert(program.len()) {
                    self.get_indices_with_filter(program, |_| true)
                } else {
                    Vec::new()
                }
            }
            MutationType::ChangeImmediate => {
                self.get_indices_with_filter(program, |instr| {
                    matches!(instr.format(), 
                        InstructionFormat::I | 
                        InstructionFormat::S | 
                        InstructionFormat::B | 
                        InstructionFormat::U | 
                        InstructionFormat::J) && instr.opcode() != 0x73 // Exclude CSR instructions
                })
            }
            MutationType::SwapRegistersInInstruction => {
                self.get_indices_with_filter(program, |instr| {
                    (instr.has_rs1() && instr.has_rs2()) ||
                    (instr.has_rd() && instr.has_rs1()) ||
                    (instr.has_rd() && instr.has_rs2())
                })
            }
            MutationType::RenameRegisterChain => {
                if program.len() >= 2 {
                    vec![0] // Returns a dummy index to indicate it's applicable
                } else {
                    Vec::new()
                }
            }
            MutationType::ReplaceInstruction => {
                if self.program_is_csr_only(program) {
                    return Vec::new(); // Avoid replacing instructions in a CSR-only program to prevent
                    //flood of valid non-csr instructions
                }
                self.get_indices_with_filter(program, |_| true)
            }
            MutationType::ChangeAllImmediates => {
                // Check if there's at least one instruction with an immediate
                let has_immediate = program.instructions.iter().any(|instr| {
                    matches!(instr.format(), 
                        InstructionFormat::I | 
                        InstructionFormat::S | 
                        InstructionFormat::B | 
                        InstructionFormat::U | 
                        InstructionFormat::J)
                        // && instr.opcode() != 0x73 // Exclude CSR instructions
                });
                if has_immediate {
                    vec![0] // Returns a dummy index to indicate it's applicable
                } else {
                    Vec::new()
                }
            }
            MutationType::SetRegisterToObservedImmediate => {
                if self.program_is_csr_only(program) || !self.can_insert(program.len()) {
                    Vec::new()
                } else {
                    let immediates = self.collect_observed_immediates(program);
                    if !immediates.is_empty() {
                        vec![0] // Returns a dummy index to indicate it's applicable
                    } else {
                        Vec::new()
                    }
                }
            }
            MutationType::ChangeCSR => {
                self.get_indices_with_filter(program, |instr| instr.opcode() == 0x73)
            }
        }
    }

    /// Helper to get indices matching a filter, respecting interesting_instructions
    fn get_indices_with_filter<F>(&self, program: &Program, filter: F) -> Vec<usize>
    where
        F: Fn(&Instruction) -> bool,
    {
        match &self.interesting_instructions {
            Some(set) => {
                set.borrow().iter()
                    .filter(|&&idx| idx < program.len() && filter(&program.instructions[idx]))
                    .copied()
                    .collect()
            }
            None => {
                (0..program.len())
                    .filter(|&idx| filter(&program.instructions[idx]))
                    .collect()
            }
        }
    }

    /// Select a mutation type that can be applied to the current program
    /// Returns None if no mutations are applicable
    pub fn select_applicable_mutation(&self, program: &Program, rng: &mut impl Rng) -> Option<MutationType> {
        // Filter mutation types to only those that can be applied
        let applicable: Vec<(MutationType, f64)> = self.mutation_types.iter()
            .zip(self.mutation_weights.iter())
            .filter(|(mutation_type, _)| {
                !self.get_applicable_indices(program, **mutation_type).is_empty()
            })
            .map(|(mt, w)| (*mt, *w))
            .collect();

        if applicable.is_empty() {
            return None;
        }
        println!("Applicable mutations: {:?}", applicable.iter().map(|(m,_)| m).collect::<Vec<_>>());
        // Create weighted distribution from applicable mutations
        let weights: Vec<f64> = applicable.iter().map(|(_, w)| *w).collect();
        println!("Weights: {:?}", weights);
        let dist = WeightedIndex::new(&weights).unwrap();
        let selected_idx = dist.sample(rng);
        let returned_mutation = applicable[selected_idx].0;
        print!("Selected mutation: {:?}\n", returned_mutation);
        Some(returned_mutation)
    }

    pub fn mutate(&self, program: &Program, num_mutations: usize, rng: &mut impl Rng) -> Program {
        self.mutate_with_tracking(program, num_mutations, rng).0
    }



    /// Mutate a program and return both the mutated program and the list of mutations applied
    pub fn mutate_with_tracking(&self, program: &Program, num_mutations: usize, rng: &mut impl Rng) -> (Program, Vec<(MutationType, Option<usize>)>) {
        let mut mutated = program.clone();
        let mut applied_mutations = Vec::new();

        if mutated.is_empty() {
            return (mutated, applied_mutations);
        }

        for _ in 0..num_mutations {
            // Select only mutations that can be applied
            if let Some(mutation_type) = self.select_applicable_mutation(&mutated, rng) {
                let idx = self.apply_mutation(&mut mutated, mutation_type, rng);
                applied_mutations.push((mutation_type, idx));
            }
            // If no applicable mutation, skip this iteration
        }
        println!("Applied mutations: {:?}", applied_mutations);
        (mutated, applied_mutations)
    }

    /// Check whether the program already contains loads/stores or CSR instructions
    fn program_allows_loads_stores(&self, program: &Program) -> bool {
        for instr in &program.instructions {
            // Use opcode() accessor if available; fall back to raw mask if not.
            let opc = instr.opcode();
            if opc == 0x03 || opc == 0x23 {
                return true;
            }
        }
        false
    }

    fn program_allows_csr(&self, program: &Program) -> bool {
        for instr in &program.instructions {
            let opc = instr.opcode();
            if opc == 0x73 {
                return true;
            }
        }
        false
    }

    /// Check whether the program is CSR-only (every instruction has opcode 0x73)
    fn program_is_csr_only(&self, program: &Program) -> bool {
        if program.is_empty() {
            return false;
        }
        if program.len() == 1 {
            return program.instructions[0].opcode() == 0x73;
        }
        let csr_only = program.instructions.iter().all(|instr| instr.opcode() == 0x73 || instr.is_nop());
        // for instr in &program.instructions {
        //     if instr.opcode() == 0x73 {
        //         return true;
        //     }
        // }
        return csr_only;
    }

    /// Generate a random but semantically valid RISC-V instruction.
    fn generate_valid_instruction(&self, program: &Program, rng: &mut impl Rng) -> Instruction {
        // If program is CSR-only, don't emit other instruction kinds (avoid jalr, addi, etc.)
        // if self.program_is_csr_only(program) {
        //     // generate either a CSR-like instruction or a fence (as safe CSR-friendly options)
        //     let choice = rng.gen_range(0..2);
        //     if choice == 0 {
        //         // simple CSR-like I-type: csr[11:0] | rs1 << 15 | funct3 << 12 | rd << 7 | opcode(0x73)
        //         let rd = rng.gen_range(0..32);
        //         let rs1 = rng.gen_range(0..32);
        //         let csr = rng.gen_range(0..4096) as u32 & 0xFFF;
        //         let instr = (csr << 20) | (rs1 as u32) << 15 | (0u32 << 12) | (rd as u32) << 7 | (0x73u32);
        //         return Instruction::new(instr);
        //     } else {
        //         // FENCE instruction as a safe non-arithmetic alternative
        //         let fences = [Instruction::new(0x0ff0000f), Instruction::new(0x0000100f)];
        //         return *fences.choose(rng).unwrap();
        //     }
        // }

        // Choose a valid opcode for RISC-V base ISA (RV32I)
        // Default set excludes loads/stores and CSR (0x03,0x23,0x73).
        // Only include them if the program already contains any such instruction.
        // let mut opcodes = vec![0x33u8, 0x13, 0x63, 0x6F, 0x67, 0x37, 0x17, 0x51];
        //No branch instructions
        let mut opcodes = vec![0x33u8, 0x13, 0x37, 0x17, 0x51];
        let allow_load_store = self.program_allows_loads_stores(program);
        let allow_csr = self.program_allows_csr(program);
        if allow_load_store {
            // add loads/stores and CSR if program already contains them
            opcodes.push(0x03);
            opcodes.push(0x23);
        } else {
            opcodes.push(0x73);
        }
        let opcode = if allow_csr && program.len() == 1 {
            0x73
        } else {
            *opcodes.choose(rng).unwrap()
        };

        // Generate fields according to the opcode type
        match opcode {
            0x33 => {
                // R-type: opcode | rd | funct3 | rs1 | rs2 | funct7
                let rd = rng.gen_range(1..32);
                let funct3 = rng.gen_range(0..8);
                let rs1 = rng.gen_range(0..32);
                let rs2 = rng.gen_range(0..32);
                let funct7 = rng.gen_range(0..128);
                let instr = (funct7 as u32) << 25
                    | (rs2 as u32) << 20
                    | (rs1 as u32) << 15
                    | (funct3 as u32) << 12
                    | (rd as u32) << 7
                    | (opcode as u32);
                Instruction::new(instr)
            }
            0x73 => {
                // CSR/system instructions - create a simple ecall-like CSR (keep minimal fields)
                // We'll emit an ECALL/CSR-friendly pattern as a fallback; choose a typical CSR instruction encoding
                // Use CSRRW/CSRRS-like I-type encoding with funct3 in CSR space; keep rd and rs1 randomized
                if !self.program_allows_loads_stores(program) && !allow_csr {
                    // If loads/stores/CSR not allowed, fallback to FENCE
                    let random_list = vec![Instruction::new(0x0ff0000f), Instruction::new(0x0000100f)]; // FENCE instruction
                    return *random_list.choose(rng).unwrap();
                }
                let rd = rng.gen_range(0..32);
                let rs1 = rng.gen_range(0..32);
                let csr = rng.gen_range(0..4096) as u32 & 0xFFF;
                // Construct as: csr[11:0] | rs1 << 15 | funct3 << 12 | rd << 7 | opcode
                let instr = (csr << 20) | (rs1 as u32) << 15 | (0u32 << 12) | (rd as u32) << 7 | (opcode as u32);
                Instruction::new(instr)
            }
            0x13 | 0x03 | 0x67 => {
                // I-type: opcode | rd | funct3 | rs1 | imm[11:0]
                let rd = rng.gen_range(1..32);
                let funct3 = rng.gen_range(0..8);
                let rs1 = rng.gen_range(0..32);
                let imm = rng.gen_range(-2048..2048) as u32 & 0xFFF;
                let instr = (imm << 20)
                    | (rs1 as u32) << 15
                    | (funct3 as u32) << 12
                    | (rd as u32) << 7
                    | (opcode as u32);
                Instruction::new(instr)
            }
            0x23 => {
                // S-type: opcode | imm[11:5] | rs2 | rs1 | funct3 | imm[4:0]
                let funct3 = rng.gen_range(0..8);
                let rs1 = rng.gen_range(0..32);
                let rs2 = rng.gen_range(0..32);
                let imm = rng.gen_range(-2048..2048) as u32 & 0xFFF;
                let imm_11_5 = (imm >> 5) & 0x7F;
                let imm_4_0 = imm & 0x1F;
                let instr = (imm_11_5 << 25)
                    | (rs2 as u32) << 20
                    | (rs1 as u32) << 15
                    | (funct3 as u32) << 12
                    | (imm_4_0 << 7)
                    | (opcode as u32);
                Instruction::new(instr)
            }
            0x63 => {
                // B-type: opcode | imm[12|10:5] | rs2 | rs1 | funct3 | imm[4:1|11]
                let funct3 = rng.gen_range(0..8);
                let rs1 = rng.gen_range(0..32);
                let rs2 = rng.gen_range(0..32);
                let imm = rng.gen_range(-4096..4096) as u32 & 0x1FFF;
                let imm_12 = (imm >> 12) & 0x1;
                let imm_10_5 = (imm >> 5) & 0x3F;
                let imm_4_1 = (imm >> 1) & 0xF;
                let imm_11 = (imm >> 11) & 0x1;
                let instr = (imm_12 << 31)
                    | (imm_10_5 << 25)
                    | (rs2 as u32) << 20
                    | (rs1 as u32) << 15
                    | (funct3 as u32) << 12
                    | (imm_4_1 << 8)
                    | (imm_11 << 7)
                    | (opcode as u32);
                Instruction::new(instr)
            }
            0x6F => {
                // J-type: opcode | rd | imm[20|10:1|11|19:12]
                let rd = rng.gen_range(1..32);
                let imm = rng.gen_range(-1048576..1048576) as u32 & 0xFFFFF;
                let imm_20 = (imm >> 20) & 0x1;
                let imm_10_1 = (imm >> 1) & 0x3FF;
                let imm_11 = (imm >> 11) & 0x1;
                let imm_19_12 = (imm >> 12) & 0xFF;
                let instr = (imm_20 << 31)
                    | (imm_19_12 << 12)
                    | (imm_11 << 20)
                    | (imm_10_1 << 21)
                    | (rd as u32) << 7
                    | (opcode as u32);
                Instruction::new(instr)
            }
            0x37 | 0x17 => {
                // U-type: opcode | rd | imm[31:12]
                let rd = rng.gen_range(1..32);
                let imm = rng.gen_range(-524288..524288) as u32 & 0xFFFFF000;
                let instr = imm | (rd as u32) << 7 | (opcode as u32);
                Instruction::new(instr)
            }
            _ => {
                // Fallback: random R-type
                let rd = rng.gen_range(1..32);
                let funct3 = rng.gen_range(0..8);
                let rs1 = rng.gen_range(0..32);
                let rs2 = rng.gen_range(0..32);
                let funct7 = rng.gen_range(0..128);
                let instr = (funct7 as u32) << 25
                    | (rs2 as u32) << 20
                    | (rs1 as u32) << 15
                    | (funct3 as u32) << 12
                    | (rd as u32) << 7
                    | (0x33u32);
                Instruction::new(instr)
            }
        }
    }

    fn apply_mutation(&self, program: &mut Program, mutation: MutationType, rng: &mut impl Rng) -> Option<usize> {
        if program.is_empty() {
            return None;
        }
        // println!("Applying mutation: {:?}", mutation);

        match mutation {
            MutationType::ChangeOpcode => {
                let idx = self.select_mutable_index_with_filter(program, rng, |_| true);
               
                if let Some(idx) = idx {
                    let mut opcodes = get_opcodes_for_format(program.instructions[idx].format());
                    if !(self.program_allows_csr(program)) {
                        // If CSR not allowed, filter out CSR opcode
                        opcodes.retain(|&opc| opc != 0x73);
                    }
                    if !(self.program_allows_loads_stores(program)) {
                        // If loads/stores not allowed, filter out load/store opcodes
                        opcodes.retain(|&opc| opc != 0x03 && opc != 0x23);
                    }
                    
                    // If all opcodes were filtered out, skip this mutation
                    if opcodes.is_empty() {
                        println!("No valid opcodes available for mutation at index {:?} instruction {}, skipping ChangeOpcode", idx, program.instructions[idx]);
                        return None;
                    }
                    
                    let new_opcode = *opcodes.choose(rng).unwrap();
                    let mut new_opcode = new_opcode;
                    let forbidden = [0x03u8, 0x23u8, 0x73u8];
                    if !self.program_allows_loads_stores(program) {
                        // Re-roll until we pick a non-load/store/CSR opcode
                        while forbidden.contains(&new_opcode) {
                            new_opcode = rng.gen_range(0..128) as u8;
                        }
                    }
                    println!("Changing opcode at index {:?} to 0x{:02x}", idx, new_opcode);
                    program.instructions[idx].set_opcode(new_opcode);
                    return Some(idx);
                } else {
                    return None;
                }
            }
            MutationType::ChangeRd => {
                let idx = self.select_mutable_index_with_filter(program, rng, |instr| instr.has_rd());
                if let Some(idx) = idx {
                    let new_rd = rng.gen_range(0..32) as u8;
                    program.instructions[idx].set_rd(new_rd);
                    return Some(idx);
                } else {
                    return None;
                }
            }
            MutationType::ChangeRs1 => {
                let idx = self.select_mutable_index_with_filter(program, rng, |instr| instr.has_rs1());
                if let Some(idx) = idx {
                    let new_rs1 = rng.gen_range(0..32) as u8;
                    program.instructions[idx].set_rs1(new_rs1);
                    return Some(idx);
                } else {
                    return None;
                }
            }
            MutationType::ChangeRs2 => {
                let idx = self.select_mutable_index_with_filter(program, rng, |instr| instr.has_rs2());
                if let Some(idx) = idx {
                    let new_rs2 = rng.gen_range(0..32) as u8;
                    program.instructions[idx].set_rs2(new_rs2);
                    return Some(idx);
                } else {
                    return None;
                }
            }
            MutationType::ChangeFunct3 => {
                let idx = self.select_mutable_index_with_filter(program, rng, |instr| instr.has_funct3());
                if let Some(idx) = idx {
                    let new_funct3 = rng.gen_range(0..8) as u8;
                    program.instructions[idx].set_funct3(new_funct3);
                    return Some(idx);
                } else {
                    return None;
                }
            }
            MutationType::ChangeFunct7 => {
                let idx = self.select_mutable_index_with_filter(program, rng, |instr| instr.has_funct7());
                if let Some(idx) = idx {
                    let new_funct7 = rng.gen_range(0..128) as u8;
                    program.instructions[idx].set_funct7(new_funct7);
                    return Some(idx);
                } else {
                    return None;
                }
            }
            MutationType::SwapInstructions => {
                if program.len() >= 2 {
                    if let (Some(idx1), Some(idx2)) = (
                        self.select_mutable_index_with_filter(program, rng, |_| true),
                        self.select_mutable_index_with_filter(program, rng, |_| true),
                    ) {
                        program.instructions.swap(idx1, idx2);
                        return Some(idx1);
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            MutationType::DeleteInstruction => {
                if program.len() > 1 {
                    if let Some(deleted_idx) = self.select_mutable_index_with_filter(program, rng, |_| true) {
                        program.instructions.remove(deleted_idx);
                        
                        // Update interesting_instructions: shift indices after the deleted one
                        if let Some(ref set_cell) = self.interesting_instructions {
                            let mut set = set_cell.borrow_mut();
                            let mut new_set = HashSet::new();
                            
                            for &idx in set.iter() {
                                if idx < deleted_idx {
                                    // Indices before the deleted one stay the same
                                    new_set.insert(idx);
                                } else if idx > deleted_idx {
                                    // Indices after the deleted one shift down by 1
                                    new_set.insert(idx - 1);
                                }
                                // If idx == deleted_idx, it's removed (not added to new_set)
                            }
                            
                            *set = new_set;
                        }
                        return Some(deleted_idx);
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            MutationType::InsertInstruction => {
                // Only insert if we haven't reached max length
                // if !self.can_insert(program.len()) {
                //     return;
                // }
                // If program is CSR-only, do not insert non-CSR instructions (generate_valid_instruction already respects CSR-only)
                let insert_idx = self.select_insertion_index(program, rng);
                let random_instr = self.generate_valid_instruction(program, rng);
                program.instructions.insert(insert_idx, random_instr);
                
                // Update interesting_instructions: shift indices at or after the inserted one
                if let Some(ref set_cell) = self.interesting_instructions {
                    let mut set = set_cell.borrow_mut();
                    let mut new_set = HashSet::new();
                    
                    for &idx in set.iter() {
                        if idx < insert_idx {
                            // Indices before the insertion stay the same
                            new_set.insert(idx);
                        } else {
                            // Indices at or after the insertion shift up by 1
                            new_set.insert(idx + 1);
                        }
                    }
                    
                    *set = new_set;
                }
                return Some(insert_idx);
            }
            MutationType::DuplicateInstruction => {
                // Only duplicate if we haven't reached max length
                // if !self.can_insert(program.len()) {
                //     return;
                // }
                
                if let Some(idx) = self.select_mutable_index_with_filter(program, rng, |_| true) {
                    let instr = program.instructions[idx];
                    let insert_idx = rng.gen_range(0..=program.len());
                    program.instructions.insert(insert_idx, instr);
                    return Some(insert_idx);
                }  else {
                    return None;
                }
            }
            MutationType::ChangeImmediate => {
                return self.mutate_immediate(program, rng);
            }
            MutationType::SwapRegistersInInstruction => {
                let idx = self.select_mutable_index_with_filter(program, rng, |instr| {
                    // Need at least two register fields to swap
                    (instr.has_rs1() && instr.has_rs2()) ||
                    (instr.has_rd() && instr.has_rs1()) ||
                    (instr.has_rd() && instr.has_rs2())
                });
                if let Some(idx) = idx {
                    self.mutate_swap_registers_at(program, idx, rng);
                    return Some(idx);
                } else {
                    return None;
                }
            }
            MutationType::RenameRegisterChain => {
                self.mutate_rename_register_chain(program, rng);
                return Some(0);
            }
            MutationType::ReplaceInstruction => {
                let idx = self.select_mutable_index_with_filter(program, rng, |_| true);
                //println!("Replacing instruction at index {:?} with new instruction", idx);
                let chance_take_old = 0.5;
                if let Some(idx) = idx {
                    // Only apply this replacement if there are NO CSR instructions in the program
                    
                    {
                        let orig = program.instructions[idx];
                        // Start by picking a random opcode (avoid loads/stores/CSR because caller skipped CSR programs)
                        // let mut opcodes = vec![0x33u8, 0x13u8, 0x63u8, 0x6Fu8, 0x67u8, 0x37u8, 0x17u8];

                        // let opcode = *opcodes.choose(rng).unwrap();
                        let mut opcodes = get_opcodes_for_format(program.instructions[idx].format());
                        if !(self.program_allows_csr(program)) {
                             // If CSR exists, skip this replacement to avoid introducing non-compliant arithmetic
                             opcodes.retain(|&x| x != 0x73); // CSR opcode
                            // return;
                        }
                        let opcode = if opcodes.is_empty() {
                            0x33 // default to R-type if unknown
                        } else {
                            *opcodes.choose(rng).unwrap()
                        };
                    

                        let new_instr = match opcode {
                            0x33 => {
                                // R-type
                                let rd = if rng.gen_bool(chance_take_old) && orig.has_rd() { orig.rd() } else { rng.gen_range(1..32) as u8 };
                                let funct3 = if rng.gen_bool(chance_take_old) && orig.has_funct3() { orig.funct3() } else { rng.gen_range(0..8) as u8 };
                                let rs1 = if rng.gen_bool(chance_take_old) && orig.has_rs1() { orig.rs1() } else { rng.gen_range(0..32) as u8 };
                                let rs2 = if rng.gen_bool(chance_take_old) && orig.has_rs2() { orig.rs2() } else { rng.gen_range(0..32) as u8 };
                                let funct7 = if rng.gen_bool(chance_take_old) && orig.has_funct7() { orig.funct7() } else { get_valid_funct7_for_opcode(opcode, rng)};
                                let bits = (funct7 as u32) << 25
                                    | (rs2 as u32) << 20
                                    | (rs1 as u32) << 15
                                    | (funct3 as u32) << 12
                                    | (rd as u32) << 7
                                    | (0x33u32);
                                Instruction::new(bits)
                            }
                            0x13 | 0x03 | 0x67 => {
                                // I-type
                                let rd = if rng.gen_bool(chance_take_old) && orig.has_rd() { orig.rd() } else { rng.gen_range(1..32) as u8 };
                                let funct3 = if rng.gen_bool(chance_take_old) && orig.has_funct3() { orig.funct3() } else { rng.gen_range(0..8) as u8 };
                                let rs1 = if rng.gen_bool(chance_take_old) && orig.has_rs1() { orig.rs1() } else { rng.gen_range(0..32) as u8 };
                                let imm = if rng.gen_bool(chance_take_old) { orig.imm_i() } else { rng.gen_range(-2048..2048) };
                                let imm_u32 = (imm as u32) & 0xFFF;
                                let bits = (imm_u32 << 20)
                                    | (rs1 as u32) << 15
                                    | (funct3 as u32) << 12
                                    | (rd as u32) << 7
                                    | (opcode as u32);
                                Instruction::new(bits)
                            }
                            0x23 => {
                                // S-type
                                let funct3 = if rng.gen_bool(chance_take_old) && orig.has_funct3() { orig.funct3() } else { rng.gen_range(0..8) as u8 };
                                let rs1 = if rng.gen_bool(chance_take_old) && orig.has_rs1() { orig.rs1() } else { rng.gen_range(0..32) as u8 };
                                let rs2 = if rng.gen_bool(chance_take_old) && orig.has_rs2() { orig.rs2() } else { rng.gen_range(0..32) as u8 };
                                let imm = if rng.gen_bool(chance_take_old) { orig.imm_s() } else { rng.gen_range(-2048..2048) };
                                let imm_u = (imm as u32) & 0xFFF;
                                let imm_11_5 = (imm_u >> 5) & 0x7F;
                                let imm_4_0 = imm_u & 0x1F;
                                let bits = (imm_11_5 << 25)
                                    | (rs2 as u32) << 20
                                    | (rs1 as u32) << 15
                                    | (funct3 as u32) << 12
                                    | (imm_4_0 << 7)
                                    | (opcode as u32);
                                Instruction::new(bits)
                            }
                            0x63 => {
                                // B-type
                                let funct3 = if rng.gen_bool(chance_take_old) && orig.has_funct3() { orig.funct3() } else { rng.gen_range(0..8) as u8 };
                                let rs1 = if rng.gen_bool(chance_take_old) && orig.has_rs1() { orig.rs1() } else { rng.gen_range(0..32) as u8 };
                                let rs2 = if rng.gen_bool(chance_take_old) && orig.has_rs2() { orig.rs2() } else { rng.gen_range(0..32) as u8 };
                                let imm = if rng.gen_bool(chance_take_old) { orig.imm_b() } else { rng.gen_range(-4096..4096) };
                                let imm_u = (imm as u32) & 0x1FFF;
                                let imm_12 = (imm_u >> 12) & 0x1;
                                let imm_10_5 = (imm_u >> 5) & 0x3F;
                                let imm_4_1 = (imm_u >> 1) & 0xF;
                                let imm_11 = (imm_u >> 11) & 0x1;
                                let bits = (imm_12 << 31)
                                    | (imm_10_5 << 25)
                                    | (rs2 as u32) << 20
                                    | (rs1 as u32) << 15
                                    | (funct3 as u32) << 12
                                    | (imm_4_1 << 8)
                                    | (imm_11 << 7)
                                    | (opcode as u32);
                                Instruction::new(bits)
                            }
                            0x6F => {
                                // J-type
                                let rd = if rng.gen_bool(chance_take_old) && orig.has_rd() { orig.rd() } else { rng.gen_range(1..32) as u8 };
                                let imm = if rng.gen_bool(chance_take_old) { orig.imm_j() } else { rng.gen_range(-1048576..1048576) };
                                let imm_u = (imm as u32) & 0xFFFFF;
                                let imm_20 = (imm_u >> 20) & 0x1;
                                let imm_10_1 = (imm_u >> 1) & 0x3FF;
                                let imm_11 = (imm_u >> 11) & 0x1;
                                let imm_19_12 = (imm_u >> 12) & 0xFF;
                                let bits = (imm_20 << 31)
                                    | (imm_19_12 << 12)
                                    | (imm_11 << 20)
                                    | (imm_10_1 << 21)
                                    | (rd as u32) << 7
                                    | (opcode as u32);
                                Instruction::new(bits)
                            }
                            0x37 | 0x17 => {
                                // U-type: opcode | rd | imm[31:12]
                                let rd = if rng.gen_bool(chance_take_old) && orig.has_rd() { orig.rd() } else { rng.gen_range(1..32) as u8 };
                                // Keep upper 20 bits; if using existing, extract upper bits, otherwise random upper
                                let upper = if rng.gen_bool(chance_take_old) {
                                    (orig.imm_u() as u32) & 0xFFFFF000
                                } else {
                                    ((rng.gen_range(-524288i32..524288i32) as u32) & 0xFFFFF000)
                                };
                                let bits = upper | (rd as u32) << 7 | (opcode as u32);
                                Instruction::new(bits)
                            }
                            _ => {
                                // fallback to a random R-type
                                let rd = rng.gen_range(1..32) as u8;
                                let funct3 = rng.gen_range(0..8) as u8;
                                let rs1 = rng.gen_range(0..32) as u8;
                                let rs2 = rng.gen_range(0..32) as u8;
                                let funct7 = rng.gen_range(0..128) as u8;
                                let bits = (funct7 as u32) << 25
                                    | (rs2 as u32) << 20
                                    | (rs1 as u32) << 15
                                    | (funct3 as u32) << 12
                                    | (rd as u32) << 7
                                    | 0x33u32;
                                Instruction::new(bits)
                            }
                        };
                        println!("Replacing instruction {} at index {} with new instruction {}",program.instructions[idx], idx, new_instr);
                        program.instructions[idx] = new_instr;
                        return Some(idx);
                    }
                } else {
                    return None;
                }
            }
            MutationType::ChangeAllImmediates => {
                self.mutate_all_immediates(program, rng);
                return Some(0);
            }
            MutationType::SetRegisterToObservedImmediate => {
                // If program is CSR-only, don't insert addi/lui
                if self.program_is_csr_only(program) {
                    return None;
                }
                self.mutate_set_register_to_observed_immediate(program, rng);
                return Some(0);
            }
            MutationType::ChangeCSR => {
                let idx = self.select_mutable_index_with_filter(program, rng, |instr| {
                    // Filter for CSR instructions (opcode 0x73)
                    instr.opcode() == 0x73
                });
                
                if let Some(idx) = idx {
                    // Generate new CSR address in range [0, 4096)
                    let mut new_csr = rng.gen_range(0..4096);   
                    //With high change, pick a valid CSR, out of the 4096 possible, that is commonly used (e.g., mcycle, minstret, etc.)
                    //Define constants for commonly used CSRs
                    let MINSTRET = 0xB02;
                    let MCYCLE = 0xB00;
                    let MINSTRETH = 0xB82;
                    let MSCRATCH = 0x340;
                    let MEPC = 0x341;
                    let MCAUSE = 0x342;
                    let MSTATUS = 0x300;
                    let MISA = 0x301;
                    let MTVEC = 0x305;
                    let MIP = 0x344;
                    if rng.gen_bool(0.7) {
                        let common_csrs = [MINSTRET, MCYCLE, MINSTRETH, MSCRATCH, MEPC, MCAUSE, MSTATUS, MISA, MTVEC];
                        if let Some(&csr) = common_csrs.choose(rng) {
                            new_csr = csr;
                        }
                    }

                    
                    let instr = &mut program.instructions[idx];
                    instr.set_imm_i(new_csr as i32);
                    
                    return Some(idx);
                } else {
                    return None;
                }
            }
        }
    }

    /// Generate a random index that can be mutated with an optional filter
    fn select_mutable_index_with_filter<F>(&self, program: &Program, rng: &mut impl Rng, filter: F) -> Option<usize>
    where
        F: Fn(&Instruction) -> bool,
    {
        match &self.interesting_instructions {
            Some(set) => {
                let valid_indices: Vec<_> = set.borrow().iter()
                    .filter(|&&idx| idx < program.len() && filter(&program.instructions[idx]))
                    .copied()
                    .collect();
                if valid_indices.is_empty() {
                    None
                } else {
                    Some(valid_indices[rng.gen_range(0..valid_indices.len())])
                }
            }
            None => {
                let valid_indices: Vec<_> = (0..program.len())
                    .filter(|&idx| filter(&program.instructions[idx]))
                    .collect();
                if valid_indices.is_empty() {
                    None
                } else {
                    Some(valid_indices[rng.gen_range(0..valid_indices.len())])
                }
            }
        }
    }

    
    /// Select an index for insertion (next to a mutable instruction if restricted)
    fn select_insertion_index(&self, program: &Program, rng: &mut impl Rng) -> usize {
        match &self.interesting_instructions {
            Some(set) => {
                let valid_indices: Vec<_> = set.borrow().iter()
                    .filter(|&&idx| idx <= program.len())
                    .copied()
                    .collect();
                if valid_indices.is_empty() {
                    rng.gen_range(0..=program.len())
                } else {
                    valid_indices[rng.gen_range(0..valid_indices.len())]
                }
            }
            None => rng.gen_range(0..=program.len()),
        }
    }

    /// Swap registers within a single instruction at the given index
    fn mutate_swap_registers_at(&self, program: &mut Program, idx: usize, rng: &mut impl Rng) {
        let instr = &mut program.instructions[idx];

        // Collect valid swap operations based on available fields
        let mut valid_swaps = Vec::new();
        if instr.has_rs1() && instr.has_rs2() {
            valid_swaps.push(0); // Swap rs1 and rs2
        }
        if instr.has_rd() && instr.has_rs1() {
            valid_swaps.push(1); // Swap rd and rs1
        }
        if instr.has_rd() && instr.has_rs2() {
            valid_swaps.push(2); // Swap rd and rs2
        }

        if valid_swaps.is_empty() {
            return;
        }

        let swap_type = valid_swaps[rng.gen_range(0..valid_swaps.len())];
        match swap_type {
            0 => {
                // Swap rs1 and rs2
                let rs1 = instr.rs1();
                let rs2 = instr.rs2();
                instr.set_rs1(rs2);
                instr.set_rs2(rs1);
            }
            1 => {
                // Swap rd and rs1
                let rd = instr.rd();
                let rs1 = instr.rs1();
                instr.set_rd(rs1);
                instr.set_rs1(rd);
            }
            _ => {
                // Swap rd and rs2
                let rd = instr.rd();
                let rs2 = instr.rs2();
                instr.set_rd(rs2);
                instr.set_rs2(rd);
            }
        }
    }

    // Replace the original mutate_swap_registers method
    /// Swap registers within a single instruction (e.g., swap rs1 and rs2)
    pub fn mutate_swap_registers(&self, program: &mut Program, rng: &mut impl Rng) {
        let idx = self.select_mutable_index_with_filter(program, rng, |instr| {
            (instr.has_rs1() && instr.has_rs2()) ||
            (instr.has_rd() && instr.has_rs1()) ||
            (instr.has_rd() && instr.has_rs2())
        });
        if let Some(idx) = idx {
            self.mutate_swap_registers_at(program, idx, rng);
        }
    }

    /// Mutate immediate value in an instruction
    pub fn mutate_immediate(&self, program: &mut Program, rng: &mut impl Rng) -> Option<usize> {
        let idx = self.select_mutable_index_with_filter(program, rng, |instr| {
            matches!(instr.format(), 
                InstructionFormat::I | 
                InstructionFormat::S | 
                InstructionFormat::B | 
                InstructionFormat::U | 
                InstructionFormat::J) && instr.opcode() != 0x73 // Exclude CSR instructions
        });
        if idx.is_none() {
            return None;
        }
        let idx = idx.unwrap();
        let instr = &mut program.instructions[idx];

        match instr.format() {
            InstructionFormat::I => {
                let current = instr.imm_i();
                let new_imm = self.generate_new_immediate(current, rng);
                instr.set_imm_i(new_imm);
            }
            InstructionFormat::S => {
                let current = instr.imm_s();
                let new_imm = self.generate_new_immediate(current, rng);
                instr.set_imm_s(new_imm);
            }
            InstructionFormat::B => {
                let current = instr.imm_b();
                let new_imm = self.generate_new_immediate(current, rng);
                instr.set_imm_b(new_imm);
            }
            InstructionFormat::U => {
                let current = instr.imm_u();
                let new_imm = ((self.generate_new_immediate(current >> 12, rng) << 12) as u32 & 0xFFFFF000) as i32;
                instr.set_imm_u(new_imm);
            }
            InstructionFormat::J => {
                let current = instr.imm_j();
                let new_imm = self.generate_new_immediate(current, rng);
                instr.set_imm_j(new_imm);
            }
            _ => {
                // For R-type or unknown, try I-type anyway
                let new_imm = rng.gen_range(-2048..2048);
                instr.set_imm_i(new_imm);
            }
        }
        Some(idx)
    }

    /// Generate a new immediate value based on mutation strategy
    fn generate_new_immediate(&self, current: i32, rng: &mut impl Rng) -> i32 {
        let strategy = rng.gen_range(0..6);
        match strategy {
            0 => current.wrapping_add(rng.gen_range(-16..16)),  // Small offset
            1 => current.wrapping_mul(2),                        // Double
            2 => current.wrapping_div(2),                        // Halve
            3 => !current,                                       // Bitwise NOT
            4 => rng.gen_range(-2048..2048),                    // Random small
            _ => current ^ rng.gen_range(0..4096),              // XOR with random
        }
    }

    /// Rename a register throughout its definition-use chain
    /// This finds all writes to a source register and renames them consistently
    pub fn mutate_rename_register_chain(&self, program: &mut Program, rng: &mut impl Rng) {
        if program.len() < 2 {
            return;
        }

        // Pick a random instruction to start from (must be mutable)
        let start_idx = self.select_mutable_index_with_filter(program, rng, |_| true);
        if start_idx.is_none() {
            return;
        }
        let start_idx = start_idx.unwrap();
        // let start_idx = 2;
        // println!("Selected instruction index {} for renaming", start_idx);
        let target_instr = &program.instructions[start_idx];

        // Pick which source register to track (rs1 or rs2)
        let old_reg = if rng.gen_bool(0.5) {
            target_instr.rs1()
        } else {
            target_instr.rs2()
        };
        // println!("Renaming register x{} starting from instruction {}", old_reg, start_idx);

        // // Don't rename x0 (hardwired zero)
        // if old_reg == 0 {
        //     return;
        // }

        // Pick a new register to rename to
        let new_reg = rng.gen_range(1..32) as u8;
                // let new_reg = 4;
        if new_reg == old_reg {
            return;
        }

        //println!("Renaming x{} to x{}", old_reg, new_reg);

        // Find all instructions that write to old_reg before start_idx
        let mut writers = Vec::new();
        for i in 0..start_idx {
            if program.instructions[i].rd() == old_reg {
                writers.push(i);
            }
        }

        // Also rename the source register at start_idx
        let start_instr = &mut program.instructions[start_idx];
        if start_instr.rs1() == old_reg {
            start_instr.set_rs1(new_reg);
        }
        if start_instr.rs2() == old_reg {
            start_instr.set_rs2(new_reg);
        }

        // Rename all writers and their subsequent uses
        for &writer_idx in &writers {
            // Find the range where this definition is live
            let mut live_until = start_idx;

            // Look for the next write to old_reg after writer_idx
            for i in (writer_idx + 1)..program.len() {
                if program.instructions[i].rd() == old_reg {
                    live_until = i;
                    break;
                }
            }

            // Rename rd in the writer
            program.instructions[writer_idx].set_rd(new_reg);

            // Rename all uses of old_reg in the live range
            for i in (writer_idx + 1)..live_until {
                let instr = &mut program.instructions[i];
                if instr.rs1() == old_reg {
                    instr.set_rs1(new_reg);
                }
                if instr.rs2() == old_reg {
                    instr.set_rs2(new_reg);
                }
            }
        }
    }

    pub fn mutate_nop_instruction(&self, program: &mut Program, index: usize) {
        program.instructions[index] = Instruction::nop();
    }

    /// Mutate all immediate values in the program
    /// Mutate all immediate values in the program to the same value (clamped per format)
    pub fn mutate_all_immediates(&self, program: &mut Program, rng: &mut impl Rng) {
        // Generate a single random immediate value to use for all instructions
        // Use a reasonable range that covers most immediate types
        let base_imm = rng.gen_range(-1048576..1048576); // Range for J-type (largest)
        
        for idx in 0..program.len() {
            // Skip if we have interesting instructions and this isn't one of them
            if !self.can_mutate_index(idx) {
                continue;
            }

            let instr = &mut program.instructions[idx];

            match instr.format() {
                InstructionFormat::I => {
                    // I-type: 12-bit signed immediate [-2048, 2047]
                    let clamped = base_imm.max(-2048).min(2047);
                    instr.set_imm_i(clamped);
                }
                InstructionFormat::S => {
                    // S-type: 12-bit signed immediate [-2048, 2047]
                    let clamped = base_imm.max(-2048).min(2047);
                    instr.set_imm_s(clamped);
                }
                InstructionFormat::B => {
                    // B-type: 13-bit signed immediate [-4096, 4094] (must be even)
                    let clamped = base_imm.max(-4096).min(4094) & !1; // Clear LSB to make even
                    instr.set_imm_b(clamped);
                }
                InstructionFormat::U => {
                    // U-type: upper 20 bits [0, 1048575] << 12
                    let clamped_20bit = (base_imm.abs() % 1048576) as u32;
                    let new_imm = ((clamped_20bit << 12) & 0xFFFFF000) as i32;
                    instr.set_imm_u(new_imm);
                }
                InstructionFormat::J => {
                    // J-type: 21-bit signed immediate [-1048576, 1048575] (must be even)
                    let clamped = base_imm.max(-1048576).min(1048575) & !1; // Clear LSB to make even
                    instr.set_imm_j(clamped);
                }
                _ => {
                    // R-type or unknown - skip
                }
            }
        }
    }
    /// Check if an instruction index can be mutated
    fn can_mutate_index(&self, idx: usize) -> bool {
        match &self.interesting_instructions {
            Some(set) => set.borrow().contains(&idx),
            None => true,
        }
    }

    /// Collect all immediate values observed in the program
    fn collect_observed_immediates(&self, program: &Program) -> Vec<i32> {
        let mut immediates = Vec::new();
        
        for instr in &program.instructions {
            match instr.format() {
                InstructionFormat::I => {
                    immediates.push(instr.imm_i());
                }
                InstructionFormat::S => {
                    immediates.push(instr.imm_s());
                }
                InstructionFormat::B => {
                    immediates.push(instr.imm_b());
                }
                InstructionFormat::U => {
                    immediates.push(instr.imm_u());
                }
                InstructionFormat::J => {
                    immediates.push(instr.imm_j());
                }
                _ => {}
            }
        }
        
        immediates
    }

    /// Set a random register to an immediate value observed elsewhere in the program
    pub fn mutate_set_register_to_observed_immediate(&self, program: &mut Program, rng: &mut impl Rng) {
        // Only insert if we haven't reached max length
        if !self.can_insert(program.len()) {
            return;
        }

        // Collect all observed immediates
        let immediates = self.collect_observed_immediates(program);
        
        if immediates.is_empty() {
            return;
        }
        
        // Pick a random immediate
        let imm = immediates[rng.gen_range(0..immediates.len())];
        
        // Pick a random destination register (not x0)
        let rd = rng.gen_range(1..32) as u8;
        
        // Decide whether to use addi or lui based on the immediate value
        let new_instr = if imm >= -2048 && imm < 2048 {
            // Use addi: addi rd, x0, imm
            let imm_u32 = (imm as u32) & 0xFFF;
            let instr_bits = (imm_u32 << 20)
                | ((rd as u32) << 7)
                | 0x13; // I-type opcode
            Instruction::new(instr_bits)
        } else {
            // Use lui for larger immediates: lui rd, imm[31:12]
            // LUI takes upper 20 bits, so we need to extract them
            let upper_20 = ((imm as u32) & 0xFFFFF000) as i32;
            let instr_bits = (upper_20 as u32)
                | ((rd as u32) << 7)
                | 0x37; // U-type opcode for LUI
            Instruction::new(instr_bits)
        };
        
        // Insert at a random or restricted position
        let insert_idx = self.select_insertion_index(program, rng);
        program.instructions.insert(insert_idx, new_instr);
        
        // Update interesting_instructions if needed
        if let Some(ref set_cell) = self.interesting_instructions {
            let mut set = set_cell.borrow_mut();
            let mut new_set = HashSet::new();
            
            for &idx in set.iter() {
                if idx < insert_idx {
                    new_set.insert(idx);
                } else {
                    new_set.insert(idx + 1);
                }
            }
            
            *set = new_set;
        }
    }
}

impl Default for Mutator {
    fn default() -> Self {
        Self::new()
    }
}
