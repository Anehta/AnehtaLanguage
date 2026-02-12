use anehta_lexer::{Token, TokenType, Span};
use crate::ast::*;

mod stmts;
mod boolean;
mod exprs;
#[cfg(test)]
mod tests;

/// Recursive-descent parser for AnehtaLanguage
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse all tokens into a Program AST
    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let stmts = self.main_statement()?;
        Ok(Program { statements: stmts })
    }

    // ── Token navigation ─────────────────────────────────────

    fn current(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn current_type(&self) -> TokenType {
        self.current().token_type
    }

    fn current_span(&self) -> Span {
        self.current().span
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn back(&mut self) {
        if self.pos > 0 {
            self.pos -= 1;
        }
    }

    fn expect(&mut self, expected: TokenType) -> Result<Token, ParseError> {
        let tok = self.advance().clone();
        if tok.token_type != expected {
            return Err(self.error_at(
                tok.span,
                format!("expected {:?}, found {:?} '{}'", expected, tok.token_type, tok.value),
            ));
        }
        Ok(tok)
    }

    fn peek_type(&self) -> TokenType {
        self.current_type()
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len() || self.current_type() == TokenType::Eof
    }

    fn error_at(&self, span: Span, message: String) -> ParseError {
        ParseError::Error {
            line: span.line,
            column: span.column,
            message,
        }
    }

    fn skip_newlines(&mut self) {
        while self.current_type() == TokenType::Newline {
            self.advance();
        }
    }

    // ── MainStatement ────────────────────────────────────────
    // <MainStatement> ::= <Statement> (EOF <Statement>)* EOF(end)

    fn main_statement(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();

        self.skip_newlines();

        while !self.is_at_end() {
            let stmt = self.statement()?;
            stmts.push(stmt);
            // consume newlines between statements
            self.skip_newlines();
        }

        Ok(stmts)
    }

    // ── Statement ────────────────────────────────────────────
    // Dispatch based on current token

    fn statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek_type() {
            TokenType::Func => self.func_statement(),
            TokenType::Var => self.var_statement(),
            TokenType::LBrace => self.block_statement_as_stmt(),
            TokenType::If => self.if_statement(),
            TokenType::For => self.for_statement(),
            TokenType::Timer => self.timer_statement(),
            TokenType::Word => self.word_dispatch_statement(),
            _ => {
                let tok = self.current().clone();
                Err(self.error_at(
                    tok.span,
                    format!(
                        "unexpected '{}', expected func/var/if/for/word/{{ ->Statement",
                        tok.value
                    ),
                ))
            }
        }
    }

    /// When we see a Word at statement level, look ahead to decide
    /// if it is a CallFunc or an Assignment.
    fn word_dispatch_statement(&mut self) -> Result<Statement, ParseError> {
        // peek: WORD
        let word_tok = self.advance().clone(); // consume WORD
        let next = self.advance().clone(); // peek next

        if next.token_type == TokenType::LParen {
            // CallFunc: word(...)
            self.back(); // put back '('
            self.back(); // put back WORD
            let call = self.call_func_statement()?;
            Ok(Statement::CallFunc(call))
        } else if next.token_type == TokenType::Assignment
            || next.token_type == TokenType::Comma
        {
            // Assignment: word, ... = ...
            self.back(); // put back '=' or ','
            self.back(); // put back WORD
            let assign = self.assignment_statement()?;
            Ok(Statement::Assignment(assign))
        } else if next.token_type == TokenType::Dot {
            // Could be field assignment (word.field = expr) or method call (word.field(args))
            let span = word_tok.span;
            let field_tok = self.expect(TokenType::Word)?;
            if self.peek_type() == TokenType::LParen {
                // Method call: word.field(args) — may chain further: word.field(args).field2(args2)
                self.advance(); // consume (
                let mut args = Vec::new();
                if self.peek_type() != TokenType::RParen {
                    loop {
                        args.push(self.arithmetic_expression()?);
                        if self.peek_type() == TokenType::Comma {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(TokenType::RParen)?;
                let callee = Expr::FieldAccess(FieldAccess {
                    object: Box::new(Expr::Variable(word_tok.value, span)),
                    field: field_tok.value.clone(),
                    span,
                });
                let mut result = Expr::MethodCall(MethodCall {
                    callee: Box::new(callee),
                    args,
                    span,
                });
                // Handle further chaining: .field, ["key"], (args)
                loop {
                    match self.peek_type() {
                        TokenType::Dot => {
                            let s = self.current_span();
                            self.advance();
                            let ft = self.expect(TokenType::Word)?;
                            result = Expr::FieldAccess(FieldAccess {
                                object: Box::new(result),
                                field: ft.value,
                                span: s,
                            });
                        }
                        TokenType::LBracket => {
                            let s = self.current_span();
                            self.advance();
                            let index = self.arithmetic_expression()?;
                            self.expect(TokenType::RBracket)?;
                            result = Expr::IndexAccess(IndexAccess {
                                object: Box::new(result),
                                index: Box::new(index),
                                span: s,
                            });
                        }
                        TokenType::LParen => {
                            if matches!(result, Expr::FieldAccess(_) | Expr::IndexAccess(_) | Expr::MethodCall(_)) {
                                let s = self.current_span();
                                self.advance();
                                let mut a = Vec::new();
                                if self.peek_type() != TokenType::RParen {
                                    loop {
                                        a.push(self.arithmetic_expression()?);
                                        if self.peek_type() == TokenType::Comma {
                                            self.advance();
                                        } else {
                                            break;
                                        }
                                    }
                                }
                                self.expect(TokenType::RParen)?;
                                result = Expr::MethodCall(MethodCall {
                                    callee: Box::new(result),
                                    args: a,
                                    span: s,
                                });
                            } else {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                // Extract the outermost MethodCall for the statement
                match result {
                    Expr::MethodCall(mc) => Ok(Statement::MethodCall(mc)),
                    _ => Err(self.error_at(span, "expected method call statement".to_string())),
                }
            } else {
                // Field assignment: word.field = expr (original logic)
                self.expect(TokenType::Assignment)?;
                let value = self.arithmetic_expression()?;
                Ok(Statement::FieldAssign(FieldAssign {
                    object: word_tok.value,
                    field: field_tok.value,
                    value,
                    span,
                }))
            }
        } else if next.token_type == TokenType::LBracket {
            // Index assignment: word["key"] = expr
            let span = word_tok.span;
            let index = self.arithmetic_expression()?;
            self.expect(TokenType::RBracket)?;
            self.expect(TokenType::Assignment)?;
            let value = self.arithmetic_expression()?;
            Ok(Statement::IndexAssign(IndexAssign {
                object: word_tok.value,
                index,
                value,
                span,
            }))
        } else {
            Err(self.error_at(
                word_tok.span,
                format!(
                    "unexpected '{}' after '{}', expected '(' or '=' or ',' or '.' or '[' ->Statement",
                    next.value, word_tok.value
                ),
            ))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Parse error at line {line}, column {column}: {message}")]
    Error {
        line: usize,
        column: usize,
        message: String,
    },
}
