use restrict_lang::parse_program;

#[test]
#[ignore = "TAT (Temporal Affine Types) syntax - deferred to v2.0"]
fn test_function_with_comments() {
    // Test with inline comments
    let with_comments = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = {
    val file = File { handle: 1 };  // file: File<~io>
    file  // ERROR: Cannot return File<~io> outside ~io
}"#;
    
    eprintln!("=== Testing with comments ===");
    match parse_program(with_comments) {
        Ok((remaining, program)) => {
            eprintln!("Parsed {} declarations", program.declarations.len());
            eprintln!("Remaining: {} chars", remaining.len());
            if !remaining.trim().is_empty() {
                eprintln!("Remaining content: {:?}", remaining);
            }
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
        }
    }
    
    // Test without comments
    let without_comments = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = {
    val file = File { handle: 1 };
    file
}"#;
    
    eprintln!("\n=== Testing without comments ===");
    match parse_program(without_comments) {
        Ok((remaining, program)) => {
            eprintln!("Parsed {} declarations", program.declarations.len());
            eprintln!("Remaining: {} chars", remaining.len());
            assert_eq!(program.declarations.len(), 2, "Should parse both declarations");
            assert!(remaining.trim().is_empty(), "Should parse all input");
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}