use restrict_lang::{parse_program, TypeChecker, TypeError};

fn type_check_str(input: &str) -> Result<(), TypeError> {
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    checker.check_program(&program)
}

#[test]
fn test_simple_lambda_type_check() {
    let input = "val f = |x| x + 1";
    assert!(type_check_str(input).is_ok());
}

#[test]
fn test_lambda_with_multiple_params() {
    let input = "val add = |x, y| x + y";
    assert!(type_check_str(input).is_ok());
}

#[test]
fn test_lambda_no_params() {
    let input = "val constant = || 42";
    assert!(type_check_str(input).is_ok());
}

#[test]
fn test_nested_lambda() {
    let input = "val curry_add = |x| |y| x + y";
    assert!(type_check_str(input).is_ok());
}

#[test]
fn test_lambda_with_block() {
    let input = r#"val compute = |x| {
        val doubled = x * 2;
        val result = doubled + 1;
        result
    }"#;
    assert!(type_check_str(input).is_ok());
}

#[test]
fn test_lambda_in_function() {
    let input = r#"fun test = {
        val add_one = |x| x + 1;
        val result = 5;
        result
    }"#;
    assert!(type_check_str(input).is_ok());
}

#[test]
fn test_lambda_application() {
    // Due to OSV syntax, add(5, 10) is parsed as 10(add, 5)
    // So we need to use the pipe syntax or parentheses
    let input = r#"fun test = {
        val add = |x, y| x + y;
        val result = (5, 10) add;
        result
    }"#;
    assert!(type_check_str(input).is_ok());
}

#[test]
fn test_lambda_captures_variable() {
    let input = r#"fun test = {
        val x = 10;
        val add_x = |y| x + y;
        val result = 5;
        result
    }"#;
    assert!(type_check_str(input).is_ok());
}

#[test]
fn test_lambda_affine_types() {
    let input = r#"fun test = {
        val x = 10;
        val use_x = |y| x + y;
        // x should be consumed by the lambda
        val another = |z| x + z;  // This should fail
        42
    }"#;
    let result = type_check_str(input);
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, TypeError::AffineViolation(_)));
    }
}