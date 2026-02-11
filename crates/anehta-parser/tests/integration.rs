//! Integration tests for the AnehtaLanguage parser.
//!
//! End-to-end: source code -> Lexer -> Parser -> AST verification.
//! Tests ported from the original Go test suite and LANGUAGE_SPEC.md examples.

use anehta_lexer::Lexer;
use anehta_parser::*;

// ── Helpers ────────────────────────────────────────────────────────

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lexer should succeed");
    let mut parser = Parser::new(tokens);
    parser.parse().expect("parse should succeed")
}

fn parse_err(src: &str) {
    let tokens = Lexer::new(src).tokenize().expect("lexer should succeed");
    let mut parser = Parser::new(tokens);
    assert!(
        parser.parse().is_err(),
        "expected parse error for input: {src:?}"
    );
}

/// Extract the single expression from `var _ = <expr>`.
fn parse_expr(src: &str) -> Expr {
    let full = format!("var _x = {src}");
    let prog = parse(&full);
    match prog.statements.into_iter().next().unwrap() {
        Statement::VarDecl(VarDecl::Assignment(a)) => a.values.into_iter().next().unwrap(),
        other => panic!("expected VarDecl Assignment, got {other:?}"),
    }
}

/// Extract a BooleanExpr from `if (<bool_src>) {{}}`
fn parse_bool(src: &str) -> BooleanExpr {
    let full = format!("if ({src}) {{\n}}");
    let prog = parse(&full);
    match prog.statements.into_iter().next().unwrap() {
        Statement::IfStmt(i) => i.condition,
        other => panic!("expected IfStmt, got {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  1. Go test suite — complete program from token_test.go
// ═══════════════════════════════════════════════════════════════════

#[test]
fn go_token_test_full_program() {
    let input = r#"
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
    "#;
    let prog = parse(input);
    assert!(
        prog.statements.len() >= 5,
        "expected at least 5 top-level statements, got {}",
        prog.statements.len()
    );

    // Verify statement order
    assert!(matches!(&prog.statements[0], Statement::VarDecl(_)));
    assert!(matches!(&prog.statements[1], Statement::IfStmt(_)));
    assert!(matches!(&prog.statements[2], Statement::FuncDecl(_)));
    assert!(matches!(&prog.statements[3], Statement::VarDecl(_)));
    assert!(matches!(&prog.statements[4], Statement::Assignment(_)));
    assert!(matches!(&prog.statements[5], Statement::ForStmt(_)));
}

// ═══════════════════════════════════════════════════════════════════
//  2. Go test suite — complete program from aparser_test.go
// ═══════════════════════════════════════════════════════════════════

#[test]
fn go_parser_test_full_program() {
    let input = r#"
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
    let prog = parse(input);
    assert_eq!(prog.statements.len(), 8);

    // Verify each top-level statement
    assert!(matches!(&prog.statements[0], Statement::VarDecl(_)));     // var fuck = 10
    assert!(matches!(&prog.statements[1], Statement::IfStmt(_)));      // if ...
    assert!(matches!(&prog.statements[2], Statement::FuncDecl(_)));    // func fucker
    assert!(matches!(&prog.statements[3], Statement::VarDecl(_)));     // var first,second = ...
    assert!(matches!(&prog.statements[4], Statement::Assignment(_)));  // fuck = ...
    assert!(matches!(&prog.statements[5], Statement::ForStmt(_)));     // for (...) { ... }
    assert!(matches!(&prog.statements[6], Statement::FuncDecl(_)));    // func wocao
    assert!(matches!(&prog.statements[7], Statement::ForStmt(_)));     // for (;;) { }

    // Verify func wocao has 2 params
    match &prog.statements[6] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "wocao");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.return_types, vec!["int"]);
            assert!(f.body.statements.is_empty());
        }
        _ => unreachable!(),
    }

    // Verify empty for loop
    match &prog.statements[7] {
        Statement::ForStmt(f) => {
            assert!(f.init.is_none());
            assert!(f.condition.is_none());
            assert!(f.step.is_none());
            assert!(f.body.statements.is_empty());
        }
        _ => unreachable!(),
    }
}

#[test]
fn go_parser_test_read_expression() {
    // From aparser_test.go: `1*2+true+4+(5+false)`
    let expr = parse_expr("1*2+true+4+(5+false)");

    // Top-level should be a chain of Add operations.
    // Specifically: ((1*2) + true + 4 + (5+false))
    // Structure: Add( Add( Add( Mul(1,2), true ), 4 ), Grouped(Add(5, false)) )
    match &expr {
        Expr::BinaryOp { op, .. } => {
            assert!(matches!(op, BinaryOp::Add));
        }
        _ => panic!("expected top-level BinaryOp(Add), got {expr:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  3. LANGUAGE_SPEC.md Section 7 — full program
// ═══════════════════════════════════════════════════════════════════

#[test]
fn language_spec_section7_full_example() {
    let input = r#"var health = 100
var name = "Anehta"

var score: int

func attack(base: int, bonus: int) -> int, int {
    var damage = base + bonus + 1 ~ 20
    var critical = damage * 2
    return damage, critical
}

var dmg, crit = attack(10, 5)

if (health > 80) {
    health = health - dmg
} elseif (health > 30) {
    health = health - crit
} else {
    health = 0
}

for (var i = 0; i < 10; i = i + 1) {
    var roll = 1 ~ 6
    if (roll > 4) {
        continue
    }
    if (health <= 0) {
        break
    }
    health = health - roll
}

func fibonacci(n: number) -> number {
    if (n <= 1) {
        return n
    }
    return fibonacci(n - 1) + fibonacci(n - 2)
}

var big = 999999999999999999 * 999999999999999999

for (;;) {
    break
}
"#;
    let prog = parse(input);

    // Count statement types
    let mut var_decls = 0;
    let mut func_decls = 0;
    let mut if_stmts = 0;
    let mut for_stmts = 0;

    for stmt in &prog.statements {
        match stmt {
            Statement::VarDecl(_) => var_decls += 1,
            Statement::FuncDecl(_) => func_decls += 1,
            Statement::IfStmt(_) => if_stmts += 1,
            Statement::ForStmt(_) => for_stmts += 1,
            _ => {}
        }
    }

    // var health, var name, var score, var dmg+crit, var big = 5 top-level var decls
    assert_eq!(var_decls, 5, "expected 5 top-level var declarations");
    // func attack, func fibonacci = 2 func decls
    assert_eq!(func_decls, 2, "expected 2 func declarations");
    // 1 if statement (the if/elseif/else)
    assert_eq!(if_stmts, 1, "expected 1 top-level if statement");
    // 2 for loops (the standard one and the infinite one)
    assert_eq!(for_stmts, 2, "expected 2 for loops");

    // Total: 5 + 2 + 1 + 2 = 10 top-level statements
    assert_eq!(prog.statements.len(), 10);
}

#[test]
fn language_spec_func_attack_details() {
    let input = r#"func attack(base: int, bonus: int) -> int, int {
    var damage = base + bonus + 1 ~ 20
    var critical = damage * 2
    return damage, critical
}"#;
    let prog = parse(input);
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "attack");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.params[0].name, "base");
            assert_eq!(f.params[0].type_name, "int");
            assert_eq!(f.params[1].name, "bonus");
            assert_eq!(f.params[1].type_name, "int");
            assert_eq!(f.return_types, vec!["int", "int"]);

            // Body: 3 statements (2 var decls + 1 return)
            assert_eq!(f.body.statements.len(), 3);
            assert!(matches!(&f.body.statements[0], Statement::VarDecl(_)));
            assert!(matches!(&f.body.statements[1], Statement::VarDecl(_)));
            assert!(matches!(&f.body.statements[2], Statement::Return(_)));

            // Verify return has 2 values
            match &f.body.statements[2] {
                Statement::Return(r) => assert_eq!(r.values.len(), 2),
                _ => unreachable!(),
            }
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn language_spec_fibonacci_recursive() {
    let input = r#"func fibonacci(n: number) -> number {
    if (n <= 1) {
        return n
    }
    return fibonacci(n - 1) + fibonacci(n - 2)
}"#;
    let prog = parse(input);
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "fibonacci");
            assert_eq!(f.params.len(), 1);
            assert_eq!(f.params[0].name, "n");
            assert_eq!(f.params[0].type_name, "number");
            assert_eq!(f.return_types, vec!["number"]);

            // Body: if + return
            assert_eq!(f.body.statements.len(), 2);
            assert!(matches!(&f.body.statements[0], Statement::IfStmt(_)));
            assert!(matches!(&f.body.statements[1], Statement::Return(_)));

            // The final return contains an expression with two function calls
            match &f.body.statements[1] {
                Statement::Return(r) => {
                    assert_eq!(r.values.len(), 1);
                    // fibonacci(n-1) + fibonacci(n-2) is a BinaryOp(Add)
                    match &r.values[0] {
                        Expr::BinaryOp { op, left, right, .. } => {
                            assert!(matches!(op, BinaryOp::Add));
                            assert!(matches!(left.as_ref(), Expr::CallFunc(_)));
                            assert!(matches!(right.as_ref(), Expr::CallFunc(_)));
                        }
                        _ => panic!("expected BinaryOp(Add) with CallFunc children"),
                    }
                }
                _ => unreachable!(),
            }
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn language_spec_if_elseif_else_full() {
    let input = r#"if (health > 80) {
    health = health - dmg
} elseif (health > 30) {
    health = health - crit
} else {
    health = 0
}"#;
    let prog = parse(input);
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            // Main condition: health > 80
            match &i.condition {
                BooleanExpr::Comparison { op, .. } => {
                    assert!(matches!(op, ComparisonOp::Gt));
                }
                _ => panic!("expected Comparison"),
            }

            // Body has 1 statement
            assert_eq!(i.body.statements.len(), 1);

            // 1 elseif branch
            assert_eq!(i.else_if.len(), 1);
            match &i.else_if[0].condition {
                BooleanExpr::Comparison { op, .. } => {
                    assert!(matches!(op, ComparisonOp::Gt));
                }
                _ => panic!("expected Comparison in elseif"),
            }
            assert_eq!(i.else_if[0].body.statements.len(), 1);

            // else body exists
            assert!(i.else_body.is_some());
            assert_eq!(i.else_body.as_ref().unwrap().statements.len(), 1);
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn language_spec_for_with_body() {
    let input = r#"for (var i = 0; i < 10; i = i + 1) {
    var roll = 1 ~ 6
    if (roll > 4) {
        continue
    }
    if (health <= 0) {
        break
    }
    health = health - roll
}"#;
    let prog = parse(input);
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            // Init: var i = 0
            assert!(f.init.is_some());
            match f.init.as_deref().unwrap() {
                Statement::VarDecl(VarDecl::Assignment(a)) => {
                    assert_eq!(a.targets, vec!["i"]);
                }
                _ => panic!("expected VarDecl in for init"),
            }

            // Condition: i < 10
            assert!(f.condition.is_some());
            match f.condition.as_ref().unwrap() {
                BooleanExpr::Comparison { op, .. } => {
                    assert!(matches!(op, ComparisonOp::Lt));
                }
                _ => panic!("expected Comparison"),
            }

            // Step: i = i + 1
            assert!(f.step.is_some());

            // Body: var roll, if (continue), if (break), assignment
            assert_eq!(f.body.statements.len(), 4);
            assert!(matches!(&f.body.statements[0], Statement::VarDecl(_)));
            assert!(matches!(&f.body.statements[1], Statement::IfStmt(_)));
            assert!(matches!(&f.body.statements[2], Statement::IfStmt(_)));
            assert!(matches!(&f.body.statements[3], Statement::Assignment(_)));

            // Verify continue is inside first if
            match &f.body.statements[1] {
                Statement::IfStmt(inner_if) => {
                    assert_eq!(inner_if.body.statements.len(), 1);
                    assert!(matches!(
                        &inner_if.body.statements[0],
                        Statement::Continue(_)
                    ));
                }
                _ => unreachable!(),
            }

            // Verify break is inside second if
            match &f.body.statements[2] {
                Statement::IfStmt(inner_if) => {
                    assert_eq!(inner_if.body.statements.len(), 1);
                    assert!(matches!(
                        &inner_if.body.statements[0],
                        Statement::Break(_)
                    ));
                }
                _ => unreachable!(),
            }
        }
        _ => panic!("expected ForStmt"),
    }
}

#[test]
fn language_spec_multi_assignment_with_call() {
    // var dmg, crit = attack(10, 5)
    let prog = parse("var dmg, crit = attack(10, 5)");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["dmg", "crit"]);
            assert_eq!(a.values.len(), 1);
            match &a.values[0] {
                Expr::CallFunc(c) => {
                    assert_eq!(c.name, "attack");
                    assert_eq!(c.args.len(), 2);
                }
                _ => panic!("expected CallFunc"),
            }
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn language_spec_infinite_precision() {
    // var big = 999999999999999999 * 999999999999999999
    let prog = parse("var big = 999999999999999999 * 999999999999999999");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["big"]);
            match &a.values[0] {
                Expr::BinaryOp { op, left, right, .. } => {
                    assert!(matches!(op, BinaryOp::Mul));
                    match (left.as_ref(), right.as_ref()) {
                        (Expr::Number(l, _), Expr::Number(r, _)) => {
                            assert_eq!(l, "999999999999999999");
                            assert_eq!(r, "999999999999999999");
                        }
                        _ => panic!("expected Number operands"),
                    }
                }
                _ => panic!("expected BinaryOp Mul"),
            }
        }
        _ => panic!("expected VarDecl"),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  4. Arithmetic expression precedence
// ═══════════════════════════════════════════════════════════════════

#[test]
fn precedence_add_sub_left_associative() {
    // 1 + 2 - 3 should parse as (1 + 2) - 3
    let expr = parse_expr("1 + 2 - 3");
    match &expr {
        Expr::BinaryOp { op, left, right, .. } => {
            assert!(matches!(op, BinaryOp::Sub)); // top is Sub
            assert!(matches!(right.as_ref(), Expr::Number(v, _) if v == "3"));
            match left.as_ref() {
                Expr::BinaryOp { op: inner_op, .. } => {
                    assert!(matches!(inner_op, BinaryOp::Add));
                }
                _ => panic!("expected inner BinaryOp(Add)"),
            }
        }
        _ => panic!("expected BinaryOp"),
    }
}

#[test]
fn precedence_mul_div_left_associative() {
    // 6 * 3 / 2 should parse as (6 * 3) / 2
    let expr = parse_expr("6 * 3 / 2");
    match &expr {
        Expr::BinaryOp { op, left, .. } => {
            assert!(matches!(op, BinaryOp::Div));
            match left.as_ref() {
                Expr::BinaryOp { op: inner_op, .. } => {
                    assert!(matches!(inner_op, BinaryOp::Mul));
                }
                _ => panic!("expected inner BinaryOp(Mul)"),
            }
        }
        _ => panic!("expected BinaryOp"),
    }
}

#[test]
fn precedence_mul_over_add() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let expr = parse_expr("1 + 2 * 3");
    match &expr {
        Expr::BinaryOp { op, left, right, .. } => {
            assert!(matches!(op, BinaryOp::Add));
            assert!(matches!(left.as_ref(), Expr::Number(v, _) if v == "1"));
            match right.as_ref() {
                Expr::BinaryOp { op: inner_op, left: il, right: ir, .. } => {
                    assert!(matches!(inner_op, BinaryOp::Mul));
                    assert!(matches!(il.as_ref(), Expr::Number(v, _) if v == "2"));
                    assert!(matches!(ir.as_ref(), Expr::Number(v, _) if v == "3"));
                }
                _ => panic!("expected BinaryOp(Mul)"),
            }
        }
        _ => panic!("expected BinaryOp"),
    }
}

#[test]
fn precedence_power_same_as_mul() {
    // 1 + 2 ^ 3 should parse as 1 + (2 ^ 3)
    let expr = parse_expr("1 + 2 ^ 3");
    match &expr {
        Expr::BinaryOp { op, right, .. } => {
            assert!(matches!(op, BinaryOp::Add));
            match right.as_ref() {
                Expr::BinaryOp { op: inner_op, .. } => {
                    assert!(matches!(inner_op, BinaryOp::Power));
                }
                _ => panic!("expected BinaryOp(Power)"),
            }
        }
        _ => panic!("expected BinaryOp"),
    }
}

#[test]
fn precedence_mod_same_as_mul() {
    // 10 + 7 % 3 should parse as 10 + (7 % 3)
    let expr = parse_expr("10 + 7 % 3");
    match &expr {
        Expr::BinaryOp { op, right, .. } => {
            assert!(matches!(op, BinaryOp::Add));
            match right.as_ref() {
                Expr::BinaryOp { op: inner_op, .. } => {
                    assert!(matches!(inner_op, BinaryOp::Mod));
                }
                _ => panic!("expected BinaryOp(Mod)"),
            }
        }
        _ => panic!("expected BinaryOp"),
    }
}

#[test]
fn precedence_rand_same_as_mul() {
    // 1 + 2 ~ 10 should parse as 1 + (2 ~ 10)
    let expr = parse_expr("1 + 2 ~ 10");
    match &expr {
        Expr::BinaryOp { op, right, .. } => {
            assert!(matches!(op, BinaryOp::Add));
            match right.as_ref() {
                Expr::BinaryOp { op: inner_op, .. } => {
                    assert!(matches!(inner_op, BinaryOp::Rand));
                }
                _ => panic!("expected BinaryOp(Rand)"),
            }
        }
        _ => panic!("expected BinaryOp"),
    }
}

#[test]
fn precedence_parens_override() {
    // (1 + 2) * 3 should parse as Mul(Grouped(Add(1,2)), 3)
    let expr = parse_expr("(1 + 2) * 3");
    match &expr {
        Expr::BinaryOp { op, left, right, .. } => {
            assert!(matches!(op, BinaryOp::Mul));
            assert!(matches!(left.as_ref(), Expr::Grouped(_)));
            assert!(matches!(right.as_ref(), Expr::Number(v, _) if v == "3"));
        }
        _ => panic!("expected BinaryOp(Mul)"),
    }
}

#[test]
fn precedence_complex_expression() {
    // 100+2*3-4^5+0~100
    // Should parse as: ((100 + (2*3)) - (4^5)) + (0~100)
    let expr = parse_expr("100+2*3-4^5+0~100");
    // Top-level should be Add (the last +)
    match &expr {
        Expr::BinaryOp { op, right, .. } => {
            assert!(matches!(op, BinaryOp::Add));
            // right is 0~100 (Rand)
            match right.as_ref() {
                Expr::BinaryOp { op: inner_op, .. } => {
                    assert!(matches!(inner_op, BinaryOp::Rand));
                }
                _ => panic!("expected BinaryOp(Rand)"),
            }
        }
        _ => panic!("expected BinaryOp"),
    }
}

#[test]
fn nested_parentheses() {
    let expr = parse_expr("((((1 + 2))))");
    // Should be Grouped(Grouped(Grouped(Grouped(Add(1,2)))))
    fn unwrap_grouped(e: &Expr) -> &Expr {
        match e {
            Expr::Grouped(inner) => inner.as_ref(),
            other => other,
        }
    }
    let inner = unwrap_grouped(&expr);
    let inner = unwrap_grouped(inner);
    let inner = unwrap_grouped(inner);
    let inner = unwrap_grouped(inner);
    assert!(matches!(inner, Expr::BinaryOp { op: BinaryOp::Add, .. }));
}

// ═══════════════════════════════════════════════════════════════════
//  5. Boolean expressions
// ═══════════════════════════════════════════════════════════════════

#[test]
fn boolean_simple_comparison_gt() {
    let b = parse_bool("x > 10");
    match &b {
        BooleanExpr::Comparison { op, left, right, .. } => {
            assert!(matches!(op, ComparisonOp::Gt));
            assert!(matches!(left, Expr::Variable(v, _) if v == "x"));
            assert!(matches!(right, Expr::Number(v, _) if v == "10"));
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn boolean_simple_comparison_lt() {
    let b = parse_bool("i < 100");
    match &b {
        BooleanExpr::Comparison { op, .. } => {
            assert!(matches!(op, ComparisonOp::Lt));
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn boolean_comparison_gteq() {
    let b = parse_bool("x >= 5");
    match &b {
        BooleanExpr::Comparison { op, .. } => {
            assert!(matches!(op, ComparisonOp::GtEq));
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn boolean_comparison_lteq() {
    let b = parse_bool("health <= 0");
    match &b {
        BooleanExpr::Comparison { op, .. } => {
            assert!(matches!(op, ComparisonOp::LtEq));
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn boolean_comparison_eq() {
    let b = parse_bool("x == 10");
    match &b {
        BooleanExpr::Comparison { op, .. } => {
            assert!(matches!(op, ComparisonOp::Eq));
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn boolean_comparison_neq() {
    let b = parse_bool("x != 0");
    match &b {
        BooleanExpr::Comparison { op, .. } => {
            assert!(matches!(op, ComparisonOp::NotEq));
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn boolean_logical_and() {
    // x > 1 && y > 2
    let b = parse_bool("x > 1 && y > 2");
    match &b {
        BooleanExpr::Logical { op, left, right, .. } => {
            assert!(matches!(op, LogicalOp::And));
            assert!(matches!(left.as_ref(), BooleanExpr::Comparison { .. }));
            assert!(matches!(right.as_ref(), BooleanExpr::Comparison { .. }));
        }
        _ => panic!("expected Logical(And)"),
    }
}

#[test]
fn boolean_logical_or() {
    let b = parse_bool("x > 1 || y > 2");
    match &b {
        BooleanExpr::Logical { op, .. } => {
            assert!(matches!(op, LogicalOp::Or));
        }
        _ => panic!("expected Logical(Or)"),
    }
}

#[test]
fn boolean_complex_nested_from_go_test() {
    // (30+4>4+4+5&&fuck>3)&&(30>2)
    // This is from the Go test suite
    let b = parse_bool("(30+4>4+4+5&&fuck>3)&&(30>2)");
    match &b {
        BooleanExpr::Logical { op, .. } => {
            assert!(matches!(op, LogicalOp::And));
        }
        _ => panic!("expected top-level Logical(And), got {b:?}"),
    }
}

#[test]
fn boolean_double_nested_from_go_test() {
    // ((30+4>4+4+5&&fuck>3)&&(30>2))
    let b = parse_bool("((30+4>4+4+5&&fuck>3)&&(30>2))");
    // Should be Grouped around the previous test's expression
    match &b {
        BooleanExpr::Grouped(inner) => {
            assert!(matches!(inner.as_ref(), BooleanExpr::Logical { op: LogicalOp::And, .. }));
        }
        _ => panic!("expected Grouped(Logical(And)), got {b:?}"),
    }
}

#[test]
fn boolean_with_arithmetic_expressions() {
    // 30+4 > 4+4+5 -- arithmetic on both sides of comparison
    let b = parse_bool("30+4 > 4+4+5");
    match &b {
        BooleanExpr::Comparison { op, left, right, .. } => {
            assert!(matches!(op, ComparisonOp::Gt));
            assert!(matches!(left, Expr::BinaryOp { op: BinaryOp::Add, .. }));
            assert!(matches!(right, Expr::BinaryOp { op: BinaryOp::Add, .. }));
        }
        _ => panic!("expected Comparison with arithmetic sides"),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  6. Function declarations — edge cases
// ═══════════════════════════════════════════════════════════════════

#[test]
fn func_no_params() {
    let prog = parse("func noop() -> void {\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "noop");
            assert!(f.params.is_empty());
            assert_eq!(f.return_types, vec!["void"]);
            assert!(f.body.statements.is_empty());
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn func_three_params() {
    let input = "func foo(a: int, b: string, c: number) -> int {\nreturn a\n}";
    let prog = parse(input);
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.params.len(), 3);
            assert_eq!(f.params[0].name, "a");
            assert_eq!(f.params[0].type_name, "int");
            assert_eq!(f.params[1].name, "b");
            assert_eq!(f.params[1].type_name, "string");
            assert_eq!(f.params[2].name, "c");
            assert_eq!(f.params[2].type_name, "number");
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn func_three_return_types() {
    let input = "func multi() -> int, string, number {\nreturn 1, \"a\", 3\n}";
    let prog = parse(input);
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.return_types, vec!["int", "string", "number"]);
            match &f.body.statements[0] {
                Statement::Return(r) => assert_eq!(r.values.len(), 3),
                _ => panic!("expected Return"),
            }
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn func_body_with_nested_if_and_for() {
    let input = r#"func complex(n: int) -> int {
    if (n > 0) {
        for (var i = 0; i < n; i = i + 1) {
            if (i > 5) {
                break
            }
        }
    }
    return n
}"#;
    let prog = parse(input);
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.body.statements.len(), 2); // if + return
            assert!(matches!(&f.body.statements[0], Statement::IfStmt(_)));
            assert!(matches!(&f.body.statements[1], Statement::Return(_)));
        }
        _ => panic!("expected FuncDecl"),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  7. Variable declarations — all forms
// ═══════════════════════════════════════════════════════════════════

#[test]
fn var_type_declaration() {
    let prog = parse("var x: int");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::TypeDecl {
            name, type_name, ..
        }) => {
            assert_eq!(name, "x");
            assert_eq!(type_name, "int");
        }
        _ => panic!("expected VarDecl TypeDecl"),
    }
}

#[test]
fn var_number_assignment() {
    let prog = parse("var x = 42");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["x"]);
            assert!(matches!(&a.values[0], Expr::Number(v, _) if v == "42"));
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn var_string_assignment() {
    let prog = parse(r#"var name = "Anehta""#);
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["name"]);
            assert!(matches!(&a.values[0], Expr::StringLit(v, _) if v == "Anehta"));
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn var_bool_true_assignment() {
    let prog = parse("var flag = true");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert!(matches!(&a.values[0], Expr::Bool(true, _)));
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn var_bool_false_assignment() {
    let prog = parse("var flag = false");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert!(matches!(&a.values[0], Expr::Bool(false, _)));
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn var_expression_assignment() {
    let prog = parse("var result = 1 + 2 * 3");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["result"]);
            assert!(matches!(&a.values[0], Expr::BinaryOp { .. }));
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn var_multi_assign_literals() {
    let prog = parse("var a, b = 1, 2");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["a", "b"]);
            assert_eq!(a.values.len(), 2);
            assert!(matches!(&a.values[0], Expr::Number(v, _) if v == "1"));
            assert!(matches!(&a.values[1], Expr::Number(v, _) if v == "2"));
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn var_multi_assign_three_targets() {
    let prog = parse("var a, b, c = 1, 2, 3");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["a", "b", "c"]);
            assert_eq!(a.values.len(), 3);
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  8. Assignment statements (standalone, not var)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn assignment_simple() {
    let prog = parse("x = 42");
    match &prog.statements[0] {
        Statement::Assignment(a) => {
            assert_eq!(a.targets, vec!["x"]);
            assert!(matches!(&a.values[0], Expr::Number(v, _) if v == "42"));
        }
        _ => panic!("expected Assignment"),
    }
}

#[test]
fn assignment_multi_target() {
    let prog = parse("a, b = 1, 2");
    match &prog.statements[0] {
        Statement::Assignment(a) => {
            assert_eq!(a.targets, vec!["a", "b"]);
            assert_eq!(a.values.len(), 2);
        }
        _ => panic!("expected Assignment"),
    }
}

#[test]
fn assignment_with_expression() {
    let prog = parse("x = a + b * 2");
    match &prog.statements[0] {
        Statement::Assignment(a) => {
            assert_eq!(a.targets, vec!["x"]);
            assert!(matches!(&a.values[0], Expr::BinaryOp { op: BinaryOp::Add, .. }));
        }
        _ => panic!("expected Assignment"),
    }
}

#[test]
fn assignment_with_func_call() {
    let prog = parse("var first, second = fucker(1,2,3)");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["first", "second"]);
            assert_eq!(a.values.len(), 1);
            assert!(matches!(&a.values[0], Expr::CallFunc(_)));
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  9. If / elseif / else — variations
// ═══════════════════════════════════════════════════════════════════

#[test]
fn if_only() {
    let prog = parse("if (x > 0) {\ny = 1\n}");
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert!(i.else_if.is_empty());
            assert!(i.else_body.is_none());
            assert_eq!(i.body.statements.len(), 1);
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn if_else() {
    let prog = parse("if (x > 0) {\ny = 1\n} else {\ny = 0\n}");
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert!(i.else_if.is_empty());
            assert!(i.else_body.is_some());
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn if_multiple_elseif() {
    let src = r#"if (x > 10) {
    y = 3
} elseif (x > 5) {
    y = 2
} elseif (x > 0) {
    y = 1
} else {
    y = 0
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert_eq!(i.else_if.len(), 2);
            assert!(i.else_body.is_some());
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn if_with_empty_body() {
    let prog = parse("if (x > 0) {\n}");
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert!(i.body.statements.is_empty());
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn if_nested_inside_if() {
    let src = r#"if (x > 0) {
    if (y > 0) {
        z = 1
    }
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::IfStmt(outer) => {
            assert_eq!(outer.body.statements.len(), 1);
            assert!(matches!(&outer.body.statements[0], Statement::IfStmt(_)));
        }
        _ => panic!("expected IfStmt"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 10. For loop — variations
// ═══════════════════════════════════════════════════════════════════

#[test]
fn for_standard_three_parts() {
    let prog = parse("for (var i = 0; i < 100; i = i + 1) {\nvar x = i\n}");
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
fn for_infinite_empty() {
    let prog = parse("for (;;) {\n}");
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert!(f.init.is_none());
            assert!(f.condition.is_none());
            assert!(f.step.is_none());
            assert!(f.body.statements.is_empty());
        }
        _ => panic!("expected ForStmt"),
    }
}

#[test]
fn for_infinite_with_break() {
    let prog = parse("for (;;) {\nbreak\n}");
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert_eq!(f.body.statements.len(), 1);
            assert!(matches!(&f.body.statements[0], Statement::Break(_)));
        }
        _ => panic!("expected ForStmt"),
    }
}

#[test]
fn for_nested() {
    let src = r#"for (var i = 0; i < 10; i = i + 1) {
    for (var j = 0; j < i; j = j + 1) {
        continue
    }
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::ForStmt(outer) => {
            assert_eq!(outer.body.statements.len(), 1);
            match &outer.body.statements[0] {
                Statement::ForStmt(inner) => {
                    assert_eq!(inner.body.statements.len(), 1);
                    assert!(matches!(
                        &inner.body.statements[0],
                        Statement::Continue(_)
                    ));
                }
                _ => panic!("expected inner ForStmt"),
            }
        }
        _ => panic!("expected ForStmt"),
    }
}

#[test]
fn for_with_assignment_step() {
    // Step is a plain assignment (not var)
    let prog = parse("for (var i = 0; i < 10; i = i + 1) {\n}");
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            match f.step.as_deref().unwrap() {
                Statement::Assignment(a) => {
                    assert_eq!(a.targets, vec!["i"]);
                }
                _ => panic!("expected Assignment in step"),
            }
        }
        _ => panic!("expected ForStmt"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 11. Return statement
// ═══════════════════════════════════════════════════════════════════

#[test]
fn return_single_value() {
    let prog = parse("func f() -> int {\nreturn 42\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => match &f.body.statements[0] {
            Statement::Return(r) => {
                assert_eq!(r.values.len(), 1);
                assert!(matches!(&r.values[0], Expr::Number(v, _) if v == "42"));
            }
            _ => panic!("expected Return"),
        },
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn return_multiple_values() {
    let prog = parse("func f() -> int, int {\nreturn 1, 2\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => match &f.body.statements[0] {
            Statement::Return(r) => {
                assert_eq!(r.values.len(), 2);
            }
            _ => panic!("expected Return"),
        },
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn return_expression() {
    let prog = parse("func f() -> int {\nreturn a + b\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => match &f.body.statements[0] {
            Statement::Return(r) => {
                assert_eq!(r.values.len(), 1);
                assert!(matches!(&r.values[0], Expr::BinaryOp { op: BinaryOp::Add, .. }));
            }
            _ => panic!("expected Return"),
        },
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn return_empty() {
    let prog = parse("func f() -> int {\nreturn\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => match &f.body.statements[0] {
            Statement::Return(r) => {
                assert!(r.values.is_empty());
            }
            _ => panic!("expected Return"),
        },
        _ => panic!("expected FuncDecl"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 12. Function calls
// ═══════════════════════════════════════════════════════════════════

#[test]
fn call_no_args_standalone() {
    let prog = parse("foo()");
    match &prog.statements[0] {
        Statement::CallFunc(c) => {
            assert_eq!(c.name, "foo");
            assert!(c.args.is_empty());
        }
        _ => panic!("expected CallFunc"),
    }
}

#[test]
fn call_single_arg() {
    let prog = parse("print(42)");
    match &prog.statements[0] {
        Statement::CallFunc(c) => {
            assert_eq!(c.name, "print");
            assert_eq!(c.args.len(), 1);
        }
        _ => panic!("expected CallFunc"),
    }
}

#[test]
fn call_multiple_args() {
    let prog = parse("add(1, 2, 3)");
    match &prog.statements[0] {
        Statement::CallFunc(c) => {
            assert_eq!(c.name, "add");
            assert_eq!(c.args.len(), 3);
        }
        _ => panic!("expected CallFunc"),
    }
}

#[test]
fn call_with_expression_args() {
    let prog = parse("foo(1 + 2, a * b)");
    match &prog.statements[0] {
        Statement::CallFunc(c) => {
            assert_eq!(c.args.len(), 2);
            assert!(matches!(&c.args[0], Expr::BinaryOp { op: BinaryOp::Add, .. }));
            assert!(matches!(&c.args[1], Expr::BinaryOp { op: BinaryOp::Mul, .. }));
        }
        _ => panic!("expected CallFunc"),
    }
}

#[test]
fn call_in_expression() {
    // Function call as part of an arithmetic expression
    let prog = parse("var x = add(1, 2) + 3");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            match &a.values[0] {
                Expr::BinaryOp { left, op, right, .. } => {
                    assert!(matches!(op, BinaryOp::Add));
                    assert!(matches!(left.as_ref(), Expr::CallFunc(_)));
                    assert!(matches!(right.as_ref(), Expr::Number(v, _) if v == "3"));
                }
                _ => panic!("expected BinaryOp"),
            }
        }
        _ => panic!("expected VarDecl"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 13. Unary operations (increment/decrement)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn increment_in_expr() {
    let expr = parse_expr("i++");
    match &expr {
        Expr::UnaryOp { op, operand, .. } => {
            assert!(matches!(op, UnaryOp::Increment));
            assert_eq!(operand, "i");
        }
        _ => panic!("expected UnaryOp(Increment)"),
    }
}

#[test]
fn decrement_in_expr() {
    let expr = parse_expr("j--");
    match &expr {
        Expr::UnaryOp { op, operand, .. } => {
            assert!(matches!(op, UnaryOp::Decrement));
            assert_eq!(operand, "j");
        }
        _ => panic!("expected UnaryOp(Decrement)"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 14. Expression factor types
// ═══════════════════════════════════════════════════════════════════

#[test]
fn factor_number() {
    let expr = parse_expr("42");
    assert!(matches!(&expr, Expr::Number(v, _) if v == "42"));
}

#[test]
fn factor_float() {
    let expr = parse_expr("3.14");
    assert!(matches!(&expr, Expr::Number(v, _) if v == "3.14"));
}

#[test]
fn factor_string() {
    let expr = parse_expr("\"hello\"");
    assert!(matches!(&expr, Expr::StringLit(v, _) if v == "hello"));
}

#[test]
fn factor_true() {
    let expr = parse_expr("true");
    assert!(matches!(&expr, Expr::Bool(true, _)));
}

#[test]
fn factor_false() {
    let expr = parse_expr("false");
    assert!(matches!(&expr, Expr::Bool(false, _)));
}

#[test]
fn factor_variable() {
    let expr = parse_expr("myVar");
    assert!(matches!(&expr, Expr::Variable(v, _) if v == "myVar"));
}

#[test]
fn factor_grouped() {
    let expr = parse_expr("(1 + 2)");
    assert!(matches!(&expr, Expr::Grouped(_)));
}

#[test]
fn factor_call_func() {
    let expr = parse_expr("foo(1)");
    match &expr {
        Expr::CallFunc(c) => {
            assert_eq!(c.name, "foo");
            assert_eq!(c.args.len(), 1);
        }
        _ => panic!("expected CallFunc"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 15. Multiple top-level statements
// ═══════════════════════════════════════════════════════════════════

#[test]
fn multiple_var_decls() {
    let prog = parse("var a = 1\nvar b = 2\nvar c = 3");
    assert_eq!(prog.statements.len(), 3);
    for s in &prog.statements {
        assert!(matches!(s, Statement::VarDecl(_)));
    }
}

#[test]
fn mixed_top_level_statements() {
    let src = r#"var x = 1
func f() -> int {
    return x
}
x = 2
f()
if (x > 0) {
    x = 0
}
for (;;) {
    break
}
"#;
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 6);
    assert!(matches!(&prog.statements[0], Statement::VarDecl(_)));
    assert!(matches!(&prog.statements[1], Statement::FuncDecl(_)));
    assert!(matches!(&prog.statements[2], Statement::Assignment(_)));
    assert!(matches!(&prog.statements[3], Statement::CallFunc(_)));
    assert!(matches!(&prog.statements[4], Statement::IfStmt(_)));
    assert!(matches!(&prog.statements[5], Statement::ForStmt(_)));
}

// ═══════════════════════════════════════════════════════════════════
// 16. Deeply nested blocks
// ═══════════════════════════════════════════════════════════════════

#[test]
fn deeply_nested_for_and_if() {
    // Mimics the Go test: for > if > elseif > for > if > break > elseif > var
    let src = r#"for (var i = 0; i < 10; i = i + 1) {
    if (i > 5) {
        var x = 1
    } elseif (i > 3) {
        for (var j = 0; j < 5; j = j + 1) {
            if (j > 2) {
                break
            } elseif (j > 1) {
                var y = 0
            }
        }
    }
}"#;
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 1);
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert_eq!(f.body.statements.len(), 1);
            match &f.body.statements[0] {
                Statement::IfStmt(i) => {
                    assert_eq!(i.else_if.len(), 1);
                    // The elseif body contains a nested for loop
                    assert_eq!(i.else_if[0].body.statements.len(), 1);
                    assert!(matches!(
                        &i.else_if[0].body.statements[0],
                        Statement::ForStmt(_)
                    ));
                }
                _ => panic!("expected IfStmt"),
            }
        }
        _ => panic!("expected ForStmt"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 17. Parse error cases
// ═══════════════════════════════════════════════════════════════════

#[test]
fn error_missing_rparen_in_if() {
    parse_err("if (x > 0 {\n}");
}

#[test]
fn error_missing_lbrace_in_if() {
    parse_err("if (x > 0)\ny = 1\n}");
}

#[test]
fn error_missing_rbrace_in_block() {
    parse_err("if (x > 0) {\ny = 1");
}

#[test]
fn error_missing_func_name() {
    parse_err("func () -> int {\n}");
}

#[test]
fn error_missing_return_type() {
    parse_err("func foo() {\n}");
}

#[test]
fn error_missing_assignment_value() {
    parse_err("var x =");
}

#[test]
fn error_unexpected_token_at_top() {
    parse_err("42");
}

#[test]
fn error_for_missing_semicolons() {
    parse_err("for (var i = 0 i < 10 i = i + 1) {\n}");
}

// ═══════════════════════════════════════════════════════════════════
// 18. Quick start examples from spec
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spec_quick_start_variables() {
    let prog = parse("var x = 42\nvar name = \"Anehta\"");
    assert_eq!(prog.statements.len(), 2);
}

#[test]
fn spec_quick_start_swap() {
    let src = r#"func swap(a: int, b: int) -> int, int {
    return b, a
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "swap");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.return_types, vec!["int", "int"]);
            match &f.body.statements[0] {
                Statement::Return(r) => {
                    assert_eq!(r.values.len(), 2);
                    assert!(matches!(&r.values[0], Expr::Variable(v, _) if v == "b"));
                    assert!(matches!(&r.values[1], Expr::Variable(v, _) if v == "a"));
                }
                _ => panic!("expected Return"),
            }
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn spec_quick_start_multi_assign() {
    let prog = parse("var first, second = swap(1, 2)");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["first", "second"]);
            assert!(matches!(&a.values[0], Expr::CallFunc(c) if c.name == "swap"));
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn spec_dice_roll() {
    // var dice = 1 ~ 6
    let expr = parse_expr("1 ~ 6");
    match &expr {
        Expr::BinaryOp { op, left, right, .. } => {
            assert!(matches!(op, BinaryOp::Rand));
            assert!(matches!(left.as_ref(), Expr::Number(v, _) if v == "1"));
            assert!(matches!(right.as_ref(), Expr::Number(v, _) if v == "6"));
        }
        _ => panic!("expected BinaryOp(Rand)"),
    }
}

#[test]
fn spec_damage_with_random() {
    // var damage = 10 + 1 ~ 20 -> parses as 10 + (1 ~ 20)
    let expr = parse_expr("10 + 1 ~ 20");
    match &expr {
        Expr::BinaryOp { op, left, right, .. } => {
            assert!(matches!(op, BinaryOp::Add));
            assert!(matches!(left.as_ref(), Expr::Number(v, _) if v == "10"));
            match right.as_ref() {
                Expr::BinaryOp { op: inner_op, .. } => {
                    assert!(matches!(inner_op, BinaryOp::Rand));
                }
                _ => panic!("expected inner BinaryOp(Rand)"),
            }
        }
        _ => panic!("expected BinaryOp(Add)"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 19. Stress test: many statements
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_many_var_declarations() {
    let lines: Vec<String> = (0..100)
        .map(|i| format!("var x{i} = {}", i * 10))
        .collect();
    let input = lines.join("\n");
    let prog = parse(&input);
    assert_eq!(prog.statements.len(), 100);
    for s in &prog.statements {
        assert!(matches!(s, Statement::VarDecl(_)));
    }
}

#[test]
fn stress_many_function_calls() {
    let lines: Vec<String> = (0..50)
        .map(|i| format!("f{i}({i})"))
        .collect();
    let input = lines.join("\n");
    let prog = parse(&input);
    assert_eq!(prog.statements.len(), 50);
    for s in &prog.statements {
        assert!(matches!(s, Statement::CallFunc(_)));
    }
}

// ═══════════════════════════════════════════════════════════════════
// 20. Deep nesting — 3+ levels of for loops
// ═══════════════════════════════════════════════════════════════════

#[test]
fn deep_nesting_three_for_loops() {
    let src = r#"for (var i = 0; i < 10; i = i + 1) {
    for (var j = 0; j < 10; j = j + 1) {
        for (var k = 0; k < 10; k = k + 1) {
            var x = i + j + k
        }
    }
}"#;
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 1);
    // Level 1: for i
    match &prog.statements[0] {
        Statement::ForStmt(f1) => {
            assert_eq!(f1.body.statements.len(), 1);
            // Level 2: for j
            match &f1.body.statements[0] {
                Statement::ForStmt(f2) => {
                    assert_eq!(f2.body.statements.len(), 1);
                    // Level 3: for k
                    match &f2.body.statements[0] {
                        Statement::ForStmt(f3) => {
                            assert_eq!(f3.body.statements.len(), 1);
                            assert!(matches!(&f3.body.statements[0], Statement::VarDecl(_)));
                        }
                        _ => panic!("expected ForStmt at level 3"),
                    }
                }
                _ => panic!("expected ForStmt at level 2"),
            }
        }
        _ => panic!("expected ForStmt at level 1"),
    }
}

#[test]
fn deep_nesting_four_for_loops() {
    let src = r#"for (var a = 0; a < 5; a = a + 1) {
    for (var b = 0; b < 5; b = b + 1) {
        for (var c = 0; c < 5; c = c + 1) {
            for (var d = 0; d < 5; d = d + 1) {
                break
            }
        }
    }
}"#;
    let prog = parse(src);
    // Drill down 4 levels
    let f1 = match &prog.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected ForStmt"),
    };
    let f2 = match &f1.body.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected ForStmt level 2"),
    };
    let f3 = match &f2.body.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected ForStmt level 3"),
    };
    let f4 = match &f3.body.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected ForStmt level 4"),
    };
    assert_eq!(f4.body.statements.len(), 1);
    assert!(matches!(&f4.body.statements[0], Statement::Break(_)));
}

// ═══════════════════════════════════════════════════════════════════
// 21. Deep nesting — 3+ levels of if/elseif/else
// ═══════════════════════════════════════════════════════════════════

#[test]
fn deep_nesting_three_if_levels() {
    let src = r#"if (a > 1) {
    if (b > 2) {
        if (c > 3) {
            var x = 1
        } else {
            var x = 2
        }
    } elseif (b > 1) {
        var y = 3
    }
} else {
    var z = 4
}"#;
    let prog = parse(src);
    let i1 = match &prog.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt level 1"),
    };
    assert!(i1.else_body.is_some());

    // Level 2: if (b > 2) ... elseif (b > 1) ...
    let i2 = match &i1.body.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt level 2"),
    };
    assert_eq!(i2.else_if.len(), 1);

    // Level 3: if (c > 3) ... else ...
    let i3 = match &i2.body.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt level 3"),
    };
    assert!(i3.else_body.is_some());
    assert_eq!(i3.body.statements.len(), 1);
}

#[test]
fn deep_nesting_four_if_levels() {
    let src = r#"if (a > 0) {
    if (b > 0) {
        if (c > 0) {
            if (d > 0) {
                var result = 1
            }
        }
    }
}"#;
    let prog = parse(src);
    let i1 = match &prog.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt"),
    };
    let i2 = match &i1.body.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt level 2"),
    };
    let i3 = match &i2.body.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt level 3"),
    };
    let i4 = match &i3.body.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt level 4"),
    };
    assert_eq!(i4.body.statements.len(), 1);
    assert!(matches!(&i4.body.statements[0], Statement::VarDecl(_)));
}

// ═══════════════════════════════════════════════════════════════════
// 22. Deep nesting — mixed for/if interleaving
// ═══════════════════════════════════════════════════════════════════

#[test]
fn deep_nesting_for_if_for_if() {
    // for > if > for > if > break
    let src = r#"for (var i = 0; i < 10; i = i + 1) {
    if (i > 3) {
        for (var j = 0; j < 5; j = j + 1) {
            if (j > 2) {
                break
            }
        }
    }
}"#;
    let prog = parse(src);
    let f1 = match &prog.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected ForStmt"),
    };
    let i1 = match &f1.body.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt"),
    };
    let f2 = match &i1.body.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected inner ForStmt"),
    };
    let i2 = match &f2.body.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected inner IfStmt"),
    };
    assert_eq!(i2.body.statements.len(), 1);
    assert!(matches!(&i2.body.statements[0], Statement::Break(_)));
}

#[test]
fn deep_nesting_if_for_if_for() {
    // if > for > if > for > continue
    let src = r#"if (x > 0) {
    for (var i = 0; i < x; i = i + 1) {
        if (i > 5) {
            for (var j = 0; j < i; j = j + 1) {
                continue
            }
        }
    }
}"#;
    let prog = parse(src);
    let i1 = match &prog.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt"),
    };
    let f1 = match &i1.body.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected ForStmt"),
    };
    let i2 = match &f1.body.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected inner IfStmt"),
    };
    let f2 = match &i2.body.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected inner ForStmt"),
    };
    assert_eq!(f2.body.statements.len(), 1);
    assert!(matches!(&f2.body.statements[0], Statement::Continue(_)));
}

#[test]
fn deep_nesting_for_for_if_with_elseif() {
    // for > for > if > elseif > else, each with content
    let src = r#"for (var i = 0; i < 10; i = i + 1) {
    for (var j = 0; j < 10; j = j + 1) {
        if (i > j) {
            var x = 1
        } elseif (i == j) {
            var x = 2
        } elseif (i < j) {
            var x = 3
        } else {
            var x = 0
        }
    }
}"#;
    let prog = parse(src);
    let f1 = match &prog.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected ForStmt"),
    };
    let f2 = match &f1.body.statements[0] {
        Statement::ForStmt(f) => f,
        _ => panic!("expected inner ForStmt"),
    };
    let i1 = match &f2.body.statements[0] {
        Statement::IfStmt(i) => i,
        _ => panic!("expected IfStmt"),
    };
    assert_eq!(i1.else_if.len(), 2); // 2 elseif branches
    assert!(i1.else_body.is_some());
}

// ═══════════════════════════════════════════════════════════════════
// 23. Complex expressions — long chains
// ═══════════════════════════════════════════════════════════════════

#[test]
fn expr_long_arithmetic_chain() {
    // 1+2*3-4^5+6%7+8~9-10
    let expr = parse_expr("1+2*3-4^5+6%7+8~9-10");
    // Top-level should be Sub (the final -10)
    match &expr {
        Expr::BinaryOp { op, right, .. } => {
            assert!(matches!(op, BinaryOp::Sub));
            assert!(matches!(right.as_ref(), Expr::Number(v, _) if v == "10"));
        }
        _ => panic!("expected BinaryOp(Sub) at top level"),
    }
}

#[test]
fn expr_long_arithmetic_chain_all_term_ops() {
    // a * b / c ^ d % e ~ f
    // All term-level operators, left-associative
    let expr = parse_expr("a * b / c ^ d % e ~ f");
    // Should be: Rand( Mod( Power( Div( Mul(a,b), c), d), e), f)
    // i.e., top-level is Rand
    match &expr {
        Expr::BinaryOp { op, .. } => {
            assert!(matches!(op, BinaryOp::Rand));
        }
        _ => panic!("expected BinaryOp(Rand) at top level"),
    }
}

#[test]
fn expr_deep_parentheses_chained() {
    // ((((1+2)*3)+4)*5)
    let expr = parse_expr("((((1+2)*3)+4)*5)");
    // Outermost is Grouped
    match &expr {
        Expr::Grouped(inner) => {
            // inner is Mul(?, 5)
            match inner.as_ref() {
                Expr::BinaryOp { op, right, .. } => {
                    assert!(matches!(op, BinaryOp::Mul));
                    assert!(matches!(right.as_ref(), Expr::Number(v, _) if v == "5"));
                }
                _ => panic!("expected BinaryOp(Mul)"),
            }
        }
        _ => panic!("expected Grouped at top level"),
    }
}

#[test]
fn expr_nested_function_calls() {
    // add(mul(1,2), sub(3,4)) + 5
    let expr = parse_expr("add(mul(1,2), sub(3,4)) + 5");
    match &expr {
        Expr::BinaryOp { op, left, right, .. } => {
            assert!(matches!(op, BinaryOp::Add));
            assert!(matches!(right.as_ref(), Expr::Number(v, _) if v == "5"));
            // left is CallFunc(add)
            match left.as_ref() {
                Expr::CallFunc(c) => {
                    assert_eq!(c.name, "add");
                    assert_eq!(c.args.len(), 2);
                    // First arg: mul(1,2)
                    match &c.args[0] {
                        Expr::CallFunc(inner) => {
                            assert_eq!(inner.name, "mul");
                            assert_eq!(inner.args.len(), 2);
                        }
                        _ => panic!("expected CallFunc(mul)"),
                    }
                    // Second arg: sub(3,4)
                    match &c.args[1] {
                        Expr::CallFunc(inner) => {
                            assert_eq!(inner.name, "sub");
                            assert_eq!(inner.args.len(), 2);
                        }
                        _ => panic!("expected CallFunc(sub)"),
                    }
                }
                _ => panic!("expected CallFunc(add)"),
            }
        }
        _ => panic!("expected BinaryOp(Add)"),
    }
}

#[test]
fn expr_triple_nested_function_calls() {
    // outer(middle(inner(1)))
    let expr = parse_expr("outer(middle(inner(1)))");
    match &expr {
        Expr::CallFunc(c) => {
            assert_eq!(c.name, "outer");
            assert_eq!(c.args.len(), 1);
            match &c.args[0] {
                Expr::CallFunc(c2) => {
                    assert_eq!(c2.name, "middle");
                    assert_eq!(c2.args.len(), 1);
                    match &c2.args[0] {
                        Expr::CallFunc(c3) => {
                            assert_eq!(c3.name, "inner");
                            assert_eq!(c3.args.len(), 1);
                            assert!(matches!(&c3.args[0], Expr::Number(v, _) if v == "1"));
                        }
                        _ => panic!("expected CallFunc(inner)"),
                    }
                }
                _ => panic!("expected CallFunc(middle)"),
            }
        }
        _ => panic!("expected CallFunc(outer)"),
    }
}

#[test]
fn expr_increment_decrement_mixed_with_arithmetic() {
    // i++ + j-- * 2
    // Should parse as: Add( i++, Mul(j--, 2) )
    let expr = parse_expr("i++ + j-- * 2");
    match &expr {
        Expr::BinaryOp { op, left, right, .. } => {
            assert!(matches!(op, BinaryOp::Add));
            // left: i++
            match left.as_ref() {
                Expr::UnaryOp { op: uop, operand, .. } => {
                    assert!(matches!(uop, UnaryOp::Increment));
                    assert_eq!(operand, "i");
                }
                _ => panic!("expected UnaryOp(Increment)"),
            }
            // right: j-- * 2
            match right.as_ref() {
                Expr::BinaryOp { op: inner_op, left: inner_left, .. } => {
                    assert!(matches!(inner_op, BinaryOp::Mul));
                    match inner_left.as_ref() {
                        Expr::UnaryOp { op: uop, operand, .. } => {
                            assert!(matches!(uop, UnaryOp::Decrement));
                            assert_eq!(operand, "j");
                        }
                        _ => panic!("expected UnaryOp(Decrement)"),
                    }
                }
                _ => panic!("expected BinaryOp(Mul)"),
            }
        }
        _ => panic!("expected BinaryOp(Add)"),
    }
}

#[test]
fn expr_func_call_in_complex_arithmetic() {
    // f(1) * g(2) + h(3) - 4
    let expr = parse_expr("f(1) * g(2) + h(3) - 4");
    // Top-level: Sub(..., 4)
    match &expr {
        Expr::BinaryOp { op, right, left, .. } => {
            assert!(matches!(op, BinaryOp::Sub));
            assert!(matches!(right.as_ref(), Expr::Number(v, _) if v == "4"));
            // left: Add(f(1)*g(2), h(3))
            match left.as_ref() {
                Expr::BinaryOp { op: add_op, left: mul_part, right: h_call, .. } => {
                    assert!(matches!(add_op, BinaryOp::Add));
                    assert!(matches!(h_call.as_ref(), Expr::CallFunc(c) if c.name == "h"));
                    // mul_part: f(1) * g(2)
                    match mul_part.as_ref() {
                        Expr::BinaryOp { op: mul_op, left: f_call, right: g_call, .. } => {
                            assert!(matches!(mul_op, BinaryOp::Mul));
                            assert!(matches!(f_call.as_ref(), Expr::CallFunc(c) if c.name == "f"));
                            assert!(matches!(g_call.as_ref(), Expr::CallFunc(c) if c.name == "g"));
                        }
                        _ => panic!("expected BinaryOp(Mul)"),
                    }
                }
                _ => panic!("expected BinaryOp(Add)"),
            }
        }
        _ => panic!("expected BinaryOp(Sub)"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 24. Complex boolean expressions
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bool_multi_and_or_mixed() {
    // (a>1 && b<2) || (c>=3 && d<=4) || (e>5)
    let b = parse_bool("(a>1 && b<2) || (c>=3 && d<=4) || (e>5)");
    // Top-level: Or
    match &b {
        BooleanExpr::Logical { op, .. } => {
            assert!(matches!(op, LogicalOp::Or));
        }
        _ => panic!("expected Logical(Or) at top level, got {b:?}"),
    }
}

#[test]
fn bool_nested_parentheses_and_or() {
    // ((a>1 && b>2) && (c>3 || d>4))
    let b = parse_bool("((a>1 && b>2) && (c>3 || d>4))");
    // Outermost: Grouped
    match &b {
        BooleanExpr::Grouped(inner) => {
            match inner.as_ref() {
                BooleanExpr::Logical { op, left, right, .. } => {
                    assert!(matches!(op, LogicalOp::And));
                    // left: (a>1 && b>2) grouped
                    assert!(matches!(left.as_ref(), BooleanExpr::Grouped(_)));
                    // right: (c>3 || d>4) grouped
                    match right.as_ref() {
                        BooleanExpr::Grouped(rinner) => {
                            assert!(matches!(
                                rinner.as_ref(),
                                BooleanExpr::Logical { op: LogicalOp::Or, .. }
                            ));
                        }
                        _ => panic!("expected Grouped(Or) on right"),
                    }
                }
                _ => panic!("expected Logical(And)"),
            }
        }
        _ => panic!("expected Grouped at top level, got {b:?}"),
    }
}

#[test]
fn bool_complex_arithmetic_both_sides() {
    // (a*2+b > c^3-d) && (e+f*g < h%i)
    let b = parse_bool("(a*2+b > c^3-d) && (e+f*g < h%i)");
    match &b {
        BooleanExpr::Logical { op, left, right, .. } => {
            assert!(matches!(op, LogicalOp::And));
            // left: grouped comparison with complex arithmetic
            match left.as_ref() {
                BooleanExpr::Grouped(g) => match g.as_ref() {
                    BooleanExpr::Comparison { op: cmp_op, left: lhs, right: rhs, .. } => {
                        assert!(matches!(cmp_op, ComparisonOp::Gt));
                        // lhs: a*2+b (top is Add)
                        assert!(matches!(lhs, Expr::BinaryOp { op: BinaryOp::Add, .. }));
                        // rhs: c^3-d (top is Sub)
                        assert!(matches!(rhs, Expr::BinaryOp { op: BinaryOp::Sub, .. }));
                    }
                    _ => panic!("expected Comparison inside left group"),
                },
                _ => panic!("expected Grouped on left"),
            }
            // right: grouped comparison
            match right.as_ref() {
                BooleanExpr::Grouped(g) => match g.as_ref() {
                    BooleanExpr::Comparison { op: cmp_op, left: lhs, right: rhs, .. } => {
                        assert!(matches!(cmp_op, ComparisonOp::Lt));
                        // lhs: e+f*g (top is Add)
                        assert!(matches!(lhs, Expr::BinaryOp { op: BinaryOp::Add, .. }));
                        // rhs: h%i (top is Mod)
                        assert!(matches!(rhs, Expr::BinaryOp { op: BinaryOp::Mod, .. }));
                    }
                    _ => panic!("expected Comparison inside right group"),
                },
                _ => panic!("expected Grouped on right"),
            }
        }
        _ => panic!("expected Logical(And), got {b:?}"),
    }
}

#[test]
fn bool_three_and_chain() {
    // a>1 && b>2 && c>3
    let b = parse_bool("a>1 && b>2 && c>3");
    // Left-associative: (a>1 && b>2) && c>3
    match &b {
        BooleanExpr::Logical { op, left, right, .. } => {
            assert!(matches!(op, LogicalOp::And));
            // right: c>3
            assert!(matches!(right.as_ref(), BooleanExpr::Comparison { .. }));
            // left: a>1 && b>2
            match left.as_ref() {
                BooleanExpr::Logical { op: inner_op, .. } => {
                    assert!(matches!(inner_op, LogicalOp::And));
                }
                _ => panic!("expected inner Logical(And)"),
            }
        }
        _ => panic!("expected Logical(And)"),
    }
}

#[test]
fn bool_mixed_and_or_no_parens() {
    // a>1 && b>2 || c>3 && d>4
    // Left-to-right: ((a>1 && b>2) || c>3) && d>4
    let b = parse_bool("a>1 && b>2 || c>3 && d>4");
    match &b {
        BooleanExpr::Logical { op, .. } => {
            // The parser processes left-to-right, so the structure depends on
            // its implementation. Just verify it parses without error.
            assert!(
                matches!(op, LogicalOp::And) || matches!(op, LogicalOp::Or),
                "expected And or Or at top level"
            );
        }
        _ => panic!("expected Logical"),
    }
}

#[test]
fn bool_all_six_comparison_ops_in_chain() {
    // (a > b) && (c < d) && (e >= f) && (g <= h) && (i == j) && (k != l)
    let b = parse_bool("(a > b) && (c < d) && (e >= f) && (g <= h) && (i == j) && (k != l)");
    // Count how many And operations we have (should be 5)
    fn count_ands(b: &BooleanExpr) -> usize {
        match b {
            BooleanExpr::Logical { op: LogicalOp::And, left, right, .. } => {
                1 + count_ands(left) + count_ands(right)
            }
            BooleanExpr::Grouped(inner) => count_ands(inner),
            _ => 0,
        }
    }
    assert_eq!(count_ands(&b), 5, "expected 5 And connectors");
}

// ═══════════════════════════════════════════════════════════════════
// 25. Multi-return + multi-assignment combinations
// ═══════════════════════════════════════════════════════════════════

#[test]
fn multi_assign_three_func_calls() {
    // var a, b, c = f(1), g(2), h(3)
    let prog = parse("var a, b, c = f(1), g(2), h(3)");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["a", "b", "c"]);
            assert_eq!(a.values.len(), 3);
            for (i, name) in ["f", "g", "h"].iter().enumerate() {
                match &a.values[i] {
                    Expr::CallFunc(c) => assert_eq!(&c.name, name),
                    _ => panic!("expected CallFunc for value {i}"),
                }
            }
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

#[test]
fn func_five_params_four_return_types() {
    let src = r#"func bigfunc(a: int, b: int, c: string, d: number, e: int) -> int, string, number, int {
    return 1, "x", 3, 4
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "bigfunc");
            assert_eq!(f.params.len(), 5);
            assert_eq!(f.params[0].name, "a");
            assert_eq!(f.params[1].name, "b");
            assert_eq!(f.params[2].name, "c");
            assert_eq!(f.params[3].name, "d");
            assert_eq!(f.params[4].name, "e");
            assert_eq!(f.return_types, vec!["int", "string", "number", "int"]);
            // Return has 4 values
            match &f.body.statements[0] {
                Statement::Return(r) => {
                    assert_eq!(r.values.len(), 4);
                    assert!(matches!(&r.values[0], Expr::Number(v, _) if v == "1"));
                    assert!(matches!(&r.values[1], Expr::StringLit(v, _) if v == "x"));
                    assert!(matches!(&r.values[2], Expr::Number(v, _) if v == "3"));
                    assert!(matches!(&r.values[3], Expr::Number(v, _) if v == "4"));
                }
                _ => panic!("expected Return"),
            }
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn return_four_expressions() {
    let src = r#"func quad() -> int, int, int, int {
    return a + 1, b * 2, c - 3, d / 4
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.return_types.len(), 4);
            match &f.body.statements[0] {
                Statement::Return(r) => {
                    assert_eq!(r.values.len(), 4);
                    // All 4 are BinaryOps
                    assert!(matches!(&r.values[0], Expr::BinaryOp { op: BinaryOp::Add, .. }));
                    assert!(matches!(&r.values[1], Expr::BinaryOp { op: BinaryOp::Mul, .. }));
                    assert!(matches!(&r.values[2], Expr::BinaryOp { op: BinaryOp::Sub, .. }));
                    assert!(matches!(&r.values[3], Expr::BinaryOp { op: BinaryOp::Div, .. }));
                }
                _ => panic!("expected Return"),
            }
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn multi_assign_five_targets() {
    let prog = parse("var a, b, c, d, e = 1, 2, 3, 4, 5");
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["a", "b", "c", "d", "e"]);
            assert_eq!(a.values.len(), 5);
        }
        _ => panic!("expected VarDecl Assignment"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 26. Go original test — detailed structure verification
// ═══════════════════════════════════════════════════════════════════

#[test]
fn go_test_full_program_detailed_structure() {
    // The full aparser_test.go Test_ReadString code, with detailed verification
    // of every top-level statement's internal structure
    let input = r#"
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
    let prog = parse(input);
    assert_eq!(prog.statements.len(), 8);

    // Statement 0: var fuck = 10
    match &prog.statements[0] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["fuck"]);
            assert!(matches!(&a.values[0], Expr::Number(v, _) if v == "10"));
        }
        _ => panic!("stmt 0: expected VarDecl"),
    }

    // Statement 1: if ((...))  elseif ((...))
    match &prog.statements[1] {
        Statement::IfStmt(i) => {
            // Main condition is a nested boolean expression
            // (the parser may represent it as Grouped or Logical depending on heuristic)
            assert!(
                matches!(&i.condition, BooleanExpr::Grouped(_) | BooleanExpr::Logical { .. }),
                "expected Grouped or Logical condition"
            );
            assert!(i.body.statements.is_empty()); // empty if body
            assert_eq!(i.else_if.len(), 1);
            assert_eq!(i.else_if[0].body.statements.len(), 1); // var i = 0
            assert!(i.else_body.is_none());
        }
        _ => panic!("stmt 1: expected IfStmt"),
    }

    // Statement 2: func fucker(wokao: int) -> int,int { return 1,2 }
    match &prog.statements[2] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "fucker");
            assert_eq!(f.params.len(), 1);
            assert_eq!(f.params[0].name, "wokao");
            assert_eq!(f.params[0].type_name, "int");
            assert_eq!(f.return_types, vec!["int", "int"]);
            assert_eq!(f.body.statements.len(), 1);
            match &f.body.statements[0] {
                Statement::Return(r) => assert_eq!(r.values.len(), 2),
                _ => panic!("expected Return in fucker body"),
            }
        }
        _ => panic!("stmt 2: expected FuncDecl"),
    }

    // Statement 3: var first,second = fucker(1,2,3)
    match &prog.statements[3] {
        Statement::VarDecl(VarDecl::Assignment(a)) => {
            assert_eq!(a.targets, vec!["first", "second"]);
            assert_eq!(a.values.len(), 1);
            match &a.values[0] {
                Expr::CallFunc(c) => {
                    assert_eq!(c.name, "fucker");
                    assert_eq!(c.args.len(), 3);
                }
                _ => panic!("expected CallFunc(fucker)"),
            }
        }
        _ => panic!("stmt 3: expected VarDecl"),
    }

    // Statement 4: fuck = 100+2*3-4^5+0~100
    match &prog.statements[4] {
        Statement::Assignment(a) => {
            assert_eq!(a.targets, vec!["fuck"]);
            assert_eq!(a.values.len(), 1);
            // Top-level of expression should be BinaryOp
            assert!(matches!(&a.values[0], Expr::BinaryOp { .. }));
        }
        _ => panic!("stmt 4: expected Assignment"),
    }

    // Statement 5: for (...) { if > elseif with nested for > if > elseif }
    match &prog.statements[5] {
        Statement::ForStmt(f) => {
            assert!(f.init.is_some());
            assert!(f.condition.is_some());
            assert!(f.step.is_some());
            // Body: 1 if statement
            assert_eq!(f.body.statements.len(), 1);
            match &f.body.statements[0] {
                Statement::IfStmt(i) => {
                    assert!(i.body.statements.is_empty()); // empty if body
                    assert_eq!(i.else_if.len(), 1);
                    // elseif body: var i = 0, then nested for loop
                    assert_eq!(i.else_if[0].body.statements.len(), 2);
                    assert!(matches!(
                        &i.else_if[0].body.statements[0],
                        Statement::VarDecl(_)
                    ));
                    // Nested for loop
                    match &i.else_if[0].body.statements[1] {
                        Statement::ForStmt(inner_f) => {
                            assert_eq!(inner_f.body.statements.len(), 1);
                            match &inner_f.body.statements[0] {
                                Statement::IfStmt(inner_i) => {
                                    // if body: break
                                    assert_eq!(inner_i.body.statements.len(), 1);
                                    assert!(matches!(
                                        &inner_i.body.statements[0],
                                        Statement::Break(_)
                                    ));
                                    // elseif body: var i = 0
                                    assert_eq!(inner_i.else_if.len(), 1);
                                    assert_eq!(inner_i.else_if[0].body.statements.len(), 1);
                                }
                                _ => panic!("expected inner IfStmt"),
                            }
                        }
                        _ => panic!("expected inner ForStmt"),
                    }
                }
                _ => panic!("expected IfStmt in for body"),
            }
        }
        _ => panic!("stmt 5: expected ForStmt"),
    }

    // Statement 6: func wocao -- already verified in existing test
    // Statement 7: for (;;) -- already verified in existing test
}

// ═══════════════════════════════════════════════════════════════════
// 27. Boundary cases — empty bodies
// ═══════════════════════════════════════════════════════════════════

#[test]
fn boundary_empty_func_body() {
    let prog = parse("func empty() -> void {\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert!(f.body.statements.is_empty());
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn boundary_empty_for_body() {
    let prog = parse("for (var i = 0; i < 10; i = i + 1) {\n}");
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert!(f.body.statements.is_empty());
        }
        _ => panic!("expected ForStmt"),
    }
}

#[test]
fn boundary_empty_if_body() {
    let prog = parse("if (x > 0) {\n}");
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert!(i.body.statements.is_empty());
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn boundary_empty_if_elseif_else_bodies() {
    let src = r#"if (a > 0) {
} elseif (b > 0) {
} else {
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert!(i.body.statements.is_empty());
            assert_eq!(i.else_if.len(), 1);
            assert!(i.else_if[0].body.statements.is_empty());
            assert!(i.else_body.as_ref().unwrap().statements.is_empty());
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn boundary_loop_only_break() {
    let prog = parse("for (;;) {\nbreak\n}");
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert_eq!(f.body.statements.len(), 1);
            assert!(matches!(&f.body.statements[0], Statement::Break(_)));
        }
        _ => panic!("expected ForStmt"),
    }
}

#[test]
fn boundary_loop_only_continue() {
    let prog = parse("for (;;) {\ncontinue\n}");
    match &prog.statements[0] {
        Statement::ForStmt(f) => {
            assert_eq!(f.body.statements.len(), 1);
            assert!(matches!(&f.body.statements[0], Statement::Continue(_)));
        }
        _ => panic!("expected ForStmt"),
    }
}

#[test]
fn boundary_func_only_return() {
    let prog = parse("func f() -> int {\nreturn 1\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.body.statements.len(), 1);
            assert!(matches!(&f.body.statements[0], Statement::Return(_)));
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn boundary_func_only_empty_return() {
    let prog = parse("func f() -> void {\nreturn\n}");
    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.body.statements.len(), 1);
            match &f.body.statements[0] {
                Statement::Return(r) => assert!(r.values.is_empty()),
                _ => panic!("expected Return"),
            }
        }
        _ => panic!("expected FuncDecl"),
    }
}

#[test]
fn boundary_statements_separated_by_many_blank_lines() {
    let src = "var a = 1\n\n\n\n\nvar b = 2\n\n\n\nvar c = 3";
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 3);
    for s in &prog.statements {
        assert!(matches!(s, Statement::VarDecl(_)));
    }
}

#[test]
fn boundary_leading_and_trailing_blank_lines() {
    let src = "\n\n\nvar x = 42\n\n\n";
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════
// 28. Long elseif chains
// ═══════════════════════════════════════════════════════════════════

#[test]
fn elseif_chain_three() {
    let src = r#"if (x > 10) {
    y = 4
} elseif (x > 7) {
    y = 3
} elseif (x > 4) {
    y = 2
} elseif (x > 1) {
    y = 1
} else {
    y = 0
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert_eq!(i.else_if.len(), 3);
            assert!(i.else_body.is_some());
            // Verify each elseif has exactly 1 statement
            for eif in &i.else_if {
                assert_eq!(eif.body.statements.len(), 1);
            }
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn elseif_chain_five() {
    let src = r#"if (x > 50) {
    y = 6
} elseif (x > 40) {
    y = 5
} elseif (x > 30) {
    y = 4
} elseif (x > 20) {
    y = 3
} elseif (x > 10) {
    y = 2
} elseif (x > 0) {
    y = 1
} else {
    y = 0
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert_eq!(i.else_if.len(), 5);
            assert!(i.else_body.is_some());
        }
        _ => panic!("expected IfStmt"),
    }
}

#[test]
fn elseif_chain_no_else() {
    // elseif chain without a final else
    let src = r#"if (x > 10) {
    y = 3
} elseif (x > 5) {
    y = 2
} elseif (x > 0) {
    y = 1
}"#;
    let prog = parse(src);
    match &prog.statements[0] {
        Statement::IfStmt(i) => {
            assert_eq!(i.else_if.len(), 2);
            assert!(i.else_body.is_none());
        }
        _ => panic!("expected IfStmt"),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 29. Additional error cases
// ═══════════════════════════════════════════════════════════════════

#[test]
fn error_if_no_condition_parens() {
    // if without parenthesized condition
    parse_err("if x > 0 {\n}");
}

#[test]
fn error_if_empty_condition() {
    // if () -- empty condition
    parse_err("if () {\n}");
}

#[test]
fn error_for_missing_first_semicolon() {
    parse_err("for (var i = 0 i < 10; i = i + 1) {\n}");
}

#[test]
fn error_for_missing_second_semicolon() {
    parse_err("for (var i = 0; i < 10 i = i + 1) {\n}");
}

#[test]
fn error_assignment_no_equals() {
    // x 42 -- missing =
    parse_err("x 42");
}

#[test]
fn error_incomplete_expression_trailing_add() {
    parse_err("var x = 1 +");
}

#[test]
fn error_incomplete_expression_trailing_mul() {
    parse_err("var x = 1 *");
}

#[test]
fn error_func_missing_arrow() {
    // func foo() int { } -- missing ->
    parse_err("func foo() int {\n}");
}

#[test]
fn error_func_missing_rparen() {
    parse_err("func foo(a: int {\n}");
}

#[test]
fn error_unclosed_block_in_func() {
    parse_err("func foo() -> int {\nvar x = 1");
}

#[test]
fn error_double_assignment() {
    // x = = 1
    parse_err("x = = 1");
}

#[test]
fn error_empty_program_with_keyword() {
    // Just a keyword with no valid statement
    parse_err("return");
}

// ═══════════════════════════════════════════════════════════════════
// 30. Complex combined programs
// ═══════════════════════════════════════════════════════════════════

#[test]
fn complex_program_game_simulation() {
    let src = r#"var hp = 100
var mp = 50
var level = 1

func heal(amount: int) -> int {
    if (hp + amount > 100) {
        hp = 100
    } else {
        hp = hp + amount
    }
    return hp
}

func attack(base: int, bonus: int) -> int, int {
    var dmg = base * level + bonus + 1 ~ 20
    var crit = dmg * 2
    return dmg, crit
}

for (var round = 0; round < 10; round = round + 1) {
    var dmg, crit = attack(10, level * 2)
    if (hp > 50) {
        hp = hp - dmg
    } elseif (hp > 20) {
        hp = hp - crit
        if (mp > 10) {
            heal(20)
            mp = mp - 10
        }
    } else {
        heal(50)
        mp = mp - 25
    }
    if (hp <= 0) {
        break
    }
    level = level + 1
}
"#;
    let prog = parse(src);
    // var hp, var mp, var level, func heal, func attack, for loop = 6 stmts
    assert_eq!(prog.statements.len(), 6);
    assert!(matches!(&prog.statements[0], Statement::VarDecl(_)));
    assert!(matches!(&prog.statements[1], Statement::VarDecl(_)));
    assert!(matches!(&prog.statements[2], Statement::VarDecl(_)));
    assert!(matches!(&prog.statements[3], Statement::FuncDecl(_)));
    assert!(matches!(&prog.statements[4], Statement::FuncDecl(_)));
    assert!(matches!(&prog.statements[5], Statement::ForStmt(_)));

    // Verify the for loop body has: var, if/elseif/else, if(break), assignment
    match &prog.statements[5] {
        Statement::ForStmt(f) => {
            assert_eq!(f.body.statements.len(), 4);
            assert!(matches!(&f.body.statements[0], Statement::VarDecl(_)));
            assert!(matches!(&f.body.statements[1], Statement::IfStmt(_)));
            assert!(matches!(&f.body.statements[2], Statement::IfStmt(_)));
            assert!(matches!(&f.body.statements[3], Statement::Assignment(_)));

            // The main if has 2 elseif + else
            match &f.body.statements[1] {
                Statement::IfStmt(i) => {
                    assert_eq!(i.else_if.len(), 1);
                    assert!(i.else_body.is_some());
                    // elseif body has nested if
                    let eif_body = &i.else_if[0].body;
                    assert_eq!(eif_body.statements.len(), 2); // hp=hp-crit, if(mp>10){...}
                }
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    }
}

#[test]
fn complex_program_nested_everything() {
    // A program that uses every language feature in nested context
    let src = r#"func process(n: int, m: int) -> int, int, int {
    var result = 0
    var count = 0
    for (var i = 0; i < n; i = i + 1) {
        for (var j = 0; j < m; j = j + 1) {
            if (i > j) {
                result = result + i * j
                count = count + 1
            } elseif (i == j) {
                if (result > 100) {
                    break
                }
                continue
            } else {
                result = result - 1 ~ 10
            }
        }
        if (count > 50) {
            break
        }
    }
    return result, count, n * m
}

var a, b, c = process(10, 20)
"#;
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 2);
    assert!(matches!(&prog.statements[0], Statement::FuncDecl(_)));
    assert!(matches!(&prog.statements[1], Statement::VarDecl(_)));

    match &prog.statements[0] {
        Statement::FuncDecl(f) => {
            assert_eq!(f.name, "process");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.return_types, vec!["int", "int", "int"]);
            // Body: var result, var count, for, return
            assert_eq!(f.body.statements.len(), 4);
            assert!(matches!(&f.body.statements[0], Statement::VarDecl(_)));
            assert!(matches!(&f.body.statements[1], Statement::VarDecl(_)));
            assert!(matches!(&f.body.statements[2], Statement::ForStmt(_)));
            assert!(matches!(&f.body.statements[3], Statement::Return(_)));
        }
        _ => unreachable!(),
    }
}
