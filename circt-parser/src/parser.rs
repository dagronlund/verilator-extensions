//! Recursive-descent parser for custom HW assembly and the comb/seq/verif
//! operations emitted by `--lower-llhd-formal-to-core`.
//!
//! Nodes implement `ParserNode` and consume a reversed token vector
//! through shared required/optional token and group helpers. Parsing is driven
//! by grammar, never by line boundaries. Generic operation assembly and dialect
//! constructs outside this subset return positioned errors.
//!
//! ```
//! let file = parser_circt::parser::parse(0, "module { hw.module @empty() { hw.output } }")?;
//! assert_eq!(file.items.len(), 1);
//! # Ok::<(), parser_circt::parser::error::ParserError>(())
//! ```

pub mod error;
mod util;

use std::str::FromStr;

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
    parser::{
        error::{ParserError, ParserResult},
        util::{comb_operator, supported_operation},
    },
};

pub trait ParserNode: Sized {
    fn parse(parser: &mut Parser) -> ParserResult<Self>;
}

/// Owns the token stream and source, so both parser and AST outlive the input.
pub struct Parser {
    source: Bytes,
    tokens: Vec<LexerToken>,
    eof: LexerPosition,
    last_end: usize,
}

pub fn parse(file_id: usize, source: &str) -> ParserResult<File> {
    Parser::new(file_id, source)?.parse()
}

impl Parser {
    pub fn new(file_id: usize, source: &str) -> ParserResult<Self> {
        let mut tokens = Vec::new();
        let mut lexer = Lexer::new(file_id, source);
        while let Some(token) = lexer.next_syntax()? {
            tokens.push(token);
        }
        Self::from_tokens(file_id, source, tokens)
    }

    /// Parse an existing complete lexer/writer token stream over `source`.
    /// Spans must refer to this source; trivia tokens are accepted and removed.
    pub fn from_tokens(
        file_id: usize,
        source: &str,
        mut tokens: Vec<LexerToken>,
    ) -> ParserResult<Self> {
        let mut previous_end = 0;
        for token in &tokens {
            let end = token.position.index.checked_add(token.position.length);
            let text = end.and_then(|end| source.get(token.position.index..end));
            if token.position.file_id != file_id
                || token.position.index < previous_end
                || token.text.is_empty()
                || text.map(str::as_bytes) != Some(token.text.as_ref())
            {
                return Err(ParserError::Syntax {
                    message: "token span/spelling does not match parser source".into(),
                    position: token.position,
                });
            }
            previous_end = end.expect("validated token end");
        }
        tokens.retain(|token| !token.kind.is_whitespace());
        tokens.reverse();
        let mut line = 1;
        let mut column = 1;
        let mut previous_cr = false;
        for byte in source.bytes() {
            match byte {
                b'\r' => {
                    line += 1;
                    column = 1;
                }
                b'\n' => {
                    if !previous_cr {
                        line += 1;
                    }
                    column = 1;
                }
                _ => column += 1,
            }
            previous_cr = byte == b'\r';
        }
        Ok(Self {
            source: Bytes::copy_from_slice(source.as_bytes()),
            tokens,
            eof: LexerPosition {
                file_id,
                index: source.len(),
                line,
                column,
                length: 0,
            },
            last_end: 0,
        })
    }

    pub fn parse(mut self) -> ParserResult<File> {
        File::parse(&mut self)
    }

    fn peek(&self) -> Option<&LexerToken> {
        self.tokens.last()
    }

    fn at(&self, spelling: &str) -> bool {
        self.peek()
            .is_some_and(|token| token.text.as_ref() == spelling.as_bytes())
    }

    fn at_kind(&self, kind: LexerTokenKind) -> bool {
        self.peek().is_some_and(|token| token.kind == kind)
    }

    fn position(&self) -> LexerPosition {
        self.peek().map_or(self.eof, |token| token.position)
    }

    fn error(&self, message: impl Into<String>) -> ParserError {
        ParserError::Syntax {
            message: message.into(),
            position: self.position(),
        }
    }

    fn required(&mut self) -> ParserResult<LexerToken> {
        let token = self
            .tokens
            .pop()
            .ok_or_else(|| self.error("unexpected end of input"))?;
        self.last_end = token.position.index + token.position.length;
        Ok(token)
    }

    fn optional(&mut self, spelling: &str) -> bool {
        if self.at(spelling) {
            // `at` proves there is a token.
            self.required().expect("peeked token exists");
            true
        } else {
            false
        }
    }

    fn expect(&mut self, spelling: &str) -> ParserResult<()> {
        if !self.optional(spelling) {
            return Err(self.error(format!("expected `{spelling}`")));
        }
        Ok(())
    }

    fn token(&mut self, kind: LexerTokenKind, description: &str) -> ParserResult<LexerToken> {
        if !self.at_kind(kind) {
            return Err(self.error(format!("expected {description}")));
        }
        self.required()
    }

    fn name(&mut self, kind: LexerTokenKind) -> ParserResult<Name> {
        let token = self.token(kind, &format!("{kind:?}"))?;
        Ok(Name {
            spelling: token.text,
            position: token.position,
        })
    }

    fn field_name(&mut self) -> ParserResult<Name> {
        if self.at_kind(LexerTokenKind::String) {
            self.name(LexerTokenKind::String)
        } else {
            self.name(LexerTokenKind::Identifier)
        }
    }

    fn span(&self, start: LexerPosition) -> LexerPosition {
        LexerPosition {
            length: self.last_end - start.index,
            ..start
        }
    }

    fn group<T>(
        &mut self,
        open: &str,
        close: &str,
        mut parse: impl FnMut(&mut Self) -> ParserResult<T>,
    ) -> ParserResult<Vec<T>> {
        self.expect(open)?;
        let mut items = Vec::new();
        if self.optional(close) {
            return Ok(items);
        }
        loop {
            items.push(parse(self)?);
            if self.optional(close) {
                break;
            }
            self.expect(",")?;
        }
        Ok(items)
    }

    fn separated<T: ParserNode>(&mut self) -> ParserResult<Vec<T>> {
        let mut values = vec![T::parse(self)?];
        while self.optional(",") {
            values.push(T::parse(self)?);
        }
        Ok(values)
    }

    fn attributes(&mut self) -> ParserResult<Vec<Attribute>> {
        if self.at("{") {
            self.group("{", "}", Attribute::parse)
        } else {
            Ok(Vec::new())
        }
    }

    fn typed(&mut self, attributes: &mut Vec<Attribute>) -> ParserResult<Type> {
        *attributes = self.attributes()?;
        self.expect(":")?;
        Type::parse(self)
    }

    fn location(&mut self) -> ParserResult<Option<Location>> {
        if !self.at("loc") {
            return Ok(None);
        }
        let start = self.required()?.position;
        self.expect("(")?;
        let mut closes = vec![LexerSymbol::CloseParen];
        while !closes.is_empty() {
            let token = self.required()?;
            if let LexerTokenKind::Symbol(symbol) = token.kind {
                let close = match symbol {
                    LexerSymbol::OpenParen => Some(LexerSymbol::CloseParen),
                    LexerSymbol::OpenSquare => Some(LexerSymbol::CloseSquare),
                    LexerSymbol::OpenBrace => Some(LexerSymbol::CloseBrace),
                    LexerSymbol::Less => Some(LexerSymbol::Greater),
                    LexerSymbol::CloseParen
                    | LexerSymbol::CloseSquare
                    | LexerSymbol::CloseBrace
                    | LexerSymbol::Greater => {
                        if closes.pop() != Some(symbol) {
                            return Err(ParserError::Syntax {
                                message: "mismatched location delimiter".into(),
                                position: token.position,
                            });
                        }
                        None
                    }
                    _ => None,
                };
                if let Some(close) = close {
                    closes.push(close);
                }
            }
        }
        let position = self.span(start);
        Ok(Some(Location {
            spelling: self.source.slice(position.range()),
            position,
        }))
    }

    fn item(&mut self) -> ParserResult<Item> {
        if self.at("module") || self.at("builtin.module") {
            Ok(Item::Module(Module::parse(self)?))
        } else if self.at("hw.module") || self.at("hw.module.extern") {
            Ok(Item::HwModule(HwModule::parse(self)?))
        } else {
            Err(self.error("expected `module`, `hw.module`, or `hw.module.extern`"))
        }
    }

    fn parse_type(&mut self, abbreviated: bool) -> ParserResult<Type> {
        let token = self.required()?;
        let name = String::from_utf8_lossy(&token.text);
        let integer = if let Some(width) = name.strip_prefix("si") {
            Some((width, Signedness::Signed))
        } else if let Some(width) = name.strip_prefix("ui") {
            Some((width, Signedness::Unsigned))
        } else {
            name.strip_prefix('i')
                .map(|width| (width, Signedness::Signless))
        };
        if token.kind == LexerTokenKind::Identifier
            && let Some((width, signedness)) = integer
            && !width.is_empty()
            && width.bytes().all(|b| b.is_ascii_digit())
        {
            let width = width.parse::<u32>().map_err(|_| ParserError::Syntax {
                message: "integer width exceeds u32".into(),
                position: token.position,
            })?;
            if width == 0 {
                return Err(ParserError::Syntax {
                    message: "integer width must be positive".into(),
                    position: token.position,
                });
            }
            return Ok(Type::Integer { width, signedness });
        }
        match name.as_ref() {
            "!hw.array" | "array" if name == "!hw.array" || abbreviated => {
                self.expect("<")?;
                let size = self.unsigned::<u64>("array size")?;
                // The lexer sees `4xi8` as `4`, `xi8`, and `2xarray` as
                // `2`, `xarray`. Split only this grammar-specific separator.
                if !self.at_kind(LexerTokenKind::Identifier)
                    || !self.peek().is_some_and(|t| t.text.starts_with(b"x"))
                {
                    return Err(self.error("expected `x` after array size"));
                }
                let mut separator = self.required()?;
                if separator.text.len() > 1 {
                    separator.text = separator.text.slice(1..);
                    separator.position.index += 1;
                    separator.position.column += 1;
                    separator.position.length -= 1;
                    self.tokens.push(separator);
                }
                let element = self.parse_type(true)?;
                self.expect(">")?;
                Ok(Type::Array {
                    size,
                    element: Box::new(element),
                })
            }
            "!hw.struct" | "struct" if name == "!hw.struct" || abbreviated => Ok(Type::Struct(
                self.group("<", ">", |p| p.type_field(abbreviated))?,
            )),
            "!hw.union" | "union" if name == "!hw.union" || abbreviated => {
                Ok(Type::Union(self.group("<", ">", UnionField::parse)?))
            }
            "!hw.inout" | "inout" if name == "!hw.inout" || abbreviated => {
                self.expect("<")?;
                let element = self.parse_type(abbreviated)?;
                self.expect(">")?;
                Ok(Type::InOut(Box::new(element)))
            }
            "!seq.clock" => Ok(Type::Clock),
            _ if token.kind == LexerTokenKind::TypeIdentifier && !name.contains('.') => {
                Ok(Type::Alias(Name {
                    spelling: token.text,
                    position: token.position,
                }))
            }
            _ => Err(ParserError::Syntax {
                message: format!("unsupported type `{name}`"),
                position: token.position,
            }),
        }
    }

    fn type_field(&mut self, abbreviated: bool) -> ParserResult<TypeField> {
        let name = self.field_name()?;
        self.expect(":")?;
        let ty = self.parse_type(abbreviated)?;
        Ok(TypeField { name, ty })
    }

    fn unsigned<T: FromStr>(&mut self, description: &str) -> ParserResult<T> {
        let token = self.token(LexerTokenKind::Integer, description)?;
        let text = String::from_utf8_lossy(&token.text);
        text.parse().map_err(|_| ParserError::Syntax {
            message: format!("expected {description} as an in-range unsigned decimal integer"),
            position: token.position,
        })
    }

    fn reset(&mut self, kind: ResetKind) -> ParserResult<Reset> {
        let signal = Value::parse(self)?;
        self.expect(",")?;
        let value = Value::parse(self)?;
        Ok(Reset {
            kind,
            signal,
            value,
        })
    }

    fn operation_kind(
        &mut self,
        name: &str,
        attributes: &mut Vec<Attribute>,
    ) -> ParserResult<OperationKind> {
        Ok(match name {
            "hw.constant" => {
                let value = if self.optional("true") {
                    ConstantValue::Boolean(true)
                } else if self.optional("false") {
                    ConstantValue::Boolean(false)
                } else {
                    ConstantValue::Integer(IntegerLiteral::parse(self)?)
                };
                let ty = if let ConstantValue::Boolean(_) = value {
                    Type::Integer {
                        width: 1,
                        signedness: Signedness::Signless,
                    }
                } else {
                    self.expect(":")?;
                    Type::parse(self)?
                };
                *attributes = self.attributes()?;
                OperationKind::Constant { value, ty }
            }
            "hw.aggregate_constant" => {
                let fields = self.group("[", "]", AttributeValue::parse)?;
                let ty = self.typed(attributes)?;
                OperationKind::AggregateConstant { fields, ty }
            }
            "hw.output" => {
                let values = if self.at_kind(LexerTokenKind::ValueIdentifier) {
                    self.separated::<Value>()?
                } else {
                    Vec::new()
                };
                *attributes = self.attributes()?;
                let types = if !values.is_empty() {
                    self.expect(":")?;
                    self.separated::<Type>()?
                } else {
                    Vec::new()
                };
                if values.len() != types.len() {
                    return Err(self.error("output operand/type count mismatch"));
                }
                OperationKind::Output { values, types }
            }
            "hw.instance" => {
                let name = StringLiteral::parse(self)?;
                let module = self.name(LexerTokenKind::SymbolIdentifier)?;
                let inputs = self.group("(", ")", InstanceInput::parse)?;
                self.expect("->")?;
                let outputs = self.group("(", ")", TypeField::parse)?;
                *attributes = self.attributes()?;
                OperationKind::Instance {
                    name,
                    module,
                    inputs,
                    outputs,
                }
            }
            "hw.array_get" | "hw.array_inject" => {
                let array = Value::parse(self)?;
                self.expect("[")?;
                let index = Value::parse(self)?;
                self.expect("]")?;
                let value = if name == "hw.array_inject" {
                    self.expect(",")?;
                    Some(Value::parse(self)?)
                } else {
                    None
                };
                let array_type = self.typed(attributes)?;
                self.expect(",")?;
                let index_type = Type::parse(self)?;
                if let Some(value) = value {
                    OperationKind::ArrayInject {
                        array,
                        index,
                        value,
                        array_type,
                        index_type,
                    }
                } else {
                    OperationKind::ArrayGet {
                        array,
                        index,
                        array_type,
                        index_type,
                    }
                }
            }
            "hw.union_extract" => {
                let input = Value::parse(self)?;
                self.expect("[")?;
                let field = StringLiteral::parse(self)?;
                self.expect("]")?;
                let union_type = self.typed(attributes)?;
                OperationKind::UnionExtract {
                    input,
                    field,
                    union_type,
                }
            }
            "comb.icmp" => {
                let two_state = self.optional("bin");
                let predicate = ComparisonPredicate::parse(self)?;
                let lhs = Value::parse(self)?;
                self.expect(",")?;
                let rhs = Value::parse(self)?;
                let ty = self.typed(attributes)?;
                OperationKind::Compare {
                    predicate,
                    two_state,
                    lhs,
                    rhs,
                    ty,
                }
            }
            "comb.mux" => {
                let two_state = self.optional("bin");
                let condition = Value::parse(self)?;
                self.expect(",")?;
                let true_value = Value::parse(self)?;
                self.expect(",")?;
                let false_value = Value::parse(self)?;
                let ty = self.typed(attributes)?;
                OperationKind::Mux {
                    two_state,
                    condition,
                    true_value,
                    false_value,
                    ty,
                }
            }
            "comb.concat" => {
                let operands = self.separated::<Value>()?;
                *attributes = self.attributes()?;
                self.expect(":")?;
                let types = self.separated::<Type>()?;
                if operands.len() != types.len() {
                    return Err(self.error("concat operand/type count mismatch"));
                }
                OperationKind::Concat { operands, types }
            }
            "hw.bitcast" => {
                let input = Value::parse(self)?;
                *attributes = self.attributes()?;
                self.expect(":")?;
                self.expect("(")?;
                let input_type = Type::parse(self)?;
                self.expect(")")?;
                self.expect("->")?;
                let result_type = Type::parse(self)?;
                OperationKind::Bitcast {
                    input,
                    input_type,
                    result_type,
                }
            }
            "comb.replicate" => {
                let input = Value::parse(self)?;
                *attributes = self.attributes()?;
                self.expect(":")?;
                self.expect("(")?;
                let input_type = Type::parse(self)?;
                self.expect(")")?;
                self.expect("->")?;
                let result_type = Type::parse(self)?;
                OperationKind::Replicate {
                    input,
                    input_type,
                    result_type,
                }
            }
            "comb.extract" => {
                let input = Value::parse(self)?;
                self.expect("from")?;
                let offset = self.unsigned::<u64>("extract offset")?;
                *attributes = self.attributes()?;
                self.expect(":")?;
                self.expect("(")?;
                let input_type = Type::parse(self)?;
                self.expect(")")?;
                self.expect("->")?;
                let result_type = Type::parse(self)?;
                OperationKind::Extract {
                    input,
                    offset,
                    input_type,
                    result_type,
                }
            }
            "seq.to_clock" => {
                let input = Value::parse(self)?;
                *attributes = self.attributes()?;
                OperationKind::ToClock { input }
            }
            "seq.firreg" => {
                let input = Value::parse(self)?;
                self.expect("clock")?;
                let clock = Value::parse(self)?;
                let symbol = if self.optional("sym") {
                    Some(self.name(LexerTokenKind::SymbolIdentifier)?)
                } else {
                    None
                };
                let reset = if self.optional("reset") {
                    let kind = if self.optional("async") {
                        ResetKind::Async
                    } else {
                        self.expect("sync")?;
                        ResetKind::Sync
                    };
                    Some(self.reset(kind)?)
                } else {
                    None
                };
                let preset = if self.optional("preset") {
                    if self.at("-") {
                        return Err(self.error("preset value must not be negative"));
                    }
                    Some(IntegerLiteral::parse(self)?)
                } else {
                    None
                };
                let ty = self.typed(attributes)?;
                OperationKind::FirReg {
                    input,
                    clock,
                    symbol,
                    reset,
                    preset,
                    ty,
                }
            }
            "seq.compreg" => {
                let name = if self.at_kind(LexerTokenKind::String) {
                    Some(StringLiteral::parse(self)?)
                } else {
                    None
                };
                let input = Value::parse(self)?;
                self.expect(",")?;
                let clock = Value::parse(self)?;
                let reset = if self.optional("reset") {
                    Some(self.reset(ResetKind::Sync)?)
                } else {
                    None
                };
                let ty = self.typed(attributes)?;
                OperationKind::CompReg {
                    name,
                    input,
                    clock,
                    reset,
                    ty,
                }
            }
            "verif.clocked_assert" | "verif.clocked_assume" | "verif.clocked_cover" => {
                let kind = match name {
                    "verif.clocked_assert" => PropertyKind::Assert,
                    "verif.clocked_assume" => PropertyKind::Assume,
                    _ => PropertyKind::Cover,
                };
                let property = Value::parse(self)?;
                let enable = if self.optional("if") {
                    Some(Value::parse(self)?)
                } else {
                    None
                };
                self.expect(",")?;
                let edge = if self.optional("posedge") {
                    ClockEdge::Positive
                } else {
                    self.expect("negedge")?;
                    ClockEdge::Negative
                };
                let clock = Value::parse(self)?;
                let label = if self.optional("label") {
                    Some(StringLiteral::parse(self)?)
                } else {
                    None
                };
                let ty = self.typed(attributes)?;
                OperationKind::ClockedProperty {
                    kind,
                    property,
                    enable,
                    edge,
                    clock,
                    label,
                    ty,
                }
            }
            _ => {
                let operator = comb_operator(name)
                    .ok_or_else(|| self.error(format!("unsupported operation `{name}`")))?;
                let two_state = self.optional("bin");
                let operands = self.separated::<Value>()?;
                let variadic = operator == CombOperator::Add
                    || operator == CombOperator::Mul
                    || operator == CombOperator::And
                    || operator == CombOperator::Or
                    || operator == CombOperator::Xor;
                if !variadic && operands.len() != 2 {
                    return Err(self.error("invalid comb operand count"));
                }
                let ty = self.typed(attributes)?;
                OperationKind::Comb {
                    operator,
                    two_state,
                    operands,
                    ty,
                }
            }
        })
    }
}

impl ParserNode for File {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let mut items = Vec::new();
        while parser.peek().is_some() {
            items.push(Item::parse(parser)?);
        }
        Ok(Self { items })
    }
}

impl ParserNode for Item {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        parser.item()
    }
}

impl ParserNode for Module {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let start = parser.position();
        if !parser.optional("builtin.module") {
            parser.expect("module")?;
        }
        let name = if parser.at_kind(LexerTokenKind::SymbolIdentifier) {
            Some(parser.name(LexerTokenKind::SymbolIdentifier)?)
        } else {
            None
        };
        let attributes = if parser.optional("attributes") {
            parser.group("{", "}", Attribute::parse)?
        } else {
            Vec::new()
        };
        parser.expect("{")?;
        let mut items = Vec::new();
        while !parser.optional("}") {
            items.push(Item::parse(parser)?);
        }
        let location = parser.location()?;
        Ok(Self {
            name,
            attributes,
            items,
            location,
            position: parser.span(start),
        })
    }
}

impl ParserNode for HwModule {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let start = parser.position();
        let external = parser.optional("hw.module.extern");
        if !external {
            parser.expect("hw.module")?;
        }
        let visibility = if parser.optional("private") {
            Visibility::Private
        } else if parser.optional("nested") {
            Visibility::Nested
        } else {
            parser.optional("public");
            Visibility::Public
        };
        let name = parser.name(LexerTokenKind::SymbolIdentifier)?;
        let ports = parser.group("(", ")", Port::parse)?;
        let attributes = if parser.optional("attributes") {
            parser.group("{", "}", Attribute::parse)?
        } else {
            Vec::new()
        };
        let body = if external {
            None
        } else {
            parser.expect("{")?;
            let mut operations = Vec::new();
            while !parser.optional("}") {
                operations.push(Operation::parse(parser)?);
            }
            Some(operations)
        };
        let location = parser.location()?;
        Ok(Self {
            name,
            visibility,
            ports,
            attributes,
            body,
            location,
            position: parser.span(start),
        })
    }
}

impl ParserNode for Port {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let direction = if parser.optional("in") {
            PortDirection::Input
        } else if parser.optional("out") {
            PortDirection::Output
        } else {
            parser.expect("inout")?;
            PortDirection::InOut
        };
        let name = if direction == PortDirection::Output {
            parser.field_name()?
        } else {
            parser.name(LexerTokenKind::ValueIdentifier)?
        };
        parser.expect(":")?;
        let ty = Type::parse(parser)?;
        let attributes = parser.attributes()?;
        let location = parser.location()?;
        Ok(Self {
            direction,
            name,
            ty,
            attributes,
            location,
        })
    }
}

impl ParserNode for Type {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        parser.parse_type(false)
    }
}

impl ParserNode for TypeField {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        parser.type_field(false)
    }
}

impl ParserNode for UnionField {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let name = parser.name(LexerTokenKind::Identifier)?;
        parser.expect(":")?;
        let ty = Type::parse(parser)?;
        let offset = if parser.optional("offset") {
            Some(parser.unsigned::<u64>("union field offset")?)
        } else {
            None
        };
        Ok(Self { name, ty, offset })
    }
}

impl ParserNode for Attribute {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let name = parser.field_name()?;
        let value = if parser.optional("=") {
            AttributeValue::parse(parser)?
        } else {
            AttributeValue::Unit
        };
        Ok(Self { name, value })
    }
}

impl ParserNode for AttributeValue {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        Ok(if parser.at_kind(LexerTokenKind::String) {
            Self::String(StringLiteral::parse(parser)?)
        } else if parser.optional("true") {
            Self::Boolean(true)
        } else if parser.optional("false") {
            Self::Boolean(false)
        } else if parser.at("[") {
            Self::Array(parser.group("[", "]", Self::parse)?)
        } else if parser.at("{") {
            Self::Dictionary(parser.attributes()?)
        } else if parser.at_kind(LexerTokenKind::SymbolIdentifier) {
            Self::Symbol(parser.name(LexerTokenKind::SymbolIdentifier)?)
        } else if parser.at_kind(LexerTokenKind::Integer) || parser.at("-") || parser.at("+") {
            let value = IntegerLiteral::parse(parser)?;
            let ty = if parser.optional(":") {
                Some(Type::parse(parser)?)
            } else {
                None
            };
            Self::Integer { value, ty }
        } else {
            Self::Type(Type::parse(parser)?)
        })
    }
}

impl ParserNode for IntegerLiteral {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let start = parser.position();
        if !parser.optional("-") {
            parser.optional("+");
        }
        parser.token(LexerTokenKind::Integer, "integer literal")?;
        let position = parser.span(start);
        Ok(Self {
            spelling: parser.source.slice(position.range()),
            position,
        })
    }
}

impl ParserNode for StringLiteral {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let token = parser.token(LexerTokenKind::String, "string literal")?;
        Ok(Self {
            spelling: token.text,
            position: token.position,
        })
    }
}

impl ParserNode for Value {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let name = parser.name(LexerTokenKind::ValueIdentifier)?;
        let result_index = if parser.at_kind(LexerTokenKind::AttributeIdentifier) {
            let token = parser.required()?;
            let text = String::from_utf8_lossy(&token.text);
            Some(text[1..].parse::<u32>().map_err(|_| ParserError::Syntax {
                message: "expected numeric SSA result index".into(),
                position: token.position,
            })?)
        } else {
            None
        };
        Ok(Self { name, result_index })
    }
}

impl ParserNode for ResultBinding {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let name = parser.name(LexerTokenKind::ValueIdentifier)?;
        let count = if parser.optional(":") {
            parser.unsigned::<u32>("result count")?
        } else {
            1
        };
        if count == 0 {
            return Err(parser.error("result count must be positive"));
        }
        Ok(Self { name, count })
    }
}

impl ParserNode for InstanceInput {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let name = parser.field_name()?;
        parser.expect(":")?;
        let value = Value::parse(parser)?;
        parser.expect(":")?;
        let ty = Type::parse(parser)?;
        Ok(Self { name, value, ty })
    }
}

impl ParserNode for Operation {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let start = parser.position();
        let results = if parser.at_kind(LexerTokenKind::ValueIdentifier) {
            let results = parser.separated::<ResultBinding>()?;
            parser.expect("=")?;
            results
        } else {
            Vec::new()
        };
        let name = parser.token(LexerTokenKind::Identifier, "custom operation name")?;
        let spelling = String::from_utf8_lossy(&name.text);
        if !supported_operation(&spelling) {
            return Err(ParserError::Syntax {
                message: format!("unsupported operation `{spelling}`"),
                position: name.position,
            });
        }
        let mut attributes = Vec::new();
        let kind = parser.operation_kind(&spelling, &mut attributes)?;
        let expected_results = match &kind {
            OperationKind::Output { .. } | OperationKind::ClockedProperty { .. } => 0,
            OperationKind::Instance { outputs, .. } => outputs.len() as u64,
            _ => 1,
        };
        let actual_results: u64 = (&results)
            .into_iter()
            .map(|result| u64::from(result.count))
            .sum();
        if actual_results != expected_results {
            return Err(ParserError::Syntax {
                message: format!(
                    "`{spelling}` requires {expected_results} result bindings, found {actual_results}"
                ),
                position: start,
            });
        }
        let location = parser.location()?;
        Ok(Self {
            results,
            kind,
            attributes,
            location,
            position: parser.span(start),
        })
    }
}

impl ParserNode for ComparisonPredicate {
    fn parse(parser: &mut Parser) -> ParserResult<Self> {
        let token = parser.token(LexerTokenKind::Identifier, "comparison predicate")?;
        Ok(match token.text.as_ref() {
            b"eq" => Self::Equal,
            b"ne" => Self::NotEqual,
            b"slt" => Self::SignedLess,
            b"sle" => Self::SignedLessEqual,
            b"sgt" => Self::SignedGreater,
            b"sge" => Self::SignedGreaterEqual,
            b"ult" => Self::UnsignedLess,
            b"ule" => Self::UnsignedLessEqual,
            b"ugt" => Self::UnsignedGreater,
            b"uge" => Self::UnsignedGreaterEqual,
            b"ceq" => Self::CaseEqual,
            b"cne" => Self::CaseNotEqual,
            b"weq" => Self::WildcardEqual,
            b"wne" => Self::WildcardNotEqual,
            _ => {
                return Err(ParserError::Syntax {
                    message: format!("unsupported comparison predicate `{token}`"),
                    position: token.position,
                });
            }
        })
    }
}
