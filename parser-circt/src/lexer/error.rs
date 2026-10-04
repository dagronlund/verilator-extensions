use crate::lexer::position::LexerPosition;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, thiserror::Error)]
pub enum LexerErrorKind {
    #[default]
    #[error("unexpected character or malformed token")]
    UnexpectedCharacter,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{kind} at {position}")]
pub struct LexerError {
    pub kind: LexerErrorKind,
    /// For a malformed string, the span starts at the opening quote (or `@`).
    pub position: LexerPosition,
}

pub type LexerResult<T> = Result<T, LexerError>;
