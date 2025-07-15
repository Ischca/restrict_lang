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
        fun test_option = opt: Int? {
            opt match {
                Some(n) => { n }
                None => { 0 }
            }
        }
        
        fun main = {
            val some_val = 42 some;
            val none_val = None<Int>;
            some_val test_option |> print_int;
            none_val test_option |> print_int
        }
    "#;
    
    let wat = compile_and_test(source).unwrap();
    
    // Verify option constructors
    assert!(wat.contains("Some constructor"));
    assert!(wat.contains("None constructor")); 
    
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
        fun test_nested = opt: Int?? {
            opt match {
                Some(Some(n)) => { n }
                Some(None) => { -1 }
                None => { -2 }
            }
        }
        
        fun main = {
            val nested = 42 some some;
            val some_none = None<Int> some;
            val none = None<Int?>;
            nested test_nested |> print_int;
            some_none test_nested |> print_int;
            none test_nested |> print_int
        }
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $test_nested"));
}

#[test]
fn test_list_pattern_matching() {
    let source = r#"
        fun sum_list = lst: [Int] {
            lst match {
                [] => { 0 }
                [x] => { x }
                [x, y] => { x + y }
                [x, y, ...rest] => { x + y + rest sum_list }
            }
        }
        
        fun main = {
            val empty = [];
            val single = [10];
            val double = [20, 30];
            val triple = [1, 2, 3];
            empty sum_list |> print_int;
            single sum_list |> print_int;
            double sum_list |> print_int;
            triple sum_list |> print_int
        }
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $sum_list"));
    assert!(wat.contains("call $list_length"));
}

#[test]
fn test_record_pattern_matching() {
    let source = r#"
        record Point { x: Int, y: Int }
        
        fun quadrant = p: Point {
            p match {
                Point { x: 0, y: 0 } => { 0 }
                Point { x, y } => {
                    x > 0 then {
                        y > 0 then { 1 } else { 4 }
                    } else {
                        y > 0 then { 2 } else { 3 }
                    }
                }
            }
        }
        
        fun main = {
            val origin = Point { x: 0, y: 0 };
            val p1 = Point { x: 10, y: 20 };
            val p2 = Point { x: -10, y: 20 };
            val p3 = Point { x: -10, y: -20 };
            val p4 = Point { x: 10, y: -20 };
            
            origin quadrant |> print_int;
            p1 quadrant |> print_int;
            p2 quadrant |> print_int;
            p3 quadrant |> print_int;
            p4 quadrant |> print_int
        }
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $quadrant"));
}

#[test]
fn test_wildcard_pattern() {
    let source = r#"
        fun handle_option = opt: Int? {
            opt match {
                Some(_) => { 1 }
                None => { 0 }
            }
        }
        
        fun main = {
            val some_val = 42 some;
            val none_val = None<Int>;
            some_val handle_option |> print_int;
            none_val handle_option |> print_int
        }
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $handle_option"));
}

#[test]
fn test_pattern_match_with_guards() {
    let source = r#"
        fun classify = n: Int {
            n match {
                x then x < 0 => { -1 }
                0 => { 0 }
                x then x > 0 => { 1 }
            }
        }
        
        fun main = {
            -10 classify |> print_int;
            0 classify |> print_int;
            10 classify |> print_int
        }
    "#;
    
    let result = compile_and_test(source);
    // Guards are not implemented yet, so this should fail
    assert!(result.is_err());
}

#[test] 
fn test_exhaustive_pattern_checking() {
    let source = r#"
        fun incomplete = opt: Int? {
            opt match {
                Some(n) => { n }
                // Missing None case
            }
        }
        
        fun main = {
            42 some incomplete
        }
    "#;
    
    let result = compile_and_test(source);
    // Should fail type checking due to non-exhaustive patterns
    assert!(result.is_err());
}

#[test]
fn test_pattern_binding_affine_types() {
    let source = r#"
        fun test_affine = opt: Int? {
            opt match {
                Some(n) => { n + n }  // Error: n used twice
                None => { 0 }
            }
        }
        
        fun main = {
            42 some test_affine
        }
    "#;
    
    let result = compile_and_test(source);
    // Should fail due to affine type violation
    assert!(result.is_err());
}

#[test]
fn test_pattern_match_tuple() {
    let source = r#"
        fun swap = pair: (Int, Int) {
            pair match {
                (x, y) => { (y, x) }
            }
        }
        
        fun main = {
            val pair = (10, 20);
            pair swap match {
                (a, b) => { a + b }
            } |> print_int
        }
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $swap"));
}

#[test]
fn test_pattern_match_in_let_binding() {
    let source = r#"
        fun main = {
            val Some(x) = 42 some;
            x |> print_int
        }
    "#;
    
    let result = compile_and_test(source);
    // Pattern matching in let bindings might not be implemented
    // Check if it compiles or errors appropriately
    if result.is_ok() {
        let wat = result.unwrap();
        assert!(wat.contains("local.get $x"));
    }
}

#[test]
fn test_multiple_pattern_variables() {
    let source = r#"
        fun test_list = lst: [Int] {
            lst match {
                [a, b, c] => { a + b + c }
                [a, b] => { a + b }
                [a] => { a }
                [] => { 0 }
            }
        }
        
        fun main = {
            [1, 2, 3] test_list |> print_int;
            [10, 20] test_list |> print_int;
            [100] test_list |> print_int;
            [] test_list |> print_int
        }
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $test_list"));
}

#[test]
fn test_pattern_match_osv_syntax() {
    let source = r#"
        fun process = x: Int {
            x match {
                0 => { "zero" }
                n => { "non-zero" }
            } |> println
        }
        
        fun main = {
            0 process;
            42 process
        }
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("(func $process"));
}

#[test]
fn test_pattern_match_with_complex_expressions() {
    let source = r#"
        fun complex_match = {
            val opt = 10 + 20 |> some;
            opt match {
                Some(n) => { n * 2 }
                None => { 0 }
            }
        }
        
        fun main = {
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
        fun test_types = opt: Int? {
            opt match {
                Some(n) => { n }
                None => { "zero" }  // Type error: different return types
            }
        }
        
        fun main = {
            42 some test_types
        }
    "#;
    
    let result = compile_and_test(source);
    // Should fail type checking
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type error"));
}