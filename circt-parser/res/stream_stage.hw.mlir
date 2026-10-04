module {
  hw.module private @std_register(in %clk : i1, in %rst : i1, in %enable : i1, in %next : i1, out value : i1) {
    %true = hw.constant true
    %0 = comb.xor bin %rst, %true : i1
    %1 = comb.and bin %0, %enable, %next : i1
    %2 = comb.or bin %rst, %enable : i1
    %3 = seq.to_clock %clk
    %4 = comb.mux bin %2, %1, %value : i1
    %value = seq.firreg %4 clock %3 : i1
    hw.output %value : i1
  }
  hw.module private @std_register_0(in %clk : i1, in %rst : i1, in %enable : i1, in %next : i4, out value : i4) {
    %true = hw.constant true
    %c0_i4 = hw.constant 0 : i4
    %0 = comb.xor bin %enable, %true : i1
    %1 = comb.or bin %rst, %0 : i1
    %2 = comb.mux bin %1, %c0_i4, %next : i4
    %3 = comb.or bin %rst, %enable : i1
    %4 = seq.to_clock %clk
    %5 = comb.mux bin %3, %2, %value : i4
    %value = seq.firreg %5 clock %4 : i4
    hw.output %value : i4
  }
  hw.module private @stream_stage(in %clk : i1, in %rst : i1, in %stream_in_valid : i1, out stream_in_ready : i1, in %stream_in_payload : i4, out stream_out_valid : i1, in %stream_out_ready : i1, out stream_out_payload : i4) {
    %true = hw.constant true
    %false = hw.constant false
    %genblk1.valid_register_inst.value = hw.instance "genblk1.valid_register_inst" @std_register(clk: %clk: i1, rst: %rst: i1, enable: %3: i1, next: %2: i1) -> (value: i1)
    %genblk1.payload_register_inst.value = hw.instance "genblk1.payload_register_inst" @std_register_0(clk: %clk: i1, rst: %false: i1, enable: %2: i1, next: %stream_in_payload: i4) -> (value: i4)
    %0 = comb.xor bin %genblk1.valid_register_inst.value, %true : i1
    %1 = comb.or bin %stream_out_ready, %0 : i1
    %2 = comb.and bin %stream_in_valid, %1 : i1
    %3 = comb.or bin %2, %stream_out_ready : i1
    hw.output %1, %genblk1.valid_register_inst.value, %genblk1.payload_register_inst.value : i1, i1, i4
  }
  hw.module @stream_stage_formal(in %clk : i1, in %rst : i1, in %in_valid : i1, in %in_payload : i4, in %out_ready : i1) {
    %true = hw.constant true
    %c-1_i2 = hw.constant -1 : i2
    %c1_i2 = hw.constant 1 : i2
    %c-2_i2 = hw.constant -2 : i2
    %c0_i2 = hw.constant 0 : i2
    %false = hw.constant false
    %dut.stream_in_ready, %dut.stream_out_valid, %dut.stream_out_payload = hw.instance "dut" @stream_stage(clk: %clk: i1, rst: %rst: i1, stream_in_valid: %in_valid: i1, stream_in_payload: %in_payload: i4, stream_out_ready: %out_ready: i1) -> (stream_in_ready: i1, stream_out_valid: i1, stream_out_payload: i4)
    %0 = comb.and bin %in_valid, %dut.stream_in_ready : i1
    %1 = comb.and bin %dut.stream_out_valid, %out_ready : i1
    %2 = comb.xor bin %out_ready, %true : i1
    %3 = comb.and bin %dut.stream_out_valid, %2 : i1
    %4 = seq.to_clock %clk
    %5 = seq.compreg %rst, %4 : i1  
    %6 = comb.xor bin %dut.stream_in_ready, %true : i1
    %7 = comb.and bin %in_valid, %6 : i1
    %8 = seq.compreg %7, %4 : i1  
    %9 = seq.compreg %in_payload, %4 : i4  
    %10 = seq.compreg %3, %4 : i1  
    %11 = seq.compreg %dut.stream_out_payload, %4 : i4  
    %12 = comb.xor bin %past_valid, %true : i1
    %13 = comb.icmp eq %rst, %12 : i1
    verif.clocked_assume %13, posedge %clk label "" : i1
    %14 = comb.xor bin %rst, %true : i1
    %15 = comb.xor bin %5, %true : i1
    %16 = comb.and bin %past_valid, %14, %15 : i1
    %17 = comb.and %8, %16 : i1
    verif.clocked_assume %in_valid if %17, posedge %clk label "" : i1
    %18 = comb.icmp eq %in_payload, %9 : i4
    verif.clocked_assume %18 if %17, posedge %clk label "" : i1
    %19 = comb.xor %8, %true : i1
    %20 = comb.and %19, %16 : i1
    %21 = comb.xor %16, %true : i1
    %22 = comb.or %17, %20, %21 : i1
    %23 = comb.xor %rst, %true : i1
    %24 = comb.and %23, %22 : i1
    %25 = comb.concat %0, %1 : i1, i1
    %26 = comb.icmp bin eq %25, %c-2_i2 : i2
    %27 = comb.xor %26, %true : i1
    %28 = comb.and %27, %24 : i1
    %29 = comb.icmp bin eq %25, %c1_i2 : i2
    %30 = comb.xor %29, %true : i1
    %31 = comb.and %30, %occupied : i1
    %32 = comb.or %26, %31 : i1
    %33 = comb.and %26, %24 : i1
    %34 = comb.extract %stall_history from 0 : (i2) -> i1
    %35 = comb.concat %34, %3 : i1, i1
    %36 = comb.icmp eq %stall_history, %c-1_i2 : i2
    %37 = comb.or %36, %saw_two_cycle_stall : i1
    %38 = comb.and %27, %23 : i1
    %39 = comb.and %26, %23 : i1
    %40 = comb.or %38, %39 : i1
    %41 = comb.and %40, %32 : i1
    %42 = comb.xor %40, %true : i1
    %43 = comb.or %42, %26, %29 : i1
    %44 = comb.mux %40, %35, %c0_i2 : i2
    %45 = comb.and %40, %37 : i1
    %46 = comb.or %42, %36 : i1
    %47 = comb.and %rst, %22 : i1
    %48 = comb.or %28, %33, %47 : i1
    %49 = comb.and %past_valid, %14, %48 : i1
    %50 = comb.icmp eq %dut.stream_out_valid, %occupied : i1
    verif.clocked_assert %50 if %49, posedge %clk label "" : i1
    %51 = comb.and %dut.stream_out_valid, %49 : i1
    %52 = comb.icmp eq %dut.stream_out_payload, %expected_payload : i4
    verif.clocked_assert %52 if %51, posedge %clk label "" : i1
    %53 = comb.xor %dut.stream_out_valid, %true : i1
    %54 = comb.and %53, %49 : i1
    %55 = comb.or %51, %54 : i1
    %56 = comb.xor bin %1, %true : i1
    %57 = comb.or bin %56, %occupied : i1
    verif.clocked_assert %57 if %55, posedge %clk label "" : i1
    %58 = comb.xor bin %0, %true : i1
    %59 = comb.xor bin %occupied, %true : i1
    %60 = comb.or bin %58, %59, %1 : i1
    verif.clocked_assert %60 if %55, posedge %clk label "" : i1
    %61 = comb.or bin %59, %out_ready : i1
    %62 = comb.icmp eq %dut.stream_in_ready, %61 : i1
    verif.clocked_assert %62 if %55, posedge %clk label "" : i1
    %63 = comb.and %5, %55 : i1
    %64 = comb.xor bin %dut.stream_out_valid, %true : i1
    verif.clocked_assert %64 if %63, posedge %clk label "" : i1
    %65 = comb.xor %5, %true : i1
    %66 = comb.and %65, %55 : i1
    %67 = comb.or %63, %66 : i1
    %68 = comb.and bin %15, %10 : i1
    %69 = comb.and %68, %67 : i1
    verif.clocked_assert %dut.stream_out_valid if %69, posedge %clk label "" : i1
    %70 = comb.icmp eq %dut.stream_out_payload, %11 : i4
    verif.clocked_assert %70 if %69, posedge %clk label "" : i1
    %71 = comb.xor %68, %true : i1
    %72 = comb.and %71, %67 : i1
    %73 = comb.or %69, %72 : i1
    %74 = comb.and bin %59, %0 : i1
    verif.clocked_cover %74 if %73, posedge %clk label "" : i1
    %75 = comb.icmp ne %in_payload, %dut.stream_out_payload : i4
    %76 = comb.and bin %0, %1, %75 : i1
    verif.clocked_cover %76 if %73, posedge %clk label "" : i1
    %77 = comb.and bin %36, %1 : i1
    verif.clocked_cover %77 if %73, posedge %clk label "" : i1
    %78 = comb.and bin %saw_two_cycle_stall, %59 : i1
    verif.clocked_cover %78 if %73, posedge %clk label "" : i1
    %past_valid = seq.firreg %true clock %4 preset 0 : i1
    %79 = comb.mux bin %43, %41, %occupied : i1
    %occupied = seq.firreg %79 clock %4 : i1
    %stall_history = seq.firreg %44 clock %4 : i2
    %80 = comb.mux bin %46, %45, %saw_two_cycle_stall : i1
    %saw_two_cycle_stall = seq.firreg %80 clock %4 : i1
    %81 = comb.and %40, %0 : i1
    %82 = comb.mux bin %81, %in_payload, %expected_payload : i4
    %expected_payload = seq.firreg %82 clock %4 : i4
    verif.clocked_assume %13, posedge %clk : i1
    %83 = comb.icmp eq %9, %in_payload : i4
    %84 = comb.and bin %in_valid, %83 : i1
    %85 = comb.and %14, %7 : i1
    %86 = seq.firreg %85 clock %4 reset async %rst, %false preset 0 : i1
    verif.clocked_assume %84 if %86, posedge %clk : i1
    %87 = seq.firreg %rst clock %4 preset 0 : i1
    verif.clocked_assert %64 if %87, posedge %clk : i1
    %88 = comb.and %14, %past_valid : i1
    verif.clocked_assert %50 if %88, posedge %clk : i1
    %89 = comb.and %14, %past_valid, %dut.stream_out_valid : i1
    verif.clocked_assert %52 if %89, posedge %clk : i1
    %90 = comb.and %14, %past_valid, %1 : i1
    verif.clocked_assert %occupied if %90, posedge %clk : i1
    %91 = comb.or bin %59, %1 : i1
    %92 = comb.and %14, %past_valid, %0 : i1
    verif.clocked_assert %91 if %92, posedge %clk : i1
    verif.clocked_assert %62 if %88, posedge %clk : i1
    %93 = comb.and bin %past_valid, %3 : i1
    %94 = comb.icmp eq %11, %dut.stream_out_payload : i4
    %95 = comb.and bin %dut.stream_out_valid, %94 : i1
    %96 = comb.and %14, %93 : i1
    %97 = seq.firreg %96 clock %4 reset async %rst, %false preset 0 : i1
    verif.clocked_assert %95 if %97, posedge %clk : i1
    %98 = comb.icmp eq %dut.stream_out_payload, %9 : i4
    %99 = comb.and bin %dut.stream_out_valid, %98 : i1
    %100 = seq.firreg %92 clock %4 reset async %rst, %false preset 0 : i1
    verif.clocked_assert %99 if %100, posedge %clk : i1
    %101 = comb.and bin %past_valid, %59, %0 : i1
    verif.clocked_cover %101 if %14, posedge %clk : i1
    %102 = comb.and bin %past_valid, %0, %1, %75 : i1
    verif.clocked_cover %102 if %14, posedge %clk : i1
    %103 = seq.firreg %96 clock %4 reset async %rst, %false preset 0 : i1
    %104 = comb.and %103, %93 : i1
    %105 = seq.firreg %104 clock %4 reset async %rst, %false preset 0 : i1
    %106 = comb.and %105, %1 : i1
    verif.clocked_cover %106 if %14, posedge %clk : i1
    %107 = comb.and %14, %101 : i1
    %108 = seq.firreg %107 clock %4 reset async %rst, %false preset 0 : i1
    %109 = comb.and %108, %3 : i1
    %110 = seq.firreg %109 clock %4 reset async %rst, %false preset 0 : i1
    %111 = comb.and %110, %3 : i1
    %112 = seq.firreg %111 clock %4 reset async %rst, %false preset 0 : i1
    %113 = comb.and %112, %1, %58 : i1
    %114 = seq.firreg %113 clock %4 reset async %rst, %false preset 0 : i1
    %115 = comb.and %114, %59 : i1
    verif.clocked_cover %115 if %14, posedge %clk : i1
    hw.output
  }
}

