use crate::token::{Token, TokenType, Span};

/// Lexer for AnehtaLanguage source code
pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Tokenize the entire source and return the token list
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        while self.pos < self.source.len() {
            let ch = self.source[self.pos];
            match ch {
                // Whitespace (skip)
                ' ' | '\t' => {
                    self.advance();
                }

                // Newline handling
                '\n' => {
                    let span = self.span();
                    self.push_token(TokenType::Newline, "\\n".to_string(), span);
                    self.pos += 1;
                    self.line += 1;
                    self.column = 1;
                }
                '\r' => {
                    let span = self.span();
                    self.pos += 1;
                    if self.pos < self.source.len() && self.source[self.pos] == '\n' {
                        self.push_token(TokenType::Newline, "\\r\\n".to_string(), span);
                        self.pos += 1;
                    } else {
                        self.push_token(TokenType::Newline, "\\r".to_string(), span);
                    }
                    self.line += 1;
                    self.column = 1;
                }

                // String literal
                '"' => {
                    self.read_string();
                }

                // Digits -> number
                '0'..='9' => {
                    self.read_number();
                }

                // Identifiers and keywords
                'a'..='z' | 'A'..='Z' | '_' => {
                    self.read_identifier_or_keyword();
                }

                // Operators and delimiters
                '+' => self.read_plus(),
                '-' => self.read_minus(),
                '*' => self.read_star(),
                '/' => self.read_slash(),
                '^' => {
                    let span = self.span();
                    self.push_token(TokenType::Power, "^".to_string(), span);
                    self.advance();
                }
                '%' => {
                    let span = self.span();
                    self.push_token(TokenType::Mod, "%".to_string(), span);
                    self.advance();
                }
                '~' => {
                    let span = self.span();
                    self.push_token(TokenType::Rand, "~".to_string(), span);
                    self.advance();
                }
                '!' => self.read_bang(),
                '>' => self.read_gt(),
                '<' => self.read_lt(),
                '=' => self.read_eq(),
                '&' => self.read_amp(),
                '|' => self.read_pipe(),
                '.' => {
                    let span = self.span();
                    self.advance();
                    // Check for .. or .^
                    match self.current() {
                        Some('.') => {
                            self.push_token(TokenType::Range, "..".to_string(), span);
                            self.advance();
                        }
                        Some('^') => {
                            self.push_token(TokenType::DotPow, ".^".to_string(), span);
                            self.advance();
                        }
                        _ => {
                            self.push_token(TokenType::Dot, ".".to_string(), span);
                        }
                    }
                }
                ',' => {
                    let span = self.span();
                    self.push_token(TokenType::Comma, ",".to_string(), span);
                    self.advance();
                }
                ':' => {
                    let span = self.span();
                    self.push_token(TokenType::Colon, ":".to_string(), span);
                    self.advance();
                }
                ';' => {
                    let span = self.span();
                    self.push_token(TokenType::Semicolon, ";".to_string(), span);
                    self.advance();
                }
                '(' => {
                    let span = self.span();
                    self.push_token(TokenType::LParen, "(".to_string(), span);
                    self.advance();
                }
                ')' => {
                    let span = self.span();
                    self.push_token(TokenType::RParen, ")".to_string(), span);
                    self.advance();
                }
                '{' => {
                    let span = self.span();
                    self.push_token(TokenType::LBrace, "{".to_string(), span);
                    self.advance();
                }
                '}' => {
                    let span = self.span();
                    self.push_token(TokenType::RBrace, "}".to_string(), span);
                    self.advance();
                }
                '[' => {
                    let span = self.span();
                    self.push_token(TokenType::LBracket, "[".to_string(), span);
                    self.advance();
                }
                ']' => {
                    let span = self.span();
                    self.push_token(TokenType::RBracket, "]".to_string(), span);
                    self.advance();
                }
                '@' => {
                    let span = self.span();
                    self.push_token(TokenType::At, "@".to_string(), span);
                    self.advance();
                }
                '#' => {
                    let span = self.span();
                    self.push_token(TokenType::Hash, "#".to_string(), span);
                    self.advance();
                }
                '\'' => {
                    let span = self.span();
                    self.push_token(TokenType::Transpose, "'".to_string(), span);
                    self.advance();
                }
                '\\' => {
                    let span = self.span();
                    self.push_token(TokenType::Backslash, "\\".to_string(), span);
                    self.advance();
                }

                // Illegal character
                _ => {
                    self.errors.push(LexError::Error {
                        line: self.line,
                        column: self.column,
                        message: format!("illegal token '{}'", ch),
                    });
                    self.advance();
                }
            }
        }

        // Append EOF
        let span = self.span();
        self.push_token(TokenType::Eof, "End".to_string(), span);

        if let Some(err) = self.errors.pop() {
            return Err(err);
        }

        Ok(std::mem::take(&mut self.tokens))
    }

    // ── Helpers ──────────────────────────────────────────────

    fn current(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.current()?;
        self.pos += 1;
        self.column += 1;
        Some(ch)
    }

    fn span(&self) -> Span {
        Span { line: self.line, column: self.column }
    }

    fn push_token(&mut self, token_type: TokenType, value: String, span: Span) {
        self.tokens.push(Token { token_type, value, span });
    }

    // ── Number ───────────────────────────────────────────────

    fn read_number(&mut self) {
        let span = self.span();
        let mut value = String::new();
        let mut dot_count = 0;

        while let Some(ch) = self.current() {
            if ch.is_ascii_digit() {
                value.push(ch);
                self.advance();
            } else if ch == '.' {
                dot_count += 1;
                if dot_count > 1 {
                    self.errors.push(LexError::Error {
                        line: self.line,
                        column: self.column,
                        message: "illegal number".to_string(),
                    });
                    break;
                }
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        self.push_token(TokenType::Number, value, span);
    }

    // ── String ───────────────────────────────────────────────

    fn read_string(&mut self) {
        let span = self.span();
        // Skip opening quote
        self.advance();
        let mut value = String::new();
        let mut closed = false;

        while let Some(ch) = self.current() {
            if ch == '"' {
                self.advance(); // skip closing quote
                closed = true;
                break;
            }
            if ch == '\\' {
                // Escape: skip backslash and take next char literally
                self.advance();
                if let Some(escaped) = self.current() {
                    value.push(escaped);
                    self.advance();
                }
                continue;
            }
            value.push(ch);
            self.advance();
        }

        if !closed {
            self.errors.push(LexError::Error {
                line: span.line,
                column: span.column,
                message: "lose a '\"'".to_string(),
            });
        }

        self.push_token(TokenType::StringLit, value, span);
    }

    // ── Identifier / Keyword ─────────────────────────────────

    fn read_identifier_or_keyword(&mut self) {
        let span = self.span();
        let mut word = String::new();

        while let Some(ch) = self.current() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                word.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let token_type = match word.as_str() {
            "func" => TokenType::Func,
            "var" => TokenType::Var,
            "if" => TokenType::If,
            "else" => TokenType::Else,
            "elseif" => TokenType::ElseIf,
            "for" => TokenType::For,
            "break" => TokenType::Break,
            "continue" => TokenType::Continue,
            "return" => TokenType::Return,
            "true" => TokenType::True,
            "false" => TokenType::False,
            "switch" => TokenType::Switch,
            "case" => TokenType::Case,
            "new" => TokenType::New,
            "timer" => TokenType::Timer,
            _ => TokenType::Word,
        };

        self.push_token(token_type, word, span);
    }

    // ── Compound operators ───────────────────────────────────

    /// `+`, `++`, `+=`
    fn read_plus(&mut self) {
        let span = self.span();
        self.advance(); // consume '+'
        match self.current() {
            Some('+') => {
                self.advance();
                self.push_token(TokenType::AddSelf, "++".to_string(), span);
            }
            Some('=') => {
                self.advance();
                self.push_token(TokenType::CompositeAdd, "+=".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::Add, "+".to_string(), span);
            }
        }
    }

    /// `-`, `--`, `-=`, `->`
    fn read_minus(&mut self) {
        let span = self.span();
        self.advance(); // consume '-'
        match self.current() {
            Some('-') => {
                self.advance();
                self.push_token(TokenType::SubSelf, "--".to_string(), span);
            }
            Some('=') => {
                self.advance();
                self.push_token(TokenType::CompositeSub, "-=".to_string(), span);
            }
            Some('>') => {
                self.advance();
                self.push_token(TokenType::Casting, "->".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::Sub, "-".to_string(), span);
            }
        }
    }

    /// `*`, `*=`
    fn read_star(&mut self) {
        let span = self.span();
        self.advance(); // consume '*'
        match self.current() {
            Some('=') => {
                self.advance();
                self.push_token(TokenType::CompositeMul, "*=".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::Mul, "*".to_string(), span);
            }
        }
    }

    /// `/`, `/=`, `// comment`
    fn read_slash(&mut self) {
        let span = self.span();
        self.advance(); // consume '/'
        match self.current() {
            Some('/') => {
                // Line comment: skip to end of line
                while let Some(ch) = self.current() {
                    if ch == '\n' || ch == '\r' {
                        break;
                    }
                    self.advance();
                }
            }
            Some('=') => {
                self.advance();
                self.push_token(TokenType::CompositeDiv, "/=".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::Div, "/".to_string(), span);
            }
        }
    }

    /// `!`, `!=`
    fn read_bang(&mut self) {
        let span = self.span();
        self.advance(); // consume '!'
        match self.current() {
            Some('=') => {
                self.advance();
                self.push_token(TokenType::NotEq, "!=".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::Not, "!".to_string(), span);
            }
        }
    }

    /// `>`, `>=`
    fn read_gt(&mut self) {
        let span = self.span();
        self.advance(); // consume '>'
        match self.current() {
            Some('=') => {
                self.advance();
                self.push_token(TokenType::GtEq, ">=".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::Gt, ">".to_string(), span);
            }
        }
    }

    /// `<`, `<=`
    fn read_lt(&mut self) {
        let span = self.span();
        self.advance(); // consume '<'
        match self.current() {
            Some('=') => {
                self.advance();
                self.push_token(TokenType::LtEq, "<=".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::Lt, "<".to_string(), span);
            }
        }
    }

    /// `=`, `==`, `=>`
    fn read_eq(&mut self) {
        let span = self.span();
        self.advance(); // consume '='
        match self.current() {
            Some('=') => {
                self.advance();
                self.push_token(TokenType::Eq, "==".to_string(), span);
            }
            Some('>') => {
                self.advance();
                self.push_token(TokenType::FatArrow, "=>".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::Assignment, "=".to_string(), span);
            }
        }
    }

    /// `&`, `&&`
    fn read_amp(&mut self) {
        let span = self.span();
        self.advance(); // consume '&'
        match self.current() {
            Some('&') => {
                self.advance();
                self.push_token(TokenType::Also, "&&".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::And, "&".to_string(), span);
            }
        }
    }

    /// `|`, `||`
    fn read_pipe(&mut self) {
        let span = self.span();
        self.advance(); // consume '|'
        match self.current() {
            Some('|') => {
                self.advance();
                self.push_token(TokenType::Perhaps, "||".to_string(), span);
            }
            _ => {
                self.push_token(TokenType::Or, "|".to_string(), span);
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LexError {
    #[error("Lex error at line {line}, column {column}: {message}")]
    Error {
        line: usize,
        column: usize,
        message: String,
    },
}

#[cfg(test)]
mod tests;
