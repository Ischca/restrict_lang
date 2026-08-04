use restrict_lang::diagnostics::{format_lex_error, format_parse_error};
use restrict_lang::ir::builder::build_checked_ir;
use restrict_lang::module::resolve_program_imports_for_file;
use restrict_lang::{
    check_release_surface, lex, parse_program, HostAbiProfile, TypeChecker, WasmCodeGen,
};
use std::env;
use std::fs;
use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
use restrict_lang::lsp;

const BIN_NAME: &str = env!("CARGO_PKG_NAME");

fn usage_text() -> String {
    format!(
        "\
Usage: {BIN_NAME} [OPTIONS] <source_file> [output_file]
Options:
  --version     Show compiler version
  --check       Check imports, types, and the selected host ABI surface without code generation
  --host-abi <PROFILE>
                Select host ABI profile: v0.0.1 (default) or flat-record-v1
  --ast         Show AST only (no compilation)
  --verbose     Show lexing, parsing, and codegen progress details
  --lsp         Start Language Server Protocol mode
  --help        Show this help message
"
    )
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprint!("{}", usage_text());
        std::process::exit(1);
    }

    // Parse command line arguments
    let mut check_only = false;
    let mut show_ast = false;
    let mut lsp_mode = false;
    let mut verbose = false;
    let mut host_abi_profile = HostAbiProfile::V001Scalar;
    let mut source_file = String::new();
    let mut output_file = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--version" => {
                println!("{BIN_NAME} {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--check" => check_only = true,
            "--host-abi" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--host-abi requires a value; expected 'v0.0.1' or 'flat-record-v1'");
                    std::process::exit(1);
                };
                if value.starts_with("--") {
                    eprintln!("--host-abi requires a value; expected 'v0.0.1' or 'flat-record-v1'");
                    std::process::exit(1);
                }
                host_abi_profile = match value.parse() {
                    Ok(profile) => profile,
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(1);
                    }
                };
                i += 1;
            }
            "--ast" => show_ast = true,
            "--verbose" => verbose = true,
            "--lsp" => lsp_mode = true,
            "--help" => {
                print!("{}", usage_text());
                std::process::exit(0);
            }
            arg => {
                if source_file.is_empty() {
                    source_file = arg.to_string();
                } else if output_file.is_none() {
                    output_file = Some(arg.to_string());
                } else {
                    eprintln!("Too many arguments");
                    std::process::exit(1);
                }
            }
        }
        i += 1;
    }

    if lsp_mode {
        #[cfg(not(target_arch = "wasm32"))]
        {
            lsp::start_lsp_server().await;
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            eprintln!("Language Server Protocol mode not supported on WASM");
            std::process::exit(1);
        }
    }

    if source_file.is_empty() {
        eprintln!("No source file specified");
        std::process::exit(1);
    }

    let filename = &source_file;
    let source = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file {}: {}", filename, e);
            std::process::exit(1);
        }
    };

    // Lex the source
    if verbose && !show_ast {
        println!("=== Lexing ===");
    }
    let _tokens = match lex(&source) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() && verbose && !show_ast {
                eprintln!("Warning: Lexer unparsed input remaining: {:?}", remaining);
                eprintln!("This might indicate a lexer issue.");
            }
            if verbose && !show_ast {
                println!("Tokens: {:?}", tokens);
            }
            tokens
        }
        Err(e) => {
            eprintln!("{}", format_lex_error(&source, e));
            std::process::exit(1);
        }
    };

    // Parse the source
    if verbose && !show_ast {
        println!("\n=== Parsing ===");
    }
    let ast = match parse_program(&source) {
        Ok((remaining, ast)) => {
            if !remaining.trim().is_empty() {
                eprintln!(
                    "Error: Unparsed input remaining at position {}",
                    source.len() - remaining.len()
                );

                // Show context around the unparsed position
                let pos = source.len() - remaining.len();
                let start = pos.saturating_sub(40);
                let end = (pos + 40).min(source.len());

                eprintln!("Context:");
                eprintln!("  {}", &source[start..pos]);
                eprintln!("  {}^--- Parsing stopped here", " ".repeat(pos - start));
                eprintln!("  {}", &source[pos..end]);

                // If AST is empty, this is likely a complete parse failure
                if ast.declarations.is_empty() && ast.imports.is_empty() {
                    eprintln!("\nError: Failed to parse any declarations. Check syntax around the indicated position.");
                }
                std::process::exit(1);
            }
            if show_ast {
                println!("{:#?}", ast);
                return; // Exit after showing AST
            }
            if verbose {
                println!("AST: {:#?}", ast);
            }
            ast
        }
        Err(e) => {
            eprintln!("{}", format_parse_error(&source, e));
            std::process::exit(1);
        }
    };

    let ast = match resolve_program_imports_for_file(ast, Path::new(filename)) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Import resolution error: {}", e);
            std::process::exit(1);
        }
    };

    // Type check
    if verbose {
        println!("\n=== Type Checking ===");
    }
    let mut type_checker = TypeChecker::new();
    match type_checker.check_program(&ast) {
        Ok(()) => {
            if let Err(e) = check_release_surface(&ast, &type_checker, host_abi_profile) {
                eprintln!("Release surface error: {}", e);
                std::process::exit(1);
            }
            if check_only {
                println!("OK {}", filename);
                return;
            }
            if verbose {
                println!("Type checking passed!");
            }
        }
        Err(e) => {
            eprintln!("Type error: {}", e);
            std::process::exit(1);
        }
    }

    let checked_ir = match build_checked_ir(&ast, &type_checker) {
        Ok(checked_ir) => checked_ir,
        Err(_) => {
            eprintln!("Code generation error: Invalid checked compiler state: construction failed");
            std::process::exit(1);
        }
    };

    // Generate WASM
    if verbose {
        println!("\n=== WASM Code Generation ===");
    }
    let mut codegen = WasmCodeGen::with_host_abi_profile(host_abi_profile);
    let wat = match codegen.generate_checked(&ast, &checked_ir) {
        Ok(wat) => {
            if verbose {
                println!("WASM generation successful!");
            }
            wat
        }
        Err(e) => {
            eprintln!("Code generation error: {}", e);
            std::process::exit(1);
        }
    };

    // Write output
    let output_filename = output_file.unwrap_or_else(|| {
        Path::new(filename)
            .with_extension("wat")
            .to_str()
            .unwrap()
            .to_string()
    });

    match fs::write(&output_filename, wat) {
        Ok(()) => {
            println!("\n✓ Successfully compiled to {}", output_filename);
        }
        Err(e) => {
            eprintln!("Error writing output file {}: {}", output_filename, e);
            std::process::exit(1);
        }
    }
}
