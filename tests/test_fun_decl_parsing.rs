use restrict_lang::lexer::{lex_token, skip, Token};
use restrict_lang::parser::parse_program;

#[test]
fn test_tokenize_generic_function() {
    let input = "fun identity: <T>(value: T) -> T = { value }";

    let mut remaining = input;
    let mut tokens = Vec::new();

    // Manually tokenize to see what happens
    while !remaining.is_empty() {
        let (rest, _) = skip(remaining).unwrap();
        if rest.is_empty() {
            break;
        }

        match lex_token(rest) {
            Ok((new_remaining, token)) => {
                tokens.push(token);
                remaining = new_remaining;
            }
            Err(e) => {
                panic!("Tokenization error: {:?}", e);
            }
        }
    }

    assert!(tokens.len() >= 14, "Expected generic function tokens");
    assert_eq!(tokens[0], Token::Fun);
    assert_eq!(tokens[1], Token::Ident("identity".to_string()));
    assert_eq!(tokens[2], Token::Colon);
    assert_eq!(tokens[3], Token::Lt);
    assert_eq!(tokens[4], Token::Ident("T".to_string()));
    assert_eq!(tokens[5], Token::Gt);
    assert_eq!(tokens[6], Token::LParen);
}

#[test]
fn test_parse_function_variations() {
    let test_cases = vec![
        ("fun test = { () }", true, "simple function"),
        (
            "fun identity: <T>(value: T) -> T = { value }",
            true,
            "generic function",
        ),
        (
            "fun choose: <T>(value: T, fallback: T) -> T = { value }",
            true,
            "multi-parameter generic function",
        ),
        (
            "fun zero: () -> Int32 = { 0 }",
            true,
            "zero-argument function",
        ),
    ];

    for (input, should_succeed, description) in test_cases {
        match parse_program(input) {
            Ok((remaining, program)) => {
                if should_succeed {
                    assert_eq!(program.declarations.len(), 1);
                    assert!(remaining.trim().is_empty());
                } else {
                    panic!(
                        "Expected failure but parsing succeeded for: {}",
                        description
                    );
                }
            }
            Err(e) => {
                if !should_succeed {
                    let _ = e;
                } else {
                    panic!("Parsing failed for {}: {:?}", description, e);
                }
            }
        }
    }
}
