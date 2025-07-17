use restrict_lang::parse_program;

#[test]
fn test_exact_parser_issue() {
    // The exact input from the failing test
    let input = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = {
    val file = File { handle: 1 };  // file: File<~io>
    file  // ERROR: Cannot return File<~io> outside ~io
}

fun main = {
    Unit
}"#;
    
    println!("=== Testing exact input ===");
    println!("Input length: {} chars", input.len());
    
    match parse_program(input) {
        Ok((remaining, program)) => {
            println!("\nParse result:");
            println!("- Parsed {} declarations", program.declarations.len());
            println!("- Remaining: {} chars", remaining.len());
            
            for (i, decl) in program.declarations.iter().enumerate() {
                match decl {
                    restrict_lang::TopDecl::Function(f) => {
                        println!("- Declaration {}: Function '{}' with {} params", i, f.name, f.params.len());
                    }
                    restrict_lang::TopDecl::Record(r) => {
                        println!("- Declaration {}: Record '{}' with {} fields", i, r.name, r.fields.len());
                    }
                    _ => {
                        println!("- Declaration {}: Other", i);
                    }
                }
            }
            
            if remaining.len() > 0 {
                println!("\n⚠️  WARNING: Not all input was parsed!");
                println!("Remaining content starts at position {}", input.len() - remaining.len());
                
                // Find the character position where parsing stopped
                let stop_pos = input.len() - remaining.len();
                let lines: Vec<&str> = input.lines().collect();
                let mut char_count = 0;
                let mut line_num = 0;
                let mut col_num = 0;
                
                for (i, line) in lines.iter().enumerate() {
                    if char_count + line.len() + 1 > stop_pos {
                        line_num = i + 1;
                        col_num = stop_pos - char_count + 1;
                        break;
                    }
                    char_count += line.len() + 1; // +1 for newline
                }
                
                println!("Parsing stopped at line {}, column {}", line_num, col_num);
                println!("Context: {:?}", &remaining[..50.min(remaining.len())]);
            }
            
            // This should pass for the test to succeed
            assert_eq!(remaining.len(), 0, "Parser should consume all input");
        }
        Err(e) => {
            panic!("Parse completely failed: {:?}", e);
        }
    }
}