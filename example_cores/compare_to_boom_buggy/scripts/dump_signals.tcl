# STEP: Input design
set_automatic_library_search on
analyze \
  -sv -lib uArchLib -mfcu \
  -y ../boom_designs/boom_baseline \
  +libext+.v+.sv +define+SYNTHESIS \
  -y ../rtl/ \
  -v ../boom_designs/boom_baseline/chipyard.harness.TestHarness.SmallBoomV3Config.top.mems.v \
  -v ../rtl/TileLink_Memory.sv \
  -sv ../rtl/sim_top.sv \
  -bbox_m FPU* \
  -bbox_m FpPipeline* \
  -bbox_m FPUUnit* \
  -bbox_m FPToInt* \
  -bbox_m IntToFP* \
  -bbox_m PipelinedMulUnit* \
  -bbox_m DivSqrtRecF64_*

analyze \
  -lib sodorLib -mfcu \
  -y ../sodor_verilog +libext+.sv +define+SYNTHESIS \
  -sv ../sodor_verilog/Sodor_SodorInternalTile.sv
analyze -sv ../correctness.sv

elaborate -top cfg -bbox_i boom.dut.BoomTile.core.fp_rename_stage -bbox_i boom.dut.BoomTile.core.FpPipeline -bbox_i boom.dut.BoomTile.core.alu_exe_unit.IntToFPUnit -disable_auto_bbox
clock clk
reset rst -non_resettable_regs 0 -init_state ../build_config/BOOM.init 
#Dump all signals, space separated
get_design_info -verbosity silent -list signal
exit -force
