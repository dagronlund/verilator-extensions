//!no_lint

interface stream_intf #(
    parameter type                 T       = logic,
    parameter logic [$bits(T)-1:0] T_LOGIC = 'b0
) (
    input wire clk,
    input wire rst
);

    logic valid /* verilator isolate_assignments*/;
    logic ready /* verilator isolate_assignments*/;

    T payload;

    modport out (
        output valid,
        input ready,
        output payload
    );

    modport in (
        input valid,
        output ready,
        input payload
    );

    modport view (
        input valid, ready,
        input payload
    );
endinterface
