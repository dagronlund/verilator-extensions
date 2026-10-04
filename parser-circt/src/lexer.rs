//! Lossless MLIR lexing, using Logos/Bytes as the underlying mechanism.
//!
//! `Lexer` includes comments and whitespace; `next_syntax` skips them. Literal
//! values are left as source text for later interpretation by the parser.
//!
//! ```
//! use parser_circt::lexer::{Lexer, token::LexerTokenKind};
//!
//! let mut lexer = Lexer::new(0, "%sum = comb.add %a, %b : i32");
//! let token = lexer.next_syntax()?.unwrap();
//! assert_eq!(token.kind, LexerTokenKind::ValueIdentifier);
//! assert_eq!(token.text.as_ref(), b"%sum");
//! # Ok::<(), parser_circt::lexer::LexerError>(())
//! ```

pub mod position;
pub mod symbols;
pub mod token;

use std::iter::FusedIterator;

use bytes::Bytes;
use logos::Logos;

use crate::lexer::{
    position::LexerPosition,
    token::{LexerToken, LexerTokenKind},
};

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

/// An iterator of positioned tokens. Stops after its first error.
pub struct Lexer<'a> {
    file_id: usize,
    bytes: Bytes,
    lexer: logos::Lexer<'a, LexerTokenKind>,
    line: usize,
    column: usize,
    previous_cr: bool,
    finished: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(file_id: usize, source: &'a str) -> Self {
        Self {
            file_id,
            bytes: Bytes::copy_from_slice(source.as_bytes()),
            lexer: LexerTokenKind::lexer(source),
            line: 1,
            column: 1,
            previous_cr: false,
            finished: false,
        }
    }

    pub fn next_syntax(&mut self) -> LexerResult<Option<LexerToken>> {
        for token in self.by_ref() {
            let token = token?;
            if !token.kind.is_whitespace() {
                return Ok(Some(token));
            }
        }
        Ok(None)
    }
}

impl Iterator for Lexer<'_> {
    type Item = LexerResult<LexerToken>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let Some(kind) = self.lexer.next() else {
            self.finished = true;
            return None;
        };
        let span = self.lexer.span();
        let position = LexerPosition {
            file_id: self.file_id,
            index: span.start,
            line: self.line,
            column: self.column,
            length: span.len(),
        };
        let kind = match kind {
            Ok(kind) => kind,
            Err(kind) => {
                self.finished = true;
                return Some(Err(LexerError { kind, position }));
            }
        };
        for byte in self.lexer.slice().bytes() {
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
        Some(Ok(LexerToken {
            kind,
            text: self.bytes.slice(span),
            position,
        }))
    }
}

impl FusedIterator for Lexer<'_> {}
