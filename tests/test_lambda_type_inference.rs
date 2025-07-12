use restrict_lang::{parse_program, TypeChecker};

#[test]
fn test_lambda_param_inference_from_body() {
    // Test that lambda parameter types can be inferred from usage in body
    let input = r#"fun test = {
        val add_one = |x| x + 1;
        val result = (41) add_one;
        result
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_lambda_param_inference_from_application() {
    // Test that lambda parameter types can be inferred from application context
    let input = r#"fun apply_to_int = f:Int->Int, x:Int {
        val result = (x) f;
        result
    }
    
    fun test = {
        val double = |x| x * 2;
        val result = apply_to_int(double, 21);
        result
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_nested_lambda_type_inference() {
    // Test type inference for nested lambdas
    let input = r#"fun test = {
        val add = |x| |y| x + y;
        val add5 = (5) add;
        val result = (10) add5;
        result
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_lambda_in_option_context() {
    // Test that lambda types are inferred in Option context
    let input = r#"fun test = {
        val maybe_add = Some(|x| x + 1);
        maybe_add
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_lambda_param_inference_comparison() {
    // Test that comparison operators help infer numeric types
    let input = r#"fun test = {
        val is_positive = |x| x > 0;
        val result = (42) is_positive;
        result
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
#[ignore = "Float inference not yet implemented"]
fn test_lambda_float_inference() {
    // Test inference of Float64 from literal
    let input = r#"fun test = {
        val add_pi = |x| x + 3.14;
        val result = (1.0) add_pi;
        result
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}