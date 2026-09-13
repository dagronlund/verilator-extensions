module data_types #(
    parameter type VALUE_T = logic [7:0],
    parameter IMPLICIT_VALUE = 8'h3c
) (
    input logic clk,
    input logic reset_n,
    input logic [7:0] data_in,
    output logic [15:0] data_out,
    output bit bit_out,
    output logic logic_out,
    output byte byte_out,
    output shortint shortint_out,
    output int int_out,
    output integer integer_out,
    output longint longint_out,
    output time time_out
);
    typedef logic signed [7:0] signed_byte_t;
    typedef signed_byte_t alias_t;
    typedef alias_t chained_alias_t;
    typedef enum logic [1:0] {
        IDLE = 2'b00,
        RUNNING = 2'b01,
        STOPPED = 2'b10
    } state_t;
    typedef logic [0:1][3:0] ascending_packed_t;
    typedef logic [1:0][3:0] descending_packed_t;
    typedef struct packed {
        ascending_packed_t payload;
        state_t state;
        logic [5:0] flags;
    } record_t;
    typedef union packed {
        record_t fields;
        logic [15:0] raw;
    } overlay_t;
    typedef record_t row_t [1:0];
    typedef row_t matrix_t [0:2];

    chained_alias_t alias_value;
    VALUE_T parameter_value;
    descending_packed_t descending_value;
    record_t record_value;
    overlay_t overlay_value;
    matrix_t matrix_value;
    const logic [7:0] const_value = 8'h5a;

    always_ff @(posedge clk) begin
        alias_value <= data_in;
        parameter_value <= data_in;
        descending_value <= data_in;
        record_value <= {data_in, 2'b01, 6'b001100};
        overlay_value <= {data_in, data_in};
        matrix_value[2][1] <= {data_in, 2'b10, 6'b110000};
        data_out <= overlay_value.raw ^ matrix_value[2][1]
            ^ {const_value, IMPLICIT_VALUE};
        bit_out <= data_in[0];
        logic_out <= data_in[1];
        byte_out <= data_in;
        shortint_out <= {data_in, data_in};
        int_out <= {data_in, data_in, data_in, data_in};
        integer_out <= {data_in, data_in, data_in, data_in};
        longint_out <= {
            data_in, data_in, data_in, data_in,
            data_in, data_in, data_in, data_in
        };
        time_out <= {
            data_in, data_in, data_in, data_in,
            data_in, data_in, data_in, data_in
        };
    end
endmodule
