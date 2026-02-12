use super::*;

impl WasmCodegen {
    /// Intern a string literal and return its (offset, length) in the data segment.
    pub(super) fn intern_string(&mut self, s: &str) -> (u32, u32) {
        if let Some(&entry) = self.string_pool.get(s) {
            return entry;
        }
        let offset = self.string_data.len() as u32;
        let bytes = s.as_bytes();
        let len = bytes.len() as u32;
        self.string_data.extend_from_slice(bytes);
        self.string_pool.insert(s.to_string(), (offset, len));
        (offset, len)
    }

    /// Pre-pass: walk the entire program and intern every `Expr::StringLit`.
    pub(super) fn collect_strings(&mut self, program: &Program) {
        for stmt in &program.statements {
            self.collect_strings_stmt(stmt);
        }
    }

    fn collect_strings_stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::VarDecl(VarDecl::Assignment(assign)) => {
                self.collect_strings_assignment(assign);
            }
            Statement::Assignment(assign) => {
                self.collect_strings_assignment(assign);
            }
            Statement::IfStmt(if_stmt) => {
                self.collect_strings_boolean_expr(&if_stmt.condition);
                self.collect_strings_block(&if_stmt.body);
                for branch in &if_stmt.else_if {
                    self.collect_strings_boolean_expr(&branch.condition);
                    self.collect_strings_block(&branch.body);
                }
                if let Some(else_body) = &if_stmt.else_body {
                    self.collect_strings_block(else_body);
                }
            }
            Statement::ForStmt(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    self.collect_strings_stmt(init);
                }
                if let Some(cond) = &for_stmt.condition {
                    self.collect_strings_boolean_expr(cond);
                }
                if let Some(step) = &for_stmt.step {
                    self.collect_strings_stmt(step);
                }
                self.collect_strings_block(&for_stmt.body);
            }
            Statement::Block(block) => {
                self.collect_strings_block(block);
            }
            Statement::Return(ret) => {
                for val in &ret.values {
                    self.collect_strings_expr(val);
                }
            }
            Statement::CallFunc(call) => {
                for arg in &call.args {
                    self.collect_strings_expr(arg);
                }
            }
            Statement::FuncDecl(func) => {
                self.collect_strings_block(&func.body);
            }
            Statement::TimerStmt(timer) => {
                self.collect_strings_block(&timer.body);
            }
            Statement::FieldAssign(fa) => {
                self.intern_string(&fa.field);
                self.collect_strings_expr(&fa.value);
            }
            Statement::IndexAssign(ia) => {
                self.collect_strings_expr(&ia.index);
                self.collect_strings_expr(&ia.value);
            }
            Statement::MethodCall(mc) => {
                self.collect_strings_expr(&mc.callee);
                for arg in &mc.args {
                    self.collect_strings_expr(arg);
                }
            }
            _ => {}
        }
    }

    fn collect_strings_assignment(&mut self, assign: &Assignment) {
        for val in &assign.values {
            self.collect_strings_expr(val);
        }
    }

    fn collect_strings_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.collect_strings_stmt(stmt);
        }
    }

    fn collect_strings_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::StringLit(s, _) => {
                self.intern_string(s);
            }
            Expr::BinaryOp {
                left, right, ..
            } => {
                self.collect_strings_expr(left);
                self.collect_strings_expr(right);
            }
            Expr::CallFunc(call) => {
                for arg in &call.args {
                    self.collect_strings_expr(arg);
                }
            }
            Expr::Grouped(inner) => {
                self.collect_strings_expr(inner);
            }
            Expr::Closure(closure) => {
                match &closure.body {
                    ClosureBody::Expr(e) => self.collect_strings_expr(e),
                    ClosureBody::Block(b) => self.collect_strings_block(b),
                }
            }
            Expr::TableLiteral(table) => {
                for entry in &table.entries {
                    self.intern_string(&entry.key);
                    self.collect_strings_expr(&entry.value);
                }
            }
            Expr::FieldAccess(fa) => {
                self.intern_string(&fa.field);
                self.collect_strings_expr(&fa.object);
            }
            Expr::IndexAccess(ia) => {
                self.collect_strings_expr(&ia.object);
                self.collect_strings_expr(&ia.index);
            }
            Expr::MethodCall(mc) => {
                self.collect_strings_expr(&mc.callee);
                for arg in &mc.args {
                    self.collect_strings_expr(arg);
                }
            }
            Expr::VecLiteral(vec_lit) => {
                for elem in &vec_lit.elements {
                    self.collect_strings_expr(elem);
                }
            }
            Expr::MatLiteral(mat_lit) => {
                for row in &mat_lit.rows {
                    for elem in row {
                        self.collect_strings_expr(elem);
                    }
                }
            }
            Expr::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.collect_strings_expr(s);
                }
                if let Some(e) = end {
                    self.collect_strings_expr(e);
                }
            }
            Expr::BooleanExpr(bool_expr) => {
                self.collect_strings_boolean_expr(bool_expr);
            }
            _ => {}
        }
    }

    fn collect_strings_boolean_expr(&mut self, expr: &BooleanExpr) {
        match expr {
            BooleanExpr::Comparison { left, right, .. } => {
                self.collect_strings_expr(left);
                self.collect_strings_expr(right);
            }
            BooleanExpr::Logical { left, right, .. } => {
                self.collect_strings_boolean_expr(left);
                self.collect_strings_boolean_expr(right);
            }
            BooleanExpr::Grouped(inner) => {
                self.collect_strings_boolean_expr(inner);
            }
        }
    }
}
