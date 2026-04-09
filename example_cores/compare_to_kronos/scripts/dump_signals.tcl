# STEP: Input design
set_automatic_library_search on
analyze \
  -sv -lib uArchLib -mfcu \
  -y ../kronos_designs/kronos_baseline/rtl/core -y ../rtl \
  +libext+.sv \
  +incdir+../kronos_designs/kronos_baseline/rtl/core \
  +define+SYNTHESIS +define+TIMER_ADDR=32'hcc000000 +define+MEMSIZE=512 \
  -L ../kronos_designs/kronos_baseline/rtl ../kronos_designs/kronos_baseline/rtl/core/kronos_types.sv \
  -sv  ../rtl/sim_top.sv

analyze \
  -lib sodorLib -mfcu \
  -y ../sodor_verilog +libext+.sv +define+SYNTHESIS \
  -sv ../sodor_verilog/Sodor_SodorInternalTile.sv
analyze -sv ../correctness.sv

elaborate -top cfg -disable_auto_bbox
clock clk
reset rst -non_resettable_regs 0 -init_state ../build_config/KRONOS.init 
#Dump all signals, space separated
get_design_info -verbosity silent -list signal
exit -force
