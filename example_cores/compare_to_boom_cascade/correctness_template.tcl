
# STEP: Input design
set_automatic_library_search on
analyze \
  -sv -lib uArchLib -mfcu \
  -y verilog_externalmem_original \
  +libext+.v+.sv +define+SYNTHESIS \
  -y rtl/ \
  -v verilog_externalmem_original/chipyard.harness.TestHarness.SmallBoomV3Config.top.mems.v \
  -v rtl/TileLink_Memory.sv \
  -sv rtl/sim_top.sv 
#  -bbox_m FpPipeline 
#  -bbox_m FPUUnit* \
#  -bbox_m FPToInt* \
#  -bbox_m IntToFP* \
#  -bbox_m DivSqrtRecF64_*
#   -bbox_i boom.dut.BoomTile.core.FpPipeline* \
#  -bbox_m FPUExeUnit*

analyze \
  -lib sodorLib -mfcu \
  -y sodor_verilog +libext+.sv +define+SYNTHESIS \
  -sv sodor_verilog/Sodor_SodorInternalTile.sv
analyze -sv correctness.sv

elaborate -top cfg -disable_auto_bbox
#-disable_auto_bbox
  #-bbox_i boom.dut.BoomTile.core.alu_exe_unit.DivUnit 
#  -bbox_i boom.dut.BoomTile.core.alu_exe_unit.PipelinedMulUnit 
  # -bbox_i boom.dut.BoomTile.core.FpPipeline 
  # -bbox_i boom.dut.BoomTile.core.alu_exe_unit.IntToFPUnit \
  # -bbox_i boom.dut.BoomTile.core.alu_exe_unit.PipelinedMulUnit \
  # -bbox_i boom.dut.BoomTile.core.alu_exe_unit.DivUnit \
  # -bbox_i boom.dut.BoomTile.core.fp_rename_stage 

 #-bbox_i boom.dut.BoomTile.core.fp_rename_stage \
 
          #-bbox_i boom.dut.BoomTile.core.FpPipeline
#Other modules to be black boxed go here.
clock clk
reset rst -non_resettable_regs 0 -init_state BOOM.init 

# get_design_info -list multiplier
# get_design_info -list divider
# get_design_info -list modulus


set_prove_per_property_max_time_limit {TIME_LIMIT}s

#ADD MEMORY ABSTRACTION START (DO NOT EDIT)
# ADD MEMORY ABSTRACTION END

# STEP: Simplification
#assume {simplification}
# ADD ASSUMPTIONS START (DO NOT EDIT)
# ADD ASSUMPTIONS END

#set RV32I_OPCODE_SET_DIVIDER "7'b0110011"
set RV32I_OPCODE_TEST_OPCODE1 "{TESTED_OPCODE_NO1}"
set RV32I_OPCODE_TEST_OPCODE2 "{TESTED_OPCODE_NO2}"
#set RV32I_OPCODE_TEST_OPCODE "7'b1110011"
set_max_trace_length {MAX_TRACE_LENGTH}

set RV32I_OPCODE_SET "7'b0110011, 7'b0010011, 7'b1100011, \
                      7'b0000011, 7'b0100011, 7'b1101111, \
                      7'b1100111, 7'b0110111, 7'b0010111, \
                      7'b0001111, 7'b1110011"

	
set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][6:0] inside {%s}}} 0 $RV32I_OPCODE_TEST_OPCODE1]
eval $cmd

set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][38:32] inside {%s}}} 0 $RV32I_OPCODE_TEST_OPCODE2]
eval $cmd
#set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][6:0] inside {%s}}} 0 $RV32I_OPCODE_TEST_OPCODE]
#eval $cmd

#set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][38:32] inside {%s}}} 0 $RV32I_OPCODE_TEST_OPCODE]
#eval $cmd



for {set i 1} {$i < 4} {incr i} {
    set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][6:0] inside {%s}}} $i $RV32I_OPCODE_SET]
    eval $cmd

    set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][38:32] inside {%s}}} $i $RV32I_OPCODE_SET]
    eval $cmd
}


# -------------------------------
# Config: paths and bit slices
# -------------------------------
# Adjust these if your memory layout packs fields differently.
set MEM_PATH  "sodor_core.memory.mem_ext.Memory"
set OPC_MSB   6
set OPC_LSB   0
set F3_MSB    14
set F3_LSB    12
set F7_MSB    31
set F7_LSB    25

# Number of instruction words to constrain
set NUM_INSNS 4

# Helper to slice: returns "path[idx][msb:lsb]"
proc SLICE {path idx msb lsb} {
    return "${path}\[$idx\]\[$msb:$lsb\]"
}

# -------------------------------
# Opcode → allowed funct3/funct7
# (values use Verilog literal syntax)
# -------------------------------
# Only add fields that should be constrained for a given opcode.
# If a field doesn't apply (e.g., JAL), omit it.
array set OPC_RULES {
    "7'b0110011" { ;# R-type
        funct3 {3'b000 3'b001 3'b010 3'b011 3'b100 3'b101 3'b110 3'b111}
        funct7 {7'b0000000 7'b0100000 7'b0000001} ;# base + SUB/SRA + M-ext
    }
    "7'b0010011" { ;# I-type ALU (ADDI/SLTI/.., plus shifts)
        funct3 {3'b000 3'b001 3'b010 3'b011 3'b100 3'b101 3'b110 3'b111}
        ;# SLLI/SRLI/SRAI additionally restrict funct7, but we keep it broad here.
    }
    "7'b1100011" { ;# branches: BEQ,BNE,BLT,BGE,BLTU,BGEU
        funct3 {3'b000 3'b001 3'b100 3'b101 3'b110 3'b111}
    }
    "7'b0000011" { ;# loads: LB,LH,LW,LD,LBU,LHU,LWU
        funct3 {3'b000 3'b001 3'b010 3'b011 3'b100 3'b101 3'b110}
    }
    "7'b0100011" { ;# stores: SB,SH,SW,SD
        funct3 {3'b000 3'b001 3'b010 3'b011}
    }
    "7'b1100111" { ;# JALR
        funct3 {3'b000}
    }
    "7'b0001111" { ;# FENCE / FENCE.I
        funct3 {3'b000 3'b001}
    }
    "7'b1110011" { ;# SYSTEM/CSR: CSRRW/CSRRS/CSRRC/imm forms
        funct3 {3'b001 3'b010 3'b011 3'b101 3'b110 3'b111 3'b000}
    }
    "7'b0110111" { ;# LUI  (no funct3/funct7 constraints)
        {}
    }
    "7'b0010111" { ;# AUIPC (no funct3/funct7 constraints)
        {}
    }
    "7'b1101111" { ;# JAL   (no funct3/funct7 constraints)
        {}
    }
}

# -------------------------------
# Emit constraints
# -------------------------------
for {set i 0} {$i < $NUM_INSNS} {incr i} {
    set opc   [SLICE $MEM_PATH $i $OPC_MSB $OPC_LSB]
    set f3    [SLICE $MEM_PATH $i $F3_MSB  $F3_LSB]
    set f7    [SLICE $MEM_PATH $i $F7_MSB  $F7_LSB]

    # For each opcode rule, assert:
    #   (opcode == k) -> (funct3 in S) [&& (funct7 in T)]
    foreach k [array names OPC_RULES] {
        set rule $OPC_RULES($k)

        # Build the RHS (constraints) if present
        set conjuncts {}

        # funct3 set
        set pos [lsearch -exact $rule funct3]
        if {$pos != -1} {
            set f3set [lindex $rule [expr {$pos + 1}]]
            if {[llength $f3set] > 0} {
                lappend conjuncts "$f3 inside \{[join $f3set ","]\}"
            }
        }

        # funct7 set
        set pos [lsearch -exact $rule funct7]
        if {$pos != -1} {
            set f7set [lindex $rule [expr {$pos + 1}]]
            if {[llength $f7set] > 0} {
                lappend conjuncts "$f7 inside \{[join $f7set ","]\}"
            }
        }

        # If there is at least one constraint to bind to this opcode, emit implication
        if {[llength $conjuncts] > 0} {
            set rhs [join $conjuncts " && "]
            # Use an implication; adjust operator to match your tool’s SVA parser if needed (e.g., -> or |->).
            set prop "assume { !($opc == $k) || ($rhs) }"
            eval $prop
        }
    }
}

# STEP: Same initial memory
assume {same_mem}
#[TODO 07/13/2025 VU] same_init_mem did not work, because init variable was not set. Not sure if it works now.
assume {same_init_mem}
#assume {all_mem_inst_valid}
# assume {all_mem_inst_valid}
#assume {non_illegal_instruction}
# assume {!branching}
assume {valid_inst}
# STEP: Correct
#assert {correct}
#assert {counter <= 60}
#assume {correctness_inst.counter <= 40}
# STEP: Prove
assert { @(posedge clk) disable iff (rst) correct } -name correct_check
# check_assumptions -conflict
#set_trace_optimization standard
#visualize -property :noConflict -new_window -violation
#visualize -save -vcd jg_vcd_out/no_conflict.vcd -force
#-property :noConflict -violation
#assume -enable {*}[get_property_list -include {name *correctness_p_mem_instr_valid*}]

prove -property {correct_check} -asserts -dump_trace -dump_trace_type vcd -dump_trace_dir jg_vcd_out 
#prove -all -dump_trace -dump_trace_type vcd -dump_trace_dir jg_vcd_out 
#-cex_limit 1
