module {
  hw.module private @public_counter(in %clk : i1, in %reset_n : i1, in %enable : i1, out count : i4) {
    %true = hw.constant true
    %c1_i4 = hw.constant 1 : i4
    %c0_i4 = hw.constant 0 : i4
    %0 = comb.add bin %state, %c1_i4 : i4
    %1 = comb.and bin %enable, %reset_n : i1
    %2 = comb.mux bin %1, %0, %c0_i4 : i4
    %3 = comb.xor bin %enable, %true : i1
    %4 = comb.and bin %reset_n, %3 : i1
    %5 = seq.to_clock %clk
    %6 = comb.mux bin %4, %state, %2 : i4
    %state = seq.firreg %6 clock %5 : i4
    hw.output %state : i4
  }
  hw.module @public_submodules(in %clk : i1, in %reset_n : i1, in %enable0 : i1, in %enable1 : i1, out count0 : i4, out count1 : i4) {
    %true = hw.constant true
    %counter0.count = hw.instance "counter0" @public_counter(clk: %clk: i1, reset_n: %reset_n: i1, enable: %enable0: i1) -> (count: i4)
    %counter1.count = hw.instance "counter1" @public_counter(clk: %clk: i1, reset_n: %reset_n: i1, enable: %enable1: i1) -> (count: i4)
    %0 = seq.to_clock %clk
    %circt_past_valid = seq.firreg %true clock %0 preset 0 : i1
    %1 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %1, posedge %clk : i1
    hw.output %counter0.count, %counter1.count : i4, i4
  }
}

