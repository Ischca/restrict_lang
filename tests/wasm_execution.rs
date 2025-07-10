use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use std::process::Command;
use std::fs;
use tempfile::NamedTempFile;

fn compile_and_run(source: &str) -> Result<i32, Box<dyn std::error::Error>> {
    // Parse
    let (_, ast) = parse_program(source)?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)?;
    
    // Generate WASM
    let mut codegen = WasmCodeGen::new();
    let wat = codegen.generate(&ast)?;
    
    // Write WAT to temp file
    let wat_file = NamedTempFile::new()?.with_suffix(".wat");
    fs::write(wat_file.path(), &wat)?;
    
    // Convert to WASM
    let wasm_file = NamedTempFile::new()?.with_suffix(".wasm");
    let wat2wasm_output = Command::new("wat2wasm")
        .arg(wat_file.path())
        .arg("-o")
        .arg(wasm_file.path())
        .output()?;
    
    if !wat2wasm_output.status.success() {
        return Err(format!("wat2wasm failed: {}", 
            String::from_utf8_lossy(&wat2wasm_output.stderr)).into());
    }
    
    // Run with wasmtime
    let wasmtime_output = Command::new("wasmtime")
        .arg(wasm_file.path())
        .output()?;
    
    if !wasmtime_output.status.success() {
        return Err(format!("wasmtime failed: {}", 
            String::from_utf8_lossy(&wasmtime_output.stderr)).into());
    }
    
    // The exit code is the return value of main
    Ok(wasmtime_output.status.code().unwrap_or(-1))
}

#[test]
#[ignore] // Ignore by default as it requires external tools
fn test_simple_addition() {
    let source = r#"
        fun main = {
            10 + 20
        }
    "#;
    
    let result = compile_and_run(source).unwrap();
    assert_eq!(result, 30);
}

#[test]
#[ignore]
fn test_function_call() {
    let source = r#"
        fun add = a: Int b: Int {
            a + b
        }
        
        fun main = {
            (15, 25) add
        }
    "#;
    
    let result = compile_and_run(source).unwrap();
    assert_eq!(result, 40);
}

#[test]
#[ignore]
fn test_arithmetic_operations() {
    let source = r#"
        fun main = {
            val a = 100
            val b = 50
            val c = 10
            a - b + c
        }
    "#;
    
    let result = compile_and_run(source).unwrap();
    assert_eq!(result, 60);
}

#[test]
#[ignore]
fn test_comparison() {
    let source = r#"
        fun main = {
            val result = 10 < 20
            result then { 1 } else { 0 }
        }
    "#;
    
    let result = compile_and_run(source).unwrap();
    assert_eq!(result, 1);
}