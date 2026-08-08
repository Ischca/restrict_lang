use restrict_lang::{generate, parse_program, TypeChecker};

fn compile(source: &str) -> Result<String, String> {
    // Parse
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;

    // Generate code
    generate(&ast).map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_simple_list_literal() {
    let source = r#"
        fun main: () -> List<Int32> = {
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
    assert!(wat.contains("i32.const 20 ;; list size"));
    assert!(wat.contains("call $allocate"));
    assert!(wat.contains("local.set $list_tmp"));

    // Check header initialization
    assert!(wat.contains("i32.const 3 ;; length"));
    assert!(wat.contains("i32.const 3 ;; capacity"));
    assert!(wat.contains("i32.store")); // store length
}

#[test]
fn test_empty_list() {
    let source = r#"
        fun main: () -> List<Int32> = {
            val empty = [];
            empty
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok());
    let wat = result.unwrap();

    // Check that empty list is handled
    assert!(wat.contains("i32.const 8 ;; list size"));
    assert!(wat.contains("i32.const 0 ;; length"));
    assert!(wat.contains("i32.const 0 ;; capacity"));
    assert!(wat.contains("call $allocate"));
}

#[test]
fn test_list_in_expression() {
    let source = r#"
        fun main: () -> List<Int32> = {
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
    assert!(wat.contains("i32.const 20 ;; list size"));
    assert!(wat.contains("i32.add")); // for x + 1 and x + 2
}
