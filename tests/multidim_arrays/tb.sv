module multidim_arrays (
    input  logic       clk,
    input  logic       reset_n,
    input  logic       write_enable,
    input  logic       write_row,
    input  logic [1:0] write_column,
    input  logic [3:0] write_data,
    input  logic       read_row,
    input  logic [1:0] read_column,
    output logic [3:0] read_data
);
    logic [3:0] memory [1:0][0:2];
    logic [-1:-4] negative_indices;

    assign read_data = memory[read_row][read_column];
    assign negative_indices = write_data;

    always_ff @(posedge clk) begin
        if (write_enable) begin
            memory[write_row][write_column] <= write_data;
        end
    end

    cover_write: cover property (@(posedge clk) write_enable);
    assert_negative_index: assert property (
        @(posedge clk) negative_indices[-1] == write_data[3]
    );
endmodule
