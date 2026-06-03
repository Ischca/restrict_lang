use restrict_lang::{generate, parse_program, TypeChecker};

fn type_check(source: &str) -> Result<(), String> {
    // Parse
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))
}

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
fn test_some_constructor() {
    let source = r#"
        fun main = {
            val x = Some(42);
            x
        }
    "#;

    let result = type_check(source);
    assert!(result.is_ok());
}

#[test]
fn test_none_constructor() {
    let source = r#"
        fun main: () -> Option<Int32> = {
            val x: Option<Int32> = None;
            x
        }
    "#;

    let result = type_check(source);
    assert!(result.is_ok());
}

#[test]
fn test_option_match() {
    let source = r#"
        fun unwrap_or: (opt: Option<Int32>, default: Int32) -> Int32 = {
            opt match {
                Some(n) => { n }
                None => { default }
            }
        }
        
        fun main: () -> Int32 = {
            val x = Some(42);
            (x, 0) unwrap_or
        }
    "#;

    let result = type_check(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
}

#[test]
fn test_option_exhaustiveness() {
    let source = r#"
        fun main = {
            val x = Some(42);
            x match {
                Some(n) => { n }
                // Missing None case
            }
        }
    "#;

    let result = type_check(source);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exhaustive"));
}

#[test]
fn test_nested_option() {
    let source = r#"
        fun main = {
            val x = Some(Some(42));
            x match {
                Some(inner) => {
                    inner match {
                        Some(n) => { n }
                        None => { 0 }
                    }
                }
                None => { 0 }
            }
        }
    "#;

    let result = type_check(source);
    assert!(result.is_ok());
}

#[test]
fn test_option_type_mismatch() {
    let source = r#"
        fun main = {
            val x = Some(42);
            x match {
                Some(s) => { s } // s is Int32
                None => { "hello" } // Type mismatch
            }
        }
    "#;

    let result = type_check(source);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type"));
}

#[test]
fn test_safe_divide() {
    let source = r#"
        fun safe_divide: (a: Int32, b: Int32) -> Option<Int32> = {
            b == 0 then {
                None
            } else {
                Some(a / b)
            }
        }

        fun main: () -> Int32 = {
            val result = (10, 2) safe_divide;
            result match {
                Some(value) => { value }
                None => { 0 }
            }
        }
    "#;

    let result = type_check(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
}

#[test]
fn test_option_code_generation() {
    let source = r#"
        fun main = {
            val x = Some(42);
            x match {
                Some(n) => { n }
                None => { 0 }
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok());
    let wat = result.unwrap();
    // Check for Some tag (1) and None tag (0)
    assert!(wat.contains("i32.const 1"));
    assert!(wat.contains("i32.const 0"));
}

#[test]
fn some_string_constructor_generates_valid_wat() {
    let source = r#"
fun main: () -> Option<String> = {
    Some("ok")
}
"#;

    let wat = compile(source).expect("Some(String) should compile");
    assert!(wat.contains("ok"));

    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("Some(String) WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("Some(String) Wasm should validate: {err}\n\n{wat}"));
}
