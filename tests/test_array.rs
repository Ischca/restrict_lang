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
fn test_array_literal() {
    let source = r#"
        fun main = {
            val arr = [|1, 2, 3, 4, 5|];
            arr
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that array allocation happens
    assert!(wat.contains("Array literal with 5 elements"));
    assert!(wat.contains("call $allocate"));
    
    // Check that no list-style header is written
    // Arrays should directly store elements without length/capacity fields
}

#[test]
fn test_empty_array() {
    let source = r#"
        fun main = {
            val empty = [||];
            empty
        }
    "#;
    
    let result = compile(source);
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that empty array returns null
    assert!(wat.contains("Array literal with 0 elements"));
    assert!(wat.contains("i32.const 0")); // Null pointer for empty array
}

#[test]
fn test_array_get() {
    let source = r#"
        fun main = {
            val arr = [|10, 20, 30, 40, 50|];
            val third = array_get arr 2;
            third
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that array_get function is called
    assert!(wat.contains("call $array_get"));
}

#[test]
fn test_array_set() {
    let source = r#"
        fun main = {
            mut val arr = [|10, 20, 30, 40, 50|];
            array_set arr 2 35;
            val third = array_get arr 2;
            third
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that array_set function is called
    assert!(wat.contains("call $array_set"));
}

#[test]
fn test_array_vs_list() {
    let source = r#"
        fun main = {
            val list = [1, 2, 3];      // List literal
            val arr = [|1, 2, 3|];     // Array literal
            
            val list_len = list list_length;
            val arr_first = array_get arr 0;
            
            list_len + arr_first
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that both list and array literals are generated
    assert!(wat.contains("List literal with 3 elements"));
    assert!(wat.contains("Array literal with 3 elements"));
}