module {
  hw.module @multidim_arrays(in %clk : i1, in %reset_n : i1, in %write_enable : i1, in %write_row : i1, in %write_column : i2, in %write_data : i4, in %read_row : i1, in %read_column : i2, out read_data : i4) {
    %true = hw.constant true
    %c-2_i2 = hw.constant -2 : i2
    %0 = hw.array_get %memory[%read_row] : !hw.array<2xarray<3xi4>>, i1
    %1 = comb.sub bin %c-2_i2, %read_column : i2
    %2 = hw.array_get %0[%1] : !hw.array<3xi4>, i2
    %3 = comb.sub bin %c-2_i2, %write_column : i2
    %4 = hw.array_get %memory[%write_row] : !hw.array<2xarray<3xi4>>, i1
    %5 = hw.array_inject %4[%3], %write_data : !hw.array<3xi4>, i2
    %6 = hw.array_inject %memory[%write_row], %5 : !hw.array<2xarray<3xi4>>, i1
    %7 = seq.to_clock %clk
    %8 = comb.mux bin %write_enable, %6, %memory : !hw.array<2xarray<3xi4>>
    %memory = seq.firreg %8 clock %7 : !hw.array<2xarray<3xi4>>
    verif.clocked_cover %write_enable, posedge %clk : i1
    %circt_past_valid = seq.firreg %true clock %7 preset 0 : i1
    %9 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %9, posedge %clk : i1
    hw.output %2 : i4
  }
}

