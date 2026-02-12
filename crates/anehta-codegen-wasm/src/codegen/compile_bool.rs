use super::*;

impl WasmCodegen {
    pub(super) fn compile_boolean_expr(
        &self,
        expr: &BooleanExpr,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        match expr {
            BooleanExpr::Comparison {
                left, op, right, ..
            } => {
                let lt = self.infer_expr_type(left, ctx);
                let rt = self.infer_expr_type(right, ctx);
                if lt == AhType::Float || rt == AhType::Float {
                    self.emit_float_operand(left, insn, ctx)?;
                    self.emit_float_operand(right, insn, ctx)?;
                    match op {
                        ComparisonOp::Gt => { insn.f64_gt(); }
                        ComparisonOp::Lt => { insn.f64_lt(); }
                        ComparisonOp::GtEq => { insn.f64_ge(); }
                        ComparisonOp::LtEq => { insn.f64_le(); }
                        ComparisonOp::Eq => { insn.f64_eq(); }
                        ComparisonOp::NotEq => { insn.f64_ne(); }
                    }
                } else {
                    self.compile_expr(left, insn, ctx)?;
                    self.compile_expr(right, insn, ctx)?;
                    match op {
                        ComparisonOp::Gt => { insn.i64_gt_s(); }
                        ComparisonOp::Lt => { insn.i64_lt_s(); }
                        ComparisonOp::GtEq => { insn.i64_ge_s(); }
                        ComparisonOp::LtEq => { insn.i64_le_s(); }
                        ComparisonOp::Eq => { insn.i64_eq(); }
                        ComparisonOp::NotEq => { insn.i64_ne(); }
                    }
                }
            }
            BooleanExpr::Logical {
                left, op, right, ..
            } => {
                self.compile_boolean_expr(left, insn, ctx)?;
                self.compile_boolean_expr(right, insn, ctx)?;
                match op {
                    LogicalOp::And => {
                        insn.i32_and();
                    }
                    LogicalOp::Or => {
                        insn.i32_or();
                    }
                }
            }
            BooleanExpr::Grouped(inner) => {
                self.compile_boolean_expr(inner, insn, ctx)?;
            }
        }
        Ok(())
    }
}
