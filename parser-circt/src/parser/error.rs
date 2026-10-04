use crate::lexer::{error::LexerError, position::LexerPosition};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ParserError {
    #[error(transparent)]
    Lexer(#[from] LexerError),
    #[error("{message} at {position}")]
    Syntax {
        message: String,
        position: LexerPosition,
    },
}

impl ParserError {
    pub fn position(&self) -> LexerPosition {
        match self {
            Self::Lexer(error) => error.position,
            Self::Syntax { position, .. } => *position,
        }
    }
}

pub type ParserResult<T> = Result<T, ParserError>;
