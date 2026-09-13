module case_statements (
    input logic clk,
    input logic reset_n,
    input logic [2:0] selector,
    input logic [3:0] data,
    output logic [3:0] decoded,
    output logic [3:0] combinational_decoded,
    output logic [3:0] wildcard_decoded
);
    always_comb begin
        case (selector)
            3'd0, 3'd4: combinational_decoded = data ^ 4'ha;
            3'd1: combinational_decoded = data + 4'd3;
            3'd2: begin
                case (data[1:0])
                    2'd0: combinational_decoded = 4'd4;
                    2'd1, 2'd2: combinational_decoded = 4'd5;
                    default: combinational_decoded = 4'd6;
                endcase
            end
            default: combinational_decoded = 4'd7;
        endcase
    end

    always_comb begin
        casez (selector)
            3'b1??: wildcard_decoded = data;
            3'b01?: wildcard_decoded = data + 4'd1;
            3'b001: wildcard_decoded = data ^ 4'hf;
            default: wildcard_decoded = 4'd0;
        endcase
    end

    always_ff @(posedge clk) begin
        case (selector)
            3'd0, 3'd4: decoded <= data;
            3'd1: decoded <= data + 4'd1;
            3'd2: begin
                case (data[1:0])
                    2'd0: decoded <= 4'd8;
                    2'd1, 2'd2: decoded <= 4'd9;
                    default: decoded <= 4'd10;
                endcase
            end
            default: decoded <= 4'd15;
        endcase
    end

    assert_zero_case_captures_data: assert property (
        @(posedge clk) selector == 3'd0 |=> decoded == $past(data)
    );

    assert_wildcard_decode_matches: assert property (
        @(posedge clk) wildcard_decoded ==
            (selector[2] ? data :
             selector[1] ? data + 4'd1 :
             selector[0] ? data ^ 4'hf : 4'd0)
    );
endmodule
