use restrict_lang::{parse_program, TypeChecker, generate};

fn compile(source: &str) -> Result<String, String> {
    // Parse
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    
    // Generate code
    generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
#[ignore = "Closure implementation incomplete - uses non-EBNF v1.0 syntax"]
fn test_simple_closure() {
    let input = r#"fun test = {
        val y = 10;
        val add_y = |x| x + y;
        val result = (5) add_y;
        result
    }"#;
    
    let result = compile(input);
    if let Ok(wat) = &result {
        println!("Generated WAT:\n{}", wat);
        
        // Check for closure allocation
        assert!(wat.contains("call $allocate"));
        // Check for captured variable storage
        assert!(wat.contains("i32.store"));
        // Check for closure parameter in lambda
        assert!(wat.contains("(param $closure i32)"));
    } else {
        panic!("Compilation failed: {:?}", result);
    }
}

#[test]
#[ignore = "Complex closure test"]
fn test_nested_closure() {
    let input = r#"fun test = {
        val x = 1;
        val f = |y| {
            val g = |z| x + y + z;
            g
        };
        val h = (2) f;
        val result = (3) h;
        result
    }"#;
    
    let result = compile(input);
    if let Ok(wat) = &result {
        println!("Generated WAT:\n{}", wat);
    }
    assert!(result.is_ok());
}

#[test]
#[ignore = "Lambda type inference needs work - unrelated to function syntax change"]
fn test_lambda_without_captures() {
    let input = r#"fun test = {
        val id = |x| x;
        val result = (42) id;
        result
    }"#;
    
    let wat = compile(input).unwrap();
    println!("Generated WAT:\n{}", wat);
    
    // Should not have closure allocation
    assert!(!wat.contains("(param $closure i32)"));
}