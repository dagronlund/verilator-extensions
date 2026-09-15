use formal_utils::{
    formats::aiger::{
        AigerVersion,
        ascii::{read_aiger_ascii, write_aiger_ascii},
        binary::{read_aiger_binary, write_aiger_binary},
    },
    fsm::{FSM, VariableDriver, verify::VerifyOrdering},
    gate::GateType,
    sim::Simulator,
    value::Value,
    variable::Var,
};

#[test]
fn ascii_symbols_round_trip() {
    let contents = b"aag 2 1 1 1 0 1 1\n2\n4 2\n4\n0\n2\ni0 request input\nl0 saved state\no0 result output\nb0 safety property\nc0 environment constraint\n";
    let (fsm, version) = read_aiger_ascii(contents);

    assert_eq!(fsm.get_variable_label(0).as_deref(), Some("request input"));
    assert_eq!(fsm.get_variable_label(1).as_deref(), Some("saved state"));
    assert_eq!(fsm.get_output_label(0).as_deref(), Some("result output"));
    assert_eq!(fsm.get_assert_label(0).as_deref(), Some("safety property"));
    assert_eq!(
        fsm.get_assume_label(0).as_deref(),
        Some("environment constraint")
    );
    assert_eq!(write_aiger_ascii(&fsm, version), contents);
}

#[test]
fn binary_symbols_round_trip() {
    let contents = b"aig 2 1 1 1 0 1 1\n2\n4\n0\n2\ni0 request input\nl0 saved state\no0 result output\nb0 safety property\nc0 environment constraint\n";
    let (fsm, version) = read_aiger_binary(contents);

    assert_eq!(fsm.get_variable_label(0).as_deref(), Some("request input"));
    assert_eq!(fsm.get_variable_label(1).as_deref(), Some("saved state"));
    assert_eq!(fsm.get_output_label(0).as_deref(), Some("result output"));
    assert_eq!(fsm.get_assert_label(0).as_deref(), Some("safety property"));
    assert_eq!(
        fsm.get_assume_label(0).as_deref(),
        Some("environment constraint")
    );
    assert_eq!(write_aiger_binary(&fsm, version), contents);
}

#[test]
fn normalize_arbitrary_fsm_for_binary_write() {
    // Deliberately interleave every driver type, leave an unused variable, and
    // put the dependent gate before its dependency.
    let mut fsm = FSM::new(7);
    fsm.add_gate(
        GateType::Or,
        Var::from((true, 0)),
        Value::Var(Var::from((true, 4))),
        Value::Var(Var::from((true, 1))),
    );
    fsm.add_latch(
        Value::Var(Var::from((true, 0))),
        Var::from((true, 1)),
        Some(false),
    );
    fsm.add_input(Var::from((true, 3)));
    fsm.add_gate(
        GateType::And,
        Var::from((false, 4)),
        Value::Var(Var::from((true, 3))),
        Value::Var(Var::from((false, 6))),
    );
    fsm.add_latch(
        Value::Var(Var::from((true, 6))),
        Var::from((true, 5)),
        Some(true),
    );
    fsm.add_input(Var::from((true, 6)));
    fsm.add_output(Value::Var(Var::from((true, 0))));
    fsm.add_output(Value::Var(Var::from((true, 5))));
    fsm.add_assert(Value::Var(Var::from((false, 5))));
    fsm.add_assume(Value::Var(Var::from((true, 3))));
    *fsm.get_variable_label_mut(1) = Some("first latch".to_string());
    *fsm.get_variable_label_mut(3) = Some("first input".to_string());
    *fsm.get_variable_label_mut(5) = Some("second latch".to_string());
    *fsm.get_variable_label_mut(6) = Some("second input".to_string());
    *fsm.get_output_label_mut(0) = Some("first output".to_string());
    *fsm.get_assert_label_mut(0) = Some("safety".to_string());
    *fsm.get_assume_label_mut(0) = Some("environment".to_string());

    fsm.normalize_ordering();
    assert_eq!(fsm.get_num_variables(), 6);
    assert_eq!(fsm.get_inputs().len(), 2);
    assert_eq!(fsm.get_latches().len(), 2);
    assert_eq!(fsm.get_gates().len(), 2);
    assert!(
        fsm.get_variables()[0..2]
            .into_iter()
            .all(|(driver, _)| *driver == VariableDriver::Input)
    );
    assert!(
        fsm.get_variables()[2..4]
            .into_iter()
            .all(|(driver, _)| driver.get_latch().is_some())
    );
    assert!(
        fsm.get_variables()[4..6]
            .into_iter()
            .all(|(driver, _)| driver.get_gate().is_some())
    );
    assert_eq!(fsm.get_variable_label(0).as_deref(), Some("first input"));
    assert_eq!(fsm.get_variable_label(1).as_deref(), Some("second input"));
    assert_eq!(fsm.get_variable_label(2).as_deref(), Some("first latch"));
    assert_eq!(fsm.get_variable_label(3).as_deref(), Some("second latch"));
    fsm.verify(VerifyOrdering::Verify);

    fsm.normalize_outputs();
    let expected_fsm = fsm.clone();
    let contents = write_aiger_binary(&fsm, AigerVersion::V1_9);
    let actual_fsm = read_aiger_binary(&contents).0;
    assert_eq!(
        actual_fsm.get_variable_label(0).as_deref(),
        Some("first input")
    );
    assert_eq!(
        actual_fsm.get_variable_label(2).as_deref(),
        Some("first latch")
    );
    assert_eq!(
        actual_fsm.get_output_label(0).as_deref(),
        Some("first output")
    );
    assert_eq!(actual_fsm.get_assert_label(0).as_deref(), Some("safety"));
    assert_eq!(
        actual_fsm.get_assume_label(0).as_deref(),
        Some("environment")
    );

    let mut expected = Simulator::from(expected_fsm);
    let mut actual = Simulator::from(actual_fsm);
    for (input0, input1) in [(false, false), (true, false), (true, true), (false, true)] {
        expected.set_input(0, Some(input0));
        expected.set_input(1, Some(input1));
        actual.set_input(0, Some(input0));
        actual.set_input(1, Some(input1));
        expected.eval();
        actual.eval();
        assert_eq!(actual.get_output(0), expected.get_output(0));
        assert_eq!(actual.get_output(1), expected.get_output(1));
        expected.step();
        actual.step();
    }
}
