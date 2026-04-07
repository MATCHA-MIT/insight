module SimDRAM #(
    parameter ADDR_BITS = 32,
    parameter DATA_BITS = 64,
    parameter ID_BITS   = 5,
    parameter MEM_SIZE  = 1000 * 1000 * 1000,
    parameter LINE_SIZE = 64,
    parameter WORD_SIZE = DATA_BITS / 8,
    parameter CLOCK_HZ  = 100000,
    parameter STRB_BITS = DATA_BITS / 8,
    parameter MEM_BASE  = 0,
    parameter CHIP_ID   = 0
)(
  input                  clock,
  input                  reset,
  output                 axi_aw_ready,
  input                  axi_aw_valid,
  input  [ADDR_BITS-1:0] axi_aw_bits_addr,
  input  [7:0]           axi_aw_bits_len,
  input  [2:0]           axi_aw_bits_size,
  input  [1:0]           axi_aw_bits_burst,
  input                  axi_aw_bits_lock,
  input  [3:0]           axi_aw_bits_cache,
  input  [2:0]           axi_aw_bits_prot,
  input  [3:0]           axi_aw_bits_qos,
  input  [ID_BITS-1:0]   axi_aw_bits_id,
  output                 axi_w_ready,
  input                  axi_w_valid,
  input  [DATA_BITS-1:0] axi_w_bits_data,
  input                  axi_w_bits_last,
  input  [STRB_BITS-1:0] axi_w_bits_strb,
  input                  axi_b_ready,
  output                 axi_b_valid,
  output [1:0]           axi_b_bits_resp,
  output [ID_BITS-1:0]   axi_b_bits_id,
  output                 axi_ar_ready,
  input                  axi_ar_valid,
  input  [ADDR_BITS-1:0] axi_ar_bits_addr,
  input  [7:0]           axi_ar_bits_len,
  input  [2:0]           axi_ar_bits_size,
  input  [1:0]           axi_ar_bits_burst,
  input                  axi_ar_bits_lock,
  input  [3:0]           axi_ar_bits_cache,
  input  [2:0]           axi_ar_bits_prot,
  input  [3:0]           axi_ar_bits_qos,
  input  [ID_BITS-1:0]   axi_ar_bits_id,
  input                  axi_r_ready,
  output                 axi_r_valid,
  output [1:0]           axi_r_bits_resp,
  output [DATA_BITS-1:0] axi_r_bits_data,
  output                 axi_r_bits_last,
  output [ID_BITS-1:0]   axi_r_bits_id
);

  // Minimal AXI slave stub:
  // - Accepts one read burst at a time, returns zero data + OKAY.
  // - Accepts one write burst at a time, returns OKAY response.
  // This is intentionally simple; it exists to avoid DPI symbols from SimDRAM.v.

  localparam int unsigned OKAY = 2'b00;

  // Read channel state
  reg                  r_valid_r;
  reg [ID_BITS-1:0]     r_id_r;
  reg [DATA_BITS-1:0]   r_data_r;
  reg [8:0]             r_beats_left; // supports len up to 255

  wire r_fire = axi_r_valid && axi_r_ready;

  // Write channel state
  reg                  w_busy;
  reg                  b_valid_r;
  reg [ID_BITS-1:0]     b_id_r;
  reg [8:0]             w_beats_left;

  wire aw_fire = axi_aw_valid && axi_aw_ready;
  wire w_fire  = axi_w_valid && axi_w_ready;
  wire b_fire  = axi_b_valid && axi_b_ready;

  // Ready/valid outputs
  assign axi_ar_ready     = !r_valid_r;      // single outstanding read burst
  assign axi_r_valid      = r_valid_r;
  assign axi_r_bits_id    = r_id_r;
  assign axi_r_bits_data  = r_data_r;
  assign axi_r_bits_resp  = OKAY;
  assign axi_r_bits_last  = (r_beats_left == 9'd1);

  assign axi_aw_ready     = !w_busy && !b_valid_r;
  assign axi_w_ready      = w_busy && !b_valid_r;
  assign axi_b_valid      = b_valid_r;
  assign axi_b_bits_id    = b_id_r;
  assign axi_b_bits_resp  = OKAY;

  always @(posedge clock) begin
    if (reset) begin
      r_valid_r     <= 1'b0;
      r_id_r        <= {ID_BITS{1'b0}};
      r_data_r      <= {DATA_BITS{1'b0}};
      r_beats_left  <= 9'd0;

      w_busy        <= 1'b0;
      b_valid_r     <= 1'b0;
      b_id_r        <= {ID_BITS{1'b0}};
      w_beats_left  <= 9'd0;
    end else begin
      // Latch a new read burst
      if (axi_ar_ready && axi_ar_valid) begin
        r_valid_r    <= 1'b1;
        r_id_r       <= axi_ar_bits_id;
        r_data_r     <= {DATA_BITS{1'b0}};
        r_beats_left <= {1'b0, axi_ar_bits_len} + 9'd1;
      end

      // Advance read beats
      if (r_fire) begin
        if (r_beats_left <= 9'd1) begin
          r_valid_r    <= 1'b0;
          r_beats_left <= 9'd0;
        end else begin
          r_beats_left <= r_beats_left - 9'd1;
        end
      end

      // Latch a new write burst
      if (aw_fire) begin
        w_busy       <= 1'b1;
        b_id_r       <= axi_aw_bits_id;
        w_beats_left <= {1'b0, axi_aw_bits_len} + 9'd1;
      end

      // Consume write data beats
      if (w_fire && w_busy) begin
        if (w_beats_left <= 9'd1 || axi_w_bits_last) begin
          w_busy       <= 1'b0;
          w_beats_left <= 9'd0;
          b_valid_r    <= 1'b1;
        end else begin
          w_beats_left <= w_beats_left - 9'd1;
        end
      end

      // Complete write response
      if (b_fire) begin
        b_valid_r <= 1'b0;
      end
    end
  end

  // Unused inputs are intentionally ignored in this stub
  wire _unused = &{1'b0,
    axi_aw_bits_addr,
    axi_aw_bits_size,
    axi_aw_bits_burst,
    axi_aw_bits_lock,
    axi_aw_bits_cache,
    axi_aw_bits_prot,
    axi_aw_bits_qos,
    axi_w_bits_data,
    axi_w_bits_strb,
    axi_ar_bits_addr,
    axi_ar_bits_size,
    axi_ar_bits_burst,
    axi_ar_bits_lock,
    axi_ar_bits_cache,
    axi_ar_bits_prot,
    axi_ar_bits_qos};

endmodule
