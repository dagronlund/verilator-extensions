use crate::{fsm::FSM, gate::GateType, option_bit_vec::OptionBitVec, value::Value, variable::Var};

/// Simulator for finite-state machines, wraps FSM object and keeps track of
/// internal variables/state. All variables are modeled as either true, false,
/// or unknown.
#[derive(Clone, Debug)]
pub struct Simulator {
    fsm: FSM,
    variables: OptionBitVec,
}

impl From<FSM> for Simulator {
    fn from(fsm: FSM) -> Self {
        let variables = OptionBitVec::new(fsm.get_variables().len());
        let mut sim = Self { fsm, variables };
        sim.reset_state();
        sim
    }
}

impl Simulator {
    /// Overrides the given variables index with the value
    pub fn set_variable(&mut self, variable_index: usize, value: Option<bool>) {
        self.variables.set_option(variable_index, value);
    }

    /// Retrieves the value for this cycle of the given variable index
    pub fn get_variable(&self, variable_index: usize) -> Option<bool> {
        self.variables.get(variable_index)
    }

    /// Retrieves the signed value for this cycle of the given variable
    pub fn get_variable_signed(&self, variable: Var) -> Option<bool> {
        self.get_variable(variable.index())
            .map(|value| value ^ !variable.sign())
    }

    /// Retrieves the signed value for this cycle of the given value
    pub fn get_value_signed(&self, value: Value) -> Option<bool> {
        match value {
            Value::Var(v) => self.get_variable_signed(v),
            Value::Constant(c) => Some(c),
        }
    }

    /// Assigns an input value for this cycle for the given variable index
    pub fn set_input(&mut self, variable_index: usize, value: Option<bool>) {
        assert!(self.fsm.get_variables()[variable_index].0.is_input());
        self.set_variable(variable_index, value);
    }

    /// Retrieves the value for this cycle for the given output index, not the
    /// variable index
    pub fn get_output(&self, index: usize) -> Option<bool> {
        self.get_value_signed(self.fsm.get_outputs()[index].0)
    }

    /// Retrieves the combined value for this cycle of all outputs for the given
    /// gate
    pub fn get_combined_outputs(&self, gate_type: GateType) -> bool {
        match gate_type {
            GateType::And => {
                (0..self.get_fsm().get_outputs().len()).all(|i| self.get_output(i).unwrap())
            }
            GateType::Or => {
                (0..self.get_fsm().get_outputs().len()).any(|i| self.get_output(i).unwrap())
            }
        }
    }

    /// Retrieves the value for this cycle for the given assertion index, not
    /// the variable index
    pub fn get_assert(&self, index: usize) -> Option<bool> {
        self.get_value_signed(self.fsm.get_asserts()[index].0)
    }

    /// Retrieves the value for this cycle for the given assumption index, not
    /// the variable index
    pub fn get_assume(&self, index: usize) -> Option<bool> {
        self.get_value_signed(self.fsm.get_assumes()[index].0)
    }

    /// Retrieves the value for this cycle for the given cover index, not the
    /// variable index
    pub fn get_cover(&self, index: usize) -> Option<bool> {
        self.get_value_signed(self.fsm.get_covers()[index].0)
    }

    /// Returns the internal variables bit vector
    pub fn get_variables(&self) -> &OptionBitVec {
        &self.variables
    }

    /// Evaluates a specific variable
    pub fn eval_variable(&mut self, variable_index: usize) {
        if let Some(gate) = self.fsm.get_gate(variable_index) {
            self.variables.set_option(
                variable_index,
                gate.gate_type
                    .eval(
                        gate.get_inputs()
                            .into_iter()
                            .map(|i| self.get_value_signed(i)),
                    )
                    .map(|value| value ^ !gate.sign),
            );
        }
    }

    /// Evaluates any gates from the current input and state
    pub fn eval(&mut self) {
        for variable_index in 0..self.fsm.get_num_variables() {
            self.eval_variable(variable_index);
        }
    }

    /// Transfers any latch input state to output state, equivalent to a clock
    /// edge
    pub fn step(&mut self) {
        // Create copy of existing variables to avoid overwriting inputs early
        let mut new_variables = self.variables.clone();
        // Update latch outputs based on their inputs
        for latch in self.fsm.get_latches() {
            new_variables.set_option(
                latch.output.index(),
                self.get_value_signed(latch.input)
                    .map(|value| value ^ !latch.output.sign()),
            );
        }
        // Replace existing variables with updated latch outputs
        self.variables = new_variables;
    }

    /// Resets any internal state to default values, does not modify inputs
    pub fn reset_state(&mut self) {
        for latch in self.fsm.get_latches() {
            self.variables.set_option(
                latch.output.index(),
                latch.reset_value.map(|value| value ^ !latch.output.sign()),
            );
        }
    }

    /// Resets all variables to None
    pub fn reset(&mut self) {
        for i in 0..self.fsm.get_variables().len() {
            self.variables.clear(i);
        }
    }

    pub fn get_fsm(&self) -> &FSM {
        &self.fsm
    }
}
