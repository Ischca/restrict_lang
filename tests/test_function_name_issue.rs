use restrict_lang::parse_program;

#[test]
fn test_different_function_names() {
    let names = vec![
        ("test", true),
        ("foo", true),
        ("myFunc", true),
        ("leak", true),
        ("leakF", true),
        ("leakFi", true),
        ("leakFil", true),
        ("leakFile", false),  // This is the problematic one
        ("leakFiles", true),
        ("fileLeaker", true),
    ];
    
    for (name, should_work) in names {
        let input = format!("fun {}<~io> = {{ Unit }}", name);
        println!("\nTesting function name '{}': {}", name, input);
        
        match parse_program(&input) {
            Ok((rem, prog)) => {
                if rem.len() > 0 {
                    println!("  ✗ Parsed but has remaining: {} chars", rem.len());
                    assert!(!should_work, "Function '{}' should have failed but didn't", name);
                } else {
                    println!("  ✓ Success: {} declarations", prog.declarations.len());
                    assert!(should_work, "Function '{}' should have worked but failed", name);
                }
            }
            Err(e) => {
                println!("  ✗ Failed: {:?}", e);
                assert!(!should_work, "Function '{}' should have worked but failed", name);
            }
        }
    }
}

#[test]
fn test_leakfile_variations() {
    // Without temporal parameter
    let without_temporal = "fun leakFile = { Unit }";
    println!("Without temporal: {}", without_temporal);
    match parse_program(without_temporal) {
        Ok((rem, prog)) => {
            println!("Result: {} decls, {} remaining", prog.declarations.len(), rem.len());
            if rem.len() > 0 {
                println!("Remaining: {:?}", rem);
            }
        }
        Err(e) => println!("Failed: {:?}", e),
    }
    
    // With different temporal name
    let different_temporal = "fun leakFile<~t> = { Unit }";
    println!("\nWith ~t instead of ~io: {}", different_temporal);
    match parse_program(different_temporal) {
        Ok((rem, prog)) => {
            println!("Result: {} decls, {} remaining", prog.declarations.len(), rem.len());
            if rem.len() > 0 {
                println!("Remaining: {:?}", rem);
            }
        }
        Err(e) => println!("Failed: {:?}", e),
    }
}