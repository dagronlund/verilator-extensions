module packet_switch #(
    parameter int unsigned WIDTH = 4,
    parameter int unsigned DEPTH = 3,
    parameter int unsigned COUNT_WIDTH = $clog2(DEPTH + 1),
    parameter int unsigned PACKET_WIDTH = WIDTH + 1
) (
    input  logic                   clk,
    input  logic                   reset_n,
    input  logic                   in0_valid,
    input  logic                   in0_dest,
    input  logic [WIDTH-1:0]       in0_data,
    output logic                   in0_ack,
    input  logic                   in1_valid,
    input  logic                   in1_dest,
    input  logic [WIDTH-1:0]       in1_data,
    output logic                   in1_ack,
    output logic                   out0_valid,
    output logic [WIDTH-1:0]       out0_data,
    input  logic                   out0_ack,
    output logic                   out1_valid,
    output logic [WIDTH-1:0]       out1_data,
    input  logic                   out1_ack,
    output logic [COUNT_WIDTH-1:0] count0,
    output logic [COUNT_WIDTH-1:0] count1,
    output logic                   grant00,
    output logic                   grant10,
    output logic                   grant01,
    output logic                   grant11,
    output logic                   pop0,
    output logic                   pop1,
    output logic                   push0,
    output logic                   push1,
    output logic                   rr0,
    output logic                   rr1
);
    localparam logic [COUNT_WIDTH-1:0] DEPTH_COUNT = COUNT_WIDTH'(DEPTH);

    logic [DEPTH*PACKET_WIDTH-1:0] entries0;
    logic [DEPTH*PACKET_WIDTH-1:0] entries1;
    logic                          head0_dest;
    logic [WIDTH-1:0]              head0_data;
    logic                          head1_dest;
    logic [WIDTH-1:0]              head1_data;
    logic                          request00;
    logic                          request10;
    logic                          request01;
    logic                          request11;
    logic                          arb_grant00;
    logic                          arb_grant10;
    logic                          arb_grant01;
    logic                          arb_grant11;
    logic                          hold0;
    logic                          hold0_source;
    logic                          hold1;
    logic                          hold1_source;

    assign head0_data = entries0[WIDTH-1:0];
    assign head0_dest = entries0[WIDTH];
    assign head1_data = entries1[WIDTH-1:0];
    assign head1_dest = entries1[WIDTH];

    assign request00 = count0 != 0 && !head0_dest;
    assign request10 = count1 != 0 && !head1_dest;
    assign request01 = count0 != 0 && head0_dest;
    assign request11 = count1 != 0 && head1_dest;

    assign arb_grant00 = request00 && (!request10 || !rr0);
    assign arb_grant10 = request10 && (!request00 || rr0);
    assign arb_grant01 = request01 && (!request11 || !rr1);
    assign arb_grant11 = request11 && (!request01 || rr1);

    assign grant00 = hold0 ? !hold0_source : arb_grant00;
    assign grant10 = hold0 ? hold0_source : arb_grant10;
    assign grant01 = hold1 ? !hold1_source : arb_grant01;
    assign grant11 = hold1 ? hold1_source : arb_grant11;

    assign out0_valid = grant00 || grant10;
    assign out0_data = grant00 ? head0_data : head1_data;
    assign out1_valid = grant01 || grant11;
    assign out1_data = grant01 ? head0_data : head1_data;

    assign pop0 = (grant00 && out0_ack) || (grant01 && out1_ack);
    assign pop1 = (grant10 && out0_ack) || (grant11 && out1_ack);
    assign in0_ack = count0 < DEPTH_COUNT || pop0;
    assign in1_ack = count1 < DEPTH_COUNT || pop1;
    assign push0 = in0_valid && in0_ack;
    assign push1 = in1_valid && in1_ack;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            entries0 <= '0;
            count0 <= '0;
        end else if (pop0 && push0) begin
            entries0 <= {{in0_dest, in0_data}, entries0[DEPTH*PACKET_WIDTH-1:PACKET_WIDTH]};
        end else if (pop0) begin
            entries0 <= entries0 >> PACKET_WIDTH;
            count0 <= count0 - 1'b1;
        end else if (push0) begin
            if (count0 == 0) begin
                entries0[PACKET_WIDTH-1:0] <= {in0_dest, in0_data};
            end else if (count0 == 1) begin
                entries0[2*PACKET_WIDTH-1:PACKET_WIDTH] <= {in0_dest, in0_data};
            end else begin
                entries0[3*PACKET_WIDTH-1:2*PACKET_WIDTH] <= {in0_dest, in0_data};
            end
            count0 <= count0 + 1'b1;
        end
    end

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            entries1 <= '0;
            count1 <= '0;
        end else if (pop1 && push1) begin
            entries1 <= {{in1_dest, in1_data}, entries1[DEPTH*PACKET_WIDTH-1:PACKET_WIDTH]};
        end else if (pop1) begin
            entries1 <= entries1 >> PACKET_WIDTH;
            count1 <= count1 - 1'b1;
        end else if (push1) begin
            if (count1 == 0) begin
                entries1[PACKET_WIDTH-1:0] <= {in1_dest, in1_data};
            end else if (count1 == 1) begin
                entries1[2*PACKET_WIDTH-1:PACKET_WIDTH] <= {in1_dest, in1_data};
            end else begin
                entries1[3*PACKET_WIDTH-1:2*PACKET_WIDTH] <= {in1_dest, in1_data};
            end
            count1 <= count1 + 1'b1;
        end
    end

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            rr0 <= 1'b0;
            rr1 <= 1'b0;
        end else begin
            if (request00 && request10 && out0_ack) begin
                rr0 <= grant00;
            end
            if (request01 && request11 && out1_ack) begin
                rr1 <= grant01;
            end
        end
    end

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            hold0 <= 1'b0;
            hold0_source <= 1'b0;
            hold1 <= 1'b0;
            hold1_source <= 1'b0;
        end else begin
            if (hold0) begin
                if (out0_ack) begin
                    hold0 <= 1'b0;
                end
            end else if (out0_valid && !out0_ack) begin
                hold0 <= 1'b1;
                hold0_source <= grant10;
            end

            if (hold1) begin
                if (out1_ack) begin
                    hold1 <= 1'b0;
                end
            end else if (out1_valid && !out1_ack) begin
                hold1 <= 1'b1;
                hold1_source <= grant11;
            end
        end
    end
endmodule

module ingress_reference #(
    parameter int unsigned WIDTH = 4,
    parameter int unsigned DEPTH = 3,
    parameter int unsigned COUNT_WIDTH = $clog2(DEPTH + 1),
    parameter int unsigned PACKET_WIDTH = WIDTH + 1
) (
    input  logic                   clk,
    input  logic                   reset_n,
    input  logic                   push,
    input  logic                   push_dest,
    input  logic [WIDTH-1:0]       push_data,
    input  logic                   pop,
    output logic                   head_dest,
    output logic [WIDTH-1:0]       head_data,
    output logic [COUNT_WIDTH-1:0] count
);
    localparam logic [COUNT_WIDTH-1:0] DEPTH_COUNT = COUNT_WIDTH'(DEPTH);

    logic [DEPTH*PACKET_WIDTH-1:0] entries;

    assign head_data = entries[WIDTH-1:0];
    assign head_dest = entries[WIDTH];

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            entries <= '0;
            count <= '0;
        end else if (pop && push) begin
            entries <= {{push_dest, push_data}, entries[DEPTH*PACKET_WIDTH-1:PACKET_WIDTH]};
        end else if (pop) begin
            entries <= entries >> PACKET_WIDTH;
            count <= count - 1'b1;
        end else if (push) begin
            if (count == 0) begin
                entries[PACKET_WIDTH-1:0] <= {push_dest, push_data};
            end else if (count == 1) begin
                entries[2*PACKET_WIDTH-1:PACKET_WIDTH] <= {push_dest, push_data};
            end else begin
                entries[3*PACKET_WIDTH-1:2*PACKET_WIDTH] <= {push_dest, push_data};
            end
            count <= count + 1'b1;
        end
    end

    assert_reference_no_underflow: assert property (
        @(posedge clk) disable iff (!reset_n) pop |-> count != 0
    );

    assert_reference_no_overflow: assert property (
        @(posedge clk) disable iff (!reset_n) push && !pop |-> count < DEPTH_COUNT
    );
endmodule

module packet_switch_tb #(
    parameter int unsigned WIDTH = 4,
    parameter int unsigned DEPTH = 3,
    parameter int unsigned COUNT_WIDTH = $clog2(DEPTH + 1)
) (
    input  logic             clk,
    input  logic             reset_n,
    input  logic             in0_valid,
    input  logic             in0_dest,
    input  logic [WIDTH-1:0] in0_data,
    output logic             in0_ack,
    input  logic             in1_valid,
    input  logic             in1_dest,
    input  logic [WIDTH-1:0] in1_data,
    output logic             in1_ack,
    output logic             out0_valid,
    output logic [WIDTH-1:0] out0_data,
    input  logic             out0_ack,
    output logic             out1_valid,
    output logic [WIDTH-1:0] out1_data,
    input  logic             out1_ack
);
    localparam logic [COUNT_WIDTH-1:0] DEPTH_COUNT = COUNT_WIDTH'(DEPTH);

    logic [COUNT_WIDTH-1:0] dut_count0;
    logic [COUNT_WIDTH-1:0] dut_count1;
    logic [COUNT_WIDTH-1:0] ref_count0;
    logic [COUNT_WIDTH-1:0] ref_count1;
    logic                   ref_head0_dest;
    logic [WIDTH-1:0]       ref_head0_data;
    logic                   ref_head1_dest;
    logic [WIDTH-1:0]       ref_head1_data;
    logic                   grant00;
    logic                   grant10;
    logic                   grant01;
    logic                   grant11;
    logic                   pop0;
    logic                   pop1;
    logic                   push0;
    logic                   push1;
    logic                   rr0;
    logic                   rr1;
    logic                   request00;
    logic                   request10;
    logic                   request01;
    logic                   request11;
    logic [1:0]             stall_history;

    assign request00 = ref_count0 != 0 && !ref_head0_dest;
    assign request10 = ref_count1 != 0 && !ref_head1_dest;
    assign request01 = ref_count0 != 0 && ref_head0_dest;
    assign request11 = ref_count1 != 0 && ref_head1_dest;

    packet_switch #(
        .WIDTH(WIDTH),
        .DEPTH(DEPTH),
        .COUNT_WIDTH(COUNT_WIDTH)
    ) dut (
        .clk(clk),
        .reset_n(reset_n),
        .in0_valid(in0_valid),
        .in0_dest(in0_dest),
        .in0_data(in0_data),
        .in0_ack(in0_ack),
        .in1_valid(in1_valid),
        .in1_dest(in1_dest),
        .in1_data(in1_data),
        .in1_ack(in1_ack),
        .out0_valid(out0_valid),
        .out0_data(out0_data),
        .out0_ack(out0_ack),
        .out1_valid(out1_valid),
        .out1_data(out1_data),
        .out1_ack(out1_ack),
        .count0(dut_count0),
        .count1(dut_count1),
        .grant00(grant00),
        .grant10(grant10),
        .grant01(grant01),
        .grant11(grant11),
        .pop0(pop0),
        .pop1(pop1),
        .push0(push0),
        .push1(push1),
        .rr0(rr0),
        .rr1(rr1)
    );

    ingress_reference #(
        .WIDTH(WIDTH),
        .DEPTH(DEPTH),
        .COUNT_WIDTH(COUNT_WIDTH)
    ) reference0 (
        .clk(clk),
        .reset_n(reset_n),
        .push(push0),
        .push_dest(in0_dest),
        .push_data(in0_data),
        .pop(pop0),
        .head_dest(ref_head0_dest),
        .head_data(ref_head0_data),
        .count(ref_count0)
    );

    ingress_reference #(
        .WIDTH(WIDTH),
        .DEPTH(DEPTH),
        .COUNT_WIDTH(COUNT_WIDTH)
    ) reference1 (
        .clk(clk),
        .reset_n(reset_n),
        .push(push1),
        .push_dest(in1_dest),
        .push_data(in1_data),
        .pop(pop1),
        .head_dest(ref_head1_dest),
        .head_data(ref_head1_data),
        .count(ref_count1)
    );

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            stall_history <= '0;
        end else begin
            stall_history <= {stall_history[0], out0_valid && !out0_ack};
        end
    end

    assume_input0_stable_while_waiting: assume property (
        @(posedge clk) disable iff (!reset_n)
        in0_valid && !in0_ack |=>
            $stable(in0_valid) && $stable(in0_dest) && $stable(in0_data)
    );

    assume_input1_stable_while_waiting: assume property (
        @(posedge clk) disable iff (!reset_n)
        in1_valid && !in1_ack |=>
            $stable(in1_valid) && $stable(in1_dest) && $stable(in1_data)
    );

    assert_reset_empties_queues: assert property (
        @(posedge clk) !reset_n |=>
            dut_count0 == 0 && dut_count1 == 0 && ref_count0 == 0 && ref_count1 == 0
    );

    assert_output0_stable_while_stalled: assert property (
        @(posedge clk) disable iff (!reset_n)
        out0_valid && !out0_ack |=> $stable(out0_valid) && $stable(out0_data)
    );

    assert_output1_stable_while_stalled: assert property (
        @(posedge clk) disable iff (!reset_n)
        out1_valid && !out1_ack |=> $stable(out1_valid) && $stable(out1_data)
    );

    assert_input0_occupancy_matches: assert property (
        @(posedge clk) disable iff (!reset_n) dut_count0 == ref_count0
    );

    assert_input1_occupancy_matches: assert property (
        @(posedge clk) disable iff (!reset_n) dut_count1 == ref_count1
    );

    wire strengthen_input0_capacity_occupancy = dut_count0 == ref_count0;

    assert_input0_capacity: assert property (
        @(posedge clk) disable iff (!reset_n)
        in0_ack == ((ref_count0 < DEPTH_COUNT) || pop0)
    );

    assert_input0_capacity_strengthened: assert property (
        @(posedge clk) disable iff (!reset_n)
        strengthen_input0_capacity_occupancy &&
            (in0_ack == ((ref_count0 < DEPTH_COUNT) || pop0))
    );

    wire strengthen_input1_capacity_occupancy = dut_count1 == ref_count1;

    assert_input1_capacity: assert property (
        @(posedge clk) disable iff (!reset_n)
        in1_ack == ((ref_count1 < DEPTH_COUNT) || pop1)
    );

    assert_input1_capacity_strengthened: assert property (
        @(posedge clk) disable iff (!reset_n)
        strengthen_input1_capacity_occupancy &&
            (in1_ack == ((ref_count1 < DEPTH_COUNT) || pop1))
    );

    assert_output0_grant_exclusive: assert property (
        @(posedge clk) disable iff (!reset_n) !(grant00 && grant10)
    );

    assert_output1_grant_exclusive: assert property (
        @(posedge clk) disable iff (!reset_n) !(grant01 && grant11)
    );

    assert_input0_grant_exclusive: assert property (
        @(posedge clk) disable iff (!reset_n) !(grant00 && grant01)
    );

    assert_input1_grant_exclusive: assert property (
        @(posedge clk) disable iff (!reset_n) !(grant10 && grant11)
    );

    assert_grant00_targets_output0: assert property (
        @(posedge clk) disable iff (!reset_n) grant00 |-> request00
    );

    assert_grant10_targets_output0: assert property (
        @(posedge clk) disable iff (!reset_n) grant10 |-> request10
    );

    assert_grant01_targets_output1: assert property (
        @(posedge clk) disable iff (!reset_n) grant01 |-> request01
    );

    assert_grant11_targets_output1: assert property (
        @(posedge clk) disable iff (!reset_n) grant11 |-> request11
    );

    assert_output0_uses_reference_data: assert property (
        @(posedge clk) disable iff (!reset_n)
        out0_valid |-> out0_data == (grant00 ? ref_head0_data : ref_head1_data)
    );

    assert_output1_uses_reference_data: assert property (
        @(posedge clk) disable iff (!reset_n)
        out1_valid |-> out1_data == (grant01 ? ref_head0_data : ref_head1_data)
    );

    assert_grant00_matches_policy: assert property (
        @(posedge clk) disable iff (!reset_n)
        !$past(out0_valid && !out0_ack) |->
            grant00 == (request00 && (!request10 || !rr0))
    );

    assert_grant10_matches_policy: assert property (
        @(posedge clk) disable iff (!reset_n)
        !$past(out0_valid && !out0_ack) |->
            grant10 == (request10 && (!request00 || rr0))
    );

    assert_grant01_matches_policy: assert property (
        @(posedge clk) disable iff (!reset_n)
        !$past(out1_valid && !out1_ack) |->
            grant01 == (request01 && (!request11 || !rr1))
    );

    assert_grant11_matches_policy: assert property (
        @(posedge clk) disable iff (!reset_n)
        !$past(out1_valid && !out1_ack) |->
            grant11 == (request11 && (!request01 || rr1))
    );

    assert_rr0_stable_without_contended_transfer: assert property (
        @(posedge clk) disable iff (!reset_n)
        !(request00 && request10 && out0_ack) |=> $stable(rr0)
    );

    assert_rr1_stable_without_contended_transfer: assert property (
        @(posedge clk) disable iff (!reset_n)
        !(request01 && request11 && out1_ack) |=> $stable(rr1)
    );

    assert_rr0_prefers_loser: assert property (
        @(posedge clk) disable iff (!reset_n)
        request00 && request10 && out0_ack |=> rr0 == $past(grant00)
    );

    assert_rr1_prefers_loser: assert property (
        @(posedge clk) disable iff (!reset_n)
        request01 && request11 && out1_ack |=> rr1 == $past(grant01)
    );

    cover_full_capacity: cover property (
        @(posedge clk) disable iff (!reset_n)
        ref_count0 == DEPTH_COUNT && ref_count1 == DEPTH_COUNT
    );

    cover_same_output_contention: cover property (
        @(posedge clk) disable iff (!reset_n)
        (request00 && request10) || (request01 && request11)
    );

    cover_parallel_outputs: cover property (
        @(posedge clk) disable iff (!reset_n)
        out0_valid && out0_ack && out1_valid && out1_ack
    );

    cover_stall_then_drain: cover property (
        @(posedge clk) disable iff (!reset_n)
        stall_history == 2'b11 && out0_valid && out0_ack
    );

    cover_full_pop_push_replacement: cover property (
        @(posedge clk) disable iff (!reset_n)
        (ref_count0 == DEPTH_COUNT && pop0 && push0) ||
        (ref_count1 == DEPTH_COUNT && pop1 && push1)
    );
endmodule
