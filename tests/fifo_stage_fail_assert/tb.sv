module fifo_stage_tb #(
    parameter int unsigned WIDTH = 4,
    parameter int unsigned STAGES = 3,
    parameter int unsigned COUNT_WIDTH = $clog2(STAGES + 1)
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
    localparam logic [COUNT_WIDTH-1:0] FULL_COUNT = COUNT_WIDTH'(STAGES);

    logic [STAGES:0] valid_path;
    logic [STAGES:0] ack_path;
    logic [(STAGES+1)*WIDTH-1:0] data_path;
    logic [COUNT_WIDTH-1:0] fifo_count;
    logic reference_in_ack;
    logic reference_out_valid;
    logic [WIDTH-1:0] reference_out_data;
    logic reference_full;
    logic reference_empty;
    logic push;
    logic pop;

    assign valid_path[0] = in_valid;
    assign data_path[WIDTH-1:0] = in_data;
    assign in_ack = ack_path[0];
    assign out_valid = valid_path[STAGES];
    assign out_data = data_path[STAGES*WIDTH +: WIDTH];
    assign ack_path[STAGES] = out_ack;
    assign push = in_valid && in_ack;
    assign pop = out_valid && out_ack;

    generate
        genvar stage;
        for (stage = 0; stage < STAGES; stage = stage + 1) begin: stages
            stage #(
                .WIDTH(WIDTH)
            ) stage_i (
                .clk(clk),
                .reset_n(reset_n),
                .in_valid(valid_path[stage]),
                .in_data(data_path[stage*WIDTH +: WIDTH]),
                .in_ack(ack_path[stage]),
                .out_valid(valid_path[stage+1]),
                .out_data(data_path[(stage+1)*WIDTH +: WIDTH]),
                .out_ack(ack_path[stage+1])
            );
        end
    endgenerate

    fifo #(
        .WIDTH(WIDTH),
        .DEPTH(STAGES),
        .LEVEL_WIDTH(COUNT_WIDTH)
    ) reference_i (
        .clk(clk),
        .reset_n(reset_n),
        .in_valid(push),
        .in_data(in_data),
        .in_ack(reference_in_ack),
        .out_valid(reference_out_valid),
        .out_data(reference_out_data),
        .out_ack(pop),
        .full(reference_full),
        .empty(reference_empty),
        .level(fifo_count)
    );

    assume_input_stable_while_waiting: assume property (
        @(posedge clk) disable iff (!reset_n)
        in_valid && !in_ack |=> $stable(in_valid) && $stable(in_data)
    );

    assert_no_underflow: assert property (
        @(posedge clk) disable iff (!reset_n) pop |-> !reference_empty
    );

    assert_no_overflow: assert property (
        @(posedge clk) disable iff (!reset_n)
        push && !pop |-> !reference_full
    );

    assert_fifo_order: assert property (
        @(posedge clk) disable iff (!reset_n)
        pop |-> out_data == reference_out_data
    );

    assert_output_has_fifo_entry: assert property (
        @(posedge clk) disable iff (!reset_n) out_valid |-> fifo_count != 0
    );

    assert_capacity_matches_fifo: assert property (
        @(posedge clk) disable iff (!reset_n)
        in_ack == ((fifo_count < FULL_COUNT) || out_ack)
    );

    // Deliberately false: three accepted pushes without pops fill the pipeline,
    // so the shortest counterexample must advance through several clock edges.
    assert_fifo_never_fills: assert property (
        @(posedge clk) disable iff (!reset_n) fifo_count != FULL_COUNT
    );

    cover_fill_to_capacity: cover property (
        @(posedge clk) disable iff (!reset_n) fifo_count == FULL_COUNT
    );

    cover_fill_and_drain: cover property (
        @(posedge clk) disable iff (!reset_n)
        fifo_count == FULL_COUNT && pop
    );
endmodule
