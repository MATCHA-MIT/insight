use anyhow::{Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub bytes: u32,
}

impl std::fmt::Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let op = self.opcode();
        let rd = self.rd();
        let rs1 = self.rs1();
        let rs2 = self.rs2();
        let funct3 = self.funct3();
        let funct7 = self.funct7();

        match self.format() {
            InstructionFormat::R => {
                let mnem = match funct3 {
                    0x0 => if funct7 == 0x20 { "sub" } else { "add" },
                    0x1 => "sll",
                    0x2 => "slt",
                    0x3 => "sltu",
                    0x4 => "xor",
                    0x5 => if funct7 == 0x20 { "sra" } else { "srl" },
                    0x6 => "or",
                    0x7 => "and",
                    _ => "unknown",
                };
                write!(f, "{} x{}, x{}, x{}", mnem, rd, rs1, rs2)
            }

            InstructionFormat::I => {
                match op {
                    0x03 => { // loads
                        let imm = self.imm_i();
                        let mnem = match funct3 {
                            0x0 => "lb",
                            0x1 => "lh",
                            0x2 => "lw",
                            0x4 => "lbu",
                            0x5 => "lhu",
                            _ => "ld_unknown",
                        };
                        write!(f, "{} x{}, {}(x{})", mnem, rd, imm, rs1)
                    }
                    0x13 => { // arithmetic immediate
                        match funct3 {
                            0x0 => write!(f, "addi x{}, x{}, {}", rd, rs1, self.imm_i()),
                            0x2 => write!(f, "slti x{}, x{}, {}", rd, rs1, self.imm_i()),
                            0x3 => write!(f, "sltiu x{}, x{}, {}", rd, rs1, self.imm_i()),
                            0x4 => write!(f, "xori x{}, x{}, {}", rd, rs1, self.imm_i()),
                            0x6 => write!(f, "ori x{}, x{}, {}", rd, rs1, self.imm_i()),
                            0x7 => write!(f, "andi x{}, x{}, {}", rd, rs1, self.imm_i()),
                            0x1 => { // slli
                                let shamt = ((self.bytes >> 20) & 0x1F) as u32;
                                write!(f, "slli x{}, x{}, {}", rd, rs1, shamt)
                            }
                            0x5 => { // srli / srai
                                let shamt = ((self.bytes >> 20) & 0x1F) as u32;
                                if funct7 == 0x20 {
                                    write!(f, "srai x{}, x{}, {}", rd, rs1, shamt)
                                } else {
                                    write!(f, "srli x{}, x{}, {}", rd, rs1, shamt)
                                }
                            }
                            _ => write!(f, "imm_unknown x{}, x{}, {}", rd, rs1, self.imm_i()),
                        }
                    }
                    0x67 => { // jalr
                        write!(f, "jalr x{}, {}(x{})", rd, self.imm_i(), rs1)
                    }
                    0x73 => { // system
                        match self.imm_i() {
                            0 => write!(f, "ecall"),
                            1 => write!(f, "ebreak"),
                            _ => write!(f, "system 0x{:x}", self.imm_i()),
                        }
                    }
                    _ => write!(f, "i_unknown 0x{:08x}", self.bytes),
                }
            }

            InstructionFormat::S => {
                let mnem = match funct3 {
                    0x0 => "sb",
                    0x1 => "sh",
                    0x2 => "sw",
                    _ => "s_unknown",
                };
                write!(f, "{} x{}, {}(x{})", mnem, rs2, self.imm_s(), rs1)
            }

            InstructionFormat::B => {
                let mnem = match funct3 {
                    0x0 => "beq",
                    0x1 => "bne",
                    0x4 => "blt",
                    0x5 => "bge",
                    0x6 => "bltu",
                    0x7 => "bgeu",
                    _ => "b_unknown",
                };
                write!(f, "{} x{}, x{}, {}", mnem, rs1, rs2, self.imm_b())
            }

            InstructionFormat::U => {
                match op {
                    0x17 => write!(f, "auipc x{}, 0x{:x}", rd, self.imm_u() as u32),
                    0x37 => write!(f, "lui x{}, 0x{:x}", rd, self.imm_u() as u32),
                    _ => write!(f, "u_unknown 0x{:08x}", self.bytes),
                }
            }

            InstructionFormat::J => {
                write!(f, "jal x{}, {}", rd, self.imm_j())
            }

            InstructionFormat::Unknown => {
                write!(f, "0x{:08x}", self.bytes)
            }
        }
    }
}

impl Instruction {
    pub fn new(bytes: u32) -> Self {
        Self { bytes }
    }

    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self {
            bytes: u32::from_le_bytes(bytes),
        }
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        self.bytes.to_le_bytes()
    }

    pub fn is_nop(&self) -> bool {
        self.bytes == 0x00000013
    }

    // Extract opcode (bits 0-6)
    pub fn opcode(&self) -> u8 {
        (self.bytes & 0x7F) as u8
    }

    // Extract rd (bits 7-11)
    pub fn rd(&self) -> u8 {
        ((self.bytes >> 7) & 0x1F) as u8
    }

    // Extract rs1 (bits 15-19)
    pub fn rs1(&self) -> u8 {
        ((self.bytes >> 15) & 0x1F) as u8
    }

    // Extract rs2 (bits 20-24)
    pub fn rs2(&self) -> u8 {
        ((self.bytes >> 20) & 0x1F) as u8
    }

    // Extract funct3 (bits 12-14)
    pub fn funct3(&self) -> u8 {
        ((self.bytes >> 12) & 0x7) as u8
    }

    // Extract funct7 (bits 25-31)
    pub fn funct7(&self) -> u8 {
        ((self.bytes >> 25) & 0x7F) as u8
    }

    // Set opcode
    pub fn set_opcode(&mut self, opcode: u8) {
        self.bytes = (self.bytes & !0x7F) | (opcode as u32 & 0x7F);
    }

    // Set rd
    pub fn set_rd(&mut self, rd: u8) {
        self.bytes = (self.bytes & !(0x1F << 7)) | ((rd as u32 & 0x1F) << 7);
    }

    // Set rs1
    pub fn set_rs1(&mut self, rs1: u8) {
        self.bytes = (self.bytes & !(0x1F << 15)) | ((rs1 as u32 & 0x1F) << 15);
    }

    // Set rs2
    pub fn set_rs2(&mut self, rs2: u8) {
        self.bytes = (self.bytes & !(0x1F << 20)) | ((rs2 as u32 & 0x1F) << 20);
    }

    // Set funct3
    pub fn set_funct3(&mut self, funct3: u8) {
        self.bytes = (self.bytes & !(0x7 << 12)) | ((funct3 as u32 & 0x7) << 12);
    }

    // Set funct7
    pub fn set_funct7(&mut self, funct7: u8) {
        self.bytes = (self.bytes & !(0x7F << 25)) | ((funct7 as u32 & 0x7F) << 25);
    }

    /// Extract immediate value for I-type instructions (bits 20-31)
    pub fn imm_i(&self) -> i32 {
        (self.bytes as i32) >> 20
    }

    /// Set immediate value for I-type instructions
    pub fn set_imm_i(&mut self, imm: i32) {
        let imm_bits = (imm as u32) & 0xFFF;
        self.bytes = (self.bytes & 0xFFFFF) | (imm_bits << 20);
    }

    /// Extract immediate value for S-type instructions
    pub fn imm_s(&self) -> i32 {
        let imm_11_5 = (self.bytes >> 25) & 0x7F;
        let imm_4_0 = (self.bytes >> 7) & 0x1F;
        let imm = (imm_11_5 << 5) | imm_4_0;
        // Sign extend from 12 bits
        ((imm as i32) << 20) >> 20
    }

    /// Set immediate value for S-type instructions
    pub fn set_imm_s(&mut self, imm: i32) {
        let imm = imm as u32;
        let imm_11_5 = (imm >> 5) & 0x7F;
        let imm_4_0 = imm & 0x1F;
        self.bytes = (self.bytes & 0x1FFF07F) | (imm_11_5 << 25) | (imm_4_0 << 7);
    }

    /// Extract immediate value for B-type instructions
    pub fn imm_b(&self) -> i32 {
        let imm_12 = (self.bytes >> 31) & 0x1;
        let imm_10_5 = (self.bytes >> 25) & 0x3F;
        let imm_4_1 = (self.bytes >> 8) & 0xF;
        let imm_11 = (self.bytes >> 7) & 0x1;
        let imm = (imm_12 << 12) | (imm_11 << 11) | (imm_10_5 << 5) | (imm_4_1 << 1);
        // Sign extend from 13 bits
        ((imm as i32) << 19) >> 19
    }

    /// Set immediate value for B-type instructions
    pub fn set_imm_b(&mut self, imm: i32) {
        let imm = imm as u32;
        let imm_12 = (imm >> 12) & 0x1;
        let imm_11 = (imm >> 11) & 0x1;
        let imm_10_5 = (imm >> 5) & 0x3F;
        let imm_4_1 = (imm >> 1) & 0xF;
        self.bytes = (self.bytes & 0x1FFF07F) 
            | (imm_12 << 31) 
            | (imm_11 << 7) 
            | (imm_10_5 << 25) 
            | (imm_4_1 << 8);
    }

    /// Extract immediate value for U-type instructions (bits 12-31)
    pub fn imm_u(&self) -> i32 {
        (self.bytes & 0xFFFFF000) as i32
    }

    /// Set immediate value for U-type instructions
    pub fn set_imm_u(&mut self, imm: i32) {
        self.bytes = (self.bytes & 0xFFF) | ((imm as u32) & 0xFFFFF000);
    }

    /// Extract immediate value for J-type instructions
    pub fn imm_j(&self) -> i32 {
        let imm_20 = (self.bytes >> 31) & 0x1;
        let imm_10_1 = (self.bytes >> 21) & 0x3FF;
        let imm_11 = (self.bytes >> 20) & 0x1;
        let imm_19_12 = (self.bytes >> 12) & 0xFF;
        let imm = (imm_20 << 20) | (imm_19_12 << 12) | (imm_11 << 11) | (imm_10_1 << 1);
        // Sign extend from 21 bits
        ((imm as i32) << 11) >> 11
    }

    /// Set immediate value for J-type instructions
    pub fn set_imm_j(&mut self, imm: i32) {
        let imm = imm as u32;
        let imm_20 = (imm >> 20) & 0x1;
        let imm_19_12 = (imm >> 12) & 0xFF;
        let imm_11 = (imm >> 11) & 0x1;
        let imm_10_1 = (imm >> 1) & 0x3FF;
        self.bytes = (self.bytes & 0xFFF) 
            | (imm_20 << 31) 
            | (imm_19_12 << 12) 
            | (imm_11 << 20) 
            | (imm_10_1 << 21);
    }

    /// Determine instruction format based on opcode
    pub fn format(&self) -> InstructionFormat {
        match self.opcode() {
            0x03 | 0x13 | 0x67 | 0x73 => InstructionFormat::I,
            0x23 => InstructionFormat::S,
            0x63 => InstructionFormat::B,
            0x17 | 0x37 => InstructionFormat::U,
            0x6F => InstructionFormat::J,
            0x33 | 0x3B => InstructionFormat::R,
            _ => InstructionFormat::Unknown,
        }
    }

    pub fn has_rd(&self) -> bool {
        match self.opcode() {
            0x03 | 0x13 | 0x17 | 0x37 | 0x6F | 0x33 | 0x3B | 0x67 | 0x73 => true,
            _ => false,
        }
    }

    pub fn has_rs1(&self) -> bool {
        match self.opcode() {
            0x03 | 0x13 | 0x23 | 0x63 | 0x67 | 0x33 | 0x3B | 0x73  => true,
            _ => false,
        }
    }

    pub fn has_rs2(&self) -> bool {
        match self.opcode() {
            0x23 | 0x63 | 0x33 | 0x3B => true,
            _ => false,
        }
    }

    pub fn has_funct3(&self) -> bool {
        match self.opcode() {
            0x03 | 0x13 | 0x23 | 0x63 | 0x33 | 0x3B => true,
            _ => false,
        }
    }

    pub fn has_funct7(&self) -> bool {
        match self.opcode() {
            0x33 | 0x3B => true,
            _ => false,
        }
    }

    /// Create a NOP instruction (ADDI x0, x0, 0)
    pub fn nop() -> Self {
        Self::new(0x00000013)
    }
    // Create a simple add instruction (ADD rd, rs1, rs2)
    pub fn add(rd: u8, rs1: u8, rs2: u8) -> Self {
        let mut instr = Self::new(0);
        instr.set_opcode(0x33);
        instr.set_rd(rd);
        instr.set_rs1(rs1);
        instr.set_rs2(rs2);
        instr.set_funct3(0x0);
        instr.set_funct7(0x00);
        instr
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionFormat {
    R,  // Register
    I,  // Immediate
    S,  // Store
    B,  // Branch
    U,  // Upper immediate
    J,  // Jump
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub instructions: Vec<Instruction>,
}

impl Program {
    pub fn from_file(path: &Path) -> Result<Self> {
        let file = File::open(path).context("Failed to open input file")?;
        let mut reader = BufReader::new(file);
        let mut instructions = Vec::new();

        loop {
            match reader.read_u32::<LittleEndian>() {
                Ok(bytes) => instructions.push(Instruction::new(bytes)),
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
        }

        Ok(Self { instructions })
    }

    pub fn to_file(&self, path: &Path) -> Result<()> {
        let file = File::create(path).context("Failed to create output file")?;
        let mut writer = BufWriter::new(file);

        for instr in &self.instructions {
            writer.write_u32::<LittleEndian>(instr.bytes)?;
        }

        writer.flush()?;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }
}
