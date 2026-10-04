module {
  hw.module @fifo_stage_tb(in %clk : i1, in %reset_n : i1, in %in_valid : i1, in %in_data : i4, out in_ack : i1, out out_valid : i1, out out_data : i4, in %out_ack : i1) {
    %c0_i2 = hw.constant 0 : i2
    %true = hw.constant true
    %c-1_i2 = hw.constant -1 : i2
    %false = hw.constant false
    %stages_0.stage_i.in_ack, %stages_0.stage_i.out_valid, %stages_0.stage_i.out_data = hw.instance "stages_0.stage_i" @stage(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %in_valid: i1, in_data: %in_data: i4, out_ack: %stages_1.stage_i.in_ack: i1) -> (in_ack: i1, out_valid: i1, out_data: i4)
    %stages_1.stage_i.in_ack, %stages_1.stage_i.out_valid, %stages_1.stage_i.out_data = hw.instance "stages_1.stage_i" @stage(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %stages_0.stage_i.out_valid: i1, in_data: %stages_0.stage_i.out_data: i4, out_ack: %stages_2.stage_i.in_ack: i1) -> (in_ack: i1, out_valid: i1, out_data: i4)
    %stages_2.stage_i.in_ack, %stages_2.stage_i.out_valid, %stages_2.stage_i.out_data = hw.instance "stages_2.stage_i" @stage(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %stages_1.stage_i.out_valid: i1, in_data: %stages_1.stage_i.out_data: i4, out_ack: %out_ack: i1) -> (in_ack: i1, out_valid: i1, out_data: i4)
    %reference_i.in_ack, %reference_i.out_valid, %reference_i.out_data, %reference_i.full, %reference_i.empty, %reference_i.level = hw.instance "reference_i" @fifo(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %0: i1, in_data: %in_data: i4, out_ack: %1: i1) -> (in_ack: i1, out_valid: i1, out_data: i4, full: i1, empty: i1, level: i2) {sv.namehint = "reference_in_ack"}
    %0 = comb.and bin %in_valid, %stages_0.stage_i.in_ack : i1
    %1 = comb.and bin %stages_2.stage_i.out_valid, %out_ack : i1
    %2 = seq.to_clock %clk
    %3 = seq.compreg %in_valid, %2 : i1  
    %4 = seq.compreg %in_data, %2 : i4  
    %5 = comb.xor bin %stages_0.stage_i.in_ack, %true : i1
    %6 = comb.icmp eq %3, %in_valid : i1
    %7 = comb.icmp eq %4, %in_data : i4
    %8 = comb.and bin %6, %7 : i1
    %9 = comb.and %reset_n, %in_valid, %5 : i1
    %10 = comb.xor %reset_n, %true : i1
    %11 = seq.firreg %9 clock %2 reset async %10, %false preset 0 : i1
    verif.clocked_assume %8 if %11, posedge %clk : i1
    %12 = comb.xor bin %reference_i.empty, %true : i1
    %13 = comb.and %reset_n, %1 : i1
    verif.clocked_assert %12 if %13, posedge %clk : i1
    %14 = comb.xor bin %1, %true : i1
    %15 = comb.xor bin %reference_i.full, %true : i1
    %16 = comb.and %reset_n, %0, %14 : i1
    verif.clocked_assert %15 if %16, posedge %clk : i1
    %17 = comb.icmp eq %stages_2.stage_i.out_data, %reference_i.out_data : i4
    verif.clocked_assert %17 if %13, posedge %clk : i1
    %18 = comb.icmp ne %reference_i.level, %c0_i2 : i2
    %19 = comb.and %reset_n, %stages_2.stage_i.out_valid : i1
    verif.clocked_assert %18 if %19, posedge %clk : i1
    %20 = comb.icmp ne %reference_i.level, %c-1_i2 : i2
    %21 = comb.or bin %20, %out_ack : i1
    %22 = comb.icmp eq %stages_0.stage_i.in_ack, %21 : i1
    verif.clocked_assert %22 if %reset_n, posedge %clk : i1
    verif.clocked_assert %20 if %reset_n, posedge %clk : i1
    %23 = comb.icmp eq %reference_i.level, %c-1_i2 : i2
    verif.clocked_cover %23 if %reset_n, posedge %clk : i1
    %24 = comb.and bin %23, %1 : i1
    verif.clocked_cover %24 if %reset_n, posedge %clk : i1
    %circt_past_valid = seq.firreg %true clock %2 preset 0 : i1
    %25 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %25, posedge %clk : i1
    hw.output %stages_0.stage_i.in_ack, %stages_2.stage_i.out_valid, %stages_2.stage_i.out_data : i1, i1, i4
  }
  hw.module private @stage_valid_registered(in %clk : i1, in %reset_n : i1, in %in_valid : i1, in %in_data : i4, out in_ack : i1, out out_valid : i1, out out_data : i4, in %out_ack : i1) {
    %true = hw.constant true
    %0 = comb.xor bin %out_valid, %true : i1
    %1 = comb.or bin %0, %out_ack : i1
    %2 = comb.and bin %reset_n, %1, %in_valid : i1
    %3 = comb.xor bin %reset_n, %true : i1
    %4 = comb.or bin %3, %1 : i1
    %5 = comb.and bin %in_valid, %1 : i1
    %6 = seq.to_clock %clk
    %7 = comb.mux bin %4, %2, %out_valid : i1
    %out_valid = seq.firreg %7 clock %6 : i1
    %8 = comb.mux bin %5, %in_data, %out_data : i4
    %out_data = seq.firreg %8 clock %6 : i4
    hw.output %1, %out_valid, %out_data : i1, i1, i4
  }
  hw.module private @stage(in %clk : i1, in %reset_n : i1, in %in_valid : i1, in %in_data : i4, out in_ack : i1, out out_valid : i1, out out_data : i4, in %out_ack : i1) {
    %false = hw.constant false
    %true = hw.constant true
    %valid_registered.stage_i.in_ack, %valid_registered.stage_i.out_valid, %valid_registered.stage_i.out_data = hw.instance "valid_registered.stage_i" @stage_valid_registered(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %in_valid: i1, in_data: %in_data: i4, out_ack: %out_ack: i1) -> (in_ack: i1, out_valid: i1, out_data: i4)
    %0 = seq.to_clock %clk
    %1 = seq.compreg %valid_registered.stage_i.out_valid, %0 : i1  
    %2 = seq.compreg %valid_registered.stage_i.out_data, %0 : i4  
    %3 = comb.xor bin %out_ack, %true : i1
    %4 = comb.icmp eq %1, %valid_registered.stage_i.out_valid : i1
    %5 = comb.icmp eq %2, %valid_registered.stage_i.out_data : i4
    %6 = comb.and bin %4, %5 : i1
    %7 = comb.and %reset_n, %valid_registered.stage_i.out_valid, %3 : i1
    %8 = comb.xor %reset_n, %true : i1
    %9 = seq.firreg %7 clock %0 reset async %8, %false preset 0 : i1
    verif.clocked_assert %6 if %9, posedge %clk : i1
    hw.output %valid_registered.stage_i.in_ack, %valid_registered.stage_i.out_valid, %valid_registered.stage_i.out_data : i1, i1, i4
  }
  hw.module private @fifo(in %clk : i1, in %reset_n : i1, in %in_valid : i1, in %in_data : i4, out in_ack : i1, out out_valid : i1, out out_data : i4, in %out_ack : i1, out full : i1, out empty : i1, out level : i2) {
    %false = hw.constant false
    %true = hw.constant true
    %c1_i2 = hw.constant 1 : i2
    %c-2_i2 = hw.constant -2 : i2
    %c0_i2 = hw.constant 0 : i2
    %c-1_i2 = hw.constant -1 : i2
    %0 = comb.icmp eq %level, %c-1_i2 : i2
    %1 = comb.icmp eq %level, %c0_i2 : i2
    %2 = comb.xor bin %0, %true : i1
    %3 = comb.or bin %2, %out_ack : i1
    %4 = comb.xor bin %1, %true : i1
    %5 = comb.sub bin %c-1_i2, %read_pointer : i2
    %6 = hw.array_get %memory[%5] : !hw.array<4xi4>, i2
    %7 = comb.and bin %in_valid, %3 : i1
    %8 = comb.and bin %4, %out_ack : i1
    %9 = comb.icmp eq %write_pointer, %c-2_i2 : i2
    %10 = comb.add bin %write_pointer, %c1_i2 : i2
    %11 = comb.and bin %7, %9 : i1
    %12 = comb.mux bin %11, %c0_i2, %write_pointer : i2
    %13 = comb.xor bin %9, %true : i1
    %14 = comb.and bin %13, %7 : i1
    %15 = comb.mux bin %14, %10, %12 : i2
    %16 = comb.icmp eq %read_pointer, %c-2_i2 : i2
    %17 = comb.add bin %read_pointer, %c1_i2 : i2
    %18 = comb.and bin %8, %16 : i1
    %19 = comb.mux bin %18, %c0_i2, %read_pointer : i2
    %20 = comb.xor bin %16, %true : i1
    %21 = comb.and bin %20, %8 : i1
    %22 = comb.mux bin %21, %17, %19 : i2
    %23 = comb.xor bin %8, %true : i1
    %24 = comb.and bin %7, %23 : i1
    %25 = comb.add bin %level, %c1_i2 : i2
    %26 = comb.xor bin %7, %true : i1
    %27 = comb.and bin %26, %8 : i1
    %28 = comb.add bin %level, %c-1_i2 : i2
    %29 = comb.and bin %7, %reset_n : i1
    %30 = comb.and bin %13, %29 : i1
    %31 = comb.and bin %9, %29 : i1
    %32 = comb.and bin %26, %reset_n : i1
    %33 = comb.or bin %31, %30, %32 : i1
    %34 = comb.and bin %8, %33 : i1
    %35 = comb.and bin %20, %34 : i1
    %36 = comb.and bin %16, %34 : i1
    %37 = comb.and bin %23, %33 : i1
    %38 = comb.or bin %36, %35, %37 : i1
    %39 = comb.and bin %24, %38 : i1
    %40 = comb.xor bin %39, %true : i1
    %41 = comb.mux bin %39, %25, %c0_i2 : i2
    %42 = comb.xor bin %24, %true : i1
    %43 = comb.and bin %42, %38 : i1
    %44 = comb.and bin %27, %43 : i1
    %45 = comb.xor bin %44, %true : i1
    %46 = comb.mux bin %44, %28, %41 : i2
    %47 = comb.xor bin %27, %true : i1
    %48 = comb.and bin %43, %47 : i1
    %49 = comb.or bin %48, %44, %39 : i1
    %50 = comb.mux bin %49, %22, %c0_i2 : i2
    %51 = comb.xor bin %48, %true : i1
    %52 = comb.and bin %51, %45, %40 : i1
    %53 = comb.or bin %52, %21, %18 : i1
    %54 = comb.mux bin %49, %15, %c0_i2 : i2
    %55 = comb.or bin %52, %14, %11 : i1
    %56 = comb.sub bin %c-1_i2, %write_pointer : i2
    %57 = hw.array_inject %memory[%56], %in_data : !hw.array<4xi4>, i2
    %58 = seq.to_clock %clk
    %59 = comb.mux bin %53, %50, %read_pointer : i2
    %read_pointer = seq.firreg %59 clock %58 : i2
    %60 = comb.mux bin %55, %54, %write_pointer : i2
    %write_pointer = seq.firreg %60 clock %58 : i2
    %61 = comb.mux bin %48, %level, %46 : i2
    %level = seq.firreg %61 clock %58 : i2
    %62 = comb.mux bin %7, %57, %memory : !hw.array<4xi4>
    %memory = seq.firreg %62 clock %58 : !hw.array<4xi4>
    %63 = seq.compreg %4, %58 : i1  
    %64 = seq.compreg %6, %58 : i4  
    %65 = comb.xor bin %out_ack, %true : i1
    %66 = comb.icmp eq %63, %4 : i1
    %67 = comb.icmp eq %64, %6 : i4
    %68 = comb.and bin %66, %67 : i1
    %69 = comb.and %reset_n, %4, %65 : i1
    %70 = comb.xor %reset_n, %true : i1
    %71 = seq.firreg %69 clock %58 reset async %70, %false preset 0 : i1
    verif.clocked_assert %68 if %71, posedge %clk : i1
    hw.output %3, %4, %6, %0, %1, %level : i1, i1, i4, i1, i1, i2
  }
}

