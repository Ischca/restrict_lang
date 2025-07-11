use restrict_lang::{lexer::lex_token, parser::top_decl};
use nom::{IResult, branch::alt, combinator::map};

fn main() {
    // These inputs differ only in the list pattern
    let working = "fun f = { val x = [] (x) match { [a] => { 0 } } }";
    let failing = "fun f = { val x = [] (x) match { [a, b] => { 0 } } }";
    
    println!("Working input length: {}", working.len());
    println!("Failing input length: {}", failing.len());
    
    // Test if it's related to input length
    for i in 40..60 {
        let test_input = format!("fun f = {{ {} }}", "x".repeat(i - 13));
        match top_decl(&test_input) {
            Ok(_) => print!("✓"),
            Err(_) => print!("✗"),
        }
        if i % 10 == 0 {
            println!(" (length {})", i);
        }
    }
    println!();
    
    // Test the actual cases
    println!("\nTesting actual inputs:");
    
    println!("\nWorking case:");
    match top_decl(working) {
        Ok((rem, _)) => println!("✓ Success, {} chars remaining", rem.len()),
        Err(e) => {
            println!("✗ Failed: {:?}", e);
            if let nom::Err::Error(ref err) = e {
                println!("  Error input: {:?}", &err.input[..20.min(err.input.len())]);
            }
        }
    }
    
    println!("\nFailing case:");
    match top_decl(failing) {
        Ok((rem, _)) => println!("✓ Success, {} chars remaining", rem.len()),
        Err(e) => {
            println!("✗ Failed: {:?}", e);
            if let nom::Err::Error(ref err) = e {
                println!("  Error input: {:?}", &err.input[..20.min(err.input.len())]);
                println!("  Failed after consuming: {} chars", failing.len() - err.input.len());
            }
        }
    }
    
    // Check if the issue is specific to the pattern
    let variations = vec![
        ("Empty list", "fun f = { val x = [] (x) match { [] => { 0 } } }"),
        ("Single var", "fun f = { val x = [] (x) match { [a] => { 0 } } }"),
        ("Two vars", "fun f = { val x = [] (x) match { [a, b] => { 0 } } }"),
        ("Three vars", "fun f = { val x = [] (x) match { [a, b, c] => { 0 } } }"),
        ("Long name", "fun main = { val x = [] (x) match { [a, b] => { 0 } } }"),
    ];
    
    println!("\nTesting variations:");
    for (desc, input) in variations {
        match top_decl(input) {
            Ok(_) => println!("✓ {} (length {})", desc, input.len()),
            Err(_) => println!("✗ {} (length {})", desc, input.len()),
        }
    }
}