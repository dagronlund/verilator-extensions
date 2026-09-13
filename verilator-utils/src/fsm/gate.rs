use crate::{gate::GateType, value::Value, variable::Var};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Gate {
    pub sign: bool,
    pub gate_type: GateType,
    pub a: Value,
    pub b: Value,
}

#[derive(Clone, PartialEq, Debug)]
pub struct GateOutput {
    pub output: Var,
    pub gate_type: GateType,
    pub a: Value,
    pub b: Value,
}

impl Gate {
    pub fn get_inputs(&self) -> [Value; 2] {
        debug_assert!(self.is_inputs_sorted());
        [self.a, self.b]
    }

    pub fn get_inputs_mut(&mut self) -> [&mut Value; 2] {
        debug_assert!(self.is_inputs_sorted());
        [&mut self.a, &mut self.b]
    }

    pub fn sort_inputs(&mut self) {
        if !self.is_inputs_sorted() {
            (self.a, self.b) = (self.b, self.a);
        }
    }

    pub fn is_inputs_sorted(&self) -> bool {
        self.a <= self.b
    }

    pub fn contains_input(&self, input: Value) -> bool {
        self.a == input || self.b == input
    }

    pub fn contains_input_either(&self, input: Value) -> bool {
        self.contains_input(input) || self.contains_input(!input)
    }

    pub fn constains_constant_input(&self) -> bool {
        self.a.is_constant() || self.b.is_constant()
    }
}

impl GateOutput {
    pub fn get_inputs(&self) -> [Value; 2] {
        [self.a, self.b]
    }

    pub fn is_inputs_sorted(&self) -> bool {
        self.a <= self.b
    }

    pub fn contains_input(&self, input: Value) -> bool {
        self.a == input || self.b == input
    }

    pub fn contains_input_either(&self, input: Value) -> bool {
        self.contains_input(input) || self.contains_input(!input)
    }

    pub fn constains_constant_input(&self) -> bool {
        self.a.is_constant() || self.b.is_constant()
    }
}
