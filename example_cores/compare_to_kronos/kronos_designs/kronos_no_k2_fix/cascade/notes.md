commit 59b67e40c5eccba2852694609c3a0b955d3f0263 (K1)

To reintroduce K2, remove these lines from kronos_csr.sv:
    else if (state == WRITE) begin
        regwr_csr <= 1'b0;
    end

To reintroduce K5-a, replace from kronos_ID.sv:
          // implementing fence.i as `j f1` (jump to pc+4) 
          // as this will flush the pipeline and cause a fresh 
          // fetch of the instructions after the fence.i instruction
          is_fencei = 1'b1;
          instr_valid = 1'b1;
Back with
          if (IR[31:20] == 12'b0 && rs1 == '0 && rd =='0) begin
            // implementing fence.i as `j f1` (jump to pc+4) 
            // as this will flush the pipeline and cause a fresh 
            // fetch of the instructions after the fence.i instruction
            is_fencei = 1'b1;
            instr_valid = 1'b1;

To reintroduce K5-b, replace from kronos_ID.sv:
    instr_valid = 1'b1;
Back with
    if (funct7[6:3] == '0 && rs1 == '0 && rd =='0) instr_valid = 1'b1;
