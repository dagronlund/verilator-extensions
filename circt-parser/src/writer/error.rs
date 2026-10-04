use std::str::Utf8Error;

use crate::lexer::error::LexerError;

#[derive(Debug, thiserror::Error)]
pub enum WriterError {
    #[error(transparent)]
    Lexer(#[from] LexerError),
    #[error("AST spelling is not UTF-8: {0}")]
    Utf8(#[from] Utf8Error),
    #[error("AST cannot be represented in the supported assembly: {0}")]
    InvalidAst(&'static str),
}

pub type WriterResult<T> = Result<T, WriterError>;
