use std::{collections::VecDeque, io::Write};

use crate::{
    formats::aiger::{
        AigerType, AigerVersion, from_value, into_value, read_binary_line, read_header,
        read_latch_line, read_symbols, read_value_line, write_header, write_latch, write_symbols,
    },
    fsm::{FSM, verify::VerifyOrdering},
    gate::GateType,
    variable::Var,
};

fn read_compressed_uint(contents: &mut VecDeque<u8>) -> usize {
    let mut value = 0;
    let mut offset = 0;
    loop {
        let c = contents.pop_front().unwrap();
        value |= ((c & 0x7F) as usize) << (offset * 7);
        offset += 1;
        if c & 0x80 == 0 {
            break;
        }
    }
    value
}

fn write_compressed_uint(contents: &mut Vec<u8>, mut value: usize) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        contents.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Parses an binary aiger file (.aig) into an FSM struct
pub fn read_aiger_binary(contents: &[u8]) -> (FSM, AigerVersion) {
    let mut contents = VecDeque::from(contents.to_vec());

    // Parse header
    let header = read_header(&read_binary_line(&mut contents), AigerType::Binary);
    assert_eq!(
        header.num_variables,
        header.num_inputs + header.num_latches + header.num_ands
    );

    // Create empty FSM
    let mut fsm = FSM::new(header.num_variables);

    // Assign input variable drivers
    for input_index in 0..header.num_inputs {
        fsm.add_input(Var::from((true, input_index)));
    }

    // Parse latches
    for latch_index in 0..header.num_latches {
        let output_variable = Var::from((true, header.num_inputs + latch_index));
        let (input_value, output_variable, default_value) =
            read_latch_line(&mut contents, Some(output_variable));
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

    // Parse ands
    for and_index in 0..header.num_ands {
        let output_index = header.num_inputs + header.num_latches + and_index;
        let output_variable = Var::from((true, output_index));
        let input_index0 = (output_index + 1) * 2 - read_compressed_uint(&mut contents);
        let input_index1 = input_index0 - read_compressed_uint(&mut contents);
        let input_value0 = into_value(input_index0);
        let input_value1 = into_value(input_index1);
        fsm.add_gate(GateType::And, output_variable, input_value0, input_value1);
    }

    read_symbols(&mut contents, &mut fsm);

    fsm.normalize_aiger_assertions(header.version);
    fsm.verify(VerifyOrdering::Verify);
    (fsm, header.version)
}

/// Writes an FSM struct into a binary aiger file (.aig)
pub fn write_aiger_binary(fsm: &FSM, version: AigerVersion) -> Vec<u8> {
    let mut contents = Vec::new();
    // Compute number of inputs (this is expensive so do it once)
    let num_inputs = fsm.get_inputs().len();
    let num_latches = fsm.get_latches().len();
    // Write header
    write_header(&mut contents, fsm, AigerType::Binary, version).unwrap();
    // Check inputs
    for (input_index, input) in fsm.get_inputs().into_iter().enumerate() {
        assert_eq!(input, Var::from((true, input_index)));
    }
    // Check latch output variables, then write latches
    for (latch_index, latch) in fsm.get_latches().into_iter().enumerate() {
        assert_eq!(latch.output, Var::from((true, num_inputs + latch_index)));
        write_latch(&mut contents, latch, AigerType::Binary, version).unwrap();
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
    for (gate_index, gate) in fsm.get_gates().into_iter().enumerate() {
        // AND gates must be positive, OR gates must be negative
        assert_eq!(gate.gate_type == GateType::And, gate.output.sign());
        // Check that gate output variable is correct
        let output_index = num_inputs + num_latches + gate_index;
        assert_eq!(gate.output.index(), Var::from((true, output_index)).index());
        assert!(gate.a <= gate.b);
        // Write gate using compressed uints (swap a and b since a <= b)
        let input_value0 = from_value(gate.b.xor(!gate.output.sign()));
        let input_value1 = from_value(gate.a.xor(!gate.output.sign()));
        write_compressed_uint(&mut contents, (output_index + 1) * 2 - input_value0);
        write_compressed_uint(&mut contents, input_value0 - input_value1);
    }

    write_symbols(&mut contents, fsm).unwrap();

    contents
}
