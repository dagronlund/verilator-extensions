pub type VarType = i32;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Var(VarType);

impl Var {
    pub fn value(&self) -> VarType {
        self.0
    }

    pub fn index(&self) -> usize {
        self.0.unsigned_abs() as usize - 1
    }

    pub fn sign(&self) -> bool {
        self.0.is_positive()
    }

    /// Invert if true, buffer if false
    pub fn xor(&self, input: bool) -> Self {
        if input { !self } else { *self }
    }

    pub fn offset(&self, offset: usize) -> Self {
        Self::from((self.sign(), self.index() + offset))
    }

    pub fn null() -> Self {
        Self(0)
    }
}

impl From<VarType> for Var {
    fn from(value: VarType) -> Self {
        assert!(value != 0);
        Var(value)
    }
}

impl From<(bool, usize)> for Var {
    fn from((sign, index): (bool, usize)) -> Self {
        Var::from(if sign {
            index as VarType + 1
        } else {
            -(index as VarType + 1)
        })
    }
}

impl From<Var> for VarType {
    fn from(value: Var) -> Self {
        value.value()
    }
}

impl From<Var> for usize {
    fn from(value: Var) -> Self {
        value.index()
    }
}

impl From<Var> for bool {
    fn from(value: Var) -> Self {
        value.sign()
    }
}

impl std::fmt::Display for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Debug for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Ord for Var {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.index() != other.index() {
            // Order based on index if they differ
            self.index().cmp(&other.index())
        } else {
            // Otherwise order based on sign (negatives first)
            self.sign().cmp(&other.sign())
        }
    }
}

impl PartialOrd for Var {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::ops::Not for Var {
    type Output = Self;

    fn not(self) -> Self {
        Self(-self.0)
    }
}

impl std::ops::Not for &Var {
    type Output = Var;

    fn not(self) -> Var {
        Var(-self.0)
    }
}
