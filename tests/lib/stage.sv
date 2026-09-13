module stage_valid_registered #(
    parameter int unsigned WIDTH = 4
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
    assign in_ack = !out_valid || out_ack;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            out_valid <= 1'b0;
        end else if (in_ack) begin
            out_valid <= in_valid;
        end

        if (in_valid && in_ack) begin
            out_data <= in_data;
        end
    end

endmodule

module stage_ack_registered #(
    parameter int unsigned WIDTH = 4
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
    logic stored_valid;
    logic [WIDTH-1:0] stored_data;

    assign in_ack = !stored_valid;
    assign out_valid = stored_valid || in_valid;
    assign out_data = stored_valid ? stored_data : in_data;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            stored_valid <= 1'b0;
        end else if (stored_valid) begin
            if (out_ack) begin
                stored_valid <= 1'b0;
            end
        end else if (in_valid && !out_ack) begin
            stored_valid <= 1'b1;
        end
        
        if (in_valid && !out_ack) begin
            stored_data <= in_data;
        end
    end

endmodule

module stage #(
    parameter int unsigned WIDTH = 4,
    parameter bit PIPELINE_VALID = 1'b1,
    parameter bit PIPELINE_ACK = 1'b0
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
    generate
        if (!PIPELINE_VALID && !PIPELINE_ACK) begin: pass_through
            assign in_ack = out_ack;
            assign out_valid = in_valid;
            assign out_data = in_data;
        end else if (PIPELINE_VALID && !PIPELINE_ACK) begin: valid_registered
            stage_valid_registered #(
                .WIDTH(WIDTH)
            ) stage_i (
                .clk(clk),
                .reset_n(reset_n),
                .in_valid(in_valid),
                .in_data(in_data),
                .out_data(out_data),
                .out_valid(out_valid),
                .in_ack(in_ack),
                .out_ack(out_ack)
            );
        end else if (!PIPELINE_VALID && PIPELINE_ACK) begin: ack_registered
            stage_ack_registered #(
                .WIDTH(WIDTH)
            ) stage_i (
                .clk(clk),
                .reset_n(reset_n),
                .in_valid(in_valid),
                .in_data(in_data),
                .out_data(out_data),
                .out_valid(out_valid),
                .in_ack(in_ack),
                .out_ack(out_ack)
            );
        end else begin: valid_ack_registered
            logic intermediate_valid;
            logic [WIDTH-1:0] intermediate_data;
            logic intermediate_ack;

            stage_ack_registered #(
                .WIDTH(WIDTH)
            ) ack_registered_i (
                .clk(clk),
                .reset_n(reset_n),
                .in_valid(in_valid),
                .in_data(in_data),
                .in_ack(in_ack),
                .out_valid(intermediate_valid),
                .out_data(intermediate_data),
                .out_ack(intermediate_ack)
            );

            stage_valid_registered #(
                .WIDTH(WIDTH)
            ) valid_registered_i (
                .clk(clk),
                .reset_n(reset_n),
                .in_valid(intermediate_valid),
                .in_data(intermediate_data),
                .in_ack(intermediate_ack),
                .out_valid(out_valid),
                .out_data(out_data),
                .out_ack(out_ack)
            );
        end
    endgenerate

    assert_stable_while_stalled: assert property (
        @(posedge clk) disable iff (!reset_n)
        out_valid && !out_ack |=> $stable(out_valid) && $stable(out_data)
    );
endmodule
