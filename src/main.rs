use restrict_lang::{lex, parse_program, TypeChecker, WasmCodeGen, ModuleResolver};
use std::fs;
use std::env;
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use restrict_lang::lsp;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} [OPTIONS] <source_file> [output_file]", args[0]);
        eprintln!("Options:");
        eprintln!("  --check       Type check only (no code generation)");
        eprintln!("  --ast         Show AST only (no compilation)");
        eprintln!("  --lsp         Start Language Server Protocol mode");
        eprintln!("  --help        Show this help message");
        std::process::exit(1);
    }
    
    // Parse command line arguments
    let mut check_only = false;
    let mut show_ast = false;
    let mut lsp_mode = false;
    let mut source_file = String::new();
    let mut output_file = None;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--check" => check_only = true,
            "--ast" => show_ast = true,
            "--lsp" => lsp_mode = true,
            "--help" => {
                eprintln!("Usage: {} [OPTIONS] <source_file> [output_file]", args[0]);
                eprintln!("Options:");
                eprintln!("  --check       Type check only (no code generation)");
                eprintln!("  --ast         Show AST only (no compilation)");
                eprintln!("  --lsp         Start Language Server Protocol mode");
                eprintln!("  --help        Show this help message");
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
    if !check_only && !show_ast {
        println!("=== Lexing ===");
    }
    let _tokens = match lex(&source) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() && !check_only && !show_ast {
                eprintln!("Warning: Lexer unparsed input remaining: {:?}", remaining);
                eprintln!("This might indicate a lexer issue.");
            }
            if !check_only && !show_ast {
                println!("Tokens: {:?}", tokens);
            }
            tokens
        },
        Err(e) => {
            eprintln!("Lexing error: {:?}", e);
            std::process::exit(1);
        }
    };
    
    // Parse the source
    if !check_only && !show_ast {
        println!("\n=== Parsing ===");
    }
    let ast = match parse_program(&source) {
        Ok((remaining, ast)) => {
            if !remaining.is_empty() && !check_only && !show_ast {
                eprintln!("Warning: Unparsed input remaining at position {}", source.len() - remaining.len());
                
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
                    std::process::exit(1);
                }
            }
            if show_ast {
                println!("{:#?}", ast);
                return; // Exit after showing AST
            }
            if !check_only {
                println!("AST: {:#?}", ast);
            }
            ast
        },
        Err(e) => {
            match e {
                nom::Err::Error(e) | nom::Err::Failure(e) => {
                    let pos = source.len() - e.input.len();
                    eprintln!("Parsing error at position {}: {:?}", pos, e.code);
                    
                    // Show context
                    let start = pos.saturating_sub(40);
                    let end = (pos + 40).min(source.len());
                    eprintln!("Context:");
                    eprintln!("  {}", &source[start..pos]);
                    eprintln!("  {}^--- Error here", " ".repeat(pos - start));
                    eprintln!("  {}", &source[pos..end]);
                }
                nom::Err::Incomplete(_) => {
                    eprintln!("Parsing error: Incomplete input");
                }
            }
            std::process::exit(1);
        }
    };

    // Process imports
    let mut imported_decls = Vec::new();
    if !ast.imports.is_empty() {
        if !check_only && !show_ast {
            println!("\n=== Resolving Imports ===");
        }

        let mut resolver = ModuleResolver::new();

        // Add search paths
        // 1. Directory containing the source file
        if let Some(parent) = Path::new(filename).parent() {
            resolver.add_search_path(parent.to_path_buf());
        }
        // 2. Standard library path (relative to current dir)
        resolver.add_search_path(PathBuf::from("std"));

        for import in &ast.imports {
            match resolver.resolve_module(&import.module_path) {
                Ok(_) => {
                    match resolver.get_imported_items(&import.module_path, &import.items) {
                        Ok(items) => {
                            if !check_only {
                                println!("  Imported {} items from {}",
                                    items.len(),
                                    import.module_path.join("."));
                            }
                            imported_decls.extend(items);
                        }
                        Err(e) => {
                            eprintln!("Import error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Module resolution error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    // Type check
    if !check_only {
        println!("\n=== Type Checking ===");
    }
    let mut type_checker = TypeChecker::new();

    // Register imported declarations first
    for (name, decl) in &imported_decls {
        if let Err(e) = type_checker.register_imported_decl(name, decl) {
            eprintln!("Error registering import '{}': {}", name, e);
            std::process::exit(1);
        }
    }

    match type_checker.check_program(&ast) {
        Ok(()) => {
            if check_only {
                // For --check mode, just exit successfully after type checking
                return;
            }
            println!("Type checking passed!");
        },
        Err(e) => {
            eprintln!("Type error: {}", e);
            std::process::exit(1);
        }
    }
    
    // Generate WASM
    println!("\n=== WASM Code Generation ===");
    let mut codegen = WasmCodeGen::new();

    // Register imported declarations with codegen
    for (_name, decl) in &imported_decls {
        codegen.register_imported_decl(decl);
    }

    let wat = match codegen.generate(&ast) {
        Ok(wat) => {
            println!("WASM generation successful!");
            wat
        },
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
        },
        Err(e) => {
            eprintln!("Error writing output file {}: {}", output_filename, e);
            std::process::exit(1);
        }
    }
}