use super::*;

impl Parser {
    // ── FuncStatement ────────────────────────────────────────
    // func name(params) -> return_types { body }

    pub(super) fn func_statement(&mut self) -> Result<Statement, ParseError> {
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

    pub(super) fn var_statement(&mut self) -> Result<Statement, ParseError> {
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

    pub(super) fn assignment_statement(&mut self) -> Result<Assignment, ParseError> {
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
    pub(super) fn more_arithmetic_expressions(&mut self) -> Result<Vec<Expr>, ParseError> {
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

    pub(super) fn if_statement(&mut self) -> Result<Statement, ParseError> {
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

    pub(super) fn for_statement(&mut self) -> Result<Statement, ParseError> {
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

    pub(super) fn block_statement(&mut self) -> Result<Block, ParseError> {
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

    pub(super) fn block_statement_as_stmt(&mut self) -> Result<Statement, ParseError> {
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

    pub(super) fn call_func_statement(&mut self) -> Result<CallFunc, ParseError> {
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

    pub(super) fn timer_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::Timer)?;
        self.skip_newlines();
        let body = self.block_statement()?;
        Ok(Statement::TimerStmt(TimerStmt { body, span }))
    }
}
