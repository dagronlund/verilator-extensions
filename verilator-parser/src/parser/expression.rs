use crate::{
    ast::{BinaryOperator, DataTypeId, Expression, ExpressionKind, Literal, UnaryOperator},
    document::{AstNode, AstValue},
    parser::{ParseError, Parser, parse_access, source_info},
};

impl<'a> Parser<'a> {
    pub(super) fn parse_expression(&self, node: &AstNode) -> Result<Expression, ParseError> {
        let dtype = self.node_dtype(node)?;
        let kind = match node.node_type.as_str() {
            "CONST" => ExpressionKind::Constant(parse_literal(node)?),
            "VARREF" => ExpressionKind::Variable {
                variable: self.reference_variable_id(node)?,
                access: parse_access(node)?,
            },
            "NOT" | "NEGATE" | "REDAND" | "REDOR" | "REDXOR" | "LOGNOT" | "EXTEND" | "EXTENDS" => {
                let operand = node
                    .child("lhsp")
                    .or_else(|| node.child("rhsp"))
                    .ok_or_else(|| ParseError::node(node, "unary expression has no operand"))?;
                ExpressionKind::Unary {
                    operator: match node.node_type.as_str() {
                        "NOT" => UnaryOperator::BitwiseNot,
                        "NEGATE" => UnaryOperator::Negate,
                        "REDAND" => UnaryOperator::ReduceAnd,
                        "REDOR" => UnaryOperator::ReduceOr,
                        "REDXOR" => UnaryOperator::ReduceXor,
                        "LOGNOT" => UnaryOperator::LogicalNot,
                        "EXTEND" => UnaryOperator::ZeroExtend,
                        "EXTENDS" => UnaryOperator::SignExtend,
                        _ => unreachable!(),
                    },
                    operand: Box::new(self.parse_expression(operand)?),
                }
            }
            "AND" | "OR" | "XOR" | "ADD" | "SUB" | "MUL" | "MULS" | "DIV" | "DIVS" | "EQ"
            | "NEQ" | "LT" | "LTE" | "GT" | "GTE" | "LTS" | "LTES" | "GTS" | "GTES" | "LOGAND"
            | "LOGOR" | "SHIFTL" | "SHIFTR" | "SHIFTRS" | "CONCAT" => {
                let lhs = node
                    .child("lhsp")
                    .ok_or_else(|| ParseError::node(node, "binary expression has no lhs"))?;
                let rhs = node
                    .child("rhsp")
                    .ok_or_else(|| ParseError::node(node, "binary expression has no rhs"))?;
                ExpressionKind::Binary {
                    operator: parse_binary_operator(node),
                    lhs: Box::new(self.parse_expression(lhs)?),
                    rhs: Box::new(self.parse_expression(rhs)?),
                }
            }
            "COND" => {
                let condition = node
                    .child("condp")
                    .ok_or_else(|| ParseError::node(node, "COND has no condition"))?;
                let then_value = node
                    .child("thenp")
                    .or_else(|| node.child("thensp"))
                    .ok_or_else(|| ParseError::node(node, "COND has no true value"))?;
                let else_value = node
                    .child("elsep")
                    .or_else(|| node.child("elsesp"))
                    .ok_or_else(|| ParseError::node(node, "COND has no false value"))?;
                ExpressionKind::Conditional {
                    condition: Box::new(self.parse_expression(condition)?),
                    then_value: Box::new(self.parse_expression(then_value)?),
                    else_value: Box::new(self.parse_expression(else_value)?),
                }
            }
            "REPLICATE" => {
                let source = node
                    .child("srcp")
                    .ok_or_else(|| ParseError::node(node, "REPLICATE has no source"))?;
                let count_node = node
                    .child("countp")
                    .ok_or_else(|| ParseError::node(node, "REPLICATE has no count"))?;
                if count_node.node_type != "CONST" {
                    return Err(ParseError::node(
                        count_node,
                        "REPLICATE count must be constant",
                    ));
                }
                let count_literal = parse_literal(count_node)?;
                let count = usize::try_from(&count_literal.value)
                    .map_err(|_| ParseError::node(count_node, "REPLICATE count exceeds usize"))?;
                let parsed_source = self.parse_expression(source)?;
                let source_width = self.data_types[parsed_source.dtype.0].width;
                let result_width = source_width.checked_mul(count).ok_or_else(|| {
                    ParseError::node(node, "REPLICATE result width exceeds usize")
                })?;
                if result_width != self.data_types[dtype.0].width {
                    return Err(ParseError::node(node, "REPLICATE result width mismatch"));
                }
                ExpressionKind::Replicate {
                    source: Box::new(parsed_source),
                    count_literal,
                    count,
                }
            }
            "SEL" => {
                let value_node = node
                    .child("fromp")
                    .ok_or_else(|| ParseError::node(node, "SEL has no fromp"))?;
                let offset_node = node
                    .child("lsbp")
                    .ok_or_else(|| ParseError::node(node, "SEL has no lsbp"))?;
                let value = self.parse_expression(value_node)?;
                let offset = self.parse_expression(offset_node)?;
                let width = self.selection_width(node)?;
                let value_width = self.data_types[value.dtype.0].width;
                if let ExpressionKind::Constant(literal) = &offset.kind {
                    let offset_value = usize::try_from(&literal.value).map_err(|_| {
                        ParseError::node(offset_node, "selection constant exceeds usize")
                    })?;
                    if offset_value
                        .checked_add(width)
                        .is_none_or(|end| end > value_width)
                    {
                        return Err(ParseError::node(node, "SEL is out of range"));
                    }
                }
                ExpressionKind::Select {
                    value: Box::new(value),
                    offset: Box::new(offset),
                    width,
                }
            }
            "ARRAYSEL" => {
                let array_node = node
                    .child("fromp")
                    .ok_or_else(|| ParseError::node(node, "ARRAYSEL has no fromp"))?;
                let index_node = node
                    .child("bitp")
                    .ok_or_else(|| ParseError::node(node, "ARRAYSEL has no bitp"))?;
                let array = self.parse_expression(array_node)?;
                if self.data_types[array.dtype.0].unpacked.is_none() {
                    return Err(ParseError::node(
                        array_node,
                        "ARRAYSEL source is not an unpacked array",
                    ));
                }
                ExpressionKind::ArraySelect {
                    array: Box::new(array),
                    index: Box::new(self.parse_expression(index_node)?),
                }
            }
            _ => return Err(ParseError::node(node, "unsupported expression")),
        };
        Ok(Expression {
            source: source_info(node),
            dtype,
            kind,
        })
    }

    pub(super) fn selection_width(&self, node: &AstNode) -> Result<usize, ParseError> {
        if let Some(width) = node.child("widthp") {
            if width.node_type != "CONST" {
                return Err(ParseError::node(width, "dynamic selection is unsupported"));
            }
            return usize::try_from(&parse_literal(width)?.value)
                .map_err(|_| ParseError::node(width, "selection constant exceeds usize"));
        }
        let Some(AstValue::Number(width)) = node.field("widthConst") else {
            return Err(ParseError::node(node, "SEL has no widthp or widthConst"));
        };
        width
            .as_u64()
            .and_then(|width| usize::try_from(width).ok())
            .ok_or_else(|| ParseError::node(node, "SEL widthConst exceeds usize"))
    }

    pub(super) fn node_dtype(&self, node: &AstNode) -> Result<DataTypeId, ParseError> {
        let address = node
            .string("dtypep")
            .ok_or_else(|| ParseError::node(node, "expression has no dtypep"))?;
        self.dtype_ids
            .get(address)
            .copied()
            .ok_or_else(|| ParseError::node(node, format!("unresolved dtypep {address}")))
    }
}

pub(super) fn is_supported_expression(node_type: &str) -> bool {
    match node_type {
        "CONST" | "VARREF" | "NOT" | "NEGATE" | "REDAND" | "REDOR" | "REDXOR" | "LOGNOT"
        | "EXTEND" | "EXTENDS" | "AND" | "OR" | "XOR" | "ADD" | "SUB" | "MUL" | "MULS" | "DIV"
        | "DIVS" | "EQ" | "NEQ" | "LT" | "LTE" | "GT" | "GTE" | "LTS" | "LTES" | "GTS" | "GTES"
        | "LOGAND" | "LOGOR" | "SHIFTL" | "SHIFTR" | "SHIFTRS" | "CONCAT" | "COND"
        | "REPLICATE" | "SEL" | "ARRAYSEL" => true,
        _ => false,
    }
}

pub(super) fn parse_literal(node: &AstNode) -> Result<Literal, ParseError> {
    let spelling = node
        .name
        .clone()
        .ok_or_else(|| ParseError::node(node, "constant has no value"))?;
    let (_, digits) = spelling
        .split_once('\'')
        .ok_or_else(|| ParseError::node(node, format!("unsupported constant {spelling}")))?;
    let (radix, digits) = digits.split_at(1);
    let cleaned = digits.replace('_', "");
    if cleaned.chars().any(|character| {
        character == 'x' || character == 'X' || character == 'z' || character == 'Z'
    }) {
        return Err(ParseError::node(
            node,
            format!("unknown-valued constant {spelling} is unsupported"),
        ));
    }
    let radix = match radix {
        "b" | "B" => 2,
        "o" | "O" => 8,
        "d" | "D" => 10,
        "h" | "H" => 16,
        _ => {
            return Err(ParseError::node(
                node,
                format!("unsupported constant {spelling}"),
            ));
        }
    };
    let value = num_bigint::BigUint::parse_bytes(cleaned.as_bytes(), radix)
        .ok_or_else(|| ParseError::node(node, format!("invalid constant {spelling}")))?;
    Ok(Literal { spelling, value })
}

fn parse_binary_operator(node: &AstNode) -> BinaryOperator {
    match node.node_type.as_str() {
        "AND" => BinaryOperator::BitwiseAnd,
        "OR" => BinaryOperator::BitwiseOr,
        "XOR" => BinaryOperator::BitwiseXor,
        "ADD" => BinaryOperator::Add,
        "SUB" => BinaryOperator::Subtract,
        "MUL" => BinaryOperator::MultiplyUnsigned,
        "MULS" => BinaryOperator::MultiplySigned,
        "DIV" => BinaryOperator::DivideUnsigned,
        "DIVS" => BinaryOperator::DivideSigned,
        "EQ" => BinaryOperator::Equal,
        "NEQ" => BinaryOperator::NotEqual,
        "LT" => BinaryOperator::LessThanUnsigned,
        "LTE" => BinaryOperator::LessThanOrEqualUnsigned,
        "GT" => BinaryOperator::GreaterThanUnsigned,
        "GTE" => BinaryOperator::GreaterThanOrEqualUnsigned,
        "LTS" => BinaryOperator::LessThanSigned,
        "LTES" => BinaryOperator::LessThanOrEqualSigned,
        "GTS" => BinaryOperator::GreaterThanSigned,
        "GTES" => BinaryOperator::GreaterThanOrEqualSigned,
        "LOGAND" => BinaryOperator::LogicalAnd,
        "LOGOR" => BinaryOperator::LogicalOr,
        "SHIFTL" => BinaryOperator::ShiftLeft,
        "SHIFTR" => BinaryOperator::ShiftRight,
        "SHIFTRS" => BinaryOperator::ShiftRightArithmetic,
        "CONCAT" => BinaryOperator::Concat,
        _ => unreachable!(),
    }
}
