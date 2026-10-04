module {
  hw.module private @fifo_stage_bypass(in %clk : i1, in %reset_n : i1, in %in_valid : i1, in %in_data : i1, out in_ack : i1, out out_valid : i1, out out_data : i1, in %out_ack : i1, out fifo_count : i3, out empty : i1) {
    %true = hw.constant true
    %c1_i3 = hw.constant 1 : i3
    %c0_i3 = hw.constant 0 : i3
    %fifo_i.in_ack, %fifo_i.out_valid, %fifo_i.out_data, %fifo_i.full, %fifo_i.empty, %fifo_i.level = hw.instance "fifo_i" @fifo(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %1: i1, in_data: %2: i4, out_ack: %16: i1) -> (in_ack: i1, out_valid: i1, out_data: i4, full: i1, empty: i1, level: i3) {sv.namehint = "fifo_full"}
    %stage_i.in_ack, %stage_i.out_valid, %stage_i.out_data = hw.instance "stage_i" @stage(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %3: i1, in_data: %2: i4, out_ack: %17: i1) -> (in_ack: i1, out_valid: i1, out_data: i4) {sv.namehint = "stage_in_ack"}
    %0 = comb.xor bin %stage_i.in_ack, %true : i1
    %1 = comb.and bin %in_valid, %0 {sv.namehint = "fifo_in_valid"} : i1
    %2 = comb.concat %write_index, %in_data {sv.namehint = "fifo_in_data"} : i3, i1
    %3 = comb.and bin %in_valid, %stage_i.in_ack {sv.namehint = "stage_in_valid"} : i1
    %4 = comb.or bin %stage_i.in_ack, %fifo_i.in_ack : i1
    %5 = comb.and bin %in_valid, %4 : i1
    %6 = comb.extract %fifo_i.out_data from 1 : (i4) -> i3
    %7 = comb.icmp eq %6, %read_index : i3
    %8 = comb.and bin %fifo_i.out_valid, %7 {sv.namehint = "fifo_selected"} : i1
    %9 = comb.extract %stage_i.out_data from 1 : (i4) -> i3
    %10 = comb.icmp eq %9, %read_index : i3
    %11 = comb.and bin %stage_i.out_valid, %10 {sv.namehint = "stage_selected"} : i1
    %12 = comb.or bin %8, %11 : i1
    %13 = comb.extract %stage_i.out_data from 0 : (i4) -> i1
    %14 = comb.extract %fifo_i.out_data from 0 : (i4) -> i1
    %15 = comb.mux bin %11, %13, %14 : i1
    %16 = comb.and bin %out_ack, %8 {sv.namehint = "fifo_out_ack"} : i1
    %17 = comb.and bin %out_ack, %11 {sv.namehint = "stage_out_ack"} : i1
    %18 = comb.and bin %12, %out_ack : i1
    %19 = comb.xor bin %stage_i.out_valid, %true : i1
    %20 = comb.and bin %fifo_i.empty, %stage_i.in_ack, %19 : i1
    %21 = comb.add bin %write_index, %c1_i3 : i3
    %22 = comb.mux bin %5, %21, %write_index : i3
    %23 = comb.add bin %read_index, %c1_i3 : i3
    %24 = comb.and bin %5, %reset_n : i1
    %25 = comb.xor bin %5, %true : i1
    %26 = comb.and bin %25, %reset_n : i1
    %27 = comb.or bin %24, %26 : i1
    %28 = comb.and bin %18, %27 : i1
    %29 = comb.xor bin %28, %true : i1
    %30 = comb.mux bin %28, %23, %c0_i3 : i3
    %31 = comb.xor bin %18, %true : i1
    %32 = comb.and bin %27, %31 : i1
    %33 = comb.or bin %32, %28 : i1
    %34 = comb.mux bin %33, %22, %c0_i3 : i3
    %35 = comb.xor bin %32, %true : i1
    %36 = comb.and bin %35, %29 : i1
    %37 = comb.or bin %36, %5 : i1
    %38 = seq.to_clock %clk
    %39 = comb.mux bin %37, %34, %write_index : i3
    %write_index = seq.firreg %39 clock %38 : i3
    %40 = comb.mux bin %32, %read_index, %30 : i3
    %read_index = seq.firreg %40 clock %38 : i3
    hw.output %4, %12, %15, %fifo_i.level, %20 : i1, i1, i1, i3, i1
  }
  hw.module @fifo_stage_bypass_tb(in %clk : i1, in %reset_n : i1, in %in_valid : i1, in %in_data : i1, out in_ack : i1, out out_valid : i1, out out_data : i1, in %out_ack : i1) {
    %false = hw.constant false
    %true = hw.constant true
    %c-4_i3 = hw.constant -4 : i3
    %c1_i3 = hw.constant 1 : i3
    %c0_i3 = hw.constant 0 : i3
    %c-2_i3 = hw.constant -2 : i3
    %c-1_i3 = hw.constant -1 : i3
    %wrapper_i.in_ack, %wrapper_i.out_valid, %wrapper_i.out_data, %wrapper_i.fifo_count, %wrapper_i.empty = hw.instance "wrapper_i" @fifo_stage_bypass(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %in_valid: i1, in_data: %in_data: i1, out_ack: %out_ack: i1) -> (in_ack: i1, out_valid: i1, out_data: i1, fifo_count: i3, empty: i1)
    %0 = comb.and bin %in_valid, %wrapper_i.in_ack : i1
    %1 = comb.and bin %wrapper_i.out_valid, %out_ack : i1
    %2 = comb.sub bin %c-1_i3, %reference_read_index : i3
    %3 = hw.array_get %reference_memory[%2] : !hw.array<8xi1>, i3
    %4 = comb.add bin %reference_write_index, %c1_i3 : i3
    %5 = comb.mux bin %0, %4, %reference_write_index : i3
    %6 = comb.add bin %reference_read_index, %c1_i3 : i3
    %7 = comb.mux bin %1, %6, %reference_read_index : i3
    %8 = comb.xor bin %1, %true : i1
    %9 = comb.and bin %0, %8 : i1
    %10 = comb.add bin %reference_count, %c1_i3 : i3
    %11 = comb.xor bin %0, %true : i1
    %12 = comb.and bin %11, %1 : i1
    %13 = comb.add bin %reference_count, %c-1_i3 : i3
    %14 = comb.and bin %0, %reset_n : i1
    %15 = comb.and bin %11, %reset_n : i1
    %16 = comb.or bin %14, %15 : i1
    %17 = comb.and bin %1, %16 : i1
    %18 = comb.and bin %8, %16 : i1
    %19 = comb.or bin %17, %18 : i1
    %20 = comb.and bin %9, %19 : i1
    %21 = comb.mux bin %20, %10, %c0_i3 : i3
    %22 = comb.xor bin %20, %true : i1
    %23 = comb.xor bin %9, %true : i1
    %24 = comb.and bin %23, %19 : i1
    %25 = comb.and bin %12, %24 : i1
    %26 = comb.mux bin %25, %13, %21 : i3
    %27 = comb.xor bin %25, %true : i1
    %28 = comb.xor bin %12, %true : i1
    %29 = comb.and bin %24, %28 : i1
    %30 = comb.or bin %29, %25, %20 : i1
    %31 = comb.mux bin %30, %5, %c0_i3 : i3
    %32 = comb.xor bin %29, %true : i1
    %33 = comb.and bin %32, %27, %22 : i1
    %34 = comb.or bin %33, %0 : i1
    %35 = comb.mux bin %30, %7, %c0_i3 : i3
    %36 = comb.or bin %33, %1 : i1
    %37 = comb.sub bin %c-1_i3, %reference_write_index : i3
    %38 = hw.array_inject %reference_memory[%37], %in_data : !hw.array<8xi1>, i3
    %39 = seq.to_clock %clk
    %40 = comb.mux bin %29, %reference_count, %26 : i3
    %reference_count = seq.firreg %40 clock %39 : i3
    %41 = comb.mux bin %34, %31, %reference_write_index : i3
    %reference_write_index = seq.firreg %41 clock %39 : i3
    %42 = comb.mux bin %36, %35, %reference_read_index : i3
    %reference_read_index = seq.firreg %42 clock %39 : i3
    %43 = comb.mux bin %0, %38, %reference_memory : !hw.array<8xi1>
    %reference_memory = seq.firreg %43 clock %39 : !hw.array<8xi1>
    %44 = seq.compreg %in_valid, %39 : i1  
    %45 = seq.compreg %in_data, %39 : i1  
    %46 = comb.xor bin %wrapper_i.in_ack, %true : i1
    %47 = comb.icmp eq %44, %in_valid : i1
    %48 = comb.icmp eq %45, %in_data : i1
    %49 = comb.and bin %47, %48 : i1
    %50 = comb.and %reset_n, %in_valid, %46 : i1
    %51 = comb.xor %reset_n, %true : i1
    %52 = seq.firreg %50 clock %39 reset async %51, %false preset 0 : i1
    verif.clocked_assume %49 if %52, posedge %clk : i1
    %53 = comb.xor bin %wrapper_i.out_valid, %true : i1
    %54 = comb.and %reset_n, %wrapper_i.empty : i1
    verif.clocked_assert %53 if %54, posedge %clk : i1
    %55 = comb.icmp ne %reference_count, %c-2_i3 : i3
    %56 = comb.and %reset_n, %9 : i1
    verif.clocked_assert %55 if %56, posedge %clk : i1
    %57 = comb.icmp ne %reference_count, %c0_i3 : i3
    %58 = comb.and %reset_n, %1 : i1
    verif.clocked_assert %57 if %58, posedge %clk : i1
    %59 = comb.icmp eq %wrapper_i.out_data, %3 : i1
    %60 = comb.and bin %57, %59 : i1
    %61 = comb.and %reset_n, %wrapper_i.out_valid : i1
    verif.clocked_assert %60 if %61, posedge %clk : i1
    %62 = comb.and bin %wrapper_i.empty, %in_valid, %wrapper_i.in_ack : i1
    verif.clocked_cover %62 if %reset_n, posedge %clk : i1
    %63 = comb.icmp eq %wrapper_i.fifo_count, %c-4_i3 : i3
    %64 = comb.and bin %63, %wrapper_i.out_valid, %out_ack : i1
    verif.clocked_cover %64 if %reset_n, posedge %clk : i1
    %circt_past_valid = seq.firreg %true clock %39 preset 0 : i1
    %65 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %65, posedge %clk : i1
    hw.output %wrapper_i.in_ack, %wrapper_i.out_valid, %wrapper_i.out_data : i1, i1, i1
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
  hw.module private @stage_ack_registered(in %clk : i1, in %reset_n : i1, in %in_valid : i1, in %in_data : i4, out in_ack : i1, out out_valid : i1, out out_data : i4, in %out_ack : i1) {
    %true = hw.constant true
    %0 = comb.xor bin %stored_valid, %true : i1
    %1 = comb.or bin %stored_valid, %in_valid : i1
    %2 = comb.mux bin %stored_valid, %stored_data, %in_data : i4
    %3 = comb.xor bin %out_ack, %true : i1
    %4 = comb.and bin %in_valid, %3 : i1
    %5 = comb.and bin %0, %reset_n : i1
    %6 = comb.and bin %5, %4 : i1
    %7 = comb.xor bin %5, %true : i1
    %8 = comb.or bin %7, %4 : i1
    %9 = comb.and bin %stored_valid, %reset_n : i1
    %10 = comb.mux bin %9, %3, %6 : i1
    %11 = comb.mux bin %9, %out_ack, %8 : i1
    %12 = seq.to_clock %clk
    %13 = comb.mux bin %11, %10, %stored_valid : i1
    %stored_valid = seq.firreg %13 clock %12 : i1
    %14 = comb.mux bin %4, %in_data, %stored_data : i4
    %stored_data = seq.firreg %14 clock %12 : i4
    hw.output %0, %1, %2 : i1, i1, i4
  }
  hw.module private @stage(in %clk : i1, in %reset_n : i1, in %in_valid : i1, in %in_data : i4, out in_ack : i1, out out_valid : i1, out out_data : i4, in %out_ack : i1) {
    %false = hw.constant false
    %true = hw.constant true
    %valid_ack_registered.ack_registered_i.in_ack, %valid_ack_registered.ack_registered_i.out_valid, %valid_ack_registered.ack_registered_i.out_data = hw.instance "valid_ack_registered.ack_registered_i" @stage_ack_registered(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %in_valid: i1, in_data: %in_data: i4, out_ack: %valid_ack_registered.valid_registered_i.in_ack: i1) -> (in_ack: i1, out_valid: i1, out_data: i4) {sv.namehint = "valid_ack_registered.intermediate_data"}
    %valid_ack_registered.valid_registered_i.in_ack, %valid_ack_registered.valid_registered_i.out_valid, %valid_ack_registered.valid_registered_i.out_data = hw.instance "valid_ack_registered.valid_registered_i" @stage_valid_registered(clk: %clk: i1, reset_n: %reset_n: i1, in_valid: %valid_ack_registered.ack_registered_i.out_valid: i1, in_data: %valid_ack_registered.ack_registered_i.out_data: i4, out_ack: %out_ack: i1) -> (in_ack: i1, out_valid: i1, out_data: i4) {sv.namehint = "valid_ack_registered.intermediate_ack"}
    %0 = seq.to_clock %clk
    %1 = seq.compreg %valid_ack_registered.valid_registered_i.out_valid, %0 : i1  
    %2 = seq.compreg %valid_ack_registered.valid_registered_i.out_data, %0 : i4  
    %3 = comb.xor bin %out_ack, %true : i1
    %4 = comb.icmp eq %1, %valid_ack_registered.valid_registered_i.out_valid : i1
    %5 = comb.icmp eq %2, %valid_ack_registered.valid_registered_i.out_data : i4
    %6 = comb.and bin %4, %5 : i1
    %7 = comb.and %reset_n, %valid_ack_registered.valid_registered_i.out_valid, %3 : i1
    %8 = comb.xor %reset_n, %true : i1
    %9 = seq.firreg %7 clock %0 reset async %8, %false preset 0 : i1
    verif.clocked_assert %6 if %9, posedge %clk : i1
    hw.output %valid_ack_registered.ack_registered_i.in_ack, %valid_ack_registered.valid_registered_i.out_valid, %valid_ack_registered.valid_registered_i.out_data : i1, i1, i4
  }
  hw.module private @fifo(in %clk : i1, in %reset_n : i1, in %in_valid : i1, in %in_data : i4, out in_ack : i1, out out_valid : i1, out out_data : i4, in %out_ack : i1, out full : i1, out empty : i1, out level : i3) {
    %false = hw.constant false
    %true = hw.constant true
    %c-1_i3 = hw.constant -1 : i3
    %c1_i3 = hw.constant 1 : i3
    %c1_i2 = hw.constant 1 : i2
    %c0_i2 = hw.constant 0 : i2
    %c-1_i2 = hw.constant -1 : i2
    %c0_i3 = hw.constant 0 : i3
    %c-4_i3 = hw.constant -4 : i3
    %0 = comb.icmp eq %level, %c-4_i3 : i3
    %1 = comb.icmp eq %level, %c0_i3 : i3
    %2 = comb.xor bin %0, %true : i1
    %3 = comb.or bin %2, %out_ack : i1
    %4 = comb.xor bin %1, %true : i1
    %5 = comb.sub bin %c-1_i2, %read_pointer : i2
    %6 = hw.array_get %memory[%5] : !hw.array<4xi4>, i2
    %7 = comb.and bin %in_valid, %3 : i1
    %8 = comb.and bin %4, %out_ack : i1
    %9 = comb.icmp eq %write_pointer, %c-1_i2 : i2
    %10 = comb.add bin %write_pointer, %c1_i2 : i2
    %11 = comb.and bin %7, %9 : i1
    %12 = comb.mux bin %11, %c0_i2, %write_pointer : i2
    %13 = comb.xor bin %9, %true : i1
    %14 = comb.and bin %13, %7 : i1
    %15 = comb.mux bin %14, %10, %12 : i2
    %16 = comb.icmp eq %read_pointer, %c-1_i2 : i2
    %17 = comb.add bin %read_pointer, %c1_i2 : i2
    %18 = comb.and bin %8, %16 : i1
    %19 = comb.mux bin %18, %c0_i2, %read_pointer : i2
    %20 = comb.xor bin %16, %true : i1
    %21 = comb.and bin %20, %8 : i1
    %22 = comb.mux bin %21, %17, %19 : i2
    %23 = comb.xor bin %8, %true : i1
    %24 = comb.and bin %7, %23 : i1
    %25 = comb.add bin %level, %c1_i3 : i3
    %26 = comb.xor bin %7, %true : i1
    %27 = comb.and bin %26, %8 : i1
    %28 = comb.add bin %level, %c-1_i3 : i3
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
    %41 = comb.mux bin %39, %25, %c0_i3 : i3
    %42 = comb.xor bin %24, %true : i1
    %43 = comb.and bin %42, %38 : i1
    %44 = comb.and bin %27, %43 : i1
    %45 = comb.xor bin %44, %true : i1
    %46 = comb.mux bin %44, %28, %41 : i3
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
    %61 = comb.mux bin %48, %level, %46 : i3
    %level = seq.firreg %61 clock %58 : i3
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
    hw.output %3, %4, %6, %0, %1, %level : i1, i1, i4, i1, i1, i3
  }
}

