use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn main() {
    let input = r#"
    fun factorial = n: Int32 {
        if n <= 1 then 1
        else n * factorial(n - 1)
    }

    fun main() {
        factorial(5) |> println;
    }"#;
    
    println!("=== Parsing ===");
    let (remaining, program) = match parse_program(input) {
        Ok(result) => {
            println!("Parse successful, remaining: {:?}", remaining);
            result
        },
        Err(e) => {
            println!("Parse failed: {}", e);
            return;
        }
    };
    
    println!("Program: {:#?}", program);
    
    println!("\n=== Type Checking ===");
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {
            println!("Type checking successful");
            println!("Expression types: {:#?}", checker.expr_types);
        },
        Err(e) => {
            println!("Type check failed: {}", e);
            return;
        }
    }
    
    println!("\n=== Code Generation ===");
    let mut codegen = WasmCodeGen::new();
    codegen.expr_types = checker.expr_types.clone();
    
    match codegen.generate(&program) {
        Ok(wat_code) => {
            println!("Code generation successful");
            println!("Generated WAT:");
            println!("{}", wat_code);
        },
        Err(e) => {
            println!("Code generation failed: {}", e);
        }
    }
}