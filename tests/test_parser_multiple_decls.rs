use restrict_lang::parse_program;

#[test]
fn test_simple_two_functions() {
    let input = r#"fun first = {
    42
}

fun second = {
    84
}"#;
    
    match parse_program(input) {
        Ok((remaining, program)) => {
            assert_eq!(program.declarations.len(), 2, "Should parse 2 functions");
            assert!(remaining.trim().is_empty(), "Should parse all input");
        }
        Err(e) => panic!("Parse failed: {:?}", e),
    }
}

#[test]
fn test_record_then_function() {
    let input = r#"record Point {
    x: Int32
    y: Int32
}

fun origin = {
    Point { x: 0, y: 0 }
}"#;
    
    match parse_program(input) {
        Ok((remaining, program)) => {
            assert_eq!(program.declarations.len(), 2, "Should parse 1 record and 1 function");
            assert!(remaining.trim().is_empty(), "Should parse all input");
        }
        Err(e) => panic!("Parse failed: {:?}", e),
    }
}

#[test]
fn test_temporal_record_then_function() {
    let input = r#"record File<~f> {
    handle: Int32
}

fun useFile = {
    Unit
}"#;
    
    match parse_program(input) {
        Ok((remaining, program)) => {
            eprintln!("Parsed {} declarations", program.declarations.len());
            eprintln!("Remaining: {:?}", remaining);
            assert_eq!(program.declarations.len(), 2, "Should parse 1 record and 1 function");
            assert!(remaining.trim().is_empty(), "Should parse all input, but found: {:?}", remaining);
        }
        Err(e) => panic!("Parse failed: {:?}", e),
    }
}