// Copyright (c) 2020 Sonal Pinto
// SPDX-License-Identifier: Apache-2.0

package kronos_types;

// ============================================================
// Types
typedef logic [31:0] instr_t;

typedef struct packed {
    logic [31:0] pc;
    logic [31:0] ir;
} pipeIFID_t;

typedef struct packed {
    logic [31:0] pc;
    logic [31:0] ir;
    logic [31:0] op1;
    logic [31:0] op2;
    logic [31:0] rs1_data;
    logic [31:0] rs2_data;
    logic [31:0] addr;
    // ------------------------
    // EX controls
    logic        basic;
    logic [3:0]  aluop;
    logic        regwr_alu;
    logic        jump;
    logic        branch;
    logic        load;
    logic        store;
    logic [3:0]  mask;
    logic        csr;
    logic        system;
    logic [1:0]  sysop;
    logic        illegal;
    logic        misaligned_jmp;
    logic        misaligned_ldst;
} pipeIDEX_t;

// ============================================================
// Instruction Types: {opcode[6:2]}
localparam logic [4:0] INSTR_LOAD  = 5'b00_000;
localparam logic [4:0] INSTR_STORE = 5'b01_000;
localparam logic [4:0] INSTR_BR    = 5'b11_000;

localparam logic [4:0] INSTR_JALR  = 5'b11_001;

localparam logic [4:0] INSTR_MISC  = 5'b00_011;
localparam logic [4:0] INSTR_JAL   = 5'b11_011;

localparam logic [4:0] INSTR_OPIMM = 5'b00_100;
localparam logic [4:0] INSTR_OP    = 5'b01_100;
localparam logic [4:0] INSTR_SYS   = 5'b11_100;

localparam logic [4:0] INSTR_AUIPC = 5'b00_101;
localparam logic [4:0] INSTR_LUI   = 5'b01_101;

// ============================================================
// ALU Operations
localparam logic [3:0] ADD       = 4'b0000;
localparam logic [3:0] SUB       = 4'b1000;
localparam logic [3:0] SLT       = 4'b0010;
localparam logic [3:0] SLTU      = 4'b0011;
localparam logic [3:0] XOR       = 4'b0100;
localparam logic [3:0] OR        = 4'b0110;
localparam logic [3:0] AND       = 4'b0111;
localparam logic [3:0] SLL       = 4'b0001;
localparam logic [3:0] SRL       = 4'b0101;
localparam logic [3:0] SRA       = 4'b1101;

// ============================================================
// Branch Operations
localparam logic [2:0] BEQ       = 3'b000;
localparam logic [2:0] BNE       = 3'b001;
localparam logic [2:0] BLT       = 3'b100;
localparam logic [2:0] BGE       = 3'b101;
localparam logic [2:0] BLTU      = 3'b110;
localparam logic [2:0] BGEU      = 3'b111;

localparam logic [1:0] EQ        = 2'b00;
localparam logic [1:0] LT        = 2'b01;
localparam logic [1:0] GT        = 2'b10;

// ============================================================
// Memory Access Size
localparam logic [1:0] BYTE      = 2'b00;
localparam logic [1:0] HALF      = 2'b01;
localparam logic [1:0] WORD      = 2'b10;

// ============================================================
// System Operations
localparam logic [1:0] ECALL     = 2'b00;
localparam logic [1:0] EBREAK    = 2'b01;
localparam logic [1:0] MRET      = 2'b10;
localparam logic [1:0] WFI       = 2'b11;

// ============================================================
// Constants
localparam logic [31:0] ZERO   = 32'h0;
localparam logic [31:0] FOUR   = 32'h4;

// ============================================================
// Interrupts
localparam logic [3:0] SOFTWARE_INTERRUPT    = 4'd3;
localparam logic [3:0] TIMER_INTERRUPT       = 4'd7;
localparam logic [3:0] EXTERNAL_INTERRUPT    = 4'd11;

// ============================================================
// Exceptions
localparam logic [3:0] INSTR_ADDR_MISALIGNED = 4'd0;
localparam logic [3:0] ILLEGAL_INSTR         = 4'd2;
localparam logic [3:0] BREAKPOINT            = 4'd3;
localparam logic [3:0] LOAD_ADDR_MISALIGNED  = 4'd4;
localparam logic [3:0] STORE_ADDR_MISALIGNED = 4'd6;
localparam logic [3:0] ECALL_MACHINE         = 4'd11;

// ============================================================
// Control Status Register

// CSR operations
localparam logic [1:0]  CSR_RW       = 2'b01;
localparam logic [1:0]  CSR_RS       = 2'b10;
localparam logic [1:0]  CSR_RC       = 2'b11;

// CSR Address
localparam logic [11:0] MSTATUS      = 12'h300;
localparam logic [11:0] MIE          = 12'h304;
localparam logic [11:0] MTVEC        = 12'h305;

localparam logic [11:0] MSCRATCH     = 12'h340;
localparam logic [11:0] MEPC         = 12'h341;
localparam logic [11:0] MCAUSE       = 12'h342;
localparam logic [11:0] MTVAL        = 12'h343;
localparam logic [11:0] MIP          = 12'h344;

localparam logic [11:0] MCYCLE       = 12'hB00;
localparam logic [11:0] MINSTRET     = 12'hB02;
localparam logic [11:0] MCYCLEH      = 12'hB80;
localparam logic [11:0] MINSTRETH    = 12'hB82;

// Privilege levels
localparam logic [1:0] PRIVILEGE_MACHINE = 2'b11;
// mtvec modes
localparam logic [1:0] DIRECT_MODE   = 2'b00;
 
endpackage
