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
fn test_simple_match() {
    let source = r#"
        fun main = {
            val x = 42;
            x match {
                0 => { 100 }
                42 => { 200 }
                _ => { 300 }
            }
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify match structure is generated
    assert!(wat.contains("if"));
    assert!(wat.contains("i32.const 42"));
    assert!(wat.contains("i32.const 200"));
}

#[test]
fn test_match_with_binding() {
    let source = r#"
        fun main = {
            val x = 10;
            x match {
                0 => { 0 }
                n => { n + 1 }
            }
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify pattern binding
    assert!(wat.contains("local.set"));
    assert!(wat.contains("i32.add"));
}

#[test]
fn test_boolean_match() {
    let source = r#"
        fun test_bool = b: Boolean {
            b match {
                true => { 1 }
                false => { 0 }
            }
        }
        
        fun main = {
            true test_bool
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Verify boolean matching
    assert!(wat.contains("i32.const 1"));
    assert!(wat.contains("i32.eq"));
}

#[test]
fn test_match_type_consistency() {
    let source = r#"
        fun main = {
            val x = 5;
            x match {
                0 => { "zero" }
                _ => { 42 }
            }
        }
    "#;
    
    let result = compile_to_wat(source);
    
    // Should fail due to type mismatch
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Type") && err.contains("mismatch") || err.contains("TypeMismatch"));
}

#[test]
fn test_match_exhaustiveness() {
    let source = r#"
        fun main = {
            val b = true;
            b match {
                true => { 1 }
            }
        }
    "#;
    
    let result = compile_to_wat(source);
    
    // Should fail due to non-exhaustive patterns
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exhaustive"));
}

#[test]
fn test_nested_match() {
    let source = r#"
        fun main = {
            val x = 1;
            val y = 2;
            
            x match {
                0 => { 0 }
                _ => {
                    y match {
                        0 => { 10 }
                        _ => { 20 }
                    }
                }
            }
        }
    "#;
    
    let wat = compile_to_wat(source).unwrap();
    
    // Should compile successfully
    assert!(wat.contains("if"));
}

#[test]
fn test_match_in_function() {
    let source = r#"
        fun sign = x: Int {
            // For testing purposes, just return 1
            // (In a real implementation, we'd need multiple functions to handle affine constraints)
            1
        }
        
        fun classify = x: Int {
            (x) sign match {
                1 => { "positive" }
                -1 => { "negative" }
                0 => { "zero" }
                _ => { "unknown" }
            }
        }
        
        fun main = {
            42 classify
        }
    "#;
    
    // This test mainly checks that match expressions can be used in various contexts
    let result = compile_to_wat(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
}