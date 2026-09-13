module combinational_loops (
    input  logic        clk,
    input  logic        reset_n,
    output logic [31:0] total
);
    logic [31:0] counts [2];

    always_comb counts = '{
        32'd1,
        32'd2
    };

    assign total = counts[0] + counts[1];

    assert_total: assert property (@(posedge clk) total == 32'd3);
endmodule
