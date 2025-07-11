use restrict_lang::{parse_program, TypeChecker};

fn type_check_program(source: &str) -> Result<(), String> {
    // Parse
    let (remaining, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input: '{}'", remaining));
    }
    
    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))
}

#[test]
fn test_list_pattern_parse() {
    let input = r#"
        fun test = {
            val lst = []
            lst match {
                [] => 42
                _ => 0
            }
        }
    "#;
    
    match type_check_program(input) {
        Ok(()) => println!("Success!"),
        Err(e) => panic!("Failed: {}", e),
    }
}