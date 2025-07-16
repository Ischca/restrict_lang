use restrict_lang::{parse_program, TypeChecker};

#[test]
fn test_empty_list_inference_from_binding() {
    // Test that empty list type is inferred from binding type annotation
    let input = r#"fun test = {
        val numbers: List<Float> = [];
        numbers
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}

#[test]
fn test_empty_list_inference_from_function_param() {
    // Test that empty list type is inferred from function parameter type
    let input = r#"fun process_floats = lst: List<Float> {
        lst
    }
    
    fun test = {
        val result = ([]) process_floats;
        result
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}

#[test]
fn test_empty_array_inference_from_binding() {
    // Test that empty array type is inferred from binding type annotation
    let input = r#"fun test = {
        val arr: Array<String, 0> = array();
        arr
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}

#[test]
fn test_empty_list_in_match_arm() {
    // Test that empty list type is inferred in match arms
    let input = r#"fun test = lst: List<String> {
        lst match {
            [head | tail] => { tail }
            [] => { [] }  // Should infer as List<String> from previous arm
            _ => { [] }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}

#[test]
fn test_empty_list_in_some() {
    // Test that empty list type is inferred inside Option
    let input = r#"fun test = {
        val maybe: Option<List<Bool>> = Some([]);
        maybe
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type check failed: {:?}", e),
    }
}