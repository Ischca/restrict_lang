use restrict_lang::{parse_program, lexer::lex};

#[test]
fn test_parse_in_parts() {
    // Part 1: Just the record
    let part1 = r#"record File<~f> {
    handle: Int32
}"#;
    
    println!("=== Part 1: Just record ===");
    match parse_program(part1) {
        Ok((rem, prog)) => {
            println!("Success: {} decls, {} remaining", prog.declarations.len(), rem.len());
        }
        Err(e) => println!("Failed: {:?}", e),
    }
    
    // Part 2: Just the function
    let part2 = r#"fun leakFile<~io> = {
    val file = File { handle: 1 };  // file: File<~io>
    file  // ERROR: Cannot return File<~io> outside ~io
}"#;
    
    println!("\n=== Part 2: Just function (with comments) ===");
    match parse_program(part2) {
        Ok((rem, prog)) => {
            println!("Success: {} decls, {} remaining", prog.declarations.len(), rem.len());
            if rem.len() > 0 {
                println!("Remaining: {:?}", &rem[..30.min(rem.len())]);
            }
        }
        Err(e) => println!("Failed: {:?}", e),
    }
    
    // Part 3: Combined with single newline
    let part3 = r#"record File<~f> {
    handle: Int32
}
fun leakFile<~io> = {
    val file = File { handle: 1 };  // file: File<~io>
    file  // ERROR: Cannot return File<~io> outside ~io
}"#;
    
    println!("\n=== Part 3: Combined (single newline) ===");
    match parse_program(part3) {
        Ok((rem, prog)) => {
            println!("Success: {} decls, {} remaining", prog.declarations.len(), rem.len());
            if rem.len() > 0 {
                // Show exactly where parsing stopped
                let parsed_len = part3.len() - rem.len();
                println!("Parsed up to position: {}", parsed_len);
                let context_start = parsed_len.saturating_sub(10);
                let context_end = (parsed_len + 10).min(part3.len());
                println!("Context: '{}'[HERE]'{}'", 
                    &part3[context_start..parsed_len],
                    &part3[parsed_len..context_end]);
            }
        }
        Err(e) => println!("Failed: {:?}", e),
    }
    
    // Part 4: Original with double newline
    let part4 = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = {
    val file = File { handle: 1 };  // file: File<~io>
    file  // ERROR: Cannot return File<~io> outside ~io
}"#;
    
    println!("\n=== Part 4: Original (double newline) ===");
    match parse_program(part4) {
        Ok((rem, prog)) => {
            println!("Success: {} decls, {} remaining", prog.declarations.len(), rem.len());
            if rem.len() > 0 {
                let parsed_len = part4.len() - rem.len();
                println!("Parsed up to position: {}", parsed_len);
            }
        }
        Err(e) => println!("Failed: {:?}", e),
    }
}