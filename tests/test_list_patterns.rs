use restrict_lang::{parse_program, TypeChecker, generate};

fn compile(source: &str) -> Result<String, String> {
    // Parse
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    
    // Generate code
    generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_list_pattern_empty() {
    let input = "fun main = { with Arena { val lst = [] val result = (lst) match { [] => { 42 } _ => { 0 } } result } }";
    let wat = compile(input).unwrap();
    assert!(wat.contains("match"));
    assert!(wat.contains("i32.load")); // Check for length load
    assert!(wat.contains("i32.const 0")); // Check for empty comparison
}

#[test]
fn test_list_pattern_exact() {
    let input = "fun main = { with Arena { val lst = [1, 2, 3] val result = (lst) match { [] => { 0 } [a] => { a } [a, b] => { a + b } [a, b, c] => { a + b + c } _ => { -1 } } result } }";
    let wat = compile(input).unwrap();
    assert!(wat.contains("i32.load")); // Check for length load
    assert!(wat.contains("i32.const 3")); // Check for length comparison
}

#[test]
fn test_list_pattern_cons() {
    let input = "fun main = { with Arena { val lst = [10, 20, 30] val result = (lst) match { [] => { 0 } [head | tail] => { head + list_length(tail) } _ => { -1 } } result } }";
    let wat = compile(input).unwrap();
    assert!(wat.contains("i32.gt_s")); // Check for non-empty test
    assert!(wat.contains("memory.copy")); // Check for tail copy
}

#[test]
fn test_list_pattern_nested() {
    let input = "fun sum_list = lst: List<Int> { (lst) match { [] => { 0 } [head | tail] => { head + sum_list(tail) } _ => { 0 } } } fun main = { with Arena { sum_list([1, 2, 3, 4, 5]) } }";
    let wat = compile(input).unwrap();
    assert!(wat.contains("sum_list"));
    assert!(wat.contains("i32.gt_s")); // Check for non-empty test
}

#[test]
fn test_list_pattern_multiple_elements() {
    let input = "fun main = { with Arena { val lst = [1, 2, 3, 4] val result = (lst) match { [a, b] => { a + b } [a, b, c] => { a + b + c } [a, b, c, d] => { a + b + c + d } _ => { 0 } } result } }";
    let wat = compile(input).unwrap();
    assert!(wat.contains("i32.const 4")); // Check for length comparison
}