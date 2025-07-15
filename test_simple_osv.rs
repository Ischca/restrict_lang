use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn main() {
    let source = r#"
        fun double = x: Int {
            x * 2
        }
        
        fun main = {
            (21) double
        }
    "#;
    
    // Parse
    match parse_program(source) {
        Ok((remaining, ast)) => {
            println!("Parse succeeded!");
            println!("Remaining: {:?}", remaining);
            println!("AST: {:#?}", ast);
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}