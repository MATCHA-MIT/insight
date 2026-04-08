# STEP: Input design
set_automatic_library_search on
analyze \
  -sv -lib uArchLib -mfcu \
  -y kronos_no_missing_csr/rtl/core -y rtl \
  +libext+.sv \
  +incdir+kronos_no_missing_csr/rtl/core \
  +define+SYNTHESIS +define+TIMER_ADDR=32'hcc000000 +define+MEMSIZE=128 \
  -L kronos_no_missing_csr/rtl kronos_no_missing_csr/rtl/core/kronos_types.sv \
  -sv  rtl/sim_top.sv

analyze \
  -lib sodorLib -mfcu \
  -y sodor_verilog +libext+.sv +define+SYNTHESIS \
  -sv sodor_verilog/Sodor_SodorInternalTile.sv
analyze -sv correctness.sv
# +define+CHECKER_DATA_WIDTH=32 +define+CHECKER_TARGET_WIDTH=6 +define+CHECKER_PC_WIDTH=32 
#+define+CHECKER_MaxNumTracedInstructions=10 +define+CHECKER_MAX_EXECUTION_WINDOW=10 
elaborate -top cfg -disable_auto_bbox
clock -both_edges clk
reset rst -non_resettable_regs 0 -init_state KRONOS.init 

set_prove_per_property_max_time_limit {TIME_LIMIT}s
set_prove_time_limit {TIME_LIMIT}s


set RV32I_OPCODE_TEST_OPCODE1 "{TESTED_OPCODE_NO1}"
set RV32I_OPCODE_TEST_OPCODE2 "{TESTED_OPCODE_NO2}"
#set RV32I_OPCODE_TEST_OPCODE "7'b1110011"

set RV32I_OPCODE_SET "7'b0110011, 7'b0010011, 7'b1100011, \
                      7'b0000011, 7'b0100011, 7'b1101111, \
                      7'b1100111, 7'b0110111, 7'b0010111, \
                      7'b0001111, 7'b1110011"

	
set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][6:0] inside {%s}}} 0 $RV32I_OPCODE_TEST_OPCODE1]
eval $cmd

set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][6:0] inside {%s}}} 1 $RV32I_OPCODE_TEST_OPCODE2]
eval $cmd
#set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][6:0] inside {%s}}} 0 $RV32I_OPCODE_TEST_OPCODE]
#eval $cmd

#set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][38:32] inside {%s}}} 0 $RV32I_OPCODE_TEST_OPCODE]
#eval $cmd



for {set i 2} {$i < 5} {incr i} {
    set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][6:0] inside {%s}}} $i $RV32I_OPCODE_SET]
    eval $cmd

    #set cmd [format {assume {sodor_core.memory.mem_ext.Memory[%d][38:32] inside {%s}}} $i $RV32I_OPCODE_SET]
    #eval $cmd
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

#ADD MEMORY ABSTRACTION START (DO NOT EDIT)
# ADD MEMORY ABSTRACTION END

#abstract -init_value { \
#    sodor_core.memory.mem_ext.Memory[34]
#    kronos.dut.mem[34]
#    sodor_core.memory.mem_ext.Memory[35]
#    kronos.dut.mem[35]
#    sodor_core.memory.mem_ext.Memory[36]
#    kronos.dut.mem[36]
#}
   #   sodor_core.memory.mem_ext.Memory[35]
   #kronos.dut.mem[35]
#}

#    sodor_core.memory.mem_ext.Memory[34]
#    kronos.dut.mem[34]
# STEP: Same initial memory
assume {same_mem}
assume {same_init_mem}
assume { valid_inst }

# ADD ASSUMPTIONS START (DO NOT EDIT)
# ADD ASSUMPTIONS END

#The below assertion does not work, I get a 40 cycle long proof which assumption:
#assume {!(sodor_core.my_commit_pc[31:0] == 32'b00000000000000000000000000000000)}
#in which the sodor counter is overwritten
#assert { @(posedge clk) disable iff (rst) ##15 correct } -name correct_check 
#assert { @(posedge clk) disable iff (rst) (counter == 0) |-> ##[18:18] correct } -name correct_check 
assert { @(posedge clk) disable iff (rst) correct } -name correct_check 
#set_prove_target_bound 16
#set_max_trace_length 16

set_trace_optimization -irrelevant_value_computation true
# set_tag_irrelevant_values true
# Print whole design
#set_trace_optimization standard
set_max_trace_length 35

prove -property {correct_check} -dump_trace -dump_trace_type vcd -dump_trace_dir jg_vcd_out 
# -run -auto
