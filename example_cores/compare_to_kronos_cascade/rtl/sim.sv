module sim (
    input wire clk,
    input wire rst,
    output wire fail
);

    parameter memsize = `MEMSIZE;
    parameter depth = memsize/4;
    parameter aw = $clog2(memsize);
    //parameter timer_addr = `TIMER_ADDR;
    parameter ADDR_START = 32'h80000000;
    parameter ADDR_END = ADDR_START + memsize; // 512 bytes range

    wire ibus_cyc;
    wire ibus_gnt;
    reg ibus_ack;
    wire [31:0] ibus_addr;      // Instruction address
    reg [31:0] ibus_rdata;     // Instruction data

    wire dbus_cyc;              // Data bus cycle
    wire dbus_gnt;              // Data bus grant
    reg dbus_ack;               // Data bus acknowledge
    wire [31:0] dbus_addr;      // Data address
    wire dbus_we;               // Data write enable
    wire [3:0] dbus_be;         // Data byte enable
    wire [31:0] dbus_wdata;     // Data write data
    reg [31:0] dbus_rdata;     // Data read data

    reg [31:0] timer_expiry_time;   // Timer expiry time
    reg [31:0] timer_time;          // Timer time
    reg timer_irq;                  // Timer interrupt

    reg [31:0] mem [0:depth-1];     // Memory
    reg          mem_fail = 1'b0; // set on illegal memory access
    assign fail = mem_fail;

    wire dbus_addr_out_of_range;
    wire ibus_addr_out_of_range;

    assign dbus_addr_out_of_range = (dbus_addr < ADDR_START) || (dbus_addr >= ADDR_END);
    assign ibus_addr_out_of_range = (ibus_addr < ADDR_START) || (ibus_addr >= ADDR_END);
    assign dbus_gnt = dbus_cyc & !dbus_addr_out_of_range;
    assign ibus_gnt = ibus_cyc & !dbus_cyc & !ibus_addr_out_of_range;

    wire core_sleep;

    always @(posedge clk) begin
        // if (core_sleep && timer_time>1000) begin
        //     $finish;
        // end

        if (dbus_cyc && dbus_we) begin
            // if ($countones(dbus_addr ^ timer_addr) <= 1)
            //     timer_expiry_time <= dbus_wdata;
            // else begin
             if (!(dbus_addr_out_of_range)) begin   
                if (dbus_be[0]) mem[dbus_addr[aw-1:2]][7:0] <= dbus_wdata[7:0];
                if (dbus_be[1]) mem[dbus_addr[aw-1:2]][15:8] <= dbus_wdata[15:8];
                if (dbus_be[2]) mem[dbus_addr[aw-1:2]][23:16] <= dbus_wdata[23:16];
                if (dbus_be[3]) mem[dbus_addr[aw-1:2]][31:24] <= dbus_wdata[31:24];
            end
        end

        // if ($countones(dbus_addr ^ timer_addr) <= 1) begin
        //     dbus_rdata <= timer_time;
        // end else 
        if (!(dbus_addr_out_of_range)) begin
            dbus_rdata <= mem[dbus_addr[aw-1:2]];
        end else if (dbus_cyc) begin
            // $display("mem fail data access at %x", dbus_addr);
            mem_fail <= 1'b1;
        end
        
        if (!(ibus_addr_out_of_range)) begin
            ibus_rdata <= mem[ibus_addr[aw-1:2]];
        end else if (ibus_cyc) begin
            // $display("mem fail instruction access at %x", ibus_addr);
            mem_fail <= 1'b1;
        end

        
        // $display("ibus_addr=%h, ibus_rdata=%h, masked to=%x", ibus_addr, ibus_rdata, mem[ibus_addr[aw-1:2]]);
        //$display("ibus_addr=%h, reading from %x, returning=%x, memsize %d \n", ibus_addr, ibus_addr[aw-1:2], mem[ibus_addr[aw-1:2]], memsize);
        //$display("dbus_addr=%h, reading from %x, returning=%x \n", dbus_addr, dbus_addr[aw-1:2], mem[dbus_addr[aw-1:2]]);

        if (ibus_gnt) begin
            ibus_ack <= 1'b1;
            dbus_ack <= 1'b0;
        end else if (dbus_gnt) begin
            ibus_ack <= 1'b0;
            dbus_ack <= 1'b1;
        end else begin
            ibus_ack <= 1'b0;
            dbus_ack <= 1'b0;
        end

        // timer_time <= timer_time + 'd1;
        // timer_irq <= (timer_time >= timer_expiry_time);

        if (rst) begin
            // timer_time <= 0;
            // timer_expiry_time <= 0;
            ibus_ack <= 1'b0;
            dbus_ack <= 1'b0;
            mem_fail <= 1'b0;
        end
    end

    sim_cpu cpu (
        .clk         (clk),
        .i_rst       (rst),
        .i_timer_irq (timer_irq),

        .o_ibus_adr  (ibus_addr),
        .o_ibus_cyc  (ibus_cyc),
        .i_ibus_gnt  (ibus_gnt),
        .i_ibus_rdt  (ibus_rdata),
        .i_ibus_ack  (ibus_ack),

        .o_dbus_adr  (dbus_addr),
        .o_dbus_dat  (dbus_wdata),
        .o_dbus_we   (dbus_we),
        .o_dbus_be   (dbus_be),
        .o_dbus_cyc  (dbus_cyc),
        .i_dbus_gnt  (dbus_gnt),
        .i_dbus_rdt  (dbus_rdata),
        .i_dbus_ack  (dbus_ack)
    );
endmodule
