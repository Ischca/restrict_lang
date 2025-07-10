use std::env;
use std::fs;
use restrict_lang::lexer::lex;
use restrict_lang::parser::parse_program;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: debug_parser <file.rl>");
        std::process::exit(1);
    }

    let filename = &args[1];
    let content = fs::read_to_string(filename)
        .expect(&format!("Failed to read file: {}", filename));

    println!("=== Source Code ===");
    println!("{}", content);
    println!();

    // First, test lexing
    println!("=== Tokenization ===");
    match lex(&content) {
        Ok((remaining, tokens)) => {
            println!("Tokens: {:?}", tokens);
            println!("Remaining input: {:?}", remaining);
            println!();
        }
        Err(e) => {
            eprintln!("Lexing error: {:?}", e);
            std::process::exit(1);
        }
    }

    // Then test parsing
    println!("=== Parsing ===");
    match parse_program(&content) {
        Ok((remaining, ast)) => {
            println!("AST: {:#?}", ast);
            println!("Remaining input: {:?}", remaining);
            if !remaining.is_empty() {
                println!("WARNING: Not all input was consumed!");
            }
        }
        Err(e) => {
            eprintln!("Parsing error: {:?}", e);
            
            // Try to provide more detailed error info
            if let nom::Err::Error(err) = &e {
                eprintln!("Error at input: {:?}", err.input);
            }
        }
    }
}