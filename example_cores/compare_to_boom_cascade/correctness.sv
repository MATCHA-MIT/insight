
// NOTE: JasperGold Does not support this...,
//       but you can write similar things in .tcl
// library ISALib ../../boomISA/verilog_original/*;
// library boomLib ../verilog_original/*;

`define BOOM_PC_OFFSET 64'h0000000080000080
`define SODOR_PC_OFFSET 64'h0000000000010000


`ifndef VERILATOR
config cfg;
    design correctness;
    instance correctness.sodor_core liblist sodorLib;
    instance correctness.boom liblist uArchLib;
endconfig
`endif

`define CUSTOM_DEFINES
`define CHECKER_DATA_WIDTH 64
`define CHECKER_TARGET_WIDTH 6
`define CHECKER_PC_WIDTH 64
`define CHECKER_MaxNumTracedInstructions 2048
`define CHECKER_MAX_EXECUTION_WINDOW 32'd130
//if we divide by zero that is how long we need...

`include "../common/testbench/correctness_inner_pkg.sv"


module correctness(
    input clk,
    input rst,
    output logic correct,
    output logic [31:0] mismatch_index,
    output logic [31:0] mismatch_cycle_ref_core,
    output logic [31:0] mismatch_cycle_dut_core,
    output logic done
); 
    reg stall_boom;
    reg stall_sodor;
    commit_info_t dut_commit;
    commit_info_t ref_commit;
    reg init; 
    // reg done;
    
    initial begin
	init = 1'b1;
	stall_boom = 1'b0;
	stall_sodor = 1'b0;
    correct = 1'b1;
    done = 1'b0;
    end

    correctness_inner correctness_inst (  
        .clk(clk),
        .rst(rst),
        .ref_commit(ref_commit),
        .dut_commit(dut_commit),
        .correct(correct),
        .mismatch_index(mismatch_index),
        .mismatch_cycle_ref_core(mismatch_cycle_ref_core),
        .mismatch_cycle_dut_core(mismatch_cycle_dut_core),
        .dut_stalled_out(stall_boom),
        .ref_stalled_out(stall_sodor),
        .done_all(done)
    ); 

    sim_top boom (
        .clk(stall_boom? 1'h0: clk),
        .rst(rst)
    ); 
    Sodor_SodorInternalTile sodor_core(.clock(stall_sodor? 1'h0: clk), .reset(rst),
        .io_debug_port_req_valid(1'h0),
        .io_debug_port_req_bits_addr(64'h0),
        .io_debug_port_req_bits_data(64'h0),
        .io_debug_port_req_bits_fcn(1'h0),
        .io_debug_port_req_bits_typ(3'h0),
        //.io_master_port_0_req_valid(1'h0),
        .io_master_port_0_resp_valid(1'h0),
        //.io_master_port_1_req_valid(1'h0),
        .io_master_port_0_resp_bits_data(64'h0),
        .io_master_port_1_resp_valid(1'h0),
        .io_master_port_1_resp_bits_data(64'h0),
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
    assign ref_commit.commit_next_pc = 64'h0; // TODO: add next pc to Sodor commit_info_t
   
    assign dut_commit.commit_data = boom.dut.BoomTile.core.rob.my_commit_data;
    assign dut_commit.commit_target = boom.dut.BoomTile.core.rob.my_commit_target;
    assign dut_commit.commit_pc = boom.dut.BoomTile.core.rob.my_commit_PC;
    assign dut_commit.commit_valid = boom.dut.BoomTile.core.rob.my_commit_valid;
    assign dut_commit.commit_next_pc = 64'h0; // TODO: add next pc to BOOM commit_info_t
    `ifdef VERILATOR
        assign dut_commit.commit_rs1 = boom.dut.BoomTile.core.rob.my_commit_rs1;
        assign dut_commit.commit_rs2 = boom.dut.BoomTile.core.rob.my_commit_rs2;
        assign dut_commit.commit_instr = boom.dut.BoomTile.core.rob.my_commit_inst;
        assign dut_commit.commit_exception_code = boom.dut.BoomTile.core.rob.my_commit_exception;
        assign ref_commit.commit_rs1 = sodor_core.my_commit_rs1;
        assign ref_commit.commit_rs2 = sodor_core.my_commit_rs2;
        assign ref_commit.commit_instr = sodor_core.my_commit_inst;
        assign ref_commit.commit_exception_code = sodor_core.my_commit_exception;
    `endif

    // Instruction Fields
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

    wire all_zero_fence_i = (instruction[31:20] == 12'b0 && rs1 == '0 && rd =='0);
    wire all_zero_fence_d = (funct7[6:3] == '0 && rs1 == '0 && rd =='0);

// STEP: Same initial memory
    wire same_mem =
        sodor_core.memory.mem_ext.Memory[0] == boom.dut.TileLink_Memory.mem[0] &&
        sodor_core.memory.mem_ext.Memory[1] == boom.dut.TileLink_Memory.mem[1] &&
        sodor_core.memory.mem_ext.Memory[2] == boom.dut.TileLink_Memory.mem[2] &&
        sodor_core.memory.mem_ext.Memory[3] == boom.dut.TileLink_Memory.mem[3] &&
        sodor_core.memory.mem_ext.Memory[4] == boom.dut.TileLink_Memory.mem[4];

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

            if (boom.dut.BoomTile.core.rob.my_commit_valid) begin
                // $display("BOOM committed data %x target %d pc %x", 
                //        dut_commit.commit_data, dut_commit.commit_target, dut_commit.commit_pc);
                // $display("Commited instruction %x", 
                //        boom.dut.BoomTile.core.rob.my_commit_inst);
            end
        end
    end



    // genvar i;
    // generate
    // for (i = 0; i < 4; i++) begin : mem_check
    //     property correctness_p_mem_instr_valid_low;
    //     @(posedge clk) disable iff(!rst)
    //         is_valid_instr(sodor_core.memory.mem_ext.Memory[i][31:0]);
    //     endproperty
    //     assume_correctness_p_mem_instr_valid_low: assume property (correctness_p_mem_instr_valid_low);
    //     property correctness_p_mem_instr_valid_high;
    //     @(posedge clk) disable iff(!rst)
    //         is_valid_instr(sodor_core.memory.mem_ext.Memory[i][63:32]);
    //     endproperty
    //     assume_correctness_p_mem_instr_valid_high: assume property (correctness_p_mem_instr_valid_high);
    //     // else $error("Invalid instruction detected in memory[%0d]", i);
    // end
    // endgenerate

    // wire [7:0] mem_inst_valid;
    // genvar i;
    // generate
    //     for (i = 0; i < 4; i = i + 1) begin
    //         rv32i_decoder decoder_inst (
    //             .instr_i(sodor_core.memory.mem_ext.Memory[i][31:0]),
    //             .inst_valid(mem_inst_valid[i*2])
    //         );
    //          rv32i_decoder decoder_inst_high (
    //             .instr_i(sodor_core.memory.mem_ext.Memory[i][63:32]),
    //             .inst_valid(mem_inst_valid[i*2+1])
    //         );

    //     end
    // endgenerate
    // wire all_mem_inst_valid = &mem_inst_valid;

    // ======================================================================
    // SIMULATION INITIALIZATION BLOCKS
    // ======================================================================
    initial begin
        // ------------------------
        // BOOM Core Initialization
        // ------------------------
        boom.dut.BoomTile.core.csr.reg_mtvec        = 32'h80000000;
        boom.dut.BoomTile.core.csr.reg_mepc         = 40'h80000000;

        // PMP entry 0 configured to allow 128-byte region starting at 0x00010000
        // boom.dut.BoomTile.core.csr.reg_pmp_0_addr = 30'h0000400F; // NAPOT, 128B window
        // boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_a = 2'b11;        // NAPOT mode
        // boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_l = 1'b0;         // Unlocked
        // boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_r = 1'b1;         // Read allowed
        // boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_w = 1'b1;         // Write allowed
        // boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_x = 1'b1;         // Execute allowed

        // Other MSTATUS and machine CSRs remain reset
        boom.dut.BoomTile.core.csr.reg_mstatus_mie  = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mstatus_mpie = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mstatus_mpp  = 2'b11;
        boom.dut.BoomTile.core.csr.reg_mstatus_prv  = 2'b11;
        boom.dut.BoomTile.core.csr.reg_mstatus_mprv = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mstatus_mpv  = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mstatus_gva  = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mstatus_v    = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mstatus_sum  = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mstatus_tvm  = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mstatus_tw   = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mstatus_tsr  = 1'b0;
        boom.dut.BoomTile.core.csr.reg_mcause       = 64'h0;
        boom.dut.BoomTile.core.csr.reg_mie          = 64'h0;

        // boom.dut.BoomTile.frontend.s0_vpc[39:0] = 40'h0001_0000;
        // boom.dut.BoomTile.frontend.s0_valid = 1'b1;

        // ------------------------
        // Sodor Core Initialization
        // ------------------------
        sodor_core.core.d.csr.reg_mtvec             = 29'h80000000;
        sodor_core.core.d.csr.reg_mepc              = 31'h80000000;

        sodor_core.core.d.csr.reg_mstatus_mie       = 1'b0;
        sodor_core.core.d.csr.reg_mstatus_mpie      = 1'b0;
        sodor_core.core.d.csr.reg_mstatus_mpp       = 2'b11; // Machine mode
        sodor_core.core.d.csr.reg_mstatus_mpv       = 1'b0;
        sodor_core.core.d.csr.reg_mstatus_gva       = 1'b0;
        sodor_core.core.d.csr.reg_mstatus_v         = 1'b0;

//         boom.dut.BoomTile.core.csr.reg_pmp_0_addr
// 32'h0000400F

// boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_a
// 2'b11

// boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_l
// 1'b0

// boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_r
// 1'b1

// boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_w
// 1'b1

// boom.dut.BoomTile.core.csr.reg_pmp_0_cfg_x
// 1'b1

        // correctness.boom.dut.BoomTile.frontend.tlb._sectored_entries_0_0_data_T[41:0] = 42'h0;
        // correctness.boom.dut.BoomTile.frontend.tlb._sectored_entries_0_1_data_T[41:0] = 42'h0;
        // correctness.boom.dut.BoomTile.frontend.tlb._sectored_entries_0_2_data_T[41:0] = 42'h0;
        // correctness.boom.dut.BoomTile.frontend.tlb._sectored_entries_0_3_data_T[41:0] = 42'h0;
        // correctness.boom.dut.BoomTile.frontend.tlb._sectored_entries_0_4_data_T[41:0] = 42'h0;
        // correctness.boom.dut.BoomTile.frontend.tlb._sectored_entries_0_5_data_T[41:0] = 42'h0;
        // correctness.boom.dut.BoomTile.frontend.tlb._sectored_entries_0_6_data_T[41:0] = 42'h0;
        // correctness.boom.dut.BoomTile.frontend.tlb._sectored_entries_0_7_data_T[41:0] = 42'h0;
    end

    // assign debug_pmp_r = 


    `ifdef VERILATOR
    // export "DPI-C" task initialize_cpu_states; 
// ;
    // task automatic initialize_cpu_states; 

    //     // ------------------------
    //     // BOOM Core Initialization
    //     // ------------------------
    //     boom.dut.BoomTile.core.csr.io_pmp_0_mask  = 32'h7f; // NAPOT, 128B window

    // endtask
    `endif

endmodule
