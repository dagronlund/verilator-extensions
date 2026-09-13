use std::collections::VecDeque;

use crate::{
    fsm::{FSM, VariableDriver},
    value::Value,
    variable::Var,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unvisited,
    Assigned,
}

impl FSM {
    // Verify all the inputs/latches are have output variables with lower
    // indices than the gates, otherwise reordering would have to also
    // reorder inputs and latches
    pub fn verify_gates_last(&self) -> Option<usize> {
        let mut first_gate_index: Option<usize> = None;
        let mut last_latch_index = 0;
        let mut last_input_index = 0;
        for variable_index in 0..self.get_num_variables() {
            match self.get_variables()[variable_index].0 {
                VariableDriver::Gate(_) => {
                    first_gate_index = Some(
                        first_gate_index
                            .map_or(variable_index, |existing| existing.min(variable_index)),
                    );
                }
                VariableDriver::Latch(_) => {
                    last_latch_index = last_latch_index.max(variable_index);
                }
                VariableDriver::Input => {
                    last_input_index = last_input_index.max(variable_index);
                }
                VariableDriver::None => {}
            }
        }
        let first_gate_index = first_gate_index?;
        assert!(first_gate_index > last_input_index);
        assert!(first_gate_index > last_latch_index);
        Some(first_gate_index)
    }

    /// Reorders gates in the FSM to ensure that all gate inputs are driven by
    /// earlier gates.
    pub fn reorder_gates(&mut self) {
        self.reorder_gates_with(|_, _| {});
    }

    /// Reorders gates in the FSM to ensure that all gate inputs are driven by
    /// earlier gates, invoking `on_remap` for each gate whose variable index
    /// changes.
    pub fn reorder_gates_with<F>(&mut self, mut on_remap: F)
    where
        F: FnMut(usize, usize),
    {
        // Make sure gates exist before reordering
        let Some(first_gate_index) = self.verify_gates_last() else {
            return;
        };

        // Assign new gate indices by walking gates in original variable order
        // with a queue seeded by all variable indices. Gates are only assigned
        // once all gate inputs have already been assigned, so already-valid
        // prefixes keep their indices without a separate special case.
        let mut mapping = (0..self.get_num_variables())
            .map(|i| Var::from((true, i)))
            .collect::<Vec<_>>();
        let mut visit_state = vec![VisitState::Unvisited; self.get_num_variables()];
        let mut next_gate_index = first_gate_index;
        let mut queue = (0..self.get_num_variables()).collect::<VecDeque<_>>();
        let mut blocked = 0;
        while let Some(variable_index) = queue.pop_front() {
            if visit_state[variable_index] == VisitState::Assigned {
                continue;
            }
            let VariableDriver::Gate(gate) = self.get_variables()[variable_index].0 else {
                continue;
            };
            if !gate.get_inputs().into_iter().all(|input| match input {
                Value::Var(input) => {
                    self.get_variables()[input.index()].0.get_gate().is_none()
                        || visit_state[input.index()] == VisitState::Assigned
                }
                Value::Constant(_) => true,
            }) {
                // If inputs have not been assigned yet, skip for now
                queue.push_back(variable_index);
                blocked += 1;
                // If we have walked the whole queue without being able to
                // assign a gate because its inputs are not yet assigned
                // then there is a cycle in the gate dependencies
                assert!(
                    blocked < queue.len(),
                    "cannot reorder cyclic gate dependency involving variable {}",
                    variable_index + 1
                );
                continue;
            }
            // Assign the gate a new index
            mapping[variable_index] = Var::from((true, next_gate_index));
            next_gate_index += 1;
            visit_state[variable_index] = VisitState::Assigned;
            blocked = 0;
        }

        let remap_fn =
            |value: Value| -> Value { value.map_variable(|v| mapping[v.index()].xor(!v.sign())) };

        // Rebuild the variable array with gates moved to their remapped
        // indices and every reference updated to match.
        let mut new_variables = vec![(VariableDriver::None, None); self.get_num_variables()];
        for old_variable_index in 0..self.get_num_variables() {
            let (driver, label) = self.get_variables()[old_variable_index].clone();
            match driver {
                VariableDriver::Gate(mut gate) => {
                    // Replace the gate inputs with the remapped variable indices
                    gate.a = remap_fn(gate.a);
                    gate.b = remap_fn(gate.b);
                    gate.sort_inputs();
                    let new_variable_index = mapping[old_variable_index].index();
                    new_variables[new_variable_index] = (VariableDriver::Gate(gate), label);
                    // Indicate the gate was remapped to a new index
                    if new_variable_index != old_variable_index {
                        on_remap(old_variable_index, new_variable_index);
                    }
                }
                VariableDriver::Latch(mut latch) => {
                    // Replace the latch input with the remapped variable index
                    latch.input = remap_fn(latch.input);
                    new_variables[old_variable_index] = (VariableDriver::Latch(latch), label);
                }
                VariableDriver::Input => {
                    // Leave inputs in place
                    new_variables[old_variable_index] = (VariableDriver::Input, label);
                }
                VariableDriver::None => {}
            }
        }
        self.variables = new_variables;
        for (value, _) in self.get_outputs_mut() {
            *value = remap_fn(*value);
        }
        for (value, _) in self.get_asserts_mut() {
            *value = remap_fn(*value);
        }
        for (value, _) in self.get_assumes_mut() {
            *value = remap_fn(*value);
        }
        for (value, _) in self.get_covers_mut() {
            *value = remap_fn(*value);
        }
    }
}
