module wire_ports (
    input  wire       clk,
    input  wire       reset_n,
    input  wire [3:0] data_in,
    output wire [3:0] data_out
);
    logic [3:0] registered_data;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            registered_data <= '0;
        end else begin
            registered_data <= data_in;
        end
    end

    assign data_out = registered_data;

    assert_output_matches_register: assert property (
        @(posedge clk) disable iff (!reset_n) data_out == registered_data
    );

`ifdef CIRCT
    // Apply reset at the first rising edge, then keep it deasserted.
    logic circt_past_valid = 1'b0;
    always @(posedge clk)
        circt_past_valid <= 1'b1;

    circt_assume_initial_reset: assume property (
        @(posedge clk) reset_n == circt_past_valid
    );
`endif
endmodule
