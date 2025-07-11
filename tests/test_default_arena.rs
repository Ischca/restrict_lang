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
fn test_default_arena_in_main() {
    let source = r#"
        fun main = {
            // Default arena should be available automatically
            42
        }
    "#;
    
    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that default arena is initialized in main
    assert!(wat.contains("Initialize default arena"));
    assert!(wat.contains("call $arena_init"));
    assert!(wat.contains("global.set $current_arena"));
    
    // Check that default arena is reset at the end
    assert!(wat.contains("Reset default arena"));
    assert!(wat.contains("call $arena_reset"));
}

#[test]
fn test_no_default_arena_in_other_functions() {
    let source = r#"
        fun helper = x: Int {
            x + 1
        }
        
        fun main = {
            42 helper
        }
    "#;
    
    let result = compile(source);
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that only main has arena initialization
    let main_start = wat.find("(func $main").unwrap();
    let helper_start = wat.find("(func $helper").unwrap();
    
    let main_section = &wat[main_start..];
    let helper_section = &wat[helper_start..main_start];
    
    assert!(main_section.contains("Initialize default arena"));
    assert!(!helper_section.contains("Initialize default arena"));
}

#[test]
fn test_global_current_arena() {
    let source = r#"
        fun main = {
            42
        }
    "#;
    
    let result = compile(source);
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that global $current_arena is declared
    assert!(wat.contains("(global $current_arena (mut i32)"));
}

#[test]
fn test_allocate_function() {
    let source = r#"
        fun main = {
            42
        }
    "#;
    
    let result = compile(source);
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that allocate function is generated
    assert!(wat.contains("(func $allocate"));
    assert!(wat.contains("global.get $current_arena"));
    assert!(wat.contains("call $arena_alloc"));
}

// This test will be enabled once we implement list allocation
#[test]
#[ignore]
fn test_list_with_default_arena() {
    let source = r#"
        fun main = {
            val list = [1, 2, 3, 4, 5];
            list
        }
    "#;
    
    let result = compile(source);
    assert!(result.is_ok());
    let wat = result.unwrap();
    
    // Check that list allocation uses the allocate function
    assert!(wat.contains("call $allocate"));
}