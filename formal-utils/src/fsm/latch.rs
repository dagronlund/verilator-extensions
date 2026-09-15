use crate::{value::Value, variable::Var};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Latch {
    pub sign: bool,
    pub input: Value,
    pub reset_value: Option<bool>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LatchOutput {
    pub output: Var,
    pub input: Value,
    pub reset_value: Option<bool>,
}
