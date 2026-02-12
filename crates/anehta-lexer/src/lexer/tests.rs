use super::*;
use crate::token::TokenType;

fn lex(input: &str) -> Vec<Token> {
    Lexer::new(input).tokenize().expect("lexer should succeed")
}

fn types(tokens: &[Token]) -> Vec<TokenType> {
    tokens.iter().map(|t| t.token_type).collect()
}

#[test]
fn empty_source() {
    let tokens = lex("");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Eof);
}

#[test]
fn integer_literal() {
    let tokens = lex("42");
    assert_eq!(types(&tokens), vec![TokenType::Number, TokenType::Eof]);
    assert_eq!(tokens[0].value, "42");
}

#[test]
fn float_literal() {
    let tokens = lex("3.14");
    assert_eq!(types(&tokens), vec![TokenType::Number, TokenType::Eof]);
    assert_eq!(tokens[0].value, "3.14");
}

#[test]
fn illegal_number_multiple_dots() {
    let result = Lexer::new("1.2.3").tokenize();
    assert!(result.is_err());
}

#[test]
fn string_literal() {
    let tokens = lex("\"hello\"");
    assert_eq!(types(&tokens), vec![TokenType::StringLit, TokenType::Eof]);
    assert_eq!(tokens[0].value, "hello");
}

#[test]
fn string_with_escape() {
    let tokens = lex(r#""a\"b""#);
    assert_eq!(tokens[0].value, "a\"b");
}

#[test]
fn unclosed_string_error() {
    let result = Lexer::new("\"hello").tokenize();
    assert!(result.is_err());
}

#[test]
fn keywords() {
    let tokens = lex("func var if else elseif for break continue return true false switch case new");
    let expected = vec![
        TokenType::Func,
        TokenType::Var,
        TokenType::If,
        TokenType::Else,
        TokenType::ElseIf,
        TokenType::For,
        TokenType::Break,
        TokenType::Continue,
        TokenType::Return,
        TokenType::True,
        TokenType::False,
        TokenType::Switch,
        TokenType::Case,
        TokenType::New,
        TokenType::Eof,
    ];
    assert_eq!(types(&tokens), expected);
}

#[test]
fn identifiers() {
    let tokens = lex("foo _bar myVar123");
    assert_eq!(types(&tokens), vec![
        TokenType::Word, TokenType::Word, TokenType::Word, TokenType::Eof,
    ]);
    assert_eq!(tokens[0].value, "foo");
    assert_eq!(tokens[1].value, "_bar");
    assert_eq!(tokens[2].value, "myVar123");
}

#[test]
fn identifier_starting_with_keyword_prefix() {
    // "format" starts with "for" but should be Word
    let tokens = lex("format");
    assert_eq!(types(&tokens), vec![TokenType::Word, TokenType::Eof]);
    assert_eq!(tokens[0].value, "format");
}

#[test]
fn single_char_operators() {
    let tokens = lex("+ - * / ^ % ~ ! . , : ;");
    let expected = vec![
        TokenType::Add, TokenType::Sub, TokenType::Mul, TokenType::Div,
        TokenType::Power, TokenType::Mod, TokenType::Rand, TokenType::Not,
        TokenType::Dot, TokenType::Comma, TokenType::Colon, TokenType::Semicolon,
        TokenType::Eof,
    ];
    assert_eq!(types(&tokens), expected);
}

#[test]
fn double_char_operators() {
    let tokens = lex("++ -- += -= *= /= -> >= <= == != && ||");
    let expected = vec![
        TokenType::AddSelf, TokenType::SubSelf,
        TokenType::CompositeAdd, TokenType::CompositeSub,
        TokenType::CompositeMul, TokenType::CompositeDiv,
        TokenType::Casting, TokenType::GtEq, TokenType::LtEq,
        TokenType::Eq, TokenType::NotEq, TokenType::Also, TokenType::Perhaps,
        TokenType::Eof,
    ];
    assert_eq!(types(&tokens), expected);
}

#[test]
fn delimiters() {
    let tokens = lex("( ) { } [ ]");
    let expected = vec![
        TokenType::LParen, TokenType::RParen,
        TokenType::LBrace, TokenType::RBrace,
        TokenType::LBracket, TokenType::RBracket,
        TokenType::Eof,
    ];
    assert_eq!(types(&tokens), expected);
}

#[test]
fn assignment_vs_eq() {
    let tokens = lex("= ==");
    assert_eq!(types(&tokens), vec![
        TokenType::Assignment, TokenType::Eq, TokenType::Eof,
    ]);
}

#[test]
fn newline_tracking() {
    let tokens = lex("a\nb");
    assert_eq!(types(&tokens), vec![
        TokenType::Word, TokenType::Newline, TokenType::Word, TokenType::Eof,
    ]);
    // 'a' at line 1, col 1
    assert_eq!(tokens[0].span.line, 1);
    assert_eq!(tokens[0].span.column, 1);
    // 'b' at line 2, col 1
    assert_eq!(tokens[2].span.line, 2);
    assert_eq!(tokens[2].span.column, 1);
}

#[test]
fn crlf_newline() {
    let tokens = lex("x\r\ny");
    assert_eq!(types(&tokens), vec![
        TokenType::Word, TokenType::Newline, TokenType::Word, TokenType::Eof,
    ]);
    assert_eq!(tokens[2].span.line, 2);
}

#[test]
fn var_declaration() {
    let tokens = lex("var x = 42");
    let expected = vec![
        TokenType::Var, TokenType::Word, TokenType::Assignment,
        TokenType::Number, TokenType::Eof,
    ];
    assert_eq!(types(&tokens), expected);
}

#[test]
fn func_declaration() {
    let tokens = lex("func add(var a -> int, var b -> int) -> int {\nreturn a + b\n}");
    // Just check that it tokenizes without error and starts/ends correctly
    assert_eq!(tokens.first().unwrap().token_type, TokenType::Func);
    assert_eq!(tokens.last().unwrap().token_type, TokenType::Eof);
}

#[test]
fn for_loop() {
    let tokens = lex("for (var i = 0; i < 100; i++)");
    assert!(tokens.iter().any(|t| t.token_type == TokenType::For));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Semicolon));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::AddSelf));
}

#[test]
fn random_operator_expression() {
    let tokens = lex("1 ~ 6");
    assert_eq!(types(&tokens), vec![
        TokenType::Number, TokenType::Rand, TokenType::Number, TokenType::Eof,
    ]);
}

#[test]
fn column_tracking() {
    let tokens = lex("ab + c");
    // "ab" starts at col 1
    assert_eq!(tokens[0].span.column, 1);
    // "+" starts at col 4
    assert_eq!(tokens[1].span.column, 4);
    // "c" starts at col 6
    assert_eq!(tokens[2].span.column, 6);
}

#[test]
fn illegal_character_error() {
    let result = Lexer::new("@").tokenize();
    assert!(result.is_err());
}

#[test]
fn bitwise_operators() {
    let tokens = lex("& |");
    assert_eq!(types(&tokens), vec![
        TokenType::And, TokenType::Or, TokenType::Eof,
    ]);
}

#[test]
fn complex_example() {
    let src = r#"var health = 100
var name = "Anehta"
if (health > 80) {
    health = health - 1
}"#;
    let tokens = lex(src);
    // Should tokenize without error
    assert_eq!(tokens.last().unwrap().token_type, TokenType::Eof);
    // Check that we have some expected tokens
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Var));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::If));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::StringLit));
}
