module public_counter (
    input  logic       clk,
    input  logic       reset_n,
    input  logic       enable,
    output logic [3:0] count
);
    logic [3:0] state /*verilator public*/;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            state <= '0;
        end else if (enable) begin
            state <= state + 1'b1;
        end
    end

    assign count = state;

    assert_output_matches_state: assert property (
        @(posedge clk) count == state
    );
endmodule

module public_submodules (
    input  logic       clk,
    input  logic       reset_n,
    input  logic       enable0,
    input  logic       enable1,
    output logic [3:0] count0,
    output logic [3:0] count1
);
    public_counter counter0 (
        .clk,
        .reset_n,
        .enable(enable0),
        .count(count0)
    );

    public_counter counter1 (
        .clk,
        .reset_n,
        .enable(enable1),
        .count(count1)
    );
endmodule
