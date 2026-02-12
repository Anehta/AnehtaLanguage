use super::*;

impl WasmCodegen {
    /// Best-effort inference of the type an expression will produce at runtime.
    pub(super) fn infer_expr_type(&self, expr: &Expr, ctx: &FuncCtx) -> AhType {
        match expr {
            Expr::StringLit(..) => AhType::Str,
            Expr::Number(s, _) => {
                if s.contains('.') { AhType::Float } else { AhType::Int }
            }
            Expr::Bool(..) => AhType::Int,
            Expr::Variable(name, _) => {
                ctx.var_types.get(name).copied().unwrap_or(AhType::Int)
            }
            Expr::BinaryOp { left, op, right, .. } => {
                let lt = self.infer_expr_type(left, ctx);
                let rt = self.infer_expr_type(right, ctx);
                match op {
                    BinaryOp::Add => {
                        if lt == AhType::Mat || rt == AhType::Mat {
                            AhType::Mat
                        } else if lt == AhType::Vec || rt == AhType::Vec {
                            AhType::Vec
                        } else if lt == AhType::Str || rt == AhType::Str {
                            AhType::Str
                        } else if lt == AhType::Float || rt == AhType::Float {
                            AhType::Float
                        } else {
                            AhType::Int
                        }
                    }
                    BinaryOp::Sub => {
                        if lt == AhType::Mat || rt == AhType::Mat {
                            AhType::Mat
                        } else if lt == AhType::Vec || rt == AhType::Vec {
                            AhType::Vec
                        } else if lt == AhType::Float || rt == AhType::Float {
                            AhType::Float
                        } else {
                            AhType::Int
                        }
                    }
                    BinaryOp::Mul => {
                        // mat * mat → mat, mat * vec → vec, mat * scalar → mat
                        if lt == AhType::Mat && rt == AhType::Mat {
                            AhType::Mat
                        } else if lt == AhType::Mat && rt == AhType::Vec {
                            AhType::Vec
                        } else if lt == AhType::Mat {
                            AhType::Mat
                        } else if rt == AhType::Mat {
                            AhType::Mat
                        } else if lt == AhType::Vec || rt == AhType::Vec {
                            AhType::Vec
                        } else if lt == AhType::Float || rt == AhType::Float {
                            AhType::Float
                        } else {
                            AhType::Int
                        }
                    }
                    BinaryOp::Div | BinaryOp::Mod | BinaryOp::Power => {
                        if lt == AhType::Mat {
                            AhType::Mat
                        } else if lt == AhType::Vec {
                            AhType::Vec
                        } else if lt == AhType::Float || rt == AhType::Float {
                            AhType::Float
                        } else {
                            AhType::Int
                        }
                    }
                    BinaryOp::DotPow => {
                        // Element-wise power: vec/mat .^ scalar → vec/mat
                        if lt == AhType::Vec || lt == AhType::Mat {
                            lt
                        } else if lt == AhType::Float || rt == AhType::Float {
                            AhType::Float
                        } else {
                            AhType::Int
                        }
                    }
                    BinaryOp::Rand => AhType::Int,
                    BinaryOp::At => AhType::Float,
                    BinaryOp::Hash => AhType::Vec,
                    BinaryOp::Backslash => {
                        // mat \ vec → vec, mat \ mat → mat (solve Ax=b)
                        if rt == AhType::Vec {
                            AhType::Vec
                        } else if rt == AhType::Mat {
                            AhType::Mat
                        } else {
                            AhType::Float
                        }
                    }
                }
            }
            Expr::CallFunc(call) => {
                // Built-in conversion functions
                if call.name == "int" && call.args.len() == 1 {
                    return AhType::Int;
                }
                if call.name == "float" && call.args.len() == 1 {
                    return AhType::Float;
                }
                // Check named function return types first
                if let Some(&ty) = self.func_return_types.get(&call.name) {
                    return ty;
                }
                // Check if it's a closure variable call
                if let Some(AhType::Closure(id)) = ctx.var_types.get(&call.name) {
                    if let Some(info) = self.closures.get(*id as usize) {
                        return info.return_type;
                    }
                }
                AhType::Int
            }
            Expr::Grouped(inner) => self.infer_expr_type(inner, ctx),
            Expr::UnaryOp { operand, .. } => {
                ctx.var_types.get(operand).copied().unwrap_or(AhType::Int)
            }
            Expr::Closure(closure) => {
                // Look up the closure ID by span
                let key = (closure.span.line, closure.span.column);
                if let Some(&id) = self.closure_span_map.get(&key) {
                    AhType::Closure(id)
                } else {
                    AhType::Int
                }
            }
            Expr::TableLiteral(table) => {
                let key = (table.span.line, table.span.column);
                if let Some(&id) = self.table_type_span_map.get(&key) {
                    AhType::Table(id)
                } else {
                    AhType::Int
                }
            }
            Expr::FieldAccess(fa) => {
                let obj_ty = self.infer_expr_type(&fa.object, ctx);
                // Mat field access: .T → Mat, .det/.rows/.cols → Float/Int, .inv → Mat
                if obj_ty == AhType::Mat {
                    match fa.field.as_str() {
                        "T" | "inv" => return AhType::Mat,
                        "det" => return AhType::Float,
                        "rows" | "cols" => return AhType::Int,
                        _ => {}
                    }
                }
                // Vec field access: .len → Int, single char swizzle → Float, multi swizzle → Vec
                if obj_ty == AhType::Vec {
                    if fa.field == "len" {
                        return AhType::Int;
                    }
                    // Single char swizzle (x/y/z/w/r/g/b/a) → Float
                    if fa.field.len() == 1 && "xyzwrgba".contains(&fa.field) {
                        return AhType::Float;
                    }
                    // Multi-char swizzle → Vec
                    if fa.field.chars().all(|c| "xyzwrgba".contains(c)) {
                        return AhType::Vec;
                    }
                }
                if let AhType::Table(id) = obj_ty {
                    if let Some(info) = self.table_types.get(id as usize) {
                        return info.fields.get(&fa.field).copied().unwrap_or(AhType::Int);
                    }
                }
                AhType::Int
            }
            Expr::IndexAccess(ia) => {
                let obj_ty = self.infer_expr_type(&ia.object, ctx);
                let idx_ty = self.infer_expr_type(&ia.index, ctx);
                // Check if index is a Range (slicing), Vec (fancy indexing), or BooleanExpr (masking)
                if matches!(&*ia.index, Expr::Range { .. } | Expr::BooleanExpr(_)) || idx_ty == AhType::Vec {
                    // Slicing, fancy indexing, and masking return Vec
                    // (Mat masking flattens to Vec)
                    if obj_ty == AhType::Vec {
                        return AhType::Vec;
                    } else if obj_ty == AhType::Mat {
                        return AhType::Vec;  // Mat masking flattens to Vec
                    }
                    return obj_ty;
                }
                // Regular indexing
                if obj_ty == AhType::Mat {
                    AhType::Float  // m[i,j] → Float
                } else if obj_ty == AhType::Vec {
                    AhType::Float
                } else {
                    AhType::Int
                }
            }
            Expr::MethodCall(mc) => {
                let callee_ty = self.infer_expr_type(&mc.callee, ctx);
                if let AhType::Closure(id) = callee_ty {
                    if let Some(info) = self.closures.get(id as usize) {
                        return info.return_type;
                    }
                }
                AhType::Int
            }
            Expr::VecLiteral(_) => AhType::Vec,
            Expr::MatLiteral(_) => AhType::Mat,
            Expr::Transpose(t) => self.infer_expr_type(&t.operand, ctx), // Transpose preserves type (Mat→Mat, Vec→Vec)
            Expr::Range { .. } => AhType::Int, // Range is not a standalone value, only used in indexing
            Expr::BooleanExpr(_) => AhType::Int, // BooleanExpr only meaningful in masking context, not standalone
        }
    }

    /// Infer the return type of a block by scanning for the first return statement.
    pub(super) fn infer_block_return_type(block: &Block, codegen: &WasmCodegen, ctx: &FuncCtx) -> AhType {
        for stmt in &block.statements {
            match stmt {
                Statement::Return(ret) => {
                    if let Some(first) = ret.values.first() {
                        return codegen.infer_expr_type(first, ctx);
                    }
                    return AhType::Int;
                }
                Statement::IfStmt(if_stmt) => {
                    let ty = Self::infer_block_return_type(&if_stmt.body, codegen, ctx);
                    if ty != AhType::Int {
                        return ty;
                    }
                }
                Statement::ForStmt(for_stmt) => {
                    let ty = Self::infer_block_return_type(&for_stmt.body, codegen, ctx);
                    if ty != AhType::Int {
                        return ty;
                    }
                }
                _ => {}
            }
        }
        AhType::Int
    }
}
