module {
  hw.module @combinational_loops(in %clk : i1, in %reset_n : i1, out total : i32) {
    %true = hw.constant true
    %c3_i32 = hw.constant 3 : i32
    %0 = seq.to_clock %clk
    %circt_past_valid = seq.firreg %true clock %0 preset 0 : i1
    %1 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %1, posedge %clk : i1
    hw.output %c3_i32 : i32
  }
}

