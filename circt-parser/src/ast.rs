//! Owned syntax trees for the HW assembly used by the formal-core pipeline.
//! Names and literals retain their source spelling, including prefixes and escapes.
//! SSA references are deliberately unresolved: HW graph regions allow forward uses.

use bytes::Bytes;

use crate::lexer::position::LexerPosition;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Name {
    pub spelling: Bytes,
    pub position: LexerPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerLiteral {
    /// Includes an optional sign; arbitrary precision and hexadecimal are preserved.
    pub spelling: Bytes,
    pub position: LexerPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringLiteral {
    /// Includes quotes and MLIR escapes.
    pub spelling: Bytes,
    pub position: LexerPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    /// The complete `loc(...)` annotation. Debug metadata is retained as assembly.
    pub spelling: Bytes,
    pub position: LexerPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct File {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Module(Module),
    HwModule(HwModule),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    pub name: Option<Name>,
    pub attributes: Vec<Attribute>,
    pub items: Vec<Item>,
    pub location: Option<Location>,
    pub position: LexerPosition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Private,
    Nested,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HwModule {
    pub name: Name,
    pub visibility: Visibility,
    pub ports: Vec<Port>,
    pub attributes: Vec<Attribute>,
    /// `None` denotes `hw.module.extern`; a defined module can have an empty body.
    pub body: Option<Vec<Operation>>,
    pub location: Option<Location>,
    pub position: LexerPosition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortDirection {
    Input,
    Output,
    InOut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Port {
    pub direction: PortDirection,
    /// Input/inout names include `%`; output names are bare identifiers or strings.
    pub name: Name,
    pub ty: Type,
    pub attributes: Vec<Attribute>,
    pub location: Option<Location>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signedness {
    Signless,
    Signed,
    Unsigned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Integer {
        width: u32,
        signedness: Signedness,
    },
    Array {
        size: u64,
        element: Box<Type>,
    },
    Struct(Vec<TypeField>),
    Union(Vec<UnionField>),
    InOut(Box<Type>),
    Clock,
    /// A reference to a declared type alias, such as `!word`.
    Alias(Name),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeField {
    pub name: Name,
    pub ty: Type,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnionField {
    pub name: Name,
    pub ty: Type,
    /// Omitted offsets are zero; retain explicit zero offsets for round trips.
    pub offset: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub name: Name,
    pub value: AttributeValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttributeValue {
    Unit,
    Boolean(bool),
    Integer {
        value: IntegerLiteral,
        ty: Option<Type>,
    },
    String(StringLiteral),
    Array(Vec<AttributeValue>),
    Dictionary(Vec<Attribute>),
    Type(Type),
    Symbol(Name),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub name: Name,
    /// `%results#1` selects a result from a grouped binding.
    pub result_index: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultBinding {
    pub name: Name,
    /// `%results:2` binds two results; an ordinary binding has count one.
    pub count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operation {
    pub results: Vec<ResultBinding>,
    pub kind: OperationKind,
    pub attributes: Vec<Attribute>,
    pub location: Option<Location>,
    pub position: LexerPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstantValue {
    Boolean(bool),
    Integer(IntegerLiteral),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceInput {
    pub name: Name,
    pub value: Value,
    pub ty: Type,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombOperator {
    Add,
    Sub,
    Mul,
    DivSigned,
    DivUnsigned,
    ModSigned,
    ModUnsigned,
    And,
    Or,
    Xor,
    ShiftLeft,
    ShiftRightSigned,
    ShiftRightUnsigned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonPredicate {
    Equal,
    NotEqual,
    SignedLess,
    SignedLessEqual,
    SignedGreater,
    SignedGreaterEqual,
    UnsignedLess,
    UnsignedLessEqual,
    UnsignedGreater,
    UnsignedGreaterEqual,
    CaseEqual,
    CaseNotEqual,
    WildcardEqual,
    WildcardNotEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetKind {
    Sync,
    Async,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reset {
    pub kind: ResetKind,
    pub signal: Value,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyKind {
    Assert,
    Assume,
    Cover,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockEdge {
    Positive,
    Negative,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Constant {
        value: ConstantValue,
        ty: Type,
    },
    AggregateConstant {
        fields: Vec<AttributeValue>,
        ty: Type,
    },
    Output {
        values: Vec<Value>,
        types: Vec<Type>,
    },
    Instance {
        name: StringLiteral,
        module: Name,
        inputs: Vec<InstanceInput>,
        outputs: Vec<TypeField>,
    },
    Bitcast {
        input: Value,
        input_type: Type,
        result_type: Type,
    },
    UnionExtract {
        input: Value,
        field: StringLiteral,
        union_type: Type,
    },
    ArrayGet {
        array: Value,
        index: Value,
        array_type: Type,
        index_type: Type,
    },
    ArrayInject {
        array: Value,
        index: Value,
        value: Value,
        array_type: Type,
        index_type: Type,
    },
    Comb {
        operator: CombOperator,
        two_state: bool,
        operands: Vec<Value>,
        ty: Type,
    },
    Compare {
        predicate: ComparisonPredicate,
        two_state: bool,
        lhs: Value,
        rhs: Value,
        ty: Type,
    },
    Mux {
        two_state: bool,
        condition: Value,
        true_value: Value,
        false_value: Value,
        ty: Type,
    },
    Concat {
        operands: Vec<Value>,
        types: Vec<Type>,
    },
    Replicate {
        input: Value,
        input_type: Type,
        result_type: Type,
    },
    Extract {
        input: Value,
        offset: u64,
        input_type: Type,
        result_type: Type,
    },
    ToClock {
        input: Value,
    },
    FirReg {
        input: Value,
        clock: Value,
        symbol: Option<Name>,
        reset: Option<Reset>,
        preset: Option<IntegerLiteral>,
        ty: Type,
    },
    CompReg {
        name: Option<StringLiteral>,
        input: Value,
        clock: Value,
        reset: Option<Reset>,
        ty: Type,
    },
    ClockedProperty {
        kind: PropertyKind,
        property: Value,
        enable: Option<Value>,
        edge: ClockEdge,
        clock: Value,
        label: Option<StringLiteral>,
        ty: Type,
    },
}
