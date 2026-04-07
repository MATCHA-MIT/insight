// -----------------------------------------------------------------------------
// correctness_inner.sv (glitch-free version)
// Sequentialized 'correct' to prevent transient false negatives
// Works in Verilator and JasperGold
// -----------------------------------------------------------------------------

`ifndef CHECKER_DATA_WIDTH
    //`error "CHECKER_DATA_WIDTH is not defined"
`endif

localparam int DATA_WIDTH = `CHECKER_DATA_WIDTH;
localparam int TARGET_WIDTH = `CHECKER_TARGET_WIDTH;
localparam int PC_WIDTH = `CHECKER_PC_WIDTH;
localparam int MaxNumTracedInstructions = `CHECKER_MaxNumTracedInstructions;
localparam int MAX_EXECUTION_WINDOW = `CHECKER_MAX_EXECUTION_WINDOW;

typedef struct packed {
    logic [DATA_WIDTH-1:0] commit_data;
    logic [TARGET_WIDTH-1:0] commit_target;
    logic [PC_WIDTH-1:0] commit_pc;
    logic [31:0] commit_cycle;
    logic [PC_WIDTH-1:0] commit_next_pc;
    logic committed;
    logic [DATA_WIDTH-1:0] commit_rs1;
    logic [DATA_WIDTH-1:0] commit_rs2;
    logic [31:0] commit_instr;
    logic [DATA_WIDTH:0] commit_exception_code;
} commit_log_t;

typedef struct packed {
    logic [DATA_WIDTH-1:0] commit_data;
    logic [TARGET_WIDTH-1:0] commit_target;
    logic [PC_WIDTH-1:0] commit_pc;
    logic [PC_WIDTH-1:0] commit_next_pc;
    logic commit_valid;
    logic [DATA_WIDTH-1:0] commit_rs1;
    logic [DATA_WIDTH-1:0] commit_rs2;
    logic [31:0] commit_instr;
    logic [DATA_WIDTH:0] commit_exception_code;
} commit_info_t;


// -----------------------------------------------------------------------------

function static void which_fields_used_by_instr(
    input logic [31:0] inst,
    output logic rd_used,
    output logic rs1_used,
    output logic rs2_used
);
    logic [6:0] opcode;
    logic [4:0] rd;
    logic [2:0] funct3;
    logic rd_non_zero;

    opcode = inst[6:0];
    rd = inst[11:7];
    funct3 = inst[14:12];
    rd_non_zero = (rd != 5'd0);

    case (opcode)
        7'h33: begin // R-type
            rd_used = rd_non_zero;
            rs1_used = 1'b1;
            rs2_used = 1'b1;
        end
        7'h13: begin // I-type arithmetic
            rd_used = rd_non_zero;
            rs1_used = 1'b1;
            rs2_used = 1'b0;
        end
        7'h03: begin // Loads
            rd_used = rd_non_zero;
            rs1_used = 1'b1;
            rs2_used = 1'b0;
        end
        7'h23: begin // Stores
            rd_used = 1'b0;
            rs1_used = 1'b1;
            rs2_used = 1'b1;
        end
        7'h63: begin // Branches
            rd_used = 1'b0;
            rs1_used = 1'b1;
            rs2_used = 1'b1;
        end
        7'h6f: begin // JAL
            rd_used = rd_non_zero;
            rs1_used = 1'b0;
            rs2_used = 1'b0;
        end
        7'h67: begin // JALR
            rd_used = rd_non_zero;
            rs1_used = 1'b1;
            rs2_used = 1'b0;
        end
        7'h37: begin // LUI
            rd_used = rd_non_zero;
            rs1_used = 1'b0;
            rs2_used = 1'b0;
        end
        7'h17: begin // AUIPC
            rd_used = rd_non_zero;
            rs1_used = 1'b0;
            rs2_used = 1'b0;
        end
        7'h0f: begin // FENCE
            rd_used = 1'b0;
            rs1_used = 1'b0;
            rs2_used = 1'b0;
        end
        7'h2f: begin // AMO
            rd_used = rd_non_zero;
            rs1_used = 1'b1;
            rs2_used = 1'b1;
        end
        7'h73: begin // SYSTEM
            if (funct3 == 3'b000) begin
                // ECALL/EBREAK
                rd_used = 1'b0;
                rs1_used = 1'b0;
                rs2_used = 1'b0;
            end else begin
                // CSR types
                rd_used = rd_non_zero;
                rs1_used = 1'b1;
                rs2_used = 1'b0;
                if (funct3 == 3'b110 || funct3 == 3'b101 || funct3 == 3'b111) begin
                    // Immediate variants
                    rs1_used = 1'b0;
                end
            end
        end
        default: begin
            rd_used = 1'b0;
            rs1_used = 1'b0;
            rs2_used = 1'b0;
        end
    endcase
endfunction

function static logic compare_commit_log_entry(input commit_log_t ref_commit, input commit_log_t dut);   
    logic rd_used;
    logic rs1_used;
    logic rs2_used;
    if (!ref_commit.committed && !dut.committed) begin
        return 1'b1;
    end
    if (dut.committed && !ref_commit.committed) begin
        return 1'b0;
    end
    if (ref_commit.committed && !dut.committed) begin
        return 1'b1;
    end
    
    if (ref_commit.commit_next_pc != dut.commit_next_pc) begin
        return 1'b0;
    end
    // Decode fields used
    which_fields_used_by_instr(ref_commit.commit_instr, rd_used, rs1_used, rs2_used);

    if (ref_commit.commit_pc != dut.commit_pc) begin
        return 1'b0;
    end

    if (ref_commit.commit_instr != dut.commit_instr) begin
        return 1'b0;
    end

    if (ref_commit.commit_exception_code != dut.commit_exception_code) begin
        return 1'b0;
    end

    if (rd_used) begin
        if (ref_commit.commit_target != dut.commit_target) begin
            return 1'b0;
        end
        if (ref_commit.commit_target != 0) begin
            if (ref_commit.commit_data != dut.commit_data) begin
                // $display("Data mismatch: REF %x DUT %x instr %x", ref_commit.commit_data, dut.commit_data, ref_commit.commit_instr);
                return 1'b0;
            end
        end 
        // else begin 
        // `ifdef CASCADE
        //     // $display("Warning: rd is not used for this instruction (pc %x, instr %x), ignoring data/target mismatches", ref_commit.commit_pc, ref_commit.commit_instr);
        //     return 1'b1; // If rd is not used, we don't care about data/target mismatches - optimization for cascade where we only trace commits with rd != 0
        // `else 
        //     // $display("Warning: rd is zero for this instruction (pc %x, instr %x), but it is marked as used. This may indicate an issue with the decoder logic.", ref_commit.commit_pc, ref_commit.commit_instr);
        // `endif
        // end
    end 
    // else begin 
        // `ifdef CASCADE
        //     // $display("Warning: rd is not used for this instruction (pc %x, instr %x), ignoring data/target mismatches", ref_commit.commit_pc, ref_commit.commit_instr);
        //     // If rd is not used, we don't care about data/target mismatches - optimization for cascade where we only trace commits with rd != 0
        //     if (ref_commit.commit_target == 0) begin
        //         return 1'b1;
        //     end
        // `else 
        //     // $display("Warning: rd is zero for this instruction (pc %x, instr %x), but it is marked as used. This may indicate an issue with the decoder logic.", ref_commit.commit_pc, ref_commit.commit_instr);
        // `endif
    // end
    // `ifndef CASCADE
        if (rs1_used && (ref_commit.commit_rs1 != dut.commit_rs1)) begin
            // $display("RS1 mismatch: REF %x DUT %x instr %x", ref_commit.commit_rs1, dut.commit_rs1, ref_commit.commit_instr);
            return 1'b0;
        end

        if (rs2_used && (ref_commit.commit_rs2 != dut.commit_rs2)) begin
            // $display("RS2 mismatch: REF %x DUT %x instr %x", ref_commit.commit_rs2, dut.commit_rs2, ref_commit.commit_instr);
            return 1'b0;
        end
    // `endif

    return 1'b1;
endfunction

// -----------------------------------------------------------------------------

module correctness_inner(
    input  logic clk,
    input  logic rst,
    input  commit_info_t ref_commit,
    input  commit_info_t dut_commit,
    output logic correct,
    output logic next_correct,
    output logic [31:0] mismatch_index,
    output logic [31:0] mismatch_cycle_ref_core,
    output logic [31:0] mismatch_cycle_dut_core,
    output logic ref_stalled_out,
    output logic dut_stalled_out,
    output logic done_all
);

    commit_log_t commit_log_dut[MaxNumTracedInstructions];
    commit_log_t commit_log_ref[MaxNumTracedInstructions];
    `ifdef VERILATOR
        logic [DATA_WIDTH-1:0] dut_constants[MaxNumTracedInstructions];
        logic [DATA_WIDTH-1:0] ref_constants[MaxNumTracedInstructions];
    `endif
    
    reg [31:0] counter;
    reg [31:0] commit_counter_ref;
    reg [31:0] commit_counter_dut;
    reg [31:0] last_dut_commit_cycle;
    reg [31:0] last_ref_commit_cycle;
    reg [31:0] last_checked_index; // Track what we've already verified
    reg dut_has_committed_once;
    reg ref_has_committed_once;
    logic dut_has_stalled;
    logic ref_has_stalled;
    reg dut_has_stalled_reg;
    reg ref_has_stalled_reg;
    
    wire dut_commit_valid = dut_commit.commit_valid;
    wire ref_commit_valid = ref_commit.commit_valid;
    
    // Stall outputs
    assign ref_stalled_out = ((commit_counter_ref)  >= MaxNumTracedInstructions);
    assign dut_stalled_out = (commit_counter_dut >= MaxNumTracedInstructions);

    // NEW: Combinational helpers to avoid local decls inside always_comb
    logic [31:0] compare_limit;
    assign compare_limit = (commit_counter_dut < commit_counter_ref)
                         ? commit_counter_dut : commit_counter_ref;

    // logic done_all;
    assign done_all = (commit_counter_dut >= MaxNumTracedInstructions) &&
                      (commit_counter_ref >= MaxNumTracedInstructions) && next_correct == 1;

    always_ff @(posedge clk) begin begin
            if (done_all) begin
                `ifndef VERILATOR
                    $finish();
                `endif
            end
        end
    end
    // -------------------------------------------------------------------------
    // Sequential process: track commits and counters
    // -------------------------------------------------------------------------
    always_ff @(posedge clk or posedge rst) begin
        if (rst) begin
            counter               <= 0;
            commit_counter_dut    <= 0;
            commit_counter_ref    <= 0;
            last_dut_commit_cycle <= 0;
            last_ref_commit_cycle <= 0;
            last_checked_index    <= 0;
            dut_has_committed_once <= 0;
            ref_has_committed_once <= 0;

            for (int i = 0; i < MaxNumTracedInstructions; i++) begin
                commit_log_ref[i] <= '{default:0};
                commit_log_dut[i] <= '{default:0};
            end
        end else begin
            counter <= counter + 1;
            //Vincent: I think this might cause problems with verilator
            if (dut_commit.commit_valid && !dut_stalled_out) begin
                commit_log_dut[commit_counter_dut] <= '{
                    commit_data: dut_commit.commit_data,
                    commit_target: dut_commit.commit_target,
                    commit_pc: dut_commit.commit_pc,
                    commit_next_pc: dut_commit.commit_next_pc,
                    commit_cycle: counter,
                    committed: 1'b1,
                    commit_rs1: dut_commit.commit_rs1,
                    commit_rs2: dut_commit.commit_rs2,
                    commit_instr: dut_commit.commit_instr,
                    commit_exception_code: dut_commit.commit_exception_code
                };
                // $display("DUT committed data %x target %d pc %x next_pc %x at cycle %d index %d", 
                //          dut_commit.commit_data, dut_commit.commit_target, 
                //          dut_commit.commit_pc, dut_commit.commit_next_pc, counter, commit_counter_dut);
                commit_counter_dut    <= commit_counter_dut + 1;
                last_dut_commit_cycle <= counter;
                dut_has_committed_once <= 1;
                `ifdef VERILATOR
                    dut_constants[commit_counter_dut] <= dut_commit.commit_data;
                `endif
            end

            if (ref_commit.commit_valid && !ref_stalled_out) begin
                commit_log_ref[commit_counter_ref] <= '{
                    commit_data: ref_commit.commit_data,
                    commit_target: ref_commit.commit_target,
                    commit_pc: ref_commit.commit_pc,
                    commit_next_pc: ref_commit.commit_next_pc,
                    commit_cycle: counter,
                    committed: 1'b1,
                    commit_rs1: ref_commit.commit_rs1,
                    commit_rs2: ref_commit.commit_rs2,
                    commit_instr: ref_commit.commit_instr,
                    commit_exception_code: ref_commit.commit_exception_code
                };
                commit_counter_ref    <= commit_counter_ref + 1;
                last_ref_commit_cycle <= counter;
                ref_has_committed_once <= 1;
                `ifdef VERILATOR
                    ref_constants[commit_counter_ref] <= ref_commit.commit_data;
                `endif
            end

            // Update the verified index
            if (commit_counter_dut > 0 && commit_counter_ref > 0) begin
                automatic int new_checked = 
                    (commit_counter_dut < commit_counter_ref) ?
                    commit_counter_dut : commit_counter_ref;
                if (new_checked > last_checked_index)
                    last_checked_index <= new_checked;
            end
        end
    end

    // -------------------------------------------------------------------------
    // Sequential correctness evaluation (glitch-free)
    // -------------------------------------------------------------------------
    // logic next_correct;
    logic [31:0] next_mismatch_index;
    logic [31:0] next_mismatch_cycle_ref_core;
    logic [31:0] next_mismatch_cycle_dut_core;

    always_comb begin
        next_correct = 1'b1;
        next_mismatch_index = 0;
        next_mismatch_cycle_ref_core = 0;
        next_mismatch_cycle_dut_core = 0;
        ref_has_stalled = 1'b0;
        dut_has_stalled = 1'b0;

        // // 1. Both finished → done
        // if (commit_counter_dut >= MaxNumTracedInstructions &&
        //     commit_counter_ref >= MaxNumTracedInstructions) begin
        //     next_correct = 1'b1;
        //     // $finish();
            
        // end

        // 2. DUT never started
        if (!dut_has_committed_once && ref_has_committed_once &&
                 counter >= MAX_EXECUTION_WINDOW) begin
            next_correct = 1'b0;
            next_mismatch_cycle_ref_core = commit_log_ref[0].commit_cycle;
            next_mismatch_cycle_dut_core = 1;
	    dut_has_stalled = 1;
        end

        // 3. REF never started
        else if (!ref_has_committed_once && dut_has_committed_once &&
                 counter >= MAX_EXECUTION_WINDOW) begin
            next_correct = 1'b0;
            next_mismatch_cycle_ref_core = 0;
            next_mismatch_cycle_dut_core = commit_log_dut[0].commit_cycle;
	    ref_has_stalled = 1;
        end

        // 4. DUT stalled
        else if (dut_has_committed_once &&
                 commit_counter_ref > commit_counter_dut &&
                 counter >= last_dut_commit_cycle + MAX_EXECUTION_WINDOW) begin
            // $display("DUT stalled detected at cycle %d, commit_counter_dut %d, commit_counter_ref %d, last_dut_commit_cycle %d, last_ref_commit_cycle %d", 
                    //  counter, commit_counter_dut, commit_counter_ref, last_dut_commit_cycle, last_ref_commit_cycle);
            next_correct = 1'b0;
            next_mismatch_index = commit_counter_dut;
            next_mismatch_cycle_ref_core = commit_log_ref[commit_counter_dut].commit_cycle;
            next_mismatch_cycle_dut_core = last_dut_commit_cycle + 1;
            dut_has_stalled = 1;
        end

        // 5. REF stalled
        else if (ref_has_committed_once &&
                 commit_counter_dut > commit_counter_ref &&
                 counter >= last_ref_commit_cycle + MAX_EXECUTION_WINDOW) begin
            next_correct = 1'b0;
            next_mismatch_index = commit_counter_ref;
            next_mismatch_cycle_ref_core = last_ref_commit_cycle + 1;
            next_mismatch_cycle_dut_core = commit_log_dut[commit_counter_ref].commit_cycle;
            ref_has_stalled = 1;
        end

        // 6. Functional mismatches (compare logs)
        else if (commit_counter_dut > 0 && commit_counter_ref > 0) begin
            // JasperGold-safe constant-bounded loop
            `ifdef VERILATOR
            for (int i = 0; i < compare_limit; i++) begin
            `else
            for (int i = 0; i < MaxNumTracedInstructions; i++) begin
            `endif
                begin: comparision_block
                    if (i >= compare_limit) begin 
                        disable comparision_block; // Skip remaining iterations - optimization for JG
                    end
                    if (i >= last_checked_index) begin
                        // $display("Checking index %d at cycle %d", i, counter);
                        if (!compare_commit_log_entry(commit_log_ref[i], commit_log_dut[i])) begin
                            // $display("Mismatch detected at index %d: REF(pc=%x, target=%d, data=%x) DUT(pc=%x, target=%d, data=%x) at cycle %x",
                            //          i,
                            //          commit_log_ref[i].commit_pc,
                            //          commit_log_ref[i].commit_target,
                            //          commit_log_ref[i].commit_data,
                            //          commit_log_dut[i].commit_pc,
                            //          commit_log_dut[i].commit_target,
                            //          commit_log_dut[i].commit_data,
                            //          counter);
                            next_correct = 1'b0;
                            next_mismatch_index = i;
                            next_mismatch_cycle_ref_core = commit_log_ref[i].commit_cycle;
                            next_mismatch_cycle_dut_core = commit_log_dut[i].commit_cycle;
                            break; // Early exit on first mismatch
                        end
                    end
                end
            end
        end
    end

    // Sequentially register correctness and mismatch data
    always_ff @(posedge clk or posedge rst) begin
        if (rst) begin
            correct <= 1'b1;
            mismatch_index <= 0;
            mismatch_cycle_ref_core <= 0;
            mismatch_cycle_dut_core <= 0;
            dut_has_stalled_reg <= 1'b0;
            ref_has_stalled_reg <= 1'b0;
        end else begin
            // $display("At cycle %d, next_correct=%b, next_mismatch_index=%d, next_mismatch_cycle_ref_core=%d, next_mismatch_cycle_dut_core=%d",
                    //  counter, next_correct, next_mismatch_index, next_mismatch_cycle_ref_core, next_mismatch_cycle_dut_core);
            correct <= next_correct;
            mismatch_index <= next_mismatch_index;
            mismatch_cycle_ref_core <= next_mismatch_cycle_ref_core;
            mismatch_cycle_dut_core <= next_mismatch_cycle_dut_core;
            dut_has_stalled_reg <= dut_has_stalled;
            ref_has_stalled_reg <= ref_has_stalled;
        end
    end

    always_ff @(posedge clk) begin
        if (!correct) begin
            `ifndef VERILATOR
                $finish();
            `endif
        end
    end

    initial begin
        correct = 1'b1;
	done_all = 1'b0;
    end

    `ifdef VERILATOR
    export "DPI-C" task printCommitLog;

    task automatic printCommitLog();
        string strvar = "";
        logic print_condition_without_keep_going;
        logic print_condition;
        logic keep_going;
        if ($test$plusargs("keep-going")) begin
            keep_going = 1'b1;
        end else begin
            keep_going = 1'b0;
        end
        foreach (commit_log_dut[i]) begin
            print_condition_without_keep_going = (i <= commit_counter_dut+2) && (i <= mismatch_index+2 || correct == 1'b1);
            print_condition = (keep_going ? (i <= commit_counter_dut+2) : print_condition_without_keep_going);
            if (print_condition) begin
                strvar = {strvar,$sformatf("\n,%d %d %x %x %d %x", i, commit_log_dut[i].commit_target, 
                         commit_log_dut[i].commit_data, commit_log_dut[i].commit_pc, 
                         commit_log_dut[i].commit_cycle, commit_log_dut[i].committed)};
            end
        end
        $display("DUT commit log %s", strvar);
        strvar = "";
        foreach (commit_log_ref[i]) begin
            print_condition_without_keep_going = (i <= commit_counter_ref+2) && (i <= mismatch_index+2 || correct == 1'b1);
            print_condition = (keep_going ? (i <= commit_counter_ref+2) : print_condition_without_keep_going);
            if (print_condition) begin
                strvar = {strvar,$sformatf("\n,%d %d %x %x %d %x", i, commit_log_ref[i].commit_target, 
                         commit_log_ref[i].commit_data, commit_log_ref[i].commit_pc, 
                         commit_log_ref[i].commit_cycle, commit_log_ref[i].committed)};
            end
        end
        $display("REF commit log %s", strvar);
        $display("At cycle %d \n", counter);
        $display("REF commit counter %d \n", commit_counter_ref);
        $display("DUT commit counter %d \n", commit_counter_dut);
        $display("Last checked index %d \n", last_checked_index);
        $display("Correct %d \n", correct);
    endtask

    export "DPI-C" task printMismatchInfo;
    task automatic printMismatchInfo();
        $display("Correct %d\n", correct);
        $display("Mismatch at index %d\n", mismatch_index);
        $display("Mismatch cycle ref_core %d\n", mismatch_cycle_ref_core);
        $display("Mismatch cycle dut_core %d\n", mismatch_cycle_dut_core);
        if (!correct && ref_has_stalled_reg) begin
            $display("\n[STALL DETECTED] REF stalled at instruction %d", mismatch_index);
            $display("Last REF commit at cycle %d, current cycle %d", last_ref_commit_cycle, counter);
        end
        if (!correct && dut_has_stalled_reg) begin
            $display("\n[STALL DETECTED] DUT stalled at instruction %d", mismatch_index);
            $display("Last DUT commit at cycle %d, current cycle %d", last_dut_commit_cycle, counter);
        end
    endtask
    `endif
endmodule

// module rv32i_decoder (
//     input  logic [31:0] instr_i,   // Instruction input
//     output logic        inst_valid // Valid instruction flag
// );

//   // Helper: bitmask compare
//   function automatic bit match(
//       input logic [31:0] instr,
//       input logic [31:0] mask,
//       input logic [31:0] value);
//     return ((instr & mask) == value);
//   endfunction

//   always_comb begin
//     inst_valid = 1'b0;

//     if (
//       // Branch instructions (opcode 1100011)
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000001100011) || // BEQ
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100001100011) || // BNE
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010000001100011) || // BLT
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010100001100011) || // BGE
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011000001100011) || // BLTU
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011100001100011) || // BGEU

//       // Jumps
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000001100111) || // JALR
//       match(instr_i, 32'b0000000000000000000000000111111, 32'b0000000000000000000000001101111) || // JAL

//       // Upper immediates
//       match(instr_i, 32'b0000000000000000000000000111111, 32'b0000000000000000000000000110111) || // LUI
//       match(instr_i, 32'b0000000000000000000000000111111, 32'b0000000000000000000000000010111) || // AUIPC

//       // Immediate arithmetic
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000000010011) || // ADDI
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100000010011) || // SLTI
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000110000010011) || // SLTIU
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010000000010011) || // XORI
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011000000010011) || // ORI
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011100000010011) || // ANDI

//       // Shift immediate (RV32)
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000000100000010011) || // SLLI
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000010100000010011) || // SRLI
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0100000000000000010100000010011) || // SRAI

//       // Register arithmetic
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000000000000110011) || // ADD
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0100000000000000000000000110011) || // SUB
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000000100000110011) || // SLL
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000001000000110011) || // SLT
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000001100000110011) || // SLTU
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000010000000110011) || // XOR
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000010100000110011) || // SRL
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0100000000000000010100000110011) || // SRA
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000011000000110011) || // OR
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000011100000110011) || // AND


//             // -----------------------------------------------------
//       // RV32M extension (opcode 0110011, funct7=0000001)
//       // -----------------------------------------------------
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000000000000110011) || // MUL
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000000100000110011) || // MULH
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000001000000110011) || // MULHSU
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000001100000110011) || // MULHU
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000010000000110011) || // DIV
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000010100000110011) || // DIVU
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000011000000110011) || // REM
//       match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000011100000110011) || // REMU

//       // Loads
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000000000011) || // LB
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100000000011) || // LH
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000001000000000011) || // LW
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010000000000011) || // LBU
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010100000000011) || // LHU

//       // Stores
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000000100011) || // SB
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100000100011) || // SH
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000001000000100011) || // SW

//       // Fences
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000000001111) || // FENCE
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100000001111) || // FENCE_I

//       // CSR/System
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100001110011) || // CSRRW
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000001000001110011) || // CSRRS
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000001100001110011) || // CSRRC
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010100001110011) || // CSRRWI
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011000001110011) || // CSRRSI
//       match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011100001110011) || // CSRRCI

//       // System calls and returns
//       (instr_i == 32'b00000000000000000000000001110011) || // ECALL
//       (instr_i == 32'b00000000000100000000000001110011) || // EBREAK
//       (instr_i == 32'b00110000001000000000000001110011) || // MRET
//       //(instr_i == 32'b01111011001000000000000001110011) || // DRET (optional)
//       (instr_i == 32'b00010000010100000000000001110011) || // WFI
//       (instr_i == 32'b00000000000000000000000000000000)    // NULL/NOP
//     )
//       inst_valid = 1'b1;
//   end
// endmodule
function automatic bit match(
    input logic [31:0] instr,
    input logic [31:0] mask,
    input logic [31:0] value);
  return ((instr & mask) == value);
endfunction

function automatic bit is_valid_instr(input logic [31:0] instr_i);
    is_valid_instr = // // Branches
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000001100011) || // BEQ
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100001100011) || // BNE
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010000001100011) || // BLT
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010100001100011) || // BGE
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011000001100011) || // BLTU
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011100001100011) || // BGEU

    // Jumps
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000001100111) || // JALR
    match(instr_i, 32'b0000000000000000000000000111111, 32'b0000000000000000000000001101111) || // JAL

    // Upper immediates
    match(instr_i, 32'b0000000000000000000000000111111, 32'b0000000000000000000000000110111) || // LUI
    match(instr_i, 32'b0000000000000000000000000111111, 32'b0000000000000000000000000010111) || // AUIPC

    // Immediate arithmetic
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000000010011) || // ADDI
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100000010011) || // SLTI
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000110000010011) || // SLTIU
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010000000010011) || // XORI
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011000000010011) || // ORI
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000011100000010011) || // ANDI

    // Shifts
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000000100000010011) || // SLLI
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000010100000010011) || // SRLI
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0100000000000000010100000010011) || // SRAI

    // Register arithmetic
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000000000000110011) || // ADD
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0100000000000000000000000110011) || // SUB
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000000100000110011) || // SLL
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000001000000110011) || // SLT
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000001100000110011) || // SLTU
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000010000000110011) || // XOR
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000010100000110011) || // SRL
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0100000000000000010100000110011) || // SRA
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000011000000110011) || // OR
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000000000000000011100000110011) || // AND

    // Multiply/Divide (RV32M)
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000000000000110011) || // MUL
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000000100000110011) || // MULH
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000001000000110011) || // MULHSU
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000001100000110011) || // MULHU
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000010000000110011) || // DIV
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000010100000110011) || // DIVU
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000011000000110011) || // REM
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000011100000110011) || // REMU

    // Multiply/Divide W (RV64M)
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000000000000111011) || // MULW
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000010000000111011) || // DIVW
    match(instr_i, 32'b1111111000000000011100000111111, 32'b0000001000000000010100000111011) || // DIVUW

    // Loads
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000000000011) || // LB
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100000000011) || // LH
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000001000000000011) || // LW
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010000000000011) || // LBU
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000010100000000011) || // LHU

    // Stores
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000000000100011) || // SB
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100000100011) || // SH
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000001000000100011) || // SW

    // CSR/System
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000000100001110011) || // CSRRW
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000001000001110011) || // CSRRS
    match(instr_i, 32'b0000000000000000011100000111111, 32'b0000000000000000001100001110011) || // CSRRC

    // System calls
    (instr_i == 32'b00000000000000000000000001110011) || // ECALL
    (instr_i == 32'b00000000000100000000000001110011) || // EBREAK
    (instr_i == 32'b00110000001000000000000001110011) || // MRET
    (instr_i == 32'b00010000010100000000000001110011) || // WFI
    (instr_i == 32'b00000000000000000000000000000000);   // NOP
endfunction
