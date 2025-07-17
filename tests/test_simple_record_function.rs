use restrict_lang::parse_program;

#[test]
fn test_record_then_function_no_newline() {
    // Test without newline after }
    let no_newline = "record A { x: Int32 } fun test = { Unit }";
    
    println!("=== No newline between ===");
    match parse_program(no_newline) {
        Ok((rem, prog)) => {
            println!("Success: {} declarations", prog.declarations.len());
            assert_eq!(prog.declarations.len(), 2);
        }
        Err(e) => {
            panic!("Failed: {:?}", e);
        }
    }
}

#[test]
fn test_record_then_function_with_newline() {
    // Test with newline after }
    let with_newline = r#"record A { x: Int32 }
fun test = { Unit }"#;
    
    println!("\n=== With newline between ===");
    match parse_program(with_newline) {
        Ok((rem, prog)) => {
            println!("Success: {} declarations", prog.declarations.len());
            assert_eq!(prog.declarations.len(), 2);
        }
        Err(e) => {
            panic!("Failed: {:?}", e);
        }
    }
}

#[test]
fn test_record_then_function_double_newline() {
    // Test with double newline
    let double_newline = r#"record A { x: Int32 }

fun test = { Unit }"#;
    
    println!("\n=== With double newline between ===");
    match parse_program(double_newline) {
        Ok((rem, prog)) => {
            println!("Success: {} declarations", prog.declarations.len());
            assert_eq!(prog.declarations.len(), 2);
        }
        Err(e) => {
            panic!("Failed: {:?}", e);
        }
    }
}