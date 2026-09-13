module counter_ones #(
    parameter int unsigned WIDTH = 4
) (
    input  logic             clk,
    input  logic             reset_n,
    input  logic             enable,
    output logic [WIDTH-1:0] count
);
    always_ff @(posedge clk) begin
        if (!reset_n) begin
            count <= '1;
        end else if (enable) begin
            count <= count + 1'b1;
        end
    end

    assert_holds_when_disabled: assert property (
        @(posedge clk) disable iff (!reset_n) !enable |=> $stable(count)
    );
endmodule
