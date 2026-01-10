use restrict_lang::{parse_program, TypeChecker, generate};

fn compile(source: &str) -> Result<String, String> {
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    
    generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_basic_empty_list_pattern() {
    // Most basic test - empty list - need Arena for list allocation
    let input = "fun main: () -> Int = { with Arena { val x = [] val r = (x) match { [] => { 1 } _ => { 0 } } r } }";
    let wat = compile(input).unwrap();
    assert!(wat.contains("i32.load")); // Should load list length
}

#[test] 
fn test_basic_cons_pattern() {
    // Basic cons pattern without using tail - need Arena for list allocation
    let input = "fun main: () -> Int = { with Arena { val x = [42] val r = (x) match { [] => { 0 } [h | t] => { h } _ => { -1 } } r } }";
    let wat = compile(input).unwrap();
    assert!(wat.contains("i32.gt_s")); // Check for non-empty
}

#[test]
#[ignore = "Parser issue with inline expression"]
fn test_basic_exact_pattern() {
    // Basic exact pattern matching - need Arena for list allocation
    let input = "fun main: () -> Int = { with Arena { val x = [1, 2] val r = (x) match { [a, b] => { a + b } _ => { 0 } } r } }";
    let wat = compile(input).unwrap();
    assert!(wat.contains("i32.const 2")); // Check for length 2
}