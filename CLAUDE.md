# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```powershell
# Build entire workspace
cargo build --release

# Build a single crate
cargo build -p anehta-lexer
cargo build -p anehta-parser
cargo build -p anehta-codegen-wasm
cargo build -p anehta-cli

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p anehta-lexer
cargo test -p anehta-parser
cargo test -p anehta-codegen-wasm

# Run a single test
cargo test -p anehta-codegen-wasm -- test_name

# Compile an .ah source file to .wasm
& "target\release\anehta-cli.exe" build examples\demo.ah

# Compile and execute
& "target\release\anehta-cli.exe" run examples\demo.ah

# Check for compile errors without building
cargo check --workspace
```

## Architecture

AnehtaLanguage is an experimental language that compiles `.ah` source files to WASM, then executes them via wasmtime. The compiler is a Rust workspace with 4 crates forming a pipeline:

```
.ah source → anehta-lexer → anehta-parser → anehta-codegen-wasm → .wasm bytes
                                                                      ↓
                                                              anehta-cli (wasmtime)
```

### Crate Responsibilities

- **anehta-lexer** (`crates/anehta-lexer/`) — Tokenizer. Exports `Lexer`, `Token`, `TokenType`, `Span`. Newlines are tokens (`TokenType::Newline`) used as statement separators.
- **anehta-parser** (`crates/anehta-parser/`) — Recursive-descent parser. Exports `Parser`, `ParseError`, and all AST types. Split into submodules: `stmts.rs` (statements), `exprs.rs` (expressions), `boolean.rs` (boolean expressions).
- **anehta-codegen-wasm** (`crates/anehta-codegen-wasm/`) — WASM code generator using `wasm-encoder`. Exports `WasmCodegen`, `CodegenError`. Split into submodules detailed below.
- **anehta-cli** (`crates/anehta-cli/`) — CLI entry point with `build` and `run` commands. Contains the wasmtime runtime with all 17 host functions (`env.*`) and `RuntimeState`.

### Codegen Submodules (the most complex crate)

| Module | Purpose |
|--------|---------|
| `types.rs` | `AhType` enum (Int/Float/Str/Closure/Table), `FuncCtx`, `ClosureInfo`, `TableTypeInfo`, `CodegenError` |
| `collect_strings.rs` | Phase 0: walks AST to build string pool (deduplication, data section offsets) |
| `collect_tables.rs` | Phase 0b: collects `TableTypeInfo` for compile-time field type tracking |
| `collect_closures.rs` | Phase 0c: collects `ClosureInfo`, assigns WASM table indices, captures analysis |
| `infer.rs` | Type inference for expressions (returns `AhType`) |
| `prescan.rs` | Pre-scan function bodies to pre-allocate all local variables before compilation |
| `compile_stmt.rs` | Statement compilation (var decl, assignment, if, for, timer, field/index assign) |
| `compile_expr.rs` | Expression compilation (binary ops, calls, closures, tables, field/index access) |
| `compile_func.rs` | Function compilation (user funcs, closures, `_start` entry) |
| `compile_bool.rs` | Boolean expression compilation (comparisons, logical &&/||) |

### Value Representation

All runtime values are `i64`:
- **Integers**: direct i64 value
- **Floats**: IEEE 754 f64 bits stored in i64 (`f64::to_bits() as i64`)
- **Strings**: packed `(ptr << 32) | len` — pointer and length into WASM linear memory
- **Closures**: packed `(table_idx << 32) | env_ptr` — WASM table index + environment pointer
- **Tables**: opaque i64 handle (index into host-side `RuntimeState.tables` Vec)
- **Booleans**: 0 or 1 as i64

### Compilation Strategy

1. **Prescan pass**: walks each function body to count and pre-allocate all locals (vars, temps for power/timer/closure-call/table operations) before emitting any instructions. This is necessary because WASM requires all locals declared upfront.
2. **Closures**: compiled as separate WASM functions with an i32 `env_ptr` parameter. Captured variables are stored in linear memory. Called via `call_indirect` through the WASM function table.
3. **Tables**: host-side `HashMap<String, i64>` — WASM only holds an opaque handle. All table operations go through host function imports.
4. **Table GC**: compile-time ownership tracking with zero runtime overhead. Variables are freed on reassignment and function exit. `table_free(-1)` is a no-op (sentinel for uninitialized).
5. **String pool**: all string literals are deduplicated and placed in the WASM data section at compile time. Concatenation uses a bump allocator starting at `__heap_base`.

### Key Conventions

- `codegen_err(message, span)` — helper to create `CodegenError` with source location
- `emit_float_operand` — compiles an expression then converts to f64 on the WASM stack (auto-promotes int→float)
- `FuncCtx` tracks per-function state: locals map, loop nesting, temp indices, owned tables, var types
- Span-based maps (`closure_span_map`, `table_type_span_map`) link AST nodes to their compile-time metadata by `(line, column)` key

## Language Quick Reference

- File extension: `.ah`
- Types: `int`, `float`/`f64`, `string`/`str`, `bool`, closures, tables
- Type annotation: `var x -> int` (uses `->` not `:`)
- Closures: `|x, y| => expr` or `|x| => { block }`
- Tables: `{ key: value }`, access via `.field` or `["key"]`
- Operators: `+` `-` `*` `/` `%` `^`(power) `~`(random range) `++` `--`
- Built-in calls: `print(expr)`, `input()`, `int(expr)`, `float(expr)`
- Newlines are statement separators (no semicolons needed)
- `timer { ... }` auto-measures block execution time

## VSCode Extension

Located at `vscode-anehta/`. Provides syntax highlighting for `.ah` files.
