use restrict_lang::parser::{parse_program};
use restrict_lang::lexer::{lex_token, Token, skip};

#[test]
fn test_tokenize_temporal_function() {
    let input = "fun leakFile<~io> = { Unit }";
    
    eprintln!("=== Tokenizing temporal function ===");
    eprintln!("Input: {}", input);
    
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
                eprintln!("Token: {:?}", token);
                tokens.push(token);
                remaining = new_remaining;
            }
            Err(e) => {
                eprintln!("Tokenization error: {:?}", e);
                break;
            }
        }
    }
    
    eprintln!("\nTotal tokens: {}", tokens.len());
    
    // Check that we have the expected tokens
    assert!(tokens.len() >= 8, "Expected at least 8 tokens");
    assert_eq!(tokens[0], Token::Fun);
    assert_eq!(tokens[1], Token::Ident("leakFile".to_string()));
    assert_eq!(tokens[2], Token::Lt);
    assert_eq!(tokens[3], Token::Tilde);
    assert_eq!(tokens[4], Token::Ident("io".to_string()));
    assert_eq!(tokens[5], Token::Gt);
    assert_eq!(tokens[6], Token::Assign);
}

#[test]
#[ignore = "TAT (Temporal Affine Types) syntax - deferred to v2.0"]
fn test_parse_temporal_function_variations() {
    let test_cases = vec![
        ("fun test = { Unit }", true, "simple function"),
        ("fun test<T> = { Unit }", true, "generic function"),
        ("fun test<~t> = { Unit }", true, "temporal function"),
        ("fun test<~a, ~b> = { Unit }", true, "multiple temporal"),
        ("fun test<T, ~t> = { Unit }", true, "mixed generic and temporal"),
        ("fun leakFile<~io> = { Unit }", true, "leakFile function"),
    ];
    
    for (input, should_succeed, description) in test_cases {
        eprintln!("\n=== Testing: {} ===", description);
        eprintln!("Input: {}", input);
        
        match parse_program(input) {
            Ok((remaining, program)) => {
                if should_succeed {
                    eprintln!("✓ Success! Declarations: {}", program.declarations.len());
                    assert_eq!(program.declarations.len(), 1);
                    assert!(remaining.trim().is_empty());
                } else {
                    panic!("Expected failure but parsing succeeded for: {}", description);
                }
            }
            Err(e) => {
                if !should_succeed {
                    eprintln!("✓ Expected failure: {:?}", e);
                } else {
                    eprintln!("✗ Unexpected failure: {:?}", e);
                    panic!("Parsing failed for: {}", description);
                }
            }
        }
    }
}