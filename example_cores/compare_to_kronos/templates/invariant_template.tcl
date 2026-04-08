# STEP: Input design
set_automatic_library_search on
analyze \
  -sv -lib uArchLib -mfcu \
  -y kronos/rtl/core -y rtl \
  +libext+.sv \
  +incdir+kronos/rtl/core \
  +define+SYNTHESIS +define+TIMER_ADDR=32'hcc000000 +define+MEMSIZE=512 \
  -L kronos/rtl kronos/rtl/core/kronos_types.sv \
  -sv  rtl/sim_top.sv

analyze \
  -lib sodorLib -mfcu \
  -y sodor_verilog +libext+.sv +define+SYNTHESIS \
  -sv sodor_verilog/Vincent_SodorInternalTile.sv
analyze -sv correctness.v

elaborate -top cfg -disable_auto_bbox
clock clk
reset rst -non_resettable_regs 0 -init_state KRONOS.init 

set_prove_per_property_max_time_limit 360s
set_prove_time_limit 3600s

abstract -init_value { \
    sodor_core.memory.mem_ext.Memory[32]
    kronos.dut.mem[32]
    sodor_core.memory.mem_ext.Memory[33]
    kronos.dut.mem[33]
   sodor_core.memory.mem_ext.Memory[34]
    kronos.dut.mem[34]
}

assume {same_mem}
assume {same_init_mem}
assume {valid_inst}


#assume { @(posedge clk) disable iff (rst) ##15 correct } -name invariant_assumption

# ADD ASSUMPTIONS START (DO NOT EDIT)
# ADD ASSUMPTIONS END

# ADD ASSERTION START (DO NOT EDIT)
# ADD ASSERTION END


# Bounded proof extended for 15 cycles, see page 750 of JasperGold apps Command Reference Manual
# set_trace_extension 15
#Vincent: set_trace_extension 2 means we extend for at least 2 cycles
#I think we actually need the min_lenght property to be set, but I don't know how to do that
#To see why this is needed, consider the following assertion:
# assert {!(sodor_core.my_commit_pc[31:0] == 32'b00000000000000000000000000000000)}
#Trivially, a single sh x0, 1056(x8) makes sodor stall, the commit pc goes to zero
#and this assertion is violated
#But, the resulting trace is only one clock period long
#Therefore, the following line will fail:
#  File "/home/viniul/formal/cex-generator/formal-verif/invariant_generation/vincent_invariant_generator/vcd_trace.py", line 16, in __init__
#    self.clk_freq = self.vcd[clkPath].tv[2][0]- self.vcd[clkPath].tv[0][0]
#IndexError: list index out of range
#I don't know how else to address this issue
#Even with longer periods, the check does not work, because correctness is only checked after 5 
#ibex commits -- therefore, if jaspergold finds a counterexample that is below 15 cycles
#we never check correctness.
#This creates a mismatch between jaspergold and verilator results
prove -all -asserts -dump_trace -dump_trace_type vcd -dump_trace_dir jg_vcd_out 

# GARBAGE

#assume { !branching }
#assume { !sodor_core.core.c.data_misaligned }
#assume { !sodor_core.core.d.reg_dmiss }
#assume { !(sodor_core.core.c.io_ctl_csr_cmd >= 3'b100) }
#assume { ibex.dut.cpu.cpu.u_ibex_core.id_stage_i.instr_rdata_i[6:0] != 7'b0001111 }
#assume { ibex.dut.cpu.cpu.u_ibex_core.cs_registers_i.csr_rdata_int == 32'h0 }

# STEP: Correct
#assert {correct}

#assert {counter <= 60}
#prove { ! (TOP.correctness.ibex.dut.cpu.cpu.u_ibex_core.load_store_unit_i.addr_incr_req_o )} 
#assert {! (commit_cycle_nums_ibex[13][31:0] == 32'd19) }

# STEP: Prove
#assert { @(posedge clk) disable iff (rst) ##21 correct }

#assert { @(posedge clk) disable iff (!rst) (ibex.dut.cpu.cpu.u_ibex_core.load_store_unit_i.addr_incr_req_o == 1'b0) } -label inv_check -name inv_check
