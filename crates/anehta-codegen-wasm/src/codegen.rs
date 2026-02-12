use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;

use anehta_lexer::Span;
use anehta_parser::{
    Assignment, BinaryOp, Block, BooleanExpr, CallFunc, ClosureBody, ClosureExpr, ComparisonOp,
    Expr, FieldAccess, FieldAssign, ForStmt, FuncDecl, IfStmt, IndexAccess, IndexAssign,
    LogicalOp, MethodCall, Program, ReturnStmt, Statement, TableLiteral, TimerStmt, Transpose,
    UnaryOp, VarDecl,
};

use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, DataSection, ElementSection, Elements, EntityType,
    ExportKind, ExportSection, Function, FunctionSection, GlobalSection, GlobalType, ImportSection,
    MemArg, MemorySection, MemoryType, Module, RefType, TableSection, TableType, TypeSection,
    ValType,
};

mod types;
mod collect_strings;
mod collect_tables;
mod collect_closures;
mod infer;
mod prescan;
mod compile_stmt;
mod compile_expr;
mod compile_func;
mod compile_bool;

#[cfg(test)]
mod tests;

use types::*;
pub use types::CodegenError;

fn codegen_err(message: impl Into<String>, span: &Span) -> CodegenError {
    CodegenError::Error {
        message: message.into(),
        line: span.line,
        column: span.column,
    }
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

        // Import: env.print_float(i64) (print f64 value stored as i64 bits)
        let print_float_type_idx = self.add_type(vec![ValType::I64], vec![]);
        let print_float_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_print_float".to_string(), (print_float_func_idx, print_float_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 13;

        // Import: env.float_pow(i64, i64) -> i64 (f64 power via host)
        let float_pow_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let float_pow_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_float_pow".to_string(), (float_pow_func_idx, float_pow_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 14;

        // Import: env.float_to_str(i64) -> i64 (convert f64 bits to packed string)
        let float_to_str_type_idx = self.add_type(vec![ValType::I64], vec![ValType::I64]);
        let float_to_str_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_float_to_str".to_string(), (float_to_str_func_idx, float_to_str_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 15;

        // Import: env.float_mod(i64, i64) -> i64 (f64 remainder via host)
        let float_mod_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let float_mod_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_float_mod".to_string(), (float_mod_func_idx, float_mod_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 16;

        // Import: env.int_to_str(i64) -> i64 (convert i64 to packed string for str+int concat)
        let int_to_str_type_idx = self.add_type(vec![ValType::I64], vec![ValType::I64]);
        let int_to_str_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_int_to_str".to_string(), (int_to_str_func_idx, int_to_str_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 17;

        // Import: env.print_vec(i64) (print vec value)
        let print_vec_type_idx = self.add_type(vec![ValType::I64], vec![]);
        let print_vec_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_print_vec".to_string(), (print_vec_func_idx, print_vec_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 18;

        // Import: env.vec_get(i64, i64) -> i64 (get vec element by index)
        let vec_get_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_get_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_get".to_string(), (vec_get_func_idx, vec_get_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 19;

        // Import: env.vec_set(i64, i64, i64) (set vec element by index)
        let vec_set_type_idx = self.add_type(vec![ValType::I64, ValType::I64, ValType::I64], vec![]);
        let vec_set_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_set".to_string(), (vec_set_func_idx, vec_set_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 20;

        // Import: env.vec_add(i64, i64) -> i64 (element-wise add)
        let vec_add_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_add_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_add".to_string(), (vec_add_func_idx, vec_add_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 21;

        // Import: env.vec_sub(i64, i64) -> i64 (element-wise sub)
        let vec_sub_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_sub_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_sub".to_string(), (vec_sub_func_idx, vec_sub_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 22;

        // Import: env.vec_mul(i64, i64) -> i64 (element-wise mul)
        let vec_mul_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_mul_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_mul".to_string(), (vec_mul_func_idx, vec_mul_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 23;

        // Import: env.vec_scale(i64, i64) -> i64 (scalar multiply)
        let vec_scale_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_scale_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_scale".to_string(), (vec_scale_func_idx, vec_scale_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 24;

        // Import: env.vec_dot(i64, i64) -> i64 (dot product → f64 bits)
        let vec_dot_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_dot_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_dot".to_string(), (vec_dot_func_idx, vec_dot_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 25;

        // Import: env.vec_cross(i64, i64) -> i64 (cross product → new vec)
        let vec_cross_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_cross_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_cross".to_string(), (vec_cross_func_idx, vec_cross_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 26;

        // Import: env.vec_swizzle(i64, i64) -> i64 (multi-element swizzle)
        let vec_swizzle_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_swizzle_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_swizzle".to_string(), (vec_swizzle_func_idx, vec_swizzle_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 27;

        // Import: env.print_mat(i64) (print matrix value)
        let print_mat_type_idx = self.add_type(vec![ValType::I64], vec![]);
        let print_mat_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_print_mat".to_string(), (print_mat_func_idx, print_mat_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 28;

        // Import: env.mat_add(i64, i64) -> i64 (matrix addition)
        let mat_add_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_add_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_add".to_string(), (mat_add_func_idx, mat_add_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 29;

        // Import: env.mat_sub(i64, i64) -> i64 (matrix subtraction)
        let mat_sub_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_sub_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_sub".to_string(), (mat_sub_func_idx, mat_sub_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 30;

        // Import: env.mat_mul(i64, i64) -> i64 (matrix multiplication)
        let mat_mul_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_mul_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_mul".to_string(), (mat_mul_func_idx, mat_mul_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 31;

        // Import: env.mat_vec_mul(i64, i64) -> i64 (matrix * vector)
        let mat_vec_mul_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_vec_mul_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_vec_mul".to_string(), (mat_vec_mul_func_idx, mat_vec_mul_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 32;

        // Import: env.mat_scale(i64, i64) -> i64 (matrix * scalar)
        let mat_scale_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_scale_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_scale".to_string(), (mat_scale_func_idx, mat_scale_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 33;

        // Import: env.mat_transpose(i64) -> i64 (transpose)
        let mat_transpose_type_idx = self.add_type(vec![ValType::I64], vec![ValType::I64]);
        let mat_transpose_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_transpose".to_string(), (mat_transpose_func_idx, mat_transpose_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 34;

        // Import: env.mat_det(i64) -> i64 (determinant, returns f64 bits)
        let mat_det_type_idx = self.add_type(vec![ValType::I64], vec![ValType::I64]);
        let mat_det_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_det".to_string(), (mat_det_func_idx, mat_det_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 35;

        // Import: env.mat_inv(i64) -> i64 (inverse)
        let mat_inv_type_idx = self.add_type(vec![ValType::I64], vec![ValType::I64]);
        let mat_inv_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_inv".to_string(), (mat_inv_func_idx, mat_inv_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 36;

        // Import: env.mat_get(i64, i64) -> i64 (get element by linear index)
        let mat_get_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_get_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_get".to_string(), (mat_get_func_idx, mat_get_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 37;

        // Import: env.mat_set(i64, i64, i64) (set element by linear index)
        let mat_set_type_idx = self.add_type(vec![ValType::I64, ValType::I64, ValType::I64], vec![]);
        let mat_set_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_set".to_string(), (mat_set_func_idx, mat_set_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 38;

        // Import: env.mat_solve(i64, i64) -> i64 (solve Ax=b using LU decomposition)
        let mat_solve_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_solve_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_solve".to_string(), (mat_solve_func_idx, mat_solve_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 39;

        // Import: env.vec_pow(i64, i64) -> i64 (element-wise power for vec)
        let vec_pow_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_pow_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_pow".to_string(), (vec_pow_func_idx, vec_pow_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 40;

        // Import: env.mat_pow(i64, i64) -> i64 (element-wise power for mat)
        let mat_pow_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_pow_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_pow".to_string(), (mat_pow_func_idx, mat_pow_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 41;

        // Import: env.vec_slice(vec: i64, start: i64, end: i64) -> i64 (slice vector)
        let vec_slice_type_idx = self.add_type(vec![ValType::I64, ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_slice_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_slice".to_string(), (vec_slice_func_idx, vec_slice_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 42;

        // Import: env.mat_slice(mat: i64, start: i64, end: i64) -> i64 (row slice matrix)
        let mat_slice_type_idx = self.add_type(vec![ValType::I64, ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_slice_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_slice".to_string(), (mat_slice_func_idx, mat_slice_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 43;

        // Import: env.vec_fancy_index(vec: i64, indices: i64) -> i64 (fancy indexing)
        let vec_fancy_index_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_fancy_index_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_fancy_index".to_string(), (vec_fancy_index_func_idx, vec_fancy_index_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 44;

        // Import: env.mat_fancy_index(mat: i64, indices: i64) -> i64 (fancy row indexing)
        let mat_fancy_index_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_fancy_index_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_fancy_index".to_string(), (mat_fancy_index_func_idx, mat_fancy_index_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 45;

        // Import: env.vec_add_scalar(vec: i64, scalar: i64) -> i64 (broadcast scalar addition)
        let vec_add_scalar_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_add_scalar_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_add_scalar".to_string(), (vec_add_scalar_func_idx, vec_add_scalar_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 46;

        // Import: env.vec_sub_scalar(vec: i64, scalar: i64) -> i64 (broadcast scalar subtraction)
        let vec_sub_scalar_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_sub_scalar_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_sub_scalar".to_string(), (vec_sub_scalar_func_idx, vec_sub_scalar_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 47;

        // Import: env.vec_div_scalar(vec: i64, scalar: i64) -> i64 (broadcast scalar division)
        let vec_div_scalar_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_div_scalar_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_div_scalar".to_string(), (vec_div_scalar_func_idx, vec_div_scalar_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 48;

        // Import: env.mat_add_scalar(mat: i64, scalar: i64) -> i64 (broadcast scalar addition)
        let mat_add_scalar_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_add_scalar_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_add_scalar".to_string(), (mat_add_scalar_func_idx, mat_add_scalar_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 49;

        // Import: env.mat_sub_scalar(mat: i64, scalar: i64) -> i64 (broadcast scalar subtraction)
        let mat_sub_scalar_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_sub_scalar_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_sub_scalar".to_string(), (mat_sub_scalar_func_idx, mat_sub_scalar_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 50;

        // Import: env.mat_div_scalar(mat: i64, scalar: i64) -> i64 (broadcast scalar division)
        let mat_div_scalar_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_div_scalar_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_div_scalar".to_string(), (mat_div_scalar_func_idx, mat_div_scalar_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 51;

        // Import: env.mat_add_vec_broadcast(mat: i64, vec: i64) -> i64 (broadcast vec to each row)
        let mat_add_vec_broadcast_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_add_vec_broadcast_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_add_vec_broadcast".to_string(), (mat_add_vec_broadcast_func_idx, mat_add_vec_broadcast_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 52;

        // Import: env.mat_sub_vec_broadcast(mat: i64, vec: i64) -> i64 (broadcast vec subtraction)
        let mat_sub_vec_broadcast_type_idx = self.add_type(vec![ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_sub_vec_broadcast_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_sub_vec_broadcast".to_string(), (mat_sub_vec_broadcast_func_idx, mat_sub_vec_broadcast_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 53;

        // Import: env.vec_mask(vec: i64, threshold: i64, op: i64) -> i64 (boolean masking)
        let vec_mask_type_idx = self.add_type(vec![ValType::I64, ValType::I64, ValType::I64], vec![ValType::I64]);
        let vec_mask_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_vec_mask".to_string(), (vec_mask_func_idx, vec_mask_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 54;

        // Import: env.mat_mask(mat: i64, threshold: i64, op: i64) -> i64 (boolean masking, flatten to vec)
        let mat_mask_type_idx = self.add_type(vec![ValType::I64, ValType::I64, ValType::I64], vec![ValType::I64]);
        let mat_mask_func_idx = self.next_func_idx;
        self.func_map
            .insert("__env_mat_mask".to_string(), (mat_mask_func_idx, mat_mask_type_idx));
        self.next_func_idx += 1;
        self.num_imports = 55;

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
        import_section.import("env", "print_float", EntityType::Function(print_float_type_idx));
        import_section.import("env", "float_pow", EntityType::Function(float_pow_type_idx));
        import_section.import("env", "float_to_str", EntityType::Function(float_to_str_type_idx));
        import_section.import("env", "float_mod", EntityType::Function(float_mod_type_idx));
        import_section.import("env", "int_to_str", EntityType::Function(int_to_str_type_idx));
        import_section.import("env", "print_vec", EntityType::Function(print_vec_type_idx));
        import_section.import("env", "vec_get", EntityType::Function(vec_get_type_idx));
        import_section.import("env", "vec_set", EntityType::Function(vec_set_type_idx));
        import_section.import("env", "vec_add", EntityType::Function(vec_add_type_idx));
        import_section.import("env", "vec_sub", EntityType::Function(vec_sub_type_idx));
        import_section.import("env", "vec_mul", EntityType::Function(vec_mul_type_idx));
        import_section.import("env", "vec_scale", EntityType::Function(vec_scale_type_idx));
        import_section.import("env", "vec_dot", EntityType::Function(vec_dot_type_idx));
        import_section.import("env", "vec_cross", EntityType::Function(vec_cross_type_idx));
        import_section.import("env", "vec_swizzle", EntityType::Function(vec_swizzle_type_idx));
        import_section.import("env", "print_mat", EntityType::Function(print_mat_type_idx));
        import_section.import("env", "mat_add", EntityType::Function(mat_add_type_idx));
        import_section.import("env", "mat_sub", EntityType::Function(mat_sub_type_idx));
        import_section.import("env", "mat_mul", EntityType::Function(mat_mul_type_idx));
        import_section.import("env", "mat_vec_mul", EntityType::Function(mat_vec_mul_type_idx));
        import_section.import("env", "mat_scale", EntityType::Function(mat_scale_type_idx));
        import_section.import("env", "mat_transpose", EntityType::Function(mat_transpose_type_idx));
        import_section.import("env", "mat_det", EntityType::Function(mat_det_type_idx));
        import_section.import("env", "mat_inv", EntityType::Function(mat_inv_type_idx));
        import_section.import("env", "mat_get", EntityType::Function(mat_get_type_idx));
        import_section.import("env", "mat_set", EntityType::Function(mat_set_type_idx));
        import_section.import("env", "mat_solve", EntityType::Function(mat_solve_type_idx));
        import_section.import("env", "vec_pow", EntityType::Function(vec_pow_type_idx));
        import_section.import("env", "mat_pow", EntityType::Function(mat_pow_type_idx));
        import_section.import("env", "vec_slice", EntityType::Function(vec_slice_type_idx));
        import_section.import("env", "mat_slice", EntityType::Function(mat_slice_type_idx));
        import_section.import("env", "vec_fancy_index", EntityType::Function(vec_fancy_index_type_idx));
        import_section.import("env", "mat_fancy_index", EntityType::Function(mat_fancy_index_type_idx));
        import_section.import("env", "vec_add_scalar", EntityType::Function(vec_add_scalar_type_idx));
        import_section.import("env", "vec_sub_scalar", EntityType::Function(vec_sub_scalar_type_idx));
        import_section.import("env", "vec_div_scalar", EntityType::Function(vec_div_scalar_type_idx));
        import_section.import("env", "mat_add_scalar", EntityType::Function(mat_add_scalar_type_idx));
        import_section.import("env", "mat_sub_scalar", EntityType::Function(mat_sub_scalar_type_idx));
        import_section.import("env", "mat_div_scalar", EntityType::Function(mat_div_scalar_type_idx));
        import_section.import("env", "mat_add_vec_broadcast", EntityType::Function(mat_add_vec_broadcast_type_idx));
        import_section.import("env", "mat_sub_vec_broadcast", EntityType::Function(mat_sub_vec_broadcast_type_idx));
        import_section.import("env", "vec_mask", EntityType::Function(vec_mask_type_idx));
        import_section.import("env", "mat_mask", EntityType::Function(mat_mask_type_idx));

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
}
