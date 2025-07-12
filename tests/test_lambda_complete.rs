use restrict_lang::{parse_program, TypeChecker};

fn check_and_print(name: &str, input: &str) {
    println!("\n=== {} ===", name);
    println!("Input: {}", input);
    
    match parse_program(input) {
        Ok((_, program)) => {
            println!("Parse: OK");
            println!("AST: {:#?}", program);
            
            let mut checker = TypeChecker::new();
            match checker.check_program(&program) {
                Ok(()) => println!("Type Check: OK"),
                Err(e) => println!("Type Check Error: {}", e),
            }
        }
        Err(e) => println!("Parse Error: {:?}", e),
    }
}

#[test]
fn test_lambda_examples() {
    // Simple identity function
    check_and_print("Identity", "val id = |x| x");
    
    // Addition function
    check_and_print("Addition", "val add = |x, y| x + y");
    
    // Curried addition
    check_and_print("Curried Add", "val curry_add = |x| |y| x + y");
    
    // Lambda with block body
    check_and_print("Block Body", r#"val compute = |x| {
        val doubled = x * 2;
        val result = doubled + 1;
        result
    }"#);
    
    // Lambda application with OSV syntax
    check_and_print("Application", r#"fun test = {
        val add = |x, y| x + y;
        val result = (5, 10) add;
        result
    }"#);
    
    // Higher-order function (returns a function)
    check_and_print("Higher Order", r#"fun make_adder = n:Int {
        val adder = |x| x + n;
        adder
    }"#);
    
    // Lambda capturing variables (closure)
    check_and_print("Closure", r#"fun test = {
        val x = 10;
        val add_x = |y| x + y;
        val result = (5) add_x;
        result
    }"#);
    
    // Nested lambdas
    check_and_print("Nested", r#"fun test = {
        val f = |x| |y| |z| x + y + z;
        val g = (1) f;
        val h = (2) g;
        val result = (3) h;
        result
    }"#);
}

#[test]
fn test_lambda_type_inference() {
    // Currently we assume all parameters are Int32
    // This test documents the current behavior
    let input = r#"fun test = {
        val id = |x| x;
        val num = (42) id;
        val add = |x, y| x + y;
        val sum = (10, 20) add;
        num + sum
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_lambda_affine_semantics() {
    // Test that lambdas follow affine type rules for captured variables
    
    // This should work - x is used only once (by the lambda)
    let ok_input = r#"fun test = {
        val x = 10;
        val f = |y| x + y;
        f
    }"#;
    
    let (_, program) = parse_program(ok_input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
    
    // This should fail - x is used twice
    let fail_input = r#"fun test = {
        val x = 10;
        val f = |y| x + y;
        val g = |z| x + z;
        f
    }"#;
    
    let (_, program) = parse_program(fail_input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_err());
}