use restrict_lang::{
    lexer::{Token, lex_token},
    parser::top_decl,
    ast::TopDecl,
};
use nom::{IResult, branch::alt, combinator::map};

// Recreate the alt structure from top_decl
fn debug_top_decl(input: &str) -> IResult<&str, String> {
    println!(">>> Entering debug_top_decl");
    println!("    Input: {:?}", &input[..20.min(input.len())]);
    
    // Test each alternative individually first
    println!("\n  Testing Token::Fun:");
    match expect_token(Token::Fun)(input) {
        Ok((rem, _)) => println!("    ✓ Fun token found, remaining: {} chars", rem.len()),
        Err(_) => println!("    ✗ No Fun token"),
    }
    
    println!("\n  Testing Token::Record:");
    match expect_token(Token::Record)(input) {
        Ok((rem, _)) => println!("    ✓ Record token found, remaining: {} chars", rem.len()),
        Err(_) => println!("    ✗ No Record token"),
    }
    
    // Now test the alt combinator
    println!("\n  Testing alt combinator:");
    let result = alt((
        map(expect_token(Token::Fun), |_| "fun".to_string()),
        map(expect_token(Token::Record), |_| "record".to_string()),
        map(expect_token(Token::Impl), |_| "impl".to_string()),
        map(expect_token(Token::Context), |_| "context".to_string()),
    ))(input);
    
    match &result {
        Ok((rem, which)) => {
            println!("    ✓ alt succeeded with: {}", which);
            println!("    Remaining: {} chars", rem.len());
        }
        Err(e) => {
            println!("    ✗ alt failed: {:?}", e);
        }
    }
    
    result
}

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

fn main() {
    let tests = vec![
        ("Working", "fun f = { val x = [] (x) match { [a] => { 0 } } }"),
        ("Failing", "fun f = { val x = [] (x) match { [a, b] => { 0 } } }"),
    ];
    
    for (name, input) in tests {
        println!("\n=== {} (length {}) ===", name, input.len());
        
        // Test our debug version
        match debug_top_decl(input) {
            Ok(_) => println!("Debug version succeeded"),
            Err(_) => println!("Debug version failed"),
        }
        
        // Test the real top_decl
        println!("\nTesting real top_decl:");
        match top_decl(input) {
            Ok((rem, _)) => println!("✓ Real top_decl succeeded, {} chars remaining", rem.len()),
            Err(e) => println!("✗ Real top_decl failed: {:?}", e),
        }
    }
}