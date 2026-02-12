use super::*;

impl Parser {
    // ── Arithmetic Expression ────────────────────────────────
    // Expression -> Term ((+|-) Term)*

    pub(super) fn arithmetic_expression(&mut self) -> Result<Expr, ParseError> {
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

    // Term -> Factor ((*|/|^|.^|%|~|@|#|\) Factor)*

    fn arithmetic_term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.arithmetic_factor()?;

        loop {
            let op = match self.peek_type() {
                TokenType::Mul => BinaryOp::Mul,
                TokenType::Div => BinaryOp::Div,
                TokenType::Power => BinaryOp::Power,
                TokenType::DotPow => BinaryOp::DotPow,
                TokenType::Mod => BinaryOp::Mod,
                TokenType::Rand => BinaryOp::Rand,
                TokenType::At => BinaryOp::At,
                TokenType::Hash => BinaryOp::Hash,
                TokenType::Backslash => BinaryOp::Backslash,
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

            // [elem, elem, ...] -- vec literal
            TokenType::LBracket => {
                self.back(); // put back [
                self.parse_vec_literal()
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

                    // Check for range syntax: [start..end], [..end], [start..]
                    let index = if self.peek_type() == TokenType::Range {
                        // [..end] case
                        self.advance(); // consume ..
                        let end = if self.peek_type() == TokenType::RBracket {
                            None
                        } else {
                            Some(Box::new(self.arithmetic_expression()?))
                        };
                        Expr::Range { start: None, end, span }
                    } else {
                        let first = self.arithmetic_expression()?;
                        if self.peek_type() == TokenType::Range {
                            // [start..] or [start..end] case
                            self.advance(); // consume ..
                            let end = if self.peek_type() == TokenType::RBracket {
                                None
                            } else {
                                Some(Box::new(self.arithmetic_expression()?))
                            };
                            Expr::Range { start: Some(Box::new(first)), end, span }
                        } else if matches!(self.peek_type(),
                            TokenType::Gt | TokenType::Lt | TokenType::GtEq |
                            TokenType::LtEq | TokenType::Eq | TokenType::NotEq)
                        {
                            // Boolean expression for masking: v[v > 0]
                            let comp_op = match self.peek_type() {
                                TokenType::Gt => ComparisonOp::Gt,
                                TokenType::Lt => ComparisonOp::Lt,
                                TokenType::GtEq => ComparisonOp::GtEq,
                                TokenType::LtEq => ComparisonOp::LtEq,
                                TokenType::Eq => ComparisonOp::Eq,
                                TokenType::NotEq => ComparisonOp::NotEq,
                                _ => unreachable!(),
                            };
                            self.advance(); // consume comparison operator
                            let right = self.arithmetic_expression()?;
                            let bool_expr = BooleanExpr::Comparison {
                                left: first,
                                op: comp_op,
                                right,
                                span,
                            };
                            Expr::BooleanExpr(Box::new(bool_expr))
                        } else {
                            // Regular index [expr]
                            first
                        }
                    };

                    self.expect(TokenType::RBracket)?;
                    result = Expr::IndexAccess(IndexAccess {
                        object: Box::new(result),
                        index: Box::new(index),
                        span,
                    });
                }
                TokenType::Transpose => {
                    let span = self.current_span();
                    self.advance(); // consume '
                    result = Expr::Transpose(Transpose {
                        operand: Box::new(result),
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

    // ── Vec/Mat literal parsing ────────────────────────────────
    // Vec: [elem1, elem2, ...]
    // Mat: [row1_elem1, row1_elem2; row2_elem1, row2_elem2]

    fn parse_vec_literal(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.expect(TokenType::LBracket)?;

        // Parse first row
        let mut first_row = Vec::new();
        if self.peek_type() != TokenType::RBracket {
            loop {
                first_row.push(self.arithmetic_expression()?);
                if self.peek_type() == TokenType::Comma {
                    self.advance();
                    if self.peek_type() == TokenType::RBracket {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        // Check for semicolon (matrix) or closing bracket (vector)
        if self.peek_type() == TokenType::Semicolon {
            // Matrix literal
            let mut rows = vec![first_row];
            while self.peek_type() == TokenType::Semicolon {
                self.advance(); // consume ;
                let mut row = Vec::new();
                if self.peek_type() == TokenType::RBracket {
                    break; // trailing semicolon
                }
                loop {
                    row.push(self.arithmetic_expression()?);
                    if self.peek_type() == TokenType::Comma {
                        self.advance();
                        if self.peek_type() == TokenType::Semicolon || self.peek_type() == TokenType::RBracket {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                rows.push(row);
            }
            self.expect(TokenType::RBracket)?;
            Ok(Expr::MatLiteral(MatLiteral { rows, span }))
        } else {
            // Vector literal
            self.expect(TokenType::RBracket)?;
            Ok(Expr::VecLiteral(VecLiteral { elements: first_row, span }))
        }
    }
}
