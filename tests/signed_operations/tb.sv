module signed_operations (
    input logic clk,
    input logic reset_n,
    input logic [3:0] lhs,
    input logic [3:0] rhs,
    output logic [3:0] captured_sum,
    output logic [3:0] captured_unsigned_product,
    output logic signed [3:0] captured_signed_product,
    output logic [3:0] captured_unsigned_quotient,
    output logic signed [3:0] captured_signed_quotient
);
    logic signed [3:0] signed_lhs;
    logic signed [3:0] signed_rhs;

    assign signed_lhs = lhs;
    assign signed_rhs = rhs;

    always_ff @(posedge clk) begin
        captured_sum <= signed_lhs + signed_rhs;
        captured_unsigned_product <= lhs * rhs;
        captured_signed_product <= signed_lhs * signed_rhs;
        captured_unsigned_quotient <= lhs / rhs;
        captured_signed_quotient <= signed_lhs / signed_rhs;
    end

    assert_signed_add_matches_unsigned: assert property (
        @(posedge clk) signed_lhs + signed_rhs == lhs + rhs
    );

    assert_signed_sub_matches_unsigned: assert property (
        @(posedge clk) signed_lhs - signed_rhs == lhs - rhs
    );

    assert_signed_mul_matches_unsigned: assert property (
        @(posedge clk) signed_lhs * signed_rhs == lhs * rhs
    );

    assert_signed_lt_matches_biased_unsigned: assert property (
        @(posedge clk) (signed_lhs < signed_rhs) ==
            ((lhs ^ 4'b1000) < (rhs ^ 4'b1000))
    );

    assert_signed_lte_matches_biased_unsigned: assert property (
        @(posedge clk) (signed_lhs <= signed_rhs) ==
            ((lhs ^ 4'b1000) <= (rhs ^ 4'b1000))
    );

    assert_signed_gt_matches_biased_unsigned: assert property (
        @(posedge clk) (signed_lhs > signed_rhs) ==
            ((lhs ^ 4'b1000) > (rhs ^ 4'b1000))
    );

    assert_signed_gte_matches_biased_unsigned: assert property (
        @(posedge clk) (signed_lhs >= signed_rhs) ==
            ((lhs ^ 4'b1000) >= (rhs ^ 4'b1000))
    );
endmodule
