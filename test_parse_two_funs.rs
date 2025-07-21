use restrict_lang::parser::parse_program;

fn main() {
    let input = r#"
fun process<~p> = {
    with lifetime<~local> {
        val temp = 100;
        temp + 42
    }
}

fun main = {
    with lifetime<~main> {
        process()
    }
}"#;

    match parse_program(input) {
        Ok((remaining, program)) => {
            println!("Successfully parsed!");
            println!("Number of declarations: {}", program.declarations.len());
            println!("Remaining input: {:?}", remaining);
            println!("Remaining length: {}", remaining.len());
            
            for (i, decl) in program.declarations.iter().enumerate() {
                println!("Declaration {}: {:?}", i, decl);
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}