use restrict_lang::parse_program;

#[test]
fn test_minimal_record_in_block() {
    // Test 1: Simplest case - just record construction
    let test1 = "fun test = { File { handle: 1 } }";
    println!("Test 1: {}", test1);
    match parse_program(test1) {
        Ok((rem, prog)) => {
            if rem.len() > 0 {
                println!("✗ Partial parse: {} decls, {} chars remaining", 
                    prog.declarations.len(), rem.len());
                println!("  Remaining: {:?}", &rem[..30.min(rem.len())]);
            } else {
                println!("✓ Success: {} decls", prog.declarations.len());
            }
        }
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 2: Empty record construction
    let test2 = "fun test = { File {} }";
    println!("\nTest 2: {}", test2);
    match parse_program(test2) {
        Ok((rem, prog)) => {
            if rem.len() > 0 {
                println!("✗ Partial parse: {} decls, {} chars remaining", 
                    prog.declarations.len(), rem.len());
            } else {
                println!("✓ Success: {} decls", prog.declarations.len());
            }
        }
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 3: Record construction as statement (with semicolon)
    let test3 = "fun test = { File { handle: 1 }; }";
    println!("\nTest 3: {}", test3);
    match parse_program(test3) {
        Ok((rem, prog)) => {
            if rem.len() > 0 {
                println!("✗ Partial parse: {} decls, {} chars remaining", 
                    prog.declarations.len(), rem.len());
            } else {
                println!("✓ Success: {} decls", prog.declarations.len());
            }
        }
        Err(e) => println!("✗ Failed: {:?}", e),
    }
    
    // Test 4: Record with explicit return
    let test4 = "fun test = { val x = File { handle: 1 }; x }";
    println!("\nTest 4: {}", test4);
    match parse_program(test4) {
        Ok((rem, prog)) => {
            if rem.len() > 0 {
                println!("✗ Partial parse: {} decls, {} chars remaining", 
                    prog.declarations.len(), rem.len());
            } else {
                println!("✓ Success: {} decls", prog.declarations.len());
            }
        }
        Err(e) => println!("✗ Failed: {:?}", e),
    }
}