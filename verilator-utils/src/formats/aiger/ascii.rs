use std::{collections::VecDeque, io::Write};

use crate::{
    formats::aiger::{
        AigerType, AigerVersion, from_value, from_var, into_value, into_var, read_binary_line,
        read_header, read_latch_line, read_symbols, read_value_line, split_line, write_header,
        write_latch, write_symbols,
    },
    fsm::{FSM, verify::VerifyOrdering},
    gate::GateType,
};

/// Parses an ASCII aiger file (.aag) into an FSM struct
pub fn read_aiger_ascii(contents: &[u8]) -> (FSM, AigerVersion) {
    let mut contents = VecDeque::from(contents.to_vec());

    // Parse header
    let header = read_header(&read_binary_line(&mut contents), AigerType::Ascii);

    // Create empty FSM
    let mut fsm = FSM::new(header.num_variables);

    // Parse inputs
    for _ in 0..header.num_inputs {
        let line = read_binary_line(&mut contents);
        let split = split_line(&line);
        assert_eq!(split.len(), 1);
        fsm.add_input(into_var(split[0].parse::<usize>().unwrap()));
    }

    // Parse latches
    for _ in 0..header.num_latches {
        let (input_value, output_variable, default_value) = read_latch_line(&mut contents, None);
        fsm.add_latch(input_value, output_variable, default_value);
    }

    // Parse outputs
    for _ in 0..header.num_outputs {
        fsm.add_output(read_value_line(&mut contents));
    }

    // Parse bad states/assertions
    for _ in 0..header.num_assert {
        // Invert because AIGER says property is broken if the value can
        // ever be set high, rather than SV assert(); syntax
        fsm.add_assert(!read_value_line(&mut contents));
    }

    // Parse invariants/assumptions
    for _ in 0..header.num_assume {
        fsm.add_assume(read_value_line(&mut contents));
    }

    assert_eq!(header.num_justice, 0);
    assert_eq!(header.num_fairness, 0);

    // Parse and gates (unordered, so keep track of which variables are
    // driven by a gate to help with reordering)
    for _ in 0..header.num_ands {
        let line = read_binary_line(&mut contents);
        let split = split_line(&line);
        assert_eq!(split.len(), 3);
        let output_variable = into_var(split[0].parse::<usize>().unwrap());
        let a = into_value(split[1].parse::<usize>().unwrap());
        let b = into_value(split[2].parse::<usize>().unwrap());
        fsm.add_gate(GateType::And, output_variable, a, b);
    }

    read_symbols(&mut contents, &mut fsm);

    fsm.normalize_aiger_assertions(header.version);
    fsm.verify(VerifyOrdering::Verify);
    (fsm, header.version)
}

pub fn write_aiger_ascii(fsm: &FSM, version: AigerVersion) -> Vec<u8> {
    let mut contents = Vec::new();
    // Write header
    write_header(&mut contents, fsm, AigerType::Ascii, version).unwrap();
    // Write inputs
    for input in fsm.get_inputs() {
        writeln!(&mut contents, "{}", from_var(input)).unwrap();
    }
    // Write latches
    for latch in fsm.get_latches() {
        write_latch(&mut contents, latch, AigerType::Ascii, version).unwrap();
    }
    // Write outputs
    for &(output, _) in fsm.get_outputs() {
        writeln!(&mut contents, "{}", from_value(output)).unwrap();
    }
    // Write asserts
    for &(assert, _) in fsm.get_asserts() {
        writeln!(&mut contents, "{}", from_value(!assert)).unwrap();
    }
    // Write assumes
    for &(assume, _) in fsm.get_assumes() {
        writeln!(&mut contents, "{}", from_value(assume)).unwrap();
    }
    // Write gates
    for gate in fsm.get_gates() {
        assert_eq!(gate.gate_type == GateType::And, gate.output.sign());
        writeln!(
            &mut contents,
            "{} {} {}",
            from_var(gate.output.xor(!gate.output.sign())),
            from_value(gate.a.xor(!gate.output.sign())),
            from_value(gate.b.xor(!gate.output.sign()))
        )
        .unwrap();
    }
    write_symbols(&mut contents, fsm).unwrap();
    contents
}
