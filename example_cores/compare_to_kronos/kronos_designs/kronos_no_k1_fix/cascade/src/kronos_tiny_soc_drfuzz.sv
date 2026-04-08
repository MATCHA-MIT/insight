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

  ///////////
  // RFUZZ //
  ///////////

  input logic meta_rst_ni,
  input logic [69:0] fuzz_in,
  output logic [176:0] auto_cover_out
);

  kronos_mem_top i_kronos_mem_top (
    .clk_i              (clk_i),
    .rst_ni             (rst_ni),
    .data_mem_req       (data_mem_req),
    .data_mem_addr      (data_mem_addr),
    .data_mem_wdata     (data_mem_wdata),
    .data_mem_strb      (data_mem_strb),
    .data_mem_we        (data_mem_we),
    .instr_mem_req      (instr_mem_req),
    .instr_mem_addr     (instr_mem_addr),
    .instr_mem_wdata    (instr_mem_wdata),
    .instr_mem_strb     (instr_mem_strb),
    .instr_mem_we       (instr_mem_we),
    .fuzz_in            (fuzz_in),
    .metaReset          (~meta_reset_ni),
    .auto_cover_out     (auto_cover_out)
  );

endmodule
