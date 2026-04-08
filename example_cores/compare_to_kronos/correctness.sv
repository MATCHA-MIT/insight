`ifndef VERILATOR
config cfg;
    design correctness;
    instance correctness.sodor_core liblist sodorLib;
    instance correctness.kronos liblist uArchLib;
endconfig
`endif

`define CUSTOM_DEFINES
`define CHECKER_DATA_WIDTH 32
`define CHECKER_TARGET_WIDTH 5
`define CHECKER_PC_WIDTH 32
//Max number of instructions to trace for checking
//We check 2 concrete, and then another 4 instructions abstract, so in total max 7
`define CHECKER_MaxNumTracedInstructions 32'd6
`define CHECKER_MAX_EXECUTION_WINDOW 32'd11

`ifndef DEBUG
`define DEBUG 0
`endif

`include "../common/testbench/correctness_inner_pkg.sv"


module correctness(
    input clk,
    input rst,
    output logic correct,
    output logic next_correct,
    output logic [31:0] mismatch_index,
    output logic [31:0] mismatch_cycle_ref_core,
    output logic [31:0] mismatch_cycle_dut_core,
    output logic done
);
    reg stall_kronos;
    reg stall_sodor;
    commit_info_t dut_commit;
    commit_info_t ref_commit;
    reg init;
    // next_pc for the DUT commit:
    //  - activate_trap is a registered signal that only fires during the TRAP state,
    //    so it won't be spuriously true when a normal instruction retires.
    //  - u_ex.next_pc is a register captured alongside instret/ret_pc, so it holds
    //    the correct value for the retiring instruction.
    wire kronos_activate_trap = kronos.dut.cpu.cpu.u_ex.activate_trap;
    wire [31:0] kronos_mtvec_addr = {kronos.dut.cpu.cpu.u_ex.u_csr.mtvec.base, 2'b00};
    wire [31:0] kronos_next_pc = kronos_activate_trap ? kronos_mtvec_addr
                                                      : kronos.dut.cpu.cpu.u_ex.next_pc;

    initial begin 
        init = 1'b1;
        stall_kronos = 1'b0;
        stall_sodor = 1'b0;
        correct = 1'b1;
        done = 1'b0;
    end

    // Glitch-free clock gating for both cores.
    // Sample stall on posedge first, then update gate-enable on negedge.
    // This avoids same-edge races between counter updates and clock gating.
    logic stall_kronos_req_q;
    logic stall_sodor_req_q;
    logic en_lat_kronos = 1'b1;
    logic en_lat_sodor = 1'b1;
    wire gclk_kronos;
    wire gclk_sodor;

    always_ff @(posedge clk or posedge rst) begin
        if (rst) begin
            stall_kronos_req_q <= 1'b0;
            stall_sodor_req_q <= 1'b0;
        end else begin
            stall_kronos_req_q <= stall_kronos;
            stall_sodor_req_q <= stall_sodor;
        end
    end

    always_ff @(negedge clk or posedge rst) begin
        if (rst) begin
            en_lat_kronos <= 1'b1;
            en_lat_sodor <= 1'b1;
        end else begin
            en_lat_kronos <= ~stall_kronos_req_q;
            en_lat_sodor <= ~stall_sodor_req_q;
        end
    end

    assign gclk_kronos = clk & en_lat_kronos;
    assign gclk_sodor = clk & en_lat_sodor;

    correctness_inner correctness_inst (
        .clk(clk),
        .rst(rst),
        .ref_commit(ref_commit),
        .dut_commit(dut_commit),
        .correct(correct),
        .next_correct(next_correct),
        .mismatch_index(mismatch_index),
        .mismatch_cycle_ref_core(mismatch_cycle_ref_core),
        .mismatch_cycle_dut_core(mismatch_cycle_dut_core),
        .dut_stalled_out(stall_kronos),
        .ref_stalled_out(stall_sodor),
        .done_all(done)
    );

    sim_top kronos (
        .clk(gclk_kronos),
        .rst(rst)
    );
    Sodor_SodorInternalTile sodor_core(.clock(gclk_sodor), .reset(rst),
        .io_debug_port_req_valid(1'h0),
        .io_debug_port_req_bits_addr(32'h0),
        .io_debug_port_req_bits_data(32'h0),
        .io_debug_port_req_bits_fcn(1'h0),
        .io_debug_port_req_bits_typ(3'h0),
        //.io_master_port_0_req_valid(1'h0),
        .io_master_port_0_resp_valid(1'h0),
        //.io_master_port_1_req_valid(1'h0),
        .io_master_port_0_resp_bits_data(32'h0),
        .io_master_port_1_resp_valid(1'h0),
        .io_master_port_1_resp_bits_data(32'h0),
        .io_interrupt_debug(1'h0),
        .io_interrupt_mtip(1'h0),
        .io_interrupt_msip(1'h0),
        .io_interrupt_meip(1'h0),
        .io_hartid(1'h0),
        .io_debug_port_resp_valid()
        //.io_reset_vector(32'h80000000)
    );

    assign ref_commit.commit_data = sodor_core.my_commit_data;
    assign ref_commit.commit_target = sodor_core.my_commit_target;
    assign ref_commit.commit_pc = sodor_core.my_commit_pc;
    assign ref_commit.commit_valid = sodor_core.core.c.my_commit_valid;
    assign ref_commit.commit_next_pc = sodor_core.my_commit_pc_next;
    assign ref_commit.commit_rs1 = sodor_core.core.d.rs1_data;
    assign ref_commit.commit_rs2 = sodor_core.core.d.rs2_data;
    assign ref_commit.commit_instr = sodor_core.core.d.this_instruction_inst;
    assign ref_commit.commit_exception_code = 32'h0;//sodor_core.core.c.io_ctl_exception
        // ? sodor_core.core.c.io_ctl_exception_cause
        // : 32'h0;

    assign dut_commit.commit_data = kronos.dut.cpu.cpu.u_ex.regwr_data;
    assign dut_commit.commit_target = kronos.dut.cpu.cpu.u_ex.regwr_en ? kronos.dut.cpu.cpu.u_ex.regwr_sel : 5'd0;
    assign dut_commit.commit_pc = kronos.dut.cpu.cpu.u_ex.ret_pc;
    // commit_valid: instret already covers normal retirements AND CSR/system traps
    // (via decode.csr && trap_jump / decode.system && trap_jump in kronos_EX).
    // We only add activate_trap for exceptions that instret doesn't cover:
    // misaligned jumps, illegal instructions, misaligned load/store.
    wire kronos_non_csr_sys_trap = kronos.dut.cpu.cpu.u_ex.activate_trap
                                 && !kronos.dut.cpu.cpu.u_ex.decode.csr
                                 && !kronos.dut.cpu.cpu.u_ex.decode.system;
    assign dut_commit.commit_valid = kronos.dut.cpu.cpu.u_ex.instret || kronos_non_csr_sys_trap;
    assign dut_commit.commit_next_pc = kronos_next_pc;
    assign dut_commit.commit_rs1 = kronos.dut.cpu.cpu.u_ex.ret_rs1_data;
    assign dut_commit.commit_rs2 = kronos.dut.cpu.cpu.u_ex.ret_rs2_data;
    assign dut_commit.commit_instr = kronos.dut.cpu.cpu.u_ex.ret_ir;
    assign dut_commit.commit_exception_code = 32'h0;//kronos.dut.cpu.cpu.u_ex.activate_trap
        // ? kronos.dut.cpu.cpu.u_ex.trap_cause
        // : 32'h0;

    wire debug_en = (`DEBUG != 0) ? 1'b1 : $test$plusargs("debug=1");

    always_ff @(posedge clk) begin
        if (debug_en) begin
            if (dut_commit.commit_valid || ref_commit.commit_valid) begin
                $display("DBG cycle=%0t dut_valid=%0b dut_instr=%h ex.ir=%h id.ir=%h fetch.ir=%h ref_valid=%0b ref_instr=%h",
                         $time,
                         dut_commit.commit_valid,
                         dut_commit.commit_instr,
                         kronos.dut.cpu.cpu.u_ex.decode.ir,
                         kronos.dut.cpu.cpu.u_id.decode.ir,
                         kronos.dut.cpu.cpu.fetch.ir,
                         ref_commit.commit_valid,
                         ref_commit.commit_instr);
            end
        end
        // $display("sodor exception %x",sodor_core.core.c.io_ctl_exception );
    end

    




    // Intruction Fields
    wire [31:0] instruction = sodor_core.core.c.io_dat_inst;
    wire [6:0] opcode = instruction[6:0];
    wire [2:0] funct3 = instruction[14:12];
    wire [6:0] funct7 = instruction[31:25];
    wire [4:0] rs1 = instruction[19:15];
    wire [4:0] rs2 = instruction[24:20];
    wire [4:0] rd = instruction[11:7];
    wire [11:0] imm_i = instruction[31:20];
    wire [11:0] imm_s = {instruction[31:25], instruction[11:7]};
    wire [11:0] imm_b = {instruction[12], instruction[7], instruction[30:25], instruction[11:8]};
    wire [19:0] imm_u = instruction[31:12];
    wire [19:0] imm_j = {instruction[31], instruction[19:12], instruction[20], instruction[30:21]};

    //wire all_zero_fence_i = (instruction[31:20] == 12'b0 && rs1 == '0 && rd =='0);
    //wire all_zero_fence_d = (funct7[6:3] == '0 && rs1 == '0 && rd =='0);

    localparam MaxNumTracedInstructions = `CHECKER_MaxNumTracedInstructions;
    // STEP: Same initial memory
    // Create a vector of comparisons
    wire [MaxNumTracedInstructions-1:0] mem_equal;
    genvar i;
    generate
        for (i = 0; i < MaxNumTracedInstructions; i = i + 1) begin

            assign mem_equal[i] = (sodor_core.memory.mem_ext.Memory[i] == kronos.dut.mem[i]);
            // if (!mem_equal) begin
            //      $display("Memory mismatch at address %0d: sodor_core = %h, kronos = %h", i, sodor_core.memory.mem_ext.Memory[i], kronos.dut.mem[i]);
            // end
        end
    endgenerate
    wire same_mem = &mem_equal;

    wire same_init_mem = init? same_mem: 1'h1;

    wire illegal_instruction = sodor_core.core.c.illegal;
    wire branching = sodor_core.core.c.branching;

    wire valid_inst = sodor_core.core.c.valid_inst;

    always @(posedge clk) begin
        if (rst) begin
            init <= 1'b1; 
        end else begin
            if (init) begin
                init <= 1'b0;
            end
        end
    end
endmodule
