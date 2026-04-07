module tilelink_memory #(
    parameter ADDR_WIDTH   = 32,   // Address width
    parameter DATA_WIDTH   = 64,   // Data width per beat (must be 64 in this impl)
    parameter DEPTH_WORDS  = 65536, // Number of 64-bit words in memory
    parameter DELAY_CYCLES = 0,     // Cycles to wait before first D beat (0 = no delay)
    parameter BASE_ADDR = 32'h80000000 // Base address of this memory
)(
    input                       clk,
    input                       reset,

    // A channel (request)
    input                       tl_a_valid,
    output reg                  tl_a_ready,
    input  [2:0]                tl_a_opcode,   // 3'b000 PutFullData, 3'b001 PutPartialData, 3'b100 Get
    input  [2:0]                tl_a_param,
    input  [3:0]                tl_a_size,     // log2(bytes)
    input  [ADDR_WIDTH-1:0]     tl_a_address,
    input  [DATA_WIDTH-1:0]     tl_a_data,
    input  [DATA_WIDTH/8-1:0]   tl_a_mask,
    input  [2:0]                tl_a_source,

    // D channel (response)
    output reg                  tl_d_valid,
    input                       tl_d_ready,
    output reg [2:0]            tl_d_opcode,   // 3'b000 AccessAck, 3'b001 AccessAckData
    output reg [DATA_WIDTH-1:0] tl_d_data,
    output reg [1:0]            tl_d_param,
    output reg                  tl_d_corrupt,
    output reg                  tl_d_denied,
    output reg [3:0]            tl_d_size,
    output reg [2:0]            tl_d_source,
    output reg [1:0]            tl_d_sink
);

    localparam BEAT_BYTES = DATA_WIDTH/8;
    // Simple backing store
    reg [DATA_WIDTH-1:0] mem [0:DEPTH_WORDS-1];

    // FSM
    typedef enum reg [1:0] { S_IDLE=2'd0, S_WAIT=2'd1, S_RESP=2'd2 } state_e;
    state_e state;

    // Latched request
    reg [2:0]            a_opcode_q;
    reg [3:0]            a_size_q;
    reg [ADDR_WIDTH-1:0] a_addr_q;
    reg [DATA_WIDTH-1:0] a_data_q;
    reg [BEAT_BYTES-1:0] a_mask_q;
    reg [2:0]            a_source_q;

    // Multi-beat book-keeping
    reg [3:0]            beats_total;  // number of beats in this burst (1<<size)/BEAT_BYTES, min 1
    reg [3:0]            beat_idx;     // current beat index

    // Initial latency counter (only before first D beat)
    reg [15:0]           delay_cnt;

    // Convenience
    wire is_get   = (tl_a_opcode == 3'b100);
    wire is_put   = (tl_a_opcode == 3'b000) || (tl_a_opcode == 3'b001);

    // Expand byte mask to DATA_WIDTH bits (assumes DATA_WIDTH==64)
    function [DATA_WIDTH-1:0] mask_expand(input [BEAT_BYTES-1:0] m);
        mask_expand = {
            {8{m[7]}},{8{m[6]}},{8{m[5]}},{8{m[4]}},
            {8{m[3]}},{8{m[2]}},{8{m[1]}},{8{m[0]}}
        };
    endfunction

    // Compute number of beats from size (min 1)
	function automatic [3:0] beats_from_size(input [3:0] size);
		begin
			if (size <= $clog2(BEAT_BYTES))
				beats_from_size = 4'd1;
			else
				beats_from_size = 1 << (size - $clog2(BEAT_BYTES));
		end
	endfunction

    initial begin
        next_addr_violation = 1'b0;
    end

    //8001_9850
    //0000_FFFF

    // Compute word index in our memory array for a given byte address + beat
    function [ADDR_WIDTH-1:0] word_index(input [ADDR_WIDTH-1:0] base_addr, input [3:0] idx);
        word_index = ((base_addr & 32'h000F_FFFF) >> $clog2(BEAT_BYTES)) + idx;
    endfunction

    // Optional: synth-safe reset of memory content (comment out for speed)
    integer init_i;
    always @(posedge clk) begin
        if (reset) begin
            // for (init_i = 0; init_i < DEPTH_WORDS; init_i = init_i + 1)
            //     mem[init_i] <= {DATA_WIDTH{1'b0}};
        end
    end

    // Helper: out-of-bounds check
    function automatic bit is_oob(input [ADDR_WIDTH-1:0] wi);
        is_oob = (wi >= DEPTH_WORDS);
    endfunction

    function automatic bit is_oob_addr(input [ADDR_WIDTH-1:0] addr);
        is_oob_addr = (addr < BASE_ADDR) || (addr >= (BASE_ADDR + (DEPTH_WORDS * DATA_WIDTH)));
    endfunction

    // New: combinational address range check (inclusive low, exclusive high)
    // Keep for Verilator visibility
    wire addr_violation;
    assign addr_violation = 1'b0; 
    			  //tl_a_valid &&
                            //((tl_a_address < 32'h0001_0000) || (tl_a_address >= 32'h0001_0080));
    reg next_addr_violation;

    // Main FSM
    always @(posedge clk) begin
        if (reset) begin
            state       <= S_IDLE;
            tl_a_ready  <= 1'b1;

            tl_d_valid  <= 1'b0;
            tl_d_opcode <= 3'b000;
            tl_d_data   <= {DATA_WIDTH{1'b0}};
            tl_d_param  <= 2'b00;
            tl_d_corrupt<= 1'b0;
            tl_d_denied <= 1'b0;
            tl_d_size   <= 4'd0;
            tl_d_source <= 3'd0;
            tl_d_sink   <= 2'd0;
            next_addr_violation <= 1'b0;

            a_opcode_q  <= 3'b000;
            a_size_q    <= 4'd0;
            a_addr_q    <= {ADDR_WIDTH{1'b0}};
            a_data_q    <= {DATA_WIDTH{1'b0}};
            a_mask_q    <= {BEAT_BYTES{1'b0}};
            a_source_q  <= 3'd0;

            beats_total <= 4'd0;
            beat_idx    <= 4'd0;
            delay_cnt   <= 16'd0;
        end else begin
            //if (tl_a_valid) begin
            //    $display("TileLink_Memory: State=%0d, tl_a_valid=%b, tl_a_ready=%b, tl_d_valid=%b, tl_d_ready=%b, beat_idx=%0d/%0d addr is %h, oob %b",
            //            state, tl_a_valid, tl_a_ready, tl_d_valid, tl_d_ready, beat_idx, beats_total, tl_a_address, is_oob_addr(tl_a_address));
            //end 
            next_addr_violation <= addr_violation;
            case (state)
                // ----------------------------------------------------------------
                S_IDLE: begin
                    tl_d_valid <= 1'b0;
                    tl_a_ready <= 1'b1;

                    if (tl_a_valid && tl_a_ready) begin
                        // Latch request
                        a_opcode_q  <= tl_a_opcode;
                        a_size_q    <= tl_a_size;
                        a_addr_q    <= tl_a_address;
                        a_data_q    <= tl_a_data;
                        a_mask_q    <= tl_a_mask;
                        a_source_q  <= tl_a_source;

                        beats_total <= beats_from_size(tl_a_size);
                        beat_idx    <= 4'd0;

                        // Optional: handle special addresses (e.g., CLINT) differently here

                        // For PUT, you may choose to apply the write now (store-through),
                        // or at response time. We do store-through to make RAW reads work.
                        if (is_put) begin
                            // apply masked write on beat 0
                            begin : do_put_beat0
                                reg [DATA_WIDTH-1:0] m;
                                reg [ADDR_WIDTH-1:0] wi;
                                m  = mask_expand(tl_a_mask);
                                wi = word_index(tl_a_address, 4'd0);
                                // Only write if in-bounds; do not set tl_d_denied here
                                if (!is_oob(wi) && !is_oob_addr(tl_a_address)) begin
                                    //$display("TileLink_Memory: Put to address %h data %h mask %h", tl_a_address, tl_a_data, tl_a_mask);
                                    mem[wi] <= (mem[wi] & ~m) | (tl_a_data & m);
                                end else begin
                                    // out-of-bounds: no write
                                    //$display("TileLink_Memory: Put to out-of-bounds address %h", tl_a_address);
                                end
                            end
                        end

                        // Prepare response meta
                        tl_d_size   <= tl_a_size;
                        tl_d_source <= tl_a_source;
                        tl_d_param  <= 2'b00;
                        tl_d_corrupt<= 1'b0;
                        // (do not pre-clear tl_d_denied here; set it when issuing D beats)

                        // Insert initial latency only before first D beat
                        tl_a_ready  <= 1'b0;
                        if (DELAY_CYCLES > 0) begin
                            delay_cnt <= DELAY_CYCLES[15:0];
                            state     <= S_WAIT;
                        end else begin
                            // No delay: go straight to RESP
                            // Drive first D beat this cycle if possible
                            // (We still follow the rule: D.valid must hold until D.ready)
                            if (is_get) begin
                                reg [ADDR_WIDTH-1:0] wi0;
                                wi0 = word_index(tl_a_address, 4'd0);
                                tl_d_opcode <= 3'b001; // AccessAckData
                                tl_d_data   <= (!is_oob(wi0) && !is_oob_addr(tl_a_address)) ? mem[wi0] : {DATA_WIDTH{1'b0}};
                                tl_d_denied <= is_oob(wi0) || is_oob_addr(tl_a_address);
                                //$display("TileLink_Memory: Get from address %h data %h is_oob %b", tl_a_address, tl_d_data, is_oob(wi0));
                            end else begin
                                tl_d_opcode <= 3'b000; // AccessAck for Put
                                tl_d_data   <= {DATA_WIDTH{1'b0}};
                                // Set deny for Put beat 0 based on bounds
                                tl_d_denied <= is_oob(word_index(tl_a_address, 4'd0)) || is_oob_addr(tl_a_address);
                            end
                            tl_d_valid <= 1'b1;
                            state      <= S_RESP;
                        end
                    end
                end

                // ----------------------------------------------------------------
                S_WAIT: begin
                    // Hold A.ready low; nothing on D yet.
                    tl_a_ready <= 1'b0;
                    tl_d_valid <= 1'b0;

                    if (delay_cnt != 16'd0) begin
                        delay_cnt <= delay_cnt - 16'd1;
                    end else begin
                        // Start first D beat now (no per-beat delay after this)
                        if (a_opcode_q == 3'b100) begin
                            reg [ADDR_WIDTH-1:0] wi0;
                            wi0 = word_index(a_addr_q, 4'd0);
                            tl_d_opcode <= 3'b001;
                            tl_d_data   <= (!is_oob(wi0) && !is_oob_addr(a_addr_q)) ? mem[wi0] : {DATA_WIDTH{1'b0}};
                            tl_d_denied <= is_oob(wi0) || is_oob_addr(a_addr_q);
                        end else begin
                            tl_d_opcode <= 3'b000;
                            tl_d_data   <= {DATA_WIDTH{1'b0}};
                            // For Put beat0: set deny based on bounds
                            tl_d_denied <= is_oob(word_index(a_addr_q, 4'd0)) || is_oob_addr(a_addr_q);
                        end
                        tl_d_valid <= 1'b1;
                        state      <= S_RESP;
                    end
                end

                // ----------------------------------------------------------------
                S_RESP: begin
                    // We are in a response burst.
                    // Keep D.valid asserted until the master accepts each beat.
                    if (tl_d_valid && tl_d_ready) begin
                        // Beat accepted; advance
                        beat_idx <= beat_idx + 4'd1;

                        if (beat_idx + 4'd1 < beats_total) begin
                            if (a_opcode_q == 3'b100) begin
                                reg [ADDR_WIDTH-1:0] win;
                                win = word_index(a_addr_q, beat_idx + 4'd1);
                                tl_d_opcode <= 3'b001;
                                tl_d_data   <= (!is_oob(win) && !is_oob_addr(a_addr_q)) ? mem[win] : {DATA_WIDTH{1'b0}};
                                tl_d_denied <= is_oob(win) || is_oob_addr(a_addr_q);
                            end else begin
                                reg [DATA_WIDTH-1:0] m;
                                reg [ADDR_WIDTH-1:0] win;
                                m   = mask_expand(a_mask_q);
                                win = word_index(a_addr_q, beat_idx + 4'd1);
                                if (!is_oob(win)) begin
                                    mem[win] <= (mem[win] & ~m) | (a_data_q & m);
                                    tl_d_denied <= 1'b0;
                                end else begin
                                    tl_d_denied <= 1'b1;
                                end
                                tl_d_opcode <= 3'b000;
                                tl_d_data   <= {DATA_WIDTH{1'b0}};
                            end
                            tl_d_valid <= 1'b1; // keep asserted for next beat
                        end else begin
                            // Last beat just accepted
                            tl_d_valid <= 1'b0;
                            tl_a_ready <= 1'b1;
                            state      <= S_IDLE;
                        end
                    end else begin
                        // Not accepted yet; hold tl_d_valid/data/opcode stable
                        tl_d_valid <= tl_d_valid;
                    end
                end

                // ----------------------------------------------------------------
                default: begin
                    state <= S_IDLE;
                end
            endcase
        end
    end
endmodule
