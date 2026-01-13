use restrict_lang::{parse_program, TypeChecker};

#[test]
fn test_generic_identity_function() {
    // Test that generic type parameter is inferred from argument
    let input = r#"fun identity<T> = x: T {
        x
    }
    
    fun test = {
        val int_result = (42) identity;
        val string_result = ("hello") identity;
        val list_result = ([1, 2, 3]) identity;
        list_result
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}

#[test]
#[ignore = "Generic type inference needs improvements - uses non-EBNF v1.0 syntax"]
fn test_generic_pair_function() {
    // Test that multiple generic type parameters are inferred
    let input = r#"record Pair<A, B> { first: A second: B }

    fun make_pair<A, B> = a: A b: B {
        Pair { first: a second: b }
    }
    
    fun test = {
        val int_string_pair = (42, "hello") make_pair;
        val bool_list_pair = (true, [1, 2, 3]) make_pair;
        int_string_pair
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}

#[test]
fn test_generic_map_function() {
    // Test generic function with function parameter
    let input = r#"fun map<T, U> = lst: List<T> f: T -> U {
        lst match {
            [] => { [] }
            [head | tail] => { 
                val new_head = (head) f;
                val new_tail = (tail, f) map;
                [new_head | new_tail]
            }
            _ => { [] }
        }
    }
    
    fun test = {
        val numbers = [1, 2, 3];
        val double = |x| x * 2;
        val doubled = (numbers, double) map;
        doubled
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}

#[test]
fn test_generic_option_functions() {
    // Test generic functions with Option types
    let input = r#"fun unwrap_or<T> = opt: Option<T> default: T {
        opt match {
            Some(value) => { value }
            None => { default }
        }
    }
    
    fun test = {
        val some_int = 42 some;
        val none_int = None<Int>;
        
        val result1 = (some_int, 0) unwrap_or;
        val result2 = (none_int, 99) unwrap_or;
        
        result1 + result2
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}

#[test]
fn test_generic_compose_function() {
    // Test higher-order generic function composition
    let input = r#"fun compose<A, B, C> = f: B -> C g: A -> B {
        |x| (x) g |> f
    }
    
    fun test = {
        val add_one = |x| x + 1;
        val double = |x| x * 2;
        val add_then_double = (double, add_one) compose;
        
        val result = (5) add_then_double;  // (5 + 1) * 2 = 12
        result
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}