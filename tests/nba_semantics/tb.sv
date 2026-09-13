module nba_semantics (
    input logic clk, reset_n, enable, index,
    input logic [7:0] data,
    output logic [7:0] a, b, pipeline, packed_value, temp_result,
    output logic [7:0] mem0, mem1, blocking_count, lane0, lane1
);
    logic [7:0] memory [2];
    logic [7:0] lanes [2];
    assign lane0 = lanes[0];
    assign lane1 = lanes[1];
    logic address;
    logic [7:0] temporary;
    assign mem0 = memory[0];
    assign mem1 = memory[1];

    always @(posedge clk) begin
        if (!reset_n) begin
            a <= 8'd1;
            b <= 8'd2;
            packed_value <= 0;
            temp_result <= 0;
            memory[0] <= 0;
            memory[1] <= 0;
        end else if (enable) begin
            a <= b;
            b <= a;
            temporary = data + 8'd1;
            temp_result <= temporary;
            packed_value <= data;
            packed_value[3:0] <= temporary[3:0];
            packed_value[2:1] <= 2'b10;
            address = index;
            memory[address] <= data;
            address = !address;
            memory[address][3:0] <= temporary[3:0];
        end
    end

    // A separate process must still read a's pre-edge value.
    always @(posedge clk) begin
        if (!reset_n) pipeline <= 0;
        else if (enable) pipeline <= a;
    end

    always @(posedge clk) begin
        if (!reset_n) blocking_count = 0;
        else blocking_count = blocking_count + 8'd1;
    end

    // Separate dynamic-address writers own disjoint bits of the same memory.
    always @(posedge clk) begin
        if (!reset_n) begin
            lanes[0][3:0] <= 0;
            lanes[1][3:0] <= 0;
        end else if (enable) lanes[index][3:0] <= data[3:0];
    end
    always @(posedge clk) begin
        if (!reset_n) begin
            lanes[0][7:4] <= 0;
            lanes[1][7:4] <= 0;
        end else if (enable) lanes[index][7:4] <= data[7:4];
    end

    assert_pipeline: assert property (@(posedge clk) disable iff (!reset_n)
        enable |=> pipeline == $past(a));
    assert_sampled_blocking: assert property (@(posedge clk) disable iff (!reset_n)
        1'b1 |=> blocking_count == ($past(blocking_count) + 8'd1));
endmodule
