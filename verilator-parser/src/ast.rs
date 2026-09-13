pub mod collect;
mod decimal_biguint;
pub mod range;

use num_bigint::BigUint;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ast::range::Range;

/// An index to a verilator data type in the AST
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DataTypeId(pub usize);

/// An index to a verilator variable in the AST
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VariableId(pub usize);

/// Verilator specific information about the source of a node in the AST
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub node_type: String,
    pub address: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Edge {
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Domain {
    pub name: String,
    pub edge: Edge,
}

impl Domain {
    pub fn new(name: impl Into<String>, edge: Edge) -> Self {
        Self {
            name: name.into(),
            edge,
        }
    }
}

impl FromStr for Domain {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (name, edge) = match value.strip_prefix('!') {
            Some(name) => (name, Edge::Negative),
            None => (value, Edge::Positive),
        };
        if name.is_empty() {
            return Err("signal name must not be empty".to_string());
        }
        Ok(Self::new(name, edge))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalDomain {
    pub variable: VariableId,
    pub domain: Domain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataTypeKind {
    Basic {
        packed: Range,
    },
    Alias {
        target: DataTypeId,
    },
    Enum {
        base: DataTypeId,
        variants: Vec<EnumVariant>,
    },
    PackedArray {
        element: DataTypeId,
        declared: Range,
    },
    UnpackedArray {
        element: DataTypeId,
        declared: Range,
    },
    PackedStruct {
        members: Vec<DataTypeMember>,
    },
    PackedUnion {
        members: Vec<DataTypeMember>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub source: SourceInfo,
    pub name: String,
    pub value: Literal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataTypeMember {
    pub source: SourceInfo,
    pub name: String,
    pub dtype: DataTypeId,
    pub width: usize,
    pub lsb: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackedLayout {
    pub element_width: usize,
    pub indices: Range,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataType {
    pub source: SourceInfo,
    pub name: Option<String>,
    pub width: usize,
    pub indices: Range,
    pub signed: bool,
    pub unpacked: Option<UnpackedLayout>,
    pub kind: DataTypeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Input,
    Output,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableKind {
    Port,
    ModuleTemporary,
    Variable,
    Wire,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyKind {
    Assertion,
    Assumption,
    Cover,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Property {
    pub kind: PropertyKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    pub source: SourceInfo,
    pub name: String,
    pub original_name: Option<String>,
    pub dtype: DataTypeId,
    pub direction: Direction,
    pub kind: VariableKind,
    pub top_level: bool,
    pub internal: bool,
    pub sampled_value: Option<Expression>,
    pub property: Option<Property>,
}

impl Variable {
    pub fn display_name(&self) -> &str {
        self.original_name.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentKind {
    Blocking,
    Delayed,
    Continuous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessMode {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    Initial,
    Always,
    AlwaysPre,
    AlwaysPost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentTarget {
    Variable {
        variable: VariableId,
        access: AccessMode,
    },
    Select {
        target: Box<AssignmentTarget>,
        offset: Expression,
        width: usize,
    },
    ArrayElement {
        array: Box<AssignmentTarget>,
        index: Expression,
        dtype: DataTypeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statement {
    pub source: SourceInfo,
    pub kind: StatementKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatementKind {
    Block {
        kind: BlockKind,
        statements: Vec<Statement>,
    },
    Assignment {
        kind: AssignmentKind,
        target: AssignmentTarget,
        value: Expression,
    },
    If {
        condition: Expression,
        then_statements: Vec<Statement>,
        else_statements: Vec<Statement>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    BitwiseNot,
    Negate,
    ReduceAnd,
    ReduceOr,
    ReduceXor,
    LogicalNot,
    ZeroExtend,
    SignExtend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOperator {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Add,
    Subtract,
    MultiplyUnsigned,
    MultiplySigned,
    DivideUnsigned,
    DivideSigned,
    Equal,
    NotEqual,
    LessThanUnsigned,
    LessThanOrEqualUnsigned,
    GreaterThanUnsigned,
    GreaterThanOrEqualUnsigned,
    LessThanSigned,
    LessThanOrEqualSigned,
    GreaterThanSigned,
    GreaterThanOrEqualSigned,
    LogicalAnd,
    LogicalOr,
    ShiftLeft,
    ShiftRight,
    ShiftRightArithmetic,
    Concat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Literal {
    pub spelling: String,
    #[serde(with = "decimal_biguint")]
    pub value: BigUint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expression {
    pub source: SourceInfo,
    pub dtype: DataTypeId,
    pub kind: ExpressionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpressionKind {
    Constant(Literal),
    Variable {
        variable: VariableId,
        access: AccessMode,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },
    Binary {
        operator: BinaryOperator,
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    Conditional {
        condition: Box<Expression>,
        then_value: Box<Expression>,
        else_value: Box<Expression>,
    },
    Replicate {
        source: Box<Expression>,
        count_literal: Literal,
        count: usize,
    },
    Select {
        value: Box<Expression>,
        offset: Box<Expression>,
        width: usize,
    },
    ArraySelect {
        array: Box<Expression>,
        index: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    pub source: SourceInfo,
    pub data_types: Vec<DataType>,
    pub variables: Vec<Variable>,
    pub sensitivity_domains: Vec<SignalDomain>,
    pub shadow_registers: Vec<(VariableId, VariableId)>,
    pub initial: Vec<Statement>,
    pub combinational: Vec<Statement>,
    pub pre_edge: Vec<Statement>,
    pub sequential: Vec<Statement>,
    pub post_edge: Vec<Statement>,
}

impl Design {
    pub fn data_type(&self, id: DataTypeId) -> &DataType {
        &self.data_types[id.0]
    }

    pub fn variable(&self, id: VariableId) -> &Variable {
        &self.variables[id.0]
    }

    /// Returns the number of array-like layers in a data type.
    ///
    /// A single bit has rank zero and a wider bit vector has rank one. Aliases
    /// and enums preserve their underlying rank. Packed structs are treated as
    /// arrays whose elements may have different ranks, so their rank is one
    /// greater than the largest rank of any member.
    pub fn data_type_rank(&self, id: DataTypeId) -> usize {
        let dtype = self.data_type(id);
        match &dtype.kind {
            DataTypeKind::Alias { target } => self.data_type_rank(*target),
            DataTypeKind::Enum { base, .. } => self.data_type_rank(*base),
            DataTypeKind::PackedArray { element, .. }
            | DataTypeKind::UnpackedArray { element, .. } => self.data_type_rank(*element) + 1,
            DataTypeKind::PackedStruct { members } => {
                members
                    .iter()
                    .map(|member| self.data_type_rank(member.dtype))
                    .max()
                    .unwrap_or(0)
                    + 1
            }
            DataTypeKind::Basic { .. } => usize::from(dtype.width > 1),
            DataTypeKind::PackedUnion { .. } => 0,
        }
    }

    /// Returns the rank of a variable's data type.
    pub fn variable_rank(&self, id: VariableId) -> usize {
        self.data_type_rank(self.variable(id).dtype)
    }
}
