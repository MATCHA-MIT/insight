// Copyright 2022 Flavien Solt, ETH Zurich.
// Licensed under the General Public License, Version 3.0, see LICENSE for details.
// SPDX-License-Identifier: GPL-3.0-only

module kronos_tiny_soc #(
    // The two below must be equal.
    parameter int unsigned InstrMemDepth = 1 << 20, // 32-bit words
    parameter int unsigned DataMemDepth  = 1 << 20, // 32-bit words

    localparam type data_t = logic [31:0],
    localparam type strb_t = logic [31:0], // The strobe is bitwise.
    localparam type addr_t = logic [31:0]
) (
  input logic clk_i,
  input logic rst_ni,

  ////////////////////
  // Memory signals //
  ////////////////////

  output logic  instr_mem_req,
  output logic  instr_mem_gnt,
  output addr_t instr_mem_addr,
  output data_t instr_mem_wdata,
  output strb_t instr_mem_strb,
  output logic  instr_mem_we,
  output data_t instr_mem_rdata,

  output logic  data_mem_req,
  output logic  data_mem_gnt,
  output addr_t data_mem_addr,
  output data_t data_mem_wdata,
  output strb_t data_mem_strb,
  output logic  data_mem_we,
  output data_t data_mem_rdata
);

  /////////
  // CPU //
  /////////

  kronos_mem_top i_kronos_mem_top (
    .clk_i              (clk_i),
    .rst_ni             (rst_ni),
    .data_mem_req       (data_mem_req),
    .data_mem_gnt       (data_mem_gnt),
    .data_mem_addr      (data_mem_addr),
    .data_mem_wdata     (data_mem_wdata),
    .data_mem_strb      (data_mem_strb),
    .data_mem_we        (data_mem_we),
    .data_mem_rdata     (data_mem_rdata),
    .instr_mem_req      (instr_mem_req),
    .instr_mem_gnt      (instr_mem_gnt),
    .instr_mem_addr     (instr_mem_addr),
    .instr_mem_wdata    (instr_mem_wdata),
    .instr_mem_strb     (instr_mem_strb),
    .instr_mem_we       (instr_mem_we),
    .instr_mem_rdata    (instr_mem_rdata),
    .software_interrupt (software_interrupt),
    .timer_interrupt    (timer_interrupt),
    .external_interrupt (external_interrupt)
  );

  //////////////////////////////
  // Instruction ROM instance //
  //////////////////////////////

  sram_mem #(
    .Width(32),
    .Depth(InstrMemDepth),
    .RelocateRequestUp(64'h10000000) // 80000000 >> 3
  ) i_instr_rom (
    .clk_i,
    .rst_ni,

    .req_i(instr_mem_req),
    .write_i(instr_mem_we),
    .addr_i(instr_mem_addr >> 2), // 32-bit words
    .wdata_i(instr_mem_wdata),
    .wmask_i(instr_mem_strb),
    .rdata_o(instr_mem_rdata)
  );

  assign instr_mem_gnt = '1;

  ////////////////////////
  // Data SRAM instance //
  ////////////////////////

  sram_mem #(
    .Width(32),
    .Depth(DataMemDepth),
    .RelocateRequestUp(64'h10000000) // 80000000 >> 3
  ) i_data_sram (
    .clk_i,
    .rst_ni,

    .req_i(data_mem_req),
    .write_i(data_mem_we),
    .addr_i(data_mem_addr >> 2), // 32-bit words
    .wdata_i(data_mem_wdata),
    .wmask_i(data_mem_strb),
    .rdata_o(data_mem_rdata)
  );

  assign data_mem_gnt = '1;

  assign external_interrupt = '0;
  assign software_interrupt = '0;
  assign timer_interrupt = '0;

endmodule
