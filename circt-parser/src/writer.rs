//! Serialize AST nodes to custom assembly and positioned syntax tokens.
//!
//! Formatting is canonical; names, integer/string literals, and debug locations
//! retain their spelling. Generated token positions refer to the written source,
//! rather than the input from which the AST was parsed.
//!
//! ```
//! use parser_circt::{parser::parse, writer::{write, WriterNode}};
//! let ast = parse(0, "module { hw.module @empty() { hw.output } }")?;
//! let output = write(1, &ast)?;
//! let reparsed = parse(1, output.source())?;
//! assert_eq!(ast.to_vec(1)?, reparsed.to_vec(1)?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod error;

use std::str::from_utf8;

use bytes::Bytes;

use crate::{
    ast::{
        Attribute, AttributeValue, ClockEdge, CombOperator, ComparisonPredicate, ConstantValue,
        File, HwModule, InstanceInput, IntegerLiteral, Item, Location, Module, Name, Operation,
        OperationKind, Port, PortDirection, PropertyKind, Reset, ResetKind, ResultBinding,
        Signedness, StringLiteral, Type, TypeField, UnionField, Value, Visibility,
    },
    lexer::{
        Lexer,
        position::LexerPosition,
        symbols::LexerSymbol,
        token::{LexerToken, LexerTokenKind},
    },
    writer::error::{WriterError, WriterResult},
};

pub trait WriterNode {
    fn write(&self, writer: &mut Writer) -> WriterResult<()>;

    fn to_vec(&self, file_id: usize) -> WriterResult<Vec<LexerToken>> {
        Ok(write(file_id, self)?.into_tokens())
    }

    fn to_source(&self) -> WriterResult<String> {
        Ok(write(0, self)?.into_source())
    }
}

/// Collects generated tokens and matching source text. All tokens are syntax
/// tokens; spacing and indentation live only in `source()`.
pub struct Writer {
    file_id: usize,
    source: String,
    tokens: Vec<LexerToken>,
    line: usize,
    column: usize,
    previous_cr: bool,
    indent: usize,
}

pub fn write(file_id: usize, node: &(impl WriterNode + ?Sized)) -> WriterResult<Writer> {
    let mut writer = Writer::new(file_id);
    node.write(&mut writer)?;
    Ok(writer)
}

impl Writer {
    pub fn new(file_id: usize) -> Self {
        Self {
            file_id,
            source: String::new(),
            tokens: Vec::new(),
            line: 1,
            column: 1,
            previous_cr: false,
            indent: 0,
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn tokens(&self) -> &[LexerToken] {
        &self.tokens
    }
    pub fn into_source(self) -> String {
        self.source
    }
    pub fn into_tokens(self) -> Vec<LexerToken> {
        self.tokens
    }

    fn append(&mut self, text: &str) {
        self.source.push_str(text);
        for byte in text.bytes() {
            match byte {
                b'\r' => {
                    self.line += 1;
                    self.column = 1;
                }
                b'\n' => {
                    if !self.previous_cr {
                        self.line += 1;
                    }
                    self.column = 1;
                }
                _ => self.column += 1,
            }
            self.previous_cr = byte == b'\r';
        }
    }

    fn begin(&mut self) -> LexerPosition {
        if self.column == 1 {
            self.append(&"  ".repeat(self.indent));
        } else if !self.source.ends_with(char::is_whitespace) {
            self.append(" ");
        }
        LexerPosition {
            file_id: self.file_id,
            index: self.source.len(),
            line: self.line,
            column: self.column,
            length: 0,
        }
    }

    fn push(&mut self, kind: LexerTokenKind, text: &str) {
        let mut position = self.begin();
        position.length = text.len();
        self.tokens.push(LexerToken {
            kind,
            text: Bytes::copy_from_slice(text.as_bytes()),
            position,
        });
        self.append(text);
    }

    fn keyword(&mut self, text: &str) {
        self.push(LexerTokenKind::Identifier, text);
    }
    fn integer(&mut self, value: impl ToString) {
        self.push(LexerTokenKind::Integer, &value.to_string());
    }

    fn symbol(&mut self, text: &str) {
        let symbol = LexerSymbol::try_from(text).expect("writer punctuation is a known symbol");
        self.push(LexerTokenKind::Symbol(symbol), text);
    }

    fn newline(&mut self) {
        if !self.source.ends_with('\n') {
            self.append("\n");
        }
    }

    // Compound type spellings, opaque metadata, and retained literal/name
    // spellings are lexed to respect MLIR token boundaries. Other grammar
    // punctuation, keywords, and generated integers are emitted directly.
    fn spelling(&mut self, bytes: &[u8]) -> WriterResult<()> {
        let text = from_utf8(bytes)?;
        let mut lexer = Lexer::new(self.file_id, text);
        let mut tokens = Vec::new();
        while let Some(token) = lexer.next_syntax()? {
            tokens.push(token);
        }
        let start = self.begin();
        for mut token in tokens {
            token.position.index += start.index;
            if token.position.line == 1 {
                token.position.column += start.column - 1;
            }
            token.position.line += start.line - 1;
            self.tokens.push(token);
        }
        self.append(text);
        Ok(())
    }

    fn list<T: WriterNode>(&mut self, nodes: &[T]) -> WriterResult<()> {
        for (index, node) in nodes.into_iter().enumerate() {
            if index > 0 {
                self.symbol(",");
            }
            node.write(self)?;
        }
        Ok(())
    }

    fn group<T: WriterNode>(&mut self, open: &str, nodes: &[T], close: &str) -> WriterResult<()> {
        self.symbol(open);
        self.list(nodes)?;
        self.symbol(close);
        Ok(())
    }

    fn attributes(&mut self, attributes: &[Attribute]) -> WriterResult<()> {
        if !attributes.is_empty() {
            self.group("{", attributes, "}")?;
        }
        Ok(())
    }

    fn declared_attributes(&mut self, attributes: &[Attribute]) -> WriterResult<()> {
        if !attributes.is_empty() {
            self.keyword("attributes");
            self.attributes(attributes)?;
        }
        Ok(())
    }

    fn location(&mut self, location: &Option<Location>) -> WriterResult<()> {
        if let Some(location) = location {
            location.write(self)?;
        }
        Ok(())
    }

    fn binary(&mut self, two_state: bool) {
        if two_state {
            self.keyword("bin");
        }
    }

    fn ty(&mut self, ty: &Type) -> WriterResult<()> {
        self.symbol(":");
        ty.write(self)
    }
}

impl WriterNode for Name {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.spelling(&self.spelling)
    }
}

impl WriterNode for IntegerLiteral {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.spelling(&self.spelling)
    }
}

impl WriterNode for StringLiteral {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.spelling(&self.spelling)
    }
}

impl WriterNode for Location {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.spelling(&self.spelling)
    }
}

impl WriterNode for File {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        for item in &self.items {
            item.write(writer)?;
            writer.newline();
        }
        Ok(())
    }
}

impl WriterNode for Item {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        match self {
            Self::Module(module) => module.write(writer),
            Self::HwModule(module) => module.write(writer),
        }
    }
}

impl WriterNode for Module {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.keyword("module");
        if let Some(name) = &self.name {
            name.write(writer)?;
        }
        writer.declared_attributes(&self.attributes)?;
        writer.symbol("{");
        writer.newline();
        writer.indent += 1;
        for item in &self.items {
            item.write(writer)?;
            writer.newline();
        }
        writer.indent -= 1;
        writer.symbol("}");
        writer.location(&self.location)
    }
}

impl WriterNode for HwModule {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.keyword(if self.body.is_some() {
            "hw.module"
        } else {
            "hw.module.extern"
        });
        self.visibility.write(writer)?;
        self.name.write(writer)?;
        writer.group("(", &self.ports, ")")?;
        writer.declared_attributes(&self.attributes)?;
        if let Some(body) = &self.body {
            writer.symbol("{");
            writer.newline();
            writer.indent += 1;
            for operation in body {
                operation.write(writer)?;
                writer.newline();
            }
            writer.indent -= 1;
            writer.symbol("}");
        }
        writer.location(&self.location)
    }
}

impl WriterNode for Visibility {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        match self {
            Self::Public => {}
            Self::Private => writer.keyword("private"),
            Self::Nested => writer.keyword("nested"),
        }
        Ok(())
    }
}

impl WriterNode for Port {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.keyword(match self.direction {
            PortDirection::Input => "in",
            PortDirection::Output => "out",
            PortDirection::InOut => "inout",
        });
        self.name.write(writer)?;
        writer.ty(&self.ty)?;
        writer.attributes(&self.attributes)?;
        writer.location(&self.location)
    }
}

impl Type {
    fn assembly(&self, abbreviated: bool) -> WriterResult<String> {
        let prefix = if abbreviated { "" } else { "!hw." };
        Ok(match self {
            Self::Integer { width, signedness } => {
                let prefix = match signedness {
                    Signedness::Signless => "i",
                    Signedness::Signed => "si",
                    Signedness::Unsigned => "ui",
                };
                format!("{prefix}{width}")
            }
            Self::Array { size, element } => {
                format!("{prefix}array<{size}x{}>", element.assembly(true)?)
            }
            Self::Struct(fields) => {
                let mut source = format!("{prefix}struct<");
                for (index, field) in fields.into_iter().enumerate() {
                    if index > 0 {
                        source.push_str(", ");
                    }
                    source.push_str(from_utf8(&field.name.spelling)?);
                    source.push_str(": ");
                    source.push_str(&field.ty.assembly(false)?);
                }
                source.push('>');
                source
            }
            Self::Union(fields) => {
                let mut source = format!("{prefix}union<");
                for (index, field) in fields.into_iter().enumerate() {
                    if index > 0 {
                        source.push_str(", ");
                    }
                    source.push_str(from_utf8(&field.name.spelling)?);
                    source.push_str(": ");
                    source.push_str(&field.ty.assembly(false)?);
                    if let Some(offset) = field.offset {
                        source.push_str(&format!(" offset {offset}"));
                    }
                }
                source.push('>');
                source
            }
            Self::InOut(element) => {
                format!("{prefix}inout<{}>", element.assembly(false)?)
            }
            Self::Clock => "!seq.clock".into(),
            Self::Alias(name) => from_utf8(&name.spelling)?.into(),
        })
    }
}

impl WriterNode for Type {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.spelling(self.assembly(false)?.as_bytes())
    }
}

impl WriterNode for TypeField {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        self.name.write(writer)?;
        writer.ty(&self.ty)
    }
}

impl WriterNode for UnionField {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        self.name.write(writer)?;
        writer.ty(&self.ty)?;
        if let Some(offset) = self.offset {
            writer.keyword("offset");
            writer.integer(offset);
        }
        Ok(())
    }
}

impl WriterNode for Attribute {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        self.name.write(writer)?;
        if self.value != AttributeValue::Unit {
            writer.symbol("=");
            self.value.write(writer)?;
        }
        Ok(())
    }
}

impl WriterNode for AttributeValue {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        match self {
            Self::Unit => {}
            Self::Boolean(value) => writer.keyword(if *value { "true" } else { "false" }),
            Self::Integer { value, ty } => {
                value.write(writer)?;
                if let Some(ty) = ty {
                    writer.ty(ty)?;
                }
            }
            Self::String(value) => value.write(writer)?,
            Self::Array(values) => writer.group("[", values, "]")?,
            Self::Dictionary(attributes) => writer.group("{", attributes, "}")?,
            Self::Type(ty) => ty.write(writer)?,
            Self::Symbol(name) => name.write(writer)?,
        }
        Ok(())
    }
}

impl WriterNode for Value {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        if let Some(index) = self.result_index {
            let name = from_utf8(&self.name.spelling)?;
            writer.spelling(format!("{name}#{index}").as_bytes())
        } else {
            self.name.write(writer)
        }
    }
}

impl WriterNode for ResultBinding {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        self.name.write(writer)?;
        if self.count != 1 {
            writer.symbol(":");
            writer.integer(self.count);
        }
        Ok(())
    }
}

impl WriterNode for InstanceInput {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        self.name.write(writer)?;
        writer.symbol(":");
        self.value.write(writer)?;
        writer.ty(&self.ty)
    }
}

impl Reset {
    fn write_operands(&self, writer: &mut Writer) -> WriterResult<()> {
        self.signal.write(writer)?;
        writer.symbol(",");
        self.value.write(writer)
    }
}

impl WriterNode for Reset {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.keyword("reset");
        writer.keyword(match self.kind {
            ResetKind::Sync => "sync",
            ResetKind::Async => "async",
        });
        self.write_operands(writer)
    }
}

impl WriterNode for Operation {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        if !self.results.is_empty() {
            writer.list(&self.results)?;
            writer.symbol("=");
        }
        match &self.kind {
            OperationKind::Constant { value, ty } => {
                writer.keyword("hw.constant");
                match value {
                    ConstantValue::Boolean(value) => {
                        if *ty
                            != (Type::Integer {
                                width: 1,
                                signedness: Signedness::Signless,
                            })
                        {
                            return Err(WriterError::InvalidAst(
                                "boolean constant must have type i1",
                            ));
                        }
                        writer.keyword(if *value { "true" } else { "false" })
                    }
                    ConstantValue::Integer(value) => {
                        value.write(writer)?;
                        writer.ty(ty)?;
                    }
                }
            }
            OperationKind::AggregateConstant { fields, .. } => {
                writer.keyword("hw.aggregate_constant");
                writer.group("[", fields, "]")?;
            }
            OperationKind::Output { values, .. } => {
                writer.keyword("hw.output");
                writer.list(values)?;
            }
            OperationKind::Instance {
                name,
                module,
                inputs,
                outputs,
            } => {
                writer.keyword("hw.instance");
                name.write(writer)?;
                module.write(writer)?;
                writer.group("(", inputs, ")")?;
                writer.symbol("->");
                writer.group("(", outputs, ")")?;
            }
            OperationKind::ArrayGet { array, index, .. }
            | OperationKind::ArrayInject { array, index, .. } => {
                writer.keyword(if let OperationKind::ArrayGet { .. } = &self.kind {
                    "hw.array_get"
                } else {
                    "hw.array_inject"
                });
                array.write(writer)?;
                writer.symbol("[");
                index.write(writer)?;
                writer.symbol("]");
                if let OperationKind::ArrayInject { value, .. } = &self.kind {
                    writer.symbol(",");
                    value.write(writer)?;
                }
            }
            OperationKind::Comb {
                operator,
                two_state,
                operands,
                ..
            } => {
                operator.write(writer)?;
                writer.binary(*two_state);
                writer.list(operands)?;
            }
            OperationKind::Compare {
                predicate,
                two_state,
                lhs,
                rhs,
                ..
            } => {
                writer.keyword("comb.icmp");
                writer.binary(*two_state);
                predicate.write(writer)?;
                lhs.write(writer)?;
                writer.symbol(",");
                rhs.write(writer)?;
            }
            OperationKind::Mux {
                two_state,
                condition,
                true_value,
                false_value,
                ..
            } => {
                writer.keyword("comb.mux");
                writer.binary(*two_state);
                condition.write(writer)?;
                writer.symbol(",");
                true_value.write(writer)?;
                writer.symbol(",");
                false_value.write(writer)?;
            }
            OperationKind::Concat { operands, .. } => {
                writer.keyword("comb.concat");
                writer.list(operands)?;
            }
            OperationKind::UnionExtract { input, field, .. } => {
                writer.keyword("hw.union_extract");
                input.write(writer)?;
                writer.symbol("[");
                field.write(writer)?;
                writer.symbol("]");
            }
            OperationKind::Bitcast { input, .. } => {
                writer.keyword("hw.bitcast");
                input.write(writer)?;
            }
            OperationKind::Replicate { input, .. } => {
                writer.keyword("comb.replicate");
                input.write(writer)?;
            }
            OperationKind::Extract { input, offset, .. } => {
                writer.keyword("comb.extract");
                input.write(writer)?;
                writer.keyword("from");
                writer.integer(offset);
            }
            OperationKind::ToClock { input } => {
                writer.keyword("seq.to_clock");
                input.write(writer)?;
            }
            OperationKind::FirReg {
                input,
                clock,
                symbol,
                reset,
                preset,
                ..
            } => {
                writer.keyword("seq.firreg");
                input.write(writer)?;
                writer.keyword("clock");
                clock.write(writer)?;
                if let Some(symbol) = symbol {
                    writer.keyword("sym");
                    symbol.write(writer)?;
                }
                if let Some(reset) = reset {
                    reset.write(writer)?;
                }
                if let Some(preset) = preset {
                    writer.keyword("preset");
                    preset.write(writer)?;
                }
            }
            OperationKind::CompReg {
                name,
                input,
                clock,
                reset,
                ..
            } => {
                writer.keyword("seq.compreg");
                if let Some(name) = name {
                    name.write(writer)?;
                }
                input.write(writer)?;
                writer.symbol(",");
                clock.write(writer)?;
                if let Some(reset) = reset {
                    if reset.kind != ResetKind::Sync {
                        return Err(WriterError::InvalidAst(
                            "seq.compreg reset must be synchronous",
                        ));
                    }
                    writer.keyword("reset");
                    reset.write_operands(writer)?;
                }
            }
            OperationKind::ClockedProperty {
                kind,
                property,
                enable,
                edge,
                clock,
                label,
                ..
            } => {
                writer.keyword(match kind {
                    PropertyKind::Assert => "verif.clocked_assert",
                    PropertyKind::Assume => "verif.clocked_assume",
                    PropertyKind::Cover => "verif.clocked_cover",
                });
                property.write(writer)?;
                if let Some(enable) = enable {
                    writer.keyword("if");
                    enable.write(writer)?;
                }
                writer.symbol(",");
                writer.keyword(match edge {
                    ClockEdge::Positive => "posedge",
                    ClockEdge::Negative => "negedge",
                });
                clock.write(writer)?;
                if let Some(label) = label {
                    writer.keyword("label");
                    label.write(writer)?;
                }
            }
        }
        writer.attributes(&self.attributes)?;
        match &self.kind {
            OperationKind::AggregateConstant { ty, .. }
            | OperationKind::Comb { ty, .. }
            | OperationKind::Compare { ty, .. }
            | OperationKind::Mux { ty, .. }
            | OperationKind::FirReg { ty, .. }
            | OperationKind::CompReg { ty, .. }
            | OperationKind::ClockedProperty { ty, .. } => writer.ty(ty)?,
            OperationKind::UnionExtract { union_type, .. } => writer.ty(union_type)?,
            OperationKind::Output { types, .. } => {
                if !types.is_empty() {
                    writer.symbol(":");
                    writer.list(types)?;
                }
            }
            OperationKind::Concat { types, .. } => {
                writer.symbol(":");
                writer.list(types)?;
            }
            OperationKind::ArrayGet {
                array_type,
                index_type,
                ..
            }
            | OperationKind::ArrayInject {
                array_type,
                index_type,
                ..
            } => {
                writer.ty(array_type)?;
                writer.symbol(",");
                index_type.write(writer)?;
            }
            OperationKind::Bitcast {
                input_type,
                result_type,
                ..
            }
            | OperationKind::Replicate {
                input_type,
                result_type,
                ..
            }
            | OperationKind::Extract {
                input_type,
                result_type,
                ..
            } => {
                writer.symbol(":");
                writer.symbol("(");
                input_type.write(writer)?;
                writer.symbol(")");
                writer.symbol("->");
                result_type.write(writer)?;
            }
            OperationKind::Constant { .. }
            | OperationKind::Instance { .. }
            | OperationKind::ToClock { .. } => {}
        }
        writer.location(&self.location)
    }
}

impl WriterNode for CombOperator {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.keyword(match self {
            Self::Add => "comb.add",
            Self::Sub => "comb.sub",
            Self::Mul => "comb.mul",
            Self::DivSigned => "comb.divs",
            Self::DivUnsigned => "comb.divu",
            Self::ModSigned => "comb.mods",
            Self::ModUnsigned => "comb.modu",
            Self::And => "comb.and",
            Self::Or => "comb.or",
            Self::Xor => "comb.xor",
            Self::ShiftLeft => "comb.shl",
            Self::ShiftRightSigned => "comb.shrs",
            Self::ShiftRightUnsigned => "comb.shru",
        });
        Ok(())
    }
}

impl WriterNode for ComparisonPredicate {
    fn write(&self, writer: &mut Writer) -> WriterResult<()> {
        writer.keyword(match self {
            Self::Equal => "eq",
            Self::NotEqual => "ne",
            Self::SignedLess => "slt",
            Self::SignedLessEqual => "sle",
            Self::SignedGreater => "sgt",
            Self::SignedGreaterEqual => "sge",
            Self::UnsignedLess => "ult",
            Self::UnsignedLessEqual => "ule",
            Self::UnsignedGreater => "ugt",
            Self::UnsignedGreaterEqual => "uge",
            Self::CaseEqual => "ceq",
            Self::CaseNotEqual => "cne",
            Self::WildcardEqual => "weq",
            Self::WildcardNotEqual => "wne",
        });
        Ok(())
    }
}
