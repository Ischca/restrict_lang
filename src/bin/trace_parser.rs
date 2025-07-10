use std::env;
use std::fs;
use restrict_lang::parser::parse_program;
fn main() {
    // Enable nom tracing
    nom_trace::activate_trace!();
    
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: trace_parser <file.rl>");
        std::process::exit(1);
    }

    let filename = &args[1];
    let content = fs::read_to_string(filename)
        .expect(&format!("Failed to read file: {}", filename));

    println!("=== Source Code ===");
    println!("{}", content);
    println!();

    // Parse with tracing enabled
    println!("=== Parsing with Trace ===");
    match parse_program(&content) {
        Ok((remaining, ast)) => {
            println!("\n=== Result ===");
            println!("AST: {:#?}", ast);
            println!("Remaining input: {:?}", remaining);
            if !remaining.is_empty() {
                println!("WARNING: Not all input was consumed!");
            }
        }
        Err(e) => {
            eprintln!("\n=== Error ===");
            eprintln!("Parsing error: {:?}", e);
        }
    }
}