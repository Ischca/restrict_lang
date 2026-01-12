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
fn test_simple_match_some_none() {
    let source = r#"
        fun test_option: (opt: Option<Int>) -> Int = {
            opt match {
                Some(n) => { n }
                None => { 0 }
            }
        }

        fun main: () -> Int = {
            val some_val = 42 some;
            val none_val: Option<Int> = None;
            (some_val) test_option |> print_int;
            (none_val) test_option |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();

    // Verify option constructors
    assert!(wat.contains("Some constructor") || wat.contains("Some"));
    assert!(wat.contains("None constructor") || wat.contains("None"));
}

#[test]
fn test_nested_pattern_match() {
    let source = r#"
        fun test_nested: (opt: Option<Option<Int>>) -> Int = {
            opt match {
                Some(Some(n)) => { n }
                Some(None) => { -1 }
                None => { -2 }
            }
        }

        fun main: () -> Int = {
            val nested = (42 some) some;
            (nested) test_nested
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $test_nested"));
}

#[test]
fn test_list_pattern_matching() {
    let source = r#"
        fun sum_list: (lst: List<Int>) -> Int = {
            lst match {
                [] => { 0 }
                [x] => { x }
                [x, y] => { x + y }
                _ => { 999 }
            }
        }

        fun main: () -> Int = {
            val empty: List<Int> = [];
            val single = [10];
            val double = [20, 30];
            (empty) sum_list |> print_int;
            (single) sum_list |> print_int;
            (double) sum_list |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $sum_list"));
}

#[test]
#[ignore = "Record pattern matching codegen not fully implemented"]
fn test_record_pattern_matching() {
    let source = r#"
        record Point { x: Int y: Int }

        fun is_origin: (p: Point) -> Int = {
            p match {
                Point { x: 0, y: 0 } => { 1 }
                _ => { 0 }
            }
        }

        fun main: () -> Int = {
            val origin = Point { x: 0, y: 0 };
            val p1 = Point { x: 10, y: 20 };

            (origin) is_origin |> print_int;
            (p1) is_origin |> print_int
        }
    "#;

    match compile_and_test(source) {
        Ok(wat) => {
            assert!(wat.contains("(func $is_origin"));
        },
        Err(e) => panic!("Compilation failed: {}", e)
    };
}

#[test]
fn test_wildcard_pattern() {
    let source = r#"
        fun handle_option: (opt: Option<Int>) -> Int = {
            opt match {
                Some(_) => { 1 }
                None => { 0 }
            }
        }

        fun main: () -> Int = {
            val some_val = 42 some;
            val none_val: Option<Int> = None;
            (some_val) handle_option |> print_int;
            (none_val) handle_option |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $handle_option"));
}

#[test]
#[ignore = "Pattern guards not implemented yet"]
fn test_pattern_match_with_guards() {
    let source = r#"
        fun classify: (n: Int) -> Int = {
            n match {
                x then x < 0 => { -1 }
                0 => { 0 }
                x then x > 0 => { 1 }
            }
        }

        fun main: () -> Int = {
            (-10) classify |> print_int;
            (0) classify |> print_int;
            (10) classify |> print_int
        }
    "#;

    let result = compile_and_test(source);
    // Guards are not implemented yet, so this should fail
    assert!(result.is_err());
}

#[test]
fn test_exhaustive_pattern_checking() {
    let source = r#"
        fun incomplete: (opt: Option<Int>) -> Int = {
            opt match {
                Some(n) => { n }
            }
        }

        fun main: () -> Int = {
            (42 some) incomplete
        }
    "#;

    let result = compile_and_test(source);
    // Should fail type checking due to non-exhaustive patterns
    assert!(result.is_err());
}

#[test]
fn test_pattern_binding_affine_types() {
    let source = r#"
        fun test_affine: (opt: Option<Int>) -> Int = {
            opt match {
                Some(n) => { n + n }
                None => { 0 }
            }
        }

        fun main: () -> Int = {
            (42 some) test_affine
        }
    "#;

    let result = compile_and_test(source);
    // Int is Copy, so this should work (not affine violation)
    assert!(result.is_ok());
}

#[test]
#[ignore = "Tuple pattern matching not fully implemented"]
fn test_pattern_match_tuple() {
    let source = r#"
        fun swap: (pair: (Int, Int)) -> (Int, Int) = {
            pair match {
                (x, y) => { (y, x) }
            }
        }

        fun main: () -> Int = {
            val pair = (10, 20);
            (pair) swap match {
                (a, b) => { a + b }
            } |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $swap"));
}

#[test]
#[ignore = "Pattern matching in let bindings not implemented"]
fn test_pattern_match_in_let_binding() {
    let source = r#"
        fun main: () -> Int = {
            val Some(x) = 42 some;
            x |> print_int
        }
    "#;

    let result = compile_and_test(source);
    if result.is_ok() {
        let wat = result.unwrap();
        assert!(wat.contains("local.get $x"));
    }
}

#[test]
fn test_multiple_pattern_variables() {
    let source = r#"
        fun test_list: (lst: List<Int>) -> Int = {
            lst match {
                [a, b, c] => { a + b + c }
                [a, b] => { a + b }
                [a] => { a }
                [] => { 0 }
                _ => { -1 }
            }
        }

        fun main: () -> Int = {
            ([1, 2, 3]) test_list |> print_int;
            ([10, 20]) test_list |> print_int;
            ([100]) test_list |> print_int
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $test_list"));
}

#[test]
fn test_pattern_match_osv_syntax() {
    let source = r#"
        fun process: (x: Int) -> Int = {
            x match {
                0 => { 0 }
                n => { n }
            }
        }

        fun main: () -> Int = {
            (0) process;
            (42) process
        }
    "#;

    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $process"));
}

#[test]
fn test_pattern_match_with_complex_expressions() {
    let source = r#"
        fun complex_match: () -> Int = {
            val opt = (10 + 20) some;
            opt match {
                Some(n) => { n * 2 }
                None => { 0 }
            }
        }

        fun main: () -> Int = {
            complex_match |> print_int
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
        fun test_types: (opt: Option<Int>) -> Int = {
            opt match {
                Some(n) => { n }
                None => { "zero" }
            }
        }

        fun main: () -> Int = {
            (42 some) test_types
        }
    "#;

    let result = compile_and_test(source);
    // Should fail type checking - Int vs String
    assert!(result.is_err());
}
