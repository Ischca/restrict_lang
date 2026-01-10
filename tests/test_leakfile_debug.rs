use restrict_lang::{parse_program, lex};

#[test]
fn test_leakfile_parsing() {
    let input = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = {
    val file = File { handle: 1 };
    file
}

fun main: () -> Int = {
    Unit
}"#;
    
    // First, test lexing
    println!("=== Testing Lexer ===");
    match lex(input) {
        Ok((remaining, tokens)) => {
            println!("Lexing successful!");
            println!("Token count: {}", tokens.len());
            println!("Remaining after lex: {:?}", remaining);
            
            // Print first 10 tokens
            for (i, token) in tokens.iter().take(10).enumerate() {
                println!("Token {}: {:?}", i, token);
            }
        }
        Err(e) => {
            panic!("Lexing failed: {:?}", e);
        }
    }
    
    // Then test parsing
    println!("\n=== Testing Parser ===");
    match parse_program(input) {
        Ok((remaining, program)) => {
            println!("Parsing partially successful!");
            println!("Declarations parsed: {}", program.declarations.len());
            println!("Remaining length: {}", remaining.len());
            
            for (i, decl) in program.declarations.iter().enumerate() {
                match decl {
                    restrict_lang::TopDecl::Function(f) => {
                        println!("Declaration {}: Function '{}'", i, f.name);
                    }
                    restrict_lang::TopDecl::Record(r) => {
                        println!("Declaration {}: Record '{}'", i, r.name);
                    }
                    _ => {
                        println!("Declaration {}: Other", i);
                    }
                }
            }
            
            if !remaining.trim().is_empty() {
                println!("\nUnparsed content:");
                println!("{}", remaining);
                
                // Try to parse just the function
                println!("\n=== Trying to parse just the function ===");
                let func_only = "fun leakFile<~io> = {
    val file = File { handle: 1 };
    file
}";
                match parse_program(func_only) {
                    Ok((rem, prog)) => {
                        println!("Function-only parse: {} declarations", prog.declarations.len());
                        println!("Remaining: {:?}", rem);
                    }
                    Err(e) => {
                        println!("Function-only parse failed: {:?}", e);
                    }
                }
            }
        }
        Err(e) => {
            panic!("Parsing failed: {:?}", e);
        }
    }
}