module {
  hw.module @data_types(in %clk : i1, in %reset_n : i1, in %data_in : i8, out data_out : i16, out bit_out : i1, out logic_out : i1, out byte_out : i8, out shortint_out : i16, out int_out : i32, out integer_out : i32, out longint_out : i64, out time_out : i64) {
    %c23100_i16 = hw.constant 23100 : i16
    %c-80_i8 = hw.constant -80 : i8
    %c0_i2 = hw.constant 0 : i2
    %true = hw.constant true
    %c1000000_i64 = hw.constant 1000000 : i64
    %0 = comb.replicate %data_in : (i8) -> i16
    %1 = hw.bitcast %0 : (i16) -> !hw.union<fields: !hw.struct<payload: !hw.array<2xi4>, state: i2, flags: i6>, raw: i16>
    %2 = comb.concat %data_in, %c-80_i8 : i8, i8
    %3 = hw.bitcast %2 : (i16) -> !hw.struct<payload: !hw.array<2xi4>, state: i2, flags: i6>
    %4 = hw.array_get %matrix_value[%c0_i2] : !hw.array<3xarray<2xstruct<payload: !hw.array<2xi4>, state: i2, flags: i6>>>, i2
    %5 = hw.array_inject %4[%true], %3 : !hw.array<2xstruct<payload: !hw.array<2xi4>, state: i2, flags: i6>>, i1
    %6 = hw.array_inject %matrix_value[%c0_i2], %5 : !hw.array<3xarray<2xstruct<payload: !hw.array<2xi4>, state: i2, flags: i6>>>, i2
    %7 = hw.union_extract %overlay_value["raw"] : !hw.union<fields: !hw.struct<payload: !hw.array<2xi4>, state: i2, flags: i6>, raw: i16>
    %8 = hw.array_get %4[%true] : !hw.array<2xstruct<payload: !hw.array<2xi4>, state: i2, flags: i6>>, i1
    %9 = hw.bitcast %8 : (!hw.struct<payload: !hw.array<2xi4>, state: i2, flags: i6>) -> i16
    %10 = comb.xor bin %7, %9, %c23100_i16 : i16
    %11 = comb.extract %data_in from 0 : (i8) -> i1
    %12 = comb.extract %data_in from 1 : (i8) -> i1
    %13 = comb.replicate %data_in : (i8) -> i32
    %14 = comb.replicate %data_in : (i8) -> i64
    %15 = comb.mul bin %14, %c1000000_i64 : i64
    %time_out = seq.firreg %15 clock %16 : i64
    %16 = seq.to_clock %clk
    %overlay_value = seq.firreg %1 clock %16 : !hw.union<fields: !hw.struct<payload: !hw.array<2xi4>, state: i2, flags: i6>, raw: i16>
    %matrix_value = seq.firreg %6 clock %16 : !hw.array<3xarray<2xstruct<payload: !hw.array<2xi4>, state: i2, flags: i6>>>
    %data_out = seq.firreg %10 clock %16 : i16
    %bit_out = seq.firreg %11 clock %16 : i1
    %logic_out = seq.firreg %12 clock %16 : i1
    %byte_out = seq.firreg %data_in clock %16 : i8
    %shortint_out = seq.firreg %0 clock %16 : i16
    %int_out = seq.firreg %13 clock %16 : i32
    %integer_out = seq.firreg %13 clock %16 : i32
    %longint_out = seq.firreg %14 clock %16 : i64
    %circt_past_valid = seq.firreg %true clock %16 preset 0 : i1
    %17 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %17, posedge %clk : i1
    hw.output %data_out, %bit_out, %logic_out, %byte_out, %shortint_out, %int_out, %integer_out, %longint_out, %time_out : i16, i1, i1, i8, i16, i32, i32, i64, i64
  }
}

