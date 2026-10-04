module {
  hw.module @signed_operations(in %clk : i1, in %reset_n : i1, in %lhs : i4, in %rhs : i4, out captured_sum : i4, out captured_unsigned_product : i4, out captured_signed_product : i4, out captured_unsigned_quotient : i4, out captured_signed_quotient : i4) {
    %true = hw.constant true
    %c-8_i4 = hw.constant -8 : i4
    %0 = comb.add bin %lhs, %rhs : i4
    %1 = comb.mul bin %lhs, %rhs : i4
    %2 = comb.divu bin %lhs, %rhs : i4
    %3 = comb.divs bin %lhs, %rhs : i4
    %4 = seq.to_clock %clk
    %captured_sum = seq.firreg %0 clock %4 : i4
    %captured_unsigned_product = seq.firreg %1 clock %4 : i4
    %captured_signed_product = seq.firreg %1 clock %4 : i4
    %captured_unsigned_quotient = seq.firreg %2 clock %4 : i4
    %captured_signed_quotient = seq.firreg %3 clock %4 : i4
    %5 = comb.icmp slt %lhs, %rhs : i4
    %6 = comb.xor bin %lhs, %c-8_i4 : i4
    %7 = comb.xor bin %rhs, %c-8_i4 : i4
    %8 = comb.icmp ult %6, %7 : i4
    %9 = comb.icmp eq %5, %8 : i1
    verif.clocked_assert %9, posedge %clk : i1
    %10 = comb.icmp sle %lhs, %rhs : i4
    %11 = comb.icmp ule %6, %7 : i4
    %12 = comb.icmp eq %10, %11 : i1
    verif.clocked_assert %12, posedge %clk : i1
    %13 = comb.icmp sgt %lhs, %rhs : i4
    %14 = comb.icmp ugt %6, %7 : i4
    %15 = comb.icmp eq %13, %14 : i1
    verif.clocked_assert %15, posedge %clk : i1
    %16 = comb.icmp sge %lhs, %rhs : i4
    %17 = comb.icmp uge %6, %7 : i4
    %18 = comb.icmp eq %16, %17 : i1
    verif.clocked_assert %18, posedge %clk : i1
    %circt_past_valid = seq.firreg %true clock %4 preset 0 : i1
    %19 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %19, posedge %clk : i1
    hw.output %captured_sum, %captured_unsigned_product, %captured_signed_product, %captured_unsigned_quotient, %captured_signed_quotient : i4, i4, i4, i4, i4
  }
}

