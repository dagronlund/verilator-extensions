module counter_free #(
    parameter int unsigned WIDTH = 4
) (
    input  logic             clk,
    input  logic             reset_n,
    output logic [WIDTH-1:0] count
);
    localparam logic [WIDTH-1:0] MAX_COUNT = '1;

    // Enable is a "free" variable, meaning that the formal tool can choose its 
    // value at each clock cycle
    logic enable;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            count <= '0;
        end else if (enable) begin
            count <= count + 1'b1;
        end
    end

    // The environment does not request another increment at the maximum value.
    assume_no_overflow: assume property (
        @(posedge clk) disable iff (!reset_n) enable |-> count != MAX_COUNT
    );

    // When disabled, the counter retains its value on the following clock.
    assert_holds_when_disabled: assert property (
        @(posedge clk) disable iff (!reset_n) !enable |=> $stable(count)
    );

    // When disabled, the counter retains its value on the following clock.
    cover_saturation: cover property (
        @(posedge clk) disable iff (!reset_n) count == MAX_COUNT
    );
endmodule
