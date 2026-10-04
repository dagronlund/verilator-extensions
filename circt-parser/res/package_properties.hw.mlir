module {
  hw.module @package_properties(in %clk : i1, in %reset_n : i1, in %enable : i1, out count : i4) {
    %false = hw.constant false
    %true = hw.constant true
    %c1_i4 = hw.constant 1 : i4
    %c0_i4 = hw.constant 0 : i4
    %0 = comb.add bin %count, %c1_i4 : i4
    %1 = comb.and bin %enable, %reset_n : i1
    %2 = comb.mux bin %1, %0, %c0_i4 : i4
    %3 = comb.xor bin %enable, %true : i1
    %4 = comb.and bin %reset_n, %3 : i1
    %5 = seq.to_clock %clk
    %6 = comb.mux bin %4, %count, %2 : i4
    %count = seq.firreg %6 clock %5 : i4
    %7 = comb.xor bin %reset_n, %true : i1
    %8 = comb.icmp eq %count, %c0_i4 : i4
    %9 = seq.firreg %7 clock %5 preset 0 : i1
    verif.clocked_assert %8 if %9, posedge %clk : i1
    %10 = seq.compreg %count, %5 : i4  
    %11 = comb.icmp eq %10, %count : i4
    %12 = comb.and %reset_n, %3 : i1
    %13 = comb.xor %reset_n, %true : i1
    %14 = seq.firreg %12 clock %5 reset async %13, %false preset 0 : i1
    verif.clocked_assert %11 if %14, posedge %clk : i1
    %circt_past_valid = seq.firreg %true clock %5 preset 0 : i1
    %15 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %15, posedge %clk : i1
    hw.output %count : i4
  }
}

