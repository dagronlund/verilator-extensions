module fifo_stage_bypass #(
    parameter int unsigned WIDTH,
    parameter int unsigned FIFO_DEPTH,
    parameter int unsigned FIFO_COUNT_WIDTH = $clog2(2*FIFO_DEPTH + 1)
) (
    input  logic                        clk,
    input  logic                        reset_n,
    input  logic                        in_valid,
    input  logic [WIDTH-1:0]            in_data,
    output logic                        in_ack,
    output logic                        out_valid,
    output logic [WIDTH-1:0]            out_data,
    input  logic                        out_ack,
    output logic [FIFO_COUNT_WIDTH-1:0] fifo_count,
    output logic                        empty
);
    localparam int unsigned FIFO_STORAGE_DEPTH = 2*FIFO_DEPTH;
    localparam int unsigned TOTAL_CAPACITY = FIFO_STORAGE_DEPTH + 2;
    localparam int unsigned INDEX_WIDTH = $clog2(TOTAL_CAPACITY + 1);
    localparam int unsigned BEAT_WIDTH = WIDTH + INDEX_WIDTH;

    logic [INDEX_WIDTH-1:0] write_index;
    logic [INDEX_WIDTH-1:0] read_index;
    logic fifo_in_valid;
    logic fifo_in_ack;
    logic [BEAT_WIDTH-1:0] fifo_in_data;
    logic fifo_out_valid;
    logic [BEAT_WIDTH-1:0] fifo_out_data;
    logic fifo_out_ack;
    logic fifo_full;
    logic fifo_empty;
    logic stage_in_valid;
    logic stage_in_ack;
    logic [BEAT_WIDTH-1:0] stage_in_data;
    logic stage_out_valid;
    logic [BEAT_WIDTH-1:0] stage_out_data;
    logic stage_out_ack;
    logic fifo_selected;
    logic stage_selected;
    logic push;
    logic pop;

    assign fifo_in_valid = in_valid && !stage_in_ack;
    assign fifo_in_data = {write_index, in_data};
    assign stage_in_valid = in_valid && stage_in_ack;
    assign stage_in_data = {write_index, in_data};
    assign in_ack = stage_in_ack || fifo_in_ack;
    assign push = in_valid && in_ack;

    assign fifo_selected = fifo_out_valid
        && fifo_out_data[WIDTH +: INDEX_WIDTH] == read_index;
    assign stage_selected = stage_out_valid
        && stage_out_data[WIDTH +: INDEX_WIDTH] == read_index;
    assign out_valid = fifo_selected || stage_selected;
    assign out_data = stage_selected
        ? stage_out_data[WIDTH-1:0]
        : fifo_out_data[WIDTH-1:0];
    assign fifo_out_ack = out_ack && fifo_selected;
    assign stage_out_ack = out_ack && stage_selected;
    assign pop = out_valid && out_ack;
    assign empty = fifo_empty && stage_in_ack && !stage_out_valid;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            write_index <= '0;
            read_index <= '0;
        end else begin
            if (push) begin
                write_index <= write_index + 1'b1;
            end
            if (pop) begin
                read_index <= read_index + 1'b1;
            end
        end
    end

    fifo #(
        .WIDTH(BEAT_WIDTH),
        .DEPTH(FIFO_STORAGE_DEPTH),
        .LEVEL_WIDTH(FIFO_COUNT_WIDTH)
    ) fifo_i (
        .clk(clk),
        .reset_n(reset_n),
        .in_valid(fifo_in_valid),
        .in_data(fifo_in_data),
        .in_ack(fifo_in_ack),
        .out_valid(fifo_out_valid),
        .out_data(fifo_out_data),
        .out_ack(fifo_out_ack),
        .full(fifo_full),
        .empty(fifo_empty),
        .level(fifo_count)
    );

    stage #(
        .WIDTH(BEAT_WIDTH),
        .PIPELINE_VALID(1'b1),
        .PIPELINE_ACK(1'b1)
    ) stage_i (
        .clk(clk),
        .reset_n(reset_n),
        .in_valid(stage_in_valid),
        .in_data(stage_in_data),
        .in_ack(stage_in_ack),
        .out_valid(stage_out_valid),
        .out_data(stage_out_data),
        .out_ack(stage_out_ack)
    );
endmodule

module fifo_stage_bypass_tb #(
    parameter int unsigned WIDTH = 1,
    parameter int unsigned STAGES = 3,
    parameter int unsigned FIFO_DEPTH = STAGES - 1,
    parameter int unsigned FIFO_COUNT_WIDTH = $clog2(2*FIFO_DEPTH + 1)
) (
    input  logic             clk,
    input  logic             reset_n,
    input  logic             in_valid,
    input  logic [WIDTH-1:0] in_data,
    output logic             in_ack,
    output logic             out_valid,
    output logic [WIDTH-1:0] out_data,
    input  logic             out_ack
);
    localparam logic [FIFO_COUNT_WIDTH-1:0] FULL_COUNT =
        FIFO_COUNT_WIDTH'(2*FIFO_DEPTH);
    localparam int unsigned TOTAL_CAPACITY = 2*FIFO_DEPTH + 2;
    localparam int unsigned REFERENCE_COUNT_WIDTH = $clog2(TOTAL_CAPACITY + 1);
    localparam int unsigned REFERENCE_INDEX_WIDTH = $clog2(TOTAL_CAPACITY + 1);
    localparam int unsigned REFERENCE_MEMORY_DEPTH = 1 << REFERENCE_INDEX_WIDTH;
    localparam logic [REFERENCE_COUNT_WIDTH-1:0] REFERENCE_FULL_COUNT =
        REFERENCE_COUNT_WIDTH'(TOTAL_CAPACITY);

    logic wrapper_in_ack;
    logic wrapper_out_valid;
    logic [WIDTH-1:0] wrapper_out_data;
    logic [FIFO_COUNT_WIDTH-1:0] fifo_count;
    logic wrapper_empty;
    logic [WIDTH-1:0] reference_memory [REFERENCE_MEMORY_DEPTH];
    logic [REFERENCE_INDEX_WIDTH-1:0] reference_write_index;
    logic [REFERENCE_INDEX_WIDTH-1:0] reference_read_index;
    logic [REFERENCE_COUNT_WIDTH-1:0] reference_count;
    logic [WIDTH-1:0] reference_out_data;
    logic reference_full;
    logic reference_empty;
    logic push;
    logic pop;

    assign in_ack = wrapper_in_ack;
    assign out_valid = wrapper_out_valid;
    assign out_data = wrapper_out_data;
    assign push = in_valid && wrapper_in_ack;
    assign pop = wrapper_out_valid && out_ack;
    assign reference_out_data = reference_memory[reference_read_index];
    assign reference_full = reference_count == REFERENCE_FULL_COUNT;
    assign reference_empty = reference_count == '0;

    fifo_stage_bypass #(
        .WIDTH(WIDTH),
        .FIFO_DEPTH(FIFO_DEPTH)
    ) wrapper_i (
        .clk(clk),
        .reset_n(reset_n),
        .in_valid(in_valid),
        .in_data(in_data),
        .in_ack(wrapper_in_ack),
        .out_valid(wrapper_out_valid),
        .out_data(wrapper_out_data),
        .out_ack(out_ack),
        .fifo_count(fifo_count),
        .empty(wrapper_empty)
    );

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            reference_count <= '0;
            reference_write_index <= '0;
            reference_read_index <= '0;
        end else begin
            if (push) begin
                reference_write_index <= reference_write_index + 1'b1;
            end
            if (pop) begin
                reference_read_index <= reference_read_index + 1'b1;
            end
            if (push && !pop) begin
                reference_count <= reference_count + 1'b1;
            end else if (!push && pop) begin
                reference_count <= reference_count - 1'b1;
            end
        end

        if (push) begin
            reference_memory[reference_write_index] <= in_data;
        end
    end

    assume_input_stable_while_waiting: assume property (
        @(posedge clk) disable iff (!reset_n)
        in_valid && !in_ack |=> $stable(in_valid) && $stable(in_data)
    );

    assert_empty_has_no_output: assert property (
        @(posedge clk) disable iff (!reset_n)
        wrapper_empty |-> !wrapper_out_valid
    );

    assert_reference_does_not_overflow: assert property (
        @(posedge clk) disable iff (!reset_n)
        push && !pop |-> !reference_full
    );

    assert_reference_does_not_underflow: assert property (
        @(posedge clk) disable iff (!reset_n)
        pop |-> !reference_empty
    );

    assert_output_matches_reference: assert property (
        @(posedge clk) disable iff (!reset_n)
        wrapper_out_valid
            |-> !reference_empty && wrapper_out_data == reference_out_data
    );

    cover_accept_from_empty: cover property (
        @(posedge clk) disable iff (!reset_n)
        wrapper_empty && in_valid && wrapper_in_ack
    );

    cover_fill_and_drain: cover property (
        @(posedge clk) disable iff (!reset_n)
        fifo_count == FULL_COUNT && out_valid && out_ack
    );
endmodule
