use restrict_lang::{
    lexer::{Token, lex_token},
    parser::{simple_expr},
    ast::*,
};
use nom::{
    IResult,
    multi::many0,
    combinator::opt,
};

fn expect_token<'a>(expected: Token) -> impl Fn(&'a str) -> IResult<&'a str, ()> {
    move |input| {
        let (input, token) = lex_token(input)?;
        if token == expected {
            Ok((input, ()))
        } else {
            Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
        }
    }
}

fn ident(input: &str) -> IResult<&str, String> {
    let (input, token) = lex_token(input)?;
    match token {
        Token::Ident(name) => Ok((input, name)),
        _ => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    }
}

fn trace_fun_decl(input: &str) -> IResult<&str, ()> {
    println!("\n>>> Starting fun_decl");
    println!("    Input: {:?}", &input[..30.min(input.len())]);
    
    // Step 1: expect Token::Fun
    println!("\n  Step 1: expect Token::Fun");
    let (input, _) = expect_token(Token::Fun)(input)?;
    println!("    ✓ Found 'fun', remaining: {} chars", input.len());
    println!("    Next 20 chars: {:?}", &input[..20.min(input.len())]);
    
    // Step 2: expect identifier
    println!("\n  Step 2: expect identifier");
    let (input, name) = ident(input)?;
    println!("    ✓ Found identifier '{}', remaining: {} chars", name, input.len());
    println!("    Next 20 chars: {:?}", &input[..20.min(input.len())]);
    
    // Step 3: expect Token::Assign
    println!("\n  Step 3: expect Token::Assign");
    let (input, _) = expect_token(Token::Assign)(input)?;
    println!("    ✓ Found '=', remaining: {} chars", input.len());
    println!("    Next 20 chars: {:?}", &input[..20.min(input.len())]);
    
    // Step 4: parse params (many0)
    println!("\n  Step 4: parse params (many0)");
    // For now, just skip params parsing
    let input = input; // No params for our test
    println!("    ✓ Parsed params (none), remaining: {} chars", input.len());
    
    // Step 5: parse block_expr
    println!("\n  Step 5: parse block_expr");
    println!("    About to parse block starting with: {:?}", &input[..30.min(input.len())]);
    
    // We'll stop here since block_expr is complex
    Ok((input, ()))
}

fn main() {
    let tests = vec![
        ("Working", "fun f = { val x = [] (x) match { [a] => { 0 } } }"),
        ("Failing", "fun f = { val x = [] (x) match { [a, b] => { 0 } } }"),
    ];
    
    for (name, input) in tests {
        println!("\n{}", "=".repeat(60));
        println!("=== {} (length {}) ===", name, input.len());
        println!("{}", "=".repeat(60));
        
        match trace_fun_decl(input) {
            Ok((rem, _)) => {
                println!("\n✓ trace_fun_decl succeeded");
                println!("  Remaining: {} chars", rem.len());
            }
            Err(e) => {
                println!("\n✗ trace_fun_decl failed: {:?}", e);
            }
        }
    }
}