pub mod gate;
pub mod latch;
pub mod normalize;
pub mod reorder;
pub mod verify;

use crate::{
    fsm::{
        gate::{Gate, GateOutput},
        latch::{Latch, LatchOutput},
    },
    gate::GateType,
    value::Value,
    variable::Var,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariableDriver {
    Input,
    Latch(Latch),
    Gate(Gate),
    None,
}

impl VariableDriver {
    pub fn is_none(&self) -> bool {
        *self == VariableDriver::None
    }

    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    pub fn is_input(&self) -> bool {
        *self == VariableDriver::Input
    }

    pub fn get_latch(&self) -> Option<Latch> {
        match self {
            VariableDriver::Latch(latch) => Some(*latch),
            _ => None,
        }
    }

    pub fn get_latch_mut(&mut self) -> Option<&mut Latch> {
        match self {
            VariableDriver::Latch(latch) => Some(latch),
            _ => None,
        }
    }

    pub fn get_gate(&self) -> Option<Gate> {
        match self {
            VariableDriver::Gate(gate) => Some(*gate),
            _ => None,
        }
    }

    pub fn get_gate_mut(&mut self) -> Option<&mut Gate> {
        match self {
            VariableDriver::Gate(gate) => Some(gate),
            _ => None,
        }
    }
}

/// Generalized model of a finite-state machine, with multiple inputs, outputs,
/// state stored in latches, gates representing internal logic, and
/// assertions/assumptions.
///
/// Note: Latches in model checking are registers in hardware design, and not
/// active-high/low latches in hardware design.
#[derive(Debug, Clone, Default)]
pub struct FSM {
    variables: Vec<(VariableDriver, Option<String>)>,
    outputs: Vec<(Value, Option<String>)>,
    asserts: Vec<(Value, Option<String>)>,
    assumes: Vec<(Value, Option<String>)>,
    covers: Vec<(Value, Option<String>)>,
}

impl FSM {
    pub fn new(num_variables: usize) -> Self {
        FSM {
            variables: vec![(VariableDriver::None, None); num_variables],
            ..Default::default()
        }
    }

    pub fn from(
        variables: Vec<(VariableDriver, Option<String>)>,
        outputs: Vec<(Value, Option<String>)>,
        asserts: Vec<(Value, Option<String>)>,
        assumes: Vec<(Value, Option<String>)>,
        covers: Vec<(Value, Option<String>)>,
    ) -> Self {
        Self {
            variables,
            outputs,
            asserts,
            assumes,
            covers,
        }
    }

    /// Creates a mew variable in the FSM, returning the variable
    pub fn add_variable(&mut self) -> Var {
        let variable = Var::from((true, self.variables.len()));
        self.variables.push((VariableDriver::None, None));
        variable
    }

    /// Adds a new buffer to the FSM, indicating the variable is driven by a gate
    pub fn add_buffer(&mut self, output: Var, input: Value) {
        // TODO: Handle buffers more efficiently
        self.add_gate(GateType::And, output, input, Value::Constant(true));
    }

    /// Adds a new gate to the FSM, indicating the variable is driven by a gate
    pub fn add_gate(&mut self, gate_type: GateType, output: Var, a: Value, b: Value) {
        assert!(self.variables[output.index()].0.is_none()); // Verify variable is undriven
        let mut gate = Gate {
            sign: output.sign(),
            gate_type,
            a,
            b,
        };
        gate.sort_inputs();
        self.variables[output.index()] = (VariableDriver::Gate(gate), None);
    }

    /// Adds a new gate to the FSM, automatically creating a new driven variable
    pub fn add_variable_gate_binary(&mut self, gate: GateType, a: Value, b: Value) -> Var {
        // Create new variable
        let variable = self.add_variable();
        // Add gate driving variable
        self.add_gate(gate, variable, a, b);
        variable
    }

    /// Adds a new gate to the FSM, automatically creating a new driven variable
    pub fn add_variable_gate(&mut self, gate: GateType, mut inputs: Vec<Value>) -> Value {
        debug_assert!(!inputs.is_empty());
        while inputs.len() > 1 {
            let a = inputs.pop().unwrap();
            let b = inputs.pop().unwrap();
            let intermediate = self.add_variable_gate_binary(gate, a, b);
            inputs.push(Value::Var(intermediate));
        }
        inputs[0]
    }

    /// Adds a new buffer gate to the FSM, automatically creating a new driven variable
    pub fn add_variable_buffer(&mut self, input: Value) -> Var {
        // Create new variable
        let variable = self.add_variable();
        // Add buffer gate driving variable
        self.add_buffer(variable, input);
        variable
    }

    /// Adds a new latch to the FSM, indicating the variable is driven by a latch
    pub fn add_latch(&mut self, input: Value, output: Var, reset_value: Option<bool>) {
        while output.index() >= self.variables.len() {
            self.variables.push((VariableDriver::None, None));
        }
        assert!(self.variables[output.index()].0.is_none()); // Verify variable is undriven
        self.variables[output.index()] = (
            VariableDriver::Latch(Latch {
                sign: output.sign(),
                input,
                reset_value,
            }),
            None,
        );
    }

    pub fn add_input(&mut self, input: Var) {
        assert!(self.variables[input.index()].0.is_none()); // Verify variable is undriven
        self.variables[input.index()] = (VariableDriver::Input, None);
    }

    /// Adds a new input to the FSM, automatically creating a new driven variable
    pub fn add_variable_input(&mut self) -> Var {
        let variable = self.add_variable();
        self.add_input(variable);
        variable
    }

    pub fn add_output(&mut self, value: Value) -> usize {
        let output_index = self.outputs.len();
        self.outputs.push((value, None));
        output_index
    }

    pub fn add_assert(&mut self, value: Value) -> usize {
        let assert_index = self.asserts.len();
        self.asserts.push((value, None));
        assert_index
    }

    pub fn add_assume(&mut self, value: Value) -> usize {
        let assume_index = self.assumes.len();
        self.assumes.push((value, None));
        assume_index
    }

    pub fn add_cover(&mut self, value: Value) -> usize {
        let cover_index = self.covers.len();
        self.covers.push((value, None));
        cover_index
    }

    pub fn remove_gate(&mut self, variable_index: usize) -> Option<Gate> {
        let gate = self.get_gate(variable_index);
        self.variables[variable_index].0 = VariableDriver::None;
        gate
    }

    pub fn remove_latch(&mut self, variable_index: usize) -> Option<Latch> {
        let latch = self.get_latch(variable_index);
        self.variables[variable_index].0 = VariableDriver::None;
        latch
    }

    pub fn remove_input(&mut self, variable_index: usize) -> bool {
        let is_input = self.variables[variable_index].0.is_input();
        self.variables[variable_index].0 = VariableDriver::None;
        is_input
    }

    /// Finds all instances of the given variable and assigns them a constant
    pub fn set_unit(&mut self, unit: Var) {
        for (variable_index, (driver, _)) in (&mut self.variables).into_iter().enumerate() {
            if let VariableDriver::Gate(gate) = driver {
                assert_ne!(variable_index, unit.index());
                for input in gate.get_inputs_mut() {
                    if let Value::Var(input_var) = input
                        && input_var.index() == unit.index()
                    {
                        *input = Value::Constant(input_var.sign() == unit.sign());
                    }
                }
                gate.sort_inputs();
            }
            if let VariableDriver::Latch(latch) = driver {
                assert_ne!(variable_index, unit.index());
                if let Value::Var(input) = latch.input {
                    assert_ne!(input.index(), unit.index());
                }
            }
        }
        for (output, _) in &self.outputs {
            if let Value::Var(output) = output {
                assert_ne!(output.index(), unit.index());
            }
        }
        for (v, _) in &self.asserts {
            if let Value::Var(v) = v {
                assert_ne!(v.index(), unit.index());
            }
        }
        for (v, _) in &self.assumes {
            if let Value::Var(v) = v {
                assert_ne!(v.index(), unit.index());
            }
        }
        for (v, _) in &self.covers {
            if let Value::Var(v) = v {
                assert_ne!(v.index(), unit.index());
            }
        }
    }

    pub fn get_variables(&self) -> &Vec<(VariableDriver, Option<String>)> {
        &self.variables
    }

    pub fn get_variable_label(&self, variable_index: usize) -> &Option<String> {
        &self.variables[variable_index].1
    }

    pub fn get_variable_label_mut(&mut self, variable_index: usize) -> &mut Option<String> {
        &mut self.variables[variable_index].1
    }

    pub fn is_input(&self, variable_index: usize) -> bool {
        self.variables[variable_index].0.is_input()
    }

    pub fn get_latch(&self, variable_index: usize) -> Option<Latch> {
        self.variables[variable_index].0.get_latch()
    }

    /// Gets the latch for the given variable, accounting for the sign of the
    /// given variable
    pub fn get_latch_output(&self, variable: Var) -> Option<LatchOutput> {
        let latch = self.variables[variable.index()].0.get_latch()?;
        Some(LatchOutput {
            output: variable.xor(!latch.sign),
            input: latch.input,
            reset_value: latch.reset_value,
        })
    }

    pub fn get_latch_mut(&mut self, variable_index: usize) -> Option<&mut Latch> {
        self.variables[variable_index].0.get_latch_mut()
    }

    pub fn get_gate(&self, variable_index: usize) -> Option<Gate> {
        self.variables[variable_index].0.get_gate()
    }

    pub fn get_gate_mut(&mut self, variable_index: usize) -> Option<&mut Gate> {
        self.variables[variable_index].0.get_gate_mut()
    }

    /// Gets the gate for the given variable, accounting for the sign of the
    /// given variable
    pub fn get_gate_output(&self, variable: Var) -> Option<GateOutput> {
        let gate = self.variables[variable.index()].0.get_gate()?;
        Some(GateOutput {
            output: variable.xor(!gate.sign),
            gate_type: gate.gate_type,
            a: gate.a,
            b: gate.b,
        })
    }

    /// Returns all input variables in the FSM (computed from variable drivers)
    pub fn get_inputs(&self) -> Vec<Var> {
        (&self.variables)
            .into_iter()
            .enumerate()
            .filter_map(|(i, (driver, _))| {
                if driver.is_input() {
                    Some(Var::from((true, i)))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns all latches in the FSM (computed from variable drivers)
    pub fn get_latches(&self) -> Vec<LatchOutput> {
        (&self.variables)
            .into_iter()
            .enumerate()
            .filter_map(|(i, (driver, _))| {
                if let VariableDriver::Latch(latch) = driver {
                    Some(LatchOutput {
                        output: Var::from((latch.sign, i)),
                        input: latch.input,
                        reset_value: latch.reset_value,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns all gates in the FSM (computed from variable drivers)
    pub fn get_gates(&self) -> Vec<GateOutput> {
        (&self.variables)
            .into_iter()
            .enumerate()
            .filter_map(|(i, (driver, _))| {
                if let VariableDriver::Gate(gate) = driver {
                    Some(GateOutput {
                        output: Var::from((gate.sign, i)),
                        gate_type: gate.gate_type,
                        a: gate.a,
                        b: gate.b,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_outputs(&self) -> &Vec<(Value, Option<String>)> {
        &self.outputs
    }

    pub fn get_outputs_mut(&mut self) -> &mut Vec<(Value, Option<String>)> {
        &mut self.outputs
    }

    pub fn get_output_label(&self, index: usize) -> &Option<String> {
        &self.outputs[index].1
    }

    pub fn get_output_label_mut(&mut self, index: usize) -> &mut Option<String> {
        &mut self.outputs[index].1
    }

    pub fn get_asserts(&self) -> &Vec<(Value, Option<String>)> {
        &self.asserts
    }

    pub fn get_asserts_mut(&mut self) -> &mut Vec<(Value, Option<String>)> {
        &mut self.asserts
    }

    pub fn get_assert_label(&self, index: usize) -> &Option<String> {
        &self.asserts[index].1
    }

    pub fn get_assert_label_mut(&mut self, index: usize) -> &mut Option<String> {
        &mut self.asserts[index].1
    }

    pub fn get_assumes(&self) -> &Vec<(Value, Option<String>)> {
        &self.assumes
    }

    pub fn get_assumes_mut(&mut self) -> &mut Vec<(Value, Option<String>)> {
        &mut self.assumes
    }

    pub fn get_assume_label(&self, index: usize) -> &Option<String> {
        &self.assumes[index].1
    }

    pub fn get_assume_label_mut(&mut self, index: usize) -> &mut Option<String> {
        &mut self.assumes[index].1
    }

    pub fn get_covers(&self) -> &Vec<(Value, Option<String>)> {
        &self.covers
    }

    pub fn get_covers_mut(&mut self) -> &mut Vec<(Value, Option<String>)> {
        &mut self.covers
    }

    pub fn get_cover_label(&self, index: usize) -> &Option<String> {
        &self.covers[index].1
    }

    pub fn get_cover_label_mut(&mut self, index: usize) -> &mut Option<String> {
        &mut self.covers[index].1
    }

    pub fn get_num_variables(&self) -> usize {
        self.variables.len()
    }

    pub fn invert_output(mut self) -> Self {
        assert!(self.outputs.len() == 1);
        self.outputs[0].0 = !self.outputs[0].0;
        self
    }

    /// Generates the gates that reference each variable in the FSM, as in what
    /// other variable indices reference each variable (either input or other
    /// gate variables).
    pub fn generate_gate_references(&self) -> Vec<Vec<usize>> {
        let mut references = vec![Vec::new(); self.get_num_variables()];
        for variable_index in 0..self.get_num_variables() {
            if let Some(gate) = self.get_gate(variable_index) {
                for input in gate.get_inputs() {
                    if let Value::Var(v) = input {
                        references[v.index()].push(variable_index);
                    }
                }
            }
        }
        references
    }
}
