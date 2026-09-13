use verilator_parser::ast::SourceInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateError(String);

impl GenerateError {
    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    pub(crate) fn source(source: &SourceInfo, message: impl std::fmt::Display) -> Self {
        Self(format!(
            "{} at {}: {message}",
            source.node_type,
            source.location.as_deref().unwrap_or("unknown location")
        ))
    }
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GenerateError {}
