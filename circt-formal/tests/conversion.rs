use std::{fs, path::Path};

use circt_formal::{
    convert::{ConversionOptions, NamedFsm, NamedSignal},
    export::{self, AigerFormat, ExportOptions},
};
use formal_utils::{
    formats::aiger::{ascii::read_aiger_ascii, binary::read_aiger_binary},
    fsm::verify::VerifyOrdering,
    sim::Simulator,
    value::Value,
};
use parser_circt::parser::parse;

fn options(reset: Option<&str>) -> ConversionOptions {
    ConversionOptions {
        top: None,
        clock: "clk".parse().unwrap(),
        reset: reset.map(|s| s.parse().unwrap()),
    }
}

fn source(source: &str) -> NamedFsm {
    NamedFsm::from_source(7, source, &options(None)).unwrap()
}

fn fixture(name: &str, reset: bool) -> NamedFsm {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../circt-parser/res/{name}.hw.mlir"));
    let source = fs::read_to_string(path).unwrap();
    let reset = if reset {
        Some(if name == "stream_stage" {
            "rst"
        } else {
            "!reset_n"
        })
    } else {
        None
    };
    NamedFsm::from_source(0, &source, &options(reset)).unwrap()
}

fn simulator(model: &NamedFsm) -> Simulator {
    let mut sim = Simulator::from(model.fsm.clone());
    for input in model.fsm.get_inputs() {
        sim.set_input(input.index(), Some(false));
    }
    sim
}

fn set(model: &NamedFsm, sim: &mut Simulator, name: &str, value: u64) {
    let signal = (&model.inputs)
        .into_iter()
        .find(|signal| signal.name == name)
        .unwrap();
    for bit in &signal.bits {
        sim.set_input(
            bit.value.unwrap_variable().index(),
            Some(value & (1 << bit.index) != 0),
        );
    }
}

fn word(signal: &NamedSignal, sim: &Simulator) -> u64 {
    (&signal.bits).into_iter().fold(0, |word, bit| {
        word | (u64::from(sim.get_value_signed(bit.value).unwrap()) << bit.index)
    })
}

fn output(model: &NamedFsm, sim: &Simulator, name: &str) -> u64 {
    word(
        (&model.outputs)
            .into_iter()
            .find(|signal| signal.name == name)
            .unwrap(),
        sim,
    )
}
fn tick(sim: &mut Simulator) {
    sim.eval();
    sim.step();
    sim.eval();
}

fn check_fixture(name: &str) {
    for reset in [false, true] {
        let model = fixture(name, reset);
        model.fsm.verify(VerifyOrdering::Verify);
        assert!(!model.registers.is_empty());
        assert!(
            (&model.inputs)
                .into_iter()
                .all(|signal| signal.name != "clk")
        );
        for format in [AigerFormat::Ascii, AigerFormat::Binary] {
            let bytes = export::write(&model.fsm, format, &ExportOptions::default()).unwrap();
            let (read, _) = match format {
                AigerFormat::Ascii => read_aiger_ascii(&bytes),
                AigerFormat::Binary => read_aiger_binary(&bytes),
            };
            assert_eq!(read.get_inputs().len(), model.fsm.get_inputs().len());
            assert_eq!(read.get_latches().len(), model.fsm.get_latches().len());
            assert_eq!(read.get_outputs().len(), model.fsm.get_outputs().len());
            assert_eq!(read.get_asserts().len(), model.assertions.len());
            assert_eq!(read.get_assumes().len(), model.assumptions.len());
            assert_eq!(
                export::symbols(&read),
                export::symbols(&export::prepare(&model.fsm, &ExportOptions::default()).unwrap())
            );
            assert!(read.get_covers().is_empty());
        }
    }
}

#[test]
fn data_types() {
    check_fixture("data_types");
    let model = fixture("data_types", false);
    let overlay = (&model.registers)
        .into_iter()
        .find(|signal| signal.name == "overlay_value")
        .unwrap();
    assert_eq!(overlay.bits.len(), 16);
    let mut sim = simulator(&model);
    set(&model, &mut sim, "data_in", 0x5a);
    tick(&mut sim);
    assert_eq!(word(overlay, &sim), 0x5a5a);
    tick(&mut sim);
    assert_eq!(output(&model, &sim, "data_out"), 0x5a5a ^ 0x5ab0 ^ 23100);
    assert_eq!(output(&model, &sim, "shortint_out"), 0x5a5a);
    assert_eq!(output(&model, &sim, "longint_out"), 0x5a5a5a5a5a5a5a5a);
}

#[test]
fn case_statements() {
    check_fixture("case_statements");
}

#[test]
fn combinational_loops() {
    check_fixture("combinational_loops");
}

#[test]
fn counter() {
    check_fixture("counter");
}

#[test]
fn counter_free() {
    check_fixture("counter_free");
}

#[test]
fn counter_ones() {
    check_fixture("counter_ones");
}

#[test]
fn fifo_stage() {
    check_fixture("fifo_stage");
}

#[test]
fn fifo_stage_bypass() {
    check_fixture("fifo_stage_bypass");
}

#[test]
fn fifo_stage_fail_assert() {
    check_fixture("fifo_stage_fail_assert");
}

#[test]
fn fifo_stage_fail_free() {
    check_fixture("fifo_stage_fail_free");
}

#[test]
fn multidim_arrays() {
    check_fixture("multidim_arrays");
}

#[test]
fn package_properties() {
    check_fixture("package_properties");
}

#[test]
fn packet_switch() {
    check_fixture("packet_switch");
}

#[test]
fn public_submodules() {
    check_fixture("public_submodules");
}

#[test]
fn signed_operations() {
    check_fixture("signed_operations");
}

#[test]
fn stream_stage() {
    check_fixture("stream_stage");
}

#[test]
fn wire_ports() {
    check_fixture("wire_ports");
}

#[test]
fn counter_reset_and_history_are_consistent() {
    let model = fixture("counter", true);
    assert_eq!(model.inputs.len(), 1);
    assert_eq!(model.inputs[0].name, "enable");
    assert_eq!(model.warnings.len(), 1);
    assert!(model.warnings[0].contains("between-edge"));
    let mut sim = simulator(&model);
    sim.eval();
    assert_eq!(output(&model, &sim, "count"), 0);
    for assume in 0..model.assumptions.len() {
        assert_eq!(sim.get_assume(assume), Some(true));
    }
    for expected in 1..=10 {
        set(&model, &mut sim, "enable", 1);
        tick(&mut sim);
        assert_eq!(output(&model, &sim, "count"), expected);
        set(&model, &mut sim, "enable", 0);
        tick(&mut sim);
        assert_eq!(output(&model, &sim, "count"), expected);
        assert_eq!(sim.get_assert(0), Some(true));
    }
    let history = (&model.registers)
        .into_iter()
        .find(|s| s.name == "circt_past_valid")
        .unwrap();
    assert_eq!(word(history, &sim), 1);
    let raw = fixture("counter", false);
    assert!((&raw.inputs).into_iter().any(|s| s.name == "reset_n"));
    let count = (&raw.registers)
        .into_iter()
        .find(|s| s.name == "count")
        .unwrap();
    assert!((&count.bits).into_iter().all(|bit| {
        raw.fsm
            .get_latch(bit.value.unwrap_variable().index())
            .unwrap()
            .reset_value
            .is_none()
    }));
    let mut sim = simulator(&raw);
    tick(&mut sim);
    assert_eq!(output(&raw, &sim, "count"), 0);
}

#[test]
fn hierarchy_has_independent_state_and_named_ports() {
    let model = fixture("public_submodules", true);
    assert!(
        (&model.registers)
            .into_iter()
            .any(|s| s.name == "counter0::state")
    );
    assert!(
        (&model.registers)
            .into_iter()
            .any(|s| s.name == "counter1::state")
    );
    let mut sim = simulator(&model);
    set(&model, &mut sim, "enable0", 1);
    for count in 1..=4 {
        tick(&mut sim);
        assert_eq!(output(&model, &sim, "count0"), count);
        assert_eq!(output(&model, &sim, "count1"), 0);
    }
    set(&model, &mut sim, "enable0", 0);
    set(&model, &mut sim, "enable1", 1);
    tick(&mut sim);
    assert_eq!(output(&model, &sim, "count0"), 4);
    assert_eq!(output(&model, &sim, "count1"), 1);
}

#[test]
fn grouped_results_forward_references_and_simultaneous_updates() {
    let model = source(
        r#"
    module {
      hw.module private @child(in %clk: i1, in %d: i4, out q: i4, out twice: i4) {
        %clock = seq.to_clock %clk
        %q = seq.firreg %d clock %clock preset 1 : i4
        %sum = comb.add %q, %q : i4
        hw.output %q, %sum : i4, i4
      }
      hw.module @parent(in %clk: i1, out a: i4, out b: i4, out sum: i4) {
        %left:2 = hw.instance "left" @child(d: %right#0: i4, clk: %clk: i1) -> (q: i4, twice: i4)
        %right:2 = hw.instance "right" @child(clk: %clk: i1, d: %next: i4) -> (q: i4, twice: i4)
        %one = hw.constant 1 : i4
        %next = comb.add %left#0, %one : i4
        hw.output %left#0, %right#0, %left#1 : i4, i4, i4
      }
    }"#,
    );
    let mut sim = simulator(&model);
    sim.eval();
    assert_eq!(output(&model, &sim, "a"), 1);
    assert_eq!(output(&model, &sim, "sum"), 2);
    tick(&mut sim);
    assert_eq!(output(&model, &sim, "a"), 1);
    assert_eq!(output(&model, &sim, "b"), 2);
    tick(&mut sim);
    assert_eq!(output(&model, &sim, "a"), 2);
    assert_eq!(output(&model, &sim, "b"), 2);
}

#[test]
fn signed_arithmetic_matches_fixed_width_reference() {
    let model = fixture("signed_operations", true);
    let mut sim = simulator(&model);
    for lhs in 0..16 {
        for rhs in 0..16 {
            set(&model, &mut sim, "lhs", lhs);
            set(&model, &mut sim, "rhs", rhs);
            tick(&mut sim);
            assert_eq!(output(&model, &sim, "captured_sum"), (lhs + rhs) & 15);
            assert_eq!(
                output(&model, &sim, "captured_signed_product"),
                (lhs * rhs) & 15
            );
            assert_eq!(
                output(&model, &sim, "captured_unsigned_product"),
                (lhs * rhs) & 15
            );
            if let Some(quotient) = lhs.checked_div(rhs) {
                assert_eq!(output(&model, &sim, "captured_unsigned_quotient"), quotient);
                let signed = |value| {
                    if value < 8 {
                        value as i64
                    } else {
                        value as i64 - 16
                    }
                };
                assert_eq!(
                    output(&model, &sim, "captured_signed_quotient"),
                    (signed(lhs) / signed(rhs)) as u64 & 15
                );
            }
            for index in 0..model.assertions.len() {
                assert_eq!(sim.get_assert(index), Some(true));
            }
        }
    }
}

#[test]
fn nested_array_reads_and_writes_follow_circt_indices() {
    let model = fixture("multidim_arrays", true);
    let mut sim = simulator(&model);
    set(&model, &mut sim, "write_enable", 1);
    for row in 0..2 {
        for column in 0..3 {
            set(&model, &mut sim, "write_row", row);
            set(&model, &mut sim, "write_column", column);
            set(&model, &mut sim, "write_data", row * 3 + column + 1);
            tick(&mut sim);
        }
    }
    set(&model, &mut sim, "write_enable", 0);
    for row in 0..2 {
        for column in 0..3 {
            set(&model, &mut sim, "read_row", row);
            set(&model, &mut sim, "read_column", column);
            sim.eval();
            assert_eq!(output(&model, &sim, "read_data"), row * 3 + column + 1);
        }
    }
}

#[test]
fn presets_sync_and_async_reset_and_optional_reset() {
    let source_text = r#"hw.module @m(in %clk: i1, in %rst: i1, in %d: i4, out a: i4, out b: i4) {
      %clock = seq.to_clock %clk
      %zero = hw.constant 3 : i4
      %a = seq.firreg %d clock %clock reset sync %rst, %zero preset 9 : i4
      %b = seq.firreg %d clock %clock reset async %rst, %zero preset 5 : i4
      hw.output %a, %b : i4, i4
    }"#;
    let model = source(source_text);
    let mut sim = simulator(&model);
    sim.eval();
    assert_eq!(output(&model, &sim, "a"), 9);
    assert_eq!(output(&model, &sim, "b"), 5);
    set(&model, &mut sim, "d", 7);
    set(&model, &mut sim, "rst", 1);
    tick(&mut sim);
    assert_eq!(output(&model, &sim, "a"), 3);
    assert_eq!(output(&model, &sim, "b"), 3);
    set(&model, &mut sim, "rst", 0);
    tick(&mut sim);
    assert_eq!(output(&model, &sim, "a"), 7);
    let model = NamedFsm::from_source(0, source_text, &options(Some("rst"))).unwrap();
    let mut sim = simulator(&model);
    sim.eval();
    assert_eq!(output(&model, &sim, "a"), 3);
    assert!((&model.inputs).into_iter().all(|s| s.name != "rst"));
}

#[test]
fn clock_polarity_and_data_use_are_validated() {
    let text = r#"hw.module @m(in %clk: i1, in %d: i1, out q: i1) {
        %true = hw.constant true
        %n = comb.xor %clk, %true : i1
        %clock = seq.to_clock %n
        %q = seq.firreg %d clock %clock preset 0 : i1
        verif.clocked_assert %d, negedge %clk : i1
        hw.output %q : i1
    }"#;
    let mut opts = options(None);
    opts.clock = "!clk".parse().unwrap();
    assert!(NamedFsm::from_source(0, text, &opts).is_ok());
    assert!(
        NamedFsm::from_source(0, text, &options(None))
            .unwrap_err()
            .to_string()
            .contains("clock domain")
    );
    let invalid = "hw.module @m(in %clk: i1, out o: i1) { hw.output %clk : i1 }";
    assert!(
        NamedFsm::from_source(0, invalid, &options(None))
            .unwrap_err()
            .to_string()
            .contains("clock used")
    );
    let other = text.replace("negedge %clk", "posedge %d");
    assert!(NamedFsm::from_source(0, &other, &opts).is_err());
    let derived = text.replace("comb.xor %clk, %true", "comb.and %clk, %d");
    assert!(
        NamedFsm::from_source(0, &derived, &opts)
            .unwrap_err()
            .to_string()
            .contains("derived/gated")
    );
}

#[test]
fn enabled_properties_preserve_polarity_and_unique_labels() {
    let model = source(
        r#"hw.module @m(in %clk: i1, in %p: i1, in %e: i1) {
        verif.clocked_assert %p if %e, posedge %clk label "same" : i1
        verif.clocked_assert %p, posedge %clk label "same" : i1
        verif.clocked_assume %p if %e, posedge %clk : i1
        verif.clocked_cover %p if %e, posedge %clk label "" : i1
        hw.output
    }"#,
    );
    assert_ne!(model.assertions[0].name, model.assertions[1].name);
    let mut sim = simulator(&model);
    for p in 0..2 {
        for e in 0..2 {
            set(&model, &mut sim, "p", p);
            set(&model, &mut sim, "e", e);
            sim.eval();
            assert_eq!(sim.get_assert(0), Some(e == 0 || p == 1));
            assert_eq!(sim.get_assert(1), Some(p == 1));
            assert_eq!(sim.get_assume(0), Some(e == 0 || p == 1));
            assert_eq!(sim.get_cover(0), Some(e == 1 && p == 1));
        }
    }
    let selected = export::prepare(
        &model.fsm,
        &ExportOptions {
            cover: Some(0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(selected.get_asserts().len(), 1);
    assert_eq!(selected.get_assumes().len(), 1);
    assert!(
        export::prepare(
            &model.fsm,
            &ExportOptions {
                assertion: Some(2),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        export::prepare(
            &model.fsm,
            &ExportOptions {
                assertion: Some(0),
                cover: Some(0),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(model.fsm.get_covers().len(), 1);
    let mut sim = Simulator::from(selected);
    sim.eval();
    for input in model.fsm.get_inputs() {
        sim.set_input(input.index(), Some(true));
    }
    sim.eval();
    assert_eq!(sim.get_assert(0), Some(false));
}

#[test]
fn undefined_results_are_shared_and_nondeterministic() {
    let model = source(
        r#"hw.module @m(in %clk: i1, in %a: i4, in %b: i4, in %array: !hw.array<3xi4>, in %idx: i2, out d: i4, out d2: i4, out r: i4, out item: i4, out updated: !hw.array<3xi4>) {
        %d = comb.divu %a, %b : i4
        %r = comb.mods %a, %b : i4
        %item = hw.array_get %array[%idx] : !hw.array<3xi4>, i2
        %updated = hw.array_inject %array[%idx], %a : !hw.array<3xi4>, i2
        hw.output %d, %d, %r, %item, %updated : i4, i4, i4, i4, !hw.array<3xi4>
    }"#,
    );
    let mut sim = simulator(&model);
    set(&model, &mut sim, "a", 7);
    set(&model, &mut sim, "idx", 3);
    for name in ["$undefined::d", "$undefined::r", "$undefined::item"] {
        set(&model, &mut sim, name, 9);
    }
    set(&model, &mut sim, "$undefined::updated", 0xabc);
    sim.eval();
    assert_eq!(output(&model, &sim, "d"), 9);
    assert_eq!(output(&model, &sim, "d2"), 9);
    assert_eq!(output(&model, &sim, "r"), 9);
    assert_eq!(output(&model, &sim, "item"), 9);
    assert_eq!(output(&model, &sim, "updated"), 0xabc);
    set(&model, &mut sim, "b", 3);
    set(&model, &mut sim, "idx", 1);
    set(&model, &mut sim, "array", 0x321);
    sim.eval();
    assert_eq!(output(&model, &sim, "d"), 2);
    assert_eq!(output(&model, &sim, "r"), 1);
    assert_eq!(output(&model, &sim, "item"), 2);
    assert_eq!(output(&model, &sim, "updated"), 0x371);
}

#[test]
fn arbitrary_width_literals_concat_extract_and_struct_passthrough() {
    let model = source(
        r#"hw.module @m(in %clk: i1, in %s: !hw.struct<a: i4, b: i8>, out huge: i256, out neg: i256, out low: i8, out joined: i12, out record: !hw.struct<a: i4, b: i8>) {
        %huge = hw.constant 340282366920938463463374607431768211456 : i256
        %neg = hw.constant -1 : i256
        %hex = hw.constant 0xabc : i12
        %low = comb.extract %hex from 0 : (i12) -> i8
        %a = hw.constant 0xa : i4
        %b = hw.constant 0xbc : i8
        %joined = comb.concat %a, %b : i4, i8
        hw.output %huge, %neg, %low, %joined, %s : i256, i256, i8, i12, !hw.struct<a: i4, b: i8>
    }"#,
    );
    let mut sim = simulator(&model);
    set(&model, &mut sim, "s", 0x123);
    sim.eval();
    assert_eq!(output(&model, &sim, "low"), 0xbc);
    assert_eq!(output(&model, &sim, "joined"), 0xabc);
    assert_eq!(output(&model, &sim, "record"), 0x123);
    for bit in &model.outputs[0].bits {
        assert_eq!(sim.get_value_signed(bit.value), Some(bit.index == 128));
    }
    assert!(
        (&model.outputs[1].bits)
            .into_iter()
            .all(|bit| bit.value == Value::Constant(true))
    );
}

#[test]
fn replication_repeats_lsb_first_bits_without_extra_logic() {
    let model = source(
        r#"hw.module @m(in %clk: i1, in %a: i3, in %b: i1, out repeated: i12, out bit: i8, out same: i3, out wide: i129) {
        %repeated = comb.replicate %later : (i3) -> i12
        %later = comb.replicate %a : (i3) -> i3
        %bit = comb.replicate %b : (i1) -> i8
        %wide = comb.replicate %a : (i3) -> i129
        hw.output %repeated, %bit, %later, %wide : i12, i8, i3, i129
    }"#,
    );
    model.fsm.verify(VerifyOrdering::Verify);
    let mut sim = simulator(&model);
    for a in 0..8 {
        for b in 0..2 {
            set(&model, &mut sim, "a", a);
            set(&model, &mut sim, "b", b);
            sim.eval();
            assert_eq!(output(&model, &sim, "repeated"), a * 0x249);
            assert_eq!(output(&model, &sim, "bit"), b * 0xff);
            assert_eq!(output(&model, &sim, "same"), a);
            for bit in &model.outputs[3].bits {
                assert_eq!(
                    sim.get_value_signed(bit.value),
                    Some(a & (1 << (bit.index % 3)) != 0)
                );
            }
        }
    }
    for (result, input, width) in [
        (&model.outputs[0], &model.inputs[0], 3),
        (&model.outputs[1], &model.inputs[1], 1),
        (&model.outputs[2], &model.inputs[0], 3),
        (&model.outputs[3], &model.inputs[0], 3),
    ] {
        for bit in &result.bits {
            assert_eq!(bit.value, input.bits[(bit.index % width) as usize].value);
        }
    }
}

#[test]
fn bitcasts_preserve_bits_through_nested_aggregates_and_signedness() {
    let model = source(
        r#"hw.module @m(in %clk: i1, in %bits: i12, in %record: !hw.struct<hi: i4, lo: !hw.array<2xi4>>, out packed: !hw.struct<hi: i4, lo: !hw.array<2xi4>>, out restored: i12, out record_bits: i12, out signed_bits: si12, out row: i6, out cell: i2) {
        %restored = hw.bitcast %packed : (!hw.struct<hi: i4, lo: !hw.array<2xi4>>) -> i12
        %packed = hw.bitcast %bits : (i12) -> !hw.struct<hi: i4, lo: !hw.array<2xi4>>
        %record_bits = hw.bitcast %record : (!hw.struct<hi: i4, lo: !hw.array<2xi4>>) -> i12
        %signed_bits = hw.bitcast %restored : (i12) -> si12
        %matrix = hw.bitcast %packed : (!hw.struct<hi: i4, lo: !hw.array<2xi4>>) -> !hw.array<2xarray<3xi2>>
        %one = hw.constant 1 : i1
        %two = hw.constant 2 : i2
        %row = hw.array_get %matrix[%one] : !hw.array<2xarray<3xi2>>, i1
        %row_bits = hw.bitcast %row : (!hw.array<3xi2>) -> i6
        %cell = hw.array_get %row[%two] : !hw.array<3xi2>, i2
        hw.output %packed, %restored, %record_bits, %signed_bits, %row_bits, %cell : !hw.struct<hi: i4, lo: !hw.array<2xi4>>, i12, i12, si12, i6, i2
    }"#,
    );
    model.fsm.verify(VerifyOrdering::Verify);
    let mut sim = simulator(&model);
    for bits in [0, 1, 0x123, 0xabc, 0xfff] {
        set(&model, &mut sim, "bits", bits);
        set(&model, &mut sim, "record", bits ^ 0xfff);
        sim.eval();
        for name in ["packed", "restored", "signed_bits"] {
            assert_eq!(output(&model, &sim, name), bits);
        }
        assert_eq!(output(&model, &sim, "record_bits"), bits ^ 0xfff);
        assert_eq!(output(&model, &sim, "row"), bits >> 6);
        assert_eq!(output(&model, &sim, "cell"), bits >> 10);
    }
    for (result_index, input_index) in [(0, 0), (1, 0), (2, 1), (3, 0)] {
        for (result, input) in (&model.outputs[result_index].bits)
            .into_iter()
            .zip(&model.inputs[input_index].bits)
        {
            assert_eq!(result.value, input.value);
        }
    }
}

#[test]
fn bitcasts_preserve_arbitrary_width_values() {
    let model = source(
        r#"hw.module @m(in %clk: i1, out array: !hw.array<2xi128>, out restored: i256) {
        %value = hw.constant 340282366920938463463374607431768211457 : i256
        %array = hw.bitcast %value : (i256) -> !hw.array<2xi128>
        %restored = hw.bitcast %array : (!hw.array<2xi128>) -> i256
        hw.output %array, %restored : !hw.array<2xi128>, i256
    }"#,
    );
    let mut sim = simulator(&model);
    sim.eval();
    for signal in &model.outputs {
        for bit in &signal.bits {
            assert_eq!(
                sim.get_value_signed(bit.value),
                Some(bit.index == 0 || bit.index == 128)
            );
        }
    }
}

#[test]
fn union_extraction_uses_overlapping_storage_and_member_offsets() {
    let model = source(
        r#"hw.module @m(in %clk: i1, in %bits: i24, out raw: i16, out nibble: i4, out high: i4, out fields: !hw.struct<a: i4, b: i4>, out lane: i12) {
        %raw = hw.union_extract %u["raw"] : !hw.union<raw: i16, nibble: i4 offset 8, high: i4 offset 20, fields: !hw.struct<a: i4, b: i4> offset 4>
        %u = hw.bitcast %bits : (i24) -> !hw.union<raw: i16, nibble: i4 offset 8, high: i4 offset 20, fields: !hw.struct<a: i4, b: i4> offset 4>
        %nibble = hw.union_extract %u["nibble"] : !hw.union<raw: i16, nibble: i4 offset 8, high: i4 offset 20, fields: !hw.struct<a: i4, b: i4> offset 4>
        %high = hw.union_extract %u["high"] : !hw.union<raw: i16, nibble: i4 offset 8, high: i4 offset 20, fields: !hw.struct<a: i4, b: i4> offset 4>
        %fields = hw.union_extract %u["fields"] : !hw.union<raw: i16, nibble: i4 offset 8, high: i4 offset 20, fields: !hw.struct<a: i4, b: i4> offset 4>
        %array = hw.bitcast %bits : (i24) -> !hw.array<2xunion<small: i3, raw: i12>>
        %one = hw.constant true
        %element = hw.array_get %array[%one] : !hw.array<2xunion<small: i3, raw: i12>>, i1
        %lane = hw.union_extract %element["raw"] : !hw.union<small: i3, raw: i12>
        hw.output %raw, %nibble, %high, %fields, %lane : i16, i4, i4, !hw.struct<a: i4, b: i4>, i12
    }"#,
    );
    model.fsm.verify(VerifyOrdering::Verify);
    let mut sim = simulator(&model);
    for bits in [0, 1, 0x123456, 0xabcdef, 0xffffff] {
        set(&model, &mut sim, "bits", bits);
        sim.eval();
        for (name, shift, mask) in [
            ("raw", 0, 0xffff),
            ("nibble", 8, 0xf),
            ("high", 20, 0xf),
            ("fields", 4, 0xff),
            ("lane", 12, 0xfff),
        ] {
            assert_eq!(output(&model, &sim, name), (bits >> shift) & mask);
        }
    }
    for (signal, offset) in (&model.outputs).into_iter().zip([0, 8, 20, 4]) {
        for bit in &signal.bits {
            assert_eq!(
                bit.value,
                model.inputs[0].bits[bit.index as usize + offset].value
            );
        }
    }
}

#[test]
fn malformed_graphs_return_positioned_errors() {
    let cases = [
        (
            "hw.module @m(in %clk: i1) { %a = comb.xor %b, %b : i1 %b = comb.xor %a, %a : i1 hw.output }",
            "cycle",
        ),
        (
            "hw.module @m(in %clk: i1) { %a = comb.xor %missing, %missing : i1 hw.output }",
            "unresolved",
        ),
        (
            "hw.module @m(in %clk: i1) { %a = hw.constant true %a = hw.constant false hw.output }",
            "duplicate SSA",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i4) { %b = comb.add %a, %clk : i4 hw.output }",
            "type mismatch",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i4) { %b = comb.extract %a from 3 : (i4) -> i2 hw.output }",
            "range mismatch",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i4) { %b = comb.replicate %a : (i4) -> i6 hw.output }",
            "positive multiple",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i4) { %b = comb.replicate %a : (i4) -> i2 hw.output }",
            "positive multiple",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i4) { %b = comb.replicate %a : (i2) -> i8 hw.output }",
            "type mismatch",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: !hw.array<2xi4>) { %b = comb.replicate %a : (!hw.array<2xi4>) -> i16 hw.output }",
            "type mismatch",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i4) { %b = comb.replicate %a : (i4) -> !hw.array<2xi4> hw.output }",
            "type mismatch",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i4) { %b = hw.bitcast %a : (i4) -> i8 hw.output }",
            "width mismatch",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: !hw.array<2xi4>) { %b = hw.bitcast %a : (!hw.array<2xi4>) -> !hw.struct<hi: i4, lo: i2> hw.output }",
            "width mismatch",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i8) { %b = hw.bitcast %a : (!hw.array<2xi4>) -> i8 hw.output }",
            "operand type mismatch",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i4) { %b = hw.bitcast %a : (i4) -> !seq.clock hw.output }",
            "clock used as data",
        ),
        (
            "hw.module @m(in %clk: !seq.clock) { %b = hw.bitcast %clk : (!seq.clock) -> i1 hw.output }",
            "clock used as data",
        ),
        (
            "hw.module @m(in %clk: i1) { %b = hw.bitcast %clk : (i1) -> i1 hw.output }",
            "clock used as ordinary data",
        ),
        (
            "hw.module @m(in %clk: i1, in %u: !hw.union<raw: i8>) { %r = hw.union_extract %u[\"missing\"] : !hw.union<raw: i8> hw.output }",
            "unknown union member",
        ),
        (
            "hw.module @m(in %clk: i1, in %u: !hw.union<raw: i8>) { %r = hw.union_extract %u[\"other\"] : !hw.union<other: i8> hw.output }",
            "type mismatch",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i8) { %r = hw.union_extract %a[\"raw\"] : i8 hw.output }",
            "requires a union type",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i8) { %r = hw.bitcast %a : (i8) -> !hw.union<> hw.output }",
            "empty aggregates",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i8) { %r = hw.bitcast %a : (i8) -> !hw.union<raw: i8, raw: i4> hw.output }",
            "duplicate union member",
        ),
        (
            "hw.module @m(in %clk: i1, in %a: i8) { %r = hw.bitcast %a : (i8) -> !hw.union<raw: i8 offset 18446744073709551615> hw.output }",
            "overflow",
        ),
        (
            "hw.module @m(in %clk: i1, out o: i4) { hw.output %clk : i1 }",
            "output type",
        ),
        (
            "hw.module @m(in %clk: i1) { hw.output hw.output }",
            "terminator",
        ),
        (
            "hw.module @m(in %clk: i1) { %a = hw.instance \"missing\" @not_here() -> (x: i1) hw.output }",
            "unknown module",
        ),
    ];
    for (text, expected) in cases {
        let error = NamedFsm::from_source(9, text, &options(None)).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
        assert_eq!(error.position.unwrap().file_id, 9);
    }
    let text =
        "hw.module @m(in %clk: i1) { %a = comb.xor %bad, %bad : i1 loc(\"rtl.sv\":4:2) hw.output }";
    let error = NamedFsm::from_source(9, text, &options(None)).unwrap_err();
    assert!(error.location.unwrap().contains("rtl.sv"));
    assert!(error.instance.contains("m"));
}

#[test]
fn module_selection_namespaces_externals_and_recursion() {
    let text = "module @left { hw.module @m(in %clk: i1) { hw.output } } module @right { hw.module @m(in %clk: i1) { hw.output } }";
    assert!(
        NamedFsm::from_source(0, text, &options(None))
            .unwrap_err()
            .to_string()
            .contains("available")
    );
    let mut opts = options(None);
    opts.top = Some("left::m".to_owned());
    assert_eq!(
        NamedFsm::from_source(0, text, &opts).unwrap().top,
        "left::m"
    );
    opts.top = Some("m".to_owned());
    assert!(NamedFsm::from_source(0, text, &opts).is_err());
    let external = "module { hw.module.extern @ext(in %d: i1, out o: i1) hw.module @m(in %clk: i1, in %d: i1, out o: i1) { %o = hw.instance \"x\" @ext(d: %d: i1) -> (o: i1) hw.output %o : i1 } }";
    assert!(
        NamedFsm::from_source(0, external, &options(None))
            .unwrap_err()
            .to_string()
            .contains("external")
    );
    let recursive =
        "hw.module @m(in %clk: i1) { hw.instance \"x\" @m(clk: %clk: i1) -> () hw.output }";
    opts.top = Some("m".to_owned());
    assert!(
        NamedFsm::from_source(0, recursive, &opts)
            .unwrap_err()
            .to_string()
            .contains("recursive")
    );
    let wrong_port = external.replace(
        "hw.module.extern @ext(in %d: i1, out o: i1)",
        "hw.module private @ext(in %d: i4, out o: i4) { hw.output %d : i4 }",
    );
    assert!(
        NamedFsm::from_source(0, &wrong_port, &options(None))
            .unwrap_err()
            .to_string()
            .contains("type mismatch")
    );
    let undefined_type = "hw.module @m(in %clk: i1, in %x: !unknown) { hw.output }";
    assert!(NamedFsm::from_source(0, undefined_type, &options(None)).is_err());
}

#[test]
fn mutated_ast_is_validated_without_panicking() {
    let mut ast = parse(
        0,
        "hw.module @m(in %clk: i1) { %a = hw.constant true hw.output }",
    )
    .unwrap();
    let parser_circt::ast::Item::HwModule(module) = &mut ast.items[0] else {
        unreachable!()
    };
    module.body.as_mut().unwrap()[0].results.clear();
    assert!(NamedFsm::from_ast(&ast, &options(None)).is_err());
}

#[test]
fn export_initialization_symbols_and_behavior_round_trip() {
    let model = fixture("counter_ones", true);
    for format in [AigerFormat::Ascii, AigerFormat::Binary] {
        for zero in [false, true] {
            let opts = ExportOptions {
                zero_init: zero,
                ..Default::default()
            };
            let bytes = export::write(&model.fsm, format, &opts).unwrap();
            let (fsm, _) = match format {
                AigerFormat::Ascii => read_aiger_ascii(&bytes),
                AigerFormat::Binary => read_aiger_binary(&bytes),
            };
            let mut sim = Simulator::from(fsm.clone());
            for input in fsm.get_inputs() {
                sim.set_input(input.index(), Some(false));
            }
            sim.eval();
            let count = (0..4).fold(0, |count, bit| {
                count | (u64::from(sim.get_output(bit).unwrap()) << bit)
            });
            assert_eq!(count, if zero { 0 } else { 15 });
            for input in fsm.get_inputs() {
                sim.set_input(input.index(), Some(true));
            }
            tick(&mut sim);
            let count = (0..4).fold(0, |count, bit| {
                count | (u64::from(sim.get_output(bit).unwrap()) << bit)
            });
            assert_eq!(count, if zero { 1 } else { 0 });
            let stripped = export::prepare(
                &model.fsm,
                &ExportOptions {
                    strip_symbols: true,
                    ..opts
                },
            )
            .unwrap();
            assert!(export::symbols(&stripped).is_empty());
        }
    }
}

#[test]
fn singleton_arrays_guard_the_unused_index_bit() {
    let model = source(
        r#"hw.module @m(in %clk: i1, in %array: !hw.array<1xi4>, in %idx: i1, in %v: i4, out item: i4, out updated: !hw.array<1xi4>) {
        %item = hw.array_get %array[%idx] : !hw.array<1xi4>, i1
        %updated = hw.array_inject %array[%idx], %v : !hw.array<1xi4>, i1
        hw.output %item, %updated : i4, !hw.array<1xi4>
    }"#,
    );
    let mut sim = simulator(&model);
    set(&model, &mut sim, "array", 3);
    set(&model, &mut sim, "v", 7);
    set(&model, &mut sim, "$undefined::item", 9);
    set(&model, &mut sim, "$undefined::updated", 10);
    sim.eval();
    assert_eq!(output(&model, &sim, "item"), 3);
    assert_eq!(output(&model, &sim, "updated"), 7);
    set(&model, &mut sim, "idx", 1);
    sim.eval();
    assert_eq!(output(&model, &sim, "item"), 9);
    assert_eq!(output(&model, &sim, "updated"), 10);
}

#[test]
fn escaped_mlir_names_and_labels_export_as_single_line_symbols() {
    let model = source(
        r#"hw.module @m(in %clk: i1, in %a: i1, out "line\0Abreak": i1) {
        verif.clocked_assert %a, posedge %clk label "check\0Abreak" : i1
        hw.output %a : i1
    }"#,
    );
    assert_eq!(model.inputs[0].name, "a");
    assert_eq!(model.outputs[0].name, "line\nbreak");
    for format in [AigerFormat::Ascii, AigerFormat::Binary] {
        let contents = export::write(&model.fsm, format, &ExportOptions::default()).unwrap();
        let (fsm, _) = match format {
            AigerFormat::Ascii => read_aiger_ascii(&contents),
            AigerFormat::Binary => read_aiger_binary(&contents),
        };
        assert_eq!(fsm.get_output_label(0).as_deref(), Some("line\\nbreak"));
        assert_eq!(fsm.get_assert_label(0).as_deref(), Some("m::check\\nbreak"));
    }
}

#[test]
fn duplicate_output_ports_and_invalid_reset_inputs_are_rejected() {
    let text = "hw.module @m(in %clk: i1, out o: i1, out o: i1) { %true = hw.constant true hw.output %true, %true : i1, i1 }";
    let error = NamedFsm::from_source(3, text, &options(None)).unwrap_err();
    assert!(error.to_string().contains("duplicate port"));
    assert_eq!(error.position.unwrap().file_id, 3);
    let text = "hw.module @m(in %clk: i1, in %rst: i4) { hw.output }";
    assert!(
        NamedFsm::from_source(0, text, &options(Some("rst")))
            .unwrap_err()
            .to_string()
            .contains("reset input must be i1")
    );
}
