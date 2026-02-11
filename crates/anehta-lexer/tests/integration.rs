//! Integration tests for the AnehtaLanguage lexer.
//!
//! These tests use complete, real-world code snippets ported from the original
//! Go test suite (`token_test.go`, `aparser_test.go`) and from the language
//! specification (`LANGUAGE_SPEC.md` Section 7).

use anehta_lexer::{Lexer, Token, TokenType};

// ── Helpers ────────────────────────────────────────────────────────

fn lex(input: &str) -> Vec<Token> {
    Lexer::new(input).tokenize().expect("lexer should succeed")
}

fn lex_err(input: &str) {
    let result = Lexer::new(input).tokenize();
    assert!(result.is_err(), "expected lexer error for input: {input:?}");
}

fn types(tokens: &[Token]) -> Vec<TokenType> {
    tokens.iter().map(|t| t.token_type).collect()
}

/// Filter out Newline and Eof tokens to make assertions on "meaningful" tokens.
fn meaningful(tokens: &[Token]) -> Vec<&Token> {
    tokens
        .iter()
        .filter(|t| t.token_type != TokenType::Newline && t.token_type != TokenType::Eof)
        .collect()
}

fn meaningful_types(tokens: &[Token]) -> Vec<TokenType> {
    meaningful(tokens).iter().map(|t| t.token_type).collect()
}

// ═══════════════════════════════════════════════════════════════════
//  1. Go test suite ports — token_test.go
// ═══════════════════════════════════════════════════════════════════

#[test]
fn go_token_test_main() {
    // Complete input from token_test.go `Test_Main`
    let input = r#"
    var fuck = 10
    if ((30+4>4+4+5&&fuck>3)&&(30>2)){

    }elseif((30+4>4+4+5&&fuck>3)&&(30>2)){
        var i = 0
    }
    func fucker(var wokao -> int) -> int,int{
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

    let tokens = lex(input);

    // Should produce a valid token stream ending with Eof
    assert_eq!(tokens.last().unwrap().token_type, TokenType::Eof);

    let mt = meaningful_types(&tokens);

    // Verify key constructs are present
    assert!(mt.contains(&TokenType::Var));
    assert!(mt.contains(&TokenType::If));
    assert!(mt.contains(&TokenType::ElseIf));
    assert!(mt.contains(&TokenType::Func));
    assert!(mt.contains(&TokenType::Return));
    assert!(mt.contains(&TokenType::For));
    assert!(mt.contains(&TokenType::Break));
    assert!(mt.contains(&TokenType::Also));    // &&
    assert!(mt.contains(&TokenType::Gt));      // >
    assert!(mt.contains(&TokenType::Lt));      // <
    assert!(mt.contains(&TokenType::Rand));    // ~
    assert!(mt.contains(&TokenType::Power));   // ^
    assert!(mt.contains(&TokenType::Semicolon));
    assert!(mt.contains(&TokenType::Comma));
}

// ═══════════════════════════════════════════════════════════════════
//  2. Go test suite ports — aparser_test.go `Test_ReadString`
// ═══════════════════════════════════════════════════════════════════

#[test]
fn go_parser_test_read_string() {
    // Complete input from aparser_test.go `Test_ReadString`
    let input = r#"
    var fuck = 10

    if ((30+4>4+4+5&&fuck>3)&&(30>2)){

    }elseif((30+4>4+4+5&&fuck>3)&&(30>2)){
        var i = 0
    }

    func fucker(var wokao -> int) -> int,int{
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

    func wocao (var wocao -> int,var wocao -> int) -> int{

    }

    for (;;){

    }

    "#;

    let tokens = lex(input);
    assert_eq!(tokens.last().unwrap().token_type, TokenType::Eof);

    let mt = meaningful_types(&tokens);

    // Extra constructs present in the parser test but not in the token test
    // - empty for loop: `for (;;)` should produce For, LParen, Semicolon, Semicolon, RParen
    assert!(mt.windows(5).any(|w| {
        w == [
            TokenType::For,
            TokenType::LParen,
            TokenType::Semicolon,
            TokenType::Semicolon,
            TokenType::RParen,
        ]
    }));

    // - Two function declarations
    let func_count = mt.iter().filter(|&&t| t == TokenType::Func).count();
    assert_eq!(func_count, 2);
}

#[test]
fn go_parser_test_read_expression() {
    // From aparser_test.go `Test_ReadExpression`: `1*2+true+4+(5+false)`
    let tokens = lex("1*2+true+4+(5+false)");
    let mt = meaningful_types(&tokens);

    let expected = vec![
        TokenType::Number,    // 1
        TokenType::Mul,       // *
        TokenType::Number,    // 2
        TokenType::Add,       // +
        TokenType::True,      // true
        TokenType::Add,       // +
        TokenType::Number,    // 4
        TokenType::Add,       // +
        TokenType::LParen,    // (
        TokenType::Number,    // 5
        TokenType::Add,       // +
        TokenType::False,     // false
        TokenType::RParen,    // )
    ];
    assert_eq!(mt, expected);
}

// ═══════════════════════════════════════════════════════════════════
//  3. Detailed token sequence verification for Go test snippets
// ═══════════════════════════════════════════════════════════════════

#[test]
fn var_declaration_with_assignment() {
    // `var fuck = 10`
    let tokens = lex("var fuck = 10");
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::Var,
            TokenType::Word,
            TokenType::Assignment,
            TokenType::Number,
        ]
    );
    let tokens2 = lex("var fuck = 10");
    let m = meaningful(&tokens2);
    assert_eq!(m[1].value, "fuck");
    assert_eq!(m[3].value, "10");
}

#[test]
fn complex_boolean_expression() {
    // `(30+4>4+4+5&&fuck>3)&&(30>2)`
    let tokens = lex("(30+4>4+4+5&&fuck>3)&&(30>2)");
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::LParen,
        TokenType::Number,   // 30
        TokenType::Add,
        TokenType::Number,   // 4
        TokenType::Gt,
        TokenType::Number,   // 4
        TokenType::Add,
        TokenType::Number,   // 4
        TokenType::Add,
        TokenType::Number,   // 5
        TokenType::Also,     // &&
        TokenType::Word,     // fuck
        TokenType::Gt,
        TokenType::Number,   // 3
        TokenType::RParen,
        TokenType::Also,     // &&
        TokenType::LParen,
        TokenType::Number,   // 30
        TokenType::Gt,
        TokenType::Number,   // 2
        TokenType::RParen,
    ];
    assert_eq!(mt, expected);
}

#[test]
fn func_with_multiple_return_types() {
    // `func fucker(var wokao -> int) -> int,int{`
    let tokens = lex("func fucker(var wokao -> int) -> int,int{");
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::Func,
        TokenType::Word,      // fucker
        TokenType::LParen,
        TokenType::Var,
        TokenType::Word,      // wokao
        TokenType::Casting,   // ->
        TokenType::Word,      // int
        TokenType::RParen,
        TokenType::Casting,   // ->
        TokenType::Word,      // int
        TokenType::Comma,
        TokenType::Word,      // int
        TokenType::LBrace,
    ];
    assert_eq!(mt, expected);
}

#[test]
fn multi_variable_assignment_with_func_call() {
    // `var first,second = fucker(1,2,3)`
    let tokens = lex("var first,second = fucker(1,2,3)");
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::Var,
        TokenType::Word,       // first
        TokenType::Comma,
        TokenType::Word,       // second
        TokenType::Assignment,
        TokenType::Word,       // fucker
        TokenType::LParen,
        TokenType::Number,     // 1
        TokenType::Comma,
        TokenType::Number,     // 2
        TokenType::Comma,
        TokenType::Number,     // 3
        TokenType::RParen,
    ];
    assert_eq!(mt, expected);
}

#[test]
fn arithmetic_with_power_and_random() {
    // `fuck = 100+2*3-4^5+0~100`
    let tokens = lex("fuck = 100+2*3-4^5+0~100");
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::Word,       // fuck
        TokenType::Assignment, // =
        TokenType::Number,     // 100
        TokenType::Add,        // +
        TokenType::Number,     // 2
        TokenType::Mul,        // *
        TokenType::Number,     // 3
        TokenType::Sub,        // -
        TokenType::Number,     // 4
        TokenType::Power,      // ^
        TokenType::Number,     // 5
        TokenType::Add,        // +
        TokenType::Number,     // 0
        TokenType::Rand,       // ~
        TokenType::Number,     // 100
    ];
    assert_eq!(mt, expected);
}

#[test]
fn for_loop_with_three_clauses() {
    // `for (var i = 100;i<100;i = i + 1){`
    let tokens = lex("for (var i = 100;i<100;i = i + 1){");
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::For,
        TokenType::LParen,
        TokenType::Var,
        TokenType::Word,       // i
        TokenType::Assignment,
        TokenType::Number,     // 100
        TokenType::Semicolon,
        TokenType::Word,       // i
        TokenType::Lt,
        TokenType::Number,     // 100
        TokenType::Semicolon,
        TokenType::Word,       // i
        TokenType::Assignment,
        TokenType::Word,       // i
        TokenType::Add,
        TokenType::Number,     // 1
        TokenType::RParen,
        TokenType::LBrace,
    ];
    assert_eq!(mt, expected);
}

#[test]
fn empty_for_loop() {
    // `for (;;){}`
    let tokens = lex("for (;;){}");
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::For,
        TokenType::LParen,
        TokenType::Semicolon,
        TokenType::Semicolon,
        TokenType::RParen,
        TokenType::LBrace,
        TokenType::RBrace,
    ];
    assert_eq!(mt, expected);
}

#[test]
fn return_multiple_values() {
    // `return 1,2`
    let tokens = lex("return 1,2");
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::Return,
        TokenType::Number,
        TokenType::Comma,
        TokenType::Number,
    ];
    assert_eq!(mt, expected);
}

// ═══════════════════════════════════════════════════════════════════
//  4. LANGUAGE_SPEC.md Section 7 — full program
// ═══════════════════════════════════════════════════════════════════

#[test]
fn language_spec_section7_full_example() {
    let input = r#"var health = 100
var name = "Anehta"

var score -> int

func attack(var base -> int, var bonus -> int) -> int, int {
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

func fibonacci(var n -> number) -> number {
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

    let tokens = lex(input);
    assert_eq!(tokens.last().unwrap().token_type, TokenType::Eof);

    let mt = meaningful_types(&tokens);

    // Verify all language features appear
    assert!(mt.contains(&TokenType::Var));
    assert!(mt.contains(&TokenType::StringLit));
    assert!(mt.contains(&TokenType::Casting));    // ->
    assert!(mt.contains(&TokenType::Func));
    assert!(mt.contains(&TokenType::Return));
    assert!(mt.contains(&TokenType::Comma));
    assert!(mt.contains(&TokenType::Rand));       // ~
    assert!(mt.contains(&TokenType::If));
    assert!(mt.contains(&TokenType::ElseIf));
    assert!(mt.contains(&TokenType::Else));
    assert!(mt.contains(&TokenType::For));
    assert!(mt.contains(&TokenType::Continue));
    assert!(mt.contains(&TokenType::Break));
    assert!(mt.contains(&TokenType::LtEq));       // <=
    assert!(mt.contains(&TokenType::Gt));
    assert!(mt.contains(&TokenType::Mul));
    assert!(mt.contains(&TokenType::Sub));
    assert!(mt.contains(&TokenType::Add));
    assert!(mt.contains(&TokenType::Semicolon));

    // Count function declarations (attack, fibonacci)
    let func_count = mt.iter().filter(|&&t| t == TokenType::Func).count();
    assert_eq!(func_count, 2);

    // Count var declarations (including parameters with `var` keyword)
    let var_count = mt.iter().filter(|&&t| t == TokenType::Var).count();
    // var health, var name, var score, var base(param), var bonus(param),
    // var damage, var critical, var dmg+crit, var i, var roll,
    // var n(param), var big = 12 vars
    assert_eq!(var_count, 12);
}

#[test]
fn language_spec_section7_token_by_token_first_lines() {
    // Verify the first few lines token-by-token:
    // var health = 100
    // var name = "Anehta"
    let input = "var health = 100\nvar name = \"Anehta\"";
    let tokens = lex(input);
    let expected_types = vec![
        TokenType::Var,        // var
        TokenType::Word,       // health
        TokenType::Assignment, // =
        TokenType::Number,     // 100
        TokenType::Newline,    // \n
        TokenType::Var,        // var
        TokenType::Word,       // name
        TokenType::Assignment, // =
        TokenType::StringLit,  // "Anehta"
        TokenType::Eof,
    ];
    assert_eq!(types(&tokens), expected_types);

    // Verify values
    assert_eq!(tokens[1].value, "health");
    assert_eq!(tokens[3].value, "100");
    assert_eq!(tokens[6].value, "name");
    assert_eq!(tokens[8].value, "Anehta");
}

#[test]
fn language_spec_type_declaration() {
    // `var score -> int`
    let tokens = lex("var score -> int");
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::Var,
            TokenType::Word,    // score
            TokenType::Casting, // ->
            TokenType::Word,    // int
        ]
    );
}

#[test]
fn language_spec_func_attack() {
    // func attack(var base -> int, var bonus -> int) -> int, int {
    let input = "func attack(var base -> int, var bonus -> int) -> int, int {";
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::Func,
        TokenType::Word,      // attack
        TokenType::LParen,
        TokenType::Var,
        TokenType::Word,      // base
        TokenType::Casting,
        TokenType::Word,      // int
        TokenType::Comma,
        TokenType::Var,
        TokenType::Word,      // bonus
        TokenType::Casting,
        TokenType::Word,      // int
        TokenType::RParen,
        TokenType::Casting,
        TokenType::Word,      // int
        TokenType::Comma,
        TokenType::Word,      // int
        TokenType::LBrace,
    ];
    assert_eq!(mt, expected);
}

#[test]
fn language_spec_if_elseif_else() {
    let input = r#"if (health > 80) {
    health = health - dmg
} elseif (health > 30) {
    health = health - crit
} else {
    health = 0
}"#;
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);

    // Verify the structure
    assert_eq!(mt[0], TokenType::If);
    assert!(mt.contains(&TokenType::ElseIf));
    assert!(mt.contains(&TokenType::Else));

    // Count braces: 3 open, 3 close
    let lbraces = mt.iter().filter(|&&t| t == TokenType::LBrace).count();
    let rbraces = mt.iter().filter(|&&t| t == TokenType::RBrace).count();
    assert_eq!(lbraces, 3);
    assert_eq!(rbraces, 3);
}

#[test]
fn language_spec_for_loop_with_continue_break() {
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
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);

    assert_eq!(mt[0], TokenType::For);
    assert!(mt.contains(&TokenType::Continue));
    assert!(mt.contains(&TokenType::Break));
    assert!(mt.contains(&TokenType::Rand)); // ~
    assert!(mt.contains(&TokenType::LtEq)); // <=
}

#[test]
fn language_spec_recursive_fibonacci() {
    let input = r#"func fibonacci(var n -> number) -> number {
    if (n <= 1) {
        return n
    }
    return fibonacci(n - 1) + fibonacci(n - 2)
}"#;
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);

    assert_eq!(mt[0], TokenType::Func);
    assert!(mt.contains(&TokenType::Return));
    assert!(mt.contains(&TokenType::LtEq));

    // `fibonacci` identifier appears 3 times (declaration + 2 recursive calls)
    let m = meaningful(&tokens);
    let fib_count = m
        .iter()
        .filter(|t| t.token_type == TokenType::Word && t.value == "fibonacci")
        .count();
    assert_eq!(fib_count, 3);
}

#[test]
fn language_spec_large_number() {
    // `var big = 999999999999999999 * 999999999999999999`
    let tokens = lex("var big = 999999999999999999 * 999999999999999999");
    let m = meaningful(&tokens);
    assert_eq!(m[0].token_type, TokenType::Var);
    assert_eq!(m[1].value, "big");
    assert_eq!(m[3].value, "999999999999999999");
    assert_eq!(m[4].token_type, TokenType::Mul);
    assert_eq!(m[5].value, "999999999999999999");
}

// ═══════════════════════════════════════════════════════════════════
//  5. Boundary tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn empty_input() {
    let tokens = lex("");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Eof);
}

#[test]
fn whitespace_only() {
    let tokens = lex("   \t  \t  ");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Eof);
}

#[test]
fn newlines_only_lf() {
    let tokens = lex("\n\n\n");
    let tt = types(&tokens);
    assert_eq!(
        tt,
        vec![
            TokenType::Newline,
            TokenType::Newline,
            TokenType::Newline,
            TokenType::Eof,
        ]
    );
}

#[test]
fn newlines_only_cr() {
    let tokens = lex("\r\r\r");
    let tt = types(&tokens);
    assert_eq!(
        tt,
        vec![
            TokenType::Newline,
            TokenType::Newline,
            TokenType::Newline,
            TokenType::Eof,
        ]
    );
}

#[test]
fn newlines_only_crlf() {
    let tokens = lex("\r\n\r\n");
    let tt = types(&tokens);
    assert_eq!(
        tt,
        vec![
            TokenType::Newline,
            TokenType::Newline,
            TokenType::Eof,
        ]
    );
}

#[test]
fn mixed_line_endings() {
    // LF, CRLF, CR
    let tokens = lex("a\nb\r\nc\rd");
    let tt = types(&tokens);
    assert_eq!(
        tt,
        vec![
            TokenType::Word,    // a
            TokenType::Newline, // \n
            TokenType::Word,    // b
            TokenType::Newline, // \r\n
            TokenType::Word,    // c
            TokenType::Newline, // \r
            TokenType::Word,    // d
            TokenType::Eof,
        ]
    );
    // Line tracking
    assert_eq!(tokens[0].span.line, 1); // a
    assert_eq!(tokens[2].span.line, 2); // b
    assert_eq!(tokens[4].span.line, 3); // c
    assert_eq!(tokens[6].span.line, 4); // d
}

#[test]
fn long_identifier() {
    let long_name = "a".repeat(1000);
    let tokens = lex(&long_name);
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Word]);
    assert_eq!(meaningful(&tokens)[0].value, long_name);
}

#[test]
fn long_number() {
    let long_num = "9".repeat(500);
    let tokens = lex(&long_num);
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Number]);
    assert_eq!(meaningful(&tokens)[0].value, long_num);
}

#[test]
fn long_float_number() {
    // 250 digits . 250 digits
    let long_float = format!("{}.{}", "1".repeat(250), "2".repeat(250));
    let tokens = lex(&long_float);
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Number]);
    assert_eq!(meaningful(&tokens)[0].value, long_float);
}

#[test]
fn long_string() {
    let content = "x".repeat(10_000);
    let input = format!("\"{}\"", content);
    let tokens = lex(&input);
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::StringLit]);
    assert_eq!(meaningful(&tokens)[0].value, content);
}

#[test]
fn empty_string() {
    let tokens = lex(r#""""#);
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::StringLit]);
    assert_eq!(meaningful(&tokens)[0].value, "");
}

#[test]
fn identifier_all_underscores() {
    let tokens = lex("___");
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Word]);
    assert_eq!(meaningful(&tokens)[0].value, "___");
}

#[test]
fn number_zero() {
    let tokens = lex("0");
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Number]);
    assert_eq!(meaningful(&tokens)[0].value, "0");
}

#[test]
fn number_with_leading_zeros() {
    // Lexer treats this as a single number token "007"
    let tokens = lex("007");
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Number]);
    assert_eq!(meaningful(&tokens)[0].value, "007");
}

#[test]
fn float_with_trailing_dot() {
    // "42." — the lexer should produce a Number with value "42."
    let tokens = lex("42.");
    // The dot might be parsed as part of the number or as a separate Dot token.
    // Based on the lexer: digits come first, then '.' is consumed as part of
    // the number (dot_count=1), then no more digits, so value = "42."
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Number]);
    assert_eq!(meaningful(&tokens)[0].value, "42.");
}

// ═══════════════════════════════════════════════════════════════════
//  6. Error recovery tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn illegal_character_at() {
    lex_err("@");
}

#[test]
fn illegal_character_hash() {
    lex_err("#");
}

#[test]
fn illegal_character_dollar() {
    lex_err("$");
}

#[test]
fn illegal_character_backtick() {
    lex_err("`");
}

#[test]
fn illegal_character_backslash() {
    lex_err("\\");
}

#[test]
fn illegal_character_question_mark() {
    lex_err("?");
}

#[test]
fn multiple_dots_in_number() {
    lex_err("1.2.3");
}

#[test]
fn multiple_dots_in_number_many() {
    lex_err("1.2.3.4.5");
}

#[test]
fn unclosed_string_eof() {
    lex_err("\"hello world");
}

#[test]
fn unclosed_string_with_escape_at_end() {
    lex_err("\"hello\\");
}

#[test]
fn illegal_char_in_valid_context() {
    // Valid tokens surrounding an illegal character
    lex_err("var x = @10");
}

// ═══════════════════════════════════════════════════════════════════
//  7. All operator combinations
// ═══════════════════════════════════════════════════════════════════

#[test]
fn all_single_operators() {
    let input = "+ - * / ^ % ~ ! > < = . , : ; ( ) { } [ ] & |";
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::Add,
            TokenType::Sub,
            TokenType::Mul,
            TokenType::Div,
            TokenType::Power,
            TokenType::Mod,
            TokenType::Rand,
            TokenType::Not,
            TokenType::Gt,
            TokenType::Lt,
            TokenType::Assignment,
            TokenType::Dot,
            TokenType::Comma,
            TokenType::Colon,
            TokenType::Semicolon,
            TokenType::LParen,
            TokenType::RParen,
            TokenType::LBrace,
            TokenType::RBrace,
            TokenType::LBracket,
            TokenType::RBracket,
            TokenType::And,
            TokenType::Or,
        ]
    );
}

#[test]
fn all_double_operators() {
    let input = "++ -- += -= *= /= -> >= <= == != && ||";
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::AddSelf,
            TokenType::SubSelf,
            TokenType::CompositeAdd,
            TokenType::CompositeSub,
            TokenType::CompositeMul,
            TokenType::CompositeDiv,
            TokenType::Casting,
            TokenType::GtEq,
            TokenType::LtEq,
            TokenType::Eq,
            TokenType::NotEq,
            TokenType::Also,
            TokenType::Perhaps,
        ]
    );
}

#[test]
fn operators_without_spaces() {
    // All operators jammed together should still tokenize greedily
    let tokens = lex("++--");
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::AddSelf, TokenType::SubSelf]);
}

#[test]
fn minus_arrow_disambiguation() {
    // `->` is Casting, not Sub + Gt
    let tokens = lex("->");
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Casting]);
}

#[test]
fn operators_adjacent_to_identifiers() {
    // No spaces: `a+b`, `x>=y`
    let tokens = lex("a+b");
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![TokenType::Word, TokenType::Add, TokenType::Word]
    );

    let tokens2 = lex("x>=y");
    let mt2 = meaningful_types(&tokens2);
    assert_eq!(
        mt2,
        vec![TokenType::Word, TokenType::GtEq, TokenType::Word]
    );
}

// ═══════════════════════════════════════════════════════════════════
//  8. All keywords
// ═══════════════════════════════════════════════════════════════════

#[test]
fn all_keywords_individually() {
    let cases: Vec<(&str, TokenType)> = vec![
        ("func", TokenType::Func),
        ("var", TokenType::Var),
        ("if", TokenType::If),
        ("else", TokenType::Else),
        ("elseif", TokenType::ElseIf),
        ("for", TokenType::For),
        ("break", TokenType::Break),
        ("continue", TokenType::Continue),
        ("return", TokenType::Return),
        ("true", TokenType::True),
        ("false", TokenType::False),
        ("switch", TokenType::Switch),
        ("case", TokenType::Case),
        ("new", TokenType::New),
    ];

    for (keyword, expected_type) in cases {
        let tokens = lex(keyword);
        assert_eq!(
            tokens[0].token_type, expected_type,
            "keyword '{}' should produce {:?}",
            keyword, expected_type
        );
        assert_eq!(tokens[0].value, keyword);
    }
}

#[test]
fn keyword_like_identifiers() {
    // Identifiers that start with keywords but are not keywords
    let cases = vec![
        "format",     // starts with "for"
        "iffy",       // starts with "if"
        "variable",   // starts with "var"
        "functional", // starts with "func"
        "elsewhere",  // starts with "else"
        "breaking",   // starts with "break"
        "returning",  // starts with "return"
        "trueblood",  // starts with "true"
        "falsehood",  // starts with "false"
        "switching",  // starts with "switch"
        "caseload",   // starts with "case"
        "newbie",     // starts with "new"
        "continue2",  // starts with "continue"
    ];

    for ident in cases {
        let tokens = lex(ident);
        assert_eq!(
            tokens[0].token_type,
            TokenType::Word,
            "'{}' should be an identifier, not a keyword",
            ident
        );
        assert_eq!(tokens[0].value, ident);
    }
}

// ═══════════════════════════════════════════════════════════════════
//  9. String literal edge cases
// ═══════════════════════════════════════════════════════════════════

#[test]
fn string_with_escaped_backslash() {
    // "a\\b" -> content should be a\b (the lexer takes next char literally)
    let tokens = lex(r#""a\\b""#);
    assert_eq!(tokens[0].token_type, TokenType::StringLit);
    assert_eq!(tokens[0].value, "a\\b");
}

#[test]
fn string_with_escaped_n() {
    // "hello\nworld" — the lexer escape takes 'n' literally (not as newline)
    let tokens = lex(r#""hello\nworld""#);
    assert_eq!(tokens[0].token_type, TokenType::StringLit);
    assert_eq!(tokens[0].value, "hellonworld");
}

#[test]
fn multiple_strings_on_one_line() {
    let tokens = lex(r#""hello" "world""#);
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::StringLit, TokenType::StringLit]);
    let m = meaningful(&tokens);
    assert_eq!(m[0].value, "hello");
    assert_eq!(m[1].value, "world");
}

#[test]
fn string_with_spaces() {
    let tokens = lex(r#""hello world 123""#);
    assert_eq!(tokens[0].value, "hello world 123");
}

#[test]
fn string_with_operators_inside() {
    let tokens = lex(r#""a + b = c""#);
    assert_eq!(tokens[0].token_type, TokenType::StringLit);
    assert_eq!(tokens[0].value, "a + b = c");
}

// ═══════════════════════════════════════════════════════════════════
// 10. Span / position tracking
// ═══════════════════════════════════════════════════════════════════

#[test]
fn multiline_span_tracking() {
    let input = "var x = 1\nvar y = 2\nvar z = 3";
    let tokens = lex(input);
    let vars: Vec<&Token> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Var)
        .collect();

    assert_eq!(vars.len(), 3);
    assert_eq!(vars[0].span.line, 1);
    assert_eq!(vars[0].span.column, 1);
    assert_eq!(vars[1].span.line, 2);
    assert_eq!(vars[1].span.column, 1);
    assert_eq!(vars[2].span.line, 3);
    assert_eq!(vars[2].span.column, 1);
}

#[test]
fn column_tracking_with_tabs() {
    // Tabs count as 1 column advance in the lexer (they are just whitespace)
    let tokens = lex("\tx");
    assert_eq!(tokens[0].token_type, TokenType::Word);
    assert_eq!(tokens[0].span.column, 2);
}

#[test]
fn eof_span_after_content() {
    let tokens = lex("abc");
    let eof = tokens.last().unwrap();
    assert_eq!(eof.token_type, TokenType::Eof);
    // After reading "abc" (3 chars), pos=3 so column=4
    assert_eq!(eof.span.line, 1);
    assert_eq!(eof.span.column, 4);
}

// ═══════════════════════════════════════════════════════════════════
// 11. Whitespace handling
// ═══════════════════════════════════════════════════════════════════

#[test]
fn tabs_and_spaces_are_skipped() {
    let tokens = lex("  \t  var  \t  x  \t  ");
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Var, TokenType::Word]);
}

#[test]
fn no_whitespace_between_tokens() {
    let tokens = lex("var x=42");
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::Var,
            TokenType::Word,
            TokenType::Assignment,
            TokenType::Number,
        ]
    );
}

// ═══════════════════════════════════════════════════════════════════
// 12. Quick start examples from spec
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spec_quick_start_swap_function() {
    let input = r#"func swap(var a -> int, var b -> int) -> int, int {
    return b, a
}"#;
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);

    assert_eq!(mt[0], TokenType::Func);
    let m = meaningful(&tokens);
    assert_eq!(m[1].value, "swap");

    // Count return types: after RParen -> Casting -> int Comma int
    assert!(mt.contains(&TokenType::Return));
}

#[test]
fn spec_quick_start_multi_assignment() {
    let input = "var first, second = swap(1, 2)";
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::Var,
        TokenType::Word,       // first
        TokenType::Comma,
        TokenType::Word,       // second
        TokenType::Assignment,
        TokenType::Word,       // swap
        TokenType::LParen,
        TokenType::Number,     // 1
        TokenType::Comma,
        TokenType::Number,     // 2
        TokenType::RParen,
    ];
    assert_eq!(mt, expected);
}

#[test]
fn spec_quick_start_infinite_loop() {
    let input = "for (;;) {\n    break\n}";
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::For,
            TokenType::LParen,
            TokenType::Semicolon,
            TokenType::Semicolon,
            TokenType::RParen,
            TokenType::LBrace,
            TokenType::Break,
            TokenType::RBrace,
        ]
    );
}

#[test]
fn spec_random_expression() {
    let input = "var result = 100 + 2 * 3 - 4 ^ 5 + 0 ~ 100";
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);
    let expected = vec![
        TokenType::Var,
        TokenType::Word,       // result
        TokenType::Assignment,
        TokenType::Number,     // 100
        TokenType::Add,
        TokenType::Number,     // 2
        TokenType::Mul,
        TokenType::Number,     // 3
        TokenType::Sub,
        TokenType::Number,     // 4
        TokenType::Power,
        TokenType::Number,     // 5
        TokenType::Add,
        TokenType::Number,     // 0
        TokenType::Rand,
        TokenType::Number,     // 100
    ];
    assert_eq!(mt, expected);
}

// ═══════════════════════════════════════════════════════════════════
// 13. Elseif as single keyword (not "else if")
// ═══════════════════════════════════════════════════════════════════

#[test]
fn elseif_is_single_keyword() {
    let tokens = lex("elseif");
    assert_eq!(tokens[0].token_type, TokenType::ElseIf);
    assert_eq!(tokens[0].value, "elseif");
}

#[test]
fn else_if_are_two_tokens() {
    // "else if" with a space produces two separate tokens
    let tokens = lex("else if");
    let mt = meaningful_types(&tokens);
    assert_eq!(mt, vec![TokenType::Else, TokenType::If]);
}

// ═══════════════════════════════════════════════════════════════════
// 14. Edge cases with adjacent tokens
// ═══════════════════════════════════════════════════════════════════

#[test]
fn braces_adjacent() {
    let tokens = lex("{}[]()");
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::LBrace,
            TokenType::RBrace,
            TokenType::LBracket,
            TokenType::RBracket,
            TokenType::LParen,
            TokenType::RParen,
        ]
    );
}

#[test]
fn number_followed_by_operator_no_space() {
    let tokens = lex("42+7");
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![TokenType::Number, TokenType::Add, TokenType::Number]
    );
    let m = meaningful(&tokens);
    assert_eq!(m[0].value, "42");
    assert_eq!(m[2].value, "7");
}

#[test]
fn deeply_nested_expression() {
    let input = "((((((1))))))";
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::LParen,
            TokenType::LParen,
            TokenType::LParen,
            TokenType::LParen,
            TokenType::LParen,
            TokenType::LParen,
            TokenType::Number,
            TokenType::RParen,
            TokenType::RParen,
            TokenType::RParen,
            TokenType::RParen,
            TokenType::RParen,
            TokenType::RParen,
        ]
    );
}

#[test]
fn multiple_statements_multiline() {
    let input = "var a = 1\nvar b = 2\na = a + b";
    let tokens = lex(input);
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::Var,
            TokenType::Word,       // a
            TokenType::Assignment,
            TokenType::Number,     // 1
            TokenType::Var,
            TokenType::Word,       // b
            TokenType::Assignment,
            TokenType::Number,     // 2
            TokenType::Word,       // a
            TokenType::Assignment,
            TokenType::Word,       // a
            TokenType::Add,
            TokenType::Word,       // b
        ]
    );
}

// ═══════════════════════════════════════════════════════════════════
// 15. Increment / Decrement with identifiers
// ═══════════════════════════════════════════════════════════════════

#[test]
fn increment_decrement_postfix() {
    let tokens = lex("i++ j--");
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::Word,    // i
            TokenType::AddSelf, // ++
            TokenType::Word,    // j
            TokenType::SubSelf, // --
        ]
    );
}

#[test]
fn compound_assignment_operators() {
    let tokens = lex("x += 1 y -= 2 z *= 3 w /= 4");
    let mt = meaningful_types(&tokens);
    assert_eq!(
        mt,
        vec![
            TokenType::Word,
            TokenType::CompositeAdd,
            TokenType::Number,
            TokenType::Word,
            TokenType::CompositeSub,
            TokenType::Number,
            TokenType::Word,
            TokenType::CompositeMul,
            TokenType::Number,
            TokenType::Word,
            TokenType::CompositeDiv,
            TokenType::Number,
        ]
    );
}

// ═══════════════════════════════════════════════════════════════════
// 16. Stress: many tokens
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_many_var_declarations() {
    let lines: Vec<String> = (0..200)
        .map(|i| format!("var x{} = {}", i, i * 10))
        .collect();
    let input = lines.join("\n");
    let tokens = lex(&input);

    let var_count = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Var)
        .count();
    assert_eq!(var_count, 200);
}

#[test]
fn stress_long_expression() {
    // 1 + 2 + 3 + ... + 500
    let parts: Vec<String> = (1..=500).map(|i| i.to_string()).collect();
    let input = parts.join(" + ");
    let tokens = lex(&input);

    let num_count = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .count();
    assert_eq!(num_count, 500);

    let add_count = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Add)
        .count();
    assert_eq!(add_count, 499);
}
