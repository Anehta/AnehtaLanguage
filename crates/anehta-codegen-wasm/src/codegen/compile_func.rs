use super::*;

impl WasmCodegen {
    /// Compile a function declaration into a wasm Function
    pub(super) fn compile_func_decl(&self, func: &FuncDecl) -> Result<Function, CodegenError> {
        let mut ctx = FuncCtx::new();

        for param in &func.params {
            ctx.add_param(&param.name);
            ctx.var_types
                .insert(param.name.clone(), type_name_to_ah(&param.type_name));
            ctx.param_names.insert(param.name.clone());
        }

        self.prescan_block(&func.body, &mut ctx);

        let mut wasm_func = Function::new(
            ctx.extra_locals
                .iter()
                .map(|ty| (1u32, *ty))
                .collect::<Vec<_>>(),
        );
        let mut insn = wasm_func.instructions();

        for owned in &ctx.owned_tables {
            let var_idx = ctx.locals[owned];
            insn.i64_const(-1);
            insn.local_set(var_idx);
        }

        self.compile_block(&func.body, &mut insn, &mut ctx)?;
        self.emit_table_cleanup(&mut insn, &ctx, None);

        for _ in &func.return_types {
            insn.i64_const(0);
        }

        insn.end();
        Ok(wasm_func)
    }

    /// Compile all closure functions and add them to function/code sections.
    pub(super) fn compile_closure_functions(
        &self,
        function_section: &mut FunctionSection,
        code_section: &mut CodeSection,
        program: &Program,
    ) -> Result<(), CodegenError> {
        let mut closure_exprs: Vec<&ClosureExpr> = Vec::new();
        for stmt in &program.statements {
            Self::collect_closure_expr_refs_stmt(stmt, &mut closure_exprs);
        }

        for (i, info) in self.closures.iter().enumerate() {
            function_section.function(info.type_idx);
            let closure_expr = closure_exprs[i];
            let wasm_func = self.compile_single_closure(info, closure_expr)?;
            code_section.function(&wasm_func);
        }

        Ok(())
    }

    /// Recursively collect references to ClosureExpr nodes in depth-first order.
    fn collect_closure_expr_refs_stmt<'a>(
        stmt: &'a Statement,
        out: &mut Vec<&'a ClosureExpr>,
    ) {
        match stmt {
            Statement::VarDecl(VarDecl::Assignment(assign)) | Statement::Assignment(assign) => {
                for val in &assign.values {
                    Self::collect_closure_expr_refs_expr(val, out);
                }
            }
            Statement::VarDecl(VarDecl::TypeDecl { .. }) => {}
            Statement::IfStmt(if_stmt) => {
                Self::collect_closure_expr_refs_boolean(&if_stmt.condition, out);
                Self::collect_closure_expr_refs_block(&if_stmt.body, out);
                for branch in &if_stmt.else_if {
                    Self::collect_closure_expr_refs_boolean(&branch.condition, out);
                    Self::collect_closure_expr_refs_block(&branch.body, out);
                }
                if let Some(else_body) = &if_stmt.else_body {
                    Self::collect_closure_expr_refs_block(else_body, out);
                }
            }
            Statement::ForStmt(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    Self::collect_closure_expr_refs_stmt(init, out);
                }
                if let Some(cond) = &for_stmt.condition {
                    Self::collect_closure_expr_refs_boolean(cond, out);
                }
                if let Some(step) = &for_stmt.step {
                    Self::collect_closure_expr_refs_stmt(step, out);
                }
                Self::collect_closure_expr_refs_block(&for_stmt.body, out);
            }
            Statement::Block(block) => {
                Self::collect_closure_expr_refs_block(block, out);
            }
            Statement::Return(ret) => {
                for val in &ret.values {
                    Self::collect_closure_expr_refs_expr(val, out);
                }
            }
            Statement::CallFunc(call) => {
                for arg in &call.args {
                    Self::collect_closure_expr_refs_expr(arg, out);
                }
            }
            Statement::FuncDecl(func) => {
                Self::collect_closure_expr_refs_block(&func.body, out);
            }
            Statement::TimerStmt(timer) => {
                Self::collect_closure_expr_refs_block(&timer.body, out);
            }
            Statement::MethodCall(mc) => {
                Self::collect_closure_expr_refs_expr(&mc.callee, out);
                for arg in &mc.args {
                    Self::collect_closure_expr_refs_expr(arg, out);
                }
            }
            Statement::FieldAssign(fa) => {
                Self::collect_closure_expr_refs_expr(&fa.value, out);
            }
            Statement::IndexAssign(ia) => {
                Self::collect_closure_expr_refs_expr(&ia.index, out);
                Self::collect_closure_expr_refs_expr(&ia.value, out);
            }
            _ => {}
        }
    }

    fn collect_closure_expr_refs_block<'a>(
        block: &'a Block,
        out: &mut Vec<&'a ClosureExpr>,
    ) {
        for stmt in &block.statements {
            Self::collect_closure_expr_refs_stmt(stmt, out);
        }
    }

    fn collect_closure_expr_refs_boolean<'a>(
        expr: &'a BooleanExpr,
        out: &mut Vec<&'a ClosureExpr>,
    ) {
        match expr {
            BooleanExpr::Comparison { left, right, .. } => {
                Self::collect_closure_expr_refs_expr(left, out);
                Self::collect_closure_expr_refs_expr(right, out);
            }
            BooleanExpr::Logical { left, right, .. } => {
                Self::collect_closure_expr_refs_boolean(left, out);
                Self::collect_closure_expr_refs_boolean(right, out);
            }
            BooleanExpr::Grouped(inner) => {
                Self::collect_closure_expr_refs_boolean(inner, out);
            }
        }
    }

    fn collect_closure_expr_refs_expr<'a>(
        expr: &'a Expr,
        out: &mut Vec<&'a ClosureExpr>,
    ) {
        match expr {
            Expr::Closure(closure) => {
                match &closure.body {
                    ClosureBody::Expr(e) => Self::collect_closure_expr_refs_expr(e, out),
                    ClosureBody::Block(b) => Self::collect_closure_expr_refs_block(b, out),
                }
                out.push(closure);
            }
            Expr::BinaryOp { left, right, .. } => {
                Self::collect_closure_expr_refs_expr(left, out);
                Self::collect_closure_expr_refs_expr(right, out);
            }
            Expr::CallFunc(call) => {
                for arg in &call.args {
                    Self::collect_closure_expr_refs_expr(arg, out);
                }
            }
            Expr::Grouped(inner) => {
                Self::collect_closure_expr_refs_expr(inner, out);
            }
            Expr::MethodCall(mc) => {
                Self::collect_closure_expr_refs_expr(&mc.callee, out);
                for arg in &mc.args {
                    Self::collect_closure_expr_refs_expr(arg, out);
                }
            }
            Expr::TableLiteral(table) => {
                for entry in &table.entries {
                    Self::collect_closure_expr_refs_expr(&entry.value, out);
                }
            }
            Expr::FieldAccess(fa) => {
                Self::collect_closure_expr_refs_expr(&fa.object, out);
            }
            Expr::IndexAccess(ia) => {
                Self::collect_closure_expr_refs_expr(&ia.object, out);
                Self::collect_closure_expr_refs_expr(&ia.index, out);
            }
            _ => {}
        }
    }

    /// Compile a single closure function body.
    fn compile_single_closure(
        &self,
        info: &ClosureInfo,
        closure_expr: &ClosureExpr,
    ) -> Result<Function, CodegenError> {
        let mut ctx = FuncCtx::new();

        let env_ptr_idx = ctx.add_param_with_type("__env_ptr", ValType::I32);

        for param in &closure_expr.params {
            ctx.add_param(&param.name);
            ctx.var_types.insert(param.name.clone(), AhType::Int);
            ctx.param_names.insert(param.name.clone());
        }

        for capture in &info.captures {
            ctx.declare_local(capture);
            ctx.var_types.insert(capture.clone(), AhType::Int);
            ctx.param_names.insert(capture.clone());
        }

        match &closure_expr.body {
            ClosureBody::Expr(e) => self.prescan_expr(e, &mut ctx),
            ClosureBody::Block(b) => self.prescan_block(b, &mut ctx),
        }

        let mut wasm_func = Function::new(
            ctx.extra_locals
                .iter()
                .map(|ty| (1u32, *ty))
                .collect::<Vec<_>>(),
        );
        let mut insn = wasm_func.instructions();

        for (cap_idx, capture) in info.captures.iter().enumerate() {
            let local_idx = ctx.get_local(capture).unwrap();
            insn.local_get(env_ptr_idx);
            insn.i64_load(MemArg {
                offset: (cap_idx * 8) as u64,
                align: 3,
                memory_index: 0,
            });
            insn.local_set(local_idx);
        }

        for owned in &ctx.owned_tables {
            let var_idx = ctx.locals[owned];
            insn.i64_const(-1);
            insn.local_set(var_idx);
        }

        match &closure_expr.body {
            ClosureBody::Expr(e) => {
                self.compile_expr(e, &mut insn, &mut ctx)?;
                insn.return_();
            }
            ClosureBody::Block(b) => {
                self.compile_block(b, &mut insn, &mut ctx)?;
                self.emit_table_cleanup(&mut insn, &ctx, None);
            }
        }

        insn.i64_const(0);
        insn.end();

        Ok(wasm_func)
    }

    pub(super) fn emit_closure_call_indirect(
        &self,
        closure_id: u32,
        closure_local_idx: u32,
        call: &CallFunc,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let info = &self.closures[closure_id as usize];
        let num_args = call.args.len();

        let arg_temps = ctx.claim_closure_call_temps();

        for i in (0..num_args).rev() {
            insn.local_set(arg_temps[i]);
        }

        insn.local_get(closure_local_idx);
        insn.i32_wrap_i64();

        for i in 0..num_args {
            insn.local_get(arg_temps[i]);
        }

        insn.local_get(closure_local_idx);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.i32_wrap_i64();

        let call_type_idx = info.type_idx;
        insn.call_indirect(0, call_type_idx);

        Ok(())
    }
}
