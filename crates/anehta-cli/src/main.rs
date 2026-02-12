use std::env;
use std::fs;

// ============================================================================
// SIMD Optimized Vector Operations (AVX2 on x86_64)
// ============================================================================

#[cfg(target_arch = "x86_64")]
mod simd_ops {
    use std::arch::x86_64::*;

    /// SIMD-optimized vector addition: result[i] = a[i] + b[i]
    /// Uses AVX2 to process 4 f64 values at once
    #[target_feature(enable = "avx2")]
    pub unsafe fn vec_add_simd(a: &[f64], b: &[f64], result: &mut [f64]) {
        let len = a.len().min(b.len()).min(result.len());
        let mut i = 0;

        // Process 4 elements at a time using AVX2 (256-bit = 4 x f64)
        while i + 4 <= len {
            let va = _mm256_loadu_pd(a.as_ptr().add(i));
            let vb = _mm256_loadu_pd(b.as_ptr().add(i));
            let vr = _mm256_add_pd(va, vb);
            _mm256_storeu_pd(result.as_mut_ptr().add(i), vr);
            i += 4;
        }

        // Handle remaining elements (scalar fallback)
        for j in i..len {
            result[j] = a[j] + b[j];
        }
    }

    /// SIMD-optimized vector subtraction
    #[target_feature(enable = "avx2")]
    pub unsafe fn vec_sub_simd(a: &[f64], b: &[f64], result: &mut [f64]) {
        let len = a.len().min(b.len()).min(result.len());
        let mut i = 0;

        while i + 4 <= len {
            let va = _mm256_loadu_pd(a.as_ptr().add(i));
            let vb = _mm256_loadu_pd(b.as_ptr().add(i));
            let vr = _mm256_sub_pd(va, vb);
            _mm256_storeu_pd(result.as_mut_ptr().add(i), vr);
            i += 4;
        }

        for j in i..len {
            result[j] = a[j] - b[j];
        }
    }

    /// SIMD-optimized element-wise multiplication
    #[target_feature(enable = "avx2")]
    pub unsafe fn vec_mul_simd(a: &[f64], b: &[f64], result: &mut [f64]) {
        let len = a.len().min(b.len()).min(result.len());
        let mut i = 0;

        while i + 4 <= len {
            let va = _mm256_loadu_pd(a.as_ptr().add(i));
            let vb = _mm256_loadu_pd(b.as_ptr().add(i));
            let vr = _mm256_mul_pd(va, vb);
            _mm256_storeu_pd(result.as_mut_ptr().add(i), vr);
            i += 4;
        }

        for j in i..len {
            result[j] = a[j] * b[j];
        }
    }

    /// SIMD-optimized scalar addition: result[i] = a[i] + scalar
    #[target_feature(enable = "avx2")]
    pub unsafe fn vec_add_scalar_simd(a: &[f64], scalar: f64, result: &mut [f64]) {
        let len = a.len().min(result.len());
        let mut i = 0;
        let vs = _mm256_set1_pd(scalar); // Broadcast scalar to all 4 lanes

        while i + 4 <= len {
            let va = _mm256_loadu_pd(a.as_ptr().add(i));
            let vr = _mm256_add_pd(va, vs);
            _mm256_storeu_pd(result.as_mut_ptr().add(i), vr);
            i += 4;
        }

        for j in i..len {
            result[j] = a[j] + scalar;
        }
    }

    /// SIMD-optimized scalar subtraction
    #[target_feature(enable = "avx2")]
    pub unsafe fn vec_sub_scalar_simd(a: &[f64], scalar: f64, result: &mut [f64]) {
        let len = a.len().min(result.len());
        let mut i = 0;
        let vs = _mm256_set1_pd(scalar);

        while i + 4 <= len {
            let va = _mm256_loadu_pd(a.as_ptr().add(i));
            let vr = _mm256_sub_pd(va, vs);
            _mm256_storeu_pd(result.as_mut_ptr().add(i), vr);
            i += 4;
        }

        for j in i..len {
            result[j] = a[j] - scalar;
        }
    }

    /// SIMD-optimized scalar multiplication (scaling)
    #[target_feature(enable = "avx2")]
    pub unsafe fn vec_scale_simd(a: &[f64], scalar: f64, result: &mut [f64]) {
        let len = a.len().min(result.len());
        let mut i = 0;
        let vs = _mm256_set1_pd(scalar);

        while i + 4 <= len {
            let va = _mm256_loadu_pd(a.as_ptr().add(i));
            let vr = _mm256_mul_pd(va, vs);
            _mm256_storeu_pd(result.as_mut_ptr().add(i), vr);
            i += 4;
        }

        for j in i..len {
            result[j] = a[j] * scalar;
        }
    }

    /// SIMD-optimized scalar division
    #[target_feature(enable = "avx2")]
    pub unsafe fn vec_div_scalar_simd(a: &[f64], scalar: f64, result: &mut [f64]) {
        let len = a.len().min(result.len());
        let mut i = 0;
        let vs = _mm256_set1_pd(scalar);

        while i + 4 <= len {
            let va = _mm256_loadu_pd(a.as_ptr().add(i));
            let vr = _mm256_div_pd(va, vs);
            _mm256_storeu_pd(result.as_mut_ptr().add(i), vr);
            i += 4;
        }

        for j in i..len {
            result[j] = a[j] / scalar;
        }
    }

    /// SIMD-optimized dot product
    #[target_feature(enable = "avx2")]
    pub unsafe fn vec_dot_simd(a: &[f64], b: &[f64]) -> f64 {
        let len = a.len().min(b.len());
        let mut i = 0;
        let mut sum_vec = _mm256_setzero_pd();

        while i + 4 <= len {
            let va = _mm256_loadu_pd(a.as_ptr().add(i));
            let vb = _mm256_loadu_pd(b.as_ptr().add(i));
            let vprod = _mm256_mul_pd(va, vb);
            sum_vec = _mm256_add_pd(sum_vec, vprod);
            i += 4;
        }

        // Horizontal sum of the 4 lanes
        let mut sum_array = [0.0; 4];
        _mm256_storeu_pd(sum_array.as_mut_ptr(), sum_vec);
        let mut sum = sum_array.iter().sum::<f64>();

        // Handle remaining elements
        for j in i..len {
            sum += a[j] * b[j];
        }

        sum
    }
}

// Runtime CPU feature detection wrapper
#[cfg(target_arch = "x86_64")]
fn vec_add_optimized(a: &[f64], b: &[f64], result: &mut [f64]) {
    if is_x86_feature_detected!("avx2") {
        unsafe { simd_ops::vec_add_simd(a, b, result) }
    } else {
        // Fallback to scalar
        for i in 0..a.len().min(b.len()).min(result.len()) {
            result[i] = a[i] + b[i];
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn vec_sub_optimized(a: &[f64], b: &[f64], result: &mut [f64]) {
    if is_x86_feature_detected!("avx2") {
        unsafe { simd_ops::vec_sub_simd(a, b, result) }
    } else {
        for i in 0..a.len().min(b.len()).min(result.len()) {
            result[i] = a[i] - b[i];
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn vec_mul_optimized(a: &[f64], b: &[f64], result: &mut [f64]) {
    if is_x86_feature_detected!("avx2") {
        unsafe { simd_ops::vec_mul_simd(a, b, result) }
    } else {
        for i in 0..a.len().min(b.len()).min(result.len()) {
            result[i] = a[i] * b[i];
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn vec_add_scalar_optimized(a: &[f64], scalar: f64, result: &mut [f64]) {
    if is_x86_feature_detected!("avx2") {
        unsafe { simd_ops::vec_add_scalar_simd(a, scalar, result) }
    } else {
        for i in 0..a.len().min(result.len()) {
            result[i] = a[i] + scalar;
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn vec_sub_scalar_optimized(a: &[f64], scalar: f64, result: &mut [f64]) {
    if is_x86_feature_detected!("avx2") {
        unsafe { simd_ops::vec_sub_scalar_simd(a, scalar, result) }
    } else {
        for i in 0..a.len().min(result.len()) {
            result[i] = a[i] - scalar;
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn vec_scale_optimized(a: &[f64], scalar: f64, result: &mut [f64]) {
    if is_x86_feature_detected!("avx2") {
        unsafe { simd_ops::vec_scale_simd(a, scalar, result) }
    } else {
        for i in 0..a.len().min(result.len()) {
            result[i] = a[i] * scalar;
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn vec_div_scalar_optimized(a: &[f64], scalar: f64, result: &mut [f64]) {
    if is_x86_feature_detected!("avx2") {
        unsafe { simd_ops::vec_div_scalar_simd(a, scalar, result) }
    } else {
        for i in 0..a.len().min(result.len()) {
            result[i] = a[i] / scalar;
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn vec_dot_optimized(a: &[f64], b: &[f64]) -> f64 {
    if is_x86_feature_detected!("avx2") {
        unsafe { simd_ops::vec_dot_simd(a, b) }
    } else {
        let len = a.len().min(b.len());
        (0..len).map(|i| a[i] * b[i]).sum()
    }
}

// Fallback for non-x86_64 platforms (use scalar operations)
#[cfg(not(target_arch = "x86_64"))]
fn vec_add_optimized(a: &[f64], b: &[f64], result: &mut [f64]) {
    for i in 0..a.len().min(b.len()).min(result.len()) {
        result[i] = a[i] + b[i];
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn vec_sub_optimized(a: &[f64], b: &[f64], result: &mut [f64]) {
    for i in 0..a.len().min(b.len()).min(result.len()) {
        result[i] = a[i] - b[i];
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn vec_mul_optimized(a: &[f64], b: &[f64], result: &mut [f64]) {
    for i in 0..a.len().min(b.len()).min(result.len()) {
        result[i] = a[i] * b[i];
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn vec_add_scalar_optimized(a: &[f64], scalar: f64, result: &mut [f64]) {
    for i in 0..a.len().min(result.len()) {
        result[i] = a[i] + scalar;
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn vec_sub_scalar_optimized(a: &[f64], scalar: f64, result: &mut [f64]) {
    for i in 0..a.len().min(result.len()) {
        result[i] = a[i] - scalar;
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn vec_scale_optimized(a: &[f64], scalar: f64, result: &mut [f64]) {
    for i in 0..a.len().min(result.len()) {
        result[i] = a[i] * scalar;
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn vec_div_scalar_optimized(a: &[f64], scalar: f64, result: &mut [f64]) {
    for i in 0..a.len().min(result.len()) {
        result[i] = a[i] / scalar;
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn vec_dot_optimized(a: &[f64], b: &[f64]) -> f64 {
    let len = a.len().min(b.len());
    (0..len).map(|i| a[i] * b[i]).sum()
}

// ============================================================================
// End SIMD Module
// ============================================================================

fn compile(source_path: &str) -> Result<(String, Vec<u8>), String> {
    let source = fs::read_to_string(source_path)
        .map_err(|e| format!("Error reading file '{}': {}", source_path, e))?;

    // Step 1: Lex
    let mut lexer = anehta_lexer::Lexer::new(&source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e))?;

    // Step 2: Parse
    let mut parser = anehta_parser::Parser::new(tokens);
    let program = parser.parse().map_err(|e| format!("{}", e))?;

    // Step 3: Codegen
    let mut codegen = anehta_codegen_wasm::WasmCodegen::new();
    let wasm_bytes = codegen.compile(&program).map_err(|e| format!("{}", e))?;

    let output_path = source_path.replace(".ah", ".wasm");
    Ok((output_path, wasm_bytes))
}

fn cmd_build(source_path: &str) {
    let (output_path, wasm_bytes) = match compile(source_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    match fs::write(&output_path, &wasm_bytes) {
        Ok(_) => println!(
            "Compiled {} -> {} ({} bytes)",
            source_path,
            output_path,
            wasm_bytes.len()
        ),
        Err(e) => {
            eprintln!("Error writing '{}': {}", output_path, e);
            std::process::exit(1);
        }
    }
}

fn cmd_run(source_path: &str) {
    let (_output_path, wasm_bytes) = match compile(source_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = execute_wasm(&wasm_bytes) {
        eprintln!("Runtime error: {}", e);
        std::process::exit(1);
    }
}

struct RuntimeState {
    start_instant: std::time::Instant,
    /// Host-side tables: each slot is Some(table) or None (freed).
    /// WASM references tables by their index (table_id) in this Vec.
    tables: Vec<Option<std::collections::HashMap<String, i64>>>,
    /// Indices of freed table slots available for reuse.
    free_slots: Vec<usize>,
    /// Parent → children relationships for recursive table freeing.
    table_children: std::collections::HashMap<usize, Vec<usize>>,
}

// Helper macro to get and update heap_ptr from WASM global
macro_rules! get_heap_ptr {
    ($caller:expr) => {{
        let global = ($caller)
            .get_export("__heap_base")
            .and_then(|e| e.into_global())
            .expect("missing __heap_base global");
        let val = global.get(&mut *$caller);
        match val {
            wasmtime::Val::I32(v) => v as u32,
            _ => panic!("__heap_base is not i32"),
        }
    }};
}

macro_rules! set_heap_ptr {
    ($caller:expr, $new_ptr:expr) => {{
        let global = ($caller)
            .get_export("__heap_base")
            .and_then(|e| e.into_global())
            .expect("missing __heap_base global");
        global
            .set($caller, wasmtime::Val::I32($new_ptr as i32))
            .expect("failed to set __heap_base");
    }};
}

fn execute_wasm(wasm_bytes: &[u8]) -> Result<(), String> {
    use rand::Rng;
    use wasmtime::*;

    // 创建 Config 并启用 SIMD 支持
    let mut config = Config::new();
    config.wasm_simd(true); // 启用 WASM SIMD 指令
    let engine = Engine::new(&config).map_err(|e| format!("Failed to create engine: {}", e))?;
    let module = Module::new(&engine, wasm_bytes)
        .map_err(|e| format!("Failed to load WASM module: {:#?}", e))?;

    let state = RuntimeState {
        start_instant: std::time::Instant::now(),
        tables: Vec::new(),
        free_slots: Vec::new(),
        table_children: std::collections::HashMap::new(),
    };
    let mut store = Store::new(&engine, state);
    let mut linker = Linker::new(&engine);

    // Host function: env.print(i64)
    linker
        .func_wrap("env", "print", |_caller: Caller<'_, RuntimeState>, val: i64| {
            println!("{}", val);
        })
        .map_err(|e| format!("Failed to register env.print: {}", e))?;

    // Host function: env.random(i64, i64) -> i64
    linker
        .func_wrap(
            "env",
            "random",
            |_caller: Caller<'_, RuntimeState>, min: i64, max: i64| -> i64 {
                if min >= max {
                    return min;
                }
                let mut rng = rand::rng();
                rng.random_range(min..=max)
            },
        )
        .map_err(|e| format!("Failed to register env.random: {}", e))?;

    // Host function: env.input() -> i64
    linker
        .func_wrap("env", "input", |_caller: Caller<'_, RuntimeState>| -> i64 {
            use std::io::{self, BufRead, Write};
            io::stdout().flush().ok();
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line).ok();
            line.trim().parse::<i64>().unwrap_or(0)
        })
        .map_err(|e| format!("Failed to register env.input: {}", e))?;

    // Host function: env.clock() -> i64 (milliseconds since program start)
    linker
        .func_wrap("env", "clock", |caller: Caller<'_, RuntimeState>| -> i64 {
            caller.data().start_instant.elapsed().as_millis() as i64
        })
        .map_err(|e| format!("Failed to register env.clock: {}", e))?;

    // Host function: env.print_timer(i64) (prints elapsed time formatted)
    linker
        .func_wrap(
            "env",
            "print_timer",
            |_caller: Caller<'_, RuntimeState>, ms: i64| {
                if ms < 1 {
                    println!("[timer] <1ms");
                } else {
                    println!("[timer] {}ms", ms);
                }
            },
        )
        .map_err(|e| format!("Failed to register env.print_timer: {}", e))?;

    // Host function: env.print_str(i64)
    // Argument is a packed i64: ptr = (val >> 32), len = (val & 0xFFFF_FFFF)
    linker
        .func_wrap(
            "env",
            "print_str",
            |mut caller: Caller<'_, RuntimeState>, val: i64| {
                let ptr = (val >> 32) as u32;
                let len = (val & 0xFFFF_FFFF) as u32;
                if len == 0 {
                    println!();
                    return;
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let start = ptr as usize;
                let end = start + len as usize;
                if end <= data.len() {
                    let s = std::str::from_utf8(&data[start..end]).unwrap_or("<invalid utf8>");
                    println!("{}", s);
                } else {
                    println!("<out of bounds string>");
                }
            },
        )
        .map_err(|e| format!("Failed to register env.print_str: {}", e))?;

    // Host function: env.str_concat(i64, i64) -> i64
    // Both arguments are packed strings. Reads from memory, writes concatenation
    // at heap_ptr, advances heap_ptr, returns new packed i64.
    linker
        .func_wrap(
            "env",
            "str_concat",
            |mut caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let a_ptr = (a >> 32) as u32;
                let a_len = (a & 0xFFFF_FFFF) as u32;
                let b_ptr = (b >> 32) as u32;
                let b_len = (b & 0xFFFF_FFFF) as u32;

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");

                // Read both source strings into temporary buffers
                let data = memory.data(&caller);
                let a_bytes = data[a_ptr as usize..(a_ptr + a_len) as usize].to_vec();
                let b_bytes = data[b_ptr as usize..(b_ptr + b_len) as usize].to_vec();

                let new_len = a_len + b_len;
                let dest = get_heap_ptr!(&mut caller);

                // Write concatenated result into linear memory
                let data_mut = memory.data_mut(&mut caller);
                data_mut[dest as usize..dest as usize + a_len as usize]
                    .copy_from_slice(&a_bytes);
                data_mut[dest as usize + a_len as usize..dest as usize + new_len as usize]
                    .copy_from_slice(&b_bytes);

                // Advance the bump allocator
                set_heap_ptr!(&mut caller, dest + new_len);

                // Return packed i64
                ((dest as i64) << 32) | (new_len as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.str_concat: {}", e))?;

    // Host function: env.table_new() -> i64
    // Creates a new empty table and returns its handle (index into tables vec).
    // Reuses freed slots when available.
    linker
        .func_wrap(
            "env",
            "table_new",
            |mut caller: Caller<'_, RuntimeState>| -> i64 {
                let state = caller.data_mut();
                if let Some(slot) = state.free_slots.pop() {
                    state.tables[slot] = Some(std::collections::HashMap::new());
                    slot as i64
                } else {
                    let id = state.tables.len();
                    state.tables.push(Some(std::collections::HashMap::new()));
                    id as i64
                }
            },
        )
        .map_err(|e| format!("Failed to register env.table_new: {}", e))?;

    // Host function: env.table_set(table_id: i64, key: i64, value: i64)
    // key is a packed string (ptr << 32 | len). Reads the key from WASM memory.
    linker
        .func_wrap(
            "env",
            "table_set",
            |mut caller: Caller<'_, RuntimeState>, table_id: i64, key: i64, value: i64| {
                let key_ptr = (key >> 32) as u32;
                let key_len = (key & 0xFFFF_FFFF) as u32;

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let start = key_ptr as usize;
                let end = start + key_len as usize;
                let key_str = if end <= data.len() {
                    std::str::from_utf8(&data[start..end])
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::new()
                };

                let id = table_id as usize;
                if id < caller.data().tables.len() {
                    if let Some(ref mut table) = caller.data_mut().tables[id] {
                        table.insert(key_str, value);
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to register env.table_set: {}", e))?;

    // Host function: env.table_get(table_id: i64, key: i64) -> i64
    // key is a packed string. Reads the key from WASM memory, returns stored value or 0.
    linker
        .func_wrap(
            "env",
            "table_get",
            |mut caller: Caller<'_, RuntimeState>, table_id: i64, key: i64| -> i64 {
                let key_ptr = (key >> 32) as u32;
                let key_len = (key & 0xFFFF_FFFF) as u32;

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let start = key_ptr as usize;
                let end = start + key_len as usize;
                let key_str = if end <= data.len() {
                    std::str::from_utf8(&data[start..end])
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::new()
                };

                let id = table_id as usize;
                if id < caller.data().tables.len() {
                    if let Some(ref table) = caller.data().tables[id] {
                        table.get(&key_str).copied().unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    0
                }
            },
        )
        .map_err(|e| format!("Failed to register env.table_get: {}", e))?;

    // Host function: env.table_free(table_id: i64)
    // Recursively frees a table and all its child tables.
    // -1 sentinel is a no-op. Out-of-bounds or already-freed slots are ignored.
    linker
        .func_wrap(
            "env",
            "table_free",
            |mut caller: Caller<'_, RuntimeState>, table_id: i64| {
                if table_id < 0 {
                    return;
                }
                let id = table_id as usize;
                let state = caller.data_mut();
                if id >= state.tables.len() {
                    return;
                }
                if state.tables[id].is_none() {
                    return;
                }
                let mut stack = vec![id];
                while let Some(current) = stack.pop() {
                    if current >= state.tables.len() {
                        continue;
                    }
                    if state.tables[current].is_none() {
                        continue;
                    }
                    if let Some(children) = state.table_children.remove(&current) {
                        stack.extend(children);
                    }
                    state.tables[current] = None;
                    state.free_slots.push(current);
                }
            },
        )
        .map_err(|e| format!("Failed to register env.table_free: {}", e))?;

    // Host function: env.print_float(i64) — print f64 value stored as i64 bits
    linker
        .func_wrap(
            "env",
            "print_float",
            |_caller: Caller<'_, RuntimeState>, val: i64| {
                let f = f64::from_bits(val as u64);
                if f.fract() == 0.0 && f.is_finite() {
                    println!("{:.1}", f);
                } else {
                    println!("{}", f);
                }
            },
        )
        .map_err(|e| format!("Failed to register env.print_float: {}", e))?;

    // Host function: env.float_pow(i64, i64) -> i64 — f64 power
    linker
        .func_wrap(
            "env",
            "float_pow",
            |_caller: Caller<'_, RuntimeState>, base: i64, exp: i64| -> i64 {
                let b = f64::from_bits(base as u64);
                let e = f64::from_bits(exp as u64);
                let result = b.powf(e);
                result.to_bits() as i64
            },
        )
        .map_err(|e| format!("Failed to register env.float_pow: {}", e))?;

    // Host function: env.float_to_str(i64) -> i64 — convert f64 bits to packed string
    linker
        .func_wrap(
            "env",
            "float_to_str",
            |mut caller: Caller<'_, RuntimeState>, val: i64| -> i64 {
                let f = f64::from_bits(val as u64);
                let s = if f.fract() == 0.0 && f.is_finite() {
                    format!("{:.1}", f)
                } else {
                    format!("{}", f)
                };
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let heap_ptr = get_heap_ptr!(&mut caller);
                let bytes = s.as_bytes();
                memory.data_mut(&mut caller)
                    [heap_ptr as usize..heap_ptr as usize + bytes.len()]
                    .copy_from_slice(bytes);
                set_heap_ptr!(&mut caller, heap_ptr + bytes.len() as u32);
                ((heap_ptr as i64) << 32) | (bytes.len() as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.float_to_str: {}", e))?;

    // Host function: env.float_mod(i64, i64) -> i64 — f64 remainder
    linker
        .func_wrap(
            "env",
            "float_mod",
            |_caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let fa = f64::from_bits(a as u64);
                let fb = f64::from_bits(b as u64);
                let result = fa % fb;
                result.to_bits() as i64
            },
        )
        .map_err(|e| format!("Failed to register env.float_mod: {}", e))?;

    // Host function: env.int_to_str(i64) -> i64 — convert i64 to packed string (for str+int concat)
    linker
        .func_wrap(
            "env",
            "int_to_str",
            |mut caller: Caller<'_, RuntimeState>, val: i64| -> i64 {
                let s = format!("{}", val);
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let heap_ptr = get_heap_ptr!(&mut caller);
                let bytes = s.as_bytes();
                memory.data_mut(&mut caller)
                    [heap_ptr as usize..heap_ptr as usize + bytes.len()]
                    .copy_from_slice(bytes);
                set_heap_ptr!(&mut caller, heap_ptr + bytes.len() as u32);
                ((heap_ptr as i64) << 32) | (bytes.len() as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.int_to_str: {}", e))?;

    // Host function: env.table_set_table(table_id: i64, key: i64, child_id: i64)
    // Same as table_set but also registers a parent→child relationship for recursive free.
    linker
        .func_wrap(
            "env",
            "table_set_table",
            |mut caller: Caller<'_, RuntimeState>, table_id: i64, key: i64, child_id: i64| {
                let key_ptr = (key >> 32) as u32;
                let key_len = (key & 0xFFFF_FFFF) as u32;

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let start = key_ptr as usize;
                let end = start + key_len as usize;
                let key_str = if end <= data.len() {
                    std::str::from_utf8(&data[start..end])
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::new()
                };

                let pid = table_id as usize;
                let cid = child_id as usize;
                let state = caller.data_mut();
                if pid < state.tables.len() {
                    if let Some(ref mut table) = state.tables[pid] {
                        table.insert(key_str, child_id);
                    }
                    state.table_children.entry(pid).or_default().push(cid);
                }
            },
        )
        .map_err(|e| format!("Failed to register env.table_set_table: {}", e))?;

    // Host function: env.print_vec(i64) — print vec value as [x, y, z, ...]
    linker
        .func_wrap(
            "env",
            "print_vec",
            |mut caller: Caller<'_, RuntimeState>, val: i64| {
                let ptr = (val >> 32) as u32;
                let len = (val & 0xFFFF_FFFF) as u32;
                if len == 0 {
                    println!("[]");
                    return;
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let mut elements = Vec::new();
                for i in 0..len as usize {
                    let offset = ptr as usize + i * 8;
                    if offset + 8 <= data.len() {
                        let bytes = &data[offset..offset + 8];
                        let f = f64::from_le_bytes(bytes.try_into().unwrap());
                        elements.push(f);
                    }
                }
                print!("[");
                for (i, e) in elements.iter().enumerate() {
                    if i > 0 {
                        print!(", ");
                    }
                    if e.fract() == 0.0 && e.is_finite() {
                        print!("{:.1}", e);
                    } else {
                        print!("{}", e);
                    }
                }
                println!("]");
            },
        )
        .map_err(|e| format!("Failed to register env.print_vec: {}", e))?;

    // Host function: env.vec_get(vec: i64, index: i64) -> i64 — get element as f64 bits
    linker
        .func_wrap(
            "env",
            "vec_get",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, index: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as u32;
                let idx = index as usize;
                if idx >= len as usize {
                    return 0; // out of bounds → 0.0
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let offset = ptr as usize + idx * 8;
                if offset + 8 <= data.len() {
                    let bytes = &data[offset..offset + 8];
                    let f = f64::from_le_bytes(bytes.try_into().unwrap());
                    f.to_bits() as i64
                } else {
                    0
                }
            },
        )
        .map_err(|e| format!("Failed to register env.vec_get: {}", e))?;

    // Host function: env.vec_set(vec: i64, index: i64, value: i64) — set element from f64 bits
    linker
        .func_wrap(
            "env",
            "vec_set",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, index: i64, value: i64| {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as u32;
                let idx = index as usize;
                if idx >= len as usize {
                    return; // out of bounds → no-op
                }
                let f = f64::from_bits(value as u64);
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data_mut(&mut caller);
                let offset = ptr as usize + idx * 8;
                if offset + 8 <= data.len() {
                    data[offset..offset + 8].copy_from_slice(&f.to_le_bytes());
                }
            },
        )
        .map_err(|e| format!("Failed to register env.vec_set: {}", e))?;

    // Host function: env.vec_add(a: i64, b: i64) -> i64 — element-wise add
    linker
        .func_wrap(
            "env",
            "vec_add",
            |mut caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let a_ptr = (a >> 32) as u32;
                let a_len = (a & 0xFFFF_FFFF) as u32;
                let b_ptr = (b >> 32) as u32;
                let b_len = (b & 0xFFFF_FFFF) as u32;
                let n = a_len.min(b_len) as usize;
                if n == 0 {
                    return 0;
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let mut result_data = Vec::new();
                for i in 0..n {
                    let a_off = a_ptr as usize + i * 8;
                    let b_off = b_ptr as usize + i * 8;
                    let a_val = f64::from_le_bytes(data[a_off..a_off + 8].try_into().unwrap());
                    let b_val = f64::from_le_bytes(data[b_off..b_off + 8].try_into().unwrap());
                    result_data.push(a_val + b_val);
                }
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (n * 8) as u32);
                ((dest as i64) << 32) | (n as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_add: {}", e))?;

    // Host function: env.vec_sub(a: i64, b: i64) -> i64 — element-wise sub
    linker
        .func_wrap(
            "env",
            "vec_sub",
            |mut caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let a_ptr = (a >> 32) as u32;
                let a_len = (a & 0xFFFF_FFFF) as u32;
                let b_ptr = (b >> 32) as u32;
                let b_len = (b & 0xFFFF_FFFF) as u32;
                let n = a_len.min(b_len) as usize;
                if n == 0 {
                    return 0;
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let mut result_data = Vec::new();
                for i in 0..n {
                    let a_off = a_ptr as usize + i * 8;
                    let b_off = b_ptr as usize + i * 8;
                    let a_val = f64::from_le_bytes(data[a_off..a_off + 8].try_into().unwrap());
                    let b_val = f64::from_le_bytes(data[b_off..b_off + 8].try_into().unwrap());
                    result_data.push(a_val - b_val);
                }
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (n * 8) as u32);
                ((dest as i64) << 32) | (n as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_sub: {}", e))?;

    // Host function: env.vec_mul(a: i64, b: i64) -> i64 — element-wise mul
    linker
        .func_wrap(
            "env",
            "vec_mul",
            |mut caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let a_ptr = (a >> 32) as u32;
                let a_len = (a & 0xFFFF_FFFF) as u32;
                let b_ptr = (b >> 32) as u32;
                let b_len = (b & 0xFFFF_FFFF) as u32;
                let n = a_len.min(b_len) as usize;
                if n == 0 {
                    return 0;
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let mut result_data = Vec::new();
                for i in 0..n {
                    let a_off = a_ptr as usize + i * 8;
                    let b_off = b_ptr as usize + i * 8;
                    let a_val = f64::from_le_bytes(data[a_off..a_off + 8].try_into().unwrap());
                    let b_val = f64::from_le_bytes(data[b_off..b_off + 8].try_into().unwrap());
                    result_data.push(a_val * b_val);
                }
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (n * 8) as u32);
                ((dest as i64) << 32) | (n as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_mul: {}", e))?;

    // Host function: env.vec_scale(vec: i64, scalar: i64) -> i64 — scalar multiply
    linker
        .func_wrap(
            "env",
            "vec_scale",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, scalar: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as u32;
                let n = len as usize;
                if n == 0 {
                    return 0;
                }
                let s = f64::from_bits(scalar as u64);
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let mut result_data = Vec::new();
                for i in 0..n {
                    let offset = ptr as usize + i * 8;
                    let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                    result_data.push(val * s);
                }
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (n * 8) as u32);
                ((dest as i64) << 32) | (n as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_scale: {}", e))?;

    // Host function: env.vec_dot(a: i64, b: i64) -> i64 — dot product → f64 bits
    linker
        .func_wrap(
            "env",
            "vec_dot",
            |mut caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let a_ptr = (a >> 32) as u32;
                let a_len = (a & 0xFFFF_FFFF) as u32;
                let b_ptr = (b >> 32) as u32;
                let b_len = (b & 0xFFFF_FFFF) as u32;
                let n = a_len.min(b_len) as usize;
                if n == 0 {
                    return 0;
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let mut sum = 0.0;
                for i in 0..n {
                    let a_off = a_ptr as usize + i * 8;
                    let b_off = b_ptr as usize + i * 8;
                    let a_val = f64::from_le_bytes(data[a_off..a_off + 8].try_into().unwrap());
                    let b_val = f64::from_le_bytes(data[b_off..b_off + 8].try_into().unwrap());
                    sum += a_val * b_val;
                }
                sum.to_bits() as i64
            },
        )
        .map_err(|e| format!("Failed to register env.vec_dot: {}", e))?;

    // Host function: env.vec_cross(a: i64, b: i64) -> i64 — cross product (3D only)
    linker
        .func_wrap(
            "env",
            "vec_cross",
            |mut caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let a_ptr = (a >> 32) as u32;
                let a_len = (a & 0xFFFF_FFFF) as u32;
                let b_ptr = (b >> 32) as u32;
                let b_len = (b & 0xFFFF_FFFF) as u32;
                if a_len != 3 || b_len != 3 {
                    return 0; // cross product only for 3D vectors
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let a_x = f64::from_le_bytes(data[a_ptr as usize..a_ptr as usize + 8].try_into().unwrap());
                let a_y = f64::from_le_bytes(data[a_ptr as usize + 8..a_ptr as usize + 16].try_into().unwrap());
                let a_z = f64::from_le_bytes(data[a_ptr as usize + 16..a_ptr as usize + 24].try_into().unwrap());
                let b_x = f64::from_le_bytes(data[b_ptr as usize..b_ptr as usize + 8].try_into().unwrap());
                let b_y = f64::from_le_bytes(data[b_ptr as usize + 8..b_ptr as usize + 16].try_into().unwrap());
                let b_z = f64::from_le_bytes(data[b_ptr as usize + 16..b_ptr as usize + 24].try_into().unwrap());
                let c_x = a_y * b_z - a_z * b_y;
                let c_y = a_z * b_x - a_x * b_z;
                let c_z = a_x * b_y - a_y * b_x;
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                data_mut[dest as usize..dest as usize + 8].copy_from_slice(&c_x.to_le_bytes());
                data_mut[dest as usize + 8..dest as usize + 16].copy_from_slice(&c_y.to_le_bytes());
                data_mut[dest as usize + 16..dest as usize + 24].copy_from_slice(&c_z.to_le_bytes());
                set_heap_ptr!(&mut caller, dest + 24);
                ((dest as i64) << 32) | 3
            },
        )
        .map_err(|e| format!("Failed to register env.vec_cross: {}", e))?;

    // Host function: env.vec_swizzle(vec: i64, pattern: i64) -> i64
    // pattern: low 4 bits = count, then 4 bits per index
    linker
        .func_wrap(
            "env",
            "vec_swizzle",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, pattern: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as u32;
                let count = (pattern & 0xF) as usize;
                if count == 0 {
                    return 0;
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let mut result_data = Vec::new();
                for i in 0..count {
                    let idx = ((pattern >> (4 + i * 4)) & 0xF) as usize;
                    if idx < len as usize {
                        let offset = ptr as usize + idx * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        result_data.push(val);
                    } else {
                        result_data.push(0.0);
                    }
                }
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (count * 8) as u32);
                ((dest as i64) << 32) | (count as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_swizzle: {}", e))?;

    // ========== Matrix Host Functions ==========

    // Host function: env.print_mat(mat: i64) — print matrix
    linker
        .func_wrap(
            "env",
            "print_mat",
            |mut caller: Caller<'_, RuntimeState>, mat: i64| {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;
                if rows == 0 || cols == 0 {
                    println!("[[]]");
                    return;
                }
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                print!("[");
                for r in 0..rows {
                    if r > 0 {
                        print!("; ");
                    }
                    for c in 0..cols {
                        if c > 0 {
                            print!(", ");
                        }
                        let offset = ptr as usize + (r * cols + c) * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        if val.fract() == 0.0 && val.is_finite() {
                            print!("{:.1}", val);
                        } else {
                            print!("{}", val);
                        }
                    }
                }
                println!("]");
            },
        )
        .map_err(|e| format!("Failed to register env.print_mat: {}", e))?;

    // Host function: env.mat_add(a: i64, b: i64) -> i64
    linker
        .func_wrap(
            "env",
            "mat_add",
            |mut caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let ptr_a = (a >> 32) as u32;
                let meta_a = (a & 0xFFFF_FFFF) as u32;
                let rows_a = (meta_a >> 16) as usize;
                let cols_a = (meta_a & 0xFFFF) as usize;

                let ptr_b = (b >> 32) as u32;
                let meta_b = (b & 0xFFFF_FFFF) as u32;
                let rows_b = (meta_b >> 16) as usize;
                let cols_b = (meta_b & 0xFFFF) as usize;

                if rows_a != rows_b || cols_a != cols_b {
                    panic!("Matrix dimension mismatch");
                }

                let total = rows_a * cols_a;
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                let mut result = Vec::with_capacity(total);
                for i in 0..total {
                    let val_a = f64::from_le_bytes(
                        data[ptr_a as usize + i * 8..ptr_a as usize + (i + 1) * 8]
                            .try_into()
                            .unwrap(),
                    );
                    let val_b = f64::from_le_bytes(
                        data[ptr_b as usize + i * 8..ptr_b as usize + (i + 1) * 8]
                            .try_into()
                            .unwrap(),
                    );
                    result.push(val_a + val_b);
                }

                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result.iter().enumerate() {
                    data_mut[dest as usize + i * 8..dest as usize + (i + 1) * 8]
                        .copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (total * 8) as u32);
                let result_meta = ((rows_a as i64) << 16) | (cols_a as i64);
                ((dest as i64) << 32) | result_meta
            },
        )
        .map_err(|e| format!("Failed to register env.mat_add: {}", e))?;

    // Host function: env.mat_sub(a: i64, b: i64) -> i64
    linker
        .func_wrap(
            "env",
            "mat_sub",
            |mut caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let ptr_a = (a >> 32) as u32;
                let meta_a = (a & 0xFFFF_FFFF) as u32;
                let rows_a = (meta_a >> 16) as usize;
                let cols_a = (meta_a & 0xFFFF) as usize;

                let ptr_b = (b >> 32) as u32;
                let meta_b = (b & 0xFFFF_FFFF) as u32;
                let rows_b = (meta_b >> 16) as usize;
                let cols_b = (meta_b & 0xFFFF) as usize;

                if rows_a != rows_b || cols_a != cols_b {
                    panic!("Matrix dimension mismatch");
                }

                let total = rows_a * cols_a;
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                let mut result = Vec::with_capacity(total);
                for i in 0..total {
                    let val_a = f64::from_le_bytes(
                        data[ptr_a as usize + i * 8..ptr_a as usize + (i + 1) * 8]
                            .try_into()
                            .unwrap(),
                    );
                    let val_b = f64::from_le_bytes(
                        data[ptr_b as usize + i * 8..ptr_b as usize + (i + 1) * 8]
                            .try_into()
                            .unwrap(),
                    );
                    result.push(val_a - val_b);
                }

                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result.iter().enumerate() {
                    data_mut[dest as usize + i * 8..dest as usize + (i + 1) * 8]
                        .copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (total * 8) as u32);
                let result_meta = ((rows_a as i64) << 16) | (cols_a as i64);
                ((dest as i64) << 32) | result_meta
            },
        )
        .map_err(|e| format!("Failed to register env.mat_sub: {}", e))?;

    // Host function: env.mat_mul(a: i64, b: i64) -> i64
    linker
        .func_wrap(
            "env",
            "mat_mul",
            |mut caller: Caller<'_, RuntimeState>, a: i64, b: i64| -> i64 {
                let ptr_a = (a >> 32) as u32;
                let meta_a = (a & 0xFFFF_FFFF) as u32;
                let rows_a = (meta_a >> 16) as usize;
                let cols_a = (meta_a & 0xFFFF) as usize;

                let ptr_b = (b >> 32) as u32;
                let meta_b = (b & 0xFFFF_FFFF) as u32;
                let rows_b = (meta_b >> 16) as usize;
                let cols_b = (meta_b & 0xFFFF) as usize;

                if cols_a != rows_b {
                    panic!("Matrix dimension mismatch");
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                let mut result = vec![0.0; rows_a * cols_b];
                for i in 0..rows_a {
                    for j in 0..cols_b {
                        let mut sum = 0.0;
                        for k in 0..cols_a {
                            let val_a = f64::from_le_bytes(
                                data[ptr_a as usize + (i * cols_a + k) * 8
                                    ..ptr_a as usize + (i * cols_a + k + 1) * 8]
                                    .try_into()
                                    .unwrap(),
                            );
                            let val_b = f64::from_le_bytes(
                                data[ptr_b as usize + (k * cols_b + j) * 8
                                    ..ptr_b as usize + (k * cols_b + j + 1) * 8]
                                    .try_into()
                                    .unwrap(),
                            );
                            sum += val_a * val_b;
                        }
                        result[i * cols_b + j] = sum;
                    }
                }

                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result.iter().enumerate() {
                    data_mut[dest as usize + i * 8..dest as usize + (i + 1) * 8]
                        .copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (result.len() * 8) as u32);
                let result_meta = ((rows_a as i64) << 16) | (cols_b as i64);
                ((dest as i64) << 32) | result_meta
            },
        )
        .map_err(|e| format!("Failed to register env.mat_mul: {}", e))?;

    // Host function: env.mat_vec_mul(mat: i64, vec: i64) -> i64
    linker
        .func_wrap(
            "env",
            "mat_vec_mul",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, vec: i64| -> i64 {
                let ptr_m = (mat >> 32) as u32;
                let meta_m = (mat & 0xFFFF_FFFF) as u32;
                let rows_m = (meta_m >> 16) as usize;
                let cols_m = (meta_m & 0xFFFF) as usize;

                let ptr_v = (vec >> 32) as u32;
                let len_v = (vec & 0xFFFF_FFFF) as usize;

                if cols_m != len_v {
                    panic!("Dimension mismatch");
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                let mut result = vec![0.0; rows_m];
                for i in 0..rows_m {
                    let mut sum = 0.0;
                    for j in 0..cols_m {
                        let val_m = f64::from_le_bytes(
                            data[ptr_m as usize + (i * cols_m + j) * 8
                                ..ptr_m as usize + (i * cols_m + j + 1) * 8]
                                .try_into()
                                .unwrap(),
                        );
                        let val_v = f64::from_le_bytes(
                            data[ptr_v as usize + j * 8..ptr_v as usize + (j + 1) * 8]
                                .try_into()
                                .unwrap(),
                        );
                        sum += val_m * val_v;
                    }
                    result[i] = sum;
                }

                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result.iter().enumerate() {
                    data_mut[dest as usize + i * 8..dest as usize + (i + 1) * 8]
                        .copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (result.len() * 8) as u32);
                ((dest as i64) << 32) | (rows_m as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.mat_vec_mul: {}", e))?;

    // Host function: env.mat_scale(mat: i64, scalar: i64) -> i64
    linker
        .func_wrap(
            "env",
            "mat_scale",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, scalar: i64| -> i64 {
                let ptr_m = (mat >> 32) as u32;
                let meta_m = (mat & 0xFFFF_FFFF) as u32;
                let rows_m = (meta_m >> 16) as usize;
                let cols_m = (meta_m & 0xFFFF) as usize;
                let total = rows_m * cols_m;

                let scalar_f64 = f64::from_bits(scalar as u64);
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                let mut result = Vec::with_capacity(total);
                for i in 0..total {
                    let val = f64::from_le_bytes(
                        data[ptr_m as usize + i * 8..ptr_m as usize + (i + 1) * 8]
                            .try_into()
                            .unwrap(),
                    );
                    result.push(val * scalar_f64);
                }

                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result.iter().enumerate() {
                    data_mut[dest as usize + i * 8..dest as usize + (i + 1) * 8]
                        .copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (total * 8) as u32);
                let result_meta = ((rows_m as i64) << 16) | (cols_m as i64);
                ((dest as i64) << 32) | result_meta
            },
        )
        .map_err(|e| format!("Failed to register env.mat_scale: {}", e))?;

    // Host function: env.mat_transpose(mat: i64) -> i64
    linker
        .func_wrap(
            "env",
            "mat_transpose",
            |mut caller: Caller<'_, RuntimeState>, mat: i64| -> i64 {
                let ptr_m = (mat >> 32) as u32;
                let meta_m = (mat & 0xFFFF_FFFF) as u32;
                let rows_m = (meta_m >> 16) as usize;
                let cols_m = (meta_m & 0xFFFF) as usize;
                let total = rows_m * cols_m;

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                let mut result = vec![0.0; total];
                for i in 0..rows_m {
                    for j in 0..cols_m {
                        let val = f64::from_le_bytes(
                            data[ptr_m as usize + (i * cols_m + j) * 8
                                ..ptr_m as usize + (i * cols_m + j + 1) * 8]
                                .try_into()
                                .unwrap(),
                        );
                        result[j * rows_m + i] = val;
                    }
                }

                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result.iter().enumerate() {
                    data_mut[dest as usize + i * 8..dest as usize + (i + 1) * 8]
                        .copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (total * 8) as u32);
                let result_meta = ((cols_m as i64) << 16) | (rows_m as i64);
                ((dest as i64) << 32) | result_meta
            },
        )
        .map_err(|e| format!("Failed to register env.mat_transpose: {}", e))?;

    // Host function: env.mat_det(mat: i64) -> i64
    linker
        .func_wrap(
            "env",
            "mat_det",
            |mut caller: Caller<'_, RuntimeState>, mat: i64| -> i64 {
                let ptr_m = (mat >> 32) as u32;
                let meta_m = (mat & 0xFFFF_FFFF) as u32;
                let rows_m = (meta_m >> 16) as usize;
                let cols_m = (meta_m & 0xFFFF) as usize;

                if rows_m != cols_m {
                    panic!("Determinant requires square matrix");
                }

                let n = rows_m;
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                let mut matrix = vec![vec![0.0; n]; n];
                for i in 0..n {
                    for j in 0..n {
                        matrix[i][j] = f64::from_le_bytes(
                            data[ptr_m as usize + (i * n + j) * 8..ptr_m as usize + (i * n + j + 1) * 8]
                                .try_into()
                                .unwrap(),
                        );
                    }
                }

                calculate_determinant(&matrix).to_bits() as i64
            },
        )
        .map_err(|e| format!("Failed to register env.mat_det: {}", e))?;

    // Host function: env.mat_inv(mat: i64) -> i64
    linker
        .func_wrap(
            "env",
            "mat_inv",
            |mut caller: Caller<'_, RuntimeState>, mat: i64| -> i64 {
                let ptr_m = (mat >> 32) as u32;
                let meta_m = (mat & 0xFFFF_FFFF) as u32;
                let rows_m = (meta_m >> 16) as usize;
                let cols_m = (meta_m & 0xFFFF) as usize;

                if rows_m != cols_m {
                    panic!("Inverse requires square matrix");
                }

                let n = rows_m;
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                let mut matrix = vec![vec![0.0; n * 2]; n];
                for i in 0..n {
                    for j in 0..n {
                        matrix[i][j] = f64::from_le_bytes(
                            data[ptr_m as usize + (i * n + j) * 8..ptr_m as usize + (i * n + j + 1) * 8]
                                .try_into()
                                .unwrap(),
                        );
                    }
                    for j in 0..n {
                        matrix[i][n + j] = if i == j { 1.0 } else { 0.0 };
                    }
                }

                if !gauss_jordan_inversion(&mut matrix, n) {
                    panic!("Matrix is singular");
                }

                let mut result = vec![0.0; n * n];
                for i in 0..n {
                    for j in 0..n {
                        result[i * n + j] = matrix[i][n + j];
                    }
                }

                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result.iter().enumerate() {
                    data_mut[dest as usize + i * 8..dest as usize + (i + 1) * 8]
                        .copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (result.len() * 8) as u32);
                let result_meta = ((rows_m as i64) << 16) | (cols_m as i64);
                ((dest as i64) << 32) | result_meta
            },
        )
        .map_err(|e| format!("Failed to register env.mat_inv: {}", e))?;

    // Host function: env.mat_get(mat: i64, linear_index: i64) -> i64
    linker
        .func_wrap(
            "env",
            "mat_get",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, linear_index: i64| -> i64 {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;
                let idx = linear_index as usize;

                if idx >= rows * cols {
                    panic!("Index out of bounds");
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);
                let val = f64::from_le_bytes(
                    data[ptr as usize + idx * 8..ptr as usize + (idx + 1) * 8]
                        .try_into()
                        .unwrap(),
                );
                val.to_bits() as i64
            },
        )
        .map_err(|e| format!("Failed to register env.mat_get: {}", e))?;

    // Host function: env.mat_set(mat: i64, linear_index: i64, value: i64)
    linker
        .func_wrap(
            "env",
            "mat_set",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, linear_index: i64, value: i64| {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;
                let idx = linear_index as usize;

                if idx >= rows * cols {
                    panic!("Index out of bounds");
                }

                let val = f64::from_bits(value as u64);
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data_mut = memory.data_mut(&mut caller);
                data_mut[ptr as usize + idx * 8..ptr as usize + (idx + 1) * 8]
                    .copy_from_slice(&val.to_le_bytes());
            },
        )
        .map_err(|e| format!("Failed to register env.mat_set: {}", e))?;

    // Host function: env.mat_solve(A: i64, b: i64) -> i64
    // Solves Ax=b using LU decomposition with partial pivoting
    // A is m×n matrix, b can be m×1 vector or m×k matrix
    linker
        .func_wrap(
            "env",
            "mat_solve",
            |mut caller: Caller<'_, RuntimeState>, a_val: i64, b_val: i64| -> i64 {
                // Extract matrix A
                let a_ptr = (a_val >> 32) as u32;
                let a_meta = (a_val & 0xFFFF_FFFF) as u32;
                let a_rows = (a_meta >> 16) as usize;
                let a_cols = (a_meta & 0xFFFF) as usize;
                if a_rows == 0 || a_cols == 0 || a_rows != a_cols {
                    return 0; // Must be square matrix
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                // Read A
                let mut a = vec![vec![0.0; a_cols]; a_rows];
                for i in 0..a_rows {
                    for j in 0..a_cols {
                        let offset = a_ptr as usize + (i * a_cols + j) * 8;
                        a[i][j] = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                    }
                }

                // Check if b is vector or matrix
                let b_ptr = (b_val >> 32) as u32;
                let b_len_or_meta = (b_val & 0xFFFF_FFFF) as u32;

                let is_vec = b_len_or_meta <= 0xFFFF;
                let (b_rows, b_cols) = if is_vec {
                    (b_len_or_meta as usize, 1)
                } else {
                    ((b_len_or_meta >> 16) as usize, (b_len_or_meta & 0xFFFF) as usize)
                };

                if b_rows != a_rows {
                    return 0; // Dimension mismatch
                }

                // Read b
                let mut b = vec![vec![0.0; b_cols]; b_rows];
                if is_vec {
                    for i in 0..b_rows {
                        let offset = b_ptr as usize + i * 8;
                        b[i][0] = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                    }
                } else {
                    for i in 0..b_rows {
                        for j in 0..b_cols {
                            let offset = b_ptr as usize + (i * b_cols + j) * 8;
                            b[i][j] = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        }
                    }
                }

                // LU decomposition with partial pivoting
                let n = a_rows;
                let mut lu = a.clone();
                let mut perm: Vec<usize> = (0..n).collect();

                for k in 0..n {
                    // Find pivot
                    let mut max_row = k;
                    for i in k + 1..n {
                        if lu[i][k].abs() > lu[max_row][k].abs() {
                            max_row = i;
                        }
                    }
                    if lu[max_row][k].abs() < 1e-10 {
                        return 0; // Singular matrix
                    }
                    if max_row != k {
                        lu.swap(k, max_row);
                        perm.swap(k, max_row);
                    }

                    // Elimination
                    for i in k + 1..n {
                        lu[i][k] /= lu[k][k];
                        for j in k + 1..n {
                            lu[i][j] -= lu[i][k] * lu[k][j];
                        }
                    }
                }

                // Solve for each column of b
                let mut x = vec![vec![0.0; b_cols]; n];
                for col in 0..b_cols {
                    // Permute b
                    let mut pb = vec![0.0; n];
                    for i in 0..n {
                        pb[i] = b[perm[i]][col];
                    }

                    // Forward substitution (Ly = Pb)
                    let mut y = vec![0.0; n];
                    for i in 0..n {
                        y[i] = pb[i];
                        for j in 0..i {
                            y[i] -= lu[i][j] * y[j];
                        }
                    }

                    // Backward substitution (Ux = y)
                    for i in (0..n).rev() {
                        x[i][col] = y[i];
                        for j in i + 1..n {
                            x[i][col] -= lu[i][j] * x[j][col];
                        }
                        x[i][col] /= lu[i][i];
                    }
                }

                // Allocate and write result
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);

                if is_vec {
                    // Return as vector
                    for i in 0..n {
                        let offset = dest as usize + i * 8;
                        data_mut[offset..offset + 8].copy_from_slice(&x[i][0].to_le_bytes());
                    }
                    set_heap_ptr!(&mut caller, dest + (n * 8) as u32);
                    ((dest as i64) << 32) | (n as i64)
                } else {
                    // Return as matrix
                    for i in 0..n {
                        for j in 0..b_cols {
                            let offset = dest as usize + (i * b_cols + j) * 8;
                            data_mut[offset..offset + 8].copy_from_slice(&x[i][j].to_le_bytes());
                        }
                    }
                    set_heap_ptr!(&mut caller, dest + (n * b_cols * 8) as u32);
                    let meta = ((n as i64) << 16) | (b_cols as i64);
                    ((dest as i64) << 32) | meta
                }
            },
        )
        .map_err(|e| format!("Failed to register env.mat_solve: {}", e))?;

    // Host function: env.vec_pow(vec: i64, exp: i64) -> i64
    // Element-wise power: each element raised to exp (f64 bits)
    linker
        .func_wrap(
            "env",
            "vec_pow",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, exp: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as u32;
                if len == 0 {
                    return 0;
                }

                let exp_f = f64::from_bits(exp as u64);
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                // Compute element-wise power
                let mut result_data = Vec::new();
                for i in 0..len as usize {
                    let offset = ptr as usize + i * 8;
                    let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                    result_data.push(val.powf(exp_f));
                }

                // Allocate and write result
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (len * 8) as u32);
                ((dest as i64) << 32) | (len as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_pow: {}", e))?;

    // Host function: env.mat_pow(mat: i64, exp: i64) -> i64
    // Element-wise power: each element raised to exp (f64 bits)
    linker
        .func_wrap(
            "env",
            "mat_pow",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, exp: i64| -> i64 {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;
                if rows == 0 || cols == 0 {
                    return 0;
                }

                let exp_f = f64::from_bits(exp as u64);
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                // Compute element-wise power
                let mut result_data = Vec::new();
                for i in 0..rows {
                    for j in 0..cols {
                        let offset = ptr as usize + (i * cols + j) * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        result_data.push(val.powf(exp_f));
                    }
                }

                // Allocate and write result
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (rows * cols * 8) as u32);
                let result_meta = ((rows as i64) << 16) | (cols as i64);
                ((dest as i64) << 32) | result_meta
            },
        )
        .map_err(|e| format!("Failed to register env.mat_pow: {}", e))?;

    // Host function: env.vec_slice(vec: i64, start: i64, end: i64) -> i64
    // Slice vector: v[start..end]
    // start=-1 means from beginning, end=-1 means to end
    linker
        .func_wrap(
            "env",
            "vec_slice",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, start: i64, end: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as usize;

                // Handle start and end indices
                let start_idx = if start == -1 { 0 } else { start as usize };
                let end_idx = if end == -1 { len } else { end as usize };

                // Validate range
                if start_idx > len || end_idx > len || start_idx > end_idx {
                    return 0; // Return empty vec on invalid range
                }

                let slice_len = end_idx - start_idx;
                if slice_len == 0 {
                    return 0;
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                // Copy slice data
                let mut result_data = Vec::with_capacity(slice_len);
                for i in start_idx..end_idx {
                    let offset = ptr as usize + i * 8;
                    let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                    result_data.push(val);
                }

                // Allocate and write result
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (slice_len * 8) as u32);
                ((dest as i64) << 32) | (slice_len as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_slice: {}", e))?;

    // Host function: env.mat_slice(mat: i64, start: i64, end: i64) -> i64
    // Slice matrix rows: m[start..end]
    // Returns a matrix with rows [start, end)
    linker
        .func_wrap(
            "env",
            "mat_slice",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, start: i64, end: i64| -> i64 {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;

                // Handle start and end indices
                let start_idx = if start == -1 { 0 } else { start as usize };
                let end_idx = if end == -1 { rows } else { end as usize };

                // Validate range
                if start_idx > rows || end_idx > rows || start_idx > end_idx {
                    return 0; // Return empty mat on invalid range
                }

                let slice_rows = end_idx - start_idx;
                if slice_rows == 0 {
                    return 0;
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                // Copy slice data (rows start_idx to end_idx-1)
                let mut result_data = Vec::with_capacity(slice_rows * cols);
                for i in start_idx..end_idx {
                    for j in 0..cols {
                        let offset = ptr as usize + (i * cols + j) * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        result_data.push(val);
                    }
                }

                // Allocate and write result
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (result_data.len() * 8) as u32);
                let result_meta = ((slice_rows as i64) << 16) | (cols as i64);
                ((dest as i64) << 32) | result_meta
            },
        )
        .map_err(|e| format!("Failed to register env.mat_slice: {}", e))?;

    // Host function: env.vec_fancy_index(vec: i64, indices: i64) -> i64
    // Fancy indexing: v[[i1, i2, i3]]
    // indices is a packed vec containing integer indices
    linker
        .func_wrap(
            "env",
            "vec_fancy_index",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, indices: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as usize;

                let idx_ptr = (indices >> 32) as u32;
                let idx_len = (indices & 0xFFFF_FFFF) as usize;

                if idx_len == 0 {
                    return 0;
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                // Read indices and extract elements
                let mut result_data = Vec::with_capacity(idx_len);
                for i in 0..idx_len {
                    let idx_offset = idx_ptr as usize + i * 8;
                    let idx_f64 = f64::from_le_bytes(data[idx_offset..idx_offset + 8].try_into().unwrap());
                    let idx = idx_f64 as usize;
                    if idx < len {
                        let offset = ptr as usize + idx * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        result_data.push(val);
                    } else {
                        result_data.push(0.0); // Out of bounds → 0.0
                    }
                }

                // Allocate and write result
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (idx_len * 8) as u32);
                ((dest as i64) << 32) | (idx_len as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_fancy_index: {}", e))?;

    // Host function: env.mat_fancy_index(mat: i64, indices: i64) -> i64
    // Fancy row indexing: m[[i1, i2, i3]] selects rows i1, i2, i3
    linker
        .func_wrap(
            "env",
            "mat_fancy_index",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, indices: i64| -> i64 {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;

                let idx_ptr = (indices >> 32) as u32;
                let idx_len = (indices & 0xFFFF_FFFF) as usize;

                if idx_len == 0 {
                    return 0;
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                // Read row indices and extract rows
                let mut result_data = Vec::with_capacity(idx_len * cols);
                for i in 0..idx_len {
                    let idx_offset = idx_ptr as usize + i * 8;
                    let idx_f64 = f64::from_le_bytes(data[idx_offset..idx_offset + 8].try_into().unwrap());
                    let row_idx = idx_f64 as usize;
                    if row_idx < rows {
                        // Copy entire row
                        for j in 0..cols {
                            let offset = ptr as usize + (row_idx * cols + j) * 8;
                            let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                            result_data.push(val);
                        }
                    } else {
                        // Out of bounds → fill with zeros
                        for _ in 0..cols {
                            result_data.push(0.0);
                        }
                    }
                }

                // Allocate and write result
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, val) in result_data.iter().enumerate() {
                    let offset = dest as usize + i * 8;
                    data_mut[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (result_data.len() * 8) as u32);
                let result_meta = ((idx_len as i64) << 16) | (cols as i64);
                ((dest as i64) << 32) | result_meta
            },
        )
        .map_err(|e| format!("Failed to register env.mat_fancy_index: {}", e))?;

    // Host function: env.vec_add_scalar(vec: i64, scalar: i64) -> i64
    // Broadcast scalar addition: v + s → [v[0]+s, v[1]+s, ...]
    linker
        .func_wrap(
            "env",
            "vec_add_scalar",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, scalar: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as usize;
                let scalar_f64 = f64::from_bits(scalar as u64);

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");

                // Read all data first (immutable borrow)
                let data = memory.data(&caller);
                let result_data: Vec<f64> = (0..len)
                    .map(|i| {
                        let offset = ptr as usize + i * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        val + scalar_f64
                    })
                    .collect();

                // Now allocate and write (mutable borrow)
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (len * 8) as u32);
                ((dest as i64) << 32) | (len as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_add_scalar: {}", e))?;

    // Host function: env.vec_sub_scalar(vec: i64, scalar: i64) -> i64
    // Broadcast scalar subtraction: v - s → [v[0]-s, v[1]-s, ...]
    linker
        .func_wrap(
            "env",
            "vec_sub_scalar",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, scalar: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as usize;
                let scalar_f64 = f64::from_bits(scalar as u64);

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");

                // Read all data first (immutable borrow)
                let data = memory.data(&caller);
                let result_data: Vec<f64> = (0..len)
                    .map(|i| {
                        let offset = ptr as usize + i * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        val - scalar_f64
                    })
                    .collect();

                // Now allocate and write (mutable borrow)
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (len * 8) as u32);
                ((dest as i64) << 32) | (len as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_sub_scalar: {}", e))?;

    // Host function: env.vec_div_scalar(vec: i64, scalar: i64) -> i64
    // Broadcast scalar division: v / s → [v[0]/s, v[1]/s, ...]
    linker
        .func_wrap(
            "env",
            "vec_div_scalar",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, scalar: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as usize;
                let scalar_f64 = f64::from_bits(scalar as u64);

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");

                // Read all data first (immutable borrow)
                let data = memory.data(&caller);
                let result_data: Vec<f64> = (0..len)
                    .map(|i| {
                        let offset = ptr as usize + i * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        val / scalar_f64
                    })
                    .collect();

                // Now allocate and write (mutable borrow)
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (len * 8) as u32);
                ((dest as i64) << 32) | (len as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_div_scalar: {}", e))?;

    // Host function: env.mat_add_scalar(mat: i64, scalar: i64) -> i64
    // Broadcast scalar addition to all elements
    linker
        .func_wrap(
            "env",
            "mat_add_scalar",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, scalar: i64| -> i64 {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;
                let scalar_f64 = f64::from_bits(scalar as u64);

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");

                // Read all data first (immutable borrow)
                let total = rows * cols;
                let data = memory.data(&caller);
                let result_data: Vec<f64> = (0..total)
                    .map(|i| {
                        let offset = ptr as usize + i * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        val + scalar_f64
                    })
                    .collect();

                // Now allocate and write (mutable borrow)
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (total * 8) as u32);
                ((dest as i64) << 32) | (meta as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.mat_add_scalar: {}", e))?;

    // Host function: env.mat_sub_scalar(mat: i64, scalar: i64) -> i64
    // Broadcast scalar subtraction from all elements
    linker
        .func_wrap(
            "env",
            "mat_sub_scalar",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, scalar: i64| -> i64 {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;
                let scalar_f64 = f64::from_bits(scalar as u64);

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");

                // Read all data first (immutable borrow)
                let total = rows * cols;
                let data = memory.data(&caller);
                let result_data: Vec<f64> = (0..total)
                    .map(|i| {
                        let offset = ptr as usize + i * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        val - scalar_f64
                    })
                    .collect();

                // Now allocate and write (mutable borrow)
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (total * 8) as u32);
                ((dest as i64) << 32) | (meta as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.mat_sub_scalar: {}", e))?;

    // Host function: env.mat_div_scalar(mat: i64, scalar: i64) -> i64
    // Broadcast scalar division to all elements
    linker
        .func_wrap(
            "env",
            "mat_div_scalar",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, scalar: i64| -> i64 {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;
                let scalar_f64 = f64::from_bits(scalar as u64);

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");

                // Read all data first (immutable borrow)
                let total = rows * cols;
                let data = memory.data(&caller);
                let result_data: Vec<f64> = (0..total)
                    .map(|i| {
                        let offset = ptr as usize + i * 8;
                        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                        val / scalar_f64
                    })
                    .collect();

                // Now allocate and write (mutable borrow)
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (total * 8) as u32);
                ((dest as i64) << 32) | (meta as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.mat_div_scalar: {}", e))?;

    // Host function: env.mat_add_vec_broadcast(mat: i64, vec: i64) -> i64
    // Broadcast vector addition to each row: M + v → each row M[i] + v
    linker
        .func_wrap(
            "env",
            "mat_add_vec_broadcast",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, vec: i64| -> i64 {
                let mat_ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;

                let vec_ptr = (vec >> 32) as u32;
                let vec_len = (vec & 0xFFFF_FFFF) as usize;

                if vec_len != cols {
                    // Dimension mismatch → return zero matrix
                    return 0;
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");

                // Read all data first (immutable borrow)
                let data = memory.data(&caller);
                let mut result_data = Vec::with_capacity(rows * cols);
                for i in 0..rows {
                    for j in 0..cols {
                        let mat_offset = mat_ptr as usize + (i * cols + j) * 8;
                        let mat_val = f64::from_le_bytes(data[mat_offset..mat_offset + 8].try_into().unwrap());

                        let vec_offset = vec_ptr as usize + j * 8;
                        let vec_val = f64::from_le_bytes(data[vec_offset..vec_offset + 8].try_into().unwrap());

                        result_data.push(mat_val + vec_val);
                    }
                }

                // Now allocate and write (mutable borrow)
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (result_data.len() * 8) as u32);
                ((dest as i64) << 32) | (meta as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.mat_add_vec_broadcast: {}", e))?;

    // Host function: env.mat_sub_vec_broadcast(mat: i64, vec: i64) -> i64
    // Broadcast vector subtraction from each row: M - v → each row M[i] - v
    linker
        .func_wrap(
            "env",
            "mat_sub_vec_broadcast",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, vec: i64| -> i64 {
                let mat_ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;

                let vec_ptr = (vec >> 32) as u32;
                let vec_len = (vec & 0xFFFF_FFFF) as usize;

                if vec_len != cols {
                    return 0;
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");

                // Read all data first (immutable borrow)
                let data = memory.data(&caller);
                let mut result_data = Vec::with_capacity(rows * cols);
                for i in 0..rows {
                    for j in 0..cols {
                        let mat_offset = mat_ptr as usize + (i * cols + j) * 8;
                        let mat_val = f64::from_le_bytes(data[mat_offset..mat_offset + 8].try_into().unwrap());

                        let vec_offset = vec_ptr as usize + j * 8;
                        let vec_val = f64::from_le_bytes(data[vec_offset..vec_offset + 8].try_into().unwrap());

                        result_data.push(mat_val - vec_val);
                    }
                }

                // Now allocate and write (mutable borrow)
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (result_data.len() * 8) as u32);
                ((dest as i64) << 32) | (meta as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.mat_sub_vec_broadcast: {}", e))?;

    // Host function: env.vec_mask(vec: i64, threshold: i64, op: i64) -> i64
    // Boolean masking: v[v > threshold] → filter elements that satisfy the condition
    // op codes: 0=>, 1=<, 2=>=, 3=<=, 4===, 5=!=
    linker
        .func_wrap(
            "env",
            "vec_mask",
            |mut caller: Caller<'_, RuntimeState>, vec: i64, threshold: i64, op: i64| -> i64 {
                let ptr = (vec >> 32) as u32;
                let len = (vec & 0xFFFF_FFFF) as usize;
                let threshold_f64 = f64::from_bits(threshold as u64);

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                // Filter elements based on comparison operator
                let mut result_data = Vec::new();
                for i in 0..len {
                    let offset = ptr as usize + i * 8;
                    let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                    let matches = match op {
                        0 => val > threshold_f64,   // >
                        1 => val < threshold_f64,   // <
                        2 => val >= threshold_f64,  // >=
                        3 => val <= threshold_f64,  // <=
                        4 => val == threshold_f64,  // ==
                        5 => val != threshold_f64,  // !=
                        _ => false,
                    };
                    if matches {
                        result_data.push(val);
                    }
                }

                // Allocate and write filtered result
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (result_data.len() * 8) as u32);
                ((dest as i64) << 32) | (result_data.len() as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.vec_mask: {}", e))?;

    // Host function: env.mat_mask(mat: i64, threshold: i64, op: i64) -> i64
    // Boolean masking for matrix: flatten and filter elements that satisfy the condition
    linker
        .func_wrap(
            "env",
            "mat_mask",
            |mut caller: Caller<'_, RuntimeState>, mat: i64, threshold: i64, op: i64| -> i64 {
                let ptr = (mat >> 32) as u32;
                let meta = (mat & 0xFFFF_FFFF) as u32;
                let rows = (meta >> 16) as usize;
                let cols = (meta & 0xFFFF) as usize;
                let threshold_f64 = f64::from_bits(threshold as u64);

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("missing memory export");
                let data = memory.data(&caller);

                // Flatten matrix and filter elements
                let total = rows * cols;
                let mut result_data = Vec::new();
                for i in 0..total {
                    let offset = ptr as usize + i * 8;
                    let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                    let matches = match op {
                        0 => val > threshold_f64,
                        1 => val < threshold_f64,
                        2 => val >= threshold_f64,
                        3 => val <= threshold_f64,
                        4 => val == threshold_f64,
                        5 => val != threshold_f64,
                        _ => false,
                    };
                    if matches {
                        result_data.push(val);
                    }
                }

                // Return as Vec (flattened)
                let dest = get_heap_ptr!(&mut caller);
                let data_mut = memory.data_mut(&mut caller);
                for (i, &val) in result_data.iter().enumerate() {
                    let dest_offset = dest as usize + i * 8;
                    data_mut[dest_offset..dest_offset + 8].copy_from_slice(&val.to_le_bytes());
                }
                set_heap_ptr!(&mut caller, dest + (result_data.len() * 8) as u32);
                ((dest as i64) << 32) | (result_data.len() as i64)
            },
        )
        .map_err(|e| format!("Failed to register env.mat_mask: {}", e))?;

    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| format!("Failed to instantiate module: {}", e))?;

    // heap_ptr is now managed via __heap_base global in WASM, no initialization needed

    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .map_err(|e| format!("No _start function found: {}", e))?;

    start
        .call(&mut store, ())
        .map_err(|e| format!("Execution failed: {}", e))?;

    Ok(())
}

// Matrix helper: calculate determinant using LU decomposition
fn calculate_determinant(matrix: &[Vec<f64>]) -> f64 {
    let n = matrix.len();
    if n == 0 { return 0.0; }
    if n == 1 { return matrix[0][0]; }
    if n == 2 { return matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]; }

    let mut a = matrix.to_vec();
    let mut det = 1.0;

    for i in 0..n {
        let mut max_row = i;
        for k in i+1..n {
            if a[k][i].abs() > a[max_row][i].abs() {
                max_row = k;
            }
        }
        if max_row != i {
            a.swap(i, max_row);
            det = -det;
        }
        if a[i][i].abs() < 1e-10 { return 0.0; }
        det *= a[i][i];
        for k in i+1..n {
            let factor = a[k][i] / a[i][i];
            for j in i+1..n {
                a[k][j] -= factor * a[i][j];
            }
        }
    }
    det
}

// Matrix helper: Gauss-Jordan elimination for matrix inversion
fn gauss_jordan_inversion(matrix: &mut [Vec<f64>], n: usize) -> bool {
    for i in 0..n {
        let mut max_row = i;
        for k in i+1..n {
            if matrix[k][i].abs() > matrix[max_row][i].abs() {
                max_row = k;
            }
        }
        if max_row != i {
            matrix.swap(i, max_row);
        }
        if matrix[i][i].abs() < 1e-10 { return false; }
        let pivot = matrix[i][i];
        for j in 0..n*2 {
            matrix[i][j] /= pivot;
        }
        for k in 0..n {
            if k != i {
                let factor = matrix[k][i];
                for j in 0..n*2 {
                    matrix[k][j] -= factor * matrix[i][j];
                }
            }
        }
    }
    true
}

fn print_usage() {
    eprintln!("AnehtaLanguage Compiler & Runtime");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  anehta build <source.ah>    Compile to .wasm");
    eprintln!("  anehta run <source.ah>      Compile and execute");
    eprintln!("  anehta <source.ah>          Compile to .wasm (shorthand)");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        1 => {
            print_usage();
            std::process::exit(1);
        }
        2 => {
            // anehta <file.ah> — default to build
            cmd_build(&args[1]);
        }
        3 => match args[1].as_str() {
            "build" => cmd_build(&args[2]),
            "run" => cmd_run(&args[2]),
            _ => {
                eprintln!("Unknown command: {}", args[1]);
                print_usage();
                std::process::exit(1);
            }
        },
        _ => {
            print_usage();
            std::process::exit(1);
        }
    }
}
