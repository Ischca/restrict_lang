use restrict_lang::parse_program;

#[test]
fn test_record_then_simple_function() {
    let input = r#"record A {
    x: Int32
}

fun test = {
    Unit
}"#;
    
    eprintln!("Input:\n{}", input);
    
    match parse_program(input) {
        Ok((remaining, program)) => {
            eprintln!("\nParsed {} declarations", program.declarations.len());
            eprintln!("Remaining: {} chars", remaining.len());
            
            if !remaining.is_empty() {
                eprintln!("Remaining content: {:?}", remaining);
                
                // Try to understand where parsing stopped
                let parsed_len = input.len() - remaining.len();
                eprintln!("Parsed up to position {}", parsed_len);
                eprintln!("Context: ...{}[STOPPED HERE]{}", 
                    &input[parsed_len.saturating_sub(10)..parsed_len],
                    &remaining[..20.min(remaining.len())]);
            }
            
            assert_eq!(program.declarations.len(), 2, "Should parse record and function");
            assert!(remaining.trim().is_empty(), "Should parse all input");
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}

#[test]
fn test_record_with_temporal_then_function() {
    let input = r#"record File<~f> {
    handle: Int32
}

fun test = {
    Unit
}"#;
    
    eprintln!("\nInput with temporal:\n{}", input);
    
    match parse_program(input) {
        Ok((remaining, program)) => {
            eprintln!("\nParsed {} declarations", program.declarations.len());
            eprintln!("Remaining: {} chars", remaining.len());
            
            if !remaining.is_empty() {
                eprintln!("Remaining content: {:?}", remaining);
            }
            
            assert_eq!(program.declarations.len(), 2, "Should parse record and function");
            assert!(remaining.trim().is_empty(), "Should parse all input");
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}