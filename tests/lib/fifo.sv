module fifo #(
    parameter int unsigned WIDTH = 4,
    parameter int unsigned DEPTH = 4,
    parameter int unsigned LEVEL_WIDTH = $clog2(DEPTH + 1),
    parameter int unsigned POINTER_WIDTH = DEPTH > 1 ? $clog2(DEPTH) : 1
) (
    input  logic                   clk,
    input  logic                   reset_n,
    input  logic                   in_valid,
    input  logic [WIDTH-1:0]       in_data,
    output logic                   in_ack,
    output logic                   out_valid,
    output logic [WIDTH-1:0]       out_data,
    input  logic                   out_ack,
    output logic                   full,
    output logic                   empty,
    output logic [LEVEL_WIDTH-1:0] level
);
    localparam logic [LEVEL_WIDTH-1:0] FULL_LEVEL = LEVEL_WIDTH'(DEPTH);
    localparam logic [POINTER_WIDTH-1:0] LAST_ADDRESS = POINTER_WIDTH'(DEPTH - 1);
    localparam int unsigned MEMORY_DEPTH = 1 << POINTER_WIDTH;

    logic [WIDTH-1:0] memory [MEMORY_DEPTH];
    logic [POINTER_WIDTH-1:0] read_pointer;
    logic [POINTER_WIDTH-1:0] write_pointer;
    logic push;
    logic pop;

    assign full = level == FULL_LEVEL;
    assign empty = level == '0;
    assign in_ack = !full || out_ack;
    assign out_valid = !empty;
    assign out_data = memory[read_pointer];
    assign push = in_valid && in_ack;
    assign pop = out_valid && out_ack;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            read_pointer <= '0;
            write_pointer <= '0;
            level <= '0;
        end else begin
            if (push) begin
                if (write_pointer == LAST_ADDRESS) begin
                    write_pointer <= '0;
                end else begin
                    write_pointer <= write_pointer + 1'b1;
                end
            end

            if (pop) begin
                if (read_pointer == LAST_ADDRESS) begin
                    read_pointer <= '0;
                end else begin
                    read_pointer <= read_pointer + 1'b1;
                end
            end

            if (push && !pop) begin
                level <= level + 1'b1;
            end else if (!push && pop) begin
                level <= level - 1'b1;
            end
        end

        if (push) begin
            memory[write_pointer] <= in_data;
        end
    end

    assert_stable_while_stalled: assert property (
        @(posedge clk) disable iff (!reset_n)
        out_valid && !out_ack |=> $stable(out_valid) && $stable(out_data)
    );
endmodule
