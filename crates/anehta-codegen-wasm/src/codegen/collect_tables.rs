use super::*;

impl WasmCodegen {
    /// Pre-pass: walk the program and register TableTypeInfo for each table literal.
    /// This enables compile-time type inference for field accesses (e.g. print dispatch).
    pub(super) fn collect_table_types(&mut self, program: &Program) {
        for stmt in &program.statements {
            self.collect_table_types_stmt(stmt);
        }
    }

    fn collect_table_types_stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::VarDecl(VarDecl::Assignment(assign)) | Statement::Assignment(assign) => {
                for val in &assign.values {
                    self.collect_table_types_expr(val);
                }
            }
            Statement::IfStmt(if_stmt) => {
                self.collect_table_types_block(&if_stmt.body);
                for branch in &if_stmt.else_if {
                    self.collect_table_types_block(&branch.body);
                }
                if let Some(else_body) = &if_stmt.else_body {
                    self.collect_table_types_block(else_body);
                }
            }
            Statement::ForStmt(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    self.collect_table_types_stmt(init);
                }
                if let Some(step) = &for_stmt.step {
                    self.collect_table_types_stmt(step);
                }
                self.collect_table_types_block(&for_stmt.body);
            }
            Statement::Block(block) => {
                self.collect_table_types_block(block);
            }
            Statement::Return(ret) => {
                for val in &ret.values {
                    self.collect_table_types_expr(val);
                }
            }
            Statement::CallFunc(call) => {
                for arg in &call.args {
                    self.collect_table_types_expr(arg);
                }
            }
            Statement::FuncDecl(func) => {
                self.collect_table_types_block(&func.body);
            }
            Statement::TimerStmt(timer) => {
                self.collect_table_types_block(&timer.body);
            }
            Statement::FieldAssign(fa) => {
                self.collect_table_types_expr(&fa.value);
            }
            Statement::IndexAssign(ia) => {
                self.collect_table_types_expr(&ia.index);
                self.collect_table_types_expr(&ia.value);
            }
            Statement::MethodCall(mc) => {
                self.collect_table_types_expr(&mc.callee);
                for arg in &mc.args {
                    self.collect_table_types_expr(arg);
                }
            }
            _ => {}
        }
    }

    fn collect_table_types_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.collect_table_types_stmt(stmt);
        }
    }

    fn collect_table_types_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::TableLiteral(table) => {
                // Recurse into values first (nested tables)
                for entry in &table.entries {
                    self.collect_table_types_expr(&entry.value);
                }
                // Build field type map from the literal
                let mut fields = HashMap::new();
                let tmp_ctx = FuncCtx::new();
                for entry in &table.entries {
                    let ty = self.infer_expr_type(&entry.value, &tmp_ctx);
                    fields.insert(entry.key.clone(), ty);
                }
                let id = self.table_types.len() as u32;
                self.table_types.push(TableTypeInfo { fields });
                self.table_type_span_map
                    .insert((table.span.line, table.span.column), id);
            }
            Expr::BinaryOp { left, right, .. } => {
                self.collect_table_types_expr(left);
                self.collect_table_types_expr(right);
            }
            Expr::CallFunc(call) => {
                for arg in &call.args {
                    self.collect_table_types_expr(arg);
                }
            }
            Expr::Grouped(inner) => {
                self.collect_table_types_expr(inner);
            }
            Expr::Closure(closure) => {
                match &closure.body {
                    ClosureBody::Expr(e) => self.collect_table_types_expr(e),
                    ClosureBody::Block(b) => self.collect_table_types_block(b),
                }
            }
            Expr::FieldAccess(fa) => {
                self.collect_table_types_expr(&fa.object);
            }
            Expr::IndexAccess(ia) => {
                self.collect_table_types_expr(&ia.object);
                self.collect_table_types_expr(&ia.index);
            }
            Expr::MethodCall(mc) => {
                self.collect_table_types_expr(&mc.callee);
                for arg in &mc.args {
                    self.collect_table_types_expr(arg);
                }
            }
            Expr::VecLiteral(vec_lit) => {
                for elem in &vec_lit.elements {
                    self.collect_table_types_expr(elem);
                }
            }
            Expr::MatLiteral(mat_lit) => {
                for row in &mat_lit.rows {
                    for elem in row {
                        self.collect_table_types_expr(elem);
                    }
                }
            }
            Expr::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.collect_table_types_expr(s);
                }
                if let Some(e) = end {
                    self.collect_table_types_expr(e);
                }
            }
            Expr::BooleanExpr(bool_expr) => {
                self.collect_table_types_boolean_expr(bool_expr);
            }
            _ => {}
        }
    }

    fn collect_table_types_boolean_expr(&mut self, expr: &BooleanExpr) {
        match expr {
            BooleanExpr::Comparison { left, right, .. } => {
                self.collect_table_types_expr(left);
                self.collect_table_types_expr(right);
            }
            BooleanExpr::Logical { left, right, .. } => {
                self.collect_table_types_boolean_expr(left);
                self.collect_table_types_boolean_expr(right);
            }
            BooleanExpr::Grouped(inner) => {
                self.collect_table_types_boolean_expr(inner);
            }
        }
    }

    /// Re-infer table field types after closures are collected.
    /// During collect_table_types (Phase 0b), closure variables weren't known yet,
    /// so fields like `{asd: readB}` where readB is a closure were typed as Int.
    /// Now that closures are collected, we can walk VarDecl assignments to fix this.
    pub(super) fn fixup_table_types(&mut self, program: &Program) {
        // Build a simple var_types map from top-level assignments
        let mut var_types = HashMap::new();
        self.fixup_table_types_stmts(&program.statements, &mut var_types);
    }

    fn fixup_table_types_stmts(&mut self, stmts: &[Statement], var_types: &mut HashMap<String, AhType>) {
        for stmt in stmts {
            match stmt {
                Statement::VarDecl(VarDecl::Assignment(assign)) | Statement::Assignment(assign) => {
                    // First pass: record variable types from this assignment
                    for (i, target) in assign.targets.iter().enumerate() {
                        if let Some(val) = assign.values.get(i) {
                            let ctx = FuncCtx::new_with_var_types(var_types.clone());
                            let ty = self.infer_expr_type(val, &ctx);
                            var_types.insert(target.clone(), ty);

                            // If value is a table literal, re-infer its field types
                            if let AhType::Table(id) = ty {
                                if let Expr::TableLiteral(table) = val {
                                    // Collect updates first to avoid borrow conflict
                                    let updates: Vec<(String, AhType)> = table.entries.iter().map(|entry| {
                                        let field_ty = self.infer_expr_type(&entry.value, &ctx);
                                        (entry.key.clone(), field_ty)
                                    }).collect();
                                    if let Some(info) = self.table_types.get_mut(id as usize) {
                                        for (key, field_ty) in updates {
                                            info.fields.insert(key, field_ty);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Statement::FuncDecl(func) => {
                    let mut inner_types = var_types.clone();
                    for param in &func.params {
                        inner_types.insert(param.name.clone(), type_name_to_ah(&param.type_name));
                    }
                    self.fixup_table_types_stmts(
                        &func.body.statements,
                        &mut inner_types,
                    );
                }
                Statement::IfStmt(if_stmt) => {
                    self.fixup_table_types_stmts(&if_stmt.body.statements, var_types);
                    for branch in &if_stmt.else_if {
                        self.fixup_table_types_stmts(&branch.body.statements, var_types);
                    }
                    if let Some(else_body) = &if_stmt.else_body {
                        self.fixup_table_types_stmts(&else_body.statements, var_types);
                    }
                }
                Statement::ForStmt(for_stmt) => {
                    if let Some(init) = &for_stmt.init {
                        self.fixup_table_types_stmts(std::slice::from_ref(init.as_ref()), var_types);
                    }
                    self.fixup_table_types_stmts(&for_stmt.body.statements, var_types);
                }
                Statement::Block(block) => {
                    self.fixup_table_types_stmts(&block.statements, var_types);
                }
                _ => {}
            }
        }
    }
}
