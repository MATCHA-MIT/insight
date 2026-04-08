set signals [list \
    "top.clk_i" \
    "top.rst_ni" \
    "top.kronos_tiny_soc.instr_mem_req" \
    "top.kronos_tiny_soc.instr_mem_gnt" \
    "top.kronos_tiny_soc.instr_mem_addr" \
    "top.kronos_tiny_soc.instr_mem_wdata" \
    "top.kronos_tiny_soc.instr_mem_strb" \
    "top.kronos_tiny_soc.instr_mem_we" \
    "top.kronos_tiny_soc.instr_mem_rdata" \
    "top.kronos_tiny_soc.instr_mem_req_t0" \
    "top.kronos_tiny_soc.instr_mem_gnt_t0" \
    "top.kronos_tiny_soc.instr_mem_addr_t0" \
    "top.kronos_tiny_soc.instr_mem_wdata_t0" \
    "top.kronos_tiny_soc.instr_mem_strb_t0" \
    "top.kronos_tiny_soc.instr_mem_we_t0" \
    "top.kronos_tiny_soc.instr_mem_rdata_t0" \
]

gtkwave::addSignalsFromList $signals