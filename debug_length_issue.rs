use restrict_lang::{lexer::lex_tokens, parse_program};

fn main() {
    // Test different inputs to find the pattern
    let inputs = vec![
        // Working cases
        ("49 chars", "fun f = { val x = [] (x) match { [a] => { 0 } } }"),
        ("literal list", "fun f = { val x = [1, 2] x }"),
        
        // Failing cases
        ("52 chars", "fun f = { val x = [] (x) match { [a, b] => { 0 } } }"),
        ("55 chars", "fun main = { val x = [] (x) match { [a, b] => { 0 } } }"),
        
        // Test variations
        ("no match", "fun f = { val x = [a, b] x }"),
        ("simple list", "fun f = { [a, b] }"),
    ];
    
    println!("=== Testing lexer and parser behavior ===\n");
    
    for (desc, input) in inputs {
        println!("\n{} (length {}):", desc, input.len());
        println!("Input: {:?}", input);
        
        // First test lexer
        match lex_tokens(input) {
            Ok(tokens) => {
                println!("✓ Lexer succeeded: {} tokens", tokens.len());
                // Show first few tokens
                for (i, token) in tokens.iter().take(10).enumerate() {
                    println!("  Token[{}]: {:?}", i, token);
                }
                if tokens.len() > 10 {
                    println!("  ... and {} more tokens", tokens.len() - 10);
                }
            }
            Err(e) => {
                println!("✗ Lexer failed: {}", e);
                continue;
            }
        }
        
        // Then test parser
        match parse_program(input) {
            Ok((rem, prog)) => {
                println!("✓ Parser succeeded: {} declarations, {} chars remaining", 
                         prog.declarations.len(), rem.len());
            }
            Err(e) => {
                println!("✗ Parser failed: {:?}", e);
            }
        }
    }
}