
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
assume {!((sodor_core.core.c._csignals_T_77 == 1))}
assume {valid_inst || !correctness_inst.ref_stalled_out}
assume {!((sodor_core.core.d.imm_i_sext == sodor_core.core.d.csr.value && (5'h9 == sodor_core.core._c_io_ctl_alu_fun) == 1))}
assume {!((sodor_core.core.d.imm_i_sext == sodor_core.core.d.alu_op1 && sodor_core.core.c._csignals_T_31 == 1))}
assume {!((sodor_core.core.c._csignals_T_45 == 1 && sodor_core.core.d.regfile_ext.W0_en == 1))}
assume {!((sodor_core.core.c._csignals_T_85 == 1))}
assume {!((boom.dut.BoomTile.core.rob.rob_uop_0_uopc == 30))}
assume {!(((5'h2 == sodor_core.core._c_io_ctl_alu_fun) == 1 && sodor_core.core.d.regfile_ext.W0_en == 1))}
assume {!((boom.dut.BoomTile.core.rename_stage.r_uop_lrs3 == boom.dut.BoomTile.core.rob._GEN_22[7] && boom.dut.BoomTile.core.rob.rob_uop_0_uopc == 58))}
assume {!((boom.dut.BoomTile.core.rob.rob_uop_0_uopc == 33))}
assume {sodor_core.router.resp_in_range == 1 || sodor_core.router.io_corePort_req_valid == 0}
assume {!((boom.dut.BoomTile.core._rob_io_commit_uops_0_is_jalr == 1))}
assume {!(((&sodor_core.core.d.csr.io_rw_cmd[32'h0 +: 2]) == 1))}
assume {boom.dut.TileLink_Memory.addr_violation == 0}
assume {!((sodor_core.core._c_io_ctl_exception == 1 && sodor_core.core.d._tval_inst_ma_T == 1))}
assume {same_mem}
assume {!((boom.dut.BoomTile.core.rob.rob_uop_0_uopc == 31))}
assume {!((sodor_core.core.c._csignals_T_79 == 1))}
assume {sodor_core.router_1.resp_in_range == 1 || sodor_core.router_1.io_corePort_req_valid == 0}
assume {!((boom.dut.BoomTile.core.rob.rob_uop_0_uopc == 57 && boom.dut.BoomTile.core.rename_stage.r_uop_lrs3 == boom.dut.BoomTile.core.rob._GEN_22[7]))}
assume {!((sodor_core.core.d.regfile_ext.W0_en == 1 && sodor_core.core.c._csignals_T_47 == 1))}
# ADD ASSUMPTIONS END

# STEP: Correct
#assert {correct}

#assert {counter <= 60}

# STEP: Prove
#assert { @(posedge clk) disable iff (rst) correct } -name correct_check

prove -property {correct_check} -asserts -dump_trace -dump_trace_type vcd -dump_trace_dir jg_vcd_out 
#-cex_limit 1
