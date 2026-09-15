use crate::{
    fsm::{FSM, VariableDriver},
    value::Value,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerifyOrdering {
    /// Enforces that gates can only depend on variables with lower indices
    Verify,
    /// Allows gates to depend on variables with higher indices
    Ignore,
}

impl FSM {
    pub fn verify(&self, ordering: VerifyOrdering) {
        // Verify that all variables indicate the correct driver
        for variable_index in 0..self.get_num_variables() {
            match self.get_variables()[variable_index].0 {
                VariableDriver::Gate(gate) => {
                    for value in gate.get_inputs() {
                        if let Value::Var(variable) = value {
                            // Check that any variable inputs to the gate are driven
                            assert!(self.get_variables()[variable.index()].0.is_some());
                            // Check that the input variable has a lower index than the
                            // gate output
                            if ordering == VerifyOrdering::Verify {
                                assert!(variable.index() < variable_index);
                            }
                        }
                    }
                    // Check that gate inputs are sorted
                    assert!(gate.is_inputs_sorted());
                }
                VariableDriver::Latch(latch) => {
                    // Check all latch inputs are driven
                    if let Value::Var(variable) = &latch.input {
                        assert!(self.get_variables()[variable.index()].0.is_some());
                    }
                }
                VariableDriver::Input => {}
                VariableDriver::None => {}
            }
        }
        // Check all outputs/asserts/assumes/covers are driven
        for values in [
            self.get_outputs(),
            self.get_asserts(),
            self.get_assumes(),
            self.get_covers(),
        ] {
            for &(value, _) in values {
                if let Value::Var(variable) = value {
                    assert!(self.get_variables()[variable.index()].0.is_some());
                }
            }
        }
    }
}
