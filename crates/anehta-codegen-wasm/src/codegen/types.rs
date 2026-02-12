use std::collections::HashMap;
use std::collections::HashSet;

use wasm_encoder::ValType;

/// Simple type tag used to distinguish integer vs string vs closure values at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AhType {
    Int,
    Float,
    Str,
    Vec,
    Mat,
    /// Closure value. The u32 is the closure ID (index into closures vec).
    Closure(u32),
    /// Table value. The u32 is the table type ID (index into table_types vec).
    Table(u32),
}

/// Map a source-level type name (e.g. "int", "str") to our internal tag.
pub(crate) fn type_name_to_ah(name: &str) -> AhType {
    match name {
        "str" | "string" => AhType::Str,
        "float" | "f64" => AhType::Float,
        "vec" => AhType::Vec,
        "mat" | "matrix" => AhType::Mat,
        _ => AhType::Int,
    }
}

/// Information about a single closure collected during the analysis pass.
#[allow(dead_code)]
pub(crate) struct ClosureInfo {
    /// Internal function name, e.g. `__closure_0`
    pub(crate) name: String,
    /// Function index in the WASM module
    pub(crate) func_idx: u32,
    /// Type index for the closure function: (i32, i64*N) -> i64
    pub(crate) type_idx: u32,
    /// Names of captured variables (in order)
    pub(crate) captures: Vec<String>,
    /// Number of explicit parameters (not counting env_ptr)
    pub(crate) param_count: usize,
    /// Index into the WASM function table (0, 1, 2, ...)
    pub(crate) table_idx: u32,
    /// Inferred return type of the closure body
    pub(crate) return_type: AhType,
}

/// Compile-time type info for a table literal's fields.
pub(crate) struct TableTypeInfo {
    pub(crate) fields: HashMap<String, AhType>,
}

/// Context for compiling a single function body
pub(crate) struct FuncCtx {
    /// Map from variable name to local index
    pub(crate) locals: HashMap<String, u32>,
    /// Next local index to assign
    pub(crate) next_local: u32,
    /// Additional locals declared in the body (beyond parameters)
    pub(crate) extra_locals: Vec<ValType>,
    /// Current nesting depth of break/continue targets.
    pub(crate) loop_depth_stack: Vec<LoopInfo>,
    /// Current block nesting depth (incremented for every block/loop/if)
    pub(crate) block_depth: u32,
    /// Pre-allocated temp local indices for power operations (groups of 3)
    pub(crate) power_temps: Vec<(u32, u32, u32)>,
    /// Index into power_temps for the next power operation to consume
    pub(crate) power_temps_cursor: usize,
    /// Pre-allocated temp local indices for timer blocks (start, end)
    pub(crate) timer_temps: Vec<(u32, u32)>,
    /// Index into timer_temps for the next timer block to consume
    pub(crate) timer_temps_cursor: usize,
    /// Inferred types for local variables
    pub(crate) var_types: HashMap<String, AhType>,
    /// Pre-allocated temp local indices for closure call_indirect argument reordering.
    /// Each entry is a Vec of local indices for saving arguments.
    pub(crate) closure_call_temps: Vec<Vec<u32>>,
    /// Index into closure_call_temps for the next closure call to consume
    pub(crate) closure_call_temps_cursor: usize,
    /// Pre-allocated temp locals for closure env_ptr during closure creation (one per closure with captures)
    pub(crate) closure_env_temps: Vec<u32>,
    /// Index into closure_env_temps for the next closure creation to consume
    pub(crate) closure_env_temps_cursor: usize,
    /// Pre-allocated temp locals for table literal construction (one per table literal)
    pub(crate) table_temps: Vec<u32>,
    /// Index into table_temps for the next table literal to consume
    pub(crate) table_temps_cursor: usize,
    /// Table variables owned by this function (created via table literals)
    pub(crate) owned_tables: Vec<String>,
    /// Parameter names (borrowed references, not freed by this function)
    pub(crate) param_names: HashSet<String>,
    /// Table variables captured by closures in this function (must NOT be freed)
    pub(crate) captured_tables: HashSet<String>,
    /// Pre-allocated temp locals for saving return values during table cleanup
    pub(crate) return_save_temps: Vec<Vec<u32>>,
    /// Index into return_save_temps for the next return statement to consume
    pub(crate) return_save_temps_cursor: usize,
    /// Pre-allocated temp locals for vec literal construction (one per vec literal)
    pub(crate) vec_literal_temps: Vec<u32>,
    /// Index into vec_literal_temps for the next vec literal to consume
    pub(crate) vec_literal_temps_cursor: usize,
    /// Pre-allocated temp locals for mat literal construction (one per mat literal)
    pub(crate) mat_literal_temps: Vec<u32>,
    /// Index into mat_literal_temps for the next mat literal to consume
    pub(crate) mat_literal_temps_cursor: usize,
    /// Pre-allocated temp locals for vector destructuring (one per destructuring assignment)
    pub(crate) destructure_temps: Vec<u32>,
    /// Index into destructure_temps for the next destructuring to consume
    pub(crate) destructure_temps_cursor: usize,
    /// Pre-allocated fixed helper locals for inline SIMD operations (always 12 locals)
    /// Used by emit_vec_add_simd and similar inline SIMD codegen helpers
    pub(crate) simd_helpers: [u32; 12],
}

#[derive(Clone, Copy)]
pub(crate) struct LoopInfo {
    /// Label depth for `break` (the outer block)
    pub(crate) break_depth: u32,
    /// Label depth for `continue` (the loop itself)
    pub(crate) continue_depth: u32,
}

impl FuncCtx {
    pub(crate) fn new() -> Self {
        Self {
            locals: HashMap::new(),
            next_local: 12, // 从 12 开始，因为 0-11 被 SIMD helpers 占用
            extra_locals: vec![ValType::I64; 12], // 预先添加 12 个 i64 locals
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
            vec_literal_temps: Vec::new(),
            vec_literal_temps_cursor: 0,
            mat_literal_temps: Vec::new(),
            mat_literal_temps_cursor: 0,
            destructure_temps: Vec::new(),
            destructure_temps_cursor: 0,
            simd_helpers: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], // 预分配 SIMD helper locals
        }
    }

    pub(crate) fn new_with_var_types(var_types: HashMap<String, AhType>) -> Self {
        let mut ctx = Self::new();
        ctx.var_types = var_types;
        ctx
    }

    pub(crate) fn add_param(&mut self, name: &str) -> u32 {
        let idx = self.next_local;
        self.locals.insert(name.to_string(), idx);
        self.next_local += 1;
        idx
    }

    /// Add a parameter with a specific ValType (used for i32 env_ptr in closures)
    pub(crate) fn add_param_with_type(&mut self, name: &str, _vt: ValType) -> u32 {
        let idx = self.next_local;
        self.locals.insert(name.to_string(), idx);
        self.next_local += 1;
        idx
    }

    pub(crate) fn declare_local(&mut self, name: &str) -> u32 {
        if let Some(&idx) = self.locals.get(name) {
            return idx;
        }
        let idx = self.next_local;
        self.locals.insert(name.to_string(), idx);
        self.next_local += 1;
        self.extra_locals.push(ValType::I64);
        idx
    }

    pub(crate) fn alloc_anonymous_local(&mut self) -> u32 {
        let idx = self.next_local;
        self.next_local += 1;
        self.extra_locals.push(ValType::I64);
        idx
    }

    pub(crate) fn get_local(&self, name: &str) -> Option<u32> {
        self.locals.get(name).copied()
    }

    /// Pre-allocate a group of 3 temp locals for a power operation
    pub(crate) fn alloc_power_temps(&mut self) {
        let base = self.alloc_anonymous_local();
        let exp = self.alloc_anonymous_local();
        let result = self.alloc_anonymous_local();
        self.power_temps.push((base, exp, result));
    }

    /// Claim the next pre-allocated power temp group
    pub(crate) fn claim_power_temps(&mut self) -> (u32, u32, u32) {
        let temps = self.power_temps[self.power_temps_cursor];
        self.power_temps_cursor += 1;
        temps
    }

    /// Pre-allocate a pair of temp locals for a timer block (start, end)
    pub(crate) fn alloc_timer_temps(&mut self) {
        let start = self.alloc_anonymous_local();
        let end = self.alloc_anonymous_local();
        self.timer_temps.push((start, end));
    }

    /// Claim the next pre-allocated timer temp pair
    pub(crate) fn claim_timer_temps(&mut self) -> (u32, u32) {
        let temps = self.timer_temps[self.timer_temps_cursor];
        self.timer_temps_cursor += 1;
        temps
    }

    /// Pre-allocate temp locals for a closure call_indirect (one per argument)
    pub(crate) fn alloc_closure_call_temps(&mut self, num_args: usize) {
        let mut temps = Vec::with_capacity(num_args);
        for _ in 0..num_args {
            temps.push(self.alloc_anonymous_local());
        }
        self.closure_call_temps.push(temps);
    }

    /// Claim the next pre-allocated closure call temp group
    pub(crate) fn claim_closure_call_temps(&mut self) -> Vec<u32> {
        let temps = self.closure_call_temps[self.closure_call_temps_cursor].clone();
        self.closure_call_temps_cursor += 1;
        temps
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
