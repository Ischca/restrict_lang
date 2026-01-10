#[test]
fn test_debug_pattern() {
    use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
    
    let source = r#"
        fun test_option = opt: Int? {
            opt match {
                Some(n) => { n }
                None => { 0 }
            }
        }
        
        fun main: () -> Int = {
            val some_val = 42 some;
            val none_val = None<Int>;
            some_val test_option |> print_int;
            none_val test_option |> print_int
        }
    "#;
    
    // Parse
    match parse_program(source) {
        Ok((_, ast)) => {
            println!("Parse succeeded!");
            
            // Type check
            let mut type_checker = TypeChecker::new();
            match type_checker.check_program(&ast) {
                Ok(_) => {
                    println!("Type check succeeded!");
                    
                    // Generate WASM
                    let mut codegen = WasmCodeGen::new();
                    match codegen.generate(&ast) {
                        Ok(wat) => {
                            println!("Code generation succeeded!");
                            println!("Generated WAT:\n{}", wat);
                        }
                        Err(e) => {
                            println!("Codegen error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Type error: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}