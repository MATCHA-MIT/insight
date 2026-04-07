`ifndef CHECKER_DATA_WIDTH
    `define CHECKER_DATA_WIDTH 32
`endif

`ifndef CHECKER_TARGET_WIDTH
    `define CHECKER_TARGET_WIDTH 6
`endif

`ifndef CHECKER_PC_WIDTH
    `define CHECKER_PC_WIDTH 32
`endif

`ifndef CHECKER_MaxNumTracedInstructions
    `define CHECKER_MaxNumTracedInstructions 2016 // (Memory size / 4) - 32
`endif

`ifndef CHECKER_MAX_EXECUTION_WINDOW
    `define CHECKER_MAX_EXECUTION_WINDOW 32'd10
`endif