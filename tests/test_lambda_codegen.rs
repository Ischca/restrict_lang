use restrict_lang::{generate, parse_program, TypeChecker};

fn compile(source: &str) -> Result<String, String> {
    // Parse
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;

    // Generate code
    generate(&ast).map_err(|e| format!("Codegen error: {}", e))
}

#[test]
#[ignore = "Uses non-EBNF v1.0 syntax"]
fn test_simple_lambda_codegen() {
    let input = r#"fun test: () -> Int32 = {
        val id: Int32 -> Int32 = |x| x;
        val result = 42 |> id;
        result
    }"#;

    let wat = compile(input).unwrap();
    // Check for lambda function
    assert!(wat.contains("(func $lambda_"));
    // Check for function table
    assert!(wat.contains("(table"));
    assert!(wat.contains("funcref"));
    // Check for call_indirect
    assert!(wat.contains("call_indirect"));
}

#[test]
#[ignore = "Uses non-EBNF v1.0 syntax"]
fn test_lambda_with_closure() {
    let input = r#"fun test: () -> Int32 = {
        val y = 10;
        val add_y: Int32 -> Int32 = |x| x + y;
        val result = 5 |> add_y;
        result
    }"#;

    let wat = compile(input).unwrap();
    // Check for lambda function
    assert!(wat.contains("(func $lambda_"));
}

#[test]
fn test_float_function_value_call_codegen() {
    let input = r#"fun test: () -> Float64 = {
        val scale: Float64 -> Float64 = |x| x + 1.5;
        val result = 2.0 |> scale;
        result
    }"#;

    let wat = compile(input).expect("Float64 function value call should compile");

    assert!(wat.contains("closure_call_1_f64_to_f64"));
    assert!(wat.contains("call_indirect (type $closure_call_1_f64_to_f64)"));
}

#[test]
fn test_higher_order_function() {
    let input = r#"fun make_adder: (n: Int32) -> Int32 -> Int32 = {
        val adder: Int32 -> Int32 = |x| x + n;
        adder
    }"#;

    let wat = compile(input).unwrap();
    // Check that lambda index is returned
    assert!(wat.contains("i32.const")); // Table index
}

#[test]
fn test_nested_lambda_calls() {
    let input = r#"fun test: () -> Int32 = {
        val f: Int32 -> Int32 -> Int32 = |x| |y| x + y;
        val g = 1 |> f;
        val result = 2 |> g;
        result
    }"#;

    let wat = compile(input).unwrap();
    assert!(wat.contains("(func $lambda_"));
    assert!(wat.contains("call_indirect (type $closure_call_1)"));
}
