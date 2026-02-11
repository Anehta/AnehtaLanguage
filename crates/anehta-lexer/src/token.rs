/// Source location span
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

/// All token types in AnehtaLanguage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    // Literals
    Number,       // 42, 3.14
    StringLit,    // "hello"
    True,         // true
    False,        // false
    Word,         // identifiers

    // Arithmetic operators
    Add,          // +
    Sub,          // -
    Mul,          // *
    Div,          // /
    Power,        // ^
    Mod,          // %
    Rand,         // ~
    AddSelf,      // ++
    SubSelf,      // --

    // Compound assignment
    CompositeAdd, // +=
    CompositeSub, // -=
    CompositeMul, // *=
    CompositeDiv, // /=

    // Comparison
    Gt,           // >
    Lt,           // <
    GtEq,         // >=
    LtEq,         // <=
    Eq,           // ==
    NotEq,        // !=

    // Logical
    Not,          // !
    Also,         // &&
    Perhaps,      // ||

    // Bitwise (reserved)
    And,          // &
    Or,           // |

    // Assignment & type
    Assignment,   // =
    FatArrow,     // =>
    Casting,      // ->
    Dot,          // .

    // Delimiters
    LParen,       // (
    RParen,       // )
    LBrace,       // {
    RBrace,       // }
    LBracket,     // [
    RBracket,     // ]
    Comma,        // ,
    Colon,        // :
    Semicolon,    // ;

    // Keywords
    Func,
    Var,
    If,
    Else,
    ElseIf,
    For,
    Break,
    Continue,
    Return,
    Switch,
    Case,
    New,
    Timer,

    // Special
    Newline,      // \n, \r, \r\n (statement separator)
    Eof,          // end of file
}

/// A single token
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub token_type: TokenType,
    pub value: String,
    pub span: Span,
}
