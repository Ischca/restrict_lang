use restrict_lang::parse_program;

#[test]
fn test_minimal_leakfile() {
    // Start with the absolute minimal version
    let minimal = "fun leakFile = { Unit }";
    
    eprintln!("=== Minimal (no type params) ===");
    match parse_program(minimal) {
        Ok((rem, prog)) => {
            eprintln!("✓ Success: {} declarations", prog.declarations.len());
        }
        Err(e) => {
            eprintln!("✗ Failed: {:?}", e);
        }
    }
    
    // Add temporal type parameter
    let with_temporal = "fun leakFile<~io> = { Unit }";
    
    eprintln!("\n=== With temporal type param ===");
    match parse_program(with_temporal) {
        Ok((rem, prog)) => {
            eprintln!("✓ Success: {} declarations", prog.declarations.len());
        }
        Err(e) => {
            eprintln!("✗ Failed: {:?}", e);
        }
    }
    
    // With record before it
    let with_record = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = { Unit }"#;
    
    eprintln!("\n=== With record before ===");
    match parse_program(with_record) {
        Ok((rem, prog)) => {
            eprintln!("✓ Success: {} declarations", prog.declarations.len());
            eprintln!("Remaining: {} chars", rem.len());
            
            if !rem.trim().is_empty() {
                eprintln!("Remaining content: {:?}", rem);
                
                // Check what character is at the break point
                let parsed_len = with_record.len() - rem.len();
                if parsed_len > 0 {
                    let before_char = with_record.chars().nth(parsed_len - 1);
                    let after_char = with_record.chars().nth(parsed_len);
                    eprintln!("Character before break: {:?}", before_char);
                    eprintln!("Character at break: {:?}", after_char);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed: {:?}", e);
        }
    }
}