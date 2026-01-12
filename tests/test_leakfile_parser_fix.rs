use restrict_lang::{parse_program, parser::top_decl, lexer::skip};

#[test]
#[ignore = "TAT (Temporal Affine Types) syntax - deferred to v2.0"]
fn test_parse_leakfile_step_by_step() {
    let full_input = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = {
    val file = File { handle: 1 };
    file
}

fun main: () -> Int = {
    Unit
}"#;
    
    eprintln!("=== Full input ===");
    eprintln!("{}", full_input);
    
    // Step 1: Parse the whole program
    eprintln!("\n=== Parsing full program ===");
    match parse_program(full_input) {
        Ok((remaining, program)) => {
            eprintln!("Parsed {} declarations", program.declarations.len());
            eprintln!("Remaining length: {}", remaining.len());
            eprintln!("Remaining content: {:?}", remaining);
            
            // The test expects all input to be parsed
            assert!(remaining.trim().is_empty(), 
                "Expected all input to be parsed, but {} chars remain", 
                remaining.len());
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            panic!("Failed to parse program");
        }
    }
    
    // Step 2: Test parsing just the function part
    eprintln!("\n=== Testing function parsing alone ===");
    let func_only = r#"fun leakFile<~io> = {
    val file = File { handle: 1 };
    file
}"#;
    
    match parse_program(func_only) {
        Ok((remaining, program)) => {
            eprintln!("Function-only parse successful");
            eprintln!("Declarations: {}", program.declarations.len());
            eprintln!("Remaining: {:?}", remaining);
        }
        Err(e) => {
            eprintln!("Function-only parse failed: {:?}", e);
        }
    }
    
    // Step 3: Test parsing record + function without temporal
    eprintln!("\n=== Testing without temporal types ===");
    let no_temporal = r#"record File {
    handle: Int32
}

fun leakFile = {
    val file = File { handle: 1 };
    file
}"#;
    
    match parse_program(no_temporal) {
        Ok((remaining, program)) => {
            eprintln!("No-temporal parse successful");
            eprintln!("Declarations: {}", program.declarations.len());
            eprintln!("Remaining: {:?}", remaining);
        }
        Err(e) => {
            eprintln!("No-temporal parse failed: {:?}", e);
        }
    }
}