use restrict_lang::parser::top_decl;

#[test]
#[ignore = "TAT (Temporal Affine Types) syntax - deferred to v2.0"]
fn test_top_decl_on_function() {
    let func_input = r#"fun leakFile<~io> = {
    val file = File { handle: 1 };
    file
}"#;
    
    eprintln!("Testing top_decl on function:");
    eprintln!("{}", func_input);
    
    match top_decl(func_input) {
        Ok((remaining, decl)) => {
            eprintln!("Success! Parsed declaration");
            eprintln!("Remaining: {:?}", remaining);
            match decl {
                restrict_lang::TopDecl::Function(f) => {
                    eprintln!("Parsed function: {}", f.name);
                }
                _ => {
                    eprintln!("Parsed other declaration type");
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to parse: {:?}", e);
            
            // Try to understand what went wrong
            if let nom::Err::Error(err) = &e {
                eprintln!("Error at input position: {:?}", err.input.chars().take(30).collect::<String>());
                eprintln!("Error kind: {:?}", err.code);
            }
            
            panic!("top_decl failed on function declaration");
        }
    }
}

#[test]
fn test_top_decl_simple_function() {
    let simple_func = "fun test: () -> Int = { 42 }";
    
    eprintln!("\nTesting top_decl on simple function:");
    eprintln!("{}", simple_func);
    
    match top_decl(simple_func) {
        Ok((remaining, decl)) => {
            eprintln!("Success! Remaining: {:?}", remaining);
        }
        Err(e) => {
            eprintln!("Failed: {:?}", e);
            panic!("Failed to parse simple function");
        }
    }
}