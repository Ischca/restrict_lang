#[cfg(test)]
mod tests {
    use restrict_lang::parse_program;

    #[test]
    fn debug_parse() {
    let input = r#"record File<~f> {
        path: String,
        content: String
    }
    
    fun main: () -> Int = {
        with lifetime<~f> {
            val file = File { path = "test.txt", content = "data" };
            file.content
        }
    }"#;
    
    let result = parse_program(input);
    match result {
        Ok((remaining, program)) => {
            println!("Remaining input: {:?}", remaining);
            println!("Program: {:#?}", program);
            println!("Number of declarations: {}", program.declarations.len());
            for (i, decl) in program.declarations.iter().enumerate() {
                println!("Declaration {}: {:?}", i, decl);
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
    }
}