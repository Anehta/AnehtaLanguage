use super::*;

impl WasmCodegen {
    /// Pre-scan a statement to discover all variable declarations and power ops (for locals allocation)
    pub(super) fn prescan_stmt(&self, stmt: &Statement, ctx: &mut FuncCtx) {
        match stmt {
            Statement::VarDecl(VarDecl::TypeDecl { name, type_name, .. }) => {
                ctx.declare_local(name);
                ctx.var_types.insert(name.clone(), type_name_to_ah(type_name));
            }
            Statement::VarDecl(VarDecl::Assignment(assign)) => {
                // Check for vector destructuring: allocate temp for vec storage
                if assign.values.len() == 1 && assign.targets.len() > 1 {
                    let val_ty = self.infer_expr_type(&assign.values[0], ctx);
                    if val_ty == AhType::Vec {
                        let temp = ctx.alloc_anonymous_local();
                        ctx.destructure_temps.push(temp);
                    }
                }

                for (i, target) in assign.targets.iter().enumerate() {
                    ctx.declare_local(target);
                    // Infer type from value expression when possible
                    if i < assign.values.len() {
                        let ty = self.infer_expr_type(&assign.values[i], ctx);
                        ctx.var_types.insert(target.clone(), ty);
                        // Track table ownership
                        if matches!(ty, AhType::Table(_))
                            && !ctx.param_names.contains(target)
                            && !ctx.owned_tables.contains(target)
                        {
                            ctx.owned_tables.push(target.clone());
                        }
                    }
                }
                for val in &assign.values {
                    self.prescan_expr(val, ctx);
                }
            }
            Statement::Assignment(assign) => {
                // Check for vector destructuring: allocate temp for vec storage
                if assign.values.len() == 1 && assign.targets.len() > 1 {
                    let val_ty = self.infer_expr_type(&assign.values[0], ctx);
                    if val_ty == AhType::Vec {
                        let temp = ctx.alloc_anonymous_local();
                        ctx.destructure_temps.push(temp);
                    }
                }

                for (i, target) in assign.targets.iter().enumerate() {
                    ctx.declare_local(target);
                    if i < assign.values.len() {
                        let ty = self.infer_expr_type(&assign.values[i], ctx);
                        ctx.var_types.insert(target.clone(), ty);
                        // Track table ownership
                        if matches!(ty, AhType::Table(_))
                            && !ctx.param_names.contains(target)
                            && !ctx.owned_tables.contains(target)
                        {
                            ctx.owned_tables.push(target.clone());
                        }
                    }
                }
                for val in &assign.values {
                    self.prescan_expr(val, ctx);
                }
            }
            Statement::IfStmt(if_stmt) => {
                self.prescan_boolean_expr(&if_stmt.condition, ctx);
                self.prescan_block(&if_stmt.body, ctx);
                for branch in &if_stmt.else_if {
                    self.prescan_boolean_expr(&branch.condition, ctx);
                    self.prescan_block(&branch.body, ctx);
                }
                if let Some(else_body) = &if_stmt.else_body {
                    self.prescan_block(else_body, ctx);
                }
            }
            Statement::ForStmt(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    self.prescan_stmt(init, ctx);
                }
                if let Some(cond) = &for_stmt.condition {
                    self.prescan_boolean_expr(cond, ctx);
                }
                if let Some(step) = &for_stmt.step {
                    self.prescan_stmt(step, ctx);
                }
                self.prescan_block(&for_stmt.body, ctx);
            }
            Statement::Block(block) => {
                self.prescan_block(block, ctx);
            }
            Statement::Return(ret) => {
                for val in &ret.values {
                    self.prescan_expr(val, ctx);
                }
                // Pre-allocate temp locals for saving return values during table cleanup
                if !ret.values.is_empty() {
                    let mut temps = Vec::new();
                    for _ in &ret.values {
                        temps.push(ctx.alloc_anonymous_local());
                    }
                    ctx.return_save_temps.push(temps);
                }
            }
            Statement::CallFunc(call) => {
                for arg in &call.args {
                    self.prescan_expr(arg, ctx);
                }
            }
            Statement::TimerStmt(timer) => {
                ctx.alloc_timer_temps();
                self.prescan_block(&timer.body, ctx);
            }
            Statement::FieldAssign(fa) => {
                self.prescan_expr(&fa.value, ctx);
            }
            Statement::IndexAssign(ia) => {
                self.prescan_expr(&ia.index, ctx);
                self.prescan_expr(&ia.value, ctx);
            }
            Statement::MethodCall(mc) => {
                self.prescan_expr(&mc.callee, ctx);
                for arg in &mc.args {
                    self.prescan_expr(arg, ctx);
                }
                // Always allocate closure call temps: num_args + 1 (extra for closure value)
                ctx.alloc_closure_call_temps(mc.args.len() + 1);
            }
            _ => {}
        }
    }

    pub(super) fn prescan_block(&self, block: &Block, ctx: &mut FuncCtx) {
        for stmt in &block.statements {
            self.prescan_stmt(stmt, ctx);
        }
    }

    pub(super) fn prescan_expr(&self, expr: &Expr, ctx: &mut FuncCtx) {
        match expr {
            Expr::BinaryOp {
                left, op, right, ..
            } => {
                self.prescan_expr(left, ctx);
                self.prescan_expr(right, ctx);
                if matches!(op, BinaryOp::Power | BinaryOp::DotPow) {
                    // Only allocate power temps for integer power; float/vec/mat power uses host function
                    let lt = self.infer_expr_type(left, ctx);
                    let rt = self.infer_expr_type(right, ctx);
                    if lt != AhType::Float && lt != AhType::Vec && lt != AhType::Mat && rt != AhType::Float {
                        ctx.alloc_power_temps();
                    }
                }
            }
            Expr::CallFunc(call) => {
                for arg in &call.args {
                    self.prescan_expr(arg, ctx);
                }
                // If the call target is not a known function, it may be a closure call.
                // Pre-allocate temp locals for argument reordering.
                if !self.func_map.contains_key(&call.name) {
                    if let Some(AhType::Closure(_)) = ctx.var_types.get(&call.name) {
                        ctx.alloc_closure_call_temps(call.args.len());
                    }
                }
            }
            Expr::Grouped(inner) => {
                self.prescan_expr(inner, ctx);
            }
            Expr::Closure(closure) => {
                // Recurse into the closure body for prescan (e.g. nested power ops)
                match &closure.body {
                    ClosureBody::Expr(e) => self.prescan_expr(e, ctx),
                    ClosureBody::Block(b) => self.prescan_block(b, ctx),
                }
                // Pre-allocate a temp local for env_ptr (used when closure has captures)
                let key = (closure.span.line, closure.span.column);
                if let Some(&closure_id) = self.closure_span_map.get(&key) {
                    let info = &self.closures[closure_id as usize];
                    if !info.captures.is_empty() {
                        let temp = ctx.alloc_anonymous_local();
                        ctx.closure_env_temps.push(temp);
                    }
                    // Record captured variables so their tables are NOT freed
                    for cap in &info.captures {
                        ctx.captured_tables.insert(cap.clone());
                    }
                }
            }
            Expr::TableLiteral(table) => {
                for entry in &table.entries {
                    self.prescan_expr(&entry.value, ctx);
                }
                // Pre-allocate a temp local for the table_id during construction
                let temp = ctx.alloc_anonymous_local();
                ctx.table_temps.push(temp);
            }
            Expr::FieldAccess(fa) => {
                self.prescan_expr(&fa.object, ctx);
            }
            Expr::IndexAccess(ia) => {
                self.prescan_expr(&ia.object, ctx);
                self.prescan_expr(&ia.index, ctx);
            }
            Expr::MethodCall(mc) => {
                self.prescan_expr(&mc.callee, ctx);
                for arg in &mc.args {
                    self.prescan_expr(arg, ctx);
                }
                // Always allocate closure call temps: num_args + 1 (extra for closure value)
                ctx.alloc_closure_call_temps(mc.args.len() + 1);
            }
            Expr::VecLiteral(vec_lit) => {
                for elem in &vec_lit.elements {
                    self.prescan_expr(elem, ctx);
                }
                let temp = ctx.alloc_anonymous_local();
                ctx.vec_literal_temps.push(temp);
            }
            Expr::MatLiteral(mat_lit) => {
                for row in &mat_lit.rows {
                    for elem in row {
                        self.prescan_expr(elem, ctx);
                    }
                }
                let temp = ctx.alloc_anonymous_local();
                ctx.mat_literal_temps.push(temp);
            }
            Expr::Transpose(t) => {
                self.prescan_expr(&t.operand, ctx);
            }
            Expr::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.prescan_expr(s, ctx);
                }
                if let Some(e) = end {
                    self.prescan_expr(e, ctx);
                }
            }
            Expr::BooleanExpr(bool_expr) => {
                self.prescan_boolean_expr(bool_expr, ctx);
            }
            _ => {}
        }
    }

    pub(super) fn prescan_boolean_expr(&self, expr: &BooleanExpr, ctx: &mut FuncCtx) {
        match expr {
            BooleanExpr::Comparison { left, right, .. } => {
                self.prescan_expr(left, ctx);
                self.prescan_expr(right, ctx);
            }
            BooleanExpr::Logical { left, right, .. } => {
                self.prescan_boolean_expr(left, ctx);
                self.prescan_boolean_expr(right, ctx);
            }
            BooleanExpr::Grouped(inner) => {
                self.prescan_boolean_expr(inner, ctx);
            }
        }
    }
}
