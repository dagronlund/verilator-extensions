module {
  hw.module private @packet_switch(in %clk : i1, in %reset_n : i1, in %in0_valid : i1, in %in0_dest : i1, in %in0_data : i4, out in0_ack : i1, in %in1_valid : i1, in %in1_dest : i1, in %in1_data : i4, out in1_ack : i1, out out0_valid : i1, out out0_data : i4, in %out0_ack : i1, out out1_valid : i1, out out1_data : i4, in %out1_ack : i1, out count0 : i2, out count1 : i2, out grant00 : i1, out grant10 : i1, out grant01 : i1, out grant11 : i1, out pop0 : i1, out pop1 : i1, out push0 : i1, out push1 : i1, out rr0 : i1, out rr1 : i1) {
    %true = hw.constant true
    %c0_i5 = hw.constant 0 : i5
    %c1_i2 = hw.constant 1 : i2
    %c0_i2 = hw.constant 0 : i2
    %c0_i15 = hw.constant 0 : i15
    %c-1_i2 = hw.constant -1 : i2
    %0 = comb.extract %entries0 from 0 : (i15) -> i4
    %1 = comb.extract %entries0 from 4 {sv.namehint = "head0_dest"} : (i15) -> i1
    %2 = comb.extract %entries1 from 0 : (i15) -> i4
    %3 = comb.extract %entries1 from 4 {sv.namehint = "head1_dest"} : (i15) -> i1
    %4 = comb.icmp ne %count0, %c0_i2 : i2
    %5 = comb.xor bin %1, %true : i1
    %6 = comb.and bin %4, %5 : i1
    %7 = comb.icmp ne %count1, %c0_i2 : i2
    %8 = comb.xor bin %3, %true : i1
    %9 = comb.and bin %7, %8 : i1
    %10 = comb.and bin %4, %1 : i1
    %11 = comb.and bin %7, %3 : i1
    %12 = comb.xor bin %9, %true : i1
    %13 = comb.xor bin %rr0, %true : i1
    %14 = comb.or bin %12, %13 : i1
    %15 = comb.and bin %6, %14 : i1
    %16 = comb.xor bin %6, %true : i1
    %17 = comb.or bin %16, %rr0 : i1
    %18 = comb.and bin %9, %17 : i1
    %19 = comb.xor bin %11, %true : i1
    %20 = comb.xor bin %rr1, %true : i1
    %21 = comb.or bin %19, %20 : i1
    %22 = comb.and bin %10, %21 : i1
    %23 = comb.xor bin %10, %true : i1
    %24 = comb.or bin %23, %rr1 : i1
    %25 = comb.and bin %11, %24 : i1
    %26 = comb.xor bin %hold0_source, %true : i1
    %27 = comb.mux bin %hold0, %26, %15 : i1
    %28 = comb.mux bin %hold0, %hold0_source, %18 : i1
    %29 = comb.xor bin %hold1_source, %true : i1
    %30 = comb.mux bin %hold1, %29, %22 : i1
    %31 = comb.mux bin %hold1, %hold1_source, %25 : i1
    %32 = comb.or bin %27, %28 : i1
    %33 = comb.mux bin %27, %0, %2 : i4
    %34 = comb.or bin %30, %31 : i1
    %35 = comb.mux bin %30, %0, %2 : i4
    %36 = comb.and bin %27, %out0_ack : i1
    %37 = comb.and bin %30, %out1_ack : i1
    %38 = comb.or bin %36, %37 : i1
    %39 = comb.and bin %28, %out0_ack : i1
    %40 = comb.and bin %31, %out1_ack : i1
    %41 = comb.or bin %39, %40 : i1
    %42 = comb.icmp ne %count0, %c-1_i2 : i2
    %43 = comb.or bin %42, %38 : i1
    %44 = comb.icmp ne %count1, %c-1_i2 : i2
    %45 = comb.or bin %44, %41 : i1
    %46 = comb.and bin %in0_valid, %43 : i1
    %47 = comb.and bin %in1_valid, %45 : i1
    %48 = comb.and bin %38, %46 : i1
    %49 = comb.extract %entries0 from 5 : (i15) -> i10
    %50 = comb.concat %in0_dest, %in0_data, %49 : i1, i4, i10
    %51 = comb.concat %c0_i5, %49 : i5, i10
    %52 = comb.add bin %count0, %c-1_i2 : i2
    %53 = comb.icmp eq %count0, %c0_i2 : i2
    %54 = comb.concat %49, %in0_dest, %in0_data : i10, i1, i4
    %55 = comb.icmp eq %count0, %c1_i2 : i2
    %56 = comb.extract %entries0 from 10 : (i15) -> i5
    %57 = comb.extract %entries0 from 0 : (i15) -> i5
    %58 = comb.concat %56, %in0_dest, %in0_data, %57 : i5, i1, i4, i5
    %59 = comb.extract %entries0 from 0 : (i15) -> i10
    %60 = comb.concat %in0_dest, %in0_data, %59 : i1, i4, i10
    %61 = comb.xor bin %53, %true : i1
    %62 = comb.and bin %55, %61 : i1
    %63 = comb.mux bin %62, %58, %54 : i15
    %64 = comb.xor bin %55, %true : i1
    %65 = comb.or bin %55, %53 : i1
    %66 = comb.mux bin %65, %63, %60 : i15
    %67 = comb.add bin %count0, %c1_i2 : i2
    %68 = comb.and bin %48, %reset_n : i1
    %69 = comb.mux bin %68, %50, %c0_i15 : i15
    %70 = comb.mux bin %68, %count0, %c0_i2 : i2
    %71 = comb.xor bin %68, %true : i1
    %72 = comb.xor bin %48, %true : i1
    %73 = comb.and bin %72, %reset_n : i1
    %74 = comb.and bin %38, %73 : i1
    %75 = comb.mux bin %74, %51, %69 : i15
    %76 = comb.mux bin %74, %52, %70 : i2
    %77 = comb.xor bin %38, %true : i1
    %78 = comb.and bin %77, %73 : i1
    %79 = comb.and bin %46, %78 : i1
    %80 = comb.and bin %53, %79 : i1
    %81 = comb.and bin %61, %79 : i1
    %82 = comb.and bin %55, %81 : i1
    %83 = comb.and bin %64, %81 : i1
    %84 = comb.or bin %80, %82, %83 : i1
    %85 = comb.mux bin %84, %66, %75 : i15
    %86 = comb.mux bin %84, %67, %76 : i2
    %87 = comb.or bin %84, %74, %71 : i1
    %88 = comb.xor bin %46, %true : i1
    %89 = comb.and bin %78, %88 : i1
    %90 = comb.xor bin %89, %true : i1
    %91 = comb.and bin %90, %87 : i1
    %92 = seq.to_clock %clk
    %93 = comb.mux bin %89, %entries0, %85 : i15
    %entries0 = seq.firreg %93 clock %92 : i15
    %94 = comb.xor bin %91, %true : i1
    %95 = comb.or bin %94, %89 : i1
    %96 = comb.mux bin %95, %count0, %86 : i2
    %count0 = seq.firreg %96 clock %92 : i2
    %97 = comb.and bin %41, %47 : i1
    %98 = comb.extract %entries1 from 5 : (i15) -> i10
    %99 = comb.concat %in1_dest, %in1_data, %98 : i1, i4, i10
    %100 = comb.concat %c0_i5, %98 : i5, i10
    %101 = comb.add bin %count1, %c-1_i2 : i2
    %102 = comb.icmp eq %count1, %c0_i2 : i2
    %103 = comb.concat %98, %in1_dest, %in1_data : i10, i1, i4
    %104 = comb.icmp eq %count1, %c1_i2 : i2
    %105 = comb.extract %entries1 from 10 : (i15) -> i5
    %106 = comb.extract %entries1 from 0 : (i15) -> i5
    %107 = comb.concat %105, %in1_dest, %in1_data, %106 : i5, i1, i4, i5
    %108 = comb.extract %entries1 from 0 : (i15) -> i10
    %109 = comb.concat %in1_dest, %in1_data, %108 : i1, i4, i10
    %110 = comb.xor bin %102, %true : i1
    %111 = comb.and bin %104, %110 : i1
    %112 = comb.mux bin %111, %107, %103 : i15
    %113 = comb.xor bin %104, %true : i1
    %114 = comb.or bin %104, %102 : i1
    %115 = comb.mux bin %114, %112, %109 : i15
    %116 = comb.add bin %count1, %c1_i2 : i2
    %117 = comb.and bin %97, %reset_n : i1
    %118 = comb.mux bin %117, %99, %c0_i15 : i15
    %119 = comb.mux bin %117, %count1, %c0_i2 : i2
    %120 = comb.xor bin %117, %true : i1
    %121 = comb.xor bin %97, %true : i1
    %122 = comb.and bin %121, %reset_n : i1
    %123 = comb.and bin %41, %122 : i1
    %124 = comb.mux bin %123, %100, %118 : i15
    %125 = comb.mux bin %123, %101, %119 : i2
    %126 = comb.xor bin %41, %true : i1
    %127 = comb.and bin %126, %122 : i1
    %128 = comb.and bin %47, %127 : i1
    %129 = comb.and bin %102, %128 : i1
    %130 = comb.and bin %110, %128 : i1
    %131 = comb.and bin %104, %130 : i1
    %132 = comb.and bin %113, %130 : i1
    %133 = comb.or bin %129, %131, %132 : i1
    %134 = comb.mux bin %133, %115, %124 : i15
    %135 = comb.mux bin %133, %116, %125 : i2
    %136 = comb.or bin %133, %123, %120 : i1
    %137 = comb.xor bin %47, %true : i1
    %138 = comb.and bin %127, %137 : i1
    %139 = comb.xor bin %138, %true : i1
    %140 = comb.and bin %139, %136 : i1
    %141 = comb.mux bin %138, %entries1, %134 : i15
    %entries1 = seq.firreg %141 clock %92 : i15
    %142 = comb.xor bin %140, %true : i1
    %143 = comb.or bin %142, %138 : i1
    %144 = comb.mux bin %143, %count1, %135 : i2
    %count1 = seq.firreg %144 clock %92 : i2
    %145 = comb.and bin %6, %9, %out0_ack : i1
    %146 = comb.and bin %10, %11, %out1_ack : i1
    %147 = comb.and bin %reset_n, %145, %27 : i1
    %148 = comb.xor bin %reset_n, %true : i1
    %149 = comb.or bin %148, %145 : i1
    %150 = comb.and bin %reset_n, %146, %30 : i1
    %151 = comb.or bin %148, %146 : i1
    %152 = comb.mux bin %149, %147, %rr0 : i1
    %rr0 = seq.firreg %152 clock %92 : i1
    %153 = comb.mux bin %151, %150, %rr1 : i1
    %rr1 = seq.firreg %153 clock %92 : i1
    %154 = comb.xor bin %out0_ack, %true : i1
    %155 = comb.and bin %32, %154 : i1
    %156 = comb.or bin %hold0, %32 : i1
    %157 = comb.mux bin %hold0, %out0_ack, %155 : i1
    %158 = comb.xor bin %hold0, %true : i1
    %159 = comb.and bin %158, %155 : i1
    %160 = comb.xor bin %out1_ack, %true : i1
    %161 = comb.and bin %34, %160 : i1
    %162 = comb.and bin %158, %reset_n : i1
    %163 = comb.and bin %hold0, %reset_n : i1
    %164 = comb.or bin %162, %163 : i1
    %165 = comb.xor bin %hold1, %true : i1
    %166 = comb.and bin %165, %164 : i1
    %167 = comb.xor bin %166, %true : i1
    %168 = comb.and bin %166, %161 : i1
    %169 = comb.or bin %167, %161 : i1
    %170 = comb.and bin %hold1, %164 : i1
    %171 = comb.or bin %170, %166 : i1
    %172 = comb.and bin %171, %156, %154 : i1
    %173 = comb.xor bin %170, %true : i1
    %174 = comb.and bin %173, %167 : i1
    %175 = comb.or bin %174, %157 : i1
    %176 = comb.and bin %171, %158, %155, %28 : i1
    %177 = comb.or bin %174, %159 : i1
    %178 = comb.mux bin %170, %160, %168 : i1
    %179 = comb.mux bin %170, %out1_ack, %169 : i1
    %180 = comb.and bin %173, %166, %161, %31 : i1
    %181 = comb.and bin %173, %169 : i1
    %182 = comb.mux bin %175, %172, %hold0 : i1
    %hold0 = seq.firreg %182 clock %92 : i1
    %183 = comb.mux bin %177, %176, %hold0_source : i1
    %hold0_source = seq.firreg %183 clock %92 : i1
    %184 = comb.mux bin %179, %178, %hold1 : i1
    %hold1 = seq.firreg %184 clock %92 : i1
    %185 = comb.mux bin %181, %180, %hold1_source : i1
    %hold1_source = seq.firreg %185 clock %92 : i1
    hw.output %43, %45, %32, %33, %34, %35, %count0, %count1, %27, %28, %30, %31, %38, %41, %46, %47, %rr0, %rr1 : i1, i1, i1, i4, i1, i4, i2, i2, i1, i1, i1, i1, i1, i1, i1, i1, i1, i1
  }
  hw.module private @ingress_reference(in %clk : i1, in %reset_n : i1, in %push : i1, in %push_dest : i1, in %push_data : i4, in %pop : i1, out head_dest : i1, out head_data : i4, out count : i2) {
    %true = hw.constant true
    %c0_i5 = hw.constant 0 : i5
    %c-1_i2 = hw.constant -1 : i2
    %c1_i2 = hw.constant 1 : i2
    %c0_i2 = hw.constant 0 : i2
    %c0_i15 = hw.constant 0 : i15
    %0 = comb.extract %entries from 0 : (i15) -> i4
    %1 = comb.extract %entries from 4 : (i15) -> i1
    %2 = comb.and bin %pop, %push : i1
    %3 = comb.extract %entries from 5 : (i15) -> i10
    %4 = comb.concat %push_dest, %push_data, %3 : i1, i4, i10
    %5 = comb.concat %c0_i5, %3 : i5, i10
    %6 = comb.add bin %count, %c-1_i2 : i2
    %7 = comb.icmp eq %count, %c0_i2 : i2
    %8 = comb.concat %3, %push_dest, %push_data : i10, i1, i4
    %9 = comb.icmp eq %count, %c1_i2 : i2
    %10 = comb.extract %entries from 10 : (i15) -> i5
    %11 = comb.extract %entries from 0 : (i15) -> i5
    %12 = comb.concat %10, %push_dest, %push_data, %11 : i5, i1, i4, i5
    %13 = comb.extract %entries from 0 : (i15) -> i10
    %14 = comb.concat %push_dest, %push_data, %13 : i1, i4, i10
    %15 = comb.xor bin %7, %true : i1
    %16 = comb.and bin %9, %15 : i1
    %17 = comb.mux bin %16, %12, %8 : i15
    %18 = comb.xor bin %9, %true : i1
    %19 = comb.or bin %9, %7 : i1
    %20 = comb.mux bin %19, %17, %14 : i15
    %21 = comb.add bin %count, %c1_i2 : i2
    %22 = comb.and bin %2, %reset_n : i1
    %23 = comb.mux bin %22, %4, %c0_i15 : i15
    %24 = comb.mux bin %22, %count, %c0_i2 : i2
    %25 = comb.xor bin %22, %true : i1
    %26 = comb.xor bin %2, %true : i1
    %27 = comb.and bin %26, %reset_n : i1
    %28 = comb.and bin %pop, %27 : i1
    %29 = comb.mux bin %28, %5, %23 : i15
    %30 = comb.mux bin %28, %6, %24 : i2
    %31 = comb.xor bin %pop, %true : i1
    %32 = comb.and bin %31, %27 : i1
    %33 = comb.and bin %push, %32 : i1
    %34 = comb.and bin %7, %33 : i1
    %35 = comb.and bin %15, %33 : i1
    %36 = comb.and bin %9, %35 : i1
    %37 = comb.and bin %18, %35 : i1
    %38 = comb.or bin %34, %36, %37 : i1
    %39 = comb.mux bin %38, %20, %29 : i15
    %40 = comb.mux bin %38, %21, %30 : i2
    %41 = comb.or bin %38, %28, %25 : i1
    %42 = comb.xor bin %push, %true : i1
    %43 = comb.and bin %32, %42 : i1
    %44 = comb.xor bin %43, %true : i1
    %45 = comb.and bin %44, %41 : i1
    %46 = seq.to_clock %clk
    %47 = comb.mux bin %43, %entries, %39 : i15
    %entries = seq.firreg %47 clock %46 : i15
    %48 = comb.xor bin %45, %true : i1
    %49 = comb.or bin %48, %43 : i1
    %50 = comb.mux bin %49, %count, %40 : i2
    %count = seq.firreg %50 clock %46 : i2
    %51 = comb.icmp ne %count, %c0_i2 : i2
    %52 = comb.and %reset_n, %pop : i1
    verif.clocked_assert %51 if %52, posedge %clk : i1
    %53 = comb.icmp ne %count, %c-1_i2 : i2
    %54 = comb.and %reset_n, %push, %31 : i1
    verif.clocked_assert %53 if %54, posedge %clk : i1
    hw.output %1, %0, %count : i1, i4, i2
  }
  hw.module @packet_switch_tb(in %clk : i1, in %reset_n : i1, in %in0_valid : i1, in %in0_dest : i1, in %in0_data : i4, out in0_ack : i1, in %in1_valid : i1, in %in1_dest : i1, in %in1_data : i4, out in1_ack : i1, out out0_valid : i1, out out0_data : i4, in %out0_ack : i1, out out1_valid : i1, out out1_data : i4, in %out1_ack : i1) {
    %false = hw.constant false
    %true = hw.constant true
    %c-1_i2 = hw.constant -1 : i2
    %c0_i2 = hw.constant 0 : i2
    %dut.in0_ack, %dut.in1_ack, %dut.out0_valid, %dut.out0_data, %dut.out1_valid, %dut.out1_data, %dut.count0, %dut.count1, %dut.grant00, %dut.grant10, %dut.grant01, %dut.grant11, %dut.pop0, %dut.pop1, %dut.push0, %dut.push1, %dut.rr0, %dut.rr1 = hw.instance "dut" @packet_switch(clk: %clk: i1, reset_n: %reset_n: i1, in0_valid: %in0_valid: i1, in0_dest: %in0_dest: i1, in0_data: %in0_data: i4, in1_valid: %in1_valid: i1, in1_dest: %in1_dest: i1, in1_data: %in1_data: i4, out0_ack: %out0_ack: i1, out1_ack: %out1_ack: i1) -> (in0_ack: i1, in1_ack: i1, out0_valid: i1, out0_data: i4, out1_valid: i1, out1_data: i4, count0: i2, count1: i2, grant00: i1, grant10: i1, grant01: i1, grant11: i1, pop0: i1, pop1: i1, push0: i1, push1: i1, rr0: i1, rr1: i1)
    %reference0.head_dest, %reference0.head_data, %reference0.count = hw.instance "reference0" @ingress_reference(clk: %clk: i1, reset_n: %reset_n: i1, push: %dut.push0: i1, push_dest: %in0_dest: i1, push_data: %in0_data: i4, pop: %dut.pop0: i1) -> (head_dest: i1, head_data: i4, count: i2) {sv.namehint = "ref_head0_dest"}
    %reference1.head_dest, %reference1.head_data, %reference1.count = hw.instance "reference1" @ingress_reference(clk: %clk: i1, reset_n: %reset_n: i1, push: %dut.push1: i1, push_dest: %in1_dest: i1, push_data: %in1_data: i4, pop: %dut.pop1: i1) -> (head_dest: i1, head_data: i4, count: i2) {sv.namehint = "ref_head1_dest"}
    %0 = comb.icmp ne %reference0.count, %c0_i2 : i2
    %1 = comb.xor bin %reference0.head_dest, %true : i1
    %2 = comb.and bin %0, %1 : i1
    %3 = comb.icmp ne %reference1.count, %c0_i2 : i2
    %4 = comb.xor bin %reference1.head_dest, %true : i1
    %5 = comb.and bin %3, %4 : i1
    %6 = comb.and bin %0, %reference0.head_dest : i1
    %7 = comb.and bin %3, %reference1.head_dest : i1
    %8 = comb.extract %stall_history from 0 : (i2) -> i1
    %9 = comb.xor bin %out0_ack, %true : i1
    %10 = comb.and bin %dut.out0_valid, %9 : i1
    %11 = comb.concat %8, %10 : i1, i1
    %12 = comb.mux bin %reset_n, %11, %c0_i2 : i2
    %13 = seq.to_clock %clk
    %stall_history = seq.firreg %12 clock %13 : i2
    %14 = seq.compreg %in0_valid, %13 : i1  
    %15 = seq.compreg %in0_dest, %13 : i1  
    %16 = seq.compreg %in0_data, %13 : i4  
    %17 = comb.xor bin %dut.in0_ack, %true : i1
    %18 = comb.icmp eq %14, %in0_valid : i1
    %19 = comb.icmp eq %15, %in0_dest : i1
    %20 = comb.icmp eq %16, %in0_data : i4
    %21 = comb.and bin %18, %19, %20 : i1
    %22 = comb.and %reset_n, %in0_valid, %17 : i1
    %23 = comb.xor %reset_n, %true : i1
    %24 = seq.firreg %22 clock %13 reset async %23, %false preset 0 : i1
    verif.clocked_assume %21 if %24, posedge %clk : i1
    %25 = seq.compreg %in1_valid, %13 : i1  
    %26 = seq.compreg %in1_dest, %13 : i1  
    %27 = seq.compreg %in1_data, %13 : i4  
    %28 = comb.xor bin %dut.in1_ack, %true : i1
    %29 = comb.icmp eq %25, %in1_valid : i1
    %30 = comb.icmp eq %26, %in1_dest : i1
    %31 = comb.icmp eq %27, %in1_data : i4
    %32 = comb.and bin %29, %30, %31 : i1
    %33 = comb.and %reset_n, %in1_valid, %28 : i1
    %34 = seq.firreg %33 clock %13 reset async %23, %false preset 0 : i1
    verif.clocked_assume %32 if %34, posedge %clk : i1
    %35 = comb.xor bin %reset_n, %true : i1
    %36 = comb.icmp eq %dut.count0, %c0_i2 : i2
    %37 = comb.icmp eq %dut.count1, %c0_i2 : i2
    %38 = comb.icmp eq %reference0.count, %c0_i2 : i2
    %39 = comb.icmp eq %reference1.count, %c0_i2 : i2
    %40 = comb.and bin %36, %37, %38, %39 : i1
    %41 = seq.firreg %35 clock %13 preset 0 : i1
    verif.clocked_assert %40 if %41, posedge %clk : i1
    %42 = seq.compreg %dut.out0_valid, %13 : i1  
    %43 = seq.compreg %dut.out0_data, %13 : i4  
    %44 = comb.icmp eq %42, %dut.out0_valid : i1
    %45 = comb.icmp eq %43, %dut.out0_data : i4
    %46 = comb.and bin %44, %45 : i1
    %47 = comb.and %reset_n, %10 : i1
    %48 = seq.firreg %47 clock %13 reset async %23, %false preset 0 : i1
    verif.clocked_assert %46 if %48, posedge %clk : i1
    %49 = seq.compreg %dut.out1_valid, %13 : i1  
    %50 = seq.compreg %dut.out1_data, %13 : i4  
    %51 = comb.xor bin %out1_ack, %true : i1
    %52 = comb.and bin %dut.out1_valid, %51 : i1
    %53 = comb.icmp eq %49, %dut.out1_valid : i1
    %54 = comb.icmp eq %50, %dut.out1_data : i4
    %55 = comb.and bin %53, %54 : i1
    %56 = comb.and %reset_n, %52 : i1
    %57 = seq.firreg %56 clock %13 reset async %23, %false preset 0 : i1
    verif.clocked_assert %55 if %57, posedge %clk : i1
    %58 = comb.icmp eq %dut.count0, %reference0.count : i2
    verif.clocked_assert %58 if %reset_n, posedge %clk : i1
    %59 = comb.icmp eq %dut.count1, %reference1.count : i2
    verif.clocked_assert %59 if %reset_n, posedge %clk : i1
    %60 = comb.icmp ne %reference0.count, %c-1_i2 : i2
    %61 = comb.or bin %60, %dut.pop0 : i1
    %62 = comb.icmp eq %dut.in0_ack, %61 : i1
    verif.clocked_assert %62 if %reset_n, posedge %clk : i1
    %63 = comb.and bin %58, %62 : i1
    verif.clocked_assert %63 if %reset_n, posedge %clk : i1
    %64 = comb.icmp ne %reference1.count, %c-1_i2 : i2
    %65 = comb.or bin %64, %dut.pop1 : i1
    %66 = comb.icmp eq %dut.in1_ack, %65 : i1
    verif.clocked_assert %66 if %reset_n, posedge %clk : i1
    %67 = comb.and bin %59, %66 : i1
    verif.clocked_assert %67 if %reset_n, posedge %clk : i1
    %68 = comb.and bin %dut.grant00, %dut.grant10 : i1
    %69 = comb.xor bin %68, %true : i1
    verif.clocked_assert %69 if %reset_n, posedge %clk : i1
    %70 = comb.and bin %dut.grant01, %dut.grant11 : i1
    %71 = comb.xor bin %70, %true : i1
    verif.clocked_assert %71 if %reset_n, posedge %clk : i1
    %72 = comb.and bin %dut.grant00, %dut.grant01 : i1
    %73 = comb.xor bin %72, %true : i1
    verif.clocked_assert %73 if %reset_n, posedge %clk : i1
    %74 = comb.and bin %dut.grant10, %dut.grant11 : i1
    %75 = comb.xor bin %74, %true : i1
    verif.clocked_assert %75 if %reset_n, posedge %clk : i1
    %76 = comb.and %reset_n, %dut.grant00 : i1
    verif.clocked_assert %2 if %76, posedge %clk : i1
    %77 = comb.and %reset_n, %dut.grant10 : i1
    verif.clocked_assert %5 if %77, posedge %clk : i1
    %78 = comb.and %reset_n, %dut.grant01 : i1
    verif.clocked_assert %6 if %78, posedge %clk : i1
    %79 = comb.and %reset_n, %dut.grant11 : i1
    verif.clocked_assert %7 if %79, posedge %clk : i1
    %80 = comb.mux bin %dut.grant00, %reference0.head_data, %reference1.head_data : i4
    %81 = comb.icmp eq %dut.out0_data, %80 : i4
    %82 = comb.and %reset_n, %dut.out0_valid : i1
    verif.clocked_assert %81 if %82, posedge %clk : i1
    %83 = comb.mux bin %dut.grant01, %reference0.head_data, %reference1.head_data : i4
    %84 = comb.icmp eq %dut.out1_data, %83 : i4
    %85 = comb.and %reset_n, %dut.out1_valid : i1
    verif.clocked_assert %84 if %85, posedge %clk : i1
    %86 = seq.compreg %10, %13 : i1  
    %87 = comb.xor bin %86, %true : i1
    %88 = comb.xor bin %5, %true : i1
    %89 = comb.xor bin %dut.rr0, %true : i1
    %90 = comb.or bin %88, %89 : i1
    %91 = comb.and bin %2, %90 : i1
    %92 = comb.icmp eq %dut.grant00, %91 : i1
    %93 = comb.and %reset_n, %87 : i1
    verif.clocked_assert %92 if %93, posedge %clk : i1
    %94 = comb.xor bin %2, %true : i1
    %95 = comb.or bin %94, %dut.rr0 : i1
    %96 = comb.and bin %5, %95 : i1
    %97 = comb.icmp eq %dut.grant10, %96 : i1
    verif.clocked_assert %97 if %93, posedge %clk : i1
    %98 = seq.compreg %52, %13 : i1  
    %99 = comb.xor bin %98, %true : i1
    %100 = comb.xor bin %7, %true : i1
    %101 = comb.xor bin %dut.rr1, %true : i1
    %102 = comb.or bin %100, %101 : i1
    %103 = comb.and bin %6, %102 : i1
    %104 = comb.icmp eq %dut.grant01, %103 : i1
    %105 = comb.and %reset_n, %99 : i1
    verif.clocked_assert %104 if %105, posedge %clk : i1
    %106 = comb.xor bin %6, %true : i1
    %107 = comb.or bin %106, %dut.rr1 : i1
    %108 = comb.and bin %7, %107 : i1
    %109 = comb.icmp eq %dut.grant11, %108 : i1
    verif.clocked_assert %109 if %105, posedge %clk : i1
    %110 = seq.compreg %dut.rr0, %13 : i1  
    %111 = comb.and bin %2, %5, %out0_ack : i1
    %112 = comb.xor bin %111, %true : i1
    %113 = comb.icmp eq %110, %dut.rr0 : i1
    %114 = comb.and %reset_n, %112 : i1
    %115 = seq.firreg %114 clock %13 reset async %23, %false preset 0 : i1
    verif.clocked_assert %113 if %115, posedge %clk : i1
    %116 = seq.compreg %dut.rr1, %13 : i1  
    %117 = comb.and bin %6, %7, %out1_ack : i1
    %118 = comb.xor bin %117, %true : i1
    %119 = comb.icmp eq %116, %dut.rr1 : i1
    %120 = comb.and %reset_n, %118 : i1
    %121 = seq.firreg %120 clock %13 reset async %23, %false preset 0 : i1
    verif.clocked_assert %119 if %121, posedge %clk : i1
    %122 = seq.compreg %dut.grant00, %13 : i1  
    %123 = comb.icmp eq %dut.rr0, %122 : i1
    %124 = comb.and %reset_n, %111 : i1
    %125 = seq.firreg %124 clock %13 reset async %23, %false preset 0 : i1
    verif.clocked_assert %123 if %125, posedge %clk : i1
    %126 = seq.compreg %dut.grant01, %13 : i1  
    %127 = comb.icmp eq %dut.rr1, %126 : i1
    %128 = comb.and %reset_n, %117 : i1
    %129 = seq.firreg %128 clock %13 reset async %23, %false preset 0 : i1
    verif.clocked_assert %127 if %129, posedge %clk : i1
    %130 = comb.icmp eq %reference0.count, %c-1_i2 : i2
    %131 = comb.icmp eq %reference1.count, %c-1_i2 : i2
    %132 = comb.and bin %130, %131 : i1
    verif.clocked_cover %132 if %reset_n, posedge %clk : i1
    %133 = comb.and bin %2, %5 : i1
    %134 = comb.and bin %6, %7 : i1
    %135 = comb.or bin %133, %134 : i1
    verif.clocked_cover %135 if %reset_n, posedge %clk : i1
    %136 = comb.and bin %dut.out0_valid, %out0_ack, %dut.out1_valid, %out1_ack : i1
    verif.clocked_cover %136 if %reset_n, posedge %clk : i1
    %137 = comb.icmp eq %stall_history, %c-1_i2 : i2
    %138 = comb.and bin %137, %dut.out0_valid, %out0_ack : i1
    verif.clocked_cover %138 if %reset_n, posedge %clk : i1
    %139 = comb.and bin %130, %dut.pop0, %dut.push0 : i1
    %140 = comb.and bin %131, %dut.pop1, %dut.push1 : i1
    %141 = comb.or bin %139, %140 : i1
    verif.clocked_cover %141 if %reset_n, posedge %clk : i1
    %circt_past_valid = seq.firreg %true clock %13 preset 0 : i1
    %142 = comb.icmp eq %reset_n, %circt_past_valid : i1
    verif.clocked_assume %142, posedge %clk : i1
    hw.output %dut.in0_ack, %dut.in1_ack, %dut.out0_valid, %dut.out0_data, %dut.out1_valid, %dut.out1_data : i1, i1, i1, i4, i1, i4
  }
}

