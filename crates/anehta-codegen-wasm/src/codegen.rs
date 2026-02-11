use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;

use anehta_lexer::Span;
use anehta_parser::{
    Assignment, BinaryOp, Block, BooleanExpr, CallFunc, ClosureBody, ClosureExpr, ComparisonOp,
    Expr, FieldAccess, FieldAssign, ForStmt, FuncDecl, IfStmt, IndexAccess, IndexAssign,
    LogicalOp, MethodCall, Program, ReturnStmt, Statement, TableLiteral, TimerStmt, UnaryOp,
    VarDecl,
};

fn codegen_err(message: impl Into<String>, span: &Span) -> CodegenError {
    CodegenError::Error {
        message: message.into(),
        line: span.line,
        column: span.column,
    }
}

use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, DataSection, ElementSection, Elements, EntityType,
    ExportKind, ExportSection, Function, FunctionSection, GlobalSection, GlobalType, ImportSection,
    MemArg, MemorySection, MemoryType, Module, RefType, TableSection, TableType, TypeSection,
    ValType,
};

// ---------------------------------------------------------------------------
// Type inference helpers
// ---------------------------------------------------------------------------

/// Simple type tag used to distinguish integer vs string vs closure values at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AhType {
    Int,
    Str,
    /// Closure value. The u32 is the closure ID (index into closures vec).
    Closure(u32),
    /// Table value. The u32 is the table type ID (index into table_types vec).
    Table(u32),
}

/// Map a source-level type name (e.g. "int", "str") to our internal tag.
fn type_name_to_ah(name: &str) -> AhType {
    match name {
        "str" | "string" => AhType::Str,
        _ => AhType::Int,
    }
}

// ---------------------------------------------------------------------------
// Closure support types
// ---------------------------------------------------------------------------

/// Information about a single closure collected during the analysis pass.
#[allow(dead_code)]
struct ClosureInfo {
    /// Internal function name, e.g. `__closure_0`
    name: String,
    /// Function index in the WASM module
    func_idx: u32,
    /// Type index for the closure function: (i32, i64*N) -> i64
    type_idx: u32,
    /// Names of captured variables (in order)
    captures: Vec<String>,
    /// Number of explicit parameters (not counting env_ptr)
    param_count: usize,
    /// Index into the WASM function table (0, 1, 2, ...)
    table_idx: u32,
    /// Inferred return type of the closure body
    return_type: AhType,
}

/// Compile-time type info for a table literal's fields.
struct TableTypeInfo {
    fields: HashMap<String, AhType>,
}

/// WASM code generator: compiles AST into .wasm binary
pub struct WasmCodegen {
    /// Type section entries: each is (params, results)
    types: Vec<(Vec<ValType>, Vec<ValType>)>,
    /// Number of imported functions (they come before local function indices)
    num_imports: u32,
    /// Map from function name to (function_index, type_index)
    func_map: HashMap<String, (u32, u32)>,
    /// Next function index to assign
    next_func_idx: u32,
    /// String pool: maps a string literal to (offset, length) in the data segment
    string_pool: HashMap<String, (u32, u32)>,
    /// Raw bytes that will be placed into linear memory at offset 0
    string_data: Vec<u8>,
    /// Inferred return types for user-defined functions (by name)
    func_return_types: HashMap<String, AhType>,
    /// Counter for generating unique closure names
    closure_counter: u32,
    /// Collected closure info (one per closure expression in the program)
    closures: Vec<ClosureInfo>,
    /// Maps closure span (line, column) to closure ID for lookup during compilation
    closure_span_map: HashMap<(usize, usize), u32>,
    /// Compile-time type info for table literals (one per table literal)
    table_types: Vec<TableTypeInfo>,
    /// Maps table literal span (line, column) to table_type_id
    table_type_span_map: HashMap<(usize, usize), u32>,
}

/// Context for compiling a single function body
struct FuncCtx {
    /// Map from variable name to local index
    locals: HashMap<String, u32>,
    /// Next local index to assign
    next_local: u32,
    /// Additional locals declared in the body (beyond parameters)
    extra_locals: Vec<ValType>,
    /// Current nesting depth of break/continue targets.
    loop_depth_stack: Vec<LoopInfo>,
    /// Current block nesting depth (incremented for every block/loop/if)
    block_depth: u32,
    /// Pre-allocated temp local indices for power operations (groups of 3)
    power_temps: Vec<(u32, u32, u32)>,
    /// Index into power_temps for the next power operation to consume
    power_temps_cursor: usize,
    /// Pre-allocated temp local indices for timer blocks (start, end)
    timer_temps: Vec<(u32, u32)>,
    /// Index into timer_temps for the next timer block to consume
    timer_temps_cursor: usize,
    /// Inferred types for local variables
    var_types: HashMap<String, AhType>,
    /// Pre-allocated temp local indices for closure call_indirect argument reordering.
    /// Each entry is a Vec of local indices for saving arguments.
    closure_call_temps: Vec<Vec<u32>>,
    /// Index into closure_call_temps for the next closure call to consume
    closure_call_temps_cursor: usize,
    /// Pre-allocated temp locals for closure env_ptr during closure creation (one per closure with captures)
    closure_env_temps: Vec<u32>,
    /// Index into closure_env_temps for the next closure creation to consume
    closure_env_temps_cursor: usize,
    /// Pre-allocated temp locals for table literal construction (one per table literal)
    table_temps: Vec<u32>,
    /// Index into table_temps for the next table literal to consume
    table_temps_cursor: usize,
    /// Table variables owned by this function (created via table literals)
    owned_tables: Vec<String>,
    /// Parameter names (borrowed references, not freed by this function)
    param_names: HashSet<String>,
    /// Table variables captured by closures in this function (must NOT be freed)
    captured_tables: HashSet<String>,
    /// Pre-allocated temp locals for saving return values during table cleanup
    return_save_temps: Vec<Vec<u32>>,
    /// Index into return_save_temps for the next return statement to consume
    return_save_temps_cursor: usize,
}

#[derive(Clone, Copy)]
struct LoopInfo {
    /// Label depth for `break` (the outer block)
    break_depth: u32,
    /// Label depth for `continue` (the loop itself)
    continue_depth: u32,
}

impl FuncCtx {
    fn new() -> Self {
        Self {
            locals: HashMap::new(),
            next_local: 0,
            extra_locals: Vec::new(),
            loop_depth_stack: Vec::new(),
            block_depth: 0,
            power_temps: Vec::new(),
            power_temps_cursor: 0,
            timer_temps: Vec::new(),
            timer_temps_cursor: 0,
            var_types: HashMap::new(),
            closure_call_temps: Vec::new(),
            closure_call_temps_cursor: 0,
            closure_env_temps: Vec::new(),
            closure_env_temps_cursor: 0,
            table_temps: Vec::new(),
            table_temps_cursor: 0,
            owned_tables: Vec::new(),
            param_names: HashSet::new(),
            captured_tables: HashSet::new(),
            return_save_temps: Vec::new(),
            return_save_temps_cursor: 0,
        }
    }

    fn new_with_var_types(var_types: HashMap<String, AhType>) -> Self {
        let mut ctx = Self::new();
        ctx.var_types = var_types;
        ctx
    }

    fn add_param(&mut self, name: &str) -> u32 {
        let idx = self.next_local;
        self.locals.insert(name.to_string(), idx);
        self.next_local += 1;
        idx
    }

    /// Add a parameter with a specific ValType (used for i32 env_ptr in closures)
    fn add_param_with_type(&mut self, name: &str, _vt: ValType) -> u32 {
        let idx = self.next_local;
        self.locals.insert(name.to_string(), idx);
        self.next_local += 1;
        idx
    }

    fn declare_local(&mut self, name: &str) -> u32 {
        if let Some(&idx) = self.locals.get(name) {
            return idx;
        }
        let idx = self.next_local;
        self.locals.insert(name.to_string(), idx);
        self.next_local += 1;
        self.extra_locals.push(ValType::I64);
        idx
    }

    fn alloc_anonymous_local(&mut self) -> u32 {
        let idx = self.next_local;
        self.next_local += 1;
        self.extra_locals.push(ValType::I64);
        idx
    }

    fn get_local(&self, name: &str) -> Option<u32> {
        self.locals.get(name).copied()
    }

    /// Pre-allocate a group of 3 temp locals for a power operation
    fn alloc_power_temps(&mut self) {
        let base = self.alloc_anonymous_local();
        let exp = self.alloc_anonymous_local();
        let result = self.alloc_anonymous_local();
        self.power_temps.push((base, exp, result));
    }

    /// Claim the next pre-allocated power temp group
    fn claim_power_temps(&mut self) -> (u32, u32, u32) {
        let temps = self.power_temps[self.power_temps_cursor];
        self.power_temps_cursor += 1;
        temps
    }

    /// Pre-allocate a pair of temp locals for a timer block (start, end)
    fn alloc_timer_temps(&mut self) {
        let start = self.alloc_anonymous_local();
        let end = self.alloc_anonymous_local();
        self.timer_temps.push((start, end));
    }

    /// Claim the next pre-allocated timer temp pair
    fn claim_timer_temps(&mut self) -> (u32, u32) {
        let temps = self.timer_temps[self.timer_temps_cursor];
        self.timer_temps_cursor += 1;
        temps
    }

    /// Pre-allocate temp locals for a closure call_indirect (one per argument)
    fn alloc_closure_call_temps(&mut self, num_args: usize) {
        let mut temps = Vec::with_capacity(num_args);
        for _ in 0..num_args {
            temps.push(self.alloc_anonymous_local());
        }
        self.closure_call_temps.push(temps);
    }

    /// Claim the next pre-allocated closure call temp group
    fn claim_closure_call_temps(&mut self) -> Vec<u32> {
        let temps = self.closure_call_temps[self.closure_call_temps_cursor].clone();
        self.closure_call_temps_cursor += 1;
        temps
    }
}

impl WasmCodegen {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            num_imports: 0,
            func_map: HashMap::new(),
            next_func_idx: 0,
            string_pool: HashMap::new(),
            string_data: Vec::new(),
            func_return_types: HashMap::new(),
            closure_counter: 0,
            closures: Vec::new(),
            closure_span_map: HashMap::new(),
            table_types: Vec::new(),
            table_type_span_map: HashMap::new(),
        }
    }

    /// Register a type and return its index
    fn add_type(&mut self, params: Vec<ValType>, results: Vec<ValType>) -> u32 {
        // Check if an identical type already exists
        for (i, (p, r)) in self.types.iter().enumerate() {
            if p == &params && r == &results {
                return i as u32;
            }
        }
        let idx = self.types.len() as u32;
        self.types.push((params, results));
        idx
    }

    // -----------------------------------------------------------------------
    // String interning
    // -----------------------------------------------------------------------

    /// Intern a string literal and return its (offset, length) in the data segment.
    fn intern_string(&mut self, s: &str) -> (u32, u32) {
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
    fn collect_strings(&mut self, program: &Program) {
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

    // -----------------------------------------------------------------------
    // Table type collection pass
    // -----------------------------------------------------------------------

    /// Pre-pass: walk the program and register TableTypeInfo for each table literal.
    /// This enables compile-time type inference for field accesses (e.g. print dispatch).
    fn collect_table_types(&mut self, program: &Program) {
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
            _ => {}
        }
    }

    /// Re-infer table field types after closures are collected.
    /// During collect_table_types (Phase 0b), closure variables weren't known yet,
    /// so fields like `{asd: readB}` where readB is a closure were typed as Int.
    /// Now that closures are collected, we can walk VarDecl assignments to fix this.
    fn fixup_table_types(&mut self, program: &Program) {
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

    // -----------------------------------------------------------------------
    // Closure collection pass
    // -----------------------------------------------------------------------

    /// Walk the entire program AST and collect all closure expressions.
    /// For each closure, register a hidden WASM function and record its metadata.
    fn collect_closures(&mut self, program: &Program) {
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
    fn find_variables_expr(expr: &Expr, vars: &mut HashSet<String>) {
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

    fn find_variables_block(block: &Block, vars: &mut HashSet<String>) {
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
    fn find_declared_vars_block(block: &Block, declared: &mut HashSet<String>) {
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

    // -----------------------------------------------------------------------
    // Type inference
    // -----------------------------------------------------------------------

    /// Best-effort inference of the type an expression will produce at runtime.
    fn infer_expr_type(&self, expr: &Expr, ctx: &FuncCtx) -> AhType {
        match expr {
            Expr::StringLit(..) => AhType::Str,
            Expr::Number(..) | Expr::Bool(..) => AhType::Int,
            Expr::Variable(name, _) => {
                ctx.var_types.get(name).copied().unwrap_or(AhType::Int)
            }
            Expr::BinaryOp { left, op, right, .. } => {
                match op {
                    BinaryOp::Add => {
                        let lt = self.infer_expr_type(left, ctx);
                        let rt = self.infer_expr_type(right, ctx);
                        if lt == AhType::Str || rt == AhType::Str {
                            AhType::Str
                        } else {
                            AhType::Int
                        }
                    }
                    _ => AhType::Int,
                }
            }
            Expr::CallFunc(call) => {
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
            Expr::UnaryOp { .. } => AhType::Int,
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
                if let AhType::Table(id) = obj_ty {
                    if let Some(info) = self.table_types.get(id as usize) {
                        return info.fields.get(&fa.field).copied().unwrap_or(AhType::Int);
                    }
                }
                AhType::Int
            }
            Expr::IndexAccess(_) => AhType::Int,
            Expr::MethodCall(mc) => {
                let callee_ty = self.infer_expr_type(&mc.callee, ctx);
                if let AhType::Closure(id) = callee_ty {
                    if let Some(info) = self.closures.get(id as usize) {
                        return info.return_type;
                    }
                }
                AhType::Int
            }
        }
    }

    /// Infer the return type of a block by scanning for the first return statement.
    fn infer_block_return_type(block: &Block, codegen: &WasmCodegen, ctx: &FuncCtx) -> AhType {
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

    // -----------------------------------------------------------------------
    // Main compile entry point
    // -----------------------------------------------------------------------

    /// Compile a Program AST into WASM bytecode
    pub fn compile(&mut self, program: &Program) -> Result<Vec<u8>, CodegenError> {
        // Phase 0: Collect all string literals into the string pool
        self.collect_strings(program);

        // Phase 0b: Collect table type info for compile-time field type inference
        self.collect_table_types(program);

        // Phase 1: Collect all function declarations and build the function map.
        // Also determine imports needed.

        // Import: env.random(i64, i64) -> i64 (type index assigned later)
        let random_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let random_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_random".to_string(), (random_func_idx, random_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 1;

        // Import: env.print(i64) -> [] (for print calls -- integer print)
        let print_type_idx = self.add_type(vec![ValType::I64], vec![]);
        let print_func_idx = self.next_func_idx;
        self.func_map
            .insert("print".to_string(), (print_func_idx, print_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 2;

        // Import: env.input() -> i64 (for reading user input)
        let input_type_idx = self.add_type(vec![], vec![ValType::I64]);
        let input_func_idx = self.next_func_idx;
        self.func_map
            .insert("input".to_string(), (input_func_idx, input_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 3;

        // Import: env.clock() -> i64 (for timer blocks, returns ms)
        let clock_type_idx = self.add_type(vec![], vec![ValType::I64]);
        let clock_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_clock".to_string(), (clock_func_idx, clock_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 4;

        // Import: env.print_timer(i64) (for timer blocks, prints elapsed time)
        let print_timer_type_idx = self.add_type(vec![ValType::I64], vec![]);
        let print_timer_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_print_timer".to_string(), (print_timer_func_idx, print_timer_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 5;

        // Import: env.print_str(i64) (for printing string values)
        let print_str_type_idx = self.add_type(vec![ValType::I64], vec![]);
        let print_str_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_print_str".to_string(), (print_str_func_idx, print_str_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 6;

        // Import: env.str_concat(i64, i64) -> i64 (concatenate two packed strings)
        let str_concat_type_idx =
            self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let str_concat_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_str_concat".to_string(), (str_concat_func_idx, str_concat_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 7;

        // Import: env.table_new() -> i64 (create new table, return handle)
        let table_new_type_idx = self.add_type(vec![], vec![ValType::I64]);
        let table_new_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_table_new".to_string(), (table_new_func_idx, table_new_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 8;

        // Import: env.table_set(table_id: i64, key: i64, value: i64)
        let table_set_type_idx =
            self.add_type(vec![ValType::I64, ValType::I64, ValType::I64], vec![]);
        let table_set_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_table_set".to_string(), (table_set_func_idx, table_set_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 9;

        // Import: env.table_get(table_id: i64, key: i64) -> i64
        let table_get_type_idx =
            self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let table_get_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_table_get".to_string(), (table_get_func_idx, table_get_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 10;

        // Import: env.table_free(table_id: i64) (free table and its children recursively)
        let table_free_type_idx = self.add_type(vec![ValType::I64], vec![]);
        let table_free_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_table_free".to_string(), (table_free_func_idx, table_free_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 11;

        // Import: env.table_set_table(parent: i64, key: i64, child: i64) (set + register parent-child)
        let table_set_table_type_idx =
            self.add_type(vec![ValType::I64, ValType::I64, ValType::I64], vec![]);
        let table_set_table_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_table_set_table".to_string(), (table_set_table_func_idx, table_set_table_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 12;

        // Collect user-defined functions and their return types
        for stmt in &program.statements {
            if let Statement::FuncDecl(func) = stmt {
                let params: Vec<ValType> = func.params.iter().map(|_| ValType::I64).collect();
                let results: Vec<ValType> =
                    func.return_types.iter().map(|_| ValType::I64).collect();
                let type_idx = self.add_type(params, results);
                let func_idx = self.next_func_idx;
                self.func_map
                    .insert(func.name.clone(), (func_idx, type_idx));
                self.next_func_idx += 1;

                // Infer return type from the first return type annotation
                if let Some(first_ret) = func.return_types.first() {
                    self.func_return_types
                        .insert(func.name.clone(), type_name_to_ah(first_ret));
                }
            }
        }

        // Check if there are top-level statements (non-function declarations)
        let has_top_level = program
            .statements
            .iter()
            .any(|s| !matches!(s, Statement::FuncDecl(_)));

        if has_top_level {
            let start_type_idx = self.add_type(vec![], vec![]);
            let start_func_idx = self.next_func_idx;
            self.func_map
                .insert("_start".to_string(), (start_func_idx, start_type_idx));
            self.next_func_idx += 1;
        }

        // Collect closures (must happen after user functions and _start are registered)
        self.collect_closures(program);

        // Fixup table field types: now that closures are collected, re-infer fields
        // that were wrongly typed as Int during collect_table_types (Phase 0b).
        self.fixup_table_types(program);

        // Phase 2: Build sections

        // Type section
        let mut type_section = TypeSection::new();
        for (params, results) in &self.types {
            type_section
                .ty()
                .function(params.iter().copied(), results.iter().copied());
        }

        // Import section
        let mut import_section = ImportSection::new();
        import_section.import("env", "random", EntityType::Function(random_type_idx));
        import_section.import("env", "print", EntityType::Function(print_type_idx));
        import_section.import("env", "input", EntityType::Function(input_type_idx));
        import_section.import("env", "clock", EntityType::Function(clock_type_idx));
        import_section.import("env", "print_timer", EntityType::Function(print_timer_type_idx));
        import_section.import("env", "print_str", EntityType::Function(print_str_type_idx));
        import_section.import("env", "str_concat", EntityType::Function(str_concat_type_idx));
        import_section.import("env", "table_new", EntityType::Function(table_new_type_idx));
        import_section.import("env", "table_set", EntityType::Function(table_set_type_idx));
        import_section.import("env", "table_get", EntityType::Function(table_get_type_idx));
        import_section.import("env", "table_free", EntityType::Function(table_free_type_idx));
        import_section.import("env", "table_set_table", EntityType::Function(table_set_table_type_idx));

        // Function section (declares type index for each local function)
        let mut function_section = FunctionSection::new();
        // Code section (function bodies)
        let mut code_section = CodeSection::new();

        // Compile user-defined functions
        for stmt in &program.statements {
            if let Statement::FuncDecl(func) = stmt {
                let (_, type_idx) = self.func_map[&func.name];
                function_section.function(type_idx);
                let wasm_func = self.compile_func_decl(func)?;
                code_section.function(&wasm_func);
            }
        }

        // Compile _start function if there are top-level statements
        if has_top_level {
            let (_, start_type_idx) = self.func_map["_start"];
            function_section.function(start_type_idx);

            let mut ctx = FuncCtx::new();

            // Pre-scan top-level statements to declare all variables
            for stmt in &program.statements {
                if !matches!(stmt, Statement::FuncDecl(_)) {
                    self.prescan_stmt(stmt, &mut ctx);
                }
            }

            let mut func = Function::new(
                ctx.extra_locals
                    .iter()
                    .map(|ty| (1u32, *ty))
                    .collect::<Vec<_>>(),
            );
            let mut insn = func.instructions();

            // Initialize owned table variables to -1
            for owned in &ctx.owned_tables {
                let var_idx = ctx.locals[owned];
                insn.i64_const(-1);
                insn.local_set(var_idx);
            }

            for stmt in &program.statements {
                if !matches!(stmt, Statement::FuncDecl(_)) {
                    self.compile_stmt(stmt, &mut insn, &mut ctx)?;
                }
            }

            // Free all owned tables before _start exits
            self.emit_table_cleanup(&mut insn, &ctx, None);

            insn.end();
            code_section.function(&func);
        }

        // Compile closure functions
        self.compile_closure_functions(&mut function_section, &mut code_section, program)?;

        // Table section: funcref table for closures
        let mut table_section = TableSection::new();
        let num_closures = self.closures.len() as u64;
        if num_closures > 0 {
            table_section.table(TableType {
                element_type: RefType::FUNCREF,
                minimum: num_closures,
                maximum: Some(num_closures),
                table64: false,
                shared: false,
            });
        }

        // Element section: populate the table with closure function indices
        let mut element_section = ElementSection::new();
        if !self.closures.is_empty() {
            let func_indices: Vec<u32> = self.closures.iter().map(|c| c.func_idx).collect();
            let offset = ConstExpr::i32_const(0);
            element_section.active(
                None, // table 0 (MVP encoding for funcref)
                &offset,
                Elements::Functions(Cow::Borrowed(&[])),
            );
            // We need to rebuild because Cow::Borrowed won't work with a local vec.
            // Clear and redo with owned data:
            element_section = ElementSection::new();
            element_section.active(
                None,
                &offset,
                Elements::Functions(Cow::Owned(func_indices)),
            );
        }

        // Memory section: 1 page (64 KiB)
        let mut memory_section = MemorySection::new();
        memory_section.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });

        // Global section: __heap_base (mutable i32) initialised to end of string data
        let heap_base_value = self.string_data.len() as i32;
        let mut global_section = GlobalSection::new();
        global_section.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(heap_base_value),
        );

        // Export section
        let mut export_section = ExportSection::new();
        for stmt in &program.statements {
            if let Statement::FuncDecl(func) = stmt {
                let (func_idx, _) = self.func_map[&func.name];
                export_section.export(&func.name, ExportKind::Func, func_idx);
            }
        }
        if has_top_level {
            let (start_idx, _) = self.func_map["_start"];
            export_section.export("_start", ExportKind::Func, start_idx);
        }
        // Export memory so the host can read string data
        export_section.export("memory", ExportKind::Memory, 0);
        // Export __heap_base global (index 0)
        export_section.export("__heap_base", ExportKind::Global, 0);

        // Data section: string pool
        let mut data_section = DataSection::new();
        if !self.string_data.is_empty() {
            let offset_expr = ConstExpr::i32_const(0);
            data_section.active(0, &offset_expr, self.string_data.iter().copied());
        }

        // Assemble module
        // Section order: type, import, function, table, memory, global, export, element, code, data
        let mut module = Module::new();
        module.section(&type_section);
        module.section(&import_section);
        module.section(&function_section);
        if num_closures > 0 {
            module.section(&table_section);
        }
        module.section(&memory_section);
        module.section(&global_section);
        module.section(&export_section);
        if !self.closures.is_empty() {
            module.section(&element_section);
        }
        module.section(&code_section);
        if !self.string_data.is_empty() {
            module.section(&data_section);
        }

        Ok(module.finish())
    }

    // -----------------------------------------------------------------------
    // Pre-scan pass
    // -----------------------------------------------------------------------

    /// Pre-scan a statement to discover all variable declarations and power ops (for locals allocation)
    fn prescan_stmt(&self, stmt: &Statement, ctx: &mut FuncCtx) {
        match stmt {
            Statement::VarDecl(VarDecl::TypeDecl { name, type_name, .. }) => {
                ctx.declare_local(name);
                ctx.var_types.insert(name.clone(), type_name_to_ah(type_name));
            }
            Statement::VarDecl(VarDecl::Assignment(assign)) => {
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

    fn prescan_block(&self, block: &Block, ctx: &mut FuncCtx) {
        for stmt in &block.statements {
            self.prescan_stmt(stmt, ctx);
        }
    }

    fn prescan_expr(&self, expr: &Expr, ctx: &mut FuncCtx) {
        match expr {
            Expr::BinaryOp {
                left, op, right, ..
            } => {
                self.prescan_expr(left, ctx);
                self.prescan_expr(right, ctx);
                if matches!(op, BinaryOp::Power) {
                    ctx.alloc_power_temps();
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
            _ => {}
        }
    }

    fn prescan_boolean_expr(&self, expr: &BooleanExpr, ctx: &mut FuncCtx) {
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

    // -----------------------------------------------------------------------
    // Function compilation
    // -----------------------------------------------------------------------

    /// Compile a function declaration into a wasm Function
    fn compile_func_decl(&self, func: &FuncDecl) -> Result<Function, CodegenError> {
        let mut ctx = FuncCtx::new();

        // Register parameters as locals, record their types, and mark as borrowed
        for param in &func.params {
            ctx.add_param(&param.name);
            ctx.var_types
                .insert(param.name.clone(), type_name_to_ah(&param.type_name));
            ctx.param_names.insert(param.name.clone());
        }

        // Pre-scan body for variable declarations
        self.prescan_block(&func.body, &mut ctx);

        let mut wasm_func = Function::new(
            ctx.extra_locals
                .iter()
                .map(|ty| (1u32, *ty))
                .collect::<Vec<_>>(),
        );
        let mut insn = wasm_func.instructions();

        // Initialize owned table variables to -1 (sentinel: table_free(-1) is a no-op)
        for owned in &ctx.owned_tables {
            let var_idx = ctx.locals[owned];
            insn.i64_const(-1);
            insn.local_set(var_idx);
        }

        // Compile function body
        self.compile_block(&func.body, &mut insn, &mut ctx)?;

        // Free all owned tables before implicit return
        self.emit_table_cleanup(&mut insn, &ctx, None);

        // If the function has return types, push default 0 values to ensure
        // the stack is valid even if no explicit return was executed.
        for _ in &func.return_types {
            insn.i64_const(0);
        }

        insn.end();
        Ok(wasm_func)
    }

    // -----------------------------------------------------------------------
    // Closure function compilation
    // -----------------------------------------------------------------------

    /// Compile all closure functions and add them to function/code sections.
    fn compile_closure_functions(
        &self,
        function_section: &mut FunctionSection,
        code_section: &mut CodeSection,
        program: &Program,
    ) -> Result<(), CodegenError> {
        // We need to find the actual ClosureExpr AST nodes in the same order they
        // were collected (depth-first). We collect references to them:
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
                // Recurse into the body first (nested closures are collected first)
                match &closure.body {
                    ClosureBody::Expr(e) => Self::collect_closure_expr_refs_expr(e, out),
                    ClosureBody::Block(b) => Self::collect_closure_expr_refs_block(b, out),
                }
                // Then add this closure
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

        // Parameter 0: env_ptr (i32). We add it to locals for indexing but it is i32.
        let env_ptr_idx = ctx.add_param_with_type("__env_ptr", ValType::I32);

        // Parameters 1..N: closure params (i64 each, borrowed)
        for param in &closure_expr.params {
            ctx.add_param(&param.name);
            ctx.var_types.insert(param.name.clone(), AhType::Int);
            ctx.param_names.insert(param.name.clone());
        }

        // Declare locals for captured variables (borrowed from outer scope, not freed here)
        for capture in &info.captures {
            ctx.declare_local(capture);
            ctx.var_types.insert(capture.clone(), AhType::Int);
            ctx.param_names.insert(capture.clone());
        }

        // Pre-scan the closure body for additional locals (power temps, etc.)
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

        // Load captured variables from memory at env_ptr + offset
        for (cap_idx, capture) in info.captures.iter().enumerate() {
            let local_idx = ctx.get_local(capture).unwrap();
            // local.get $env_ptr (i32 address)
            insn.local_get(env_ptr_idx);
            // i64.load offset=<cap_idx * 8>
            insn.i64_load(MemArg {
                offset: (cap_idx * 8) as u64,
                align: 3, // 2^3 = 8 byte alignment
                memory_index: 0,
            });
            insn.local_set(local_idx);
        }

        // Initialize owned table variables to -1
        for owned in &ctx.owned_tables {
            let var_idx = ctx.locals[owned];
            insn.i64_const(-1);
            insn.local_set(var_idx);
        }

        // Compile the closure body
        match &closure_expr.body {
            ClosureBody::Expr(e) => {
                self.compile_expr(e, &mut insn, &mut ctx)?;
                insn.return_();
            }
            ClosureBody::Block(b) => {
                self.compile_block(b, &mut insn, &mut ctx)?;
                // Free owned tables before implicit return
                self.emit_table_cleanup(&mut insn, &ctx, None);
            }
        }

        // Default return value
        insn.i64_const(0);
        insn.end();

        Ok(wasm_func)
    }

    // -----------------------------------------------------------------------
    // Block / statement compilation
    // -----------------------------------------------------------------------

    fn compile_block(
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

    fn compile_stmt(
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
        // For each target/value pair, compile the value and set the local
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

        // Structure:
        //   block $break        ;; break jumps here
        //     loop $continue    ;; continue jumps here
        //       <condition check: br_if $break if false>
        //       <body>
        //       <step>
        //       br $continue
        //     end
        //   end

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
        // Special dispatch: print(str_expr) -> env.print_str
        if call.name == "print" && call.args.len() == 1 {
            let arg_type = self.infer_expr_type(&call.args[0], ctx);
            if arg_type == AhType::Str {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_str"];
                insn.call(func_idx);
                return Ok(());
            }
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

        // Check if the return value is an owned table variable → transfer ownership
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
    fn emit_table_cleanup(
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

    // -----------------------------------------------------------------------
    // Closure call_indirect emission
    // -----------------------------------------------------------------------

    /// Emit the call_indirect sequence for calling a closure stored in a local variable.
    /// The explicit arguments have already been compiled onto the stack BEFORE this is called,
    /// so we need to reorganize: we need env_ptr as the first arg on stack, then the explicit args.
    ///
    /// Strategy: we already pushed the explicit args. We need to pop them into temp locals,
    /// push env_ptr, push them back, then call_indirect.
    ///
    /// Actually, the args were already compiled before we get here (compile_call_func_stmt
    /// compiles args first). So the stack has [arg0, arg1, ...]. We need to rearrange to
    /// [env_ptr, arg0, arg1, ...] then call_indirect. We do NOT have the stack in the right
    /// order. So we need to save args to temp locals, then reorder.
    ///
    /// Wait -- looking at the call flow again: in compile_call_func_stmt, args are compiled
    /// before the function lookup. So the stack currently has the explicit args.
    /// We need: [env_ptr_i32, arg0_i64, arg1_i64, ..., table_idx_i32] for call_indirect.
    /// Actually call_indirect pops the table index from the top of the stack, then the args
    /// in order.
    ///
    /// The full stack layout that call_indirect expects (bottom to top):
    ///   env_ptr(i32), param0(i64), ..., paramN(i64), table_idx(i32)
    ///
    /// Currently on stack: param0(i64), ..., paramN(i64)
    /// We need to insert env_ptr below and table_idx on top.
    ///
    /// Simplest: save all args to temp locals, then push in order.
    fn emit_closure_call_indirect(
        &self,
        closure_id: u32,
        closure_local_idx: u32,
        call: &CallFunc,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let info = &self.closures[closure_id as usize];
        let num_args = call.args.len();

        // Use pre-allocated temp locals for saving arguments
        let arg_temps = ctx.claim_closure_call_temps();

        // Pop arguments from the stack into temp locals (reverse order since stack is LIFO)
        for i in (0..num_args).rev() {
            insn.local_set(arg_temps[i]);
        }

        // Now emit: env_ptr(i32), arg0(i64), ..., argN-1(i64), table_idx(i32)

        // Extract env_ptr: closure_val & 0xFFFFFFFF as i32
        insn.local_get(closure_local_idx); // i64 closure value
        insn.i32_wrap_i64(); // low 32 bits = env_ptr

        // Push args back
        for i in 0..num_args {
            insn.local_get(arg_temps[i]);
        }

        // Extract table_idx: (closure_val >> 32) as i32
        insn.local_get(closure_local_idx); // i64 closure value
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.i32_wrap_i64(); // table_idx as i32

        // call_indirect with the appropriate type
        // The type must match (i32, i64*N) -> i64
        let call_type_idx = info.type_idx;
        insn.call_indirect(0, call_type_idx); // table 0

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Expression compilation
    // -----------------------------------------------------------------------

    fn compile_expr(
        &self,
        expr: &Expr,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        match expr {
            Expr::Number(s, span) => {
                // Parse as i64 (try integer first, then float truncated to i64)
                if let Ok(val) = s.parse::<i64>() {
                    insn.i64_const(val);
                } else if let Ok(val) = s.parse::<f64>() {
                    insn.i64_const(val as i64);
                } else {
                    return Err(codegen_err(
                        format!("invalid number literal: {}", s),
                        span,
                    ));
                }
            }
            Expr::Bool(b, _) => {
                insn.i64_const(if *b { 1 } else { 0 });
            }
            Expr::StringLit(s, _) => {
                // Look up the interned string and emit a packed i64 = (offset << 32) | len
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
                        // ~ operator: call env.random(left, right) -> i64
                        self.compile_expr(left, insn, ctx)?;
                        self.compile_expr(right, insn, ctx)?;
                        let (func_idx, _) = self.func_map["__env_random"];
                        insn.call(func_idx);
                    }
                    BinaryOp::Power => {
                        // Implement integer power via a loop approach using
                        // pre-allocated temp locals from prescan.
                        let (base_local, exp_local, result_local) = ctx.claim_power_temps();

                        // base = left
                        self.compile_expr(left, insn, ctx)?;
                        insn.local_set(base_local);
                        // exp = right
                        self.compile_expr(right, insn, ctx)?;
                        insn.local_set(exp_local);
                        // result = 1
                        insn.i64_const(1);
                        insn.local_set(result_local);

                        // block $done
                        //   loop $loop
                        //     exp <= 0 -> br_if $done
                        //     result = result * base
                        //     exp = exp - 1
                        //     br $loop
                        //   end
                        // end
                        insn.block(BlockType::Empty);
                        ctx.block_depth += 1;
                        let done_depth = ctx.block_depth;

                        insn.loop_(BlockType::Empty);
                        ctx.block_depth += 1;
                        let loop_depth = ctx.block_depth;

                        // Check if exp <= 0
                        insn.local_get(exp_local);
                        insn.i64_const(0);
                        insn.i64_le_s();
                        insn.br_if(ctx.block_depth - done_depth);

                        // result *= base
                        insn.local_get(result_local);
                        insn.local_get(base_local);
                        insn.i64_mul();
                        insn.local_set(result_local);

                        // exp -= 1
                        insn.local_get(exp_local);
                        insn.i64_const(1);
                        insn.i64_sub();
                        insn.local_set(exp_local);

                        // br $loop
                        insn.br(ctx.block_depth - loop_depth);

                        insn.end(); // end loop
                        ctx.block_depth -= 1;
                        insn.end(); // end block
                        ctx.block_depth -= 1;

                        // Push result
                        insn.local_get(result_local);
                    }
                    BinaryOp::Add => {
                        // Check if either operand is a string -- use str_concat
                        let lt = self.infer_expr_type(left, ctx);
                        let rt = self.infer_expr_type(right, ctx);
                        if lt == AhType::Str || rt == AhType::Str {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            let (func_idx, _) = self.func_map["__env_str_concat"];
                            insn.call(func_idx);
                        } else {
                            self.compile_expr(left, insn, ctx)?;
                            self.compile_expr(right, insn, ctx)?;
                            insn.i64_add();
                        }
                    }
                    _ => {
                        self.compile_expr(left, insn, ctx)?;
                        self.compile_expr(right, insn, ctx)?;
                        match op {
                            BinaryOp::Sub => {
                                insn.i64_sub();
                            }
                            BinaryOp::Mul => {
                                insn.i64_mul();
                            }
                            BinaryOp::Div => {
                                insn.i64_div_s();
                            }
                            BinaryOp::Mod => {
                                insn.i64_rem_s();
                            }
                            BinaryOp::Add | BinaryOp::Power | BinaryOp::Rand => unreachable!(),
                        }
                    }
                }
            }
            Expr::UnaryOp { op, operand, span } => {
                // i++ or i--: load, modify, store, and leave the *original* value on stack
                if let Some(idx) = ctx.get_local(operand) {
                    insn.local_get(idx);
                    // Duplicate: get again, modify, store
                    insn.local_get(idx);
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
                    insn.local_set(idx);
                    // The original value is still on the stack (post-increment/decrement semantics)
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
        }
        Ok(())
    }

    /// Compile a closure expression: allocate captures in memory and produce a packed i64.
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
            // No captures: env_ptr = 0
            // packed = (table_idx << 32) | 0
            let packed = (info.table_idx as i64) << 32;
            insn.i64_const(packed);
        } else {
            // Allocate space in linear memory for captures:
            // 1. Load current __heap_base (global 0, i32)
            // 2. For each capture: store its value at heap_base + offset
            // 3. Advance __heap_base by 8 * num_captures
            // 4. Construct packed i64: (table_idx << 32) | env_ptr

            // Use pre-allocated temp local for env_ptr (i32 stored as i64 for manipulation)
            let env_ptr_temp = ctx.closure_env_temps[ctx.closure_env_temps_cursor];
            ctx.closure_env_temps_cursor += 1;

            // Load heap_base and save as our env_ptr
            insn.global_get(0); // i32 heap_base
            insn.i64_extend_i32_u(); // promote to i64 for later packing
            insn.local_set(env_ptr_temp);

            // Store each capture's value at heap_base + offset
            for (cap_idx, capture) in info.captures.iter().enumerate() {
                // Address for i64.store: needs i32 on stack
                insn.global_get(0); // i32 heap_base

                // Value to store: the capture variable's current value
                if let Some(local_idx) = ctx.get_local(capture) {
                    insn.local_get(local_idx);
                } else {
                    // Capture not found in scope -- store 0
                    insn.i64_const(0);
                }

                insn.i64_store(MemArg {
                    offset: (cap_idx * 8) as u64,
                    align: 3, // 2^3 = 8
                    memory_index: 0,
                });
            }

            // Advance __heap_base by 8 * num_captures
            insn.global_get(0); // current heap_base (i32)
            insn.i32_const((num_captures * 8) as i32);
            insn.i32_add();
            insn.global_set(0);

            // Construct packed i64: (table_idx << 32) | env_ptr
            // table_idx is known at compile time
            insn.i64_const((info.table_idx as i64) << 32);
            insn.local_get(env_ptr_temp); // env_ptr as i64
            insn.i64_or();
        }

        Ok(())
    }

    /// Compile a table literal: { key: val, ... }
    fn compile_table_literal(
        &self,
        table: &TableLiteral,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let (table_new_idx, _) = self.func_map["__env_table_new"];
        let (table_set_idx, _) = self.func_map["__env_table_set"];
        let (table_set_table_idx, _) = self.func_map["__env_table_set_table"];

        // Claim pre-allocated temp local for the table_id
        let tmp = ctx.table_temps[ctx.table_temps_cursor];
        ctx.table_temps_cursor += 1;

        // Call table_new() and store the handle
        insn.call(table_new_idx);
        insn.local_set(tmp);

        // For each entry: table_set(handle, key, value) or table_set_table for nested tables
        for entry in &table.entries {
            insn.local_get(tmp); // table_id

            // Key: intern the field name and emit packed i64
            let (offset, len) = self
                .string_pool
                .get(entry.key.as_str())
                .copied()
                .unwrap_or((0, 0));
            let packed_key: i64 = ((offset as i64) << 32) | (len as i64);
            insn.i64_const(packed_key);

            // Value
            self.compile_expr(&entry.value, insn, ctx)?;

            // Use table_set_table for nested table values (registers parent→child)
            let val_ty = self.infer_expr_type(&entry.value, ctx);
            if matches!(val_ty, AhType::Table(_)) {
                insn.call(table_set_table_idx);
            } else {
                insn.call(table_set_idx);
            }
        }

        // Leave the table handle on the stack
        insn.local_get(tmp);
        Ok(())
    }

    /// Compile a field access: expr.field
    fn compile_field_access(
        &self,
        fa: &FieldAccess,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let (table_get_idx, _) = self.func_map["__env_table_get"];

        // Compile the object expression (pushes table_id)
        self.compile_expr(&fa.object, insn, ctx)?;

        // Push the field name as packed string
        let (offset, len) = self
            .string_pool
            .get(fa.field.as_str())
            .copied()
            .unwrap_or((0, 0));
        let packed_key: i64 = ((offset as i64) << 32) | (len as i64);
        insn.i64_const(packed_key);

        // Call table_get
        insn.call(table_get_idx);
        Ok(())
    }

    /// Compile an index access: expr["key"]
    fn compile_index_access(
        &self,
        ia: &IndexAccess,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let (table_get_idx, _) = self.func_map["__env_table_get"];

        // Compile the object expression (pushes table_id)
        self.compile_expr(&ia.object, insn, ctx)?;

        // Compile the index expression (pushes key as packed string)
        self.compile_expr(&ia.index, insn, ctx)?;

        // Call table_get
        insn.call(table_get_idx);
        Ok(())
    }

    /// Compile field assignment: object.field = value
    fn compile_field_assign(
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

    /// Compile index assignment: object["key"] = value
    fn compile_index_assign(
        &self,
        ia: &IndexAssign,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
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

    /// Compile a method call expression: expr(args) where expr evaluates to a closure.
    /// The callee (e.g. table.field) produces a packed closure i64 on the stack.
    /// We save it, compile args, reorder for call_indirect: env_ptr, args..., table_idx.
    fn compile_method_call_expr(
        &self,
        mc: &MethodCall,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        let num_args = mc.args.len();

        // Use pre-allocated temps: [arg0, arg1, ..., argN-1, closure_val]
        let temps = ctx.claim_closure_call_temps();

        // 1. Compile callee (produces closure packed i64 on stack)
        self.compile_expr(&mc.callee, insn, ctx)?;

        // 2. Save closure value to the last temp
        let closure_temp = temps[num_args]; // last slot is for closure value
        insn.local_set(closure_temp);

        // 3. Compile arguments
        for arg in &mc.args {
            self.compile_expr(arg, insn, ctx)?;
        }

        // 4. Save arguments to temp locals (reverse order since stack is LIFO)
        for i in (0..num_args).rev() {
            insn.local_set(temps[i]);
        }

        // 5. Push env_ptr (low 32 bits of closure value) as i32
        insn.local_get(closure_temp);
        insn.i32_wrap_i64();

        // 6. Restore arguments in order
        for i in 0..num_args {
            insn.local_get(temps[i]);
        }

        // 7. Push table_idx (high 32 bits of closure value) as i32
        insn.local_get(closure_temp);
        insn.i64_const(32);
        insn.i64_shr_u();
        insn.i32_wrap_i64();

        // 8. Determine the closure type: (i32, i64*N) -> i64
        //    Try to infer from callee type, otherwise compute from args count
        let callee_ty = self.infer_expr_type(&mc.callee, ctx);
        let call_type_idx = if let AhType::Closure(id) = callee_ty {
            self.closures[id as usize].type_idx
        } else {
            // Build the type from args count: (i32, i64 * num_args) -> i64
            let mut params = vec![ValType::I32];
            for _ in 0..num_args {
                params.push(ValType::I64);
            }
            // Look for existing type or report error
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

        // 9. call_indirect with table 0
        insn.call_indirect(0, call_type_idx);

        Ok(())
    }

    fn compile_call_func_expr(
        &self,
        call: &CallFunc,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        // Special dispatch: print(str_expr) -> env.print_str (when used as expr)
        if call.name == "print" && call.args.len() == 1 {
            let arg_type = self.infer_expr_type(&call.args[0], ctx);
            if arg_type == AhType::Str {
                self.compile_expr(&call.args[0], insn, ctx)?;
                let (func_idx, _) = self.func_map["__env_print_str"];
                insn.call(func_idx);
                // print_str returns void; push 0 so the caller has a value
                insn.i64_const(0);
                return Ok(());
            }
        }

        for arg in &call.args {
            self.compile_expr(arg, insn, ctx)?;
        }
        if let Some(&(func_idx, type_idx)) = self.func_map.get(&call.name) {
            // Check argument count
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
            // Check if it is a closure variable
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

    // -----------------------------------------------------------------------
    // Boolean expression compilation
    // -----------------------------------------------------------------------

    fn compile_boolean_expr(
        &self,
        expr: &BooleanExpr,
        insn: &mut wasm_encoder::InstructionSink<'_>,
        ctx: &mut FuncCtx,
    ) -> Result<(), CodegenError> {
        match expr {
            BooleanExpr::Comparison {
                left, op, right, ..
            } => {
                self.compile_expr(left, insn, ctx)?;
                self.compile_expr(right, insn, ctx)?;
                match op {
                    ComparisonOp::Gt => {
                        insn.i64_gt_s();
                    }
                    ComparisonOp::Lt => {
                        insn.i64_lt_s();
                    }
                    ComparisonOp::GtEq => {
                        insn.i64_ge_s();
                    }
                    ComparisonOp::LtEq => {
                        insn.i64_le_s();
                    }
                    ComparisonOp::Eq => {
                        insn.i64_eq();
                    }
                    ComparisonOp::NotEq => {
                        insn.i64_ne();
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

#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    #[error("Codegen error at line {line}, column {column}: {message}")]
    Error {
        message: String,
        line: usize,
        column: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use anehta_lexer::Lexer;
    use anehta_parser::Parser;

    fn compile_source(src: &str) -> Vec<u8> {
        let tokens = Lexer::new(src).tokenize().expect("lexer failed");
        let mut parser = Parser::new(tokens);
        let program = parser.parse().expect("parser failed");
        let mut codegen = WasmCodegen::new();
        codegen.compile(&program).expect("codegen failed")
    }

    fn validate_wasm(bytes: &[u8]) {
        let mut validator = wasmparser::Validator::new();
        validator.validate_all(bytes).expect("invalid wasm");
    }

    #[test]
    fn simple_arithmetic() {
        let wasm = compile_source("var x = 1 + 2 * 3");
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn function_declaration() {
        let src = "func add(a: int, b: int) -> int {\nreturn a + b\n}";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn if_statement() {
        let src = r#"var x = 10
if (x > 5) {
    x = 1
}"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn if_else_statement() {
        let src = r#"var x = 10
if (x > 5) {
    x = 1
} else {
    x = 2
}"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn if_elseif_else_statement() {
        let src = r#"var x = 10
if (x > 15) {
    x = 1
} elseif (x > 5) {
    x = 2
} else {
    x = 3
}"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn for_loop() {
        let src = r#"var sum = 0
for (var i = 0; i < 10; i = i + 1) {
    sum = sum + i
}"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn for_loop_infinite_with_break() {
        let src = r#"for (;;) {
    break
}"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn function_with_call() {
        let src = r#"func double(x: int) -> int {
    return x * 2
}
var result = double(21)"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn complex_boolean_expression() {
        let src = r#"var x = 10
if ((x > 5 && x < 20) && (x > 3)) {
    x = 100
}"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn power_operator() {
        let src = "var x = 2 ^ 10";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn modulo_operator() {
        let src = "var x = 10 % 3";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn random_operator() {
        let src = "var x = 1 ~ 100";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn multiple_return_values() {
        let src = r#"func swap(a: int, b: int) -> int, int {
    return b, a
}"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn nested_for_and_if() {
        let src = r#"var total = 0
for (var i = 0; i < 10; i = i + 1) {
    if (i > 5) {
        break
    }
    for (var j = 0; j < 5; j = j + 1) {
        total = total + 1
        continue
    }
}"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn empty_function_body() {
        let src = "func noop() -> int {\n}";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn var_type_declaration() {
        let src = "var x: int";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn wasm_starts_with_magic() {
        let wasm = compile_source("var x = 42");
        // WASM magic number: \0asm
        assert_eq!(&wasm[0..4], &[0x00, 0x61, 0x73, 0x6D]);
        // WASM version 1
        assert_eq!(&wasm[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn full_program() {
        let src = r#"var health = 100

func damage(hp: int, amount: int) -> int {
    return hp - amount
}

for (var i = 0; i < 10; i = i + 1) {
    if (i > 5) {
        health = damage(health, 10)
    }
}
"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    // -------------------------------------------------------------------
    // String support tests
    // -------------------------------------------------------------------

    #[test]
    fn string_literal() {
        let src = r#"var s = "hello""#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn string_print() {
        let src = r#"print("hello world")"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn string_concat() {
        let src = r#"var a = "hello"
var b = " world"
var c = a + b"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn string_function() {
        let src = r#"func greet(name: str) -> str {
    return "hello " + name
}
var msg = greet("world")
print(msg)"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    // -------------------------------------------------------------------
    // Closure support tests
    // -------------------------------------------------------------------

    #[test]
    fn closure_basic() {
        let src = "var f = |x| => x * 2";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn closure_no_params() {
        let src = "var f = || => 42";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn closure_multi_params() {
        let src = "var f = |x, y| => x + y";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn closure_with_capture() {
        let src = "var a = 10\nvar f = |x| => x + a";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn closure_block_body() {
        let src = "var f = |x| => {\nvar y = x * 2\nreturn y\n}";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn closure_call() {
        let src = "var f = |x| => x * 2\nvar r = f(5)";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    // ── Table tests ───────────────────────────────────────

    #[test]
    fn table_empty() {
        let src = "var t = {}";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_literal() {
        let src = "var p = { hp: 100, mp: 50 }";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_field_access() {
        let src = "var p = { hp: 100 }\nvar x = p.hp";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_field_assign() {
        let src = "var p = { hp: 100 }\np.hp = 50";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_index_access() {
        let src = r#"var p = { hp: 100 }
var x = p["hp"]"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_index_assign() {
        let src = r#"var p = { hp: 100 }
p["hp"] = 50"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_string_field() {
        let src = r#"var p = { name: "Alice" }"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_print_field() {
        let src = "var p = { hp: 100 }\nprint(p.hp)";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_nested() {
        let src = "var a = { inner: { x: 42 } }\nvar v = a.inner";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    // --- Table GC tests ---

    #[test]
    fn table_gc_reassignment() {
        let src = "var t = { hp: 100 }\nt = { hp: 200 }\nprint(t.hp)";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_gc_loop_reassignment() {
        let src = r#"var t = { n: 0 }
for (var i = 0; i < 10; i = i + 1) {
    t = { n: i }
}
print(t.n)"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_gc_func_auto_free() {
        let src = r#"func make() -> int {
    var t = { x: 10 }
    return t.x
}
print(make())"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_gc_return_ownership_transfer() {
        let src = r#"func makeT() -> int {
    var t = { hp: 500 }
    return t
}
var h = makeT()
print(h)"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_gc_nested_recursive_free() {
        let src = "var t = { child: { grandchild: { val: 42 } } }\nprint(t.child.grandchild.val)";
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_gc_field_assign_table_rejected() {
        let src = "var a = {}\nvar b = {}\na.child = b";
        let tokens = Lexer::new(src).tokenize().expect("lexer failed");
        let mut parser = Parser::new(tokens);
        let program = parser.parse().expect("parser failed");
        let mut codegen = WasmCodegen::new();
        let result = codegen.compile(&program);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("cannot assign a table to a field"));
    }

    #[test]
    fn table_gc_index_assign_table_rejected() {
        let src = "var a = {}\nvar b = {}\na[\"child\"] = b";
        let tokens = Lexer::new(src).tokenize().expect("lexer failed");
        let mut parser = Parser::new(tokens);
        let program = parser.parse().expect("parser failed");
        let mut codegen = WasmCodegen::new();
        let result = codegen.compile(&program);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("cannot assign a table to a field"));
    }

    #[test]
    fn table_gc_multiple_tables_in_func() {
        let src = r#"func multi() -> int {
    var a = { val: 1 }
    var b = { val: 2 }
    var c = { val: 3 }
    return a.val + b.val + c.val
}
print(multi())"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_gc_if_branch_table() {
        let src = r#"var t = { val: 0 }
if (1 > 0) {
    t = { val: 100 }
}
print(t.val)"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_gc_closure_captures_table() {
        let src = r#"var t = { hp: 100 }
var f = |x| => t.hp + x
print(f(10))"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_gc_closure_creates_table() {
        let src = r#"var f = |n| => {
    var t = { val: n }
    return t.val
}
print(f(42))"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }

    #[test]
    fn table_gc_func_with_captured_and_uncaptured() {
        let src = r#"func test() -> int {
    var captured = { val: 1 }
    var uncaptured = { val: 2 }
    var f = |x| => captured.val + x
    return f(10)
}
print(test())"#;
        let wasm = compile_source(src);
        assert!(!wasm.is_empty());
        validate_wasm(&wasm);
    }
}
