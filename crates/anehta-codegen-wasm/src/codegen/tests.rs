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
