use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

#[test]
fn test_exact_input_parsing() {
    let input = r#"record Point<~p> {
    x: Int32
    y: Int32
}

fun main: () -> Int = {
    with lifetime<~outer> {
        val p1 = Point { x = 10, y = 20 };
        
        with lifetime<~inner> {
            val p2 = Point { x = 30, y = 40 };
            p2.x
        };
        
        p1.x
    }
}"#;
    
    match parse_program(input) {
        Ok((rem, prog)) => {
            println!("Parse success!");
            println!("Remaining input length: {}", rem.len());
            println!("Parsed {} declarations", prog.declarations.len());
            for (i, decl) in prog.declarations.iter().enumerate() {
                match decl {
                    restrict_lang::TopDecl::Function(f) => {
                        println!("  [{}] Function: {}", i, f.name);
                    }
                    restrict_lang::TopDecl::Record(r) => {
                        println!("  [{}] Record: {}", i, r.name);
                    }
                    _ => {
                        println!("  [{}] Other", i);
                    }
                }
            }
            
            // Should have 2 declarations
            assert_eq!(prog.declarations.len(), 2);
        }
        Err(e) => {
            panic!("Parse failed: {:?}", e);
        }
    }
}

#[test]
fn test_simple_parsing() {
    let input = r#"record Point {
    x: Int32
    y: Int32
}

fun main: () -> Int = {
    42
}"#;
    
    match parse_program(input) {
        Ok((rem, prog)) => {
            println!("Parse success!");
            println!("Remaining input length: {}", rem.len());
            println!("Parsed {} declarations", prog.declarations.len());
            
            // Should have 2 declarations
            assert_eq!(prog.declarations.len(), 2);
        }
        Err(e) => {
            panic!("Parse failed: {:?}", e);
        }
    }
}