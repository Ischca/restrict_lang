use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile_and_test(source: &str) -> Result<String, String> {
    // Parse
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;

    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;

    // Generate WASM
    let mut codegen = WasmCodeGen::new();
    codegen.generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_integer_pattern_match() {
    let source = r#"
        fun classify: (n: Int) -> Int = {
            n match {
                0 => { 0 }
                1 => { 1 }
                2 => { 2 }
                _ => { 99 }
            }
        }

        fun main: () -> Int = {
            (0) classify |> print_int;
            (1) classify |> print_int;
            (5) classify |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $classify"));
}

#[test]
fn test_match_with_variable_binding() {
    let source = r#"
        fun test_binding: (n: Int) -> Int = {
            n match {
                0 => { 0 }
                x => { 42 }
            }
        }

        fun main: () -> Int = {
            (-5) test_binding |> print_int;
            (0) test_binding |> print_int;
            (10) test_binding |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $test_binding"));
}

#[test]
fn test_match_in_expression_context() {
    let source = r#"
        fun main: () -> Int = {
            val result = 42 match {
                0 => { 100 }
                42 => { 200 }
                _ => { 300 }
            };
            result |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.const 42"));
    assert!(wat.contains("i32.const 200"));
}

#[test]
fn test_nested_match() {
    let source = r#"
        fun nested: (x: Int) -> Int = {
            x match {
                0 => { 10 }
                1 => { 20 }
                _ => { 30 }
            }
        }

        fun main: () -> Int = {
            (0) nested |> print_int;
            (1) nested |> print_int;
            (2) nested |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $nested"));
}

#[test]
fn test_match_with_complex_expressions() {
    let source = r#"
        fun process: (x: Int) -> Int = {
            (x + 10) match {
                10 => { 0 }
                20 => { 1 }
                30 => { 2 }
                _ => { -1 }
            }
        }

        fun main: () -> Int = {
            (0) process |> print_int;
            (10) process |> print_int;
            (20) process |> print_int;
            (100) process |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.add"));
}

#[test]
fn test_match_osv_syntax() {
    let source = r#"
        fun main: () -> Int = {
            42 match {
                0 => { 100 }
                42 => { 200 }
                _ => { 300 }
            } |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.const 42"));
    assert!(wat.contains("i32.const 200"));
}

#[test]
fn test_match_with_block_patterns() {
    let source = r#"
        fun complex_match: (n: Int) -> Int = {
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

        fun main: () -> Int = {
            (0) complex_match |> print_int;
            (1) complex_match |> print_int;
            (2) complex_match |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $complex_match"));
}

#[test]
fn test_wildcard_pattern() {
    let source = r#"
        fun always_42: (x: Int) -> Int = {
            x match {
                _ => { 42 }
            }
        }

        fun main: () -> Int = {
            (100) always_42 |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.const 42"));
}

#[test]
fn test_match_exhaustiveness_with_wildcard() {
    let source = r#"
        fun safe_divide: (a: Int, b: Int) -> Int = {
            b match {
                0 => { 0 }
                divisor => { a / divisor }
            }
        }

        fun main: () -> Int = {
            (10, 2) safe_divide |> print_int;
            (10, 0) safe_divide |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.div_s"));
}
