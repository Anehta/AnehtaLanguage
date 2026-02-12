use super::*;

impl WasmCodegen {
    /// Compile an expression and convert the result to f64 on the WASM stack.
    pub(super) fn emit_float_operand(
        &self,
        expr: &Expr,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        self.compile_expr(expr, insn, ctx)?;
        let ty = self.infer_expr_type(expr, ctx);
        if ty == AhType::Float {
            insn.f64_reinterpret_i64();
        } else {
            insn.f64_convert_i64_s();
        }
        Ok(())
    }

    pub(super) fn compile_expr(
        &self,
        expr: &Expr,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        match expr {
            Expr::Number(s, span) => {
                if s.contains('.') {
                    let val = s.parse::<f64>().map_err(|_| {
                        codegen_err(format!("invalid float literal: {}", s), span)
                    })?;
                    insn.i64_const(val.to_bits() as i64);
                } else {
                    let val = s.parse::<i64>().map_err(|_| {
                        codegen_err(format!("invalid number literal: {}", s), span)
                    })?;
                    insn.i64_const(val);
                }
            }
            Expr::Bool(b, _) => {
                insn.i64_const(if *b { 1 } else { 0 });
            }
            Expr::StringLit(s, _) => {
                let (offset, len) = self
                    .string_pool
                    .get(s.as_str())
                    .copied()
                    .unwrap_or((0, 0));
                let packed: i64 = ((offset as i64) << 32) | (len as i64);
                insn.i64_const(packed);
            }
            Expr::Variable(name, span) => {
                if let Some(idx) = ctx.get_local(name) {
                    insn.local_get(idx);
                } else {
                    return Err(codegen_err(
                        format!("undefined variable: {}", name),
                        span,
                    ));
                }
            }
            Expr::BinaryOp {
                left, op, right, ..
            } => {
                match op {
                    BinaryOp::Rand => {
                        self.compile_expr(left, insn, ctx)?;
                        self.compile_expr(right, insn, ctx)?;
                        let (func_idx, _) = self.func_map["__env_random"];
                        insn.call(func_idx);
                    }
                    BinaryOp::Power => {
                        let lt = self.infer_expr_type(left, ctx);
                        let rt = self.infer_expr_type(right, ctx);
                        if lt == AhType::Float || rt == AhType::Float {
                            self.emit_float_operand(left, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            let (func_idx, _) = self.func_map["__env_float_pow"];
                            insn.call(func_idx);
                        } else {
                            let (base_local, exp_local, result_local) = ctx.claim_power_temps();

                            self.compile_expr(left, insn, ctx)?;
                            insn.local_set(base_local);
                            self.compile_expr(right, insn, ctx)?;
                            insn.local_set(exp_local);
                            insn.i64_const(1);
                            insn.local_set(result_local);

                            insn.block(BlockType::Empty);
                            ctx.block_depth += 1;
                            let done_depth = ctx.block_depth;

                            insn.loop_(BlockType::Empty);
                            ctx.block_depth += 1;
                            let loop_depth = ctx.block_depth;

                            insn.local_get(exp_local);
                            insn.i64_const(0);
                            insn.i64_le_s();
                            insn.br_if(ctx.block_depth - done_depth);

                            insn.local_get(result_local);
                            insn.local_get(base_local);
                            insn.i64_mul();
                            insn.local_set(result_local);

                            insn.local_get(exp_local);
                            insn.i64_const(1);
                            insn.i64_sub();
                            insn.local_set(exp_local);

                            insn.br(ctx.block_depth - loop_depth);

                            insn.end();
                            ctx.block_depth -= 1;
                            insn.end();
                            ctx.block_depth -= 1;

                            insn.local_get(result_local);
                        }
                    }
                    BinaryOp::Add => {
                        let lt = self.infer_expr_type(left, ctx);
                        let rt = self.infer_expr_type(right, ctx);
                        if lt == AhType::Mat && rt == AhType::Mat {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            // 使用内联 WASM SIMD
                            self.emit_mat_add_simd(insn, ctx)?;
                        } else if lt == AhType::Mat && rt == AhType::Vec {
                            // mat + vec: broadcast vec to each row
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            let (idx, _) = self.func_map["__env_mat_add_vec_broadcast"];
                            insn.call(idx);
                        } else if lt == AhType::Mat && (rt == AhType::Float || rt == AhType::Int) {
                            // mat + scalar
                            self.compile_expr(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            let (idx, _) = self.func_map["__env_mat_add_scalar"];
                            insn.call(idx);
                        } else if (lt == AhType::Float || lt == AhType::Int) && rt == AhType::Mat {
                            // scalar + mat
                            self.compile_expr(right, insn, ctx)?;
                            self.emit_float_operand(left, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            let (idx, _) = self.func_map["__env_mat_add_scalar"];
                            insn.call(idx);
                        } else if lt == AhType::Vec && rt == AhType::Vec {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            // 使用内联 WASM SIMD 代替 host function
                            self.emit_vec_add_simd(insn, ctx)?;
                        } else if lt == AhType::Vec && (rt == AhType::Float || rt == AhType::Int) {
                            // Vec + scalar: broadcast scalar addition
                            self.compile_expr(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            // 使用内联 WASM SIMD
                            self.emit_vec_add_scalar_simd(insn, ctx)?;
                        } else if (lt == AhType::Float || lt == AhType::Int) && rt == AhType::Vec {
                            // scalar + Vec: broadcast scalar addition
                            self.compile_expr(right, insn, ctx)?;
                            self.emit_float_operand(left, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            // 使用内联 WASM SIMD
                            self.emit_vec_add_scalar_simd(insn, ctx)?;
                        } else if lt == AhType::Str || rt == AhType::Str {
                            self.compile_expr(left, insn, ctx)?;
                            if lt == AhType::Float {
                                let (idx, _) = self.func_map["__env_float_to_str"];
                                insn.call(idx);
                            } else if lt == AhType::Int {
                                let (idx, _) = self.func_map["__env_int_to_str"];
                                insn.call(idx);
                            }
                            self.compile_expr(right, insn, ctx)?;
                            if rt == AhType::Float {
                                let (idx, _) = self.func_map["__env_float_to_str"];
                                insn.call(idx);
                            } else if rt == AhType::Int {
                                let (idx, _) = self.func_map["__env_int_to_str"];
                                insn.call(idx);
                            }
                            let (func_idx, _) = self.func_map["__env_str_concat"];
                            insn.call(func_idx);
                        } else if lt == AhType::Float || rt == AhType::Float {
                            self.emit_float_operand(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.f64_add();
                            insn.i64_reinterpret_f64();
                        } else {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            insn.i64_add();
                        }
                    }
                    BinaryOp::Sub => {
                        let lt = self.infer_expr_type(left, ctx);
                        let rt = self.infer_expr_type(right, ctx);
                        if lt == AhType::Mat && rt == AhType::Mat {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            // 使用内联 WASM SIMD
                            self.emit_mat_sub_simd(insn, ctx)?;
                        } else if lt == AhType::Mat && rt == AhType::Vec {
                            // mat - vec: broadcast vec subtraction from each row
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            let (idx, _) = self.func_map["__env_mat_sub_vec_broadcast"];
                            insn.call(idx);
                        } else if lt == AhType::Mat && (rt == AhType::Float || rt == AhType::Int) {
                            // mat - scalar
                            self.compile_expr(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            let (idx, _) = self.func_map["__env_mat_sub_scalar"];
                            insn.call(idx);
                        } else if lt == AhType::Vec && rt == AhType::Vec {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            // 使用内联 WASM SIMD
                            self.emit_vec_sub_simd(insn, ctx)?;
                        } else if lt == AhType::Vec && (rt == AhType::Float || rt == AhType::Int) {
                            // vec - scalar
                            self.compile_expr(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            // 使用内联 WASM SIMD
                            self.emit_vec_sub_scalar_simd(insn, ctx)?;
                        } else if lt == AhType::Float || rt == AhType::Float {
                            self.emit_float_operand(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.f64_sub();
                            insn.i64_reinterpret_f64();
                        } else {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            insn.i64_sub();
                        }
                    }
                    BinaryOp::Mul => {
                        let lt = self.infer_expr_type(left, ctx);
                        let rt = self.infer_expr_type(right, ctx);
                        if lt == AhType::Mat && rt == AhType::Mat {
                            // mat * mat → matrix multiplication (SIMD)
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            self.emit_mat_mul_simd(insn, ctx)?;
                        } else if lt == AhType::Mat && rt == AhType::Vec {
                            // mat * vec → vector (SIMD)
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            self.emit_mat_vec_mul_simd(insn, ctx)?;
                        } else if lt == AhType::Mat && (rt == AhType::Float || rt == AhType::Int) {
                            // mat * scalar
                            self.compile_expr(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            // 使用内联 WASM SIMD
                            self.emit_mat_scale_simd(insn, ctx)?;
                        } else if (lt == AhType::Float || lt == AhType::Int) && rt == AhType::Mat {
                            // scalar * mat
                            self.compile_expr(right, insn, ctx)?;
                            self.emit_float_operand(left, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            // 使用内联 WASM SIMD
                            self.emit_mat_scale_simd(insn, ctx)?;
                        } else if lt == AhType::Vec && rt == AhType::Vec {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            // 使用内联 WASM SIMD
                            self.emit_vec_mul_simd(insn, ctx)?;
                        } else if lt == AhType::Vec && (rt == AhType::Float || rt == AhType::Int) {
                            // Vec * scalar
                            self.compile_expr(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            // 使用内联 WASM SIMD
                            self.emit_vec_scale_simd(insn, ctx)?;
                        } else if (lt == AhType::Float || lt == AhType::Int) && rt == AhType::Vec {
                            // scalar * Vec
                            self.compile_expr(right, insn, ctx)?;
                            self.emit_float_operand(left, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            // 使用内联 WASM SIMD
                            self.emit_vec_scale_simd(insn, ctx)?;
                        } else if lt == AhType::Float || rt == AhType::Float {
                            self.emit_float_operand(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.f64_mul();
                            insn.i64_reinterpret_f64();
                        } else {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            insn.i64_mul();
                        }
                    }
                    BinaryOp::Div => {
                        let lt = self.infer_expr_type(left, ctx);
                        let rt = self.infer_expr_type(right, ctx);
                        if lt == AhType::Mat && (rt == AhType::Float || rt == AhType::Int) {
                            // mat / scalar
                            self.compile_expr(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            let (idx, _) = self.func_map["__env_mat_div_scalar"];
                            insn.call(idx);
                        } else if lt == AhType::Vec && (rt == AhType::Float || rt == AhType::Int) {
                            // vec / scalar
                            self.compile_expr(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            // 使用内联 WASM SIMD
                            self.emit_vec_div_scalar_simd(insn, ctx)?;
                        } else if lt == AhType::Float || rt == AhType::Float {
                            self.emit_float_operand(left, insn, ctx)?;
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.f64_div();
                            insn.i64_reinterpret_f64();
                        } else {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            insn.i64_div_s();
                        }
                    }
                    BinaryOp::Mod => {
                        let lt = self.infer_expr_type(left, ctx);
                        let rt = self.infer_expr_type(right, ctx);
                        if lt == AhType::Float || rt == AhType::Float {
                            self.emit_float_operand(left, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            self.emit_float_operand(right, insn, ctx)?;
                            insn.i64_reinterpret_f64();
                            let (func_idx, _) = self.func_map["__env_float_mod"];
                            insn.call(func_idx);
                        } else {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            insn.i64_rem_s();
                        }
                    }
                    BinaryOp::At => {
                        self.compile_expr(left, insn, ctx)?;
                        self.compile_expr(right, insn, ctx)?;
                        // 使用内联 WASM SIMD
                        self.emit_vec_dot_simd(insn, ctx)?;
                    }
                    BinaryOp::Hash => {
                        self.compile_expr(left, insn, ctx)?;
                        self.compile_expr(right, insn, ctx)?;
                        // 使用内联 WASM 代码
                        self.emit_vec_cross_inline(insn, ctx)?;
                    }
                    BinaryOp::Backslash => {
                        // mat \ b  →  solve(mat, b)
                        // Supports: mat \ vec → vec, mat \ mat → mat
                        self.compile_expr(left, insn, ctx)?;
                        self.compile_expr(right, insn, ctx)?;
                        let (func_idx, _) = self.func_map["__env_mat_solve"];
                        insn.call(func_idx);
                    }
                    BinaryOp::DotPow => {
                        // Element-wise power: v .^ exponent
                        let lt = self.infer_expr_type(left, ctx);
                        if lt == AhType::Vec {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            let (func_idx, _) = self.func_map["__env_vec_pow"];
                            insn.call(func_idx);
                        } else if lt == AhType::Mat {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            let (func_idx, _) = self.func_map["__env_mat_pow"];
                            insn.call(func_idx);
                        } else {
                            // Scalar: just use regular power (same as ^)
                            let rt = self.infer_expr_type(right, ctx);
                            if lt == AhType::Float || rt == AhType::Float {
                                self.emit_float_operand(left, insn, ctx)?;
                                self.emit_float_operand(right, insn, ctx)?;
                                let (func_idx, _) = self.func_map["__env_float_pow"];
                                insn.call(func_idx);
                            } else {
                                // Int power (reuse existing implementation)
                                let (base_temp, exp_temp, result_temp) = ctx.claim_power_temps();
                                self.compile_expr(left, insn, ctx)?;
                                insn.local_set(base_temp);
                                self.compile_expr(right, insn, ctx)?;
                                insn.local_set(exp_temp);
                                insn.i64_const(1);
                                insn.local_set(result_temp);
                                insn.block(BlockType::Empty);
                                insn.loop_(BlockType::Empty);
                                insn.local_get(exp_temp);
                                insn.i64_const(0);
                                insn.i64_le_s();
                                insn.br_if(1);
                                insn.local_get(result_temp);
                                insn.local_get(base_temp);
                                insn.i64_mul();
                                insn.local_set(result_temp);
                                insn.local_get(exp_temp);
                                insn.i64_const(1);
                                insn.i64_sub();
                                insn.local_set(exp_temp);
                                insn.br(0);
                                insn.end();
                                insn.end();
                                insn.local_get(result_temp);
                            }
                        }
                    }
                }
            }
            Expr::UnaryOp { op, operand, span } => {
                if let Some(idx) = ctx.get_local(operand) {
                    let var_ty = ctx.var_types.get(operand).copied().unwrap_or(AhType::Int);
                    insn.local_get(idx);
                    insn.local_get(idx);
                    if var_ty == AhType::Float {
                        insn.f64_reinterpret_i64();
                        insn.f64_const(1.0);
                        match op {
                            UnaryOp::Increment => { insn.f64_add(); }
                            UnaryOp::Decrement => { insn.f64_sub(); }
                        }
                        insn.i64_reinterpret_f64();
                    } else {
                        match op {
                            UnaryOp::Increment => {
                                insn.i64_const(1);
                                insn.i64_add();
                            }
                            UnaryOp::Decrement => {
                                insn.i64_const(1);
                                insn.i64_sub();
                            }
                        }
                    }
                    insn.local_set(idx);
                } else {
                    return Err(codegen_err(
                        format!("undefined variable: {}", operand),
                        span,
                    ));
                }
            }
            Expr::CallFunc(call) => {
                self.compile_call_func_expr(call, insn, ctx)?;
            }
            Expr::Grouped(inner) => {
                self.compile_expr(inner, insn, ctx)?;
            }
            Expr::Closure(closure) => {
                self.compile_closure_expr(closure, insn, ctx)?;
            }
            Expr::TableLiteral(table) => {
                self.compile_table_literal(table, insn, ctx)?;
            }
            Expr::FieldAccess(fa) => {
                self.compile_field_access(fa, insn, ctx)?;
            }
            Expr::IndexAccess(ia) => {
                self.compile_index_access(ia, insn, ctx)?;
            }
            Expr::MethodCall(mc) => {
                self.compile_method_call_expr(mc, insn, ctx)?;
            }
            Expr::VecLiteral(vec_lit) => {
                self.compile_vec_literal(vec_lit, insn, ctx)?;
            }
            Expr::MatLiteral(mat_lit) => {
                self.compile_mat_literal(mat_lit, insn, ctx)?;
            }
            Expr::Transpose(t) => {
                self.compile_expr(&t.operand, insn, ctx)?;
                self.emit_transpose_simd(insn, ctx)?;
            }
            Expr::BooleanExpr(boxed) => {
                // BooleanExpr should only appear in IndexAccess context for masking
                let span = match &**boxed {
                    BooleanExpr::Comparison { span, .. } => span,
                    BooleanExpr::Logical { span, .. } => span,
                    BooleanExpr::Grouped(inner) => {
                        match &**inner {
                            BooleanExpr::Comparison { span, .. } => span,
                            BooleanExpr::Logical { span, .. } => span,
                            _ => &Span { line: 0, column: 0 },
                        }
                    }
                };
                return Err(codegen_err("Boolean expression not allowed in this context (use in v[v > 0] for masking)", span));
            }
            Expr::Range { span, .. } => {
                // Range should only appear in IndexAccess context
                return Err(codegen_err("Range expression not allowed in this context", span));
            }
        }
        Ok(())
    }

    fn compile_closure_expr(
        &self,
        closure: &ClosureExpr,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let key = (closure.span.line, closure.span.column);
        let closure_id = self.closure_span_map[&key];
        let info = &self.closures[closure_id as usize];

        let num_captures = info.captures.len();

        if num_captures == 0 {
            let packed = (info.table_idx as i64) << 32;
            insn.i64_const(packed);
        } else {
            let env_ptr_temp = ctx.closure_env_temps[ctx.closure_env_temps_cursor];
            ctx.closure_env_temps_cursor += 1;

            insn.global_get(0);
            insn.i64_extend_i32_u();
            insn.local_set(env_ptr_temp);

            for (cap_idx, capture) in info.captures.iter().enumerate() {
                insn.global_get(0);
                if let Some(local_idx) = ctx.get_local(capture) {
                    insn.local_get(local_idx);
                } else {
                    insn.i64_const(0);
                }
                insn.i64_store(MemArg {
                    offset: (cap_idx * 8) as u64,
                    align: 3,
                    memory_index: 0,
                });
            }

            insn.global_get(0);
            insn.i32_const((num_captures * 8) as i32);
            insn.i32_add();
            insn.global_set(0);

            insn.i64_const((info.table_idx as i64) << 32);
            insn.local_get(env_ptr_temp);
            insn.i64_or();
        }

        Ok(())
    }

    fn compile_table_literal(
        &self,
        table: &TableLiteral,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let (table_new_idx, _) = self.func_map["__env_table_new"];
        let (table_set_idx, _) = self.func_map["__env_table_set"];
        let (table_set_table_idx, _) = self.func_map["__env_table_set_table"];

        let tmp = ctx.table_temps[ctx.table_temps_cursor];
        ctx.table_temps_cursor += 1;

        insn.call(table_new_idx);
        insn.local_set(tmp);

        for entry in &table.entries {
            insn.local_get(tmp);
            let (offset, len) = self
                .string_pool
                .get(entry.key.as_str())
                .copied()
                .unwrap_or((0, 0));
            let packed_key: i64 = ((offset as i64) << 32) | (len as i64);
            insn.i64_const(packed_key);
            self.compile_expr(&entry.value, insn, ctx)?;
            let val_ty = self.infer_expr_type(&entry.value, ctx);
            if matches!(val_ty, AhType::Table(_)) {
                insn.call(table_set_table_idx);
            } else {
                insn.call(table_set_idx);
            }
        }

        insn.local_get(tmp);
        Ok(())
    }

    fn compile_field_access(
        &self,
        fa: &FieldAccess,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let obj_ty = self.infer_expr_type(&fa.object, ctx);
        if obj_ty == AhType::Vec {
            return self.compile_vec_field_access(fa, insn, ctx);
        }
        if obj_ty == AhType::Mat {
            return self.compile_mat_field_access(fa, insn, ctx);
        }
        let (table_get_idx, _) = self.func_map["__env_table_get"];
        self.compile_expr(&fa.object, insn, ctx)?;
        let (offset, len) = self
            .string_pool
            .get(fa.field.as_str())
            .copied()
            .unwrap_or((0, 0));
        let packed_key: i64 = ((offset as i64) << 32) | (len as i64);
        insn.i64_const(packed_key);
        insn.call(table_get_idx);
        Ok(())
    }

    fn compile_vec_field_access(
        &self,
        fa: &FieldAccess,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        // .len → extract len from packed i64
        if fa.field == "len" {
            self.compile_expr(&fa.object, insn, ctx)?;
            insn.i64_const(0xFFFF_FFFF);
            insn.i64_and();
            return Ok(());
        }

        // Single char swizzle → load one element
        if fa.field.len() == 1 {
            let index = match fa.field.chars().next().unwrap() {
                'x' | 'r' => 0,
                'y' | 'g' => 1,
                'z' | 'b' => 2,
                'w' | 'a' => 3,
                _ => return Err(codegen_err(format!("invalid swizzle: {}", fa.field), &fa.span)),
            };
            self.compile_expr(&fa.object, insn, ctx)?;
            insn.i64_const(32);
            insn.i64_shr_u();
            insn.i32_wrap_i64();
            insn.f64_load(MemArg {
                offset: (index * 8) as u64,
                align: 3,
                memory_index: 0,
            });
            insn.i64_reinterpret_f64();
            return Ok(());
        }

        // Multi-char swizzle → call host function
        // Encode pattern: low 4 bits = count, then 4 bits per index
        let mut pattern: i64 = fa.field.len() as i64;
        for (i, ch) in fa.field.chars().enumerate() {
            let idx = match ch {
                'x' | 'r' => 0,
                'y' | 'g' => 1,
                'z' | 'b' => 2,
                'w' | 'a' => 3,
                _ => return Err(codegen_err(format!("invalid swizzle: {}", fa.field), &fa.span)),
            };
            pattern |= idx << (4 + i * 4);
        }
        self.compile_expr(&fa.object, insn, ctx)?;
        insn.i64_const(pattern);
        let (func_idx, _) = self.func_map["__env_vec_swizzle"];
        insn.call(func_idx);
        Ok(())
    }

    fn compile_mat_field_access(
        &self,
        fa: &FieldAccess,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        match fa.field.as_str() {
            "T" => {
                // Transpose → call mat_transpose host function
                self.compile_expr(&fa.object, insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_mat_transpose"];
                insn.call(func_idx);
            }
            "det" => {
                // Determinant → call mat_det host function (returns f64 bits)
                self.compile_expr(&fa.object, insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_mat_det"];
                insn.call(func_idx);
            }
            "inv" => {
                // Inverse → call mat_inv host function
                self.compile_expr(&fa.object, insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_mat_inv"];
                insn.call(func_idx);
            }
            "rows" => {
                // Extract rows from packed i64: (val >> 16) & 0xFFFF
                self.compile_expr(&fa.object, insn, ctx)?;
                insn.i64_const(16);
                insn.i64_shr_u();
                insn.i64_const(0xFFFF);
                insn.i64_and();
            }
            "cols" => {
                // Extract cols from packed i64: val & 0xFFFF
                self.compile_expr(&fa.object, insn, ctx)?;
                insn.i64_const(0xFFFF);
                insn.i64_and();
            }
            _ => {
                return Err(codegen_err(
                    format!("invalid matrix field: {}", fa.field),
                    &fa.span,
                ));
            }
        }
        Ok(())
    }

    fn compile_index_access(
        &self,
        ia: &IndexAccess,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let obj_ty = self.infer_expr_type(&ia.object, ctx);

        // Check if index is a Range expression (for slicing)
        if let Expr::Range { start, end, .. } = &*ia.index {
            if obj_ty == AhType::Vec {
                // Vec slicing: v[start..end]
                self.compile_expr(&ia.object, insn, ctx)?;
                // Compile start (or -1 for None)
                if let Some(s) = start {
                    self.compile_expr(s, insn, ctx)?;
                } else {
                    insn.i64_const(-1); // -1 means from beginning
                }
                // Compile end (or -1 for None)
                if let Some(e) = end {
                    self.compile_expr(e, insn, ctx)?;
                } else {
                    insn.i64_const(-1); // -1 means to end
                }
                let (func_idx, _) = self.func_map["__env_vec_slice"];
                insn.call(func_idx);
                return Ok(());
            }
            if obj_ty == AhType::Mat {
                // Mat slicing: m[start..end] (row slicing)
                self.compile_expr(&ia.object, insn, ctx)?;
                if let Some(s) = start {
                    self.compile_expr(s, insn, ctx)?;
                } else {
                    insn.i64_const(-1);
                }
                if let Some(e) = end {
                    self.compile_expr(e, insn, ctx)?;
                } else {
                    insn.i64_const(-1);
                }
                let (func_idx, _) = self.func_map["__env_mat_slice"];
                insn.call(func_idx);
                return Ok(());
            }
            return Err(codegen_err("Slicing only supported for vec and mat", &ia.span));
        }

        // Check if index is a BooleanExpr (masking: v[v > 0])
        if let Expr::BooleanExpr(boxed_bool) = &*ia.index {
            if let BooleanExpr::Comparison { left, op, right, .. } = &**boxed_bool {
            // For now, support simple cases: v[v > threshold]
            // Compile object (vec or mat)
            self.compile_expr(&ia.object, insn, ctx)?;

            // Compile the right side (threshold value)
            self.compile_expr(right, insn, ctx)?;

            // Encode comparison operator as i64
            let op_code = match op {
                ComparisonOp::Gt => 0,
                ComparisonOp::Lt => 1,
                ComparisonOp::GtEq => 2,
                ComparisonOp::LtEq => 3,
                ComparisonOp::Eq => 4,
                ComparisonOp::NotEq => 5,
            };
            insn.i64_const(op_code);

            // Call masking host function
            if obj_ty == AhType::Vec {
                let (func_idx, _) = self.func_map["__env_vec_mask"];
                insn.call(func_idx);
                return Ok(());
            }
            if obj_ty == AhType::Mat {
                let (func_idx, _) = self.func_map["__env_mat_mask"];
                insn.call(func_idx);
                return Ok(());
            }
            return Err(codegen_err("Boolean masking only supported for vec and mat", &ia.span));
            }
        }

        // Check if index is a Vec type (fancy indexing)
        let idx_ty = self.infer_expr_type(&ia.index, ctx);
        if idx_ty == AhType::Vec {
            if obj_ty == AhType::Vec {
                // Fancy indexing: v[[i1, i2, i3]] or v[idx_vec]
                self.compile_expr(&ia.object, insn, ctx)?;
                self.compile_expr(&ia.index, insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_vec_fancy_index"];
                insn.call(func_idx);
                return Ok(());
            }
            if obj_ty == AhType::Mat {
                // Mat fancy indexing (row selection)
                self.compile_expr(&ia.object, insn, ctx)?;
                self.compile_expr(&ia.index, insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_mat_fancy_index"];
                insn.call(func_idx);
                return Ok(());
            }
            return Err(codegen_err("Fancy indexing only supported for vec and mat", &ia.span));
        }

        // Regular indexing
        if obj_ty == AhType::Vec {
            self.compile_expr(&ia.object, insn, ctx)?;
            self.compile_expr(&ia.index, insn, ctx)?;
            let (func_idx, _) = self.func_map["__env_vec_get"];
            insn.call(func_idx);
            return Ok(());
        }
        if obj_ty == AhType::Mat {
            // Matrix linear indexing: m[idx] where idx = row*cols + col
            // Returns f64 bits as i64
            self.compile_expr(&ia.object, insn, ctx)?;
            self.compile_expr(&ia.index, insn, ctx)?;
            let (func_idx, _) = self.func_map["__env_mat_get"];
            insn.call(func_idx);
            return Ok(());
        }
        let (table_get_idx, _) = self.func_map["__env_table_get"];
        self.compile_expr(&ia.object, insn, ctx)?;
        self.compile_expr(&ia.index, insn, ctx)?;
        insn.call(table_get_idx);
        Ok(())
    }

    pub(super) fn compile_method_call_expr(
        &self,
        mc: &MethodCall,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let num_args = mc.args.len();
        let temps = ctx.claim_closure_call_temps();

        self.compile_expr(&mc.callee, insn, ctx)?;
        let closure_temp = temps[num_args];
        insn.local_set(closure_temp);

        for arg in &mc.args {
            self.compile_expr(arg, insn, ctx)?;
        }

        for i in (0..num_args).rev() {
            insn.local_set(temps[i]);
        }

        insn.local_get(closure_temp);
        insn.i32_wrap_i64();

        for i in 0..num_args {
            insn.local_get(temps[i]);
        }

        insn.local_get(closure_temp);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.i32_wrap_i64();

        let callee_ty = self.infer_expr_type(&mc.callee, ctx);
        let call_type_idx = if let AhType::Closure(id) = callee_ty {
            self.closures[id as usize].type_idx
        } else {
            let mut params = vec![ValType::I32];
            for _ in 0..num_args {
                params.push(ValType::I64);
            }
            let results = vec![ValType::I64];
            let mut found = None;
            for (i, (p, r)) in self.types.iter().enumerate() {
                if p == &params && r == &results {
                    found = Some(i as u32);
                    break;
                }
            }
            found.ok_or_else(|| {
                codegen_err("cannot call a non-closure expression", &mc.span)
            })?
        };

        insn.call_indirect(0, call_type_idx);
        Ok(())
    }

    fn compile_call_func_expr(
        &self,
        call: &CallFunc,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        if call.name == "print" && call.args.len() == 1 {
            let arg_type = self.infer_expr_type(&call.args[0], ctx);
            if arg_type == AhType::Str {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_str"];
                insn.call(func_idx);
                insn.i64_const(0);
                return Ok(());
            }
            if arg_type == AhType::Float {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_float"];
                insn.call(func_idx);
                insn.i64_const(0);
                return Ok(());
            }
            if arg_type == AhType::Vec {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_vec"];
                insn.call(func_idx);
                insn.i64_const(0);
                return Ok(());
            }
            if arg_type == AhType::Mat {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_mat"];
                insn.call(func_idx);
                insn.i64_const(0);
                return Ok(());
            }
        }

        if call.name == "len" && call.args.len() == 1 {
            let arg_ty = self.infer_expr_type(&call.args[0], ctx);
            if arg_ty == AhType::Vec {
                self.compile_expr(&call.args[0], insn, ctx)?;
                insn.i64_const(0xFFFF_FFFF);
                insn.i64_and();
                return Ok(());
            }
        }

        if call.name == "int" && call.args.len() == 1 {
            self.compile_expr(&call.args[0], insn, ctx)?;
            let arg_ty = self.infer_expr_type(&call.args[0], ctx);
            if arg_ty == AhType::Float {
                insn.f64_reinterpret_i64();
                insn.i64_trunc_f64_s();
            }
            return Ok(());
        }

        if call.name == "float" && call.args.len() == 1 {
            self.compile_expr(&call.args[0], insn, ctx)?;
            let arg_ty = self.infer_expr_type(&call.args[0], ctx);
            if arg_ty == AhType::Int {
                insn.f64_convert_i64_s();
                insn.i64_reinterpret_f64();
            }
            return Ok(());
        }

        for arg in &call.args {
            self.compile_expr(arg, insn, ctx)?;
        }
        if let Some(&(func_idx, type_idx)) = self.func_map.get(&call.name) {
            let (params, _) = &self.types[type_idx as usize];
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
        } else if let Some(&local_idx) = ctx.locals.get(&call.name) {
            if let Some(AhType::Closure(closure_id)) = ctx.var_types.get(&call.name) {
                self.emit_closure_call_indirect(*closure_id, local_idx, call, insn, ctx)?;
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

    fn compile_vec_literal(
        &self,
        vec_lit: &anehta_parser::VecLiteral,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let n = vec_lit.elements.len();
        let temp = ctx.vec_literal_temps[ctx.vec_literal_temps_cursor];
        ctx.vec_literal_temps_cursor += 1;

        // 1. Save base ptr: global.get 0 → i64.extend_i32_u → local.set temp
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(temp);

        // 2. Reserve space: global.get 0, i32.const (n*8), i32.add, global.set 0
        insn.global_get(0);
        insn.i32_const((n * 8) as i32);
        insn.i32_add();
        insn.global_set(0);

        // 3. Write each element
        for (i, elem) in vec_lit.elements.iter().enumerate() {
            insn.local_get(temp);
            insn.i32_wrap_i64();
            self.emit_float_operand(elem, insn, ctx)?;
            insn.f64_store(MemArg {
                offset: (i * 8) as u64,
                align: 3,
                memory_index: 0,
            });
        }

        // 4. Construct return value: (base_ptr << 32) | len
        insn.local_get(temp);
        insn.i64_const(32);
        insn.i64_shl();
        insn.i64_const(n as i64);
        insn.i64_or();

        Ok(())
    }

    fn compile_mat_literal(
        &self,
        mat_lit: &anehta_parser::MatLiteral,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let rows = mat_lit.rows.len();
        if rows == 0 {
            // Empty matrix
            insn.i64_const(0);
            return Ok(());
        }
        let cols = mat_lit.rows[0].len();
        let total = rows * cols;
        let temp = ctx.mat_literal_temps[ctx.mat_literal_temps_cursor];
        ctx.mat_literal_temps_cursor += 1;

        // 1. Save base ptr
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(temp);

        // 2. Reserve space
        insn.global_get(0);
        insn.i32_const((total * 8) as i32);
        insn.i32_add();
        insn.global_set(0);

        // 3. Write elements (row-major)
        for (r, row) in mat_lit.rows.iter().enumerate() {
            for (c, elem) in row.iter().enumerate() {
                insn.local_get(temp);
                insn.i32_wrap_i64();
                self.emit_float_operand(elem, insn, ctx)?;
                insn.f64_store(MemArg {
                    offset: ((r * cols + c) * 8) as u64,
                    align: 3,
                    memory_index: 0,
                });
            }
        }

        // 4. Construct return value: (ptr << 32) | (rows << 16) | cols
        insn.local_get(temp);
        insn.i64_const(32);
        insn.i64_shl();
        insn.i64_const((rows << 16 | cols) as i64);
        insn.i64_or();

        Ok(())
    }

    /// 生成内联 WASM SIMD 代码来执行 vec + vec 操作
    /// 前置条件：栈上已有两个 vec（左操作数在栈底，右操作数在栈顶）
    /// 返回：将结果 vec 的 packed i64 留在栈上
    fn emit_vec_add_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        // 使用预分配的 SIMD helper locals (索引 0-7)
        let vec_b = ctx.simd_helpers[0];    // 右操作数
        let vec_a = ctx.simd_helpers[1];    // 左操作数
        let len = ctx.simd_helpers[2];      // 长度
        let ptr_a = ctx.simd_helpers[3];    // 左操作数指针
        let ptr_b = ctx.simd_helpers[4];    // 右操作数指针
        let dest_ptr = ctx.simd_helpers[5]; // 结果指针
        let loop_i = ctx.simd_helpers[6];   // 循环计数器

        // 1. 保存两个输入 vec
        insn.local_set(vec_b); // 保存右操作数
        insn.local_set(vec_a); // 保存左操作数

        // 2. 解包 vec_a：提取 ptr 和 len
        insn.local_get(vec_a);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_a); // ptr_a = vec_a >> 32

        insn.local_get(vec_a);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(len); // len = vec_a & 0xFFFFFFFF

        // 3. 解包 vec_b：提取 ptr
        insn.local_get(vec_b);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_b); // ptr_b = vec_b >> 32

        // 4. 分配目标内存
        insn.global_get(0); // 当前 heap_ptr
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr); // 保存 dest_ptr

        insn.global_get(0);
        insn.local_get(len);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul(); // len * 8
        insn.i32_add();
        insn.global_set(0); // heap_ptr += len * 8

        // 5. 初始化循环计数器
        insn.i64_const(0);
        insn.local_set(loop_i); // i = 0

        // 6. SIMD 循环：处理每 2 个 f64
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        // 检查：i + 2 <= len ?
        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(len);
        insn.i64_gt_u(); // i + 2 > len ?
        insn.br_if(1); // 如果是，跳出外层 block

        // Step 1: 计算并压入存储地址（i32）
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add(); // 栈：[i32_dest_addr]

        // Step 2: 加载 vec_a 的 2 个 f64
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        }); // 栈：[i32_dest_addr, v128_a]

        // Step 3: 加载 vec_b 的 2 个 f64
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        }); // 栈：[i32_dest_addr, v128_a, v128_b]

        // Step 4: f64x2.add
        insn.f64x2_add(); // 栈：[i32_dest_addr, v128_result]

        // Step 5: 存储结果 — 栈顺序正确：[i32_addr, v128_value]
        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        }); // 栈：[]

        // i += 2
        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0); // 继续循环
        insn.end(); // 结束 loop
        insn.end(); // 结束 block

        // 7. 处理尾部元素（如果 len 是奇数）
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(len);
        insn.i64_ge_u(); // i >= len ?
        insn.br_if(0); // 如果是，跳过尾部处理

        // 标量处理：先计算地址，再计算值
        // Step 1: 计算并压入存储地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add(); // 栈：[i32_dest_addr]

        // Step 2: 加载 ptr_a[i]
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        }); // 栈：[i32_dest_addr, f64_a]

        // Step 3: 加载 ptr_b[i]
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        }); // 栈：[i32_dest_addr, f64_a, f64_b]

        // Step 4: f64.add
        insn.f64_add(); // 栈：[i32_dest_addr, f64_result]

        // Step 5: 存储 — 栈顺序正确
        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        }); // 栈：[]

        insn.end(); // 结束尾部处理 block

        // 8. 构造返回值：(dest_ptr << 32) | len
        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(len);
        insn.i64_or();

        Ok(())
    }

    /// 生成内联 WASM SIMD 代码来执行 vec - vec 操作
    fn emit_vec_sub_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        // 使用预分配的 SIMD helper locals
        let vec_b = ctx.simd_helpers[0];
        let vec_a = ctx.simd_helpers[1];
        let len = ctx.simd_helpers[2];
        let ptr_a = ctx.simd_helpers[3];
        let ptr_b = ctx.simd_helpers[4];
        let dest_ptr = ctx.simd_helpers[5];
        let loop_i = ctx.simd_helpers[6];

        // 保存两个输入 vec
        insn.local_set(vec_b);
        insn.local_set(vec_a);

        // 解包 vec_a
        insn.local_get(vec_a);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_a);

        insn.local_get(vec_a);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(len);

        // 解包 vec_b
        insn.local_get(vec_b);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_b);

        // 分配目标内存
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(len);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        // 初始化循环计数器
        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(len);
        insn.i64_gt_u();
        insn.br_if(1);

        // 计算存储地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载 vec_a
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 加载 vec_b
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // f64x2.sub
        insn.f64x2_sub();

        // 存储结果
        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // i += 2
        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 尾部处理
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(len);
        insn.i64_ge_u();
        insn.br_if(0);

        // 计算地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载 a[i]
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 加载 b[i]
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // f64.sub
        insn.f64_sub();

        // 存储
        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        // 构造返回值
        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(len);
        insn.i64_or();

        Ok(())
    }

    /// 生成内联 WASM SIMD 代码来执行 vec * vec（逐元素乘法）
    fn emit_vec_mul_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let vec_b = ctx.simd_helpers[0];
        let vec_a = ctx.simd_helpers[1];
        let len = ctx.simd_helpers[2];
        let ptr_a = ctx.simd_helpers[3];
        let ptr_b = ctx.simd_helpers[4];
        let dest_ptr = ctx.simd_helpers[5];
        let loop_i = ctx.simd_helpers[6];

        insn.local_set(vec_b);
        insn.local_set(vec_a);

        insn.local_get(vec_a);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_a);

        insn.local_get(vec_a);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(len);

        insn.local_get(vec_b);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_b);

        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(len);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(len);
        insn.i64_gt_u();
        insn.br_if(1);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // f64x2.mul
        insn.f64x2_mul();

        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 尾部处理
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(len);
        insn.i64_ge_u();
        insn.br_if(0);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul();

        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(len);
        insn.i64_or();

        Ok(())
    }

    /// 生成内联 WASM SIMD 代码来执行 vec * scalar（标量乘法）
    fn emit_vec_scale_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        // scalar 已经在栈顶（f64 bits as i64）
        // vec 在栈底
        let scalar = ctx.simd_helpers[0];  // 保存 scalar
        let vec_val = ctx.simd_helpers[1]; // 保存 vec
        let len = ctx.simd_helpers[2];
        let ptr = ctx.simd_helpers[3];
        let dest_ptr = ctx.simd_helpers[5];
        let loop_i = ctx.simd_helpers[6];

        // 保存参数
        insn.local_set(scalar);  // 保存 scalar (i64)
        insn.local_set(vec_val); // 保存 vec

        // 解包 vec
        insn.local_get(vec_val);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr);

        insn.local_get(vec_val);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(len);

        // 分配目标内存
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(len);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        // 初始化循环
        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(len);
        insn.i64_gt_u();
        insn.br_if(1);

        // 计算存储地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载 vec[i..i+2]
        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 将 scalar 广播到 f64x2
        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64x2_splat();

        // f64x2.mul
        insn.f64x2_mul();

        // 存储结果
        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // i += 2
        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 尾部处理
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(len);
        insn.i64_ge_u();
        insn.br_if(0);

        // 计算地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载 vec[i]
        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // scalar * vec[i]
        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64_mul();

        // 存储
        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        // 构造返回值
        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(len);
        insn.i64_or();

        Ok(())
    }

    /// 生成内联 WASM SIMD 代码来执行 vec @ vec（点积）
    /// 返回 f64 bits as i64
    fn emit_vec_dot_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let vec_b = ctx.simd_helpers[0];
        let vec_a = ctx.simd_helpers[1];
        let len = ctx.simd_helpers[2];
        let ptr_a = ctx.simd_helpers[3];
        let ptr_b = ctx.simd_helpers[4];
        let loop_i = ctx.simd_helpers[6];
        // simd_helpers[5] 用于累加器（f64）

        // 保存输入
        insn.local_set(vec_b);
        insn.local_set(vec_a);

        // 解包 vec_a
        insn.local_get(vec_a);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_a);

        insn.local_get(vec_a);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(len);

        // 解包 vec_b
        insn.local_get(vec_b);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_b);

        // 初始化累加器为 0.0
        insn.f64_const(0.0);
        insn.i64_reinterpret_f64();
        insn.local_set(ctx.simd_helpers[5]); // sum = 0.0

        // 初始化循环
        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环：每次处理 2 个 f64
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(len);
        insn.i64_gt_u();
        insn.br_if(1);

        // 计算 sum += a[i]*b[i] + a[i+1]*b[i+1]
        // 方法：使用 SIMD 乘法，然后提取两个 lane 并标量累加

        // 加载并相乘
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64x2_mul(); // 栈：[v128_result = (a[i]*b[i], a[i+1]*b[i+1])]

        // 提取 lane 0 并累加
        insn.f64x2_extract_lane(0); // 栈：[f64_lane0]
        insn.local_get(ctx.simd_helpers[5]);
        insn.f64_reinterpret_i64();
        insn.f64_add();
        insn.i64_reinterpret_f64();
        insn.local_set(ctx.simd_helpers[5]); // sum += lane0

        // 重新计算 v128（因为已被消费）
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64x2_mul();

        // 提取 lane 1 并累加
        insn.f64x2_extract_lane(1); // 栈：[f64_lane1]
        insn.local_get(ctx.simd_helpers[5]);
        insn.f64_reinterpret_i64();
        insn.f64_add();
        insn.i64_reinterpret_f64();
        insn.local_set(ctx.simd_helpers[5]); // sum += lane1

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 标量循环处理尾部元素（loop_i 已经指向正确位置）
        // 不需要重置 loop_i，它已经是 SIMD 循环停止的位置

        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.local_get(len);
        insn.i64_ge_u();
        insn.br_if(1);

        // sum += a[i] * b[i]
        insn.local_get(ctx.simd_helpers[5]);
        insn.f64_reinterpret_i64();

        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul();
        insn.f64_add();
        insn.i64_reinterpret_f64();
        insn.local_set(ctx.simd_helpers[5]);

        insn.local_get(loop_i);
        insn.i64_const(1);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 返回累加结果
        insn.local_get(ctx.simd_helpers[5]);

        Ok(())
    }

    /// 生成内联 WASM SIMD 代码来执行 vec / scalar
    fn emit_vec_div_scalar_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let scalar = ctx.simd_helpers[0];
        let vec_val = ctx.simd_helpers[1];
        let len = ctx.simd_helpers[2];
        let ptr = ctx.simd_helpers[3];
        let dest_ptr = ctx.simd_helpers[5];
        let loop_i = ctx.simd_helpers[6];

        // 保存参数
        insn.local_set(scalar);
        insn.local_set(vec_val);

        // 解包 vec
        insn.local_get(vec_val);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr);

        insn.local_get(vec_val);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(len);

        // 分配目标内存
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(len);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        // 初始化循环
        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(len);
        insn.i64_gt_u();
        insn.br_if(1);

        // 计算地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载 vec[i..i+2]
        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 广播 scalar
        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64x2_splat();

        // f64x2.div
        insn.f64x2_div();

        // 存储
        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // i += 2
        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 尾部处理
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(len);
        insn.i64_ge_u();
        insn.br_if(0);

        // 计算地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载 vec[i]
        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // vec[i] / scalar
        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64_div();

        // 存储
        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        // 构造返回值
        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(len);
        insn.i64_or();

        Ok(())
    }

    /// 生成内联 WASM 代码来执行 vec # vec（叉积，仅 3D）
    /// 公式：a × b = (a.y*b.z - a.z*b.y, a.z*b.x - a.x*b.z, a.x*b.y - a.y*b.x)
    fn emit_vec_cross_inline(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::MemArg;

        let vec_b = ctx.simd_helpers[0];
        let vec_a = ctx.simd_helpers[1];
        let ptr_a = ctx.simd_helpers[3];
        let ptr_b = ctx.simd_helpers[4];
        let dest_ptr = ctx.simd_helpers[5];

        // 保存输入
        insn.local_set(vec_b);
        insn.local_set(vec_a);

        // 解包 vec_a
        insn.local_get(vec_a);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_a);

        // 解包 vec_b
        insn.local_get(vec_b);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_b);

        // 分配目标内存（3 个 f64）
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.i32_const(24); // 3 * 8 bytes
        insn.i32_add();
        insn.global_set(0);

        // 计算第一个分量：a.y*b.z - a.z*b.y
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();

        // a.y
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 8, // index 1
            align: 3,
            memory_index: 0,
        });

        // b.z
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 16, // index 2
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul(); // a.y * b.z

        // a.z
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 16, // index 2
            align: 3,
            memory_index: 0,
        });

        // b.y
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 8, // index 1
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul(); // a.z * b.y
        insn.f64_sub(); // a.y*b.z - a.z*b.y

        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 计算第二个分量：a.z*b.x - a.x*b.z
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();

        // a.z
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 16,
            align: 3,
            memory_index: 0,
        });

        // b.x
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul();

        // a.x
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // b.z
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 16,
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul();
        insn.f64_sub();

        insn.f64_store(MemArg {
            offset: 8,
            align: 3,
            memory_index: 0,
        });

        // 计算第三个分量：a.x*b.y - a.y*b.x
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();

        // a.x
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // b.y
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 8,
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul();

        // a.y
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 8,
            align: 3,
            memory_index: 0,
        });

        // b.x
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul();
        insn.f64_sub();

        insn.f64_store(MemArg {
            offset: 16,
            align: 3,
            memory_index: 0,
        });

        // 构造返回值：(dest_ptr << 32) | 3
        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.i64_const(3);
        insn.i64_or();

        Ok(())
    }

    /// 生成内联 WASM SIMD 代码来执行 vec + scalar
    fn emit_vec_add_scalar_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let scalar = ctx.simd_helpers[0];
        let vec_val = ctx.simd_helpers[1];
        let len = ctx.simd_helpers[2];
        let ptr = ctx.simd_helpers[3];
        let dest_ptr = ctx.simd_helpers[5];
        let loop_i = ctx.simd_helpers[6];

        insn.local_set(scalar);
        insn.local_set(vec_val);

        insn.local_get(vec_val);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr);

        insn.local_get(vec_val);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(len);

        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(len);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(len);
        insn.i64_gt_u();
        insn.br_if(1);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64x2_splat();

        insn.f64x2_add();

        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 尾部处理
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(len);
        insn.i64_ge_u();
        insn.br_if(0);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64_add();

        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(len);
        insn.i64_or();

        Ok(())
    }

    /// 生成内联 WASM SIMD 代码来执行 vec - scalar
    fn emit_vec_sub_scalar_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let scalar = ctx.simd_helpers[0];
        let vec_val = ctx.simd_helpers[1];
        let len = ctx.simd_helpers[2];
        let ptr = ctx.simd_helpers[3];
        let dest_ptr = ctx.simd_helpers[5];
        let loop_i = ctx.simd_helpers[6];

        insn.local_set(scalar);
        insn.local_set(vec_val);

        insn.local_get(vec_val);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr);

        insn.local_get(vec_val);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(len);

        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(len);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(len);
        insn.i64_gt_u();
        insn.br_if(1);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64x2_splat();

        insn.f64x2_sub();

        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 尾部处理
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(len);
        insn.i64_ge_u();
        insn.br_if(0);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64_sub();

        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(len);
        insn.i64_or();

        Ok(())
    }

    /// 生成内联 WASM SIMD 代码来执行 mat + mat（逐元素加法）
    fn emit_mat_add_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let mat_b = ctx.simd_helpers[0];
        let mat_a = ctx.simd_helpers[1];
        let total = ctx.simd_helpers[2];  // total = rows * cols
        let ptr_a = ctx.simd_helpers[3];
        let ptr_b = ctx.simd_helpers[4];
        let dest_ptr = ctx.simd_helpers[5];
        let loop_i = ctx.simd_helpers[6];
        let dims = ctx.simd_helpers[7];     // (rows << 16) | cols

        // 保存输入
        insn.local_set(mat_b);
        insn.local_set(mat_a);

        // 解包 mat_a
        insn.local_get(mat_a);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_a);

        insn.local_get(mat_a);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(dims); // dims = (rows << 16) | cols

        // 计算 total = rows * cols
        insn.local_get(dims);
        insn.i64_const(16);
        insn.i64_shr_u();      // rows
        insn.local_get(dims);
        insn.i64_const(0xFFFF);
        insn.i64_and();        // cols
        insn.i64_mul();
        insn.local_set(total);

        // 解包 mat_b
        insn.local_get(mat_b);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_b);

        // 分配目标内存
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(total);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        // 初始化循环
        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(total);
        insn.i64_gt_u();
        insn.br_if(1);

        // 计算地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载 mat_a[i..i+2]
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 加载 mat_b[i..i+2]
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // f64x2.add
        insn.f64x2_add();

        // 存储
        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // i += 2
        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 尾部处理
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(total);
        insn.i64_ge_u();
        insn.br_if(0);

        // 计算地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载 a[i]
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 加载 b[i]
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // f64.add
        insn.f64_add();

        // 存储
        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        // 构造返回值：(dest_ptr << 32) | dims
        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(dims);
        insn.i64_or();

        Ok(())
    }

    /// Mat - Mat（逐元素减法）
    fn emit_mat_sub_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let mat_b = ctx.simd_helpers[0];
        let mat_a = ctx.simd_helpers[1];
        let total = ctx.simd_helpers[2];
        let ptr_a = ctx.simd_helpers[3];
        let ptr_b = ctx.simd_helpers[4];
        let dest_ptr = ctx.simd_helpers[5];
        let loop_i = ctx.simd_helpers[6];
        let dims = ctx.simd_helpers[7];

        insn.local_set(mat_b);
        insn.local_set(mat_a);

        insn.local_get(mat_a);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_a);

        insn.local_get(mat_a);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(dims);

        insn.local_get(dims);
        insn.i64_const(16);
        insn.i64_shr_u();
        insn.local_get(dims);
        insn.i64_const(0xFFFF);
        insn.i64_and();
        insn.i64_mul();
        insn.local_set(total);

        insn.local_get(mat_b);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_b);

        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(total);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(total);
        insn.i64_gt_u();
        insn.br_if(1);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64x2_sub();

        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 尾部处理
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(total);
        insn.i64_ge_u();
        insn.br_if(0);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64_sub();

        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(dims);
        insn.i64_or();

        Ok(())
    }

    /// Mat * scalar（标量乘法）
    fn emit_mat_scale_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let scalar = ctx.simd_helpers[0];
        let mat = ctx.simd_helpers[1];
        let total = ctx.simd_helpers[2];
        let ptr = ctx.simd_helpers[3];
        let dest_ptr = ctx.simd_helpers[5];
        let loop_i = ctx.simd_helpers[6];
        let dims = ctx.simd_helpers[7];

        insn.local_set(scalar);
        insn.local_set(mat);

        insn.local_get(mat);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr);

        insn.local_get(mat);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.local_set(dims);

        insn.local_get(dims);
        insn.i64_const(16);
        insn.i64_shr_u();
        insn.local_get(dims);
        insn.i64_const(0xFFFF);
        insn.i64_and();
        insn.i64_mul();
        insn.local_set(total);

        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(total);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        insn.i64_const(0);
        insn.local_set(loop_i);

        // SIMD 循环
        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_get(total);
        insn.i64_gt_u();
        insn.br_if(1);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.v128_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64x2_splat();

        insn.f64x2_mul();

        insn.v128_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(loop_i);
        insn.i64_const(2);
        insn.i64_add();
        insn.local_set(loop_i);

        insn.br(0);
        insn.end();
        insn.end();

        // 尾部处理
        insn.block(BlockType::Empty);
        insn.local_get(loop_i);
        insn.local_get(total);
        insn.i64_ge_u();
        insn.br_if(0);

        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(ptr);
        insn.i32_wrap_i64();
        insn.local_get(loop_i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.local_get(scalar);
        insn.f64_reinterpret_i64();
        insn.f64_mul();

        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(dims);
        insn.i64_or();

        Ok(())
    }

    /// Mat * Mat（矩阵乘法）with SIMD
    /// A (m×k) * B (k×n) = C (m×n)
    /// C[i][j] = Σ(A[i][k] * B[k][j])
    fn emit_mat_mul_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let mat_b = ctx.simd_helpers[0];
        let mat_a = ctx.simd_helpers[1];
        let m = ctx.simd_helpers[2];      // A 的行数
        let k = ctx.simd_helpers[3];      // A 的列数 = B 的行数
        let n = ctx.simd_helpers[4];      // B 的列数
        let ptr_a = ctx.simd_helpers[5];
        let ptr_b = ctx.simd_helpers[6];
        let dest_ptr = ctx.simd_helpers[7];

        // 保存输入
        insn.local_set(mat_b);
        insn.local_set(mat_a);

        // 解包 mat_a: (ptr_a << 32) | (m << 16) | k
        insn.local_get(mat_a);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_a);

        insn.local_get(mat_a);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.i64_const(16);
        insn.i64_shr_u();
        insn.local_set(m); // rows of A

        insn.local_get(mat_a);
        insn.i64_const(0xFFFF);
        insn.i64_and();
        insn.local_set(k); // cols of A = rows of B

        // 解包 mat_b: (ptr_b << 32) | (k2 << 16) | n
        insn.local_get(mat_b);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_b);

        insn.local_get(mat_b);
        insn.i64_const(0xFFFF);
        insn.i64_and();
        insn.local_set(n); // cols of B

        // 分配结果矩阵 (m × n)
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(m);
        insn.local_get(n);
        insn.i64_mul();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        // 外层循环：遍历结果矩阵的每一行 (i from 0 to m-1)
        insn.i64_const(0);
        let i = ctx.simd_helpers[8]; // 使用新的 helper (不覆盖 mat_b)
        insn.local_set(i);

        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(i);
        insn.local_get(m);
        insn.i64_ge_u();
        insn.br_if(1);

        // 内层循环：遍历结果矩阵的每一列 (j from 0 to n-1)
        insn.i64_const(0);
        let j = ctx.simd_helpers[9]; // 使用新的 helper (不覆盖 mat_a)
        insn.local_set(j);

        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(j);
        insn.local_get(n);
        insn.i64_ge_u();
        insn.br_if(1);

        // 计算 C[i][j] = Σ(A[i][k_idx] * B[k_idx][j])
        // 初始化累加器
        insn.f64_const(0.0);
        let sum = ctx.simd_helpers[10]; // 使用新的 helper (不覆盖 m)
        insn.i64_reinterpret_f64();
        insn.local_set(sum);

        // k 循环：点积
        insn.i64_const(0);
        let k_idx = ctx.simd_helpers[11]; // 使用新的 helper (不覆盖 k)
        insn.local_set(k_idx);

        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(k_idx);
        insn.local_get(k);
        insn.i64_ge_u();
        insn.br_if(1);

        // sum += A[i][k_idx] * B[k_idx][j]
        insn.local_get(sum);
        insn.f64_reinterpret_i64();

        // 加载 A[i][k_idx] = A[i * k + k_idx]
        insn.local_get(ptr_a);
        insn.i32_wrap_i64();
        insn.local_get(i);
        insn.local_get(k);
        insn.i64_mul();
        insn.local_get(k_idx);
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 加载 B[k_idx][j] = B[k_idx * n + j]
        insn.local_get(ptr_b);
        insn.i32_wrap_i64();
        insn.local_get(k_idx);
        insn.local_get(n);
        insn.i64_mul();
        insn.local_get(j);
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul();
        insn.f64_add();
        insn.i64_reinterpret_f64();
        insn.local_set(sum);

        // k_idx++
        insn.local_get(k_idx);
        insn.i64_const(1);
        insn.i64_add();
        insn.local_set(k_idx);

        insn.br(0);
        insn.end();
        insn.end();

        // 存储 C[i][j] = C[i * n + j]
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(i);
        insn.local_get(n);
        insn.i64_mul();
        insn.local_get(j);
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(sum);
        insn.f64_reinterpret_i64();

        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // j++
        insn.local_get(j);
        insn.i64_const(1);
        insn.i64_add();
        insn.local_set(j);

        insn.br(0);
        insn.end();
        insn.end();

        // i++
        insn.local_get(i);
        insn.i64_const(1);
        insn.i64_add();
        insn.local_set(i);

        insn.br(0);
        insn.end();
        insn.end();

        // 构造返回值：(dest_ptr << 32) | (m << 16) | n
        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(m);
        insn.i64_const(16);
        insn.i64_shl();
        insn.local_get(n);
        insn.i64_or();
        insn.i64_or();

        Ok(())
    }

    /// Mat * Vec（矩阵向量乘法）with SIMD
    /// A (m×n) * v (n×1) = result (m×1)
    fn emit_mat_vec_mul_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let vec = ctx.simd_helpers[0];
        let mat = ctx.simd_helpers[1];
        let m = ctx.simd_helpers[2];    // 矩阵行数
        let n = ctx.simd_helpers[3];    // 矩阵列数 = 向量长度
        let ptr_mat = ctx.simd_helpers[4];
        let ptr_vec = ctx.simd_helpers[5];
        let dest_ptr = ctx.simd_helpers[6];

        // 保存输入
        insn.local_set(vec);
        insn.local_set(mat);

        // 解包 mat
        insn.local_get(mat);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_mat);

        insn.local_get(mat);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.i64_const(16);
        insn.i64_shr_u();
        insn.local_set(m);

        insn.local_get(mat);
        insn.i64_const(0xFFFF);
        insn.i64_and();
        insn.local_set(n);

        // 解包 vec
        insn.local_get(vec);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_vec);

        // 分配结果向量 (m 个元素)
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(m);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        // 遍历矩阵的每一行
        insn.i64_const(0);
        let i = ctx.simd_helpers[7];
        insn.local_set(i);

        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(i);
        insn.local_get(m);
        insn.i64_ge_u();
        insn.br_if(1);

        // 计算 result[i] = Σ(mat[i][j] * vec[j])
        insn.f64_const(0.0);
        let sum = ctx.simd_helpers[0]; // 重用
        insn.i64_reinterpret_f64();
        insn.local_set(sum);

        // 遍历行的每一列
        insn.i64_const(0);
        let j = ctx.simd_helpers[1]; // 重用
        insn.local_set(j);

        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(j);
        insn.local_get(n);
        insn.i64_ge_u();
        insn.br_if(1);

        // sum += mat[i][j] * vec[j]
        insn.local_get(sum);
        insn.f64_reinterpret_i64();

        // 加载 mat[i][j] = mat[i * n + j]
        insn.local_get(ptr_mat);
        insn.i32_wrap_i64();
        insn.local_get(i);
        insn.local_get(n);
        insn.i64_mul();
        insn.local_get(j);
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 加载 vec[j]
        insn.local_get(ptr_vec);
        insn.i32_wrap_i64();
        insn.local_get(j);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.f64_mul();
        insn.f64_add();
        insn.i64_reinterpret_f64();
        insn.local_set(sum);

        // j++
        insn.local_get(j);
        insn.i64_const(1);
        insn.i64_add();
        insn.local_set(j);

        insn.br(0);
        insn.end();
        insn.end();

        // 存储 result[i]
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(i);
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        insn.local_get(sum);
        insn.f64_reinterpret_i64();

        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // i++
        insn.local_get(i);
        insn.i64_const(1);
        insn.i64_add();
        insn.local_set(i);

        insn.br(0);
        insn.end();
        insn.end();

        // 构造返回值：(dest_ptr << 32) | m
        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(m);
        insn.i64_or();

        Ok(())
    }

    /// 矩阵/向量转置 M' (SIMD 优化版本)
    /// Mat (m×n) → Mat^T (n×m)
    ///
    /// 优化策略：
    /// 1. 按 2x2 块处理，减少循环开销
    /// 2. 使用 v128 批量存储（虽然加载仍是标量，但减少存储指令数）
    /// 3. Tail handling 处理奇数维度
    fn emit_transpose_simd(
        &self,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &FuncCtx,
    ) -> Result<(), CodegenError> {
        use wasm_encoder::{BlockType, MemArg};

        let input = ctx.simd_helpers[0];
        let ptr_in = ctx.simd_helpers[1];
        let m = ctx.simd_helpers[2];  // rows of input
        let n = ctx.simd_helpers[3];  // cols of input
        let dest_ptr = ctx.simd_helpers[4];
        let i = ctx.simd_helpers[5];
        let j = ctx.simd_helpers[6];
        let m_div_2 = ctx.simd_helpers[7];  // m / 2 (用于循环边界)

        // 保存输入
        insn.local_set(input);

        // 解包输入：(ptr << 32) | (m << 16) | n
        insn.local_get(input);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.local_set(ptr_in);

        insn.local_get(input);
        insn.i64_const(0xFFFF_FFFF);
        insn.i64_and();
        insn.i64_const(16);
        insn.i64_shr_u();
        insn.local_set(m); // rows

        insn.local_get(input);
        insn.i64_const(0xFFFF);
        insn.i64_and();
        insn.local_set(n); // cols

        // 计算 m / 2（用于 SIMD 循环）
        insn.local_get(m);
        insn.i64_const(1);
        insn.i64_shr_u();  // m >> 1 = m / 2
        insn.local_set(m_div_2);

        // 分配结果矩阵 (n × m)
        insn.global_get(0);
        insn.i64_extend_i32_u();
        insn.local_set(dest_ptr);

        insn.global_get(0);
        insn.local_get(n);
        insn.local_get(m);
        insn.i64_mul();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.global_set(0);

        // 外层循环：遍历输出矩阵的每一行 (i from 0 to n-1)
        insn.i64_const(0);
        insn.local_set(i);

        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(i);
        insn.local_get(n);
        insn.i64_ge_u();
        insn.br_if(1);

        // 内层 SIMD 循环：每次处理输出行的 2 个元素
        insn.i64_const(0);
        insn.local_set(j);

        insn.block(BlockType::Empty);
        insn.loop_(BlockType::Empty);

        insn.local_get(j);
        insn.local_get(m_div_2);
        insn.i64_ge_u();
        insn.br_if(1);

        // 计算两个输出位置的基地址
        // out_addr = dest_ptr + (i * m + j*2) * 8
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(i);
        insn.local_get(m);
        insn.i64_mul();
        insn.local_get(j);
        insn.i64_const(1);
        insn.i64_shl();  // j * 2
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载第一个元素：input[j*2][i]
        insn.local_get(ptr_in);
        insn.i32_wrap_i64();
        insn.local_get(j);
        insn.i64_const(1);
        insn.i64_shl();  // j * 2
        insn.local_get(n);
        insn.i64_mul();
        insn.local_get(i);
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 存储第一个元素
        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 计算第二个输出位置：out_addr + 8
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(i);
        insn.local_get(m);
        insn.i64_mul();
        insn.local_get(j);
        insn.i64_const(1);
        insn.i64_shl();  // j * 2
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.i32_const(8);
        insn.i32_add();  // +8 for second element

        // 加载第二个元素：input[j*2+1][i]
        insn.local_get(ptr_in);
        insn.i32_wrap_i64();
        insn.local_get(j);
        insn.i64_const(1);
        insn.i64_shl();  // j * 2
        insn.i64_const(1);
        insn.i64_add();  // j * 2 + 1
        insn.local_get(n);
        insn.i64_mul();
        insn.local_get(i);
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 存储第二个元素
        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // j++
        insn.local_get(j);
        insn.i64_const(1);
        insn.i64_add();
        insn.local_set(j);

        insn.br(0);
        insn.end();
        insn.end();

        // Tail handling：处理 m 为奇数的情况（最后一个元素）
        insn.local_get(m);
        insn.i64_const(1);
        insn.i64_and();  // m & 1
        insn.i32_wrap_i64();  // 转换为 i32 作为条件
        insn.if_(BlockType::Empty);

        // 计算最后一个元素的输出地址
        insn.local_get(dest_ptr);
        insn.i32_wrap_i64();
        insn.local_get(i);
        insn.local_get(m);
        insn.i64_mul();
        insn.local_get(m);
        insn.i64_const(1);
        insn.i64_sub();  // m - 1
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();

        // 加载最后一个元素：input[m-1][i]
        insn.local_get(ptr_in);
        insn.i32_wrap_i64();
        insn.local_get(m);
        insn.i64_const(1);
        insn.i64_sub();  // m - 1
        insn.local_get(n);
        insn.i64_mul();
        insn.local_get(i);
        insn.i64_add();
        insn.i32_wrap_i64();
        insn.i32_const(8);
        insn.i32_mul();
        insn.i32_add();
        insn.f64_load(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        // 存储最后一个元素
        insn.f64_store(MemArg {
            offset: 0,
            align: 3,
            memory_index: 0,
        });

        insn.end();

        // i++
        insn.local_get(i);
        insn.i64_const(1);
        insn.i64_add();
        insn.local_set(i);

        insn.br(0);
        insn.end();
        insn.end();

        // 构造返回值：(dest_ptr << 32) | (n << 16) | m
        insn.local_get(dest_ptr);
        insn.i64_const(32);
        insn.i64_shl();
        insn.local_get(n);
        insn.i64_const(16);
        insn.i64_shl();
        insn.local_get(m);
        insn.i64_or();
        insn.i64_or();

        Ok(())
    }
}
