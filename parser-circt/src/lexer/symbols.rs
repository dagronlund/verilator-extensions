use crate::lexer::error::LexerErrorKind;

/// Symbols shared by MLIR dialects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexerSymbol {
    Arrow,
    Ellipsis,
    FileMetadataBegin,
    FileMetadataEnd,
    Colon,
    Comma,
    Equal,
    Less,
    Greater,
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    OpenSquare,
    CloseSquare,
    Plus,
    Minus,
    Star,
    Slash,
    Question,
    VerticalBar,
}

impl TryFrom<&str> for LexerSymbol {
    type Error = LexerErrorKind;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "->" => Self::Arrow,
            "..." => Self::Ellipsis,
            "{-#" => Self::FileMetadataBegin,
            "#-}" => Self::FileMetadataEnd,
            ":" => Self::Colon,
            "," => Self::Comma,
            "=" => Self::Equal,
            "<" => Self::Less,
            ">" => Self::Greater,
            "(" => Self::OpenParen,
            ")" => Self::CloseParen,
            "{" => Self::OpenBrace,
            "}" => Self::CloseBrace,
            "[" => Self::OpenSquare,
            "]" => Self::CloseSquare,
            "+" => Self::Plus,
            "-" => Self::Minus,
            "*" => Self::Star,
            "/" => Self::Slash,
            "?" => Self::Question,
            "|" => Self::VerticalBar,
            _ => return Err(LexerErrorKind::UnexpectedCharacter),
        })
    }
}
