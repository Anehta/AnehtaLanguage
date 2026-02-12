use super::*;

impl WasmCodegen {
    /// Walk the entire program AST and collect all closure expressions.
    /// For each closure, register a hidden WASM function and record its metadata.
    pub(super) fn collect_closures(&mut self, program: &Program) {
        // Collect all closures from statements (depth-first order)
        let stmts: Vec<&Statement> = program.statements.iter().collect();
        for stmt in stmts {
            self.collect_closures_stmt(stmt);
        }
    }

    fn collect_closures_stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::VarDecl(VarDecl::Assignment(assign)) | Statement::Assignment(assign) => {
                for val in &assign.values {
                    self.collect_closures_expr(val);
                }
            }
            Statement::VarDecl(VarDecl::TypeDecl { .. }) => {}
            Statement::IfStmt(if_stmt) => {
                self.collect_closures_boolean_expr(&if_stmt.condition);
                self.collect_closures_block(&if_stmt.body);
                for branch in &if_stmt.else_if {
                    self.collect_closures_boolean_expr(&branch.condition);
                    self.collect_closures_block(&branch.body);
                }
                if let Some(else_body) = &if_stmt.else_body {
                    self.collect_closures_block(else_body);
                }
            }
            Statement::ForStmt(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    self.collect_closures_stmt(init);
                }
                if let Some(cond) = &for_stmt.condition {
                    self.collect_closures_boolean_expr(cond);
                }
                if let Some(step) = &for_stmt.step {
                    self.collect_closures_stmt(step);
                }
                self.collect_closures_block(&for_stmt.body);
            }
            Statement::Block(block) => {
                self.collect_closures_block(block);
            }
            Statement::Return(ret) => {
                for val in &ret.values {
                    self.collect_closures_expr(val);
                }
            }
            Statement::CallFunc(call) => {
                for arg in &call.args {
                    self.collect_closures_expr(arg);
                }
            }
            Statement::FuncDecl(func) => {
                self.collect_closures_block(&func.body);
            }
            Statement::TimerStmt(timer) => {
                self.collect_closures_block(&timer.body);
            }
            Statement::FieldAssign(fa) => {
                self.collect_closures_expr(&fa.value);
            }
            Statement::IndexAssign(ia) => {
                self.collect_closures_expr(&ia.index);
                self.collect_closures_expr(&ia.value);
            }
            Statement::MethodCall(mc) => {
                self.collect_closures_expr(&mc.callee);
                for arg in &mc.args {
                    self.collect_closures_expr(arg);
                }
            }
            _ => {}
        }
    }

    fn collect_closures_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.collect_closures_stmt(stmt);
        }
    }

    fn collect_closures_boolean_expr(&mut self, expr: &BooleanExpr) {
        match expr {
            BooleanExpr::Comparison { left, right, .. } => {
                self.collect_closures_expr(left);
                self.collect_closures_expr(right);
            }
            BooleanExpr::Logical { left, right, .. } => {
                self.collect_closures_boolean_expr(left);
                self.collect_closures_boolean_expr(right);
            }
            BooleanExpr::Grouped(inner) => {
                self.collect_closures_boolean_expr(inner);
            }
        }
    }

    fn collect_closures_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Closure(closure) => {
                // First, recurse into the closure body to collect nested closures
                match &closure.body {
                    ClosureBody::Expr(e) => self.collect_closures_expr(e),
                    ClosureBody::Block(b) => self.collect_closures_block(b),
                }

                // Extract param names
                let param_names: HashSet<String> =
                    closure.params.iter().map(|p| p.name.clone()).collect();
                let param_count = closure.params.len();

                // Walk the closure body to find all variable references
                let mut referenced = HashSet::new();
                match &closure.body {
                    ClosureBody::Expr(e) => Self::find_variables_expr(e, &mut referenced),
                    ClosureBody::Block(b) => Self::find_variables_block(b, &mut referenced),
                }

                // Build the set of names that are NOT captures
                let mut non_captures = HashSet::new();
                non_captures.extend(param_names.iter().cloned());
                // Add all known functions
                for key in self.func_map.keys() {
                    non_captures.insert(key.clone());
                }
                // Add built-in names
                non_captures.insert("print".to_string());
                non_captures.insert("input".to_string());

                // Captures = referenced - non_captures
                // Also exclude variables declared inside the closure body
                let mut body_locals = HashSet::new();
                match &closure.body {
                    ClosureBody::Expr(_) => {}
                    ClosureBody::Block(b) => Self::find_declared_vars_block(b, &mut body_locals),
                }

                let mut captures: Vec<String> = referenced
                    .into_iter()
                    .filter(|name| !non_captures.contains(name) && !body_locals.contains(name))
                    .collect();
                captures.sort(); // deterministic order

                // Create the function type: (i32, i64 * param_count) -> i64
                let mut params = vec![ValType::I32]; // env_ptr
                for _ in 0..param_count {
                    params.push(ValType::I64);
                }
                let results = vec![ValType::I64];
                let type_idx = self.add_type(params, results);

                // Assign function index
                let func_idx = self.next_func_idx;
                self.next_func_idx += 1;

                let closure_id = self.closure_counter;
                self.closure_counter += 1;
                let table_idx = closure_id;

                let name = format!("__closure_{}", closure_id);
                self.func_map
                    .insert(name.clone(), (func_idx, type_idx));

                // Infer the return type of the closure body
                let return_type = {
                    let mut tmp_ctx = FuncCtx::new();
                    for p in &closure.params {
                        tmp_ctx.add_param(&p.name);
                        if let Some(ref tn) = p.type_name {
                            tmp_ctx.var_types.insert(p.name.clone(), type_name_to_ah(tn));
                        }
                    }
                    match &closure.body {
                        ClosureBody::Expr(e) => self.infer_expr_type(e, &tmp_ctx),
                        ClosureBody::Block(b) => {
                            // Scan for the first return statement
                            Self::infer_block_return_type(b, self, &tmp_ctx)
                        }
                    }
                };

                let info = ClosureInfo {
                    name,
                    func_idx,
                    type_idx,
                    captures,
                    param_count,
                    table_idx,
                    return_type,
                };
                self.closures.push(info);

                // Map the span to the closure ID
                self.closure_span_map
                    .insert((closure.span.line, closure.span.column), closure_id);
            }
            Expr::BinaryOp { left, right, .. } => {
                self.collect_closures_expr(left);
                self.collect_closures_expr(right);
            }
            Expr::CallFunc(call) => {
                for arg in &call.args {
                    self.collect_closures_expr(arg);
                }
            }
            Expr::Grouped(inner) => {
                self.collect_closures_expr(inner);
            }
            Expr::TableLiteral(table) => {
                for entry in &table.entries {
                    self.collect_closures_expr(&entry.value);
                }
            }
            Expr::FieldAccess(fa) => {
                self.collect_closures_expr(&fa.object);
            }
            Expr::IndexAccess(ia) => {
                self.collect_closures_expr(&ia.object);
                self.collect_closures_expr(&ia.index);
            }
            Expr::MethodCall(mc) => {
                self.collect_closures_expr(&mc.callee);
                for arg in &mc.args {
                    self.collect_closures_expr(arg);
                }
            }
            _ => {}
        }
    }

    /// Find all variable references in an expression (recursively).
    pub(super) fn find_variables_expr(expr: &Expr, vars: &mut HashSet<String>) {
        match expr {
            Expr::Variable(name, _) => {
                vars.insert(name.clone());
            }
            Expr::BinaryOp { left, right, .. } => {
                Self::find_variables_expr(left, vars);
                Self::find_variables_expr(right, vars);
            }
            Expr::CallFunc(call) => {
                // The call target name is not a variable reference for capture purposes
                for arg in &call.args {
                    Self::find_variables_expr(arg, vars);
                }
            }
            Expr::Grouped(inner) => {
                Self::find_variables_expr(inner, vars);
            }
            Expr::UnaryOp { operand, .. } => {
                vars.insert(operand.clone());
            }
            Expr::Closure(closure) => {
                // Variables inside a nested closure may also reference outer scope
                match &closure.body {
                    ClosureBody::Expr(e) => Self::find_variables_expr(e, vars),
                    ClosureBody::Block(b) => Self::find_variables_block(b, vars),
                }
            }
            Expr::TableLiteral(table) => {
                for entry in &table.entries {
                    Self::find_variables_expr(&entry.value, vars);
                }
            }
            Expr::FieldAccess(fa) => {
                Self::find_variables_expr(&fa.object, vars);
            }
            Expr::IndexAccess(ia) => {
                Self::find_variables_expr(&ia.object, vars);
                Self::find_variables_expr(&ia.index, vars);
            }
            Expr::MethodCall(mc) => {
                Self::find_variables_expr(&mc.callee, vars);
                for arg in &mc.args {
                    Self::find_variables_expr(arg, vars);
                }
            }
            _ => {}
        }
    }

    pub(super) fn find_variables_block(block: &Block, vars: &mut HashSet<String>) {
        for stmt in &block.statements {
            Self::find_variables_stmt(stmt, vars);
        }
    }

    fn find_variables_stmt(stmt: &Statement, vars: &mut HashSet<String>) {
        match stmt {
            Statement::VarDecl(VarDecl::Assignment(assign)) | Statement::Assignment(assign) => {
                for val in &assign.values {
                    Self::find_variables_expr(val, vars);
                }
            }
            Statement::IfStmt(if_stmt) => {
                Self::find_variables_boolean_expr(&if_stmt.condition, vars);
                Self::find_variables_block(&if_stmt.body, vars);
                for branch in &if_stmt.else_if {
                    Self::find_variables_boolean_expr(&branch.condition, vars);
                    Self::find_variables_block(&branch.body, vars);
                }
                if let Some(else_body) = &if_stmt.else_body {
                    Self::find_variables_block(else_body, vars);
                }
            }
            Statement::ForStmt(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    Self::find_variables_stmt(init, vars);
                }
                if let Some(cond) = &for_stmt.condition {
                    Self::find_variables_boolean_expr(cond, vars);
                }
                if let Some(step) = &for_stmt.step {
                    Self::find_variables_stmt(step, vars);
                }
                Self::find_variables_block(&for_stmt.body, vars);
            }
            Statement::Block(block) => {
                Self::find_variables_block(block, vars);
            }
            Statement::Return(ret) => {
                for val in &ret.values {
                    Self::find_variables_expr(val, vars);
                }
            }
            Statement::CallFunc(call) => {
                for arg in &call.args {
                    Self::find_variables_expr(arg, vars);
                }
            }
            Statement::FieldAssign(fa) => {
                vars.insert(fa.object.clone());
                Self::find_variables_expr(&fa.value, vars);
            }
            Statement::IndexAssign(ia) => {
                vars.insert(ia.object.clone());
                Self::find_variables_expr(&ia.index, vars);
                Self::find_variables_expr(&ia.value, vars);
            }
            Statement::MethodCall(mc) => {
                Self::find_variables_expr(&mc.callee, vars);
                for arg in &mc.args {
                    Self::find_variables_expr(arg, vars);
                }
            }
            _ => {}
        }
    }

    fn find_variables_boolean_expr(expr: &BooleanExpr, vars: &mut HashSet<String>) {
        match expr {
            BooleanExpr::Comparison { left, right, .. } => {
                Self::find_variables_expr(left, vars);
                Self::find_variables_expr(right, vars);
            }
            BooleanExpr::Logical { left, right, .. } => {
                Self::find_variables_boolean_expr(left, vars);
                Self::find_variables_boolean_expr(right, vars);
            }
            BooleanExpr::Grouped(inner) => {
                Self::find_variables_boolean_expr(inner, vars);
            }
        }
    }

    /// Find all variables declared inside a block (for excluding from captures).
    pub(super) fn find_declared_vars_block(block: &Block, declared: &mut HashSet<String>) {
        for stmt in &block.statements {
            Self::find_declared_vars_stmt(stmt, declared);
        }
    }

    fn find_declared_vars_stmt(stmt: &Statement, declared: &mut HashSet<String>) {
        match stmt {
            Statement::VarDecl(VarDecl::Assignment(assign)) => {
                for target in &assign.targets {
                    declared.insert(target.clone());
                }
            }
            Statement::VarDecl(VarDecl::TypeDecl { name, .. }) => {
                declared.insert(name.clone());
            }
            Statement::IfStmt(if_stmt) => {
                Self::find_declared_vars_block(&if_stmt.body, declared);
                for branch in &if_stmt.else_if {
                    Self::find_declared_vars_block(&branch.body, declared);
                }
                if let Some(else_body) = &if_stmt.else_body {
                    Self::find_declared_vars_block(else_body, declared);
                }
            }
            Statement::ForStmt(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    Self::find_declared_vars_stmt(init, declared);
                }
                Self::find_declared_vars_block(&for_stmt.body, declared);
            }
            Statement::Block(block) => {
                Self::find_declared_vars_block(block, declared);
            }
            _ => {}
        }
    }
}
