use crate::variable::Var;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Var(Var),
    Constant(bool),
}

impl Value {
    pub fn unwrap_variable(&self) -> Var {
        match self {
            Self::Var(variable) => *variable,
            Self::Constant(_) => panic!("Value is a constant!"),
        }
    }

    pub fn map_variable<F>(&self, f: F) -> Self
    where
        F: FnOnce(Var) -> Var,
    {
        match self {
            Self::Var(variable) => Self::Var(f(*variable)),
            Self::Constant(value) => Self::Constant(*value),
        }
    }

    pub fn map_variable_value<F>(&self, f: F) -> Self
    where
        F: FnOnce(Var) -> Value,
    {
        match self {
            Self::Var(variable) => f(*variable),
            Self::Constant(value) => Self::Constant(*value),
        }
    }

    pub fn map_variable_bool<F>(&self, f: F) -> Option<bool>
    where
        F: FnOnce(Var) -> Option<bool>,
    {
        match self {
            Self::Var(variable) => f(*variable),
            Self::Constant(value) => Some(*value),
        }
    }

    pub fn get_variable(&self) -> Option<Var> {
        match self {
            Self::Var(variable) => Some(*variable),
            Self::Constant(_) => None,
        }
    }

    pub fn get_constant(&self) -> Option<bool> {
        match self {
            Self::Var(_) => None,
            Self::Constant(value) => Some(*value),
        }
    }

    pub fn offset(self, offset: usize) -> Self {
        match self {
            Self::Var(var) => Self::Var(var.offset(offset)),
            Self::Constant(value) => Self::Constant(value),
        }
    }

    pub fn xor(&self, invert: bool) -> Self {
        match self {
            Self::Var(var) => Self::Var(var.xor(invert)),
            Self::Constant(value) => Self::Constant(*value ^ invert),
        }
    }

    pub fn is_constant(&self) -> bool {
        matches!(self, Self::Constant(_))
    }
}

impl std::ops::Not for Value {
    type Output = Value;

    fn not(self) -> Value {
        match self {
            Self::Var(var) => Self::Var(!var),
            Self::Constant(value) => Self::Constant(!value),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Var(var) => write!(f, "{var}"),
            Self::Constant(false) => write!(f, "F"),
            Self::Constant(true) => write!(f, "T"),
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Constant(value)
    }
}

impl From<Var> for Value {
    fn from(var: Var) -> Self {
        Value::Var(var)
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Constants come before variables
        match (self, other) {
            (Value::Constant(a), Value::Constant(b)) => a.cmp(b),
            (Value::Var(a), Value::Var(b)) => a.cmp(b),
            (Value::Constant(_), Value::Var(_)) => std::cmp::Ordering::Less,
            (Value::Var(_), Value::Constant(_)) => std::cmp::Ordering::Greater,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
