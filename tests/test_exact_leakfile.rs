use restrict_lang::parse_program;

#[test]
fn test_exact_leakfile_issue() {
    // Exact content from the failing test
    let input = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = {
    val file = File { handle: 1 };  // file: File<~io>
    file  // ERROR: Cannot return File<~io> outside ~io
}

fun main = {
    Unit
}"#;
    
    eprintln!("=== Testing exact leakfile content ===");
    eprintln!("Input length: {} chars", input.len());
    
    match parse_program(input) {
        Ok((remaining, program)) => {
            eprintln!("Parsed {} declarations", program.declarations.len());
            eprintln!("Remaining: {} chars", remaining.len());
            
            for (i, decl) in program.declarations.iter().enumerate() {
                match decl {
                    restrict_lang::TopDecl::Function(f) => {
                        eprintln!("Declaration {}: Function '{}'", i, f.name);
                    }
                    restrict_lang::TopDecl::Record(r) => {
                        eprintln!("Declaration {}: Record '{}'", i, r.name);
                    }
                    _ => {
                        eprintln!("Declaration {}: Other", i);
                    }
                }
            }
            
            if !remaining.is_empty() {
                eprintln!("\nRemaining content starts with: {:?}", 
                    &remaining[..40.min(remaining.len())]);
                
                // Now test just the remaining part separately
                eprintln!("\n=== Testing remaining part separately ===");
                match parse_program(remaining) {
                    Ok((rem2, prog2)) => {
                        eprintln!("Remaining part parsed {} declarations", prog2.declarations.len());
                        eprintln!("Still remaining: {} chars", rem2.len());
                    }
                    Err(e) => {
                        eprintln!("Failed to parse remaining: {:?}", e);
                    }
                }
            }
            
            // The test should parse all 3 declarations
            assert_eq!(program.declarations.len(), 3, 
                "Should parse 3 declarations (record, leakFile, main)");
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}