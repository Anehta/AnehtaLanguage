use super::*;

impl WasmCodegen {
    pub(super) fn compile_block(
        &self,
        block: &Block,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        for stmt in &block.statements {
            self.compile_stmt(stmt, insn, ctx)?;
        }
        Ok(())
    }

    pub(super) fn compile_stmt(
        &self,
        stmt: &Statement,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        match stmt {
            Statement::VarDecl(VarDecl::TypeDecl { name, .. }) => {
                // Type-only declaration: initialize to 0
                let idx = ctx.declare_local(name);
                insn.i64_const(0);
                insn.local_set(idx);
            }
            Statement::VarDecl(VarDecl::Assignment(assign)) => {
                self.compile_assignment(assign, insn, ctx)?;
            }
            Statement::Assignment(assign) => {
                self.compile_assignment(assign, insn, ctx)?;
            }
            Statement::IfStmt(if_stmt) => {
                self.compile_if(if_stmt, insn, ctx)?;
            }
            Statement::ForStmt(for_stmt) => {
                self.compile_for(for_stmt, insn, ctx)?;
            }
            Statement::Block(block) => {
                self.compile_block(block, insn, ctx)?;
            }
            Statement::CallFunc(call) => {
                self.compile_call_func_stmt(call, insn, ctx)?;
            }
            Statement::Return(ret) => {
                self.compile_return(ret, insn, ctx)?;
            }
            Statement::Break(span) => {
                if let Some(loop_info) = ctx.loop_depth_stack.last() {
                    let relative = ctx.block_depth - loop_info.break_depth;
                    insn.br(relative);
                } else {
                    return Err(codegen_err("break outside of loop", span));
                }
            }
            Statement::Continue(span) => {
                if let Some(loop_info) = ctx.loop_depth_stack.last() {
                    let relative = ctx.block_depth - loop_info.continue_depth;
                    insn.br(relative);
                } else {
                    return Err(codegen_err("continue outside of loop", span));
                }
            }
            Statement::TimerStmt(timer) => {
                self.compile_timer(timer, insn, ctx)?;
            }
            Statement::FuncDecl(_) => {
                // Nested function declarations are not supported at statement level in codegen.
                // They should only appear at top level.
            }
            Statement::FieldAssign(fa) => {
                self.compile_field_assign(fa, insn, ctx)?;
            }
            Statement::IndexAssign(ia) => {
                self.compile_index_assign(ia, insn, ctx)?;
            }
            Statement::MethodCall(mc) => {
                self.compile_method_call_expr(mc, insn, ctx)?;
                insn.drop(); // discard return value in statement context
            }
        }
        Ok(())
    }

    fn compile_assignment(
        &self,
        assign: &Assignment,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let (table_free_idx, _) = self.func_map["__env_table_free"];

        // Check for vector destructuring: [x, y, z] = vec
        // (single value that is Vec type with multiple targets)
        if assign.values.len() == 1 && assign.targets.len() > 1 {
            let val_ty = self.infer_expr_type(&assign.values[0], ctx);
            if val_ty == AhType::Vec {
                // Vector destructuring: compile vec once, then extract elements
                self.compile_expr(&assign.values[0], insn, ctx)?;
                let vec_temp = ctx.destructure_temps[ctx.destructure_temps_cursor];
                ctx.destructure_temps_cursor += 1;
                insn.local_set(vec_temp);

                let (vec_get_idx, _) = self.func_map["__env_vec_get"];
                for (i, target) in assign.targets.iter().enumerate() {
                    // Free old table if needed
                    if ctx.owned_tables.contains(target) && !ctx.captured_tables.contains(target) {
                        if matches!(ctx.var_types.get(target), Some(AhType::Table(_))) {
                            let var_idx = ctx.locals[target];
                            insn.local_get(var_idx);
                            insn.call(table_free_idx);
                        }
                    }

                    // Extract element: vec_get(vec, i)
                    insn.local_get(vec_temp);
                    insn.i64_const(i as i64);
                    insn.call(vec_get_idx);

                    // Element is f64 bits, track as Float
                    ctx.var_types.insert(target.clone(), AhType::Float);
                    let idx = ctx.declare_local(target);
                    insn.local_set(idx);
                }
                return Ok(());
            }
        }

        // Regular assignment: for each target/value pair
        for (i, target) in assign.targets.iter().enumerate() {
            // Free old table if this is an owned table variable being reassigned
            // Skip if captured by a closure (old value may still be referenced)
            if ctx.owned_tables.contains(target) && !ctx.captured_tables.contains(target) {
                if matches!(ctx.var_types.get(target), Some(AhType::Table(_))) {
                    let var_idx = ctx.locals[target];
                    insn.local_get(var_idx);
                    insn.call(table_free_idx);
                }
            }

            if i < assign.values.len() {
                // Track type of the assigned value
                let ty = self.infer_expr_type(&assign.values[i], ctx);
                ctx.var_types.insert(target.clone(), ty);
                self.compile_expr(&assign.values[i], insn, ctx)?;
            } else {
                // If fewer values than targets, use 0
                insn.i64_const(0);
            }
            let idx = ctx.declare_local(target);
            insn.local_set(idx);
        }
        Ok(())
    }

    fn compile_if(
        &self,
        if_stmt: &IfStmt,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        // Compile condition (produces i32 on stack)
        self.compile_boolean_expr(&if_stmt.condition, insn, ctx)?;

        // if (empty block type since we don't produce values)
        insn.if_(BlockType::Empty);
        ctx.block_depth += 1;

        // Then body
        self.compile_block(&if_stmt.body, insn, ctx)?;

        // Handle elseif / else chains
        if !if_stmt.else_if.is_empty() || if_stmt.else_body.is_some() {
            insn.else_();

            for branch in &if_stmt.else_if {
                self.compile_boolean_expr(&branch.condition, insn, ctx)?;
                insn.if_(BlockType::Empty);
                ctx.block_depth += 1;
                self.compile_block(&branch.body, insn, ctx)?;
                insn.else_();
            }

            if let Some(else_block) = &if_stmt.else_body {
                self.compile_block(else_block, insn, ctx)?;
            }

            // Close all the nested if-else blocks from elseif chains
            for _ in &if_stmt.else_if {
                insn.end();
                ctx.block_depth -= 1;
            }
        }

        // Close the main if
        insn.end();
        ctx.block_depth -= 1;

        Ok(())
    }

    fn compile_for(
        &self,
        for_stmt: &ForStmt,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        // Compile init
        if let Some(init) = &for_stmt.init {
            self.compile_stmt(init, insn, ctx)?;
        }

        // block (for break)
        insn.block(BlockType::Empty);
        ctx.block_depth += 1;
        let break_depth = ctx.block_depth;

        // loop (for continue)
        insn.loop_(BlockType::Empty);
        ctx.block_depth += 1;
        let continue_depth = ctx.block_depth;

        ctx.loop_depth_stack.push(LoopInfo {
            break_depth,
            continue_depth,
        });

        // Condition check
        if let Some(cond) = &for_stmt.condition {
            self.compile_boolean_expr(cond, insn, ctx)?;
            // If condition is false (i32.eqz), break out
            insn.i32_eqz();
            // br_if to the block (break out). The block is 1 level up from current.
            insn.br_if(ctx.block_depth - break_depth);
        }

        // Body
        self.compile_block(&for_stmt.body, insn, ctx)?;

        // Step
        if let Some(step) = &for_stmt.step {
            self.compile_stmt(step, insn, ctx)?;
        }

        // Jump back to loop start
        insn.br(ctx.block_depth - continue_depth);

        // End loop
        insn.end();
        ctx.block_depth -= 1;

        // End block
        insn.end();
        ctx.block_depth -= 1;

        ctx.loop_depth_stack.pop();

        Ok(())
    }

    fn compile_call_func_stmt(
        &self,
        call: &CallFunc,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        // Special dispatch: print(str_expr) -> env.print_str, print(float) -> env.print_float, print(vec) -> env.print_vec
        if call.name == "print" && call.args.len() == 1 {
            let arg_type = self.infer_expr_type(&call.args[0], ctx);
            if arg_type == AhType::Str {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_str"];
                insn.call(func_idx);
                return Ok(());
            }
            if arg_type == AhType::Float {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_float"];
                insn.call(func_idx);
                return Ok(());
            }
            if arg_type == AhType::Vec {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_vec"];
                insn.call(func_idx);
                return Ok(());
            }
            if arg_type == AhType::Mat {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_mat"];
                insn.call(func_idx);
                return Ok(());
            }
        }

        // Built-in conversion: int(expr)
        if call.name == "int" && call.args.len() == 1 {
            self.compile_expr(&call.args[0], insn, ctx)?;
            let arg_ty = self.infer_expr_type(&call.args[0], ctx);
            if arg_ty == AhType::Float {
                insn.f64_reinterpret_i64();
                insn.i64_trunc_f64_s();
            }
            insn.drop(); // statement context: discard result
            return Ok(());
        }

        // Built-in conversion: float(expr)
        if call.name == "float" && call.args.len() == 1 {
            self.compile_expr(&call.args[0], insn, ctx)?;
            let arg_ty = self.infer_expr_type(&call.args[0], ctx);
            if arg_ty == AhType::Int {
                insn.f64_convert_i64_s();
                insn.i64_reinterpret_f64();
            }
            insn.drop(); // statement context: discard result
            return Ok(());
        }

        // Compile arguments
        for arg in &call.args {
            self.compile_expr(arg, insn, ctx)?;
        }

        // Look up function index
        if let Some(&(func_idx, type_idx)) = self.func_map.get(&call.name) {
            // Check argument count
            let (params, results) = &self.types[type_idx as usize];
            if call.args.len() != params.len() {
                return Err(codegen_err(
                    format!(
                        "function '{}' expects {} argument(s), but {} were given",
                        call.name, params.len(), call.args.len()
                    ),
                    &call.span,
                ));
            }
            insn.call(func_idx);
            // If the function returns values, drop them (this is a statement call)
            for _ in results {
                insn.drop();
            }
        } else if let Some(&local_idx) = ctx.locals.get(&call.name) {
            // Check if it is a closure variable
            if let Some(AhType::Closure(closure_id)) = ctx.var_types.get(&call.name) {
                self.emit_closure_call_indirect(*closure_id, local_idx, call, insn, ctx)?;
                // call_indirect returns i64; drop it for statement context
                insn.drop();
            } else {
                return Err(codegen_err(
                    format!("undefined function: {}", call.name),
                    &call.span,
                ));
            }
        } else {
            return Err(codegen_err(
                format!("undefined function: {}", call.name),
                &call.span,
            ));
        }

        Ok(())
    }

    fn compile_return(
        &self,
        ret: &ReturnStmt,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        // If no owned tables, compile normally (no cleanup needed)
        if ctx.owned_tables.is_empty() {
            for val in &ret.values {
                self.compile_expr(val, insn, ctx)?;
            }
            insn.return_();
            return Ok(());
        }

        // Check if the return value is an owned table variable -> transfer ownership
        let skip_var: Option<String> = if ret.values.len() == 1 {
            if let Expr::Variable(name, _) = &ret.values[0] {
                if ctx.owned_tables.contains(name) {
                    Some(name.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if !ret.values.is_empty() {
            // Compile return values
            for val in &ret.values {
                self.compile_expr(val, insn, ctx)?;
            }

            // Save return values to pre-allocated temp locals
            let temps = ctx.return_save_temps[ctx.return_save_temps_cursor].clone();
            ctx.return_save_temps_cursor += 1;
            for i in (0..ret.values.len()).rev() {
                insn.local_set(temps[i]);
            }

            // Free owned tables (skip the one being returned)
            self.emit_table_cleanup(insn, ctx, skip_var.as_deref());

            // Restore return values
            for temp in &temps {
                insn.local_get(*temp);
            }
        } else {
            // No return value, just cleanup
            self.emit_table_cleanup(insn, ctx, None);
        }

        insn.return_();
        Ok(())
    }

    /// Emit table_free calls for all owned table variables (except skip_var and captured tables).
    pub(super) fn emit_table_cleanup(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
        skip_var: Option<&str>,
    ) {
        if ctx.owned_tables.is_empty() {
            return;
        }
        let (table_free_idx, _) = self.func_map["__env_table_free"];
        for owned in &ctx.owned_tables {
            if skip_var == Some(owned.as_str()) {
                continue;
            }
            // Don't free tables captured by closures (ownership transferred)
            if ctx.captured_tables.contains(owned) {
                continue;
            }
            let var_idx = ctx.locals[owned];
            insn.local_get(var_idx);
            insn.call(table_free_idx);
        }
    }

    fn compile_timer(
        &self,
        timer: &TimerStmt,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let (start_local, end_local) = ctx.claim_timer_temps();
        let (clock_idx, _) = self.func_map["__env_clock"];
        let (print_timer_idx, _) = self.func_map["__env_print_timer"];

        // start = clock()
        insn.call(clock_idx);
        insn.local_set(start_local);

        // execute body
        self.compile_block(&timer.body, insn, ctx)?;

        // end = clock()
        insn.call(clock_idx);
        insn.local_set(end_local);

        // print_timer(end - start)
        insn.local_get(end_local);
        insn.local_get(start_local);
        insn.i64_sub();
        insn.call(print_timer_idx);

        Ok(())
    }

    pub(super) fn compile_field_assign(
        &self,
        fa: &FieldAssign,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        // Reject assigning a table to a field (prevents cycles, ensures tree structure)
        let val_ty = self.infer_expr_type(&fa.value, ctx);
        if matches!(val_ty, AhType::Table(_)) {
            return Err(codegen_err(
                "cannot assign a table to a field; nest tables in the literal instead",
                &fa.span,
            ));
        }

        let (table_set_idx, _) = self.func_map["__env_table_set"];

        // Push object (table_id)
        if let Some(idx) = ctx.get_local(&fa.object) {
            insn.local_get(idx);
        } else {
            return Err(codegen_err(
                format!("undefined variable: {}", fa.object),
                &fa.span,
            ));
        }

        // Push field name as packed string
        let (offset, len) = self
            .string_pool
            .get(fa.field.as_str())
            .copied()
            .unwrap_or((0, 0));
        let packed_key: i64 = ((offset as i64) << 32) | (len as i64);
        insn.i64_const(packed_key);

        // Push value
        self.compile_expr(&fa.value, insn, ctx)?;

        // Call table_set
        insn.call(table_set_idx);
        Ok(())
    }

    pub(super) fn compile_index_assign(
        &self,
        ia: &IndexAssign,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        // Check if object is a Vec
        let obj_ty = ctx.var_types.get(&ia.object).copied().unwrap_or(AhType::Int);
        if obj_ty == AhType::Vec {
            // v[i] = expr → vec_set(v, i, f64_bits)
            let (vec_set_idx, _) = self.func_map["__env_vec_set"];
            if let Some(idx) = ctx.get_local(&ia.object) {
                insn.local_get(idx);
            } else {
                return Err(codegen_err(
                    format!("undefined variable: {}", ia.object),
                    &ia.span,
                ));
            }
            self.compile_expr(&ia.index, insn, ctx)?;
            // Compile value and ensure it's f64 bits
            self.emit_float_operand(&ia.value, insn, ctx)?;
            insn.i64_reinterpret_f64();
            insn.call(vec_set_idx);
            return Ok(());
        }
        if obj_ty == AhType::Mat {
            // m[idx] = expr → mat_set(m, idx, f64_bits)
            let (mat_set_idx, _) = self.func_map["__env_mat_set"];
            if let Some(idx) = ctx.get_local(&ia.object) {
                insn.local_get(idx);
            } else {
                return Err(codegen_err(
                    format!("undefined variable: {}", ia.object),
                    &ia.span,
                ));
            }
            self.compile_expr(&ia.index, insn, ctx)?;
            // Compile value and ensure it's f64 bits
            self.emit_float_operand(&ia.value, insn, ctx)?;
            insn.i64_reinterpret_f64();
            insn.call(mat_set_idx);
            return Ok(());
        }

        // Reject assigning a table to an index (prevents cycles, ensures tree structure)
        let val_ty = self.infer_expr_type(&ia.value, ctx);
        if matches!(val_ty, AhType::Table(_)) {
            return Err(codegen_err(
                "cannot assign a table to a field; nest tables in the literal instead",
                &ia.span,
            ));
        }

        let (table_set_idx, _) = self.func_map["__env_table_set"];

        // Push object (table_id)
        if let Some(idx) = ctx.get_local(&ia.object) {
            insn.local_get(idx);
        } else {
            return Err(codegen_err(
                format!("undefined variable: {}", ia.object),
                &ia.span,
            ));
        }

        // Push key expression
        self.compile_expr(&ia.index, insn, ctx)?;

        // Push value
        self.compile_expr(&ia.value, insn, ctx)?;

        // Call table_set
        insn.call(table_set_idx);
        Ok(())
    }
}
