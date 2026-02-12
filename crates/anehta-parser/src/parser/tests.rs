use super::*;
use anehta_lexer::Lexer;

fn parse_source(src: &str) -> Result<Program, ParseError> {
    let tokens = Lexer::new(src).tokenize().expect("lexer should succeed");
    let mut parser = Parser::new(tokens);
    parser.parse()
}

fn parse_ok(src: &str) -> Program {
    parse_source(src).expect("parse should succeed")
}

// ── Arithmetic expression priority ──────────────────────

#[test]
fn simple_arithmetic_add() {
    // We need to test via assignment: var x = 1 + 2
    let prog = parse_ok("var x = 1 + 2");
    assert_eq!(prog.statements.len(), 1);
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["x"]);
            assert_eq!(a.values.len(), 1);
            match &a.values[0] {
                Expr::BinaryOp { op, .. } => {
                    assert!(matches!(op, BinaryOp::Add));
                }
                _ => panic!("expected BinaryOp"),
            }
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn arithmetic_precedence_mul_over_add() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let prog = parse_ok("var x = 1 + 2 * 3");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            match &a.values[0] {
                Expr::BinaryOp { left, op, right, .. } => {
                    assert!(matches!(op, BinaryOp::Add));
                    // left should be Number(1)
                    assert!(matches!(left.as_ref(), Expr::Number(v, _) if v == "1"));
                    // right should be BinaryOp(Mul, 2, 3)
                    match right.as_ref() {
                        Expr::BinaryOp { op: inner_op, .. } => {
                            assert!(matches!(inner_op, BinaryOp::Mul));
                        }
                        _ => panic!("expected inner BinaryOp for mul"),
                    }
                }
                _ => panic!("expected BinaryOp"),
            }
        }
        _ => panic!("expected VarDecl"),
    }
}

#[test]
fn arithmetic_with_parens() {
    let prog = parse_ok("var x = (1 + 2) * 3");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            match &a.values[0] {
                Expr::BinaryOp { left, op, .. } => {
                    assert!(matches!(op, BinaryOp::Mul));
                    assert!(matches!(left.as_ref(), Expr::Grouped(_)));
                }
                _ => panic!("expected BinaryOp"),
            }
        }
        _ => panic!("expected VarDecl"),
    }
}

#[test]
fn random_operator() {
    let prog = parse_ok("var x = 1 ~ 6");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            match &a.values[0] {
                Expr::BinaryOp { op, .. } => {
                    assert!(matches!(op, BinaryOp::Rand));
                }
                _ => panic!("expected BinaryOp with Rand"),
            }
        }
        _ => panic!("expected VarDecl"),
    }
}

// ── Variable declaration ────────────────────────────────

#[test]
fn var_type_decl() {
    let prog = parse_ok("var x: int");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::TypeDecl { name, type_name, .. }) => {
            assert_eq!(name, "x");
            assert_eq!(type_name, "int");
        }
        _ => panic!("expected VarDecl TypeDecl"),
    }
}

#[test]
fn var_assignment() {
    let prog = parse_ok("var x = 42");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["x"]);
            assert_eq!(a.values.len(), 1);
            assert!(matches!(&a.values[0], Expr::Number(v, _) if v == "42"));
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn var_multi_assignment() {
    let prog = parse_ok("var a, b = 1, 2");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["a", "b"]);
            assert_eq!(a.values.len(), 2);
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

// ── Function declaration ────────────────────────────────

#[test]
fn func_decl_simple() {
    let prog = parse_ok("func add(a: int, b: int) -> int {\nreturn a + b\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.params[0].name, "a");
            assert_eq!(f.params[0].type_name, "int");
            assert_eq!(f.params[1].name, "b");
            assert_eq!(f.params[1].type_name, "int");
            assert_eq!(f.return_types, vec!["int"]);
            assert_eq!(f.body.statements.len(), 1);
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn func_decl_multi_return() {
    let prog = parse_ok("func swap(a: int, b: int) -> int, int {\nreturn b, a\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "swap");
            assert_eq!(f.return_types, vec!["int", "int"]);
            match &f.body.statements[0] {
                Statement::Return(r) => {
                    assert_eq!(r.values.len(), 2);
                }
                _ => panic!("expected Return"),
            }
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn func_decl_empty_body() {
    let prog = parse_ok("func noop() -> int {\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "noop");
            assert!(f.params.is_empty());
            assert!(f.body.statements.is_empty());
        }
        _ => panic!("expected FuncDecl"),
    }
}

// ── If / elseif / else ──────────────────────────────────

#[test]
fn if_simple() {
    let prog = parse_ok("if (x > 10) {\ny = 1\n}");
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert!(i.else_if.is_empty());
            assert!(i.else_body.is_none());
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn if_elseif_else() {
    let src = "if (x > 10) {\ny = 1\n} elseif (x > 5) {\ny = 2\n} else {\ny = 0\n}";
    let prog = parse_ok(src);
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert_eq!(i.else_if.len(), 1);
            assert!(i.else_body.is_some());
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn if_complex_boolean() {
    let src = "if ((30+4>4+4+5&&x>3)&&(30>2)) {\n}";
    let prog = parse_ok(src);
    assert!(matches!(&prog.statements[0], Statement::IfStmt(_)));
}

// ── For loop ────────────────────────────────────────────

#[test]
fn for_loop_standard() {
    let src = "for (var i = 0; i < 100; i = i + 1) {\nvar x = i\n}";
    let prog = parse_ok(src);
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert!(f.init.is_some());
            assert!(f.condition.is_some());
            assert!(f.step.is_some());
            assert_eq!(f.body.statements.len(), 1);
        }
        _ => panic!("expected ForStmt"),
    }
}

#[test]
fn for_loop_infinite() {
    let src = "for (;;) {\nbreak\n}";
    let prog = parse_ok(src);
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert!(f.init.is_none());
            assert!(f.condition.is_none());
            assert!(f.step.is_none());
            assert_eq!(f.body.statements.len(), 1);
            assert!(matches!(&f.body.statements[0], Statement::Break(_)));
        }
        _ => panic!("expected ForStmt"),
    }
}

// ── Function call ───────────────────────────────────────

#[test]
fn call_no_args() {
    let prog = parse_ok("foo()");
    match &prog.statements[0] {
        Statement::CallFunc(c) => {
            assert_eq!(c.name, "foo");
            assert!(c.args.is_empty());
        }
        _ => panic!("expected CallFunc"),
    }
}

#[test]
fn call_with_args() {
    let prog = parse_ok("print(1, 2, 3)");
    match &prog.statements[0] {
        Statement::CallFunc(c) => {
            assert_eq!(c.name, "print");
            assert_eq!(c.args.len(), 3);
        }
        _ => panic!("expected CallFunc"),
    }
}

// ── Break / Continue ────────────────────────────────────

#[test]
fn break_and_continue_in_loop() {
    let src = "for (;;) {\nbreak\ncontinue\n}";
    let prog = parse_ok(src);
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert_eq!(f.body.statements.len(), 2);
            assert!(matches!(&f.body.statements[0], Statement::Break(_)));
            assert!(matches!(&f.body.statements[1], Statement::Continue(_)));
        }
        _ => panic!("expected ForStmt"),
    }
}

// ── Go test complete code snippet ───────────────────────

#[test]
fn go_test_full_program() {
    let src = r#"
var fuck = 10

if ((30+4>4+4+5&&fuck>3)&&(30>2)){

}elseif((30+4>4+4+5&&fuck>3)&&(30>2)){
    var i = 0
}

func fucker(wokao: int) -> int,int{
    return 1,2
}
var first,second = fucker(1,2,3)

fuck = 100+2*3-4^5+0~100

for (var i = 100;i<100;i = i + 1){
    if ((30+4>4+4+5&&fuck>3)&&(30>2)){

    }elseif((30+4>4+4+5&&fuck>3+1)&&(30>2)){
        var i = 0
        for (var i = 100;i<100;i = i + 1){
            if ((30+4>4+4+5&&fuck>3)&&(30>2)){
                break
            }elseif((30+4>4+4+5&&fuck>3)&&(30>2)){
                var i = 0
            }
        }
    }
}

func wocao (wocao: int,wocao: int) -> int{

}

for (;;){

}
"#;
    let prog = parse_ok(src);
    // Should have parsed multiple top-level statements
    assert!(prog.statements.len() >= 7, "expected at least 7 top-level statements, got {}", prog.statements.len());

    // Verify statement types
    assert!(matches!(&prog.statements[0], Statement::VarDecl(_))); // var fuck = 10
    assert!(matches!(&prog.statements[1], Statement::IfStmt(_))); // if ...
    assert!(matches!(&prog.statements[2], Statement::FuncDecl(_))); // func fucker
    assert!(matches!(&prog.statements[3], Statement::VarDecl(_))); // var first, second = ...
    assert!(matches!(&prog.statements[4], Statement::Assignment(_))); // fuck = ...
    assert!(matches!(&prog.statements[5], Statement::ForStmt(_))); // for (...)
    assert!(matches!(&prog.statements[6], Statement::FuncDecl(_))); // func wocao
    assert!(matches!(&prog.statements[7], Statement::ForStmt(_))); // for (;;)
}

// ── Expression with function call ───────────────────────

#[test]
fn expression_with_call() {
    let prog = parse_ok("var x = add(1, 2) + 3");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            match &a.values[0] {
                Expr::BinaryOp { left, op, .. } => {
                    assert!(matches!(op, BinaryOp::Add));
                    assert!(matches!(left.as_ref(), Expr::CallFunc(_)));
                }
                _ => panic!("expected BinaryOp"),
            }
        }
        _ => panic!("expected VarDecl"),
    }
}

#[test]
fn increment_decrement() {
    // i++ as an expression factor (not as a for-loop step statement)
    let prog = parse_ok("var x = i++");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            match &a.values[0] {
                Expr::UnaryOp { op, operand, .. } => {
                    assert!(matches!(op, UnaryOp::Increment));
                    assert_eq!(operand, "i");
                }
                _ => panic!("expected UnaryOp"),
            }
        }
        _ => panic!("expected VarDecl"),
    }
}

#[test]
fn string_literal_in_var() {
    let prog = parse_ok(r#"var name = "Anehta""#);
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["name"]);
            match &a.values[0] {
                Expr::StringLit(s, _) => assert_eq!(s, "Anehta"),
                _ => panic!("expected StringLit"),
            }
        }
        _ => panic!("expected VarDecl"),
    }
}

#[test]
fn complex_expression() {
    // 100+2*3-4^5+0~100
    let prog = parse_ok("var x = 100+2*3-4^5+0~100");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            // The expression should parse without error
            assert_eq!(a.values.len(), 1);
        }
        _ => panic!("expected VarDecl"),
    }
}

#[test]
fn nested_for_and_if() {
    let src = r#"for (var i = 0; i < 10; i = i + 1) {
    if (i > 5) {
        break
    }
    for (var j = 0; j < i; j = j + 1) {
        continue
    }
}"#;
    let prog = parse_ok(src);
    assert_eq!(prog.statements.len(), 1);
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert_eq!(f.body.statements.len(), 2);
            assert!(matches!(&f.body.statements[0], Statement::IfStmt(_)));
            assert!(matches!(&f.body.statements[1], Statement::ForStmt(_)));
        }
        _ => panic!("expected ForStmt"),
    }
}

#[test]
fn bool_expression_eq_neq() {
    let src = "if (x == 10) {\n}";
    let prog = parse_ok(src);
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            match &i.condition {
                BooleanExpr::Comparison { op, .. } => {
                    assert!(matches!(op, ComparisonOp::Eq));
                }
                _ => panic!("expected Comparison"),
            }
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn multi_statement_program() {
    let src = r#"var health = 100
var name = "Anehta"
if (health > 80) {
    health = health - 1
}"#;
    let prog = parse_ok(src);
    assert_eq!(prog.statements.len(), 3);
}
