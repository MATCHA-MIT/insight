module sim_cpu
(
    input  wire        clk,
    input  wire        i_rst,
    input  wire        i_timer_irq,
    output wire        fail,

    output wire [31:0] o_ibus_adr,
    output wire        o_ibus_cyc,
    input  wire        i_ibus_gnt,
    input  wire [31:0] i_ibus_rdt,
    input  wire        i_ibus_ack,

    output wire [31:0] o_dbus_adr,
    output wire [31:0] o_dbus_dat,
    output wire        o_dbus_we,
    output wire [3:0]  o_dbus_be,
    output wire        o_dbus_cyc,
    input  wire        i_dbus_gnt,
    input  wire [31:0] i_dbus_rdt,
    input  wire        i_dbus_ack
);

    // TODO: probably best to compare ibus_adr to trap handler
    //fail = 1'b0;

    kronos_core #(
        .BOOT_ADDR (32'h80000000)
    ) cpu (
        .clk (clk),
        .rstz (!i_rst),

        .instr_addr (o_ibus_adr),
        .instr_data (i_ibus_rdt),
        .instr_req (o_ibus_cyc),
        .instr_ack (i_ibus_ack),

        .data_addr (o_dbus_adr),
        .data_rd_data (i_dbus_rdt),
        .data_wr_data (o_dbus_dat),
        .data_mask (o_dbus_be),
        .data_wr_en (o_dbus_we),
        .data_req (o_dbus_cyc),
        .data_ack (i_dbus_ack),

        .software_interrupt (1'b0),
        .timer_interrupt (1'b0),
        .external_interrupt (1'b0)
    );

endmodule
