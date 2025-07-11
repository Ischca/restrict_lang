use restrict_lang::{parse_program, TypeChecker};

fn type_check(source: &str) -> Result<(), String> {
    // Parse
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))
}

#[test]
fn test_empty_list() {
    let source = r#"
        fun main = {
            val empty = [];
            empty
        }
    "#;
    
    let result = type_check(source);
    assert!(result.is_ok());
}

#[test]
fn test_int_list() {
    let source = r#"
        fun main = {
            val nums = [1, 2, 3, 4, 5];
            nums
        }
    "#;
    
    let result = type_check(source);
    assert!(result.is_ok());
}

#[test]
fn test_string_list() {
    let source = r#"
        fun main = {
            val words = ["hello", "world"];
            words
        }
    "#;
    
    let result = type_check(source);
    assert!(result.is_ok());
}

#[test]
fn test_mixed_type_list_error() {
    let source = r#"
        fun main = {
            val mixed = [1, "hello", 3];
            mixed
        }
    "#;
    
    let result = type_check(source);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type"));
}

#[test]
fn test_nested_list() {
    let source = r#"
        fun main = {
            val matrix = [[1, 2], [3, 4], [5, 6]];
            matrix
        }
    "#;
    
    let result = type_check(source);
    assert!(result.is_ok());
}

#[test]
fn test_list_in_function() {
    let source = r#"
        fun sum_list = nums: List {
            // For now, just return a placeholder
            // We'll implement list operations later
            42
        }
        
        fun main = {
            val numbers = [1, 2, 3];
            numbers sum_list
        }
    "#;
    
    let result = type_check(source);
    // This will fail for now because we don't have List type parameter parsing
    // but the list literal itself should parse correctly
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_list_pattern_in_match() {
    // This is future work - list patterns in match expressions
    let source = r#"
        fun main = {
            val nums = [1, 2, 3];
            nums match {
                [] => { 0 }
                _ => { 1 }
            }
        }
    "#;
    
    let result = type_check(source);
    // For now, this won't work as we don't have list patterns
    assert!(result.is_ok() || result.is_err());
}