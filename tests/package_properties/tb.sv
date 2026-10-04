package property_pkg;
    parameter int unsigned WIDTH = 4;

    property reset_clears_count(
        logic clk,
        logic reset_n,
        logic [WIDTH-1:0] count
    );
        @(posedge clk) !reset_n |=> count == '0;
    endproperty

    property disabled_holds_count(
        logic clk,
        logic reset_n,
        logic enable,
        logic [WIDTH-1:0] count
    );
        @(posedge clk) disable iff (!reset_n) !enable |=> $stable(count);
    endproperty
endpackage

module package_properties (
    input  logic                           clk,
    input  logic                           reset_n,
    input  logic                           enable,
    output logic [property_pkg::WIDTH-1:0] count
);
    import property_pkg::*;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            count <= '0;
        end else if (enable) begin
            count <= count + 1'b1;
        end
    end

    assert_reset_clears_count: assert property (
        reset_clears_count(clk, reset_n, count)
    );

    assert_disabled_holds_count: assert property (
        disabled_holds_count(clk, reset_n, enable, count)
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
