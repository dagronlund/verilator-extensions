use crate::{
    fsm::{FSM, VariableDriver},
    gate::GateType,
    value::Value,
    variable::Var,
};

impl FSM {
    /// Reorder and compact variables into the order required by binary AIGER:
    /// inputs, latch outputs, then topologically ordered gate outputs.
    pub fn normalize_ordering(&mut self) {
        let mut mapping = vec![None; self.get_num_variables()];
        let mut next_variable_index = 0;

        // Preserve the relative order of inputs, latches, and gates with each
        // other while grouping them inputs, latches, then gates
        for (variable_index, (driver, _)) in self.get_variables().into_iter().enumerate() {
            if let VariableDriver::Input = driver {
                mapping[variable_index] = Some(Var::from((true, next_variable_index)));
                next_variable_index += 1;
            }
        }
        for (variable_index, (driver, _)) in self.get_variables().into_iter().enumerate() {
            if let VariableDriver::Latch(_) = driver {
                mapping[variable_index] = Some(Var::from((true, next_variable_index)));
                next_variable_index += 1;
            }
        }
        for (variable_index, (driver, _)) in self.get_variables().into_iter().enumerate() {
            if let VariableDriver::Gate(_) = driver {
                mapping[variable_index] = Some(Var::from((true, next_variable_index)));
                next_variable_index += 1;
            }
        }

        let remap_value = |value: Value| {
            value.map_variable(|variable| mapping[variable.index()].unwrap().xor(!variable.sign()))
        };
        let mut new_variables = vec![(VariableDriver::None, None); next_variable_index];
        for (variable_index, (driver, label)) in
            self.get_variables().clone().into_iter().enumerate()
        {
            let Some(new_variable) = mapping[variable_index] else {
                assert_eq!(driver, VariableDriver::None);
                continue;
            };
            let remapped_driver = match driver {
                VariableDriver::Gate(mut gate) => {
                    gate.a = remap_value(gate.a);
                    gate.b = remap_value(gate.b);
                    gate.sort_inputs();
                    VariableDriver::Gate(gate)
                }
                VariableDriver::Latch(mut latch) => {
                    latch.input = remap_value(latch.input);
                    VariableDriver::Latch(latch)
                }
                VariableDriver::Input => VariableDriver::Input,
                VariableDriver::None => unreachable!(),
            };
            new_variables[new_variable.index()] = (remapped_driver, label);
        }
        self.variables = new_variables;
        for values in [
            &mut self.outputs,
            &mut self.asserts,
            &mut self.assumes,
            &mut self.covers,
        ] {
            for (value, _) in values {
                *value = remap_value(*value);
            }
        }
        self.reorder_gates();
    }

    /// Normalize gate/latch outputs to support AIGER format, which requires all
    /// gate/latch outputs to be positive. However, since AIGER only allows AND
    /// gates, OR gates are represented as inverted AND gates, which means that
    /// the output of an OR gate must be negative.
    pub fn normalize_outputs(&mut self) {
        // Find all flipped output variables from gates and latches, mark them,
        // and then flip them to be positive
        let mut flipped = vec![false; self.get_num_variables()];
        for (variable_index, (driver, _)) in (&mut self.variables).into_iter().enumerate() {
            if let VariableDriver::Gate(gate) = driver {
                match gate.gate_type {
                    GateType::And => {
                        if !gate.sign {
                            gate.sign = !gate.sign;
                            flipped[variable_index] = true;
                        }
                    }
                    GateType::Or => {
                        if gate.sign {
                            gate.sign = !gate.sign;
                            flipped[variable_index] = true;
                        }
                    }
                }
            }
            if let VariableDriver::Latch(latch) = driver
                && !latch.sign
            {
                latch.sign = !latch.sign;
                flipped[variable_index] = true;
            }
        }
        // Update inputs to reflect flipped outputs
        for (driver, _) in &mut self.variables {
            if let VariableDriver::Gate(gate) = driver {
                for input in gate.get_inputs_mut() {
                    *input = input.map_variable(|v| v.xor(flipped[v.index()]));
                }
                gate.sort_inputs();
            }
            if let VariableDriver::Latch(latch) = driver {
                latch.input = latch.input.map_variable(|v| v.xor(flipped[v.index()]));
            }
        }
        for (v, _) in &mut self.outputs {
            *v = v.map_variable(|v| v.xor(flipped[v.index()]));
        }
        for (v, _) in &mut self.asserts {
            *v = v.map_variable(|v| v.xor(flipped[v.index()]));
        }
        for (v, _) in &mut self.assumes {
            *v = v.map_variable(|v| v.xor(flipped[v.index()]));
        }
        for (v, _) in &mut self.covers {
            *v = v.map_variable(|v| v.xor(flipped[v.index()]));
        }
    }
}
