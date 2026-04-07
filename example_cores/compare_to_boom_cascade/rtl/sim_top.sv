module sim_top (
    input wire clk,
    input wire rst, // active high reset
    output wire fail
);

    sim dut(clk, rst, fail);

endmodule