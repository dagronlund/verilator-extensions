use formal_utils::{
    fsm::FSM,
    ops::{FsmOps, ShiftOperation},
    sim::Simulator,
    value::Value,
};

fn inputs(fsm: &mut FSM, width: usize) -> Vec<Value> {
    (0..width)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect()
}
fn set(sim: &mut Simulator, offset: usize, width: usize, value: usize) {
    for bit in 0..width {
        sim.set_input(offset + bit, Some(value & (1 << bit) != 0));
    }
}
fn word(sim: &Simulator, offset: usize, width: usize) -> usize {
    (0..width).fold(0, |word, bit| {
        word | (usize::from(sim.get_output(offset + bit).unwrap()) << bit)
    })
}

#[test]
fn exhaustive_small_width_division_and_remainder() {
    for width in 1..=5 {
        let mask = (1usize << width) - 1;
        let signed = |value: usize| {
            if value & (1 << (width - 1)) == 0 {
                value as isize
            } else {
                value as isize - (1 << width)
            }
        };
        let mut fsm = FSM::default();
        let lhs = inputs(&mut fsm, width);
        let rhs = inputs(&mut fsm, width);
        let groups = [
            fsm.create_unsigned_division(&lhs, &rhs),
            fsm.create_signed_division(&lhs, &rhs),
            fsm.create_unsigned_remainder(&lhs, &rhs),
            fsm.create_signed_remainder(&lhs, &rhs),
        ];
        for group in groups {
            for bit in group {
                fsm.add_output(bit);
            }
        }
        let mut sim = Simulator::from(fsm);
        for a in 0..=mask {
            for b in 1..=mask {
                set(&mut sim, 0, width, a);
                set(&mut sim, width, width, b);
                sim.eval();
                assert_eq!(word(&sim, 0, width), a / b);
                assert_eq!(
                    word(&sim, width, width),
                    (signed(a) / signed(b)) as usize & mask
                );
                assert_eq!(word(&sim, 2 * width, width), a % b);
                assert_eq!(
                    word(&sim, 3 * width, width),
                    (signed(a) % signed(b)) as usize & mask
                );
            }
        }
    }
}

#[test]
fn full_width_shifts_saturate_and_rotates_wrap() {
    for width in [1, 3, 5, 8] {
        let amount_width = 7;
        let mask = (1usize << width) - 1;
        let mut fsm = FSM::default();
        let value = inputs(&mut fsm, width);
        let amount = inputs(&mut fsm, amount_width);
        let operations = [
            ShiftOperation::ShiftLeft,
            ShiftOperation::ShiftRight,
            ShiftOperation::ShiftRightArithmetic,
            ShiftOperation::RotateLeft,
            ShiftOperation::RotateRight,
        ];
        for operation in operations {
            for bit in fsm.create_shifter(&value, &amount, operation) {
                fsm.add_output(bit);
            }
        }
        let mut sim = Simulator::from(fsm);
        for value in 0..=mask {
            for amount in 0..128 {
                set(&mut sim, 0, width, value);
                set(&mut sim, width, amount_width, amount);
                sim.eval();
                let signed = if value & (1 << (width - 1)) == 0 {
                    value as isize
                } else {
                    value as isize - (1 << width)
                };
                let rotate = amount % width;
                let expected = [
                    if amount >= width {
                        0
                    } else {
                        (value << amount) & mask
                    },
                    if amount >= width { 0 } else { value >> amount },
                    if amount >= width {
                        if signed < 0 { mask } else { 0 }
                    } else {
                        (signed >> amount) as usize & mask
                    },
                    ((value << rotate) | (value >> (width - rotate))) & mask,
                    ((value >> rotate) | (value << (width - rotate))) & mask,
                ];
                for (index, expected) in expected.into_iter().enumerate() {
                    assert_eq!(
                        word(&sim, index * width, width),
                        expected,
                        "width={width}, value={value}, amount={amount}, operation={:?}",
                        operations[index]
                    );
                }
            }
        }
    }
}

#[test]
fn shifter_accepts_amounts_wider_than_host_index() {
    let mut fsm = FSM::default();
    let value = vec![Value::Constant(true); 3];
    let mut amount = vec![Value::Constant(false); usize::BITS as usize + 1];
    amount[usize::BITS as usize] = Value::Constant(true);
    for operation in [
        ShiftOperation::ShiftLeft,
        ShiftOperation::ShiftRight,
        ShiftOperation::ShiftRightArithmetic,
    ] {
        let result = fsm.create_shifter(&value, &amount, operation);
        assert_eq!(
            result,
            vec![Value::Constant(operation == ShiftOperation::ShiftRightArithmetic); 3]
        );
    }
}

fn set_simulator_word(simulator: &mut Simulator, offset: usize, width: usize, value: usize) {
    for bit in 0..width {
        simulator.set_input(offset + bit, Some(value & (1 << bit) != 0));
    }
}

fn simulator_output_word(simulator: &Simulator, offset: usize, width: usize) -> usize {
    (0..width).fold(0, |value, bit| {
        value | (usize::from(simulator.get_output(offset + bit) == Some(true)) << bit)
    })
}

#[test]
fn local_select_uses_binary_index() {
    const SELECT_WIDTH: usize = 3;
    const INPUT_COUNT: usize = 1 << SELECT_WIDTH;

    let mut fsm = FSM::default();
    let inputs = (0..INPUT_COUNT)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    let selector = (0..SELECT_WIDTH)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    let output = FsmOps::create_select(&mut fsm, &inputs, &selector);
    fsm.add_output(output);

    let mut simulator = Simulator::from(fsm);
    let input_values = 0b1010_0110usize;
    for index in 0..INPUT_COUNT {
        simulator.set_input(index, Some(input_values & (1 << index) != 0));
    }
    for selected in 0..INPUT_COUNT {
        for bit in 0..SELECT_WIDTH {
            simulator.set_input(INPUT_COUNT + bit, Some(selected & (1 << bit) != 0));
        }
        simulator.eval();
        assert_eq!(
            simulator.get_output(0),
            Some(input_values & (1 << selected) != 0)
        );
    }
}

#[test]
fn local_shifter_supports_rotates() {
    const WIDTH: usize = 4;
    const SHIFT_WIDTH: usize = 2;
    const MASK: usize = (1 << WIDTH) - 1;

    let mut fsm = FSM::default();
    let input = (0..WIDTH)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    let amount = (0..SHIFT_WIDTH)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    for operation in [ShiftOperation::RotateLeft, ShiftOperation::RotateRight] {
        for output in FsmOps::create_shifter(&mut fsm, &input, &amount, operation) {
            fsm.add_output(output);
        }
    }

    let mut simulator = Simulator::from(fsm);
    for input in 0usize..=MASK {
        for amount in 0usize..(1 << SHIFT_WIDTH) {
            for bit in 0..WIDTH {
                simulator.set_input(bit, Some(input & (1 << bit) != 0));
            }
            for bit in 0..SHIFT_WIDTH {
                simulator.set_input(WIDTH + bit, Some(amount & (1 << bit) != 0));
            }
            simulator.eval();

            let expected = [
                ((input << amount) | (input >> (WIDTH - amount))) & MASK,
                ((input >> amount) | (input << (WIDTH - amount))) & MASK,
            ];
            for (operation, expected) in expected.into_iter().enumerate() {
                for bit in 0..WIDTH {
                    assert_eq!(
                        simulator.get_output(operation * WIDTH + bit),
                        Some(expected & (1 << bit) != 0)
                    );
                }
            }
        }
    }
}

#[test]
fn local_multiplication_supports_unsigned_and_signed_values() {
    const WIDTH: usize = 4;
    const MASK: usize = (1 << WIDTH) - 1;

    let mut fsm = FSM::default();
    let lhs = (0..WIDTH)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    let rhs = (0..WIDTH)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    for output in FsmOps::create_multiplication(&mut fsm, &lhs, &rhs) {
        fsm.add_output(output);
    }

    let mut simulator = Simulator::from(fsm);
    for lhs in 0..=MASK {
        for rhs in 0..=MASK {
            set_simulator_word(&mut simulator, 0, WIDTH, lhs);
            set_simulator_word(&mut simulator, WIDTH, WIDTH, rhs);
            simulator.eval();

            let signed_lhs = if lhs & (1 << (WIDTH - 1)) == 0 {
                lhs as isize
            } else {
                lhs as isize - (1 << WIDTH)
            };
            let signed_rhs = if rhs & (1 << (WIDTH - 1)) == 0 {
                rhs as isize
            } else {
                rhs as isize - (1 << WIDTH)
            };
            let product = simulator_output_word(&simulator, 0, WIDTH);
            assert_eq!(product, (lhs * rhs) & MASK);
            assert_eq!(product, (signed_lhs * signed_rhs) as usize & MASK);
        }
    }
}

#[test]
fn local_unsigned_division_produces_integer_quotients() {
    const WIDTH: usize = 4;
    const MASK: usize = (1 << WIDTH) - 1;

    let mut fsm = FSM::default();
    let dividend = (0..WIDTH)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    let divisor = (0..WIDTH)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    for output in FsmOps::create_unsigned_division(&mut fsm, &dividend, &divisor) {
        fsm.add_output(output);
    }

    let mut simulator = Simulator::from(fsm);
    for dividend in 0..=MASK {
        for divisor in 1..=MASK {
            set_simulator_word(&mut simulator, 0, WIDTH, dividend);
            set_simulator_word(&mut simulator, WIDTH, WIDTH, divisor);
            simulator.eval();

            assert_eq!(
                simulator_output_word(&simulator, 0, WIDTH),
                dividend / divisor
            );
        }
    }
}

#[test]
fn local_signed_division_truncates_toward_zero() {
    const WIDTH: usize = 4;
    const MASK: usize = (1 << WIDTH) - 1;

    let mut fsm = FSM::default();
    let dividend = (0..WIDTH)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    let divisor = (0..WIDTH)
        .map(|_| Value::from(fsm.add_variable_input()))
        .collect::<Vec<_>>();
    for output in FsmOps::create_signed_division(&mut fsm, &dividend, &divisor) {
        fsm.add_output(output);
    }

    let mut simulator = Simulator::from(fsm);
    for dividend in 0..=MASK {
        for divisor in 1..=MASK {
            set_simulator_word(&mut simulator, 0, WIDTH, dividend);
            set_simulator_word(&mut simulator, WIDTH, WIDTH, divisor);
            simulator.eval();

            let signed_dividend = if dividend & (1 << (WIDTH - 1)) == 0 {
                dividend as isize
            } else {
                dividend as isize - (1 << WIDTH)
            };
            let signed_divisor = if divisor & (1 << (WIDTH - 1)) == 0 {
                divisor as isize
            } else {
                divisor as isize - (1 << WIDTH)
            };
            assert_eq!(
                simulator_output_word(&simulator, 0, WIDTH),
                (signed_dividend / signed_divisor) as usize & MASK
            );
        }
    }
}
