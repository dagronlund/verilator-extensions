#include "Vgecko_core_wrapper.h"
#include "verilated.h"

int main(int argc, char** argv) {
    VerilatedContext context;
    context.commandArgs(argc, argv);

    Vgecko_core_wrapper model{&context};
    model.eval_step();
    model.eval_end_step();
    model.final();

    return 0;
}
