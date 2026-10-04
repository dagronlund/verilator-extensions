use std::fmt;

use parser_circt::{ast::Operation, lexer::position::LexerPosition, parser::error::ParserError};

/// A conversion diagnostic, including the retained MLIR and instance provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvertError {
    pub message: String,
    pub position: Option<LexerPosition>,
    pub location: Option<String>,
    pub instance: String,
}

impl ConvertError {
    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            position: None,
            location: None,
            instance: String::new(),
        }
    }

    pub(crate) fn at(operation: &Operation, instance: &str, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            position: Some(operation.position),
            location: operation
                .location
                .as_ref()
                .map(|loc| String::from_utf8_lossy(&loc.spelling).into_owned()),
            instance: instance.to_owned(),
        }
    }
}

impl From<ParserError> for ConvertError {
    fn from(error: ParserError) -> Self {
        Self {
            position: Some(error.position()),
            ..Self::message(error.to_string())
        }
    }
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if let Some(position) = self.position {
            write!(f, " at {position}")?;
        }
        if !self.instance.is_empty() {
            write!(f, " in {}", self.instance)?;
        }
        if let Some(location) = &self.location {
            write!(f, " ({location})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ConvertError {}
