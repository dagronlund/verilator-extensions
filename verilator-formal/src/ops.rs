use verilator_utils::{fsm::FSM, gate::GateType, value::Value};

/// The operation performed by [`FsmOps::create_comparison`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    Equals,
    NotEquals,
    LessThan,
    GreaterThan,
    LessThanOrEqual,
    GreaterThanOrEqual,
}

/// The operation performed by [`FsmOps::create_shifter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftOperation {
    ShiftLeft,
    ShiftRight,
    ShiftRightArithmetic,
    RotateLeft,
    RotateRight,
}

/// Local word-level operation builders for a verilator-utils FSM.
///
/// This is an extension trait because Rust does not permit adding inherent
/// methods to [`FSM`] outside the crate that defines it.
pub trait FsmOps {
    fn create_xor_gate(&mut self, a: Value, b: Value) -> Value;
    fn create_full_adder(&mut self, a: Value, b: Value, carry_in: Value) -> (Value, Value);
    fn create_addition(&mut self, a: &[Value], b: &[Value]) -> Vec<Value>;
    fn create_subtraction(&mut self, a: &[Value], b: &[Value]) -> Vec<Value>;
    fn create_multiplication(&mut self, a: &[Value], b: &[Value]) -> Vec<Value>;
    fn create_unsigned_division(&mut self, dividend: &[Value], divisor: &[Value]) -> Vec<Value>;
    fn create_signed_division(&mut self, dividend: &[Value], divisor: &[Value]) -> Vec<Value>;
    fn create_comparison(&mut self, a: &[Value], b: &[Value], comparison: Comparison) -> Value;
    fn create_mux_gate(&mut self, a: Value, b: Value, select: Value) -> Value;
    fn create_mux(&mut self, a: &[Value], b: &[Value], select: Value) -> Vec<Value>;
    fn create_select(&mut self, value: &[Value], index: &[Value]) -> Value;
    fn create_shifter(
        &mut self,
        value: &[Value],
        shift: &[Value],
        operation: ShiftOperation,
    ) -> Vec<Value>;
}

impl FsmOps for FSM {
    fn create_xor_gate(&mut self, a: Value, b: Value) -> Value {
        if let Value::Constant(a) = a {
            return b.xor(a);
        }
        if let Value::Constant(b) = b {
            return a.xor(b);
        }
        if a == b {
            return Value::Constant(false);
        }
        if a == !b {
            return Value::Constant(true);
        }
        let a_and_b = Value::from(self.add_variable_gate_binary(GateType::And, a, b));
        let a_or_b = Value::from(self.add_variable_gate_binary(GateType::Or, a, b));
        Value::from(self.add_variable_gate_binary(GateType::And, a_or_b, !a_and_b))
    }

    fn create_full_adder(&mut self, a: Value, b: Value, carry_in: Value) -> (Value, Value) {
        let a_xor_b = FsmOps::create_xor_gate(self, a, b);
        let sum = FsmOps::create_xor_gate(self, a_xor_b, carry_in);

        let a_and_b = Value::from(self.add_variable_gate_binary(GateType::And, a, b));
        let a_and_carry = Value::from(self.add_variable_gate_binary(GateType::And, a, carry_in));
        let b_and_carry = Value::from(self.add_variable_gate_binary(GateType::And, b, carry_in));
        let carry_out =
            self.add_variable_gate(GateType::Or, vec![a_and_b, a_and_carry, b_and_carry]);

        (sum, carry_out)
    }

    fn create_addition(&mut self, a: &[Value], b: &[Value]) -> Vec<Value> {
        assert_eq!(a.len(), b.len(), "Inputs must have equal widths!");

        let mut carry = Value::Constant(false);
        let mut sum = Vec::with_capacity(a.len());
        for (&a_bit, &b_bit) in a.into_iter().zip(b) {
            let (sum_bit, carry_out) = FsmOps::create_full_adder(self, a_bit, b_bit, carry);
            sum.push(sum_bit);
            carry = carry_out;
        }
        sum
    }

    fn create_subtraction(&mut self, a: &[Value], b: &[Value]) -> Vec<Value> {
        assert_eq!(a.len(), b.len(), "Inputs must have equal widths!");

        let mut carry = Value::Constant(true);
        let mut difference = Vec::with_capacity(a.len());
        for (&a_bit, &b_bit) in a.into_iter().zip(b) {
            let (difference_bit, carry_out) = FsmOps::create_full_adder(self, a_bit, !b_bit, carry);
            difference.push(difference_bit);
            carry = carry_out;
        }
        difference
    }

    fn create_multiplication(&mut self, a: &[Value], b: &[Value]) -> Vec<Value> {
        assert_eq!(a.len(), b.len(), "Inputs must have equal widths!");

        let width = a.len();
        let mut product = vec![Value::Constant(false); width];
        for (shift, &b_bit) in b.into_iter().enumerate() {
            let partial_product = (0..width)
                .map(|index| {
                    index
                        .checked_sub(shift)
                        .map_or(Value::Constant(false), |source| {
                            Value::from(self.add_variable_gate_binary(
                                GateType::And,
                                a[source],
                                b_bit,
                            ))
                        })
                })
                .collect::<Vec<_>>();
            product = FsmOps::create_addition(self, &product, &partial_product);
        }
        product
    }

    fn create_unsigned_division(&mut self, dividend: &[Value], divisor: &[Value]) -> Vec<Value> {
        assert_eq!(
            dividend.len(),
            divisor.len(),
            "Inputs must have equal widths!"
        );

        let width = dividend.len();
        let mut quotient = vec![Value::Constant(false); width];
        let mut remainder = vec![Value::Constant(false); width + 1];
        let mut extended_divisor = divisor.to_vec();
        extended_divisor.push(Value::Constant(false));

        for index in (0..width).rev() {
            remainder.pop();
            remainder.insert(0, dividend[index]);
            let subtract = FsmOps::create_comparison(
                self,
                &remainder,
                &extended_divisor,
                Comparison::GreaterThanOrEqual,
            );
            let difference = FsmOps::create_subtraction(self, &remainder, &extended_divisor);
            remainder = FsmOps::create_mux(self, &remainder, &difference, subtract);
            quotient[index] = subtract;
        }
        quotient
    }

    fn create_signed_division(&mut self, dividend: &[Value], divisor: &[Value]) -> Vec<Value> {
        assert_eq!(
            dividend.len(),
            divisor.len(),
            "Inputs must have equal widths!"
        );
        if dividend.is_empty() {
            return Vec::new();
        }

        let dividend_sign = *dividend.last().unwrap();
        let divisor_sign = *divisor.last().unwrap();
        let dividend_magnitude = create_conditional_negation(self, dividend, dividend_sign);
        let divisor_magnitude = create_conditional_negation(self, divisor, divisor_sign);
        let quotient =
            FsmOps::create_unsigned_division(self, &dividend_magnitude, &divisor_magnitude);
        let quotient_sign = FsmOps::create_xor_gate(self, dividend_sign, divisor_sign);
        create_conditional_negation(self, &quotient, quotient_sign)
    }

    fn create_comparison(&mut self, a: &[Value], b: &[Value], comparison: Comparison) -> Value {
        assert_eq!(a.len(), b.len(), "Inputs must have equal widths!");

        match comparison {
            Comparison::Equals => create_equals(self, a, b),
            Comparison::NotEquals => !create_equals(self, a, b),
            Comparison::LessThan => create_less_than(self, a, b),
            Comparison::GreaterThan => create_less_than(self, b, a),
            Comparison::LessThanOrEqual => !create_less_than(self, b, a),
            Comparison::GreaterThanOrEqual => !create_less_than(self, a, b),
        }
    }

    fn create_mux_gate(&mut self, a: Value, b: Value, select: Value) -> Value {
        if let Value::Constant(select) = select {
            return if select { b } else { a };
        }
        if a == b {
            return a;
        }
        if a == Value::Constant(false) && b == Value::Constant(true) {
            return select;
        }
        if a == Value::Constant(true) && b == Value::Constant(false) {
            return !select;
        }
        if a == select {
            return Value::from(self.add_variable_gate_binary(GateType::And, b, select));
        }
        if b == !select {
            return Value::from(self.add_variable_gate_binary(GateType::And, a, !select));
        }
        let select_a = Value::from(self.add_variable_gate_binary(GateType::And, a, !select));
        let select_b = Value::from(self.add_variable_gate_binary(GateType::And, b, select));
        Value::from(self.add_variable_gate_binary(GateType::Or, select_a, select_b))
    }

    fn create_mux(&mut self, a: &[Value], b: &[Value], select: Value) -> Vec<Value> {
        assert_eq!(a.len(), b.len(), "Inputs must have equal widths!");

        a.into_iter()
            .zip(b)
            .map(|(&a_bit, &b_bit)| FsmOps::create_mux_gate(self, a_bit, b_bit, select))
            .collect()
    }

    fn create_select(&mut self, value: &[Value], index: &[Value]) -> Value {
        assert!(
            index.len() < usize::BITS as usize && value.len() == 1usize << index.len(),
            "Selection input length must equal 2^selector width!"
        );

        let mut level = value.to_vec();
        for &select in index {
            level = level
                .chunks_exact(2)
                .map(|pair| FsmOps::create_mux_gate(self, pair[0], pair[1], select))
                .collect();
        }
        level[0]
    }

    fn create_shifter(
        &mut self,
        value: &[Value],
        shift: &[Value],
        operation: ShiftOperation,
    ) -> Vec<Value> {
        assert!(
            shift.len() < usize::BITS as usize && (1usize << shift.len()) <= value.len(),
            "Shifter requires 2^shift width <= input width!"
        );

        let width = value.len();
        let mut output = value.to_vec();
        let mut rotate_distance = if width == 0 { 0 } else { 1 % width };

        for (stage, &select) in shift.into_iter().enumerate() {
            let shift_distance = if stage >= usize::BITS as usize {
                width
            } else {
                1usize << stage
            };
            let distance = match operation {
                ShiftOperation::RotateLeft | ShiftOperation::RotateRight => rotate_distance,
                _ => shift_distance,
            };

            let shifted = match operation {
                ShiftOperation::ShiftLeft => (0..width)
                    .map(|index| {
                        index
                            .checked_sub(distance)
                            .map_or(Value::Constant(false), |source| output[source])
                    })
                    .collect::<Vec<_>>(),
                ShiftOperation::ShiftRight => (0..width)
                    .map(|index| {
                        index
                            .checked_add(distance)
                            .filter(|&source| source < width)
                            .map_or(Value::Constant(false), |source| output[source])
                    })
                    .collect::<Vec<_>>(),
                ShiftOperation::ShiftRightArithmetic => (0..width)
                    .map(|index| {
                        index
                            .checked_add(distance)
                            .filter(|&source| source < width)
                            .map_or(output[width - 1], |source| output[source])
                    })
                    .collect::<Vec<_>>(),
                ShiftOperation::RotateLeft => (0..width)
                    .map(|index| output[(index + width - distance) % width])
                    .collect::<Vec<_>>(),
                ShiftOperation::RotateRight => (0..width)
                    .map(|index| output[(index + distance) % width])
                    .collect::<Vec<_>>(),
            };

            output = FsmOps::create_mux(self, &output, &shifted, select);

            if width != 0 {
                rotate_distance = if rotate_distance >= width - rotate_distance {
                    rotate_distance - (width - rotate_distance)
                } else {
                    rotate_distance + rotate_distance
                };
            }
        }
        output
    }
}

fn create_equals(fsm: &mut FSM, a: &[Value], b: &[Value]) -> Value {
    let mut equals = Value::Constant(true);
    for (&a_bit, &b_bit) in a.into_iter().zip(b) {
        let bits_equal = !FsmOps::create_xor_gate(fsm, a_bit, b_bit);
        equals = Value::from(fsm.add_variable_gate_binary(GateType::And, equals, bits_equal));
    }
    equals
}

fn create_less_than(fsm: &mut FSM, a: &[Value], b: &[Value]) -> Value {
    let mut less_than = Value::Constant(false);
    for (&a_bit, &b_bit) in a.into_iter().zip(b) {
        let bits_equal = !FsmOps::create_xor_gate(fsm, a_bit, b_bit);
        let lower_bits_less =
            Value::from(fsm.add_variable_gate_binary(GateType::And, bits_equal, less_than));
        let current_bit_less =
            Value::from(fsm.add_variable_gate_binary(GateType::And, !a_bit, b_bit));
        less_than = Value::from(fsm.add_variable_gate_binary(
            GateType::Or,
            lower_bits_less,
            current_bit_less,
        ));
    }
    less_than
}

fn create_conditional_negation(fsm: &mut FSM, value: &[Value], negate: Value) -> Vec<Value> {
    let zero = vec![Value::Constant(false); value.len()];
    let negated = FsmOps::create_subtraction(fsm, &zero, value);
    FsmOps::create_mux(fsm, value, &negated, negate)
}

#[cfg(test)]
mod tests {
    use verilator_utils::{fsm::FSM, sim::Simulator, value::Value};

    use crate::ops::{FsmOps, ShiftOperation};

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
}
