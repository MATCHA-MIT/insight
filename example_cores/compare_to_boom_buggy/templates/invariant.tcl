
# STEP: Input design
set_automatic_library_search on
analyze \
  -sv -lib uArchLib -mfcu \
  -y verilog_externalmem_original \
  +libext+.v+.sv +define+SYNTHESIS \
  -y rtl/ \
  -v verilog_externalmem_original/chipyard.harness.TestHarness.SmallBoomV3Config.top.mems.v \
  -v verilog_externalmem_original/TileLink_Memory.sv \
  -sv rtl/sim_top.sv
analyze \
  -lib sodorLib -mfcu \
  -y sodor_verilog +libext+.sv +define+SYNTHESIS \
  -sv sodor_verilog/Vincent_SodorInternalTile.sv
analyze -sv correctness.v

elaborate -top cfg -disable_auto_bbox
clock clk
reset rst -non_resettable_regs 0 -init_state BOOM.init 

set_prove_per_property_max_time_limit 360s

abstract -init_value { \
    sodor_core.memory.mem_ext.Memory[0]
    boom.dut.TileLink_Memory.mem[0]
    sodor_core.memory.mem_ext.Memory[1]
    boom.dut.TileLink_Memory.mem[1]
}



# STEP: Simplification
#assume {simplification}




# STEP: Same initial memory
#assume {same_mem}
assume {same_init_mem}
#assume {non_illegal_instruction}
assume {!branching}
assume {valid_inst}

# ADD ASSUMPTIONS START (DO NOT EDIT)
# ADD ASSUMPTIONS END

# STEP: Correct
#assert {correct}

#assert {counter <= 60}

# STEP: Prove
#assert { @(posedge clk) disable iff (rst) correct } -name correct_check

prove -property {correct_check} -asserts -dump_trace -dump_trace_type vcd -dump_trace_dir jg_vcd_out 
#-cex_limit 1
