use verilator_parser::{ast::SourceInfo, parser::ParseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertError(String);

impl ConvertError {
    pub(crate) fn source(source: &SourceInfo, message: impl std::fmt::Display) -> Self {
        Self(format!(
            "{} at {}: {message}",
            source.node_type,
            source.location.as_deref().unwrap_or("unknown location")
        ))
    }

    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl From<ParseError> for ConvertError {
    fn from(error: ParseError) -> Self {
        Self(format!("could not build typed AST: {error}"))
    }
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConvertError {}
