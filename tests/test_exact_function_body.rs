use restrict_lang::parse_program;

#[test]
fn test_function_body_problems() {
    // Test 1: Simple function with temporal
    let test1 = "fun test<~io> = { Unit }";
    println!("Test 1: {}", test1);
    match parse_program(test1) {
        Ok((_, prog)) => println!("✓ Success: {} decls", prog.declarations.len()),
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 2: Function with val binding
    let test2 = r#"fun test<~io> = {
    val x = 1;
    x
}"#;
    println!("\nTest 2: Function with val binding");
    match parse_program(test2) {
        Ok((_, prog)) => println!("✓ Success: {} decls", prog.declarations.len()),
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 3: Exact leakFile body (without record)
    let test3 = r#"fun leakFile<~io> = {
    val file = File { handle: 1 };
    file
}"#;
    println!("\nTest 3: Exact leakFile body");
    match parse_program(test3) {
        Ok((rem, prog)) => {
            println!("Success: {} decls, {} remaining", prog.declarations.len(), rem.len());
            if rem.len() > 0 {
                println!("Remaining: {:?}", rem);
            }
        }
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 4: With semicolon after val
    let test4 = r#"fun leakFile<~io> = {
    val file = File { handle: 1 };  // This semicolon
    file
}"#;
    println!("\nTest 4: With comment after semicolon");
    match parse_program(test4) {
        Ok((rem, prog)) => {
            println!("Success: {} decls, {} remaining", prog.declarations.len(), rem.len());
            if rem.len() > 0 {
                println!("Remaining: {:?}", rem);
            }
        }
        Err(e) => println!("✗ Failed: {:?}", e),
    }
}