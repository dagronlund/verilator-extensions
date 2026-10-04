use bytes::Bytes;
use logos::Logos;

use crate::lexer::{LexerErrorKind, position::LexerPosition, symbols::LexerSymbol};

/// MLIR token categories. Keywords and builtin type names remain identifiers;
/// the parser interprets their spelling in context.
#[derive(Logos, Clone, Copy, Debug, Eq, PartialEq)]
#[logos(error = LexerErrorKind)]
#[logos(subpattern suffix_id = r"([0-9]+|[a-zA-Z$._-][a-zA-Z0-9$._-]*)")]
// MLIR strings accept escaped quotes/backslashes, n/t, and two hex digits.
#[logos(subpattern quoted_string = r#""([^"\\\n\x0b\x0c]|\\(["\\nt]|[0-9a-fA-F]{2}))*""#)]
pub enum LexerTokenKind {
    /// An identifier, like `foo` or `bar`
    #[regex(r"[a-zA-Z_][a-zA-Z_0-9$.]*")]
    Identifier,
    /// A value identifier, like `%sum`
    #[regex(r"%(?&suffix_id)")]
    ValueIdentifier,
    /// A block identifier, like `^bb1`
    #[regex(r"\^(?&suffix_id)")]
    BlockIdentifier,
    /// A type identifier, like `!i32`
    #[regex(r"!(?&suffix_id)")]
    TypeIdentifier,
    /// An attribute identifier, like `#foo`
    #[regex(r"#(?&suffix_id)")]
    AttributeIdentifier,
    /// A symbol identifier, like `@foo` or `@"bar"`
    #[regex(r"@[a-zA-Z_][a-zA-Z_0-9$.]*")]
    #[regex(r"@(?&quoted_string)")]
    SymbolIdentifier,

    // Signs are separate tokens. Keep the spelling to avoid overflow and retain
    // hexadecimal floating-point bit patterns until the type is known.
    #[regex(r"[0-9]+")]
    #[regex(r"0x[0-9a-fA-F]+")]
    Integer,
    #[regex(r"[0-9]+\.[0-9]*([eE][+-]?[0-9]+)?")]
    Float,
    #[regex(r"(?&quoted_string)")]
    String,

    /// A symbol, like `->` or `...` or `{-#` or `#-}`
    #[regex(r"->|\.\.\.|\{-#|#-\}|[:,=<>(){}\[\]+*/?|-]", lex_symbol)]
    Symbol(LexerSymbol),

    /// A line comment, like `// this is a comment`
    #[regex(r"//[^\r\n]*")]
    LineComment,
    /// Whitespace, like spaces or tabs
    #[regex(r"[ \t\x00]+")]
    Whitespace,
    /// A newline, like `\n` or `\r\n`
    #[regex(r"\r\n|\r|\n")]
    NewLine,
}

impl LexerTokenKind {
    pub fn is_whitespace(self) -> bool {
        self == Self::LineComment || self == Self::Whitespace || self == Self::NewLine
    }
}

/// A token owns a cheap slice of the shared source buffer, including delimiters
/// and escapes. Concatenating every token's text reproduces the input exactly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexerToken {
    pub kind: LexerTokenKind,
    pub text: Bytes,
    pub position: LexerPosition,
}

impl std::fmt::Display for LexerToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The lexer accepts UTF-8 source and only slices at character boundaries.
        f.write_str(&String::from_utf8_lossy(&self.text))
    }
}

fn lex_symbol(lex: &mut logos::Lexer<'_, LexerTokenKind>) -> Result<LexerSymbol, LexerErrorKind> {
    LexerSymbol::try_from(lex.slice())
}
