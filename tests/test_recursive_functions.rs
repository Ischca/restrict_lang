use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

// Test helper to verify recursive function compilation
fn test_recursive_program(input: &str, expected_function: &str) {
    let (_, program) = parse_program(input)
        .expect("Should parse recursive program");
    
    let mut checker = TypeChecker::new();
    checker.check_program(&program)
        .expect("Should type check recursive program");
    
    let mut codegen = WasmCodeGen::new();
    codegen.expr_types = checker.expr_types.clone();
    let wat_code = codegen.generate(&program)
        .expect("Should generate WASM for recursive program");
    
    println!("Generated WAT for {}:\n{}", expected_function, wat_code);
    
    assert!(wat_code.contains(&format!("(func ${}", expected_function)),
        "Should contain function definition");
    assert!(wat_code.contains(&format!("call ${}", expected_function)),
        "Should contain recursive call");
}

#[test]
fn test_simple_factorial_recursive_function() {
    let input = r#"
    fun factorial = n: Int32 {
        if n <= 1 then 1
        else n * factorial(n - 1)
    }

    fun main() {
        factorial(5) |> println;
    }"#;
    
    test_recursive_program(input, "factorial");
}

#[test]  
fn test_fibonacci_recursive_function() {
    let input = r#"
    fun fibonacci = n: Int32 {
        if n <= 1 then n
        else fibonacci(n - 1) + fibonacci(n - 2)
    }

    fun main() {
        fibonacci(10) |> println;
    }"#;
    
    test_recursive_program(input, "fibonacci");
    
    // Additional check for multiple calls
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    checker.check_program(&program).unwrap();
    let mut codegen = WasmCodeGen::new();
    codegen.expr_types = checker.expr_types.clone();
    let wat_code = codegen.generate(&program).unwrap();
    
    let call_count = wat_code.matches("call $fibonacci").count();
    assert!(call_count >= 2, "Fibonacci should have at least 2 recursive calls, found {}", call_count);
}

#[test]
fn test_mutually_recursive_functions() {
    let input = r#"
    fun is_even = n: Int32 {
        if n == 0 then true
        else is_odd(n - 1)
    }
    
    fun is_odd = n: Int32 {
        if n == 0 then false
        else is_even(n - 1)
    }

    fun main() {
        is_even(10) |> println;
    }"#;
    
    let (_, program) = parse_program(input).expect("Should parse mutually recursive program");
    
    let mut checker = TypeChecker::new();
    checker.check_program(&program).expect("Should type check mutual recursion");
    
    let mut codegen = WasmCodeGen::new();
    codegen.expr_types = checker.expr_types.clone();
    let wat_code = codegen.generate(&program).expect("Should generate WASM for mutual recursion");
    
    // Check that both functions are defined and call each other
    assert!(wat_code.contains("(func $is_even"));
    assert!(wat_code.contains("(func $is_odd"));
    assert!(wat_code.contains("call $is_odd"));
    assert!(wat_code.contains("call $is_even"));
}

#[test]
fn test_tail_recursive_function() {
    let input = r#"
    fun factorial_tail = n: Int32, acc: Int32 {
        if n <= 1 then acc
        else factorial_tail(n - 1, acc * n)
    }
    
    fun factorial = n: Int32 {
        factorial_tail(n, 1)
    }

    fun main() {
        factorial(5) |> println;
    }"#;
    
    test_recursive_program(input, "factorial_tail");
    
    // Additional check for both functions
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    checker.check_program(&program).unwrap();
    let mut codegen = WasmCodeGen::new();
    codegen.expr_types = checker.expr_types.clone();
    let wat_code = codegen.generate(&program).unwrap();
    
    assert!(wat_code.contains("(func $factorial"));
    assert!(wat_code.contains("call $factorial_tail"));
}

#[test]
fn test_recursive_with_complex_types() {
    let input = r#"
    fun list_length = lst: List<Int32> {
        match lst {
            [] => 0,
            [head | tail] => 1 + list_length(tail)
        }
    }

    fun main() {
        val mylist = [1, 2, 3, 4, 5];
        list_length(mylist) |> println;
    }"#;
    
    test_recursive_program(input, "list_length");
}

#[test]
fn test_deeply_nested_recursion() {
    // Test that recursive functions with many parameters work correctly
    let input = r#"
    fun ackermann = m: Int32, n: Int32 {
        if m == 0 then n + 1
        else if n == 0 then ackermann(m - 1, 1)
        else ackermann(m - 1, ackermann(m, n - 1))
    }

    fun main() {
        ackermann(2, 3) |> println;
    }"#;
    
    test_recursive_program(input, "ackermann");
    
    // Additional check for multiple recursive calls in ackermann
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    checker.check_program(&program).unwrap();
    let mut codegen = WasmCodeGen::new();
    codegen.expr_types = checker.expr_types.clone();
    let wat_code = codegen.generate(&program).unwrap();
    
    let call_count = wat_code.matches("call $ackermann").count();
    assert!(call_count >= 2, "Ackermann should have at least 2 recursive calls, found {}", call_count);
}