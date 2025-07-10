use restrict_lang::{lex, parse_program};
use std::fs;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <source_file>", args[0]);
        std::process::exit(1);
    }
    
    let filename = &args[1];
    let source = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file {}: {}", filename, e);
            std::process::exit(1);
        }
    };
    
    // Lex the source
    println!("=== Lexing ===");
    let _tokens = match lex(&source) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() {
                eprintln!("Warning: Unparsed input remaining: {:?}", remaining);
            }
            println!("Tokens: {:?}", tokens);
            tokens
        },
        Err(e) => {
            eprintln!("Lexing error: {:?}", e);
            std::process::exit(1);
        }
    };
    
    // Parse the source
    println!("\n=== Parsing ===");
    match parse_program(&source) {
        Ok((remaining, ast)) => {
            if !remaining.is_empty() {
                eprintln!("Warning: Unparsed input remaining: {:?}", remaining);
            }
            println!("AST: {:#?}", ast);
        },
        Err(e) => {
            eprintln!("Parsing error: {:?}", e);
            std::process::exit(1);
        }
    }
}