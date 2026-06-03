use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile_and_test(source: &str) -> Result<String, String> {
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

    // Generate WASM
    let mut codegen = WasmCodeGen::new();
    codegen
        .generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_integer_pattern_match() {
    let source = r#"
        fun classify: (n: Int32) -> Int32 = {
            n match {
                0 => { 0 }
                1 => { 1 }
                2 => { 2 }
                _ => { 99 }
            }
        }
        
        fun main: () -> Int32 = {
            val a = 0 |> classify;
            val b = 1 |> classify;
            val c = 5 |> classify;
            a + b + c
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $classify"));
}

#[test]
fn test_match_with_variable_binding() {
    let source = r#"
        fun test_binding: (n: Int32) -> Int32 = {
            n match {
                0 => { 0 }
                x => { 42 }
            }
        }
        
        fun main: () -> Int32 = {
            val a = -5 |> test_binding;
            val b = 0 |> test_binding;
            val c = 10 |> test_binding;
            a + b + c
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $test_binding"));
}

#[test]
fn test_match_in_expression_context() {
    let source = r#"
        fun main: () -> Int32 = {
            val result = 42 match {
                0 => { 100 }
                42 => { 200 }
                _ => { 300 }
            };
            result
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.const 42"));
    assert!(wat.contains("i32.const 200"));
}

#[test]
fn test_nested_match() {
    let source = r#"
        fun nested: (x: Int32) -> Int32 = {
            x match {
                0 => { 10 }
                1 => { 20 }
                _ => { 30 }
            }
        }
        
        fun main: () -> Int32 = {
            val a = 0 |> nested;
            val b = 1 |> nested;
            val c = 2 |> nested;
            a + b + c
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $nested"));
}

#[test]
fn test_match_with_complex_expressions() {
    let source = r#"
        fun process: (x: Int32) -> Int32 = {
            val shifted = x + 10;
            shifted match {
                10 => { 0 }
                20 => { 1 }
                30 => { 2 }
                _ => { -1 }
            }
        }
        
        fun main: () -> Int32 = {
            val a = 0 |> process;
            val b = 10 |> process;
            val c = 20 |> process;
            val d = 100 |> process;
            a + b + c + d
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.add"));
}

#[test]
fn test_match_osv_syntax() {
    let source = r#"
        fun main: () -> Int32 = {
            val result = 42 match {
                0 => { 100 }
                42 => { 200 }
                _ => { 300 }
            };
            result
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.const 42"));
    assert!(wat.contains("i32.const 200"));
}

#[test]
fn test_match_with_block_patterns() {
    let source = r#"
        fun complex_match: (n: Int32) -> Int32 = {
            n match {
                0 => { 
                    val a = 10;
                    val b = 20;
                    a + b
                }
                1 => {
                    val x = 100;
                    x * 2
                }
                _ => {
                    999
                }
            }
        }
        
        fun main: () -> Int32 = {
            val a = 0 |> complex_match;
            val b = 1 |> complex_match;
            val c = 2 |> complex_match;
            a + b + c
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $complex_match"));
}

#[test]
fn test_wildcard_pattern() {
    let source = r#"
        fun always_42: (x: Int32) -> Int32 = {
            x match {
                _ => { 42 }
            }
        }
        
        fun main: () -> Int32 = {
            100 |> always_42
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.const 42"));
}

#[test]
fn test_match_exhaustiveness_with_wildcard() {
    let source = r#"
        fun safe_divide: (a: Int32, b: Int32) -> Int32 = {
            b match {
                0 => { 0 }  // Division by zero returns 0
                divisor => { a / divisor }
            }
        }
        
        fun main: () -> Int32 = {
            val a = (10, 2) safe_divide;
            val b = (10, 0) safe_divide;
            a + b
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.div_s"));
}
