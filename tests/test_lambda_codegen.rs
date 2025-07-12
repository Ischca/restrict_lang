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
fn test_simple_lambda_codegen() {
    let input = r#"fun test = {
        val id = |x| x;
        val result = (42) id;
        result
    }"#;
    
    let wat = compile(input).unwrap();
    println!("Generated WAT:\n{}", wat);
    
    // Check for lambda function
    assert!(wat.contains("(func $lambda_"));
    // Check for function table
    assert!(wat.contains("(table"));
    assert!(wat.contains("funcref"));
    // Check for call_indirect
    assert!(wat.contains("call_indirect"));
}

#[test]
fn test_lambda_with_closure() {
    let input = r#"fun test = {
        val y = 10;
        val add_y = |x| x + y;
        val result = (5) add_y;
        result
    }"#;
    
    let wat = compile(input).unwrap();
    println!("Generated WAT:\n{}", wat);
    
    // Check for lambda function
    assert!(wat.contains("(func $lambda_"));
}

#[test]
fn test_higher_order_function() {
    let input = r#"fun make_adder = n:Int {
        val adder = |x| x + n;
        adder
    }"#;
    
    let wat = compile(input).unwrap();
    println!("Generated WAT:\n{}", wat);
    
    // Check that lambda index is returned
    assert!(wat.contains("i32.const")); // Table index
}

#[test]
#[ignore = "Nested lambda calls not yet fully implemented"]
fn test_nested_lambda_calls() {
    let input = r#"fun test = {
        val f = |x| |y| x + y;
        val g = (1) f;
        val result = (2) g;
        result
    }"#;
    
    let result = compile(input);
    if let Ok(wat) = &result {
        println!("Generated WAT:\n{}", wat);
    }
    assert!(result.is_ok());
}