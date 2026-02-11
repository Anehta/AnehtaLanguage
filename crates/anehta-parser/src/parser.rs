use anehta_lexer::{Token, TokenType, Span};
use crate::ast::*;

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

    // ── FuncStatement ────────────────────────────────────────
    // func name(params) -> return_types { body }

    fn func_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::Func)?;
        let name_tok = self.expect(TokenType::Word)?;
        self.expect(TokenType::LParen)?;

        let params = self.func_params()?;

        self.expect(TokenType::RParen)?;
        self.expect(TokenType::Casting)?;

        let return_types = self.func_return_types()?;

        self.skip_newlines();
        let body = self.block_statement()?;

        Ok(Statement::FuncDecl(FuncDecl {
            name: name_tok.value,
            params,
            return_types,
            body,
            span,
        }))
    }

    /// Parse function parameter list (may be empty).
    /// Each param: `name: type`
    fn func_params(&mut self) -> Result<Vec<FuncParam>, ParseError> {
        let mut params = Vec::new();

        // empty param list: next token is ')'
        if self.peek_type() == TokenType::RParen {
            return Ok(params);
        }

        // first param (must start with Word)
        if self.peek_type() != TokenType::Word {
            return Ok(params);
        }

        params.push(self.func_param_factor()?);

        while self.peek_type() == TokenType::Comma {
            self.advance(); // consume ','
            params.push(self.func_param_factor()?);
        }

        Ok(params)
    }

    fn func_param_factor(&mut self) -> Result<FuncParam, ParseError> {
        let span = self.current_span();
        let name_tok = self.expect(TokenType::Word)?;
        self.expect(TokenType::Colon)?;
        let type_tok = self.expect(TokenType::Word)?;

        Ok(FuncParam {
            name: name_tok.value,
            type_name: type_tok.value,
            span,
        })
    }

    fn func_return_types(&mut self) -> Result<Vec<String>, ParseError> {
        let mut types = Vec::new();
        let first = self.expect(TokenType::Word)?;
        types.push(first.value);

        while self.peek_type() == TokenType::Comma {
            self.advance(); // consume ','
            // Check if the next token is actually a type word.
            // If we see LBrace here, the comma was part of the return type list end,
            // but actually the grammar says return types are separated by commas,
            // so we just keep parsing words.
            let t = self.expect(TokenType::Word)?;
            types.push(t.value);
        }

        Ok(types)
    }

    // ── VarStatement ─────────────────────────────────────────
    // var x: type   OR   var x = expr  OR  var a, b = e1, e2

    fn var_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::Var)?;
        let name_tok = self.expect(TokenType::Word)?;

        // Check next token to decide path
        let next = self.advance().clone();

        if next.token_type == TokenType::Colon {
            // var x: type
            let type_tok = self.expect(TokenType::Word)?;
            Ok(Statement::VarDecl(VarDecl::TypeDecl {
                name: name_tok.value,
                type_name: type_tok.value,
                span,
            }))
        } else {
            // var x = expr  OR  var x, y = expr1, expr2
            // Put back the token after WORD, and put back WORD too.
            // Then parse as assignment.
            self.back(); // put back whatever was after name
            self.back(); // put back WORD (name)
            let assign = self.assignment_statement()?;
            Ok(Statement::VarDecl(VarDecl::Assignment(assign)))
        }
    }

    // ── AssignmentStatement ──────────────────────────────────
    // word (, word)* = expr (, expr)*

    fn assignment_statement(&mut self) -> Result<Assignment, ParseError> {
        let span = self.current_span();
        let mut targets = Vec::new();

        let first = self.expect(TokenType::Word)?;
        targets.push(first.value);

        while self.peek_type() == TokenType::Comma {
            self.advance(); // consume ','
            let t = self.expect(TokenType::Word)?;
            targets.push(t.value);
        }

        self.expect(TokenType::Assignment)?;

        let values = self.more_arithmetic_expressions()?;

        Ok(Assignment {
            targets,
            values,
            span,
        })
    }

    /// Parse comma-separated list of arithmetic expressions.
    fn more_arithmetic_expressions(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut exprs = Vec::new();
        exprs.push(self.arithmetic_expression()?);

        while self.peek_type() == TokenType::Comma {
            self.advance(); // consume ','
            exprs.push(self.arithmetic_expression()?);
        }

        Ok(exprs)
    }

    // ── IFStatement ──────────────────────────────────────────
    // if (bool) { block } elseif (bool) { block } else { block }

    fn if_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::If)?;
        self.expect(TokenType::LParen)?;
        let condition = self.boolean_expression()?;
        self.expect(TokenType::RParen)?;

        self.skip_newlines();
        let body = self.block_statement()?;

        let mut else_if = Vec::new();
        let mut else_body = None;

        // Parse elseif / else chain
        loop {
            self.skip_newlines();
            if self.peek_type() == TokenType::ElseIf {
                let eif_span = self.current_span();
                self.advance(); // consume 'elseif'
                self.expect(TokenType::LParen)?;
                let eif_cond = self.boolean_expression()?;
                self.expect(TokenType::RParen)?;
                self.skip_newlines();
                let eif_body = self.block_statement()?;
                else_if.push(ElseIfBranch {
                    condition: eif_cond,
                    body: eif_body,
                    span: eif_span,
                });
            } else if self.peek_type() == TokenType::Else {
                self.advance(); // consume 'else'
                self.skip_newlines();
                let eb = self.block_statement()?;
                else_body = Some(eb);
                break;
            } else {
                break;
            }
        }

        Ok(Statement::IfStmt(IfStmt {
            condition,
            body,
            else_if,
            else_body,
            span,
        }))
    }

    // ── ForStatement ─────────────────────────────────────────
    // for (init; cond; step) { body }

    fn for_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::For)?;
        self.expect(TokenType::LParen)?;

        // init (optional)
        let init = if self.peek_type() == TokenType::Semicolon {
            None
        } else {
            Some(Box::new(self.for_init_or_step()?))
        };

        self.expect(TokenType::Semicolon)?;

        // condition (optional)
        let condition = if self.peek_type() == TokenType::Semicolon {
            None
        } else {
            Some(self.boolean_expression()?)
        };

        self.expect(TokenType::Semicolon)?;

        // step (optional)
        let step = if self.peek_type() == TokenType::RParen {
            None
        } else {
            Some(Box::new(self.for_init_or_step()?))
        };

        self.expect(TokenType::RParen)?;
        self.skip_newlines();
        let body = self.block_statement()?;

        Ok(Statement::ForStmt(ForStmt {
            init,
            condition,
            step,
            body,
            span,
        }))
    }

    /// Parse for-loop init or step: either VarStatement or AssignmentStatement.
    fn for_init_or_step(&mut self) -> Result<Statement, ParseError> {
        if self.peek_type() == TokenType::Var {
            self.var_statement()
        } else {
            let assign = self.assignment_statement()?;
            Ok(Statement::Assignment(assign))
        }
    }

    // ── BlockStatement ───────────────────────────────────────
    // { statement_list }

    fn block_statement(&mut self) -> Result<Block, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::LBrace)?;

        let mut stmts = Vec::new();
        self.skip_newlines();

        while self.peek_type() != TokenType::RBrace {
            if self.is_at_end() {
                return Err(self.error_at(span, "unclosed block, expected '}'".to_string()));
            }
            let stmt = self.block_statement_factor()?;
            stmts.push(stmt);
            self.skip_newlines();
        }

        self.expect(TokenType::RBrace)?;

        Ok(Block {
            statements: stmts,
            span,
        })
    }

    fn block_statement_as_stmt(&mut self) -> Result<Statement, ParseError> {
        let block = self.block_statement()?;
        Ok(Statement::Block(block))
    }

    /// Parse a single statement inside a block. Allows more statement types
    /// than top-level (return, break, continue).
    fn block_statement_factor(&mut self) -> Result<Statement, ParseError> {
        match self.peek_type() {
            TokenType::Var => self.var_statement(),
            TokenType::If => self.if_statement(),
            TokenType::For => self.for_statement(),
            TokenType::Return => self.return_statement(),
            TokenType::Break => self.break_statement(),
            TokenType::Continue => self.continue_statement(),
            TokenType::LBrace => self.block_statement_as_stmt(),
            TokenType::Timer => self.timer_statement(),
            TokenType::Word => self.word_dispatch_statement(),
            _ => {
                let tok = self.current().clone();
                Err(self.error_at(
                    tok.span,
                    format!(
                        "unexpected '{}', expected statement inside block ->BlockStatement_Factor",
                        tok.value
                    ),
                ))
            }
        }
    }

    // ── CallFuncStatement ────────────────────────────────────
    // name(arg1, arg2, ...)

    fn call_func_statement(&mut self) -> Result<CallFunc, ParseError> {
        let span = self.current_span();
        let name_tok = self.expect(TokenType::Word)?;
        self.expect(TokenType::LParen)?;

        let mut args = Vec::new();

        if self.peek_type() != TokenType::RParen {
            args.push(self.arithmetic_expression()?);
            while self.peek_type() == TokenType::Comma {
                self.advance(); // consume ','
                args.push(self.arithmetic_expression()?);
            }
        }

        self.expect(TokenType::RParen)?;

        Ok(CallFunc {
            name: name_tok.value,
            args,
            span,
        })
    }

    // ── ReturnStatement ──────────────────────────────────────
    // return expr1, expr2

    fn return_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::Return)?;

        let mut values = Vec::new();

        // Check if return is empty (followed by newline, }, or EOF)
        if self.peek_type() == TokenType::Newline
            || self.peek_type() == TokenType::RBrace
            || self.peek_type() == TokenType::Eof
        {
            return Ok(Statement::Return(ReturnStmt { values, span }));
        }

        values.push(self.arithmetic_expression()?);
        while self.peek_type() == TokenType::Comma {
            self.advance(); // consume ','
            values.push(self.arithmetic_expression()?);
        }

        Ok(Statement::Return(ReturnStmt { values, span }))
    }

    // ── Break / Continue ─────────────────────────────────────

    fn break_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::Break)?;
        Ok(Statement::Break(span))
    }

    fn continue_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::Continue)?;
        Ok(Statement::Continue(span))
    }

    // ── TimerStatement ───────────────────────────────────────
    // timer { body }

    fn timer_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::Timer)?;
        self.skip_newlines();
        let body = self.block_statement()?;
        Ok(Statement::TimerStmt(TimerStmt { body, span }))
    }

    // ── Boolean Expression ───────────────────────────────────
    // factor ( && | || factor )*

    fn boolean_expression(&mut self) -> Result<BooleanExpr, ParseError> {
        let mut left = self.boolean_expression_factor()?;

        loop {
            let op = match self.peek_type() {
                TokenType::Also => LogicalOp::And,
                TokenType::Perhaps => LogicalOp::Or,
                _ => break,
            };
            let span = self.current_span();
            self.advance(); // consume && or ||
            let right = self.boolean_expression_factor()?;
            left = BooleanExpr::Logical {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    fn boolean_expression_factor(&mut self) -> Result<BooleanExpr, ParseError> {
        // Check if it starts with '(' -- could be grouped boolean or arithmetic
        if self.peek_type() == TokenType::LParen {
            // Try to detect if this is a grouped boolean expression.
            // Heuristic: save position, parse inner as boolean, check for ')'.
            // If that fails, fall back to arithmetic comparison.
            let saved = self.pos;
            self.advance(); // consume '('
            if let Ok(inner) = self.boolean_expression() {
                if self.peek_type() == TokenType::RParen {
                    self.advance(); // consume ')'
                    // Check what follows: if it's a comparison op, this was actually
                    // an arithmetic expression in parens and we need to re-parse.
                    match self.peek_type() {
                        TokenType::Gt | TokenType::Lt | TokenType::GtEq
                        | TokenType::LtEq | TokenType::Eq | TokenType::NotEq => {
                            // This was actually (arithmetic_expr) op arithmetic_expr
                            // Restore and fall through to comparison parsing
                            self.pos = saved;
                        }
                        _ => {
                            return Ok(BooleanExpr::Grouped(Box::new(inner)));
                        }
                    }
                } else {
                    // Failed to match ')' -- restore and try as comparison
                    self.pos = saved;
                }
            } else {
                self.pos = saved;
            }
        }

        // comparison: expr op expr
        let span = self.current_span();
        let left = self.arithmetic_expression()?;
        let op = self.comparison_op()?;
        let right = self.arithmetic_expression()?;

        Ok(BooleanExpr::Comparison {
            left,
            op,
            right,
            span,
        })
    }

    fn comparison_op(&mut self) -> Result<ComparisonOp, ParseError> {
        let tok = self.advance().clone();
        match tok.token_type {
            TokenType::Gt => Ok(ComparisonOp::Gt),
            TokenType::Lt => Ok(ComparisonOp::Lt),
            TokenType::GtEq => Ok(ComparisonOp::GtEq),
            TokenType::LtEq => Ok(ComparisonOp::LtEq),
            TokenType::Eq => Ok(ComparisonOp::Eq),
            TokenType::NotEq => Ok(ComparisonOp::NotEq),
            _ => Err(self.error_at(
                tok.span,
                format!(
                    "expected comparison operator (> < >= <= == !=), found '{}'",
                    tok.value
                ),
            )),
        }
    }

    // ── Arithmetic Expression ────────────────────────────────
    // Expression -> Term ((+|-) Term)*

    fn arithmetic_expression(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.arithmetic_term()?;

        loop {
            let op = match self.peek_type() {
                TokenType::Add => BinaryOp::Add,
                TokenType::Sub => BinaryOp::Sub,
                _ => break,
            };
            let span = self.current_span();
            self.advance(); // consume + or -
            let right = self.arithmetic_term()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    // Term -> Factor ((*|/|^|%|~) Factor)*

    fn arithmetic_term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.arithmetic_factor()?;

        loop {
            let op = match self.peek_type() {
                TokenType::Mul => BinaryOp::Mul,
                TokenType::Div => BinaryOp::Div,
                TokenType::Power => BinaryOp::Power,
                TokenType::Mod => BinaryOp::Mod,
                TokenType::Rand => BinaryOp::Rand,
                _ => break,
            };
            let span = self.current_span();
            self.advance(); // consume operator
            let right = self.arithmetic_factor()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    // Factor -> NUM | WORD | TRUE | FALSE | STRING
    //         | WORD++ | WORD-- | WORD(args) -- call
    //         | (Expression)
    //         | |params| => body   -- closure
    //         | || => body         -- zero-param closure
    //         | { key: value, ... } -- table literal
    // Postfix: .field | ["key"]

    fn arithmetic_factor(&mut self) -> Result<Expr, ParseError> {
        let tok = self.advance().clone();

        let mut result = match tok.token_type {
            TokenType::Number => Ok(Expr::Number(tok.value, tok.span)),

            TokenType::StringLit => Ok(Expr::StringLit(tok.value, tok.span)),

            TokenType::True => Ok(Expr::Bool(true, tok.span)),

            TokenType::False => Ok(Expr::Bool(false, tok.span)),

            TokenType::LParen => {
                let inner = self.arithmetic_expression()?;
                self.expect(TokenType::RParen)?;
                Ok(Expr::Grouped(Box::new(inner)))
            }

            // { key: value, ... } -- table literal
            TokenType::LBrace => {
                self.back(); // put back {
                self.parse_table_literal()
            }

            // || => body  (zero-param closure)
            TokenType::Perhaps => {
                self.back(); // put back ||
                self.parse_closure()
            }

            // |params| => body  (closure with params)
            TokenType::Or => {
                self.back(); // put back |
                self.parse_closure()
            }

            TokenType::Word => {
                // Look ahead for ++ / -- / (
                match self.peek_type() {
                    TokenType::AddSelf => {
                        self.advance(); // consume ++
                        Ok(Expr::UnaryOp {
                            op: UnaryOp::Increment,
                            operand: tok.value,
                            span: tok.span,
                        })
                    }
                    TokenType::SubSelf => {
                        self.advance(); // consume --
                        Ok(Expr::UnaryOp {
                            op: UnaryOp::Decrement,
                            operand: tok.value,
                            span: tok.span,
                        })
                    }
                    TokenType::LParen => {
                        // Function call in expression context
                        self.back(); // put back WORD
                        let call = self.call_func_statement()?;
                        Ok(Expr::CallFunc(call))
                    }
                    _ => Ok(Expr::Variable(tok.value, tok.span)),
                }
            }

            _ => Err(self.error_at(
                tok.span,
                format!(
                    "unexpected '{}', expected number/word/true/false/string/(/{{ ->Factor",
                    tok.value
                ),
            )),
        }?;

        // Postfix operators: .field, ["key"], and (args) for method calls (supports chaining)
        loop {
            match self.peek_type() {
                TokenType::Dot => {
                    let span = self.current_span();
                    self.advance(); // consume .
                    let field_tok = self.expect(TokenType::Word)?;
                    result = Expr::FieldAccess(FieldAccess {
                        object: Box::new(result),
                        field: field_tok.value,
                        span,
                    });
                }
                TokenType::LBracket => {
                    let span = self.current_span();
                    self.advance(); // consume [
                    let index = self.arithmetic_expression()?;
                    self.expect(TokenType::RBracket)?;
                    result = Expr::IndexAccess(IndexAccess {
                        object: Box::new(result),
                        index: Box::new(index),
                        span,
                    });
                }
                TokenType::LParen => {
                    // Postfix call: expr(args) — only triggers after .field or ["key"]
                    // Plain word(args) is handled above in the Word+LParen branch
                    if matches!(result, Expr::FieldAccess(_) | Expr::IndexAccess(_) | Expr::MethodCall(_)) {
                        let span = self.current_span();
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
                        result = Expr::MethodCall(MethodCall {
                            callee: Box::new(result),
                            args,
                            span,
                        });
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        Ok(result)
    }

    // ── Table literal parsing ──────────────────────────────
    // { key: value, key: value, ... }

    fn parse_table_literal(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::LBrace)?;
        self.skip_newlines();

        let mut entries = Vec::new();

        if self.peek_type() != TokenType::RBrace {
            loop {
                self.skip_newlines();
                let key_tok = self.expect(TokenType::Word)?;
                self.expect(TokenType::Colon)?;
                let value = self.arithmetic_expression()?;
                entries.push(TableEntry {
                    key: key_tok.value,
                    value,
                });
                self.skip_newlines();
                if self.peek_type() == TokenType::Comma {
                    self.advance(); // consume ,
                    self.skip_newlines();
                    // Allow trailing comma
                    if self.peek_type() == TokenType::RBrace {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        self.skip_newlines();
        self.expect(TokenType::RBrace)?;

        Ok(Expr::TableLiteral(TableLiteral { entries, span }))
    }

    // ── Closure parsing ────────────────────────────────────
    // |params| => expr   OR   |params| => { block }
    // || => expr          OR   || => { block }

    fn parse_closure(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        let tok = self.advance().clone();

        let params = if tok.token_type == TokenType::Perhaps {
            // || — zero parameters
            vec![]
        } else {
            // | — parse parameter list until closing |
            let mut params = Vec::new();
            // Check for immediate closing |
            if self.peek_type() != TokenType::Or {
                loop {
                    let name_tok = self.expect(TokenType::Word)?;
                    let type_name = if self.peek_type() == TokenType::Colon {
                        self.advance(); // consume :
                        let ty = self.expect(TokenType::Word)?;
                        Some(ty.value)
                    } else {
                        None
                    };
                    params.push(ClosureParam {
                        name: name_tok.value,
                        type_name,
                    });
                    if self.peek_type() == TokenType::Comma {
                        self.advance(); // consume ,
                    } else {
                        break;
                    }
                }
            }
            self.expect(TokenType::Or)?; // closing |
            params
        };

        // Expect =>
        self.expect(TokenType::FatArrow)?;

        // Parse body: { block } or single expression
        let body = if self.peek_type() == TokenType::LBrace {
            let block = self.block_statement()?;
            ClosureBody::Block(block)
        } else {
            let expr = self.arithmetic_expression()?;
            ClosureBody::Expr(Box::new(expr))
        };

        Ok(Expr::Closure(ClosureExpr {
            params,
            body,
            span,
        }))
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

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anehta_lexer::Lexer;

    fn parse_source(src: &str) -> Result<Program, ParseError> {
        let tokens = Lexer::new(src).tokenize().expect("lexer should succeed");
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    fn parse_ok(src: &str) -> Program {
        parse_source(src).expect("parse should succeed")
    }

    // ── Arithmetic expression priority ──────────────────────

    #[test]
    fn simple_arithmetic_add() {
        // We need to test via assignment: var x = 1 + 2
        let prog = parse_ok("var x = 1 + 2");
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                assert_eq!(a.targets, vec!["x"]);
                assert_eq!(a.values.len(), 1);
                match &a.values[0] {
                    Expr::BinaryOp { op, .. } => {
                        assert!(matches!(op, BinaryOp::Add));
                    }
                    _ => panic!("expected BinaryOp"),
                }
            }
            _ => panic!("expected VarDecl Assignment"),
        }
    }

    #[test]
    fn arithmetic_precedence_mul_over_add() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3)
        let prog = parse_ok("var x = 1 + 2 * 3");
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                match &a.values[0] {
                    Expr::BinaryOp { left, op, right, .. } => {
                        assert!(matches!(op, BinaryOp::Add));
                        // left should be Number(1)
                        assert!(matches!(left.as_ref(), Expr::Number(v, _) if v == "1"));
                        // right should be BinaryOp(Mul, 2, 3)
                        match right.as_ref() {
                            Expr::BinaryOp { op: inner_op, .. } => {
                                assert!(matches!(inner_op, BinaryOp::Mul));
                            }
                            _ => panic!("expected inner BinaryOp for mul"),
                        }
                    }
                    _ => panic!("expected BinaryOp"),
                }
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn arithmetic_with_parens() {
        let prog = parse_ok("var x = (1 + 2) * 3");
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                match &a.values[0] {
                    Expr::BinaryOp { left, op, .. } => {
                        assert!(matches!(op, BinaryOp::Mul));
                        assert!(matches!(left.as_ref(), Expr::Grouped(_)));
                    }
                    _ => panic!("expected BinaryOp"),
                }
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn random_operator() {
        let prog = parse_ok("var x = 1 ~ 6");
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                match &a.values[0] {
                    Expr::BinaryOp { op, .. } => {
                        assert!(matches!(op, BinaryOp::Rand));
                    }
                    _ => panic!("expected BinaryOp with Rand"),
                }
            }
            _ => panic!("expected VarDecl"),
        }
    }

    // ── Variable declaration ────────────────────────────────

    #[test]
    fn var_type_decl() {
        let prog = parse_ok("var x: int");
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::TypeDecl { name, type_name, .. }) => {
                assert_eq!(name, "x");
                assert_eq!(type_name, "int");
            }
            _ => panic!("expected VarDecl TypeDecl"),
        }
    }

    #[test]
    fn var_assignment() {
        let prog = parse_ok("var x = 42");
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                assert_eq!(a.targets, vec!["x"]);
                assert_eq!(a.values.len(), 1);
                assert!(matches!(&a.values[0], Expr::Number(v, _) if v == "42"));
            }
            _ => panic!("expected VarDecl Assignment"),
        }
    }

    #[test]
    fn var_multi_assignment() {
        let prog = parse_ok("var a, b = 1, 2");
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                assert_eq!(a.targets, vec!["a", "b"]);
                assert_eq!(a.values.len(), 2);
            }
            _ => panic!("expected VarDecl Assignment"),
        }
    }

    // ── Function declaration ────────────────────────────────

    #[test]
    fn func_decl_simple() {
        let prog = parse_ok("func add(a: int, b: int) -> int {\nreturn a + b\n}");
        match &prog.statements[0] {
            Statement::FuncDecl(f) => {
                assert_eq!(f.name, "add");
                assert_eq!(f.params.len(), 2);
                assert_eq!(f.params[0].name, "a");
                assert_eq!(f.params[0].type_name, "int");
                assert_eq!(f.params[1].name, "b");
                assert_eq!(f.params[1].type_name, "int");
                assert_eq!(f.return_types, vec!["int"]);
                assert_eq!(f.body.statements.len(), 1);
            }
            _ => panic!("expected FuncDecl"),
        }
    }

    #[test]
    fn func_decl_multi_return() {
        let prog = parse_ok("func swap(a: int, b: int) -> int, int {\nreturn b, a\n}");
        match &prog.statements[0] {
            Statement::FuncDecl(f) => {
                assert_eq!(f.name, "swap");
                assert_eq!(f.return_types, vec!["int", "int"]);
                match &f.body.statements[0] {
                    Statement::Return(r) => {
                        assert_eq!(r.values.len(), 2);
                    }
                    _ => panic!("expected Return"),
                }
            }
            _ => panic!("expected FuncDecl"),
        }
    }

    #[test]
    fn func_decl_empty_body() {
        let prog = parse_ok("func noop() -> int {\n}");
        match &prog.statements[0] {
            Statement::FuncDecl(f) => {
                assert_eq!(f.name, "noop");
                assert!(f.params.is_empty());
                assert!(f.body.statements.is_empty());
            }
            _ => panic!("expected FuncDecl"),
        }
    }

    // ── If / elseif / else ──────────────────────────────────

    #[test]
    fn if_simple() {
        let prog = parse_ok("if (x > 10) {\ny = 1\n}");
        match &prog.statements[0] {
            Statement::IfStmt(i) => {
                assert!(i.else_if.is_empty());
                assert!(i.else_body.is_none());
            }
            _ => panic!("expected IfStmt"),
        }
    }

    #[test]
    fn if_elseif_else() {
        let src = "if (x > 10) {\ny = 1\n} elseif (x > 5) {\ny = 2\n} else {\ny = 0\n}";
        let prog = parse_ok(src);
        match &prog.statements[0] {
            Statement::IfStmt(i) => {
                assert_eq!(i.else_if.len(), 1);
                assert!(i.else_body.is_some());
            }
            _ => panic!("expected IfStmt"),
        }
    }

    #[test]
    fn if_complex_boolean() {
        let src = "if ((30+4>4+4+5&&x>3)&&(30>2)) {\n}";
        let prog = parse_ok(src);
        assert!(matches!(&prog.statements[0], Statement::IfStmt(_)));
    }

    // ── For loop ────────────────────────────────────────────

    #[test]
    fn for_loop_standard() {
        let src = "for (var i = 0; i < 100; i = i + 1) {\nvar x = i\n}";
        let prog = parse_ok(src);
        match &prog.statements[0] {
            Statement::ForStmt(f) => {
                assert!(f.init.is_some());
                assert!(f.condition.is_some());
                assert!(f.step.is_some());
                assert_eq!(f.body.statements.len(), 1);
            }
            _ => panic!("expected ForStmt"),
        }
    }

    #[test]
    fn for_loop_infinite() {
        let src = "for (;;) {\nbreak\n}";
        let prog = parse_ok(src);
        match &prog.statements[0] {
            Statement::ForStmt(f) => {
                assert!(f.init.is_none());
                assert!(f.condition.is_none());
                assert!(f.step.is_none());
                assert_eq!(f.body.statements.len(), 1);
                assert!(matches!(&f.body.statements[0], Statement::Break(_)));
            }
            _ => panic!("expected ForStmt"),
        }
    }

    // ── Function call ───────────────────────────────────────

    #[test]
    fn call_no_args() {
        let prog = parse_ok("foo()");
        match &prog.statements[0] {
            Statement::CallFunc(c) => {
                assert_eq!(c.name, "foo");
                assert!(c.args.is_empty());
            }
            _ => panic!("expected CallFunc"),
        }
    }

    #[test]
    fn call_with_args() {
        let prog = parse_ok("print(1, 2, 3)");
        match &prog.statements[0] {
            Statement::CallFunc(c) => {
                assert_eq!(c.name, "print");
                assert_eq!(c.args.len(), 3);
            }
            _ => panic!("expected CallFunc"),
        }
    }

    // ── Break / Continue ────────────────────────────────────

    #[test]
    fn break_and_continue_in_loop() {
        let src = "for (;;) {\nbreak\ncontinue\n}";
        let prog = parse_ok(src);
        match &prog.statements[0] {
            Statement::ForStmt(f) => {
                assert_eq!(f.body.statements.len(), 2);
                assert!(matches!(&f.body.statements[0], Statement::Break(_)));
                assert!(matches!(&f.body.statements[1], Statement::Continue(_)));
            }
            _ => panic!("expected ForStmt"),
        }
    }

    // ── Go test complete code snippet ───────────────────────

    #[test]
    fn go_test_full_program() {
        let src = r#"
var fuck = 10

if ((30+4>4+4+5&&fuck>3)&&(30>2)){

}elseif((30+4>4+4+5&&fuck>3)&&(30>2)){
    var i = 0
}

func fucker(wokao: int) -> int,int{
    return 1,2
}
var first,second = fucker(1,2,3)

fuck = 100+2*3-4^5+0~100

for (var i = 100;i<100;i = i + 1){
    if ((30+4>4+4+5&&fuck>3)&&(30>2)){

    }elseif((30+4>4+4+5&&fuck>3+1)&&(30>2)){
        var i = 0
        for (var i = 100;i<100;i = i + 1){
            if ((30+4>4+4+5&&fuck>3)&&(30>2)){
                break
            }elseif((30+4>4+4+5&&fuck>3)&&(30>2)){
                var i = 0
            }
        }
    }
}

func wocao (wocao: int,wocao: int) -> int{

}

for (;;){

}
"#;
        let prog = parse_ok(src);
        // Should have parsed multiple top-level statements
        assert!(prog.statements.len() >= 7, "expected at least 7 top-level statements, got {}", prog.statements.len());

        // Verify statement types
        assert!(matches!(&prog.statements[0], Statement::VarDecl(_))); // var fuck = 10
        assert!(matches!(&prog.statements[1], Statement::IfStmt(_))); // if ...
        assert!(matches!(&prog.statements[2], Statement::FuncDecl(_))); // func fucker
        assert!(matches!(&prog.statements[3], Statement::VarDecl(_))); // var first, second = ...
        assert!(matches!(&prog.statements[4], Statement::Assignment(_))); // fuck = ...
        assert!(matches!(&prog.statements[5], Statement::ForStmt(_))); // for (...)
        assert!(matches!(&prog.statements[6], Statement::FuncDecl(_))); // func wocao
        assert!(matches!(&prog.statements[7], Statement::ForStmt(_))); // for (;;)
    }

    // ── Expression with function call ───────────────────────

    #[test]
    fn expression_with_call() {
        let prog = parse_ok("var x = add(1, 2) + 3");
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                match &a.values[0] {
                    Expr::BinaryOp { left, op, .. } => {
                        assert!(matches!(op, BinaryOp::Add));
                        assert!(matches!(left.as_ref(), Expr::CallFunc(_)));
                    }
                    _ => panic!("expected BinaryOp"),
                }
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn increment_decrement() {
        // i++ as an expression factor (not as a for-loop step statement)
        let prog = parse_ok("var x = i++");
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                match &a.values[0] {
                    Expr::UnaryOp { op, operand, .. } => {
                        assert!(matches!(op, UnaryOp::Increment));
                        assert_eq!(operand, "i");
                    }
                    _ => panic!("expected UnaryOp"),
                }
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn string_literal_in_var() {
        let prog = parse_ok(r#"var name = "Anehta""#);
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                assert_eq!(a.targets, vec!["name"]);
                match &a.values[0] {
                    Expr::StringLit(s, _) => assert_eq!(s, "Anehta"),
                    _ => panic!("expected StringLit"),
                }
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn complex_expression() {
        // 100+2*3-4^5+0~100
        let prog = parse_ok("var x = 100+2*3-4^5+0~100");
        match &prog.statements[0] {
            Statement::VarDecl(VarDecl::Assignment(a)) => {
                // The expression should parse without error
                assert_eq!(a.values.len(), 1);
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn nested_for_and_if() {
        let src = r#"for (var i = 0; i < 10; i = i + 1) {
    if (i > 5) {
        break
    }
    for (var j = 0; j < i; j = j + 1) {
        continue
    }
}"#;
        let prog = parse_ok(src);
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::ForStmt(f) => {
                assert_eq!(f.body.statements.len(), 2);
                assert!(matches!(&f.body.statements[0], Statement::IfStmt(_)));
                assert!(matches!(&f.body.statements[1], Statement::ForStmt(_)));
            }
            _ => panic!("expected ForStmt"),
        }
    }

    #[test]
    fn bool_expression_eq_neq() {
        let src = "if (x == 10) {\n}";
        let prog = parse_ok(src);
        match &prog.statements[0] {
            Statement::IfStmt(i) => {
                match &i.condition {
                    BooleanExpr::Comparison { op, .. } => {
                        assert!(matches!(op, ComparisonOp::Eq));
                    }
                    _ => panic!("expected Comparison"),
                }
            }
            _ => panic!("expected IfStmt"),
        }
    }

    #[test]
    fn multi_statement_program() {
        let src = r#"var health = 100
var name = "Anehta"
if (health > 80) {
    health = health - 1
}"#;
        let prog = parse_ok(src);
        assert_eq!(prog.statements.len(), 3);
    }
}
