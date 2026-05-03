/// Structure du Lexer

#[derive(Debug)]
pub struct Lexer {
    pub(super) source: Vec<char>,
    pub(super) pos:    usize,
    pub(super) line:   usize,
    pub(super) col:    usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            pos:    0,
            line:   1,
            col:    1,
        }
    }
}
