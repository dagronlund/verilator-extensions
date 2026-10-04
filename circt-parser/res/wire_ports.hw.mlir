module {
  hw.module @wire_ports(in %clk : i1, in %reset_n : i1, in %data_in : i4, out data_out : i4) {
    %true = hw.constant true
    %c0_i4 = hw.constant 0 : i4
    %0 = comb.mux bin %reset_n, %data_in, %c0_i4 : i4
    %1 = seq.to_clock %clk
    %registered_data = seq.firreg %0 clock %1 : i4
    %circt_past_valid = seq.firreg %true clock %1 preset 0 : i1
    %2 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %2, posedge %clk : i1
    hw.output %registered_data : i4
  }
}

