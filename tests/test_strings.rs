use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile_to_wat(source: &str) -> Result<String, String> {
    // Parse
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    
    // Generate WASM
    let mut codegen = WasmCodeGen::new();
    codegen.generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_string_literal() {
    let source = r#"
        fun main: () -> Int = {
            val message = "Hello, World!";
            message
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify string constant is generated
    assert!(wat.contains("Hello, World!"));
    // Verify string pointer is loaded
    assert!(wat.contains("i32.const 1024")); // Or whatever offset was allocated
}

#[test]
fn test_string_println() {
    let source = r#"
        fun main: () -> Int = {
            "Hello, World!" println
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify println function is called
    assert!(wat.contains("call $println"));
    // Verify fd_write is imported
    assert!(wat.contains("fd_write"));
}

#[test]
fn test_multiple_strings() {
    let source = r#"
        fun main: () -> Int = {
            val s1 = "First string";
            val s2 = "Second string";
            s1 println;
            s2 println
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify both strings are in constant pool
    assert!(wat.contains("First string"));
    assert!(wat.contains("Second string"));
}

#[test]
fn test_string_in_function() {
    let source = r#"
        fun greet: (name: String) -> Unit = {
            name println
        }

        fun main: () -> Int = {
            ("Alice") greet;
            0
        }
    "#;

    let wat = compile_to_wat(source).unwrap();

    // Verify function call and string passing
    assert!(wat.contains("call $greet"));
    assert!(wat.contains("Alice"));
}

#[test]
fn test_empty_string() {
    let source = r#"
        fun main: () -> Int = {
            "" println
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Empty string should still be handled
    assert!(wat.contains("\\00\\00\\00\\00")); // Length 0
}