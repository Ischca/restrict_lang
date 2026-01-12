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
fn test_simple_list_literal() {
    let source = r#"
        fun main: () -> Int = {
            val list = [1, 2, 3];
            list
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that list allocation happens
    assert!(wat.contains("List literal with 3 elements"));
    assert!(wat.contains("call $allocate"));
    assert!(wat.contains("local.set $list_tmp"));
    
    // Check header initialization
    assert!(wat.contains("i32.const 3")); // length
    assert!(wat.contains("i32.store")); // store length
}

#[test]
fn test_empty_list() {
    let source = r#"
        fun main: () -> Int = {
            val empty = [];
            empty
        }
    "#;
    
    let result = compile(source);
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that empty list is handled
    assert!(wat.contains("List literal with 0 elements"));
    assert!(wat.contains("call $allocate"));
}

#[test]
fn test_list_in_expression() {
    let source = r#"
        fun main: () -> Int = {
            mut val x = 10;
            val list = [x, x + 1, x + 2];
            list
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that expressions in list elements work
    assert!(wat.contains("List literal with 3 elements"));
    assert!(wat.contains("i32.add")); // for x + 1 and x + 2
}