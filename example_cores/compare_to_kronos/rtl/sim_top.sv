module sim_top (
    input wire clk,
    input wire rst // active high reset,
);

    sim dut(clk, rst);

endmodule