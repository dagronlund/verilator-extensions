/// A source span. Offsets and lengths count bytes; lines and byte columns start at one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LexerPosition {
    pub file_id: usize,
    pub index: usize,
    pub line: usize,
    pub column: usize,
    pub length: usize,
}

impl LexerPosition {
    pub fn range(self) -> std::ops::Range<usize> {
        self.index..self.index + self.length
    }
}

impl std::fmt::Display for LexerPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file_id, self.line, self.column)
    }
}
