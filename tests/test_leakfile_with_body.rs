use restrict_lang::parse_program;

#[test]
fn test_leakfile_body_variations() {
    // Test 1: Simple body
    let simple_body = r#"fun leakFile<~io> = {
    val file = File { handle: 1 };
    file
}"#;
    
    eprintln!("=== Simple body (no comments) ===");
    match parse_program(simple_body) {
        Ok((rem, prog)) => {
            eprintln!("✓ Success: {} declarations, {} remaining", 
                prog.declarations.len(), rem.len());
        }
        Err(e) => {
            eprintln!("✗ Failed: {:?}", e);
        }
    }
    
    // Test 2: With single comment
    let with_comment = r#"fun leakFile<~io> = {
    val file = File { handle: 1 };  // comment
    file
}"#;
    
    eprintln!("\n=== With single-line comment ===");
    match parse_program(with_comment) {
        Ok((rem, prog)) => {
            eprintln!("✓ Success: {} declarations, {} remaining", 
                prog.declarations.len(), rem.len());
        }
        Err(e) => {
            eprintln!("✗ Failed: {:?}", e);
        }
    }
    
    // Test 3: Original failing case
    let original = r#"fun leakFile<~io> = {
    val file = File { handle: 1 };  // file: File<~io>
    file  // ERROR: Cannot return File<~io> outside ~io
}"#;
    
    eprintln!("\n=== Original with two comments ===");
    match parse_program(original) {
        Ok((rem, prog)) => {
            eprintln!("✓ Success: {} declarations, {} remaining", 
                prog.declarations.len(), rem.len());
        }
        Err(e) => {
            eprintln!("✗ Failed: {:?}", e);
        }
    }
    
    // Test 4: With record before
    let full_test = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = {
    val file = File { handle: 1 };  // file: File<~io>
    file  // ERROR: Cannot return File<~io> outside ~io
}"#;
    
    eprintln!("\n=== Full test with record ===");
    match parse_program(full_test) {
        Ok((rem, prog)) => {
            eprintln!("✓ Success: {} declarations, {} remaining", 
                prog.declarations.len(), rem.len());
            
            if rem.len() > 0 {
                eprintln!("Remaining starts with: {:?}", 
                    &rem[..30.min(rem.len())]);
            }
        }
        Err(e) => {
            eprintln!("✗ Failed: {:?}", e);
        }
    }
}