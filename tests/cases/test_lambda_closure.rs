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
#[ignore = "Closure implementation incomplete - uses non-EBNF v1.0 syntax"]
fn test_simple_closure() {
    let input = r#"fun test: () -> Int32 = {
        val y = 10;
        val add_y: Int32 -> Int32 = |x| x + y;
        val result = 5 |> add_y;
        result
    }"#;

    let result = compile(input);
    if let Ok(wat) = &result {
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
fn test_nested_closure() {
    let input = r#"fun test: () -> Int32 = {
        val x = 1;
        val f: Int32 -> Int32 -> Int32 = |y| {
            val g: Int32 -> Int32 = |z| x + y + z;
            g
        };
        val h = 2 |> f;
        val result = 3 |> h;
        result
    }"#;

    let wat = compile(input).unwrap();
    assert!(wat.contains("(func $lambda_"));
    assert!(wat.contains("offset for captured var"));
    assert!(wat.contains("call_indirect (type $closure_call_1)"));
}

#[test]
#[ignore = "Lambda parameter type inference not yet implemented"]
fn test_lambda_without_captures() {
    let input = r#"fun test: () -> Int32 = {
        val id: Int32 -> Int32 = |x| x;
        val result = 42 |> id;
        result
    }"#;

    let wat = compile(input).unwrap();

    // Non-capturing lambdas still use the uniform closure representation, but
    // the closure only stores the function table index.
    assert!(wat.contains("i32.const 4 ;; closure size"));
    assert!(!wat.contains("offset for captured var"));
}
