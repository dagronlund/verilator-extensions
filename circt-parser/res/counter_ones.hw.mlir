module {
  hw.module @counter_ones(in %clk : i1, in %reset_n : i1, in %enable : i1, out count : i4) {
    %false = hw.constant false
    %true = hw.constant true
    %c1_i4 = hw.constant 1 : i4
    %c-1_i4 = hw.constant -1 : i4
    %0 = comb.add bin %count, %c1_i4 : i4
    %1 = comb.and bin %enable, %reset_n : i1
    %2 = comb.mux bin %1, %0, %c-1_i4 : i4
    %3 = comb.xor bin %enable, %true : i1
    %4 = comb.and bin %reset_n, %3 : i1
    %5 = seq.to_clock %clk
    %6 = comb.mux bin %4, %count, %2 : i4
    %count = seq.firreg %6 clock %5 : i4
    %7 = seq.compreg %count, %5 : i4  
    %8 = comb.icmp eq %7, %count : i4
    %9 = comb.and %reset_n, %3 : i1
    %10 = comb.xor %reset_n, %true : i1
    %11 = seq.firreg %9 clock %5 reset async %10, %false preset 0 : i1
    verif.clocked_assert %8 if %11, posedge %clk : i1
    %circt_past_valid = seq.firreg %true clock %5 preset 0 : i1
    %12 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %12, posedge %clk : i1
    hw.output %count : i4
  }
}

