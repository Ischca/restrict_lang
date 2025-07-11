use restrict_lang::{
    lexer::{Token, lex_token},
    parser::top_decl,
};

// Create a wrapper that traces parser calls
fn trace_parser<'a, F, T>(name: &str, parser: F) -> impl Fn(&'a str) -> nom::IResult<&'a str, T>
where
    F: Fn(&'a str) -> nom::IResult<&'a str, T>,
{
    move |input: &'a str| {
        println!(">>> Entering {}", name);
        println!("    Input length: {}", input.len());
        println!("    Input start: {:?}", &input[..20.min(input.len())]);
        
        let result = parser(input);
        
        match &result {
            Ok((remaining, _)) => {
                println!("<<< {} succeeded", name);
                println!("    Consumed: {} chars", input.len() - remaining.len());
                println!("    Remaining: {} chars", remaining.len());
            }
            Err(e) => {
                println!("<<< {} failed", name);
                match e {
                    nom::Err::Error(err) | nom::Err::Failure(err) => {
                        let pos = input.len() - err.input.len();
                        println!("    Failed at position: {}", pos);
                    }
                    _ => {}
                }
            }
        }
        
        result
    }
}

fn main() {
    let input = "fun main = { with Arena { val lst = [1, 2] val result = (lst) match { [a, b] => { a + b } _ => { 0 } } result } }";
    
    println!("=== Testing parser with detailed trace ===\n");
    println!("Full input (length {}): {:?}\n", input.len(), input);
    
    // Manually trace through top_decl
    println!("Calling top_decl...");
    
    // Since we can't easily intercept the alt combinator, let's manually test each option
    let parsers = vec![
        ("record_decl", |i| -> nom::IResult<&str, ()> {
            let (i, _) = expect_token(Token::Record)(i)?;
            Ok((i, ()))
        }),
        ("impl_block", |i| -> nom::IResult<&str, ()> {
            let (i, _) = expect_token(Token::Impl)(i)?;
            Ok((i, ()))
        }),
        ("context_decl", |i| -> nom::IResult<&str, ()> {
            let (i, _) = expect_token(Token::Context)(i)?;
            Ok((i, ()))
        }),
        ("fun_decl", |i| -> nom::IResult<&str, ()> {
            let (i, _) = expect_token(Token::Fun)(i)?;
            Ok((i, ()))
        }),
    ];
    
    for (name, parser) in parsers {
        println!("\nTrying {}:", name);
        match parser(input) {
            Ok((remaining, _)) => {
                println!("  ✓ {} matched", name);
                println!("  Remaining: {} chars", remaining.len());
                if name == "fun_decl" {
                    println!("  This should be the one!");
                }
            }
            Err(_) => {
                println!("  ✗ {} didn't match (expected)", name);
            }
        }
    }
    
    // Now test the actual top_decl
    println!("\n\nActual top_decl call:");
    match top_decl(input) {
        Ok((remaining, _)) => {
            println!("✓ Success! Remaining: {} chars", remaining.len());
        }
        Err(e) => {
            println!("✗ Failed: {:?}", e);
        }
    }
}

fn expect_token<'a>(expected: Token) -> impl Fn(&'a str) -> nom::IResult<&'a str, ()> {
    move |input| {
        let (input, token) = lex_token(input)?;
        if token == expected {
            Ok((input, ()))
        } else {
            Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
        }
    }
}