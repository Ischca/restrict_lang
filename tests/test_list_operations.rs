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
fn test_list_length() {
    let source = r#"
        fun main = {
            val list = [1, 2, 3, 4, 5];
            val len = list list_length;
            len
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that list_length function is called
    assert!(wat.contains("call $list_length"));
}

#[test]
fn test_list_get() {
    let source = r#"
        fun main = {
            with Arena {
                val list = [10, 20, 30, 40, 50];
                val second = (list, 1) list_get;  // Get element at index 1
                second
            }
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that list_get function is called
    assert!(wat.contains("call $list_get"));
}

#[test]
fn test_list_operations_combined() {
    let source = r#"
        fun main = {
            with Arena {
                mut val nums = [5, 10, 15, 20];
                val len = nums list_length;
                val first = (nums, 0) list_get;
                val last_idx = len - 1;
                mut val last = (nums, last_idx) list_get;
                first + last
            }
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that both functions are used
    assert!(wat.contains("call $list_length"));
    assert!(wat.contains("call $list_get"));
}

#[test]
fn test_empty_list_length() {
    let source = r#"
        fun main = {
            val empty = [];
            val len = empty list_length;
            len
        }
    "#;
    
    let result = compile(source);
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that empty list works with list_length
    assert!(wat.contains("List literal with 0 elements"));
    assert!(wat.contains("call $list_length"));
}