module sim (
    input wire clk,
    input wire rst,
    output wire fail
);

	wire channel_a_ready;
	wire channel_a_valid;
	wire [2:0] channel_a_opcode;
	wire [2:0] channel_a_param;
	wire [3:0] channel_a_size;
	wire [2:0] channel_a_source;
	wire [31:0] channel_a_address;
	wire [7:0] channel_a_mask;
	wire [63:0] channel_a_data;
	wire channel_d_ready;
	wire channel_d_valid;
	wire [2:0] channel_d_opcode;
	wire [1:0] channel_d_param;
	wire [3:0] channel_d_size;
	wire [2:0] channel_d_source;
	wire [1:0] channel_d_sink;
	wire channel_d_denied;
	wire [63:0] channel_d_data;
	wire channel_d_corrupt;

	BoomTile BoomTile (
	     .clock                          (clk),
 	     .reset                          (rst),
 	     .auto_buffer_out_a_ready        (channel_a_ready), 
 	     .auto_buffer_out_a_valid        (channel_a_valid),
 	     .auto_buffer_out_a_bits_opcode  (channel_a_opcode),
 	     .auto_buffer_out_a_bits_param   (channel_a_param),
 	     .auto_buffer_out_a_bits_size    (channel_a_size),
	     .auto_buffer_out_a_bits_source  (channel_a_source),
	     .auto_buffer_out_a_bits_address (channel_a_address),
	     .auto_buffer_out_a_bits_mask    (channel_a_mask),
	     .auto_buffer_out_a_bits_data    (channel_a_data),
	     .auto_buffer_out_b_ready        (),
	     .auto_buffer_out_b_valid        (0), 
	     .auto_buffer_out_b_bits_opcode  (0),   
	     .auto_buffer_out_b_bits_param   (0),    
	     .auto_buffer_out_b_bits_size    (0),     
	     .auto_buffer_out_b_bits_source  (0),   
	     .auto_buffer_out_b_bits_address (0),  
	     .auto_buffer_out_b_bits_mask    (0),     
	     .auto_buffer_out_b_bits_corrupt (0),
	     .auto_buffer_out_c_ready        (0),	
             .auto_buffer_out_c_valid        (),
	     .auto_buffer_out_c_bits_opcode  (),
	     .auto_buffer_out_c_bits_param   (),
	     .auto_buffer_out_c_bits_size    (),
	     .auto_buffer_out_c_bits_source  (),
             .auto_buffer_out_c_bits_address (),
    	     .auto_buffer_out_c_bits_data    (),
             .auto_buffer_out_d_ready        (channel_d_ready),
    	     .auto_buffer_out_d_valid        (channel_d_valid), 
    	     .auto_buffer_out_d_bits_opcode  (channel_d_opcode),    
    	     .auto_buffer_out_d_bits_param   (channel_d_param),     
    	     .auto_buffer_out_d_bits_size    (channel_d_size),      
   	     .auto_buffer_out_d_bits_source  (channel_d_source),    
    	     .auto_buffer_out_d_bits_sink    (channel_d_sink),      
    	     .auto_buffer_out_d_bits_denied  (channel_d_denied),    
    	     .auto_buffer_out_d_bits_data    (channel_d_data),      
    	     .auto_buffer_out_d_bits_corrupt (channel_d_corrupt),   
    	     .auto_buffer_out_e_ready        (0),  
    	     .auto_buffer_out_e_valid        (),
    	     .auto_buffer_out_e_bits_sink    (),
    	     .auto_int_local_in_3_0          (0),    
    	     .auto_int_local_in_2_0          (0),    
    	     .auto_int_local_in_1_0          (0),    
    	     .auto_int_local_in_1_1          (0),    
    	     .auto_int_local_in_0_0          (0),      
    	     .auto_hartid_in                 (0)
	);


	tilelink_memory TileLink_Memory (
		.clk(clk),
		.reset(rst),
		.tl_a_source(channel_a_source),
		.tl_a_valid(channel_a_valid),
		.tl_a_ready(channel_a_ready),
		.tl_a_opcode(channel_a_opcode),
		.tl_a_param(channel_a_param),
		.tl_a_size(channel_a_size),
		.tl_a_address(channel_a_address),
		.tl_a_data(channel_a_data),
		.tl_a_mask(channel_a_mask),
		
		.tl_d_size(channel_d_size),
		.tl_d_source(channel_d_source),
		.tl_d_valid(channel_d_valid),
		.tl_d_ready(channel_d_ready),
		.tl_d_opcode(channel_d_opcode),
		.tl_d_data(channel_d_data),
		.tl_d_param(channel_d_param),
		.tl_d_denied(channel_d_denied),
		.tl_d_corrupt(channel_d_corrupt),
		.tl_d_sink(channel_d_sink)
	);	

endmodule
