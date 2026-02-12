use super::*;

impl Parser {
    // ── Boolean Expression ───────────────────────────────────
    // factor ( && | || factor )*

    pub(super) fn boolean_expression(&mut self) -> Result<BooleanExpr, ParseError> {
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
}
