use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile_to_wat(source: &str) -> Result<String, String> {
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

    // Generate WASM
    let mut codegen = WasmCodeGen::new();
    codegen
        .generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

fn assert_valid_wat(name: &str, source: &str) -> String {
    let wat = compile_to_wat(source).unwrap_or_else(|err| {
        panic!("{name} should compile before WAT validation: {err}");
    });

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("{name} generated invalid WAT: {err}\n\n{wat}");
    });

    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("{name} generated invalid Wasm binary: {err}\n\n{wat}");
        });

    wat
}

#[test]
fn test_string_literal() {
    let source = r#"
        fun main: () -> String = {
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
        fun main: () = {
            "Hello, World!" |> println
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
        fun main: () = {
            val s1 = "First string";
            val s2 = "Second string";
            s1 |> println;
            s2 |> println
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
        fun greet: (name: String) -> () = {
            name |> println
        }

        fun main: () = {
            "Alice" |> greet
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
        fun main: () = {
            "" |> println
        }
    "#;

    let wat = compile_to_wat(source).unwrap();

    // Empty string should still be handled
    assert!(wat.contains("\\00\\00\\00\\00")); // Length 0
}

#[test]
fn test_string_concatenation() {
    let source = r#"
        fun greet: (name: String) -> String = {
            "Hello, " + name
        }

        fun main: () = {
            "Alice" |> greet |> println
        }
    "#;

    let wat = assert_valid_wat("string_concatenation", source);

    assert!(wat.contains("call $string_concat"));
    assert!(wat.contains("Hello, "));
    assert!(wat.contains("Alice"));
}

#[test]
fn test_string_equality_uses_content_comparison() {
    let source = r#"
        fun main: () -> Int32 = {
            val same = "service " + "down" == "service down";
            val different = "service up" != ("service " + "down");
            val ready = same && different;
            ready then {
                1
            } else {
                0
            }
        }
    "#;

    let wat = assert_valid_wat("string_equality", source);

    assert!(wat.matches("call $string_eq").count() >= 2);
    assert!(wat.contains("i32.eqz"));
}
