use rand::prelude::*;
use crate::instruction::InstructionFormat;

pub fn get_valid_funct7_for_opcode(opcode: u8, rng: &mut impl Rng) -> u8 {
    match opcode {
        0x33 | 0x3B => {
            // R-type
            let funct7_options = vec![0x00, 0x20, 0x01, 0x05, 0x09, 0x0B, 0x10, 0x18, 0x1A, 0x1B];
            *funct7_options.choose(rng).unwrap()
        }
        0x13 | 0x1B => {
            // I-type ALU
            let funct7_options = vec![0x00, 0x20, 0x01, 0x05, 0x09];
            *funct7_options.choose(rng).unwrap()
        }
        0x3 | 0x67 => {
            // Load and JALR
            0x00
        }
        0x23 => {
            // S-type Store
            0x00
        }
        _ => {
            // Default to 0 for other types
            0x00
        }
    }
}

pub fn get_opcodes_for_format(format: InstructionFormat) -> Vec<u8> {
    match format {
        InstructionFormat::R => vec![0x33], //0x3B
        InstructionFormat::I => vec![0x13, 0x1B, 0x03, 0x67, 0x73], // Added 0x73 (CSR/system)
        InstructionFormat::S => vec![0x23],
        InstructionFormat::B => vec![0x63],
        InstructionFormat::U => vec![0x37, 0x17],
        InstructionFormat::J => vec![0x6F],
        InstructionFormat::Unknown => vec![],
    }
}