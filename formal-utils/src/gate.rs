#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GateType {
    Or,
    And,
}

impl std::fmt::Display for GateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Or => write!(f, "|"),
            Self::And => write!(f, "&"),
        }
    }
}

impl GateType {
    pub fn opposite(&self) -> Self {
        match self {
            GateType::And => GateType::Or,
            GateType::Or => GateType::And,
        }
    }

    /// Returns the boolean identity input for this gate type (which value
    /// leaves the output unchanged when used as an input), i.e. the opposite of
    /// the controlling input
    pub fn get_identity_input(&self) -> bool {
        match self {
            GateType::And => true,
            GateType::Or => false,
        }
    }

    /// Returns the boolean controlling input for this gate type (which value
    /// determines the output regardless of the other inputs), i.e. the opposite
    /// of the identity input
    pub fn get_controlling_input(&self) -> bool {
        match self {
            GateType::And => false,
            GateType::Or => true,
        }
    }

    /// Evaluates the gate with constant inputs
    pub fn eval_constants(&self, inputs: &[bool]) -> bool {
        match self {
            GateType::And => inputs.into_iter().all(|&v| v),
            GateType::Or => inputs.into_iter().any(|&v| v),
        }
    }

    /// Given a bool operation and an iterator of operands, computes what the
    /// output value will be, either true, false, or unknown
    pub fn eval(&self, values: impl IntoIterator<Item = Option<bool>>) -> Option<bool> {
        let mut unknown = false;
        for value in values {
            match value {
                Some(value) => {
                    if value == self.get_controlling_input() {
                        return Some(self.get_controlling_input());
                    }
                }
                None => unknown = true,
            }
        }
        if !unknown {
            Some(!self.get_controlling_input())
        } else {
            None
        }
    }
}
