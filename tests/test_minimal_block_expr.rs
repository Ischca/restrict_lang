use restrict_lang::parse_program;

#[test]
fn test_minimal_block_expressions() {
    // Test 1: Empty block
    let test1 = "fun test = { }";
    println!("Test 1 - Empty block: {}", test1);
    match parse_program(test1) {
        Ok((rem, prog)) => println!("✓ Success: {} decls", prog.declarations.len()),
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 2: Block with Unit
    let test2 = "fun test = { Unit }";
    println!("\nTest 2 - Block with Unit: {}", test2);
    match parse_program(test2) {
        Ok((rem, prog)) => println!("✓ Success: {} decls", prog.declarations.len()),
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 3: Block with val binding
    let test3 = "fun test = { val x = 1; x }";
    println!("\nTest 3 - Block with val: {}", test3);
    match parse_program(test3) {
        Ok((rem, prog)) => println!("✓ Success: {} decls", prog.declarations.len()),
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 4: Block with record construction
    let test4 = "fun test = { File { handle: 1 } }";
    println!("\nTest 4 - Block with record: {}", test4);
    match parse_program(test4) {
        Ok((rem, prog)) => println!("✓ Success: {} decls", prog.declarations.len()),
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 5: Block with val and record
    let test5 = "fun test = { val file = File { handle: 1 }; file }";
    println!("\nTest 5 - Block with val and record: {}", test5);
    match parse_program(test5) {
        Ok((rem, prog)) => println!("✓ Success: {} decls", prog.declarations.len()),
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 6: Block with newline after {
    let test6 = r#"fun test = {
    Unit
}"#;
    println!("\nTest 6 - Block with newline: {}", test6.replace('\n', "\\n"));
    match parse_program(test6) {
        Ok((rem, prog)) => println!("✓ Success: {} decls", prog.declarations.len()),
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 7: Original problematic case
    let test7 = r#"fun test = {
    val file = File { handle: 1 };
    file
}"#;
    println!("\nTest 7 - Original case: {}", test7.replace('\n', "\\n"));
    match parse_program(test7) {
        Ok((rem, prog)) => {
            if rem.len() > 0 {
                println!("⚠ Parsed but has {} remaining chars", rem.len());
            } else {
                println!("✓ Success: {} decls", prog.declarations.len());
            }
        }
        Err(e) => println!("✗ Failed: {:?}", e),
    }
}