use restrict_lang::parse_program;

#[test]
fn test_two_simple_records() {
    let input = r#"record A {
    x: Int32
}

record B {
    y: Int32
}"#;
    
    eprintln!("Input:\n{}", input);
    
    match parse_program(input) {
        Ok((remaining, program)) => {
            eprintln!("\nParsed {} declarations", program.declarations.len());
            eprintln!("Remaining: {} chars", remaining.len());
            
            for (i, decl) in program.declarations.iter().enumerate() {
                match decl {
                    restrict_lang::TopDecl::Record(r) => {
                        eprintln!("Declaration {}: Record '{}'", i, r.name);
                    }
                    _ => {
                        eprintln!("Declaration {}: Other", i);
                    }
                }
            }
            
            assert_eq!(program.declarations.len(), 2, "Should parse both records");
            assert!(remaining.trim().is_empty(), "Should parse all input");
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}