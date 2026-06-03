use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

// Test helper to verify recursive function compilation
fn test_recursive_program(input: &str, expected_function: &str) {
    let (remaining, program) = parse_program(input).expect("Should parse recursive program");
    assert!(remaining.trim().is_empty());

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("Should type check recursive program");

    let mut codegen = WasmCodeGen::new();
    let wat_code = codegen
        .generate(&program)
        .expect("Should generate WASM for recursive program");

    assert!(
        wat_code.contains(&format!("(func ${}", expected_function)),
        "Should contain function definition"
    );
    assert!(
        wat_code.contains(&format!("call ${}", expected_function)),
        "Should contain recursive call"
    );
}

#[test]
fn test_simple_factorial_recursive_function() {
    let input = r#"
    fun factorial: (n: Int32) -> Int32 = {
        n <= 1 then {
            1
        } else {
            val next = n - 1;
            val partial = next |> factorial;
            n * partial
        }
    }

    fun main: () -> Int32 = {
        5 |> factorial
    }"#;

    test_recursive_program(input, "factorial");
}

#[test]
fn test_fibonacci_recursive_function() {
    let input = r#"
    fun fibonacci: (n: Int32) -> Int32 = {
        n <= 1 then {
            n
        } else {
            val left = (n - 1) |> fibonacci;
            val right = (n - 2) |> fibonacci;
            left + right
        }
    }

    fun main: () -> Int32 = {
        10 |> fibonacci
    }"#;

    test_recursive_program(input, "fibonacci");

    // Additional check for multiple calls
    let (remaining, program) = parse_program(input).unwrap();
    assert!(remaining.trim().is_empty());
    let mut checker = TypeChecker::new();
    checker.check_program(&program).unwrap();
    let mut codegen = WasmCodeGen::new();
    let wat_code = codegen.generate(&program).unwrap();

    let call_count = wat_code.matches("call $fibonacci").count();
    assert!(
        call_count >= 2,
        "Fibonacci should have at least 2 recursive calls, found {}",
        call_count
    );
}

#[test]
fn test_mutually_recursive_functions() {
    let input = r#"
    fun is_even: (n: Int32) -> Boolean = {
        n == 0 then {
            true
        } else {
            val next = n - 1;
            next |> is_odd
        }
    }

    fun is_odd: (n: Int32) -> Boolean = {
        n == 0 then {
            false
        } else {
            val next = n - 1;
            next |> is_even
        }
    }

    fun main: () -> Boolean = {
        10 |> is_even
    }"#;

    let (remaining, program) =
        parse_program(input).expect("Should parse mutually recursive program");
    assert!(remaining.trim().is_empty());

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("Should type check mutual recursion");

    let mut codegen = WasmCodeGen::new();
    let wat_code = codegen
        .generate(&program)
        .expect("Should generate WASM for mutual recursion");

    // Check that both functions are defined and call each other
    assert!(wat_code.contains("(func $is_even"));
    assert!(wat_code.contains("(func $is_odd"));
    assert!(wat_code.contains("call $is_odd"));
    assert!(wat_code.contains("call $is_even"));
}

#[test]
fn test_tail_recursive_function() {
    let input = r#"
    fun factorial_tail: (n: Int32, acc: Int32) -> Int32 = {
        n <= 1 then {
            acc
        } else {
            val next = n - 1;
            val next_acc = acc * n;
            (next, next_acc) factorial_tail
        }
    }

    fun factorial: (n: Int32) -> Int32 = {
        (n, 1) factorial_tail
    }

    fun main: () -> Int32 = {
        5 |> factorial
    }"#;

    test_recursive_program(input, "factorial_tail");

    // Additional check for both functions
    let (remaining, program) = parse_program(input).unwrap();
    assert!(remaining.trim().is_empty());
    let mut checker = TypeChecker::new();
    checker.check_program(&program).unwrap();
    let mut codegen = WasmCodeGen::new();
    let wat_code = codegen.generate(&program).unwrap();

    assert!(wat_code.contains("(func $factorial"));
    assert!(wat_code.contains("call $factorial_tail"));
}

#[test]
fn test_recursive_with_complex_types() {
    let input = r#"
    fun list_length: (lst: List<Int32>) -> Int32 = {
        lst match {
            [] => { 0 }
            [head | tail] => {
                val rest_len = tail |> list_length;
                1 + rest_len
            }
        }
    }

    fun main: () -> Int32 = {
        val mylist = [1, 2, 3, 4, 5];
        mylist |> list_length
    }"#;

    test_recursive_program(input, "list_length");
}

#[test]
fn test_deeply_nested_recursion() {
    // Test that recursive functions with many parameters work correctly
    let input = r#"
    fun ackermann: (m: Int32, n: Int32) -> Int32 = {
        m == 0 then {
            n + 1
        } else {
            n == 0 then {
                (m - 1, 1) ackermann
            } else {
                val inner = (m, n - 1) ackermann;
                (m - 1, inner) ackermann
            }
        }
    }

    fun main: () -> Int32 = {
        (2, 3) ackermann
    }"#;

    test_recursive_program(input, "ackermann");

    // Additional check for multiple recursive calls in ackermann
    let (remaining, program) = parse_program(input).unwrap();
    assert!(remaining.trim().is_empty());
    let mut checker = TypeChecker::new();
    checker.check_program(&program).unwrap();
    let mut codegen = WasmCodeGen::new();
    let wat_code = codegen.generate(&program).unwrap();

    let call_count = wat_code.matches("call $ackermann").count();
    assert!(
        call_count >= 2,
        "Ackermann should have at least 2 recursive calls, found {}",
        call_count
    );
}
