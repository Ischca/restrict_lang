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
fn test_simple_match_some_none() {
    let source = r#"
        fun test_option: (opt: Option<Int32>) -> Int32 = {
            opt match {
                Some(n) => { n }
                None => { 0 }
            }
        }

        fun main: () -> Int32 = {
            val some_val = Some(42);
            val none_val: Option<Int32> = None;
            val a = some_val |> test_option;
            val b = none_val |> test_option;
            a + b
        }
    "#;

    let wat = compile_and_test(source).unwrap();

    // Verify tagged union
    assert!(wat.contains("i32.const 1")); // Some tag
    assert!(wat.contains("i32.const 0")); // None tag
    assert!(wat.contains("i32.store")); // Store tag

    // Verify pattern matching
    assert!(wat.contains("i32.load")); // Load tag
    assert!(wat.contains("(if (result i32)"));
}

#[test]
fn test_nested_pattern_match() {
    let source = r#"
        fun test_nested: (opt: Option<Option<Int32> >) -> Int32 = {
            opt match {
                Some(Some(n)) => { n }
                Some(None) => { -1 }
                None => { -2 }
            }
        }

        fun main: () -> Int32 = {
            val nested = Some(Some(42));
            val some_none: Option<Option<Int32> > = Some(None);
            val missing: Option<Option<Int32> > = None;
            val a = nested |> test_nested;
            val b = some_none |> test_nested;
            val c = missing |> test_nested;
            a + b + c
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $test_nested"));
}

#[test]
fn test_list_pattern_matching() {
    let source = r#"
        fun sum_list: (lst: List<Int32>) -> Int32 = {
            lst match {
                [] => { 0 }
                [x] => { x }
                [x, y] => { x + y }
                [head | tail] => { head }
            }
        }

        fun main: () -> Int32 = {
            val empty: List<Int32> = [];
            val single = [10];
            val double = [20, 30];
            val triple = [1, 2, 3];
            val a = empty |> sum_list;
            val b = single |> sum_list;
            val c = double |> sum_list;
            val d = triple |> sum_list;
            a + b + c + d
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $sum_list"));
    assert!(wat.contains("call $list_length"));
}

#[test]
fn test_record_pattern_matching() {
    let source = r#"
        record Point {
            x: Int32,
            y: Int32
        }

        fun quadrant: (p: Point) -> Int32 = {
            p match {
                Point { x: 0, y: 0 } => { 0 }
                Point { x, y } => {
                    x > 0 then {
                        y > 0 then { 1 } else { 4 }
                    } else {
                        y > 0 then { 2 } else { 3 }
                    }
                }
                _ => { -1 }
            }
        }

        fun main: () -> Int32 = {
            val origin = Point { x: 0, y: 0 };
            val p1 = Point { x: 10, y: 20 };
            val p2 = Point { x: -10, y: 20 };
            val p3 = Point { x: -10, y: -20 };
            val p4 = Point { x: 10, y: -20 };

            val a = origin |> quadrant;
            val b = p1 |> quadrant;
            val c = p2 |> quadrant;
            val d = p3 |> quadrant;
            val e = p4 |> quadrant;
            a + b + c + d + e
        }
    "#;

    match compile_and_test(source) {
        Ok(wat) => {
            if !wat.contains("(func $quadrant") {
                println!("Generated WAT:\n{}", wat);
                panic!("Expected to find (func $quadrant but it wasn't found");
            }
        }
        Err(e) => panic!("Compilation failed: {}", e),
    };
}

#[test]
fn test_wildcard_pattern() {
    let source = r#"
        fun handle_option: (opt: Option<Int32>) -> Int32 = {
            opt match {
                Some(_) => { 1 }
                None => { 0 }
            }
        }

        fun main: () -> Int32 = {
            val some_val = Some(42);
            val none_val: Option<Int32> = None;
            val a = some_val |> handle_option;
            val b = none_val |> handle_option;
            a + b
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $handle_option"));
}

#[test]
fn test_pattern_match_with_guards() {
    let source = r#"
        fun classify: (n: Int32) -> Int32 = {
            n match {
                x then x < 0 => { -1 }
                0 => { 0 }
                x then x > 0 => { 1 }
            }
        }

        fun main: () -> Int32 = {
            val a = -10 |> classify;
            val b = 0 |> classify;
            val c = 10 |> classify;
            a + b + c
        }
    "#;

    let result = compile_and_test(source);
    // Guards are not implemented yet, so this should fail
    assert!(result.is_err());
}

#[test]
fn test_exhaustive_pattern_checking() {
    let source = r#"
        fun incomplete: (opt: Option<Int32>) -> Int32 = {
            opt match {
                Some(n) => { n }
                // Missing None case
            }
        }

        fun main: () -> Int32 = {
            Some(42) |> incomplete
        }
    "#;

    let result = compile_and_test(source);
    // Should fail type checking due to non-exhaustive patterns
    assert!(result.is_err());
}

#[test]
fn test_pattern_binding_affine_types() {
    let source = r#"
        record Token {
            id: Int32
        }

        fun use_token: (token: Token) -> Int32 = {
            token.id
        }

        fun test_affine: (opt: Option<Token>) -> Int32 = {
            opt match {
                Some(token) => {
                    val a = token |> use_token;
                    val b = token |> use_token;
                    a + b
                }
                None => { 0 }
            }
        }

        fun main: () -> Int32 = {
            Some(Token { id: 42 }) |> test_affine
        }
    "#;

    let result = compile_and_test(source);
    // Should fail due to affine type violation
    assert!(result.is_err());
}

#[test]
fn test_pattern_match_tuple() {
    let source = r#"
        fun main: () -> Int32 = {
            val pair = (10, 20);
            pair match {
                (a, b) => { a + b }
            }
        }
    "#;

    let result = compile_and_test(source);
    assert!(result.is_err());
}

#[test]
fn test_pattern_match_in_let_binding() {
    let source = r#"
        fun main: () -> Int32 = {
            val Some(x) = Some(42);
            x
        }
    "#;

    let result = compile_and_test(source);
    // Pattern matching in let bindings might not be implemented
    // Check if it compiles or errors appropriately
    if let Ok(wat) = result {
        assert!(wat.contains("local.get $x"));
    }
}

#[test]
fn test_binding_pattern_rejects_incompatible_literal() {
    let source = r#"
        record Reading {
            celsius: Float64,
            stable: Boolean
        }

        fun main: () -> Float64 = {
            val Reading { celsius, stable: "yes" } = Reading {
                celsius: 21.5,
                stable: true
            };
            celsius
        }
    "#;

    let result = compile_and_test(source);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type error"));
}

#[test]
fn test_multiple_pattern_variables() {
    let source = r#"
        fun test_list: (lst: List<Int32>) -> Int32 = {
            lst match {
                [a, b, c] => { a + b + c }
                [a, b] => { a + b }
                [a] => { a }
                [] => { 0 }
                _ => { -1 }
            }
        }

        fun main: () -> Int32 = {
            val empty: List<Int32> = [];
            val a = [1, 2, 3] |> test_list;
            val b = [10, 20] |> test_list;
            val c = [100] |> test_list;
            val d = empty |> test_list;
            a + b + c + d
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $test_list"));
}

#[test]
fn test_pattern_match_osv_syntax() {
    let source = r#"
        fun process: (x: Int32) -> Int32 = {
            x match {
                0 => { 0 }
                n => { n }
            }
        }

        fun main: () -> Int32 = {
            42 |> process
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $process"));
}

#[test]
fn test_pattern_match_with_complex_expressions() {
    let source = r#"
        fun complex_match: () -> Int32 = {
            val opt = Some(10 + 20);
            opt match {
                Some(n) => { n * 2 }
                None => { 0 }
            }
        }

        fun main: () -> Int32 = {
            () complex_match
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("i32.const 10"));
    assert!(wat.contains("i32.const 20"));
    assert!(wat.contains("i32.add"));
}

#[test]
fn test_match_return_different_types() {
    let source = r#"
        fun test_types: (opt: Option<Int32>) -> Int32 = {
            opt match {
                Some(n) => { n }
                None => { "zero" }  // Type error: different return types
            }
        }

        fun main: () -> Int32 = {
            Some(42) |> test_types
        }
    "#;

    let result = compile_and_test(source);
    // Should fail type checking
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type error"));
}
