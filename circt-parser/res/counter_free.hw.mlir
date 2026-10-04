module {
  hw.module @counter_free(in %clk : i1, in %reset_n : i1, out count : i4) {
    %false = hw.constant false
    %true = hw.constant true
    %c0_i4 = hw.constant 0 : i4
    %0 = seq.to_clock %clk
    %1 = seq.compreg %c0_i4, %0 : i4  
    %2 = comb.icmp eq %1, %c0_i4 : i4
    %3 = comb.xor %reset_n, %true : i1
    %4 = seq.firreg %reset_n clock %0 reset async %3, %false preset 0 : i1
    verif.clocked_assert %2 if %4, posedge %clk : i1
    verif.clocked_cover %false if %reset_n, posedge %clk : i1
    %circt_past_valid = seq.firreg %true clock %0 preset 0 : i1
    %5 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %5, posedge %clk : i1
    hw.output %c0_i4 : i4
  }
}

