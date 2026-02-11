use std::env;
use std::fs;

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
    /// Bump-allocator pointer for string concatenation results.
    /// Initialised from the exported `__heap_base` global after instantiation.
    heap_ptr: u32,
    /// Host-side tables: each slot is Some(table) or None (freed).
    /// WASM references tables by their index (table_id) in this Vec.
    tables: Vec<Option<std::collections::HashMap<String, i64>>>,
    /// Indices of freed table slots available for reuse.
    free_slots: Vec<usize>,
    /// Parent → children relationships for recursive table freeing.
    table_children: std::collections::HashMap<usize, Vec<usize>>,
}

fn execute_wasm(wasm_bytes: &[u8]) -> Result<(), String> {
    use rand::Rng;
    use wasmtime::*;

    let engine = Engine::default();
    let module = Module::new(&engine, wasm_bytes)
        .map_err(|e| format!("Failed to load WASM module: {}", e))?;

    let state = RuntimeState {
        start_instant: std::time::Instant::now(),
        heap_ptr: 0,
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
                let dest = caller.data().heap_ptr;

                // Write concatenated result into linear memory
                let data_mut = memory.data_mut(&mut caller);
                data_mut[dest as usize..dest as usize + a_len as usize]
                    .copy_from_slice(&a_bytes);
                data_mut[dest as usize + a_len as usize..dest as usize + new_len as usize]
                    .copy_from_slice(&b_bytes);

                // Advance the bump allocator
                caller.data_mut().heap_ptr = dest + new_len;

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

    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| format!("Failed to instantiate module: {}", e))?;

    // Initialise heap pointer from __heap_base export (if present)
    if let Some(global) = instance.get_global(&mut store, "__heap_base") {
        let val = global.get(&mut store);
        if let wasmtime::Val::I32(v) = val {
            store.data_mut().heap_ptr = v as u32;
        }
    }

    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .map_err(|e| format!("No _start function found: {}", e))?;

    start
        .call(&mut store, ())
        .map_err(|e| format!("Execution failed: {}", e))?;

    Ok(())
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
